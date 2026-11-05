//! Suggested changes from reviews: `mast pr suggestions [pr]` and `mast pr suggestions apply`.
//!
//! Uses Damast's `/repos/{owner}/{repo}/pulls/{index}/damast/suggestions` API. Plain Forgejo has
//! no such route; the commands then say so instead of printing a bare 404.

use clap::{Args, Subcommand};
use eyre::{bail, eyre};
use forgejo_api::Forgejo;
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::{
    issues::IssueId,
    keys::{KeyInfo, LoginInfo},
    repo::RepoName,
};

const NOT_DAMAST: &str = "this server has no review suggestions: it runs plain Forgejo, and `mast pr suggestions` needs Damast (https://damast.dev)";

#[derive(Args, Clone, Debug)]
pub struct SuggestionsCommand {
    /// The pull request (defaults to the one for the current branch)
    pr: Option<IssueId>,
    #[clap(subcommand)]
    command: Option<SuggestionsSubcommand>,
}

#[derive(Subcommand, Clone, Debug)]
enum SuggestionsSubcommand {
    /// Commit suggestions to the pull request's branch as one commit
    Apply {
        /// The pull request (defaults to the one for the current branch)
        pr: Option<IssueId>,
        /// Comment IDs of the suggestions, as listed by `mast pr suggestions`
        ids: Vec<i64>,
        /// Apply every suggestion that can be applied and isn't resolved yet
        #[clap(long, conflicts_with = "ids")]
        all: bool,
        /// Commit message (default: "Apply suggestions from code review")
        #[clap(long, short)]
        message: Option<String>,
    },
}

#[derive(Deserialize, Debug, PartialEq)]
struct Suggestion {
    comment_id: i64,
    author: String,
    path: String,
    start_line: i64,
    end_line: i64,
    #[serde(default)]
    old_lines: Option<Vec<String>>,
    #[serde(default)]
    new_lines: Option<Vec<String>>,
    resolved: bool,
    applicable: bool,
    #[serde(default)]
    problem: Option<String>,
}

impl SuggestionsCommand {
    pub fn pr(&self) -> Option<&IssueId> {
        match &self.command {
            Some(SuggestionsSubcommand::Apply { pr: Some(pr), .. }) => Some(pr),
            _ => self.pr.as_ref(),
        }
    }

    pub async fn run(
        self,
        keys: &mut KeyInfo,
        host: &Url,
        api: &Forgejo,
        repo: &RepoName,
        id: i64,
    ) -> eyre::Result<()> {
        let client = Client::new(keys, host, api, repo, id)?;
        match self.command {
            None => {
                let list: Vec<Suggestion> =
                    serde_json::from_value(client.send(Method::GET, "", None).await?)?;
                print!("{}", format_list(&list));
            }
            Some(SuggestionsSubcommand::Apply {
                ids, all, message, ..
            }) => {
                let ids = if all {
                    let list: Vec<Suggestion> =
                        serde_json::from_value(client.send(Method::GET, "", None).await?)?;
                    open_ids(&list)
                } else {
                    ids
                };
                if ids.is_empty() {
                    bail!(if all {
                        "no suggestions left to apply"
                    } else {
                        "give the comment IDs of the suggestions to apply, or --all"
                    });
                }
                let body = json!({"comment_ids": ids, "message": message.unwrap_or_default()});
                let resp = client.send(Method::POST, "/apply", Some(&body)).await?;
                let n = ids.len();
                println!(
                    "applied {n} suggestion{} in {}",
                    if n == 1 { "" } else { "s" },
                    resp["commit"].as_str().unwrap_or_default()
                );
            }
        }
        Ok(())
    }
}

/// Suggestions `--all` applies: those that can be applied and aren't resolved.
fn open_ids(list: &[Suggestion]) -> Vec<i64> {
    list.iter()
        .filter(|s| s.applicable && !s.resolved)
        .map(|s| s.comment_id)
        .collect()
}

fn format_list(list: &[Suggestion]) -> String {
    if list.is_empty() {
        return "no suggestions\n".to_owned();
    }
    let mut out = String::new();
    for s in list {
        let lines = if s.start_line == s.end_line {
            s.start_line.to_string()
        } else {
            format!("{}-{}", s.start_line, s.end_line)
        };
        let state = match (s.problem.as_deref(), s.resolved, s.applicable) {
            (Some("outdated"), ..) => " (outdated)",
            (Some("multiple"), ..) => " (more than one suggestion in the comment)",
            (Some("old_side"), ..) => " (on the old side of the diff)",
            (_, true, _) => " (resolved)",
            (_, _, false) => " (you can't apply it)",
            _ => "",
        };
        out += &format!(
            "#{} {}:{lines} by @{}{state}\n",
            s.comment_id, s.path, s.author
        );
        for l in s.old_lines.iter().flatten() {
            out += &format!("  - {l}\n");
        }
        for l in s.new_lines.iter().flatten() {
            out += &format!("  + {l}\n");
        }
    }
    out
}

/// The suggestions API of one pull request.
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
            "/api/v1/repos/{}/{}/pulls/{id}/damast/suggestions",
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

    async fn send(
        &self,
        method: Method,
        suffix: &str,
        body: Option<&Value>,
    ) -> eyre::Result<Value> {
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
        if status.is_success() {
            return Ok(serde_json::from_slice(&bytes)?);
        }
        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            // Damast always has the route for an existing pull request; plain Forgejo answers 404 (GET) or 405 (POST).
            let pr_exists = self
                .api
                .repo_get_pull_request(self.repo.owner(), self.repo.name(), self.id)
                .await
                .is_ok();
            bail!(not_found_message(pr_exists, self.repo, self.id));
        }
        let message = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| status.to_string());
        Err(eyre!("{message}"))
    }
}

fn not_found_message(pr_exists: bool, repo: &RepoName, id: i64) -> String {
    if pr_exists {
        NOT_DAMAST.to_owned()
    } else {
        format!(
            "pull request {}/{}#{id} not found",
            repo.owner(),
            repo.name()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn sug(id: i64, applicable: bool, resolved: bool, problem: Option<&str>) -> Suggestion {
        Suggestion {
            comment_id: id,
            author: "eve".into(),
            path: "src/a.rs".into(),
            start_line: 3,
            end_line: if id == 1 { 3 } else { 4 },
            old_lines: Some(vec!["old".into()]),
            new_lines: Some(vec!["new".into()]),
            resolved,
            applicable,
            problem: problem.map(Into::into),
        }
    }

    #[test]
    fn cli() {
        let app = crate::App::try_parse_from(["mast", "pr", "suggestions", "7"]).unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("Suggestions") && s.contains("number: 7") && s.contains("command: None")
        );

        let app =
            crate::App::try_parse_from(["mast", "pr", "suggestions", "apply", "7", "11", "12"])
                .unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("number: 7") && s.contains("ids: [11, 12]"),
            "{s}"
        );

        let app = crate::App::try_parse_from([
            "mast",
            "pr",
            "suggestions",
            "apply",
            "--all",
            "-m",
            "msg",
        ])
        .unwrap();
        let s = format!("{app:?}");
        assert!(
            s.contains("all: true") && s.contains("Some(\"msg\")"),
            "{s}"
        );

        assert!(
            crate::App::try_parse_from(["mast", "pr", "suggestions", "apply", "7", "11", "--all"])
                .is_err()
        );
    }

    #[test]
    fn all_skips_resolved_and_inapplicable() {
        let list = [
            sug(1, true, false, None),
            sug(2, true, true, None),
            sug(3, false, false, Some("outdated")),
            sug(4, true, false, None),
        ];
        assert_eq!(open_ids(&list), vec![1, 4]);
    }

    #[test]
    fn list_format() {
        assert_eq!(format_list(&[]), "no suggestions\n");
        let out = format_list(&[
            sug(1, true, false, None),
            sug(2, false, false, Some("outdated")),
        ]);
        assert_eq!(
            out,
            "#1 src/a.rs:3 by @eve\n  - old\n  + new\n#2 src/a.rs:3-4 by @eve (outdated)\n  - old\n  + new\n"
        );
    }

    #[test]
    fn parses_api_response() {
        let v: Vec<Suggestion> = serde_json::from_str(
            r#"[{"comment_id":5,"review_id":2,"author":"eve","path":"a","start_line":1,"end_line":1,
                "old_lines":["x"],"new_lines":[],"resolved":false,"applicable":true,"html_url":"u"}]"#,
        )
        .unwrap();
        assert_eq!(v[0].new_lines, Some(vec![]));
        assert_eq!(v[0].problem, None);
    }

    #[test]
    fn plain_forgejo_message() {
        let repo = RepoName::new("o".into(), "r".into());
        assert_eq!(not_found_message(true, &repo, 3), NOT_DAMAST);
        assert_eq!(
            not_found_message(false, &repo, 3),
            "pull request o/r#3 not found"
        );
    }
}
