use clap::{Args, Subcommand};
use eyre::OptionExt;
use forgejo_api::{
    Forgejo,
    structs::{
        CreateLabelOption, CreateOrgOption, EditLabelOption, EditOrgOption, OrgListLabelsQuery,
    },
};
use futures::{TryStreamExt, future};

use crate::{SpecialRender, ftl_bail, ftl_eprintln, ftl_print, ftl_println, h, lh, repo::RepoInfo};

mod team;

#[derive(Args, Clone, Debug)]
pub struct OrgCommand {
    #[clap(help = h!("arg-remote"))]
    #[clap(long, short = 'R')]
    remote: Option<String>,
    #[clap(subcommand)]
    command: OrgSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum OrgSubcommand {
    #[clap(about = h!("cmd-org-list"))]
    List {
        #[clap(help = h!("arg-org-list-page"))]
        #[clap(long, short, default_value_t = 1)]
        page: u32,

        #[clap(help = h!("arg-org-list-only_member_of"))]
        #[clap(long, short, conflicts_with = "page")]
        only_member_of: bool,
    },
    #[clap(about = h!("cmd-org-view"))]
    View {
        #[clap(help = h!("arg-org-view-name"))]
        name: String,
    },
    #[clap(about = h!("cmd-org-create"))]
    Create {
        #[clap(help = h!("arg-org-create-name"), long_help = lh!("arg-org-create-name"))]
        name: String,

        #[clap(flatten)]
        options: OrgOptions,
    },
    #[clap(about = h!("cmd-org-edit"))]
    Edit {
        #[clap(help = h!("arg-org-edit-name"), long_help = lh!("arg-org-edit-name"))]
        name: String,

        #[clap(flatten)]
        options: OrgOptions,
    },
    #[clap(about = h!("cmd-org-edit"))]
    Activity {
        #[clap(help = h!("arg-org-edit-name"))]
        name: String,
    },
    #[clap(about = h!("cmd-org-members"))]
    Members {
        #[clap(help = h!("arg-org-members-org"))]
        org: String,

        #[clap(help = h!("arg-org-members-page"))]
        #[clap(long, short, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-org-visibility"))]
    Visibility {
        #[clap(help = h!("arg-org-visibility-org"))]
        org: String,

        #[clap(help = h!("arg-org-visibility-set"))]
        #[clap(long, short)]
        set: Option<OrgMemberVisibility>,
    },
    #[clap(subcommand)]
    Team(team::TeamSubcommand),
    #[clap(subcommand)]
    Label(LabelSubcommand),
    #[clap(subcommand)]
    Repo(RepoSubcommand),
}

#[derive(Args, Clone, Debug)]
pub struct OrgOptions {
    #[clap(help = h!("arg-org-options-full_name"), long_help = lh!("arg-org-options-full_name"))]
    #[clap(long, short)]
    full_name: Option<String>,

    #[clap(help = h!("arg-org-options-description"))]
    #[clap(long, short)]
    description: Option<String>,

    #[clap(help = h!("arg-org-options-email"))]
    #[clap(long, short)]
    email: Option<String>,

    #[clap(help = h!("arg-org-options-location"))]
    #[clap(long, short)]
    location: Option<String>,

    #[clap(help = h!("arg-org-options-website"))]
    #[clap(long, short)]
    website: Option<String>,

    #[clap(help = h!("arg-org-options-visibility"), long_help = lh!("arg-org-options-visibility"))]
    #[clap(long, short)]
    visibility: Option<OrgVisibility>,

    #[clap(help = h!("arg-org-options-admin_can_change_team_access"))]
    #[clap(long, short)]
    admin_can_change_team_access: Option<bool>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgMemberVisibility {
    Private,
    Public,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgVisibility {
    Private,
    Limited,
    Public,
}

impl From<OrgVisibility> for forgejo_api::structs::CreateOrgOptionVisibility {
    fn from(val: OrgVisibility) -> Self {
        use forgejo_api::structs::CreateOrgOptionVisibility as ApiVis;
        match val {
            OrgVisibility::Private => ApiVis::Private,
            OrgVisibility::Limited => ApiVis::Limited,
            OrgVisibility::Public => ApiVis::Public,
        }
    }
}

impl From<OrgVisibility> for forgejo_api::structs::EditOrgOptionVisibility {
    fn from(val: OrgVisibility) -> Self {
        use forgejo_api::structs::EditOrgOptionVisibility as ApiVis;
        match val {
            OrgVisibility::Private => ApiVis::Private,
            OrgVisibility::Limited => ApiVis::Limited,
            OrgVisibility::Public => ApiVis::Public,
        }
    }
}

impl OrgCommand {
    pub async fn run(self, keys: &mut crate::KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        let repo = RepoInfo::get_current(host_name, None, self.remote.as_deref(), keys)?;
        let api = keys.get_api(repo.host_url()).await?;
        match self.command {
            OrgSubcommand::List {
                page,
                only_member_of,
            } => list_orgs(&api, page, only_member_of).await?,
            OrgSubcommand::View { name } => view_org(&api, name).await?,
            OrgSubcommand::Create { name, options } => create_org(&api, name, options).await?,
            OrgSubcommand::Edit { name, options } => edit_org(&api, name, options).await?,
            OrgSubcommand::Activity { name } => list_activity(&api, name).await?,
            OrgSubcommand::Members { org, page } => list_org_members(&api, org, page).await?,
            OrgSubcommand::Visibility { org, set } => member_visibility(&api, org, set).await?,
            OrgSubcommand::Team(subcommand) => subcommand.run(&api).await?,
            OrgSubcommand::Label(subcommand) => subcommand.run(&api).await?,
            OrgSubcommand::Repo(subcommand) => subcommand.run(keys, &repo, &api).await?,
        }
        Ok(())
    }
}

fn is_valid_name_char(c: char) -> bool {
    match c {
        '-' | '_' | '.' => true,
        _ => c.is_ascii_alphanumeric(),
    }
}

async fn list_orgs(api: &Forgejo, page: u32, only_member_of: bool) -> eyre::Result<()> {
    let (total, orgs) = if only_member_of {
        let orgs = api.org_list_current_user_orgs().await?;
        (None, orgs)
    } else {
        let (headers, orgs) = api.org_get_all().page(page).await?;
        (Some(headers.x_total_count.unwrap_or_default() as u64), orgs)
    };

    if orgs.is_empty() {
        ftl_eprintln!("msg-org-list-no_results");
    } else {
        let SpecialRender {
            bullet,
            bold,
            reset,
            ..
        } = *crate::special_render();
        for org in orgs {
            let name = org.name.ok_or_eyre("org does not have name")?;
            println!("{bullet} {bold}{name}{reset}");
        }
        if let Some(total) = total {
            ftl_eprintln!("msg-org-list-page_number", page, total = total.div_ceil(20));
        }
    }
    Ok(())
}

async fn view_org(api: &Forgejo, name: String) -> eyre::Result<()> {
    let org = api.org_get(&name).await?;

    let SpecialRender {
        bold, dash, reset, ..
    } = *crate::special_render();

    let name = org.name.as_deref().ok_or_eyre("org does not have name")?;
    let full_name = org.full_name.as_deref().filter(|n| !n.is_empty());

    let visibility = org
        .visibility
        .as_deref()
        .ok_or_eyre("new org does not have visibility")?;

    let member_count = match api.org_list_members(name).page(1).page_size(1).await {
        Ok((members_headers, _)) => members_headers.x_total_count.unwrap_or_default(),
        Err(_) => {
            let (members_headers, _) = api
                .org_list_public_members(name)
                .page(1)
                .page_size(1)
                .await?;
            members_headers.x_total_count.unwrap_or_default()
        }
    };
    let team_count = api
        .org_list_teams(name)
        .page(1)
        .page_size(1)
        .await?
        .0
        .x_total_count
        .unwrap_or_default();

    ftl_print!("msg-org-view-org_name", full_name, name);
    print!(" {dash} ");
    ftl_println!("msg-org-view-visibility", visibility);
    ftl_print!("msg-org-view-member_count", member_count);
    print!(" {dash} ");
    ftl_println!("msg-org-view-team_count", team_count);
    println!();

    let mut first = true;
    if let Some(website) = &org.website
        && !website.is_empty()
    {
        print!("{bold}{website}{reset}");
        first = false;
    }
    if let Some(email) = &org.email
        && !email.is_empty()
    {
        if !first {
            print!(" {dash} ");
        }
        print!("{email}");
        first = false;
    }
    if let Some(location) = &org.location
        && !location.is_empty()
    {
        if !first {
            print!(" {dash} ");
        }
        print!("{location}");
        first = false;
    }
    if !first {
        println!();
    }

    if let Some(description) = &org.description
        && !description.is_empty()
    {
        println!("\n{}\n", crate::markdown(description));
    }

    Ok(())
}

async fn create_org(api: &Forgejo, name: String, options: OrgOptions) -> eyre::Result<()> {
    if !name.chars().all(is_valid_name_char) {
        ftl_bail!("msg-org-create-invalid_character");
    }
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        ftl_bail!("msg-org-create-invalid_starting_character");
    }
    if !name
        .chars()
        .last()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        ftl_bail!("msg-org-create-invalid_ending_character");
    }
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        // because of the prior check, if it isn't alphanumeric, it's definitely one of - _ or .
        if !c.is_alphanumeric() && !chars.peek().is_some_and(|c| c.is_alphanumeric()) {
            ftl_bail!("msg-org-create-invalid_consecutive_characters");
        }
    }
    let opt = CreateOrgOption {
        description: options.description,
        email: options.email,
        full_name: options.full_name,
        location: options.location,
        repo_admin_change_team_access: options.admin_can_change_team_access,
        username: name,
        visibility: options.visibility.map(|v| v.into()),
        website: options.website,
    };
    let new_org = api.org_create(opt).await?;

    let name = new_org.name.ok_or_eyre("new org does not have name")?;
    let full_name = new_org.full_name.as_deref().filter(|n| !n.is_empty());
    let visibility = new_org
        .visibility
        .ok_or_eyre("new org does not have visibility")?;

    ftl_println!("msg-org-create-success", visibility, full_name, name);
    Ok(())
}

async fn edit_org(api: &Forgejo, name: String, options: OrgOptions) -> eyre::Result<()> {
    let opt = EditOrgOption {
        description: options.description,
        email: options.email,
        full_name: options.full_name,
        location: options.location,
        repo_admin_change_team_access: options.admin_can_change_team_access,
        visibility: options.visibility.map(|v| v.into()),
        website: options.website,
    };
    api.org_edit(&name, opt).await?;
    Ok(())
}

async fn list_activity(api: &Forgejo, name: String) -> eyre::Result<()> {
    let query = forgejo_api::structs::OrgListActivityFeedsQuery::default();
    let (_, feed) = api.org_list_activity_feeds(&name, query).await?;

    for activity in feed {
        crate::user::print_activity(&activity)?;
    }
    Ok(())
}

async fn list_org_members(api: &Forgejo, org: String, page: u32) -> eyre::Result<()> {
    let my_username = api
        .user_get_current()
        .await?
        .login
        .ok_or_eyre("current user does not have username")?;
    let (count, users) = if api.org_is_member(&org, &my_username).await.is_ok() {
        let (headers, users) = api.org_list_members(&org).page(page).await?;
        (headers.x_total_count.unwrap_or_default() as u64, users)
    } else {
        let (headers, users) = api.org_list_public_members(&org).page(page).await?;
        (headers.x_total_count.unwrap_or_default() as u64, users)
    };

    let SpecialRender { bullet, .. } = crate::special_render();
    if users.is_empty() {
        ftl_println!("msg-org-members-no_results");
    } else {
        for user in users {
            let username = user
                .login
                .as_deref()
                .ok_or_eyre("repo does not have full name")?;
            let full_name = user.full_name.as_deref().filter(|s| !s.is_empty());
            print!("{bullet} ");
            ftl_println!("msg-org-members-entry", full_name, username);
        }
        ftl_println!(
            "msg-org-members-page_number",
            page,
            total = count.div_ceil(20)
        );
    }
    Ok(())
}

async fn member_visibility(
    api: &Forgejo,
    org_name: String,
    visibility: Option<OrgMemberVisibility>,
) -> eyre::Result<()> {
    let username = api
        .user_get_current()
        .await?
        .login
        .ok_or_eyre("current user does not have username")?;
    if api.org_is_member(&org_name, &username).await.is_ok() {
        match visibility {
            Some(OrgMemberVisibility::Private) => {
                api.org_conceal_member(&org_name, &username).await?;
                ftl_println!("msg-org-visibility-set_private", org_name);
            }
            Some(OrgMemberVisibility::Public) => {
                api.org_conceal_member(&org_name, &username).await?;
                ftl_println!("msg-org-visibility-set_public", org_name);
            }
            None => {
                if api.org_is_public_member(&org_name, &username).await.is_ok() {
                    ftl_println!("msg-org-visibility-public", org_name);
                } else {
                    ftl_println!("msg-org-visibility-private", org_name);
                }
            }
        }
    } else {
        ftl_println!("msg-org-visibility-not_member", org_name);
    }
    Ok(())
}

#[derive(Subcommand, Clone, Debug)]
pub enum LabelSubcommand {
    #[clap(about = h!("cmd-org-label-list"))]
    List {
        #[clap(help = h!("arg-org-label-list-org"))]
        org: String,
    },
    #[clap(about = h!("cmd-org-label-add"))]
    Add {
        #[clap(help = h!("arg-org-label-add-org"))]
        org: String,

        #[clap(help = h!("arg-org-label-add-name"))]
        name: String,

        #[clap(help = h!("arg-org-label-add-color"))]
        color: String,

        #[clap(help = h!("arg-org-label-add-description"))]
        #[clap(long, short)]
        description: Option<String>,

        #[clap(help = h!("arg-org-label-add-exclusive"))]
        #[clap(long, short)]
        exclusive: bool,
    },
    #[clap(about = h!("cmd-org-label-edit"))]
    Edit {
        #[clap(help = h!("arg-org-label-edit-org"))]
        org: String,

        #[clap(help = h!("arg-org-label-edit-name"))]
        name: String,

        #[clap(help = h!("arg-org-label-edit-new_name"))]
        #[clap(long, short)]
        new_name: Option<String>,

        #[clap(help = h!("arg-org-label-edit-color"))]
        #[clap(long, short)]
        color: Option<String>,

        #[clap(help = h!("arg-org-label-edit-description"))]
        #[clap(long, short)]
        description: Option<String>,

        #[clap(help = h!("arg-org-label-edit-exclusive"))]
        #[clap(long, short)]
        exclusive: bool,

        #[clap(help = h!("arg-org-label-edit-archived"))]
        #[clap(long, short)]
        archived: Option<bool>,
    },
    #[clap(about = h!("cmd-org-label-rm"))]
    Rm {
        #[clap(help = h!("arg-org-label-rm-org"))]
        org: String,

        #[clap(help = h!("arg-org-label-rm-label"))]
        label: String,
    },
}

impl LabelSubcommand {
    async fn run(self, api: &Forgejo) -> eyre::Result<()> {
        match self {
            LabelSubcommand::List { org } => list_org_labels(api, org).await?,
            LabelSubcommand::Add {
                org,
                name,
                color,
                description,
                exclusive,
            } => add_org_label(api, org, name, color, description, exclusive).await?,
            LabelSubcommand::Edit {
                org,
                name,
                new_name,
                color,
                description,
                exclusive,
                archived,
            } => {
                edit_org_label(
                    api,
                    org,
                    name,
                    new_name,
                    color,
                    description,
                    exclusive,
                    archived,
                )
                .await?
            }
            LabelSubcommand::Rm { org, label } => remove_org_label(api, org, label).await?,
        }
        Ok(())
    }
}

async fn list_org_labels(api: &Forgejo, org: String) -> eyre::Result<()> {
    let labels = api
        .org_list_labels(&org, OrgListLabelsQuery::default())
        .all()
        .await?;
    crate::render_label_list(&labels)?;
    Ok(())
}

async fn find_label_by_name(
    api: &Forgejo,
    org: &str,
    name: &str,
) -> eyre::Result<Option<forgejo_api::structs::Label>> {
    Ok(api
        .org_list_labels(org, OrgListLabelsQuery::default())
        .stream()
        .try_filter(|label| {
            future::ready(
                label
                    .name
                    .as_deref()
                    .is_some_and(|label_name| label_name == name),
            )
        })
        .try_next()
        .await?)
}

async fn add_org_label(
    api: &Forgejo,
    org: String,
    name: String,
    color: String,
    description: Option<String>,
    exclusive: bool,
) -> eyre::Result<()> {
    let color = color
        .strip_prefix("#")
        .map(|s| s.to_owned())
        .unwrap_or(color);
    let opt = CreateLabelOption {
        color,
        description,
        exclusive: Some(exclusive),
        is_archived: Some(false),
        name,
    };
    let label = api.org_create_label(&org, opt).await?;
    ftl_println!(
        "msg-org-label-add-success",
        label = crate::render_label(&label)?
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn edit_org_label(
    api: &Forgejo,
    org: String,
    name: String,
    new_name: Option<String>,
    color: Option<String>,
    description: Option<String>,
    exclusive: bool,
    archived: Option<bool>,
) -> eyre::Result<()> {
    let old_label = find_label_by_name(api, &org, &name)
        .await?
        .ok_or_eyre("label not found")?;
    let id = old_label.id.ok_or_eyre("label does not have id")?;
    let color = color.map(|color| {
        color
            .strip_prefix("#")
            .map(|s| s.to_owned())
            .unwrap_or(color)
    });
    let opt = EditLabelOption {
        color,
        description,
        exclusive: Some(exclusive),
        is_archived: archived,
        name: new_name,
    };
    let label = api.org_edit_label(&org, id, opt).await?;
    ftl_println!(
        "msg-org-label-edit-success",
        old_label = crate::render_label(&old_label)?,
        label = crate::render_label(&label)?
    );
    Ok(())
}

async fn remove_org_label(api: &Forgejo, org: String, name: String) -> eyre::Result<()> {
    let label = find_label_by_name(api, &org, &name)
        .await?
        .ok_or_eyre("label not found")?;
    let id = label.id.ok_or_eyre("label does not have id")?;
    api.org_delete_label(&org, id).await?;
    ftl_println!(
        "msg-org-label-remove-success",
        label = crate::render_label(&label)?
    );
    Ok(())
}

#[derive(Subcommand, Clone, Debug)]
pub enum RepoSubcommand {
    #[clap(about = h!("cmd-org-repo-list"))]
    List {
        #[clap(help = h!("arg-org-repo-list-org"))]
        org: String,

        #[clap(help = h!("arg-org-repo-list-page"))]
        #[clap(long, short, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-org-repo-create"))]
    Create {
        #[clap(help = h!("arg-org-repo-create-org"))]
        org: String,

        #[clap(flatten)]
        args: crate::repo::RepoCreateArgs,
    },
}

impl RepoSubcommand {
    async fn run(
        self,
        keys: &crate::KeyInfo,
        repo_info: &RepoInfo,
        api: &Forgejo,
    ) -> eyre::Result<()> {
        match self {
            RepoSubcommand::List { org, page } => list_org_repos(api, org, page).await?,
            RepoSubcommand::Create {
                org,
                args:
                    crate::repo::RepoCreateArgs {
                        repo,
                        description,
                        private,
                        remote,
                        push,
                        ssh,
                        #[cfg(feature = "unstable-sha256")]
                        format,
                    },
            } => {
                let url_host = crate::host_name(repo_info.host_url());
                let ssh = ssh
                    .unwrap_or_else(|| Some(keys.default_ssh.contains(url_host)))
                    .unwrap_or(true);
                crate::repo::create_repo(
                    api,
                    Some(org),
                    repo,
                    description,
                    private,
                    remote,
                    push,
                    ssh,
                    #[cfg(feature = "unstable-sha256")]
                    format,
                )
                .await?
            }
        }
        Ok(())
    }
}

async fn list_org_repos(api: &Forgejo, org: String, page: u32) -> eyre::Result<()> {
    let (headers, repos) = api.org_list_repos(&org).page(page).await?;
    let SpecialRender { bullet, .. } = crate::special_render();
    if repos.is_empty() {
        ftl_println!("msg-org-repo-list-no_results");
    } else {
        for repo in repos {
            let full_name = repo
                .full_name
                .as_deref()
                .ok_or_eyre("repo does not have full name")?;
            println!("{bullet} {full_name}");
        }
        let count = headers.x_total_count.unwrap_or_default() as u64;
        ftl_println!(
            "msg-org-repo-list-page_number",
            page,
            total = count.div_ceil(20)
        );
    }
    Ok(())
}
