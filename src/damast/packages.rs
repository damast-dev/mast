//! Repo-bound packages of Damast: `mast package ...`.
//!
//! `list`, `info`, `link`, `unlink` and `delete` use Forgejo's package API and work on any server.
//! `inherit` and `settings` use Damast's `/api/v1/damast/packages/...` routes. Plain Forgejo has none;
//! these commands then say so instead of printing a bare 404.

use clap::{Args, Subcommand, ValueEnum};
use eyre::{OptionExt, bail, eyre};
use forgejo_api::{
    ApiErrorKind, Forgejo, ForgejoError,
    structs::{ListPackagesQuery, ListPackagesQueryType, Package},
};
use futures::TryStreamExt;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use url::Url;

use crate::{
    keys::{KeyInfo, LoginInfo},
    repo::RepoInfo,
};

const NOT_DAMAST: &str = "this server runs plain Forgejo: its packages only follow owner permissions, and `mast package inherit` / `mast package settings` need Damast (https://damast.dev)";

#[derive(Args, Clone, Debug)]
pub struct PackageCommand {
    /// The git remote whose server to use
    #[clap(long, short = 'R')]
    remote: Option<String>,
    #[clap(subcommand)]
    command: PackageSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum PackageSubcommand {
    /// List the packages of a user or organization (default: yours)
    List {
        owner: Option<String>,
        /// Only packages of this type (npm, container, maven, generic, ...)
        #[clap(long, short)]
        r#type: Option<String>,
    },
    /// Show a package: its versions, its repository and whether that repository's access counts
    Info {
        #[clap(flatten)]
        package: PackageRef,
    },
    /// Link a package to a repository of the same owner
    Link {
        #[clap(flatten)]
        package: PackageRef,
        /// Repository name, without the owner
        repo: String,
    },
    /// Unlink a package from its repository
    Unlink {
        #[clap(flatten)]
        package: PackageRef,
    },
    /// Turn on or off that readers and writers of the linked repository may download and publish (Damast)
    Inherit {
        #[clap(flatten)]
        package: PackageRef,
        state: Switch,
    },
    /// Show or change who may create new packages for a user or organization (Damast)
    Settings {
        owner: String,
        /// `owners`: only people who may publish for the owner (default). `repo_writers`: repository
        /// writers and workflows too, in container, npm, Maven, NuGet, RubyGems and generic registries
        #[clap(long, value_enum)]
        new_packages: Option<NewPackages>,
    },
    /// Delete one version of a package
    Delete {
        #[clap(flatten)]
        package: PackageRef,
        version: String,
    },
}

#[derive(Args, Clone, Debug)]
pub struct PackageRef {
    /// User or organization that owns the package
    owner: String,
    /// Package type (npm, container, maven, generic, ...)
    r#type: String,
    /// Package name
    name: String,
}

impl PackageRef {
    fn label(&self) -> String {
        format!("{}/{}/{}", self.owner, self.r#type, self.name)
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    On,
    Off,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewPackages {
    Owners,
    #[value(name = "repo_writers")]
    RepoWriters,
}

impl NewPackages {
    fn as_str(self) -> &'static str {
        match self {
            NewPackages::Owners => "owners",
            NewPackages::RepoWriters => "repo_writers",
        }
    }
}

impl PackageCommand {
    pub async fn run(self, keys: &mut KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        let c = Client::new(keys, host_name, self.remote.as_deref()).await?;
        match self.command {
            PackageSubcommand::List { owner, r#type } => {
                let owner = match owner {
                    Some(owner) => owner,
                    None => c
                        .api
                        .user_get_current()
                        .await?
                        .login
                        .ok_or_eyre("the server didn't say who you are; name an owner")?,
                };
                let versions = c.versions(&owner, r#type.as_deref(), None).await?;
                if versions.is_empty() {
                    println!("{owner} has no packages you can see");
                }
                for line in list_lines(&versions) {
                    println!("{line}");
                }
            }
            PackageSubcommand::Info { package } => c.info(&package).await?,
            PackageSubcommand::Link { package, repo } => {
                c.api
                    .link_package(&package.owner, &package.r#type, &package.name, &repo)
                    .await
                    .map_err(|e| {
                        explain(
                            e,
                            "it is already linked to a repository: `mast package unlink` it first",
                            &package.owner,
                        )
                    })?;
                println!("linked {} to {}/{repo}", package.label(), package.owner);
            }
            PackageSubcommand::Unlink { package } => {
                match c
                    .api
                    .unlink_package(&package.owner, &package.r#type, &package.name)
                    .await
                {
                    // Forgejo answers 204, its API description (and so the client) says 201
                    Err(ForgejoError::UnexpectedStatusCode(StatusCode::NO_CONTENT)) | Ok(()) => {}
                    Err(e) => bail!(explain(
                        e,
                        "it isn't linked to a repository",
                        &package.owner
                    )),
                }
                println!("unlinked {}", package.label());
            }
            PackageSubcommand::Inherit { package, state } => {
                let v = c
                    .package_settings(
                        &package,
                        Some(json!({"inherit_repo_access": state == Switch::On})),
                    )
                    .await?;
                println!("{}: {}", package.label(), inherit_text(&v));
            }
            PackageSubcommand::Settings {
                owner,
                new_packages,
            } => {
                let body = new_packages.map(|n| json!({"new_packages": n.as_str()}));
                let v = c.owner_settings(&owner, body).await?;
                println!("{}", settings_text(&owner, &v));
            }
            PackageSubcommand::Delete { package, version } => {
                c.api
                    .delete_package(&package.owner, &package.r#type, &package.name, &version)
                    .await
                    .map_err(|e| explain(e, "bad request", &package.owner))?;
                println!("deleted {} {version}", package.label());
            }
        }
        Ok(())
    }
}

struct Client {
    api: Forgejo,
    http: reqwest::Client,
    host: Url,
    token: Option<String>,
}

impl Client {
    async fn new(
        keys: &mut KeyInfo,
        host: Option<&str>,
        remote: Option<&str>,
    ) -> eyre::Result<Self> {
        let info = RepoInfo::get_current(host, None, remote, keys)?;
        let api = keys.get_api(info.host_url()).await?; // refreshes an expired OAuth token
        let token = keys.get_login(info.host_url()).map(|login| match login {
            LoginInfo::Application { token } | LoginInfo::OAuth { token, .. } => token.clone(),
        });
        let http = reqwest::Client::builder()
            .user_agent(crate::USER_AGENT)
            .build()?;
        Ok(Self {
            api,
            http,
            host: info.host_url().clone(),
            token,
        })
    }

    /// All visible versions of an owner's packages, newest first; `name` keeps one package.
    async fn versions(
        &self,
        owner: &str,
        r#type: Option<&str>,
        name: Option<&str>,
    ) -> eyre::Result<Vec<Package>> {
        let query = ListPackagesQuery {
            r#type: r#type.map(parse_type).transpose()?,
            q: name.map(str::to_owned),
        };
        let mut versions: Vec<Package> = self
            .api
            .list_packages(owner, query)
            .stream()
            .try_collect()
            .await
            .map_err(|e| match status_of(&e) {
                Some(StatusCode::FORBIDDEN) => {
                    eyre!("you may not see the packages of {owner}: Forgejo's package API needs read access to the owner")
                }
                Some(StatusCode::NOT_FOUND) => {
                    eyre!("user or organization {owner} not found, or you may not see it")
                }
                _ => e.into(),
            })?;
        if let Some(name) = name {
            // `q` matches substrings
            versions.retain(|p| {
                p.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
            });
        }
        Ok(versions)
    }

    async fn info(&self, package: &PackageRef) -> eyre::Result<()> {
        let versions = self
            .versions(&package.owner, Some(&package.r#type), Some(&package.name))
            .await?;
        let first = versions
            .first()
            .ok_or_else(|| eyre!("package {} not found", package.label()))?;
        println!("{}", package.label());
        let repo = repo_of(first);
        println!("Repository: {}", repo.as_deref().unwrap_or("none"));
        let access = match self.send(Method::GET, &package_path(package), None).await? {
            (StatusCode::OK, v) if repo.is_none() => {
                format!("{}; no repository is linked", inherit_text(&v))
            }
            (StatusCode::OK, v) => inherit_text(&v),
            (StatusCode::NOT_FOUND, _) => "plain Forgejo, owner permissions only".to_owned(),
            (status, _) => format!("unknown ({status})"),
        };
        println!("Repository access: {access}");
        let names: Vec<&str> = versions
            .iter()
            .filter_map(|p| p.version.as_deref())
            .collect();
        println!("Versions ({}): {}", names.len(), names.join(", "));
        Ok(())
    }

    /// GET or (with a body) PATCH the Damast settings of a package.
    async fn package_settings(
        &self,
        package: &PackageRef,
        body: Option<Value>,
    ) -> eyre::Result<Value> {
        let method = if body.is_some() {
            Method::PATCH
        } else {
            Method::GET
        };
        match self
            .send(method, &package_path(package), body.as_ref())
            .await?
        {
            (StatusCode::OK, v) => Ok(v),
            (StatusCode::NOT_FOUND, _) => {
                let exists = !self
                    .versions(&package.owner, Some(&package.r#type), Some(&package.name))
                    .await?
                    .is_empty();
                bail!(not_found(exists, &format!("package {}", package.label())))
            }
            (StatusCode::FORBIDDEN, _) => {
                bail!(
                    "you need package write access for {} to change this",
                    package.owner
                )
            }
            (status, v) => Err(api_error(status, &v)),
        }
    }

    /// GET or (with a body) PATCH an owner's Damast package settings.
    async fn owner_settings(&self, owner: &str, body: Option<Value>) -> eyre::Result<Value> {
        let method = if body.is_some() {
            Method::PATCH
        } else {
            Method::GET
        };
        let path = ["api", "v1", "damast", "packages", owner, "settings"];
        match self.send(method, &path, body.as_ref()).await? {
            (StatusCode::OK, v) => Ok(v),
            (StatusCode::NOT_FOUND, _) => {
                let exists = self.api.user_get(owner).await.is_ok();
                bail!(not_found(exists, &format!("user or organization {owner}")))
            }
            (StatusCode::FORBIDDEN, _) => bail!("only owners of {owner} may see and change this"),
            (status, v) => Err(api_error(status, &v)),
        }
    }

    async fn send(
        &self,
        method: Method,
        path: &[&str],
        body: Option<&Value>,
    ) -> eyre::Result<(StatusCode, Value)> {
        let mut req = self.http.request(method, api_url(&self.host, path));
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
        Ok((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    }
}

fn package_path(p: &PackageRef) -> [&str; 8] {
    [
        "api", "v1", "damast", "packages", &p.owner, &p.r#type, &p.name, "settings",
    ]
}

/// Appends percent-encoded segments to the server URL, keeping a sub-path the server may live under.
fn api_url(host: &Url, segments: &[&str]) -> Url {
    let mut url = host.clone();
    url.path_segments_mut()
        .expect("http(s) URLs have a path")
        .pop_if_empty()
        .extend(segments);
    url
}

fn parse_type(t: &str) -> eyre::Result<ListPackagesQueryType> {
    serde_json::from_value(json!(t)).map_err(|_| eyre!("unknown package type {t:?}"))
}

/// A Damast route answered 404: either the thing is missing, or the server has no such route.
fn not_found(exists: bool, what: &str) -> String {
    if exists {
        NOT_DAMAST.to_owned()
    } else {
        format!("{what} not found, or you may not see it")
    }
}

fn api_error(status: StatusCode, v: &Value) -> eyre::Report {
    eyre!(
        "{}",
        v["message"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| status.to_string())
    )
}

/// The generated client keeps only the status of most errors, not the server's message.
fn status_of(e: &ForgejoError) -> Option<StatusCode> {
    match e {
        ForgejoError::UnexpectedStatusCode(status) => Some(*status),
        ForgejoError::ApiError(e) => match e.kind {
            ApiErrorKind::Forbidden => Some(StatusCode::FORBIDDEN),
            ApiErrorKind::NotFound { .. } => Some(StatusCode::NOT_FOUND),
            _ => None,
        },
        _ => None,
    }
}

fn explain(e: ForgejoError, bad_request: &str, owner: &str) -> eyre::Report {
    match status_of(&e) {
        Some(StatusCode::BAD_REQUEST) => eyre!("{bad_request}"),
        Some(StatusCode::FORBIDDEN) => eyre!(
            "you need package write access for {owner} (and write access to the repository when linking)"
        ),
        Some(StatusCode::NOT_FOUND) => {
            eyre!("package or repository not found, or you may not see it")
        }
        _ => e.into(),
    }
}

fn repo_of(p: &Package) -> Option<String> {
    p.repository.as_ref().and_then(|r| r.full_name.clone())
}

fn inherit_text(v: &Value) -> String {
    if v["inherit_repo_access"].as_bool() == Some(true) {
        "on (readers and writers of the linked repository may download and publish new versions)"
            .to_owned()
    } else {
        "off (owner permissions only)".to_owned()
    }
}

fn settings_text(owner: &str, v: &Value) -> String {
    match v["new_packages"].as_str() {
        Some("repo_writers") => format!(
            "new packages for {owner}: repo_writers (repository writers and workflows may create container, npm, Maven, NuGet, RubyGems and generic packages)"
        ),
        _ => format!("new packages for {owner}: owners (only people who may publish for {owner})"),
    }
}

/// One line per package: versions arrive newest first, one entry per version.
fn list_lines(versions: &[Package]) -> Vec<String> {
    let mut packages: Vec<(&Package, usize)> = Vec::new();
    for v in versions {
        match packages
            .iter_mut()
            .find(|(p, _)| p.r#type == v.r#type && p.name == v.name)
        {
            Some((_, n)) => *n += 1,
            None => packages.push((v, 1)),
        }
    }
    packages
        .into_iter()
        .map(|(p, n)| {
            let mut line = format!(
                "{}/{}  {n} version{}, newest {}",
                p.r#type.as_deref().unwrap_or("?"),
                p.name.as_deref().unwrap_or("?"),
                if n == 1 { "" } else { "s" },
                p.version.as_deref().unwrap_or("?"),
            );
            if let Some(repo) = repo_of(p) {
                line.push_str(&format!("  ({repo})"));
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn version(t: &str, name: &str, v: &str, repo: Option<&str>) -> Package {
        serde_json::from_value(json!({
            "type": t, "name": name, "version": v,
            "repository": repo.map(|r| json!({"full_name": r})),
        }))
        .unwrap()
    }

    #[test]
    fn list_groups_versions() {
        let lines = list_lines(&[
            version("npm", "tool", "1.1.0", Some("acme/tool")),
            version("generic", "tool", "2", None),
            version("npm", "tool", "1.0.0", Some("acme/tool")),
        ]);
        assert_eq!(
            lines,
            [
                "npm/tool  2 versions, newest 1.1.0  (acme/tool)",
                "generic/tool  1 version, newest 2",
            ]
        );
    }

    #[test]
    fn urls_keep_sub_path_and_encode_names() {
        let host = Url::parse("https://example.com/forge/").unwrap();
        let p = PackageRef {
            owner: "acme".into(),
            r#type: "npm".into(),
            name: "@acme/tool".into(),
        };
        assert_eq!(
            api_url(&host, &package_path(&p)).as_str(),
            "https://example.com/forge/api/v1/damast/packages/acme/npm/@acme%2Ftool/settings"
        );
        let host = Url::parse("https://example.com").unwrap();
        assert_eq!(
            api_url(
                &host,
                &["api", "v1", "damast", "packages", "acme", "settings"]
            )
            .as_str(),
            "https://example.com/api/v1/damast/packages/acme/settings"
        );
    }

    #[test]
    fn explains_api_errors() {
        let e = explain(
            ForgejoError::UnexpectedStatusCode(StatusCode::BAD_REQUEST),
            "already linked",
            "acme",
        );
        assert_eq!(e.to_string(), "already linked");
        let e = explain(
            ForgejoError::UnexpectedStatusCode(StatusCode::FORBIDDEN),
            "x",
            "acme",
        );
        assert!(
            e.to_string()
                .starts_with("you need package write access for acme")
        );
        assert_eq!(
            status_of(&ForgejoError::UnexpectedStatusCode(StatusCode::NO_CONTENT)),
            Some(StatusCode::NO_CONTENT)
        );
    }

    #[test]
    fn plain_forgejo_message() {
        assert_eq!(not_found(true, "package acme/npm/tool"), NOT_DAMAST);
        assert_eq!(
            not_found(false, "package acme/npm/tool"),
            "package acme/npm/tool not found, or you may not see it"
        );
    }

    #[test]
    fn texts() {
        assert!(inherit_text(&json!({"inherit_repo_access": true})).starts_with("on "));
        assert!(inherit_text(&json!({"inherit_repo_access": false})).starts_with("off "));
        assert!(
            settings_text("acme", &json!({"new_packages": "repo_writers"}))
                .contains(": repo_writers")
        );
        assert!(settings_text("acme", &json!({"new_packages": "owners"})).contains(": owners"));
        assert!(parse_type("npm").is_ok());
        assert!(parse_type("nope").is_err());
    }

    #[derive(Parser, Debug)]
    struct Cli {
        #[clap(subcommand)]
        command: PackageSubcommand,
    }

    #[test]
    fn parses_commands() {
        let cli = Cli::parse_from(["mast", "inherit", "acme", "npm", "tool", "off"]);
        assert!(matches!(
            cli.command,
            PackageSubcommand::Inherit {
                state: Switch::Off,
                ..
            }
        ));
        let cli = Cli::parse_from(["mast", "settings", "acme", "--new-packages", "repo_writers"]);
        assert!(matches!(
            cli.command,
            PackageSubcommand::Settings {
                new_packages: Some(NewPackages::RepoWriters),
                ..
            }
        ));
        let cli = Cli::parse_from(["mast", "link", "acme", "npm", "tool", "tool-repo"]);
        assert!(
            matches!(cli.command, PackageSubcommand::Link { ref repo, .. } if repo == "tool-repo")
        );
        assert!(
            Cli::try_parse_from(["mast", "settings", "acme", "--new-packages", "everyone"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["mast", "delete", "acme", "npm", "tool"]).is_err());
    }
}
