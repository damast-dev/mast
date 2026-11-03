use std::path::PathBuf;

use base64ct::Encoding;
use clap::{Args, Subcommand};
use eyre::{Context, OptionExt};
use forgejo_api::Forgejo;

use crate::{
    SpecialRender, ftl_println, h,
    repo::{RepoArg, RepoInfo, RepoName},
};

#[derive(Args, Clone, Debug)]
pub struct WikiCommand {
    #[clap(help = h!("arg-remote"))]
    #[clap(long, short = 'R', global = true)]
    remote: Option<String>,

    #[clap(help = h!("arg-repo"))]
    #[clap(long, short, global = true)]
    repo: Option<RepoArg>,

    #[clap(subcommand)]
    command: WikiSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum WikiSubcommand {
    Contents,
    View {
        page: String,
    },
    Clone {
        #[clap(long, short)]
        path: Option<PathBuf>,

        #[clap(help = h!("arg-wiki-clone-ssh"))]
        #[clap(long, short = 'S')]
        ssh: Option<Option<bool>>,

        #[clap(help = h!("arg-wiki-clone-identity_file"))]
        #[clap(long, short = 'I')]
        identity_file: Option<PathBuf>,
    },
    Browse {
        page: String,
    },
}

impl WikiCommand {
    pub async fn run(self, keys: &mut crate::KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        use WikiSubcommand::*;

        let repo =
            RepoInfo::get_current(host_name, self.repo.as_ref(), self.remote.as_deref(), keys)?;
        let api = keys.get_api(repo.host_url()).await?;
        let repo_name = repo
            .name()
            .ok_or_else(|| eyre::eyre!("couldn't guess repo"))?;

        match self.command {
            Contents => wiki_contents(repo_name, &api).await?,
            View { page } => view_wiki_page(repo_name, &api, &page).await?,
            Clone {
                path,
                ssh,
                identity_file: identity,
            } => {
                let url_host = crate::host_name(repo.host_url());
                let ssh = ssh
                    .unwrap_or_else(|| Some(keys.default_ssh.contains(url_host)))
                    .unwrap_or(true);
                clone_wiki(repo_name, &api, path, ssh, identity).await?;
            }
            Browse { page } => browse_wiki_page(repo_name, &api, &page).await?,
        }
        Ok(())
    }
}

async fn wiki_contents(repo: &RepoName, api: &Forgejo) -> eyre::Result<()> {
    let SpecialRender { bullet, .. } = *crate::special_render();

    let pages = api
        .repo_get_wiki_pages(repo.owner(), repo.name())
        .all()
        .await?;
    for page in pages {
        let title = page
            .title
            .as_deref()
            .ok_or_eyre("page does not have title")?;
        println!("{bullet} {title}");
    }

    Ok(())
}

async fn view_wiki_page(repo: &RepoName, api: &Forgejo, page: &str) -> eyre::Result<()> {
    let SpecialRender { bold, reset, .. } = *crate::special_render();

    let page = api
        .repo_get_wiki_page(repo.owner(), repo.name(), page)
        .await?;

    let title = page
        .title
        .as_deref()
        .ok_or_eyre("page does not have title")?;
    println!("{bold}{title}{reset}");
    println!();

    let contents_b64 = page
        .content_base64
        .as_deref()
        .ok_or_eyre("page does not have content")?;
    let contents = String::from_utf8(base64ct::Base64::decode_vec(contents_b64)?)
        .wrap_err("page content is not utf-8")?;

    println!("{}", crate::markdown(&contents));
    Ok(())
}

async fn browse_wiki_page(repo: &RepoName, api: &Forgejo, page: &str) -> eyre::Result<()> {
    let page = api
        .repo_get_wiki_page(repo.owner(), repo.name(), page)
        .await?;
    let html_url = page
        .html_url
        .as_ref()
        .ok_or_eyre("page does not have html url")?;
    open::that_detached(html_url.as_str()).wrap_err("Failed to open URL")?;
    Ok(())
}

async fn clone_wiki(
    repo: &RepoName,
    api: &Forgejo,
    path: Option<PathBuf>,
    ssh: bool,
    identity_file: Option<PathBuf>,
) -> eyre::Result<()> {
    let repo_data = api.repo_get(repo.owner(), repo.name()).await?;
    let clone_url = if ssh {
        repo_data
            .ssh_url
            .as_ref()
            .ok_or_eyre("repo does not have ssh url")?
    } else {
        repo_data
            .clone_url
            .as_ref()
            .ok_or_eyre("repo does not have clone url")?
    };

    let git_stripped = clone_url
        .as_str()
        .strip_suffix(".git")
        .unwrap_or(clone_url.as_str());
    let clone_url = url::Url::parse(&format!("{}.wiki.git", git_stripped))?;

    let repo_name = repo_data
        .name
        .as_deref()
        .ok_or_eyre("repo does not have name")?;
    let repo_full_name = repo_data
        .full_name
        .as_deref()
        .ok_or_eyre("repo does not have full name")?;

    let path = path.unwrap_or_else(|| PathBuf::from(format!("./{repo_name}-wiki")));

    crate::repo::clone_repo(&clone_url, &path, identity_file.as_deref())?;
    ftl_println!(
        "msg-wiki-clone-success",
        repo = repo_full_name,
        path = path.display().to_string(),
    );

    Ok(())
}
