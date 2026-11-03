use clap::{Args, Subcommand};
use eyre::OptionExt;
use forgejo_api::Forgejo;

use crate::{
    ftl_println, h,
    keys::KeyInfo,
    lh,
    repo::{RepoArg, RepoInfo, RepoName},
};

#[derive(Args, Clone, Debug)]
pub struct TagCommand {
    #[clap(help = h!("arg-remote"))]
    #[clap(long, short = 'R', global = true)]
    remote: Option<String>,

    #[clap(help = h!("arg-repo"))]
    #[clap(long, short, global = true)]
    repo: Option<RepoArg>,

    #[clap(subcommand)]
    command: TagSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum TagSubcommand {
    #[clap(about = h!("cmd-tag-create"))]
    Create {
        name: String,

        #[clap(help = h!("arg-tag-create-body"), long_help = lh!("arg-tag-create-body"))]
        #[clap(long, short)]
        body: Option<Option<String>>,

        #[clap(long, short = 'B')]
        branch: Option<String>,
    },
    #[clap(about = h!("cmd-tag-delete"))]
    Delete { name: String },
    #[clap(about = h!("cmd-tag-list"))]
    List {
        #[clap(long, short, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-tag-view"))]
    View { name: String },
}

impl TagCommand {
    pub async fn run(self, keys: &mut KeyInfo, remote_name: Option<&str>) -> eyre::Result<()> {
        let repo = RepoInfo::get_current(
            remote_name,
            self.repo.as_ref(),
            self.remote.as_deref(),
            keys,
        )?;
        let api = keys.get_api(repo.host_url()).await?;
        let repo = repo
            .name()
            .ok_or_eyre("couldn't get repo name, try specifying with --repo")?;
        match self.command {
            TagSubcommand::Create { name, body, branch } => {
                create_tag(repo, &api, name, body, branch).await?
            }
            TagSubcommand::Delete { name } => delete_tag(repo, &api, name).await?,
            TagSubcommand::List { page } => list_tags(repo, &api, page).await?,
            TagSubcommand::View { name } => view_tag(repo, &api, name).await?,
        }
        Ok(())
    }
}

async fn create_tag(
    repo: &RepoName,
    api: &Forgejo,
    name: String,
    body: Option<Option<String>>,
    branch: Option<String>,
) -> eyre::Result<()> {
    let body = match body {
        Some(Some(body)) => Some(body),
        Some(None) => {
            let mut s = String::new();
            crate::editor(&mut s, Some("md")).await?;
            Some(s)
        }
        None => None,
    };

    let opt = forgejo_api::structs::CreateTagOption {
        message: body,
        tag_name: name.clone(),
        target: branch,
    };
    api.repo_create_tag(repo.owner(), repo.name(), opt).await?;
    ftl_println!("msg-tag-create-success", name);
    Ok(())
}

async fn delete_tag(repo: &RepoName, api: &Forgejo, name: String) -> eyre::Result<()> {
    api.repo_delete_tag(repo.owner(), repo.name(), &name)
        .await?;
    ftl_println!("msg-tag-delete-success", name);
    Ok(())
}

async fn list_tags(repo: &RepoName, api: &Forgejo, page: u32) -> eyre::Result<()> {
    let (_, tags) = api
        .repo_list_tags(repo.owner(), repo.name())
        .page(page)
        .page_size(20)
        .await?;
    for tag in tags {
        if let Some(name) = tag.name.as_deref() {
            println!("{name}");
        }
    }
    Ok(())
}

async fn view_tag(repo: &RepoName, api: &Forgejo, name: String) -> eyre::Result<()> {
    let tag = api.repo_get_tag(repo.owner(), repo.name(), &name).await?;
    let name = tag.name.as_deref().ok_or_eyre("tag does not have name")?;
    let id = tag.id.as_deref().ok_or_eyre("tag does not have name")?;

    let crate::SpecialRender { bold, reset, .. } = crate::special_render();
    println!("{bold}{name}{reset}");
    println!("{id}");
    if let Some(msg) = &tag.message {
        println!();
        println!("{}", crate::markdown(msg));
    }
    Ok(())
}
