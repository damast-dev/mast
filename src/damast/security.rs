//! Security tab of Damast: `mast security policy|report` and `mast advisory ...`.
//!
//! Uses Damast's `/repos/{owner}/{repo}/security-advisories` API (GitHub's REST shape). Plain Forgejo
//! has no such route; every command then says so instead of printing a bare 404.

use clap::{Args, Subcommand};
use eyre::{OptionExt, bail, eyre};
use forgejo_api::{ApiErrorKind, Forgejo, ForgejoError, structs::RepoGetRawFileQuery};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use url::Url;

use crate::{
    keys::{KeyInfo, LoginInfo},
    repo::{RepoArg, RepoInfo, RepoName},
};

/// Where Damast looks for a security policy, in this order (same as the Security tab).
const POLICY_PATHS: [&str; 5] = [
    ".forgejo/SECURITY.md",
    ".gitea/SECURITY.md",
    ".github/SECURITY.md",
    "SECURITY.md",
    "docs/SECURITY.md",
];

const NOT_DAMAST: &str = "this server has no security advisories: it runs plain Forgejo, and `mast security` / `mast advisory` need Damast (https://damast.dev)";

#[derive(Args, Clone, Debug)]
pub struct RepoOpts {
    /// The git remote to use
    #[clap(long, short = 'R', global = true)]
    remote: Option<String>,
    /// The repository (OWNER/REPO), defaults to the current one
    #[clap(long, short, global = true)]
    repo: Option<RepoArg>,
}

#[derive(Args, Clone, Debug)]
pub struct SecurityCommand {
    #[clap(flatten)]
    repo: RepoOpts,
    #[clap(subcommand)]
    command: SecuritySubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum SecuritySubcommand {
    /// Show the repository's security policy (SECURITY.md)
    Policy,
    /// Report a vulnerability privately to the repository's admins
    Report {
        #[clap(flatten)]
        content: Box<Content>,
    },
}

#[derive(Args, Clone, Debug)]
pub struct AdvisoryCommand {
    #[clap(flatten)]
    repo: RepoOpts,
    #[clap(subcommand)]
    command: AdvisorySubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum AdvisorySubcommand {
    /// List the repository's security advisories you can see
    List {
        /// Only advisories in this state (triage, draft, published, closed, withdrawn)
        #[clap(long)]
        state: Option<String>,
        /// Print the API's JSON
        #[clap(long, conflicts_with = "osv")]
        json: bool,
        /// Print the published advisories as OSV JSON (for export)
        #[clap(long)]
        osv: bool,
    },
    /// Show one advisory
    View {
        /// The advisory ID (DAMAST-...)
        id: String,
        /// Print the API's JSON
        #[clap(long, conflicts_with = "osv")]
        json: bool,
        /// Print it as OSV JSON (published advisories only)
        #[clap(long)]
        osv: bool,
    },
    /// Create a draft advisory (repository admins)
    Create {
        #[clap(flatten)]
        content: Box<Content>,
        /// CVE ID, if one has been assigned
        #[clap(long)]
        cve: Option<String>,
    },
    /// Publish a draft advisory
    Publish { id: String },
    /// Close an advisory or a report
    Close { id: String },
    /// Withdraw a published advisory that turned out to be wrong (final; it stays visible as withdrawn)
    Withdraw { id: String },
    /// Show the private discussion of an advisory
    Comments {
        id: String,
        /// Print the API's JSON
        #[clap(long)]
        json: bool,
    },
    /// Comment in the private discussion; `-b -` reads the comment from stdin
    Comment {
        id: String,
        #[clap(long, short)]
        body: String,
    },
    /// Edit one of your comments (admins: any comment)
    CommentEdit {
        id: String,
        comment: i64,
        #[clap(long, short)]
        body: String,
    },
    /// Delete one of your comments (admins: any comment)
    CommentDelete { id: String, comment: i64 },
}

#[derive(Args, Clone, Debug)]
pub struct Content {
    /// Short summary of the vulnerability
    #[clap(long, short)]
    title: String,
    /// Details in Markdown; `-` reads them from stdin
    #[clap(long, short)]
    body: String,
    /// low, medium, high or critical (follows the CVSS score when one is given)
    #[clap(long, short, default_value = "medium")]
    severity: String,
    /// CVSS v3.1 vector, e.g. CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H
    #[clap(long)]
    cvss: Option<String>,
    /// CWE ID, may be repeated
    #[clap(long)]
    cwe: Vec<String>,
    // shortcut: one affected package from the command line; add more in the web UI or the API.
    /// Ecosystem of the affected package (npm, pip, go, maven, rust, ...)
    #[clap(long, requires = "package")]
    ecosystem: Option<String>,
    /// Name of the affected package
    #[clap(long, requires = "ecosystem")]
    package: Option<String>,
    /// Vulnerable versions, e.g. ">= 1.0, < 1.4.2"
    #[clap(long, requires = "package")]
    vulnerable: Option<String>,
    /// First patched version
    #[clap(long, requires = "package")]
    patched: Option<String>,
}

impl Content {
    fn to_json(&self, body: String) -> Value {
        let mut v = json!({
            "summary": self.title,
            "description": body,
            "severity": self.severity,
            "cwe_ids": self.cwe,
            "vulnerabilities": [],
        });
        if let Some(cvss) = &self.cvss {
            v["cvss_vector_string"] = json!(cvss);
        }
        if let (Some(ecosystem), Some(package)) = (&self.ecosystem, &self.package) {
            v["vulnerabilities"] = json!([{
                "package": {"ecosystem": ecosystem, "name": package},
                "vulnerable_version_range": self.vulnerable.as_deref().unwrap_or(""),
                "patched_versions": self.patched.as_deref().unwrap_or(""),
            }]);
        }
        v
    }

    async fn json(&self) -> eyre::Result<Value> {
        Ok(self.to_json(read_body(self.body.clone()).await?))
    }
}

/// The advisories API of one repository.
struct Client {
    http: reqwest::Client,
    base: Url,
    token: Option<String>,
    api: Forgejo,
    repo: RepoName,
}

impl Client {
    async fn new(keys: &mut KeyInfo, host: Option<&str>, opts: &RepoOpts) -> eyre::Result<Self> {
        let info = RepoInfo::get_current(host, opts.repo.as_ref(), opts.remote.as_deref(), keys)?;
        let api = keys.get_api(info.host_url()).await?; // refreshes an expired OAuth token
        let token = keys.get_login(info.host_url()).map(|login| match login {
            LoginInfo::Application { token } | LoginInfo::OAuth { token, .. } => token.clone(),
        });
        let repo = info
            .name()
            .ok_or_eyre("couldn't get repo name, try specifying with --repo")?
            .clone();
        let base = info.host_url().join(&format!(
            "/api/v1/repos/{}/{}/security-advisories",
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
        })
    }

    /// `suffix` is a path below the advisories route, optionally with a query.
    fn url(&self, suffix: &str) -> Url {
        let (path, query) = suffix.split_once('?').unwrap_or((suffix, ""));
        let mut url = self.base.clone();
        url.set_path(&format!("{}{path}", self.base.path()));
        url.set_query(Some(query).filter(|q| !q.is_empty()));
        url
    }

    fn repo_name(&self) -> String {
        format!("{}/{}", self.repo.owner(), self.repo.name())
    }

    async fn send(
        &self,
        method: Method,
        suffix: &str,
        body: Option<&Value>,
    ) -> eyre::Result<Value> {
        let mut req = self.http.request(method, self.url(suffix));
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
            if bytes.is_empty() {
                return Ok(Value::Null); // 204 No Content
            }
            return Ok(serde_json::from_slice(&bytes)?);
        }
        let message = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v["message"].as_str().map(str::to_owned));
        if status == StatusCode::NOT_FOUND {
            bail!(self.not_found(suffix).await);
        }
        Err(error_for(status, message))
    }

    /// Tells "no such advisory" from "no such repository" and from "this is not Damast".
    async fn not_found(&self, suffix: &str) -> String {
        let path = suffix.split('?').next().unwrap_or_default();
        let has_route = !path.is_empty() && {
            let mut req = self.http.get(self.url(""));
            if let Some(token) = &self.token {
                req = req.header("Authorization", format!("token {token}"));
            }
            matches!(req.send().await, Ok(r) if r.status() != StatusCode::NOT_FOUND)
        };
        if has_route && path.starts_with("/reports") {
            "private vulnerability reporting is off for this repository, or you can't report here"
                .to_owned()
        } else if has_route && path.contains("/comments/") {
            "advisory or comment not found, or it is not yours".to_owned()
        } else if has_route {
            "advisory not found, or you may not see it".to_owned()
        } else if self
            .api
            .repo_get(self.repo.owner(), self.repo.name())
            .await
            .is_ok()
        {
            NOT_DAMAST.to_owned()
        } else {
            format!("repository {} not found", self.repo_name())
        }
    }
}

fn error_for(status: StatusCode, message: Option<String>) -> eyre::Report {
    let message = message.unwrap_or_else(|| status.to_string());
    match status {
        StatusCode::TOO_MANY_REQUESTS => eyre!("{message} (report limit reached, try again later)"),
        _ => eyre!("{message}"),
    }
}

/// `-` means: read the text from stdin.
async fn read_body(body: String) -> eyre::Result<String> {
    if body != "-" {
        return Ok(body);
    }
    let mut s = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::stdin(), &mut s).await?;
    Ok(s)
}

fn comment_path(id: &str, comment: i64) -> String {
    format!("{}/comments/{comment}", id_path(id))
}

fn format_comment(c: &Value) -> String {
    format!(
        "#{} {} ({})",
        c["id"],
        str_of(&c["user"], "login"),
        str_of(c, "created_at")
    )
}

fn id_path(id: &str) -> String {
    format!(
        "/{}",
        url::form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>()
    )
}

impl SecurityCommand {
    pub async fn run(self, keys: &mut KeyInfo, host: Option<&str>) -> eyre::Result<()> {
        let client = Client::new(keys, host, &self.repo).await?;
        match self.command {
            SecuritySubcommand::Policy => policy(&client).await,
            SecuritySubcommand::Report { content } => {
                let a = client
                    .send(Method::POST, "/reports", Some(&content.json().await?))
                    .await?;
                println!("Reported privately as {}", str_of(&a, "ghsa_id"));
                println!("{}", str_of(&a, "html_url"));
                Ok(())
            }
        }
    }
}

async fn policy(client: &Client) -> eyre::Result<()> {
    for path in POLICY_PATHS {
        let file = client
            .api
            .repo_get_raw_file(
                client.repo.owner(),
                client.repo.name(),
                path,
                RepoGetRawFileQuery::default(),
            )
            .await;
        match file {
            Ok(body) => {
                let crate::SpecialRender { bold, reset, .. } = crate::special_render();
                println!("{bold}{path}{reset}\n");
                println!(
                    "{}",
                    crate::markdown(&String::from_utf8_lossy(body.as_ref()))
                );
                return Ok(());
            }
            Err(ForgejoError::ApiError(e)) if matches!(e.kind, ApiErrorKind::NotFound { .. }) => {}
            Err(e) => return Err(e.into()),
        }
    }
    println!(
        "{} has no security policy (SECURITY.md)",
        client.repo_name()
    );
    Ok(())
}

impl AdvisoryCommand {
    pub async fn run(self, keys: &mut KeyInfo, host: Option<&str>) -> eyre::Result<()> {
        let client = Client::new(keys, host, &self.repo).await?;
        match self.command {
            AdvisorySubcommand::List { state, json, osv } => {
                if osv {
                    return print_json(&client.send(Method::GET, "/osv", None).await?);
                }
                let query = state
                    .map(|s| {
                        format!(
                            "?state={}",
                            url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()
                        )
                    })
                    .unwrap_or_default();
                let list = client.send(Method::GET, &query, None).await?;
                if json {
                    return print_json(&list);
                }
                print!("{}", format_list(&list));
                Ok(())
            }
            AdvisorySubcommand::View { id, json, osv } => {
                let suffix = id_path(&id) + if osv { "/osv" } else { "" };
                let a = client.send(Method::GET, &suffix, None).await?;
                if json || osv {
                    return print_json(&a);
                }
                print!("{}", format_advisory(&a));
                if let Some(d) = a["description"].as_str() {
                    println!("\n{}", crate::markdown(d));
                }
                Ok(())
            }
            AdvisorySubcommand::Create { content, cve } => {
                let mut body = content.json().await?;
                if let Some(cve) = cve {
                    body["cve_id"] = json!(cve);
                }
                let a = client.send(Method::POST, "", Some(&body)).await?;
                println!("Created draft {}", str_of(&a, "ghsa_id"));
                println!("{}", str_of(&a, "html_url"));
                Ok(())
            }
            AdvisorySubcommand::Publish { id } => set_state(&client, &id, "published").await,
            AdvisorySubcommand::Close { id } => set_state(&client, &id, "closed").await,
            AdvisorySubcommand::Withdraw { id } => set_state(&client, &id, "withdrawn").await,
            AdvisorySubcommand::Comments { id, json } => {
                let list = client
                    .send(Method::GET, &format!("{}/comments", id_path(&id)), None)
                    .await?;
                if json {
                    return print_json(&list);
                }
                for c in list.as_array().map(Vec::as_slice).unwrap_or_default() {
                    println!("{}", format_comment(c));
                    println!("{}\n", crate::markdown(str_of(c, "body")));
                }
                Ok(())
            }
            AdvisorySubcommand::Comment { id, body } => {
                let body = read_body(body).await?;
                let c = client
                    .send(
                        Method::POST,
                        &format!("{}/comments", id_path(&id)),
                        Some(&json!({"body": body})),
                    )
                    .await?;
                println!("Added comment {}", c["id"]);
                Ok(())
            }
            AdvisorySubcommand::CommentEdit { id, comment, body } => {
                let body = read_body(body).await?;
                client
                    .send(
                        Method::PATCH,
                        &comment_path(&id, comment),
                        Some(&json!({"body": body})),
                    )
                    .await?;
                println!("Edited comment {comment}");
                Ok(())
            }
            AdvisorySubcommand::CommentDelete { id, comment } => {
                client
                    .send(Method::DELETE, &comment_path(&id, comment), None)
                    .await?;
                println!("Deleted comment {comment}");
                Ok(())
            }
        }
    }
}

async fn set_state(client: &Client, id: &str, state: &str) -> eyre::Result<()> {
    let a = client
        .send(Method::PATCH, &id_path(id), Some(&json!({"state": state})))
        .await?;
    println!("{} is now {}", str_of(&a, "ghsa_id"), str_of(&a, "state"));
    Ok(())
}

fn print_json(v: &Value) -> eyre::Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn format_list(list: &Value) -> String {
    let items = list.as_array().map(Vec::as_slice).unwrap_or_default();
    if items.is_empty() {
        return "No security advisories\n".to_owned();
    }
    items
        .iter()
        .map(|a| {
            format!(
                "{:<24} {:<9} {:<8} {}\n",
                str_of(a, "ghsa_id"),
                str_of(a, "state"),
                str_of(a, "severity"),
                str_of(a, "summary")
            )
        })
        .collect()
}

fn format_advisory(a: &Value) -> String {
    let mut out = format!(
        "{} ({}, {})\n{}\n",
        str_of(a, "summary"),
        str_of(a, "state"),
        str_of(a, "severity"),
        str_of(a, "ghsa_id"),
    );
    if let Some(cve) = a["cve_id"].as_str() {
        out += &format!("CVE: {cve}\n");
    }
    if let Some(score) = a["cvss"]["score"].as_f64() {
        out += &format!("CVSS: {score} {}\n", str_of(&a["cvss"], "vector_string"));
    }
    for v in a["vulnerabilities"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        out += &format!(
            "Affected: {}/{} {} (patched: {})\n",
            str_of(&v["package"], "ecosystem"),
            str_of(&v["package"], "name"),
            str_of(v, "vulnerable_version_range"),
            str_of(v, "patched_versions"),
        );
    }
    out + str_of(a, "html_url") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn content() -> Content {
        Content {
            title: "XSS".into(),
            body: "-".into(),
            severity: "high".into(),
            cvss: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N".into()),
            cwe: vec!["CWE-79".into()],
            ecosystem: Some("npm".into()),
            package: Some("tool".into()),
            vulnerable: Some(">= 1.0, < 1.4.2".into()),
            patched: Some("1.4.2".into()),
        }
    }

    #[test]
    fn request_body() {
        let v = content().to_json("details".into());
        assert_eq!(v["summary"], "XSS");
        assert_eq!(v["description"], "details");
        assert_eq!(v["cwe_ids"], json!(["CWE-79"]));
        assert_eq!(v["vulnerabilities"][0]["package"]["name"], "tool");
        assert_eq!(v["vulnerabilities"][0]["patched_versions"], "1.4.2");
        assert!(
            v["cvss_vector_string"]
                .as_str()
                .unwrap()
                .starts_with("CVSS:3.1/")
        );

        let mut c = content();
        (c.ecosystem, c.package, c.cvss) = (None, None, None);
        let v = c.to_json(String::new());
        assert_eq!(v["vulnerabilities"], json!([]));
        assert!(v.get("cvss_vector_string").is_none());
    }

    #[test]
    fn output() {
        let a = json!({
            "ghsa_id": "DAMAST-9c8v-8xxg-hcfp", "state": "published", "severity": "high", "summary": "XSS",
            "cve_id": "CVE-2026-1", "cvss": {"score": 6.1, "vector_string": "CVSS:3.1/..."},
            "vulnerabilities": [{"package": {"ecosystem": "npm", "name": "tool"}, "vulnerable_version_range": "< 2", "patched_versions": "2"}],
            "html_url": "https://git.example.com/o/r/security/advisories/DAMAST-9c8v-8xxg-hcfp",
        });
        let list = format_list(&json!([a]));
        assert!(
            list.starts_with("DAMAST-9c8v-8xxg-hcfp    published high     XSS"),
            "{list}"
        );
        assert_eq!(format_list(&json!([])), "No security advisories\n");
        let view = format_advisory(&a);
        assert!(view.contains("CVE: CVE-2026-1\n"));
        assert!(view.contains("CVSS: 6.1"));
        assert!(view.contains("Affected: npm/tool < 2 (patched: 2)\n"));
    }

    #[test]
    fn errors() {
        let e = error_for(StatusCode::TOO_MANY_REQUESTS, Some("too many".into()));
        assert!(e.to_string().contains("report limit"));
        let e = error_for(StatusCode::UNPROCESSABLE_ENTITY, None);
        assert!(e.to_string().contains("422"));
        assert!(NOT_DAMAST.contains("plain Forgejo"));
        assert_eq!(id_path("DAMAST-1/../x"), "/DAMAST-1%2F..%2Fx");
        assert_eq!(comment_path("DAMAST-1", 7), "/DAMAST-1/comments/7");
        let c = json!({"id": 7, "user": {"login": "alice"}, "created_at": "2026-11-05T05:34:56Z", "body": "x"});
        assert_eq!(format_comment(&c), "#7 alice (2026-11-05T05:34:56Z)");
    }

    #[test]
    fn cli() {
        let ok = |args: &[&str]| {
            crate::App::try_parse_from(args)
                .map_err(|e| e.to_string())
                .unwrap()
        };
        ok(&["mast", "security", "policy"]);
        ok(&[
            "mast", "security", "report", "-t", "x", "-b", "-", "--repo", "o/r",
        ]);
        ok(&["mast", "advisory", "list", "--osv"]);
        ok(&["mast", "advisory", "list", "--state", "draft", "--json"]);
        ok(&["mast", "advisory", "view", "DAMAST-x", "--osv"]);
        ok(&[
            "mast",
            "advisory",
            "create",
            "-t",
            "x",
            "-b",
            "y",
            "--ecosystem",
            "npm",
            "--package",
            "p",
            "--cve",
            "CVE-2026-1",
        ]);
        ok(&["mast", "advisory", "publish", "DAMAST-x"]);
        ok(&["mast", "advisory", "close", "DAMAST-x"]);
        ok(&["mast", "advisory", "withdraw", "DAMAST-x"]);
        ok(&["mast", "advisory", "comments", "DAMAST-x", "--json"]);
        ok(&["mast", "advisory", "comment", "DAMAST-x", "-b", "-"]);
        ok(&[
            "mast",
            "advisory",
            "comment-edit",
            "DAMAST-x",
            "7",
            "-b",
            "fixed",
        ]);
        ok(&["mast", "advisory", "comment-delete", "DAMAST-x", "7"]);
        assert!(
            crate::App::try_parse_from(["mast", "advisory", "comment-delete", "DAMAST-x", "seven"])
                .is_err()
        );
        assert!(
            crate::App::try_parse_from(["mast", "advisory", "list", "--osv", "--json"]).is_err()
        );
        assert!(
            crate::App::try_parse_from([
                "mast",
                "advisory",
                "create",
                "-t",
                "x",
                "-b",
                "y",
                "--package",
                "p"
            ])
            .is_err()
        );
    }
}
