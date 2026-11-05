//! Stacked pull requests: `mast pr stack [pr]`, `mast pr stack push`, `mast pr stack sync` and
//! `mast pr stack merge`.
//!
//! A pull request is a layer when its base branch is the head branch of another open pull
//! request in the same repository. `view` and `push` work with plain Forgejo's API too;
//! `sync` and `merge` need Damast's `/repos/{owner}/{repo}/pulls/{index}/damast/stack` routes.

use std::collections::{BTreeMap, HashSet};

use clap::{Args, Subcommand};
use eyre::{OptionExt, bail, eyre};
use forgejo_api::{
    Forgejo,
    structs::{CreatePullRequestOption, RepoListPullRequestsQuery, RepoListPullRequestsQueryState},
};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::{
    issues::IssueId,
    keys::{KeyInfo, LoginInfo},
    repo::RepoName,
};

const NOT_DAMAST: &str = "this server runs plain Forgejo: `mast pr stack sync` and `mast pr stack merge` need Damast (https://damast.dev). `mast pr stack` and `mast pr stack push` work everywhere";

#[derive(Args, Clone, Debug)]
pub struct StackCommand {
    /// The pull request (defaults to the one for the current branch)
    pr: Option<IssueId>,
    #[clap(subcommand)]
    command: Option<StackSubcommand>,
}

#[derive(Subcommand, Clone, Debug)]
enum StackSubcommand {
    /// Open a pull request for the current branch on top of the layer it builds on
    ///
    /// Pushes the branch, then opens a pull request whose base is the head branch of the open
    /// pull request below it: the one given with --on, or the nearest one whose head commit
    /// the current branch contains.
    Push {
        /// Title (defaults to the subject of the branch's last commit)
        title: Option<String>,
        /// The pull request to build on
        #[clap(long)]
        on: Option<IssueId>,
        /// Description of the pull request
        #[clap(long, short)]
        body: Option<String>,
    },
    /// Rebase every layer onto the layer below it, and the bottom one onto its base (Damast)
    Sync {
        /// Any pull request in the stack (defaults to the one for the current branch)
        pr: Option<IssueId>,
    },
    /// Merge the stack from the bottom up to a pull request (Damast)
    Merge {
        /// The topmost pull request to merge (defaults to the one for the current branch)
        pr: Option<IssueId>,
        /// Same as giving the pull request
        #[clap(long, conflicts_with = "pr")]
        up_to: Option<IssueId>,
        /// Merge style (default: the repository's default merge style)
        #[clap(long, value_parser = ["merge", "rebase", "rebase-merge", "squash", "fast-forward-only"])]
        style: Option<String>,
    },
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
struct Stack {
    trunk: String,
    current: i64,
    layers: Vec<Layer>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
struct Layer {
    number: i64,
    title: String,
    #[serde(default)]
    draft: bool,
    base: String,
    head: String,
    #[serde(default)]
    head_sha: String,
    #[serde(default)]
    parent: i64,
    #[serde(default)]
    depth: usize,
    #[serde(default)]
    checks: String,
}

#[derive(Deserialize, Debug)]
struct ActionResult {
    done: Vec<i64>,
    failed_at: i64,
    #[serde(default)]
    message: String,
}

impl StackCommand {
    /// The pull request whose repository and number the command works on, if it needs one.
    pub fn pr(&self) -> Option<&IssueId> {
        match &self.command {
            None => self.pr.as_ref(),
            Some(StackSubcommand::Push { on, .. }) => on.as_ref(),
            Some(StackSubcommand::Sync { pr }) => pr.as_ref().or(self.pr.as_ref()),
            Some(StackSubcommand::Merge { pr, up_to, .. }) => {
                pr.as_ref().or(up_to.as_ref()).or(self.pr.as_ref())
            }
        }
    }

    /// `push` without --on has no pull request yet: the current branch is the new layer.
    pub fn needs_pr(&self) -> bool {
        !matches!(self.command, Some(StackSubcommand::Push { on: None, .. }))
    }

    pub async fn run(
        self,
        keys: &mut KeyInfo,
        host: &Url,
        api: &Forgejo,
        repo: &RepoName,
        id: Option<i64>,
        remote: Option<&str>,
    ) -> eyre::Result<()> {
        match self.command {
            None => {
                let id = id.ok_or_eyre("no pull request")?;
                let client = Client::new(keys, host, api, repo, id)?;
                let stack = match client.send(Method::GET, "", None).await {
                    Ok((_, v)) => serde_json::from_value(v)?,
                    Err(Error::NotDamast) => derive(&open_layers(api, repo).await?, id)
                        .ok_or_else(|| eyre!("#{id} is not an open pull request"))?,
                    Err(e) => return Err(e.into()),
                };
                print!("{}", format_stack(&stack));
            }
            Some(StackSubcommand::Push { title, on, body }) => {
                push(api, repo, on.and(id), title, body, remote).await?
            }
            Some(StackSubcommand::Sync { .. }) => {
                let id = id.ok_or_eyre("no pull request")?;
                let client = Client::new(keys, host, api, repo, id)?;
                let res = client.action("/rebase", json!({}), false).await?;
                report(res, "rebased", "the stack is already up to date")?;
            }
            Some(StackSubcommand::Merge { style, .. }) => {
                let id = id.ok_or_eyre("no pull request")?;
                let client = Client::new(keys, host, api, repo, id)?;
                let res = client
                    .action("/merge", json!({"style": style.unwrap_or_default()}), true)
                    .await?;
                report(res, "merged", "nothing to merge")?;
            }
        }
        Ok(())
    }
}

fn report(res: ActionResult, verb: &str, nothing: &str) -> eyre::Result<()> {
    let done = res
        .done
        .iter()
        .map(|n| format!("#{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    match (done.is_empty(), res.failed_at) {
        (true, 0) => println!("{nothing}"),
        (false, _) => println!("{verb} {done}"),
        _ => {}
    }
    if res.failed_at != 0 {
        bail!("{}", res.message);
    }
    Ok(())
}

/// Every open pull request of the repository, as stack layers (plain Forgejo API).
async fn open_layers(api: &Forgejo, repo: &RepoName) -> eyre::Result<Vec<(Layer, bool)>> {
    let query = RepoListPullRequestsQuery {
        state: Some(RepoListPullRequestsQueryState::Open),
        ..Default::default()
    };
    let prs = api
        .repo_list_pull_requests(repo.owner(), repo.name(), query)
        .all()
        .await?;
    let mut layers = Vec::new();
    for pr in prs {
        let (Some(head), Some(base)) = (pr.head, pr.base) else {
            continue;
        };
        let same_repo = head.repo_id.is_some() && head.repo_id == base.repo_id;
        let title = pr.title.unwrap_or_default();
        layers.push((
            Layer {
                number: pr.number.unwrap_or_default(),
                draft: pr.draft.unwrap_or(false) || is_draft(&title),
                title,
                base: base.r#ref.unwrap_or_default(),
                head: head.r#ref.unwrap_or_default(),
                head_sha: head.sha.unwrap_or_default(),
                ..Default::default()
            },
            same_repo,
        ));
    }
    Ok(layers)
}

fn is_draft(title: &str) -> bool {
    // shortcut: Forgejo's default WORK_IN_PROGRESS_PREFIXES only; the server's list is not exposed
    ["WIP:", "[WIP]"].iter().any(|p| {
        title
            .get(..p.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(p))
    })
}

/// Builds the stack of `current` from open pull requests, like Damast does: walk down to the
/// bottom layer (lowest number wins when a branch heads several PRs), then collect the tree
/// above it in pre-order, children by number. Fork PRs are never layers.
fn derive(prs: &[(Layer, bool)], current: i64) -> Option<Stack> {
    let (me, same_repo) = prs.iter().find(|(l, _)| l.number == current)?;
    let alone = || Stack {
        trunk: me.base.clone(),
        current,
        layers: vec![me.clone()],
    };
    if !same_repo {
        return Some(alone());
    }
    let mut layers: Vec<&Layer> = prs.iter().filter(|(_, s)| *s).map(|(l, _)| l).collect();
    layers.sort_by_key(|l| l.number);

    let mut seen = HashSet::from([current]);
    let mut bottom = me;
    while let Some(parent) = layers
        .iter()
        .find(|l| l.head == bottom.base && !seen.contains(&l.number))
    {
        seen.insert(parent.number);
        bottom = parent;
    }

    let mut out = Vec::new();
    let mut seen = HashSet::from([bottom.number]);
    fn walk<'a>(
        l: &'a Layer,
        parent: i64,
        depth: usize,
        all: &[&'a Layer],
        seen: &mut HashSet<i64>,
        out: &mut Vec<Layer>,
    ) {
        out.push(Layer {
            parent,
            depth,
            ..l.clone()
        });
        let children: Vec<&Layer> = all
            .iter()
            .filter(|c| c.base == l.head && !seen.contains(&c.number))
            .copied()
            .collect();
        for c in &children {
            seen.insert(c.number);
        }
        for c in children {
            walk(c, l.number, depth + 1, all, seen, out);
        }
    }
    walk(bottom, 0, 0, &layers, &mut seen, &mut out);
    if !out.iter().any(|l| l.number == current) {
        return Some(alone());
    }
    Some(Stack {
        trunk: bottom.base.clone(),
        current,
        layers: out,
    })
}

fn format_stack(stack: &Stack) -> String {
    if stack.layers.len() < 2 {
        return format!("#{} is not part of a stack\n", stack.current);
    }
    let mut out = format!("{}\n", stack.trunk);
    for l in &stack.layers {
        let checks = match l.checks.as_str() {
            "" => "",
            "success" => " ✓",
            "pending" => " ●",
            "warning" => " !",
            "skipped" => " -",
            _ => " ✗",
        };
        out += &format!(
            "{}└ #{} {}{}{} ({}){}\n",
            "  ".repeat(l.depth),
            l.number,
            l.title,
            if l.draft { " [draft]" } else { "" },
            checks,
            l.head,
            if l.number == stack.current {
                "  ← this pull request"
            } else {
                ""
            }
        );
    }
    out
}

/// The open same-repo pull request the current branch builds on: the nearest one whose head
/// commit HEAD contains.
fn nearest_parent<'a>(
    git: &git2::Repository,
    head: git2::Oid,
    branch: &str,
    prs: &'a [(Layer, bool)],
) -> eyre::Result<Option<&'a Layer>> {
    let mut candidates = Vec::new();
    for (l, same_repo) in prs {
        if !same_repo || l.head == branch {
            continue;
        }
        // the server's head, or the local branch of that name when the layer was pushed
        // again after the current branch forked off it
        let local = git
            .find_branch(&l.head, git2::BranchType::Local)
            .ok()
            .and_then(|b| b.get().target());
        let contained = git2::Oid::from_str(&l.head_sha)
            .ok()
            .into_iter()
            .chain(local)
            .find(|&oid| oid == head || git.graph_descendant_of(head, oid).unwrap_or(false));
        if let Some(oid) = contained {
            candidates.push((l, oid));
        }
    }
    // the nearest candidate contains every other candidate
    let mut best: Option<(&Layer, git2::Oid)> = None;
    for (l, oid) in candidates {
        match best {
            Some((_, b)) if !git.graph_descendant_of(oid, b).unwrap_or(false) => {}
            _ => best = Some((l, oid)),
        }
    }
    Ok(best.map(|(l, _)| l))
}

async fn push(
    api: &Forgejo,
    repo: &RepoName,
    on: Option<i64>,
    title: Option<String>,
    body: Option<String>,
    remote: Option<&str>,
) -> eyre::Result<()> {
    let git = git2::Repository::discover(".")?;
    let head_ref = git.head()?;
    eyre::ensure!(head_ref.is_branch(), "HEAD is not on a branch");
    let branch = head_ref
        .shorthand()
        .map_err(|_| eyre!("branch name is not utf8"))?
        .to_owned();
    let head = head_ref.peel_to_commit()?;

    let prs = open_layers(api, repo).await?;
    if let Some((l, _)) = prs.iter().find(|(l, s)| *s && l.head == branch) {
        bail!(
            "{branch} already has pull request #{}; use `mast pr stack` to see its stack",
            l.number
        );
    }
    let parent = match on {
        Some(n) => prs
            .iter()
            .find(|(l, s)| *s && l.number == n)
            .map(|(l, _)| l)
            .ok_or_else(|| eyre!("#{n} is not an open pull request in {}/{}", repo.owner(), repo.name()))?,
        None => nearest_parent(&git, head.id(), &branch, &prs)?.ok_or_else(|| {
            eyre!("{branch} doesn't build on an open pull request; use `mast pr create`, or name the layer with --on")
        })?,
    };
    if parent.head_sha == head.id().to_string() {
        bail!("{branch} has no commits on top of #{}", parent.number);
    }

    if let Some(remote) = remote {
        let status = std::process::Command::new("git")
            .args(["push", "--set-upstream", remote, &branch])
            .status()?;
        eyre::ensure!(status.success(), "git push failed");
    } else {
        eprintln!("no remote known for this repository, not pushing {branch}");
    }

    let title = match title {
        Some(t) => t,
        None => head.summary().ok().flatten().unwrap_or(&branch).to_owned(),
    };
    let pr = api
        .repo_create_pull_request(
            repo.owner(),
            repo.name(),
            CreatePullRequestOption {
                assignee: None,
                assignees: None,
                base: Some(parent.head.clone()),
                body,
                due_date: None,
                head: Some(branch.clone()),
                labels: None,
                milestone: None,
                title: Some(title),
            },
        )
        .await?;
    println!(
        "#{} {} on top of #{} {}",
        pr.number.unwrap_or_default(),
        pr.html_url.map(|u| u.to_string()).unwrap_or_default(),
        parent.number,
        parent.title
    );
    Ok(())
}

#[derive(Debug)]
enum Error {
    NotDamast,
    Other(eyre::Report),
}

impl From<Error> for eyre::Report {
    fn from(e: Error) -> Self {
        match e {
            Error::NotDamast => eyre!(NOT_DAMAST),
            Error::Other(e) => e,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Other(e.into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(e.into())
    }
}

impl From<eyre::Report> for Error {
    fn from(e: eyre::Report) -> Self {
        Error::Other(e)
    }
}

/// The stack API of one pull request.
struct Client<'a> {
    http: reqwest::Client,
    base: Url,
    token: Option<String>,
    api: &'a Forgejo,
    repo: &'a RepoName,
    id: i64,
}

impl<'a> Client<'a> {
    fn new(
        keys: &mut KeyInfo,
        host: &Url,
        api: &'a Forgejo,
        repo: &'a RepoName,
        id: i64,
    ) -> eyre::Result<Self> {
        let token = keys.get_login(host).map(|login| match login {
            LoginInfo::Application { token } | LoginInfo::OAuth { token, .. } => token.clone(),
        });
        let base = host.join(&format!(
            "/api/v1/repos/{}/{}/pulls/{id}/damast/stack",
            repo.owner(),
            repo.name()
        ))?;
        let http = reqwest::Client::builder()
            .user_agent(crate::USER_AGENT)
            .build()?;
        Ok(Self {
            http,
            base,
            token,
            api,
            repo,
            id,
        })
    }

    /// Sends a request; 403 and 409 carry an action result and count as answers.
    async fn send(
        &self,
        method: Method,
        suffix: &str,
        body: Option<&Value>,
    ) -> Result<(StatusCode, Value), Error> {
        let mut url = self.base.clone();
        url.set_path(&format!("{}{suffix}", self.base.path()));
        let mut req = self.http.request(method, url);
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        if let Some(body) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(body)?);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        let value = serde_json::from_slice::<Value>(&bytes).ok();
        if status.is_success()
            || (matches!(status, StatusCode::FORBIDDEN | StatusCode::CONFLICT)
                && value.as_ref().is_some_and(|v| v.get("failed_at").is_some()))
        {
            return value
                .map(|v| (status, v))
                .ok_or_else(|| Error::Other(eyre!("invalid JSON from the server")));
        }
        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            // Damast always has the route for an existing pull request; plain Forgejo answers 404 (GET) or 405 (POST).
            if self
                .api
                .repo_get_pull_request(self.repo.owner(), self.repo.name(), self.id)
                .await
                .is_ok()
            {
                return Err(Error::NotDamast);
            }
            return Err(Error::Other(eyre!(
                "pull request {}/{}#{} not found",
                self.repo.owner(),
                self.repo.name(),
                self.id
            )));
        }
        let message = value
            .and_then(|v| v["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| status.to_string());
        Err(Error::Other(eyre!("{message}")))
    }

    /// Runs a stack action with the head commits the stack shows right now, so that the
    /// server refuses to touch a branch that moved in between.
    async fn action(
        &self,
        suffix: &str,
        mut body: Value,
        need_stack: bool,
    ) -> eyre::Result<ActionResult> {
        let (_, v) = self.send(Method::GET, "", None).await?;
        let stack: Stack = serde_json::from_value(v)?;
        // a lone layer can still be synced: after a squash merge below it, that drops the old commits
        if need_stack && stack.layers.len() < 2 {
            bail!(
                "#{} is not part of a stack; use `mast pr merge` for a single pull request",
                stack.current
            );
        }
        body["heads"] = json!(heads(&stack));
        let (_, v) = self.send(Method::POST, suffix, Some(&body)).await?;
        Ok(serde_json::from_value(v)?)
    }
}

fn heads(stack: &Stack) -> BTreeMap<String, String> {
    stack
        .layers
        .iter()
        .map(|l| (l.number.to_string(), l.head_sha.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn pr(number: i64, head: &str, base: &str, same_repo: bool) -> (Layer, bool) {
        (
            Layer {
                number,
                title: format!("Layer {head}"),
                head: head.into(),
                base: base.into(),
                head_sha: format!("{number}"),
                ..Default::default()
            },
            same_repo,
        )
    }

    fn shape(s: &Stack) -> Vec<String> {
        s.layers
            .iter()
            .map(|l| format!("{}<{}:{}", l.number, l.parent, l.depth))
            .collect()
    }

    #[test]
    fn derive_tree() {
        let prs = [
            pr(1, "a", "main", true),
            pr(2, "b", "a", true),
            pr(3, "c", "b", true),
            pr(4, "d", "a", true),
            pr(5, "f", "a", false),   // fork
            pr(6, "x", "main", true), // unrelated, same trunk
        ];
        for current in [1, 2, 3, 4] {
            let s = derive(&prs, current).unwrap();
            assert_eq!(s.trunk, "main");
            assert_eq!(shape(&s), ["1<0:0", "2<1:1", "3<2:2", "4<1:1"]);
        }
        assert_eq!(derive(&prs, 5).unwrap().layers.len(), 1);
        assert_eq!(derive(&prs, 6).unwrap().layers.len(), 1);
        assert!(derive(&prs, 7).is_none());
    }

    #[test]
    fn derive_cycle() {
        let prs = [pr(1, "p", "q", true), pr(2, "q", "p", true)];
        assert_eq!(derive(&prs, 1).unwrap().layers.len(), 2);
        assert_eq!(derive(&prs, 2).unwrap().layers.len(), 2);
    }

    #[test]
    fn format() {
        let s = derive(&[pr(1, "a", "main", true), pr(2, "b", "a", true)], 2).unwrap();
        let mut s = s;
        s.layers[0].checks = "success".into();
        s.layers[1].draft = true;
        assert_eq!(
            format_stack(&s),
            "main\n└ #1 Layer a ✓ (a)\n  └ #2 Layer b [draft] (b)  ← this pull request\n"
        );
        let alone = derive(&[pr(1, "a", "main", true)], 1).unwrap();
        assert_eq!(format_stack(&alone), "#1 is not part of a stack\n");
    }

    #[test]
    fn heads_by_number() {
        let s = derive(&[pr(1, "a", "main", true), pr(2, "b", "a", true)], 1).unwrap();
        assert_eq!(
            serde_json::to_value(heads(&s)).unwrap(),
            json!({"1": "1", "2": "2"})
        );
    }

    #[test]
    fn api_json() {
        let s: Stack = serde_json::from_value(json!({
            "trunk": "main", "current": 2,
            "layers": [
                {"number": 1, "title": "A", "html_url": "u", "state": "open", "draft": false, "base": "main", "head": "a", "head_sha": "s1", "parent": 0, "depth": 0, "checks": "success"},
                {"number": 2, "title": "B", "html_url": "u", "state": "open", "draft": true, "base": "a", "head": "b", "head_sha": "s2", "parent": 1, "depth": 1, "checks": ""}
            ]
        }))
        .unwrap();
        assert_eq!(shape(&s), ["1<0:0", "2<1:1"]);
        let r: ActionResult = serde_json::from_value(
            json!({"done": [1], "failed_at": 2, "message": "Stopped at #2: x"}),
        )
        .unwrap();
        assert_eq!(
            (r.done, r.failed_at, r.message.as_str()),
            (vec![1], 2, "Stopped at #2: x")
        );
    }

    #[test]
    fn drafts() {
        assert!(is_draft("WIP: x") && is_draft("[wip] x"));
        assert!(!is_draft("x WIP:"));
    }

    #[test]
    fn nearest_parent_in_git() {
        let dir = std::env::temp_dir().join(format!("mast-stack-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let git = git2::Repository::init(&dir).unwrap();
        let sig = git2::Signature::now("t", "t@example.com").unwrap();
        let tree = git
            .find_tree(git.index().unwrap().write_tree().unwrap())
            .unwrap();
        let mut parents: Vec<git2::Commit> = vec![];
        let mut ids = vec![];
        for msg in ["main", "a", "b", "c"] {
            let refs: Vec<&git2::Commit> = parents.iter().collect();
            let id = git.commit(None, &sig, &sig, msg, &tree, &refs).unwrap();
            ids.push(id);
            parents = vec![git.find_commit(id).unwrap()];
        }
        let layer = |n: i64, head: &str, base: &str, oid: git2::Oid| {
            (
                Layer {
                    number: n,
                    head: head.into(),
                    base: base.into(),
                    head_sha: oid.to_string(),
                    ..Default::default()
                },
                true,
            )
        };
        let prs = [layer(1, "a", "main", ids[1]), layer(2, "b", "a", ids[2])];
        let p = nearest_parent(&git, ids[3], "c", &prs).unwrap().unwrap();
        assert_eq!(p.number, 2);
        let p = nearest_parent(&git, ids[1], "a2", &prs).unwrap().unwrap();
        assert_eq!(p.number, 1);
        assert!(nearest_parent(&git, ids[0], "x", &prs).unwrap().is_none());

        // b was pushed again after c branched off it: the server's head of #2 is not in c,
        // but the local branch b is, so #2 (not #1) is still the layer below
        let b2 = git
            .commit(
                None,
                &sig,
                &sig,
                "b2",
                &tree,
                &[&git.find_commit(ids[2]).unwrap()],
            )
            .unwrap();
        git.branch("b", &git.find_commit(ids[2]).unwrap(), true)
            .unwrap();
        let prs = [layer(1, "a", "main", ids[1]), layer(2, "b", "a", b2)];
        let p = nearest_parent(&git, ids[3], "c", &prs).unwrap().unwrap();
        assert_eq!(p.number, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli() {
        let app = crate::App::try_parse_from(["mast", "pr", "stack", "7"]).unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("Stack") && s.contains("number: 7") && s.contains("command: None"),
            "{s}"
        );
        let app = crate::App::try_parse_from(["mast", "pr", "stack", "push", "--on", "3", "Title"])
            .unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("Push") && s.contains("number: 3") && s.contains("Some(\"Title\")"),
            "{s}"
        );
        let app = crate::App::try_parse_from(["mast", "pr", "stack", "sync"]).unwrap();
        assert!(format!("{app:?}").contains("Sync"));
        let app = crate::App::try_parse_from([
            "mast", "pr", "stack", "merge", "--up-to", "4", "--style", "squash",
        ])
        .unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("Merge") && s.contains("number: 4") && s.contains("squash"),
            "{s}"
        );
        assert!(
            crate::App::try_parse_from(["mast", "pr", "stack", "merge", "--style", "octopus"])
                .is_err()
        );
        assert!(
            crate::App::try_parse_from(["mast", "pr", "stack", "merge", "4", "--up-to", "5"])
                .is_err()
        );
    }

    #[test]
    fn needs_pr() {
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            c: StackCommand,
        }
        let cmd = |args: &[&str]| W::try_parse_from(args).unwrap().c;
        assert!(!cmd(&["stack", "push"]).needs_pr());
        assert!(cmd(&["stack", "push", "--on", "3"]).needs_pr());
        assert!(cmd(&["stack"]).needs_pr());
        assert_eq!(
            cmd(&["stack", "merge", "--up-to", "4"])
                .pr()
                .unwrap()
                .number,
            4
        );
    }
}
