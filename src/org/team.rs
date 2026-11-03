use std::collections::BTreeMap;

use clap::{Args, Subcommand};
use eyre::OptionExt;
use forgejo_api::{
    Forgejo,
    structs::{CreateTeamOption, EditTeamOption},
};
use futures::{TryStreamExt, future};

use crate::{
    SpecialRender, ftl_eprintln, ftl_println, ftl_prompt_bool, h, lh, localization::AsFluent,
};

#[derive(Subcommand, Clone, Debug)]
pub enum TeamSubcommand {
    #[clap(about = h!("cmd-org-team-list"))]
    List {
        #[clap(help = h!("arg-org-team-list-org"))]
        org: String,
    },
    #[clap(about = h!("cmd-org-team-view"))]
    View {
        #[clap(help = h!("arg-org-team-view-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-view-name"))]
        name: String,

        #[clap(long, short = 'p')]
        list_permissions: bool,
    },
    #[clap(about = h!("cmd-org-team-create"))]
    Create {
        #[clap(help = h!("arg-org-team-create-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-create-name"), long_help = lh!("arg-org-team-create-name"))]
        name: String,

        #[clap(flatten)]
        flags: TeamCreateFlags,

        #[clap(flatten)]
        options: TeamOptions,
    },
    #[clap(about = h!("cmd-org-team-edit"))]
    Edit {
        #[clap(help = h!("arg-org-team-edit-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-edit-name"))]
        name: String,

        #[clap(long, short)]
        new_name: Option<String>,

        #[clap(flatten)]
        flags: TeamEditFlags,

        #[clap(flatten)]
        options: TeamOptions,
    },
    #[clap(about = h!("cmd-org-team-delete"), long_about = lh!("cmd-org-team-delete"))]
    Delete {
        #[clap(help = h!("arg-org-team-delete-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-delete-name"))]
        name: String,
    },
    #[clap(subcommand)]
    Repo(TeamRepoSubcommand),
    #[clap(subcommand)]
    Member(TeamMemberSubcommand),
}

#[derive(Args, Clone, Debug)]
pub struct TeamOptions {
    #[clap(help = h!("arg-org-team-options-description"))]
    #[clap(long, short)]
    description: Option<String>,

    #[clap(help = h!("arg-org-team-options-read_permissions"), long_help = lh!("arg-org-team-options-read_permissions"))]
    #[clap(long, short)]
    read_permissions: Option<String>,

    #[clap(help = h!("arg-org-team-options-write_permissions"), long_help = lh!("arg-org-team-options-write_permissions"))]
    #[clap(long, short)]
    write_permissions: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct TeamCreateFlags {
    #[clap(help = h!("arg-org-team-create-can_create_repos"))]
    #[clap(long, short)]
    can_create_repos: bool,

    #[clap(help = h!("arg-org-team-create-include_all_repos"))]
    #[clap(long, short)]
    include_all_repos: bool,

    #[clap(help = h!("arg-org-team-create-admin"))]
    #[clap(long, short = 'A')]
    admin: bool,
}

#[derive(Args, Clone, Debug)]
pub struct TeamEditFlags {
    #[clap(help = h!("arg-org-team-create-can_create_repos"))]
    #[clap(long, short)]
    can_create_repos: Option<bool>,

    #[clap(help = h!("arg-org-team-create-include_all_repos"))]
    #[clap(long, short)]
    include_all_repos: Option<bool>,

    #[clap(help = h!("arg-org-team-create-admin"))]
    #[clap(long, short = 'A')]
    admin: Option<bool>,
}

impl TeamSubcommand {
    pub async fn run(self, api: &forgejo_api::Forgejo) -> eyre::Result<()> {
        match self {
            TeamSubcommand::List { org } => list_teams(api, org).await?,
            TeamSubcommand::View {
                org,
                name,
                list_permissions,
            } => view_team(api, org, name, list_permissions).await?,
            TeamSubcommand::Create {
                org,
                name,
                flags,
                options,
            } => create_team(api, org, name, flags, options).await?,
            TeamSubcommand::Edit {
                org,
                name,
                new_name,
                flags,
                options,
            } => edit_team(api, org, name, new_name, flags, options).await?,
            TeamSubcommand::Delete { org, name } => delete_team(api, org, name).await?,
            TeamSubcommand::Repo(subcommand) => subcommand.run(api).await?,
            TeamSubcommand::Member(subcommand) => subcommand.run(api).await?,
        }
        Ok(())
    }
}

async fn find_team_by_name(
    api: &Forgejo,
    org: &str,
    name: &str,
) -> eyre::Result<forgejo_api::structs::Team> {
    api.org_list_teams(org)
        .stream()
        .try_filter(|team| {
            future::ready(
                team.name
                    .as_deref()
                    .is_some_and(|team_name| team_name == name),
            )
        })
        .try_next()
        .await?
        .ok_or_else(|| eyre::eyre!("Unknown team {name}"))
}

async fn list_teams(api: &Forgejo, org: String) -> eyre::Result<()> {
    let mut teams = api.org_list_teams(&org).all().await?;
    teams.sort_unstable_by_key(permission_sort_id);

    let SpecialRender {
        bright_blue,
        bold,
        reset,
        bullet,
        ..
    } = crate::special_render();
    for team in teams {
        let team_name = team.name.as_deref().ok_or_eyre("team does not have name")?;
        println!("{bullet} {bold}{bright_blue}{team_name}{reset}");
    }
    Ok(())
}

fn permission_sort_id(team: &forgejo_api::structs::Team) -> u32 {
    use forgejo_api::structs::TeamPermission as Perm;
    match &team.permission {
        Some(Perm::Owner) => 0,
        Some(Perm::Admin) => 1,
        Some(Perm::Write) => 2,
        Some(Perm::Read) => 3,
        Some(Perm::None) | None => 4,
    }
}

const ALL_UNITS: &[&str] = &[
    "repo.wiki",
    "repo.ext_wiki",
    "repo.issues",
    "repo.ext_issues",
    "repo.pulls",
    "repo.projects",
    "repo.actions",
    "repo.code",
    "repo.releases",
    "repo.packages",
];

async fn view_team(
    api: &Forgejo,
    org: String,
    name: String,
    list_permissions: bool,
) -> eyre::Result<()> {
    let team = find_team_by_name(api, &org, &name).await?;

    let SpecialRender { bullet, .. } = crate::special_render();

    let is_admin = team
        .permission
        .is_some_and(|p| p == forgejo_api::structs::TeamPermission::Admin);
    ftl_println!("msg-org-team-view", name, org, admin = is_admin.ftl());

    if let Some(description) = &team.description
        && !description.is_empty()
    {
        println!("\n{}", crate::markdown(description));
    }

    if list_permissions {
        println!();
        let units = team
            .units_map
            .as_ref()
            .ok_or_eyre("team does not have permission units")?;
        let mut ro_perms = Vec::new();
        let mut rw_perms = Vec::new();
        for (unit, permission) in units {
            match &**permission {
                "read" => ro_perms.push(unit),
                "write" | "admin" | "owner" => rw_perms.push(unit),
                _ => (),
            }
        }

        let get_unit_fluent_id = |unit| match unit {
            "repo.wiki" => "msg-org-team-view-perms-wiki",
            "repo.ext_wiki" => "msg-org-team-view-perms-ext_wiki",
            "repo.issues" => "msg-org-team-view-perms-issues",
            "repo.ext_issues" => "msg-org-team-view-perms-ext_issues",
            "repo.pulls" => "msg-org-team-view-perms-pulls",
            "repo.projects" => "msg-org-team-view-perms-projects",
            "repo.actions" => "msg-org-team-view-perms-actions",
            "repo.code" => "msg-org-team-view-perms-code",
            "repo.releases" => "msg-org-team-view-perms-releases",
            "repo.packages" => "msg-org-team-view-perms-packages",
            _ => "msg-org-team-view-perms-unknown",
        };
        if !ro_perms.is_empty() {
            ftl_println!("msg-org-team-view-read_write");
            for unit in ro_perms {
                let unit_fluent_id = get_unit_fluent_id(unit);
                print!("{bullet} ");
                ftl_println!(unit_fluent_id);
            }
            println!();
        }
        if !rw_perms.is_empty() {
            ftl_println!("msg-org-team-view-read_write");
            for unit in rw_perms {
                let unit_fluent_id = get_unit_fluent_id(unit);
                print!("{bullet} ");
                ftl_println!(unit_fluent_id);
            }
            println!();
        }
    }

    Ok(())
}

fn create_unit_map(ro_perms: Option<&str>, rw_perms: Option<&str>) -> BTreeMap<String, String> {
    let mut units = BTreeMap::new();
    if let Some(ro_perms) = ro_perms {
        if ro_perms == "all" {
            for ro in ALL_UNITS {
                units.insert(ro.to_string(), "read".to_owned());
            }
        } else {
            for ro in ro_perms.split(",") {
                units.insert(format!("repo.{ro}"), "read".to_owned());
            }
        }
    }
    if let Some(rw_perms) = rw_perms {
        if rw_perms.trim() == "all" {
            for rw in ALL_UNITS {
                units.insert(rw.to_string(), "write".to_owned());
            }
        } else {
            for rw in rw_perms.split(",") {
                units.insert(format!("repo.{rw}"), "write".to_owned());
            }
        }
    }
    units
}

async fn create_team(
    api: &Forgejo,
    org: String,
    name: String,
    flags: TeamCreateFlags,
    options: TeamOptions,
) -> eyre::Result<()> {
    let units = create_unit_map(
        options.read_permissions.as_deref(),
        options.write_permissions.as_deref(),
    );
    let opt = CreateTeamOption {
        can_create_org_repo: Some(flags.can_create_repos),
        description: options.description,
        includes_all_repositories: Some(flags.include_all_repos),
        name,
        permission: flags
            .admin
            .then_some(forgejo_api::structs::CreateTeamOptionPermission::Admin),
        #[allow(deprecated)]
        units: None,
        units_map: Some(units),
    };
    let new_team = api.org_create_team(&org, opt).await?;
    let org = new_team.organization.ok_or_eyre("team doesn't have org")?;
    let org_name = org
        .name
        .or(org.full_name)
        .ok_or_eyre("org doesn't have name")?;
    let name = new_team.name.ok_or_eyre("team doesn't have name")?;

    ftl_println!(
        "msg-org-team-create-success",
        name,
        org = org_name,
        admin = flags.admin.ftl(),
    );
    Ok(())
}

async fn edit_team(
    api: &Forgejo,
    org: String,
    name: String,
    new_name: Option<String>,
    flags: TeamEditFlags,
    options: TeamOptions,
) -> eyre::Result<()> {
    let team = find_team_by_name(api, &org, &name).await?;
    let id = team.id.ok_or_eyre("team does not have id")?;

    // EditTeamOption's team field is a String rather than Option<String>
    // That should be fixed, but this gets around it for now.
    let new_name = new_name.unwrap_or(name);
    let units = create_unit_map(
        options.read_permissions.as_deref(),
        options.write_permissions.as_deref(),
    );

    let options = EditTeamOption {
        can_create_org_repo: flags.can_create_repos,
        description: options.description,
        includes_all_repositories: flags.include_all_repos,
        name: new_name,
        permission: flags
            .admin
            .and_then(|b| b.then_some(forgejo_api::structs::EditTeamOptionPermission::Admin)),
        #[allow(deprecated)]
        units: None,
        units_map: Some(units),
    };
    api.org_edit_team(id, options).await?;

    Ok(())
}

async fn delete_team(api: &Forgejo, org: String, name: String) -> eyre::Result<()> {
    let confirmation = ftl_prompt_bool!(
        default false; "msg-org-team-delete-confirmation", org = &org, name = &name,
    )?;
    if confirmation {
        let id = find_team_by_name(api, &org, &name)
            .await?
            .id
            .ok_or_eyre("team does not have id")?;
        api.org_delete_team(id).await?;
        println!("Team deleted.");
    } else {
        println!("Team not deleted.");
    }
    Ok(())
}

#[derive(Subcommand, Clone, Debug)]
pub enum TeamRepoSubcommand {
    #[clap(about = h!("cmd-org-team-repo-list"))]
    List {
        #[clap(help = h!("arg-org-team-repo-list-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-repo-list-team"))]
        team: String,

        #[clap(help = h!("arg-org-team-repo-list-page"))]
        #[clap(long, short, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-org-team-repo-add"))]
    Add {
        #[clap(help = h!("arg-org-team-repo-add-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-repo-add-team"))]
        team: String,

        #[clap(help = h!("arg-org-team-repo-add-repo"))]
        repo: String,
    },
    #[clap(about = h!("cmd-org-team-repo-rm"), long_about = lh!("cmd-org-team-repo-rm"))]
    Rm {
        #[clap(help = h!("arg-org-team-repo-rm-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-repo-rm-team"))]
        team: String,

        #[clap(help = h!("arg-org-team-repo-rm-repo"))]
        repo: String,
    },
}

impl TeamRepoSubcommand {
    async fn run(self, api: &Forgejo) -> eyre::Result<()> {
        match self {
            TeamRepoSubcommand::List { org, team, page } => {
                list_team_repos(api, org, team, page).await?
            }
            TeamRepoSubcommand::Add { org, team, repo } => {
                add_repo_to_team(api, org, team, repo).await?
            }
            TeamRepoSubcommand::Rm { org, team, repo } => {
                remove_repo_from_team(api, org, team, repo).await?
            }
        }
        Ok(())
    }
}

async fn list_team_repos(api: &Forgejo, org: String, team: String, page: u32) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    let (headers, repos) = api.org_list_team_repos(id).page(page).page_size(20).await?;

    let SpecialRender { bullet, .. } = crate::special_render();
    if repos.is_empty() {
        ftl_eprintln!("msg-org-team-repo-list-no_results");
    } else {
        for repo in repos {
            let full_name = repo
                .full_name
                .as_deref()
                .ok_or_eyre("repo does not have full name")?;
            println!("{bullet} {full_name}");
        }
        let count = headers.x_total_count.unwrap_or_default();
        ftl_eprintln!(
            "msg-org-team-repo-list-page_number",
            page,
            total = (count as u64).div_ceil(20),
        );
    }
    Ok(())
}

async fn add_repo_to_team(
    api: &Forgejo,
    org: String,
    team: String,
    repo: String,
) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    api.org_add_team_repository(id, &org, &repo).await?;
    ftl_println!("msg-org-team-repo-add-success", org, repo, team);
    Ok(())
}

async fn remove_repo_from_team(
    api: &Forgejo,
    org: String,
    team: String,
    repo: String,
) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    api.org_remove_team_repository(id, &org, &repo).await?;
    ftl_println!("msg-org-team-repo-rm-success", org, repo, team);
    Ok(())
}

#[derive(Subcommand, Clone, Debug)]
pub enum TeamMemberSubcommand {
    #[clap(about = h!("cmd-org-team-member-list"))]
    List {
        #[clap(help = h!("arg-org-team-member-list-org"))]
        org: String,

        #[clap(help = h!("arg-org-team-member-list-team"))]
        team: String,

        #[clap(help = h!("arg-org-team-member-list-page"))]
        #[clap(long, short, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-org-team-member-add"))]
    Add {
        #[clap(help = h!("arg-org-team-member-add-org"))]
        org: String,
        #[clap(help = h!("arg-org-team-member-add-team"))]
        team: String,
        #[clap(help = h!("arg-org-team-member-add-user"))]
        user: String,
    },
    #[clap(about = h!("cmd-org-team-member-rm"))]
    Rm {
        #[clap(help = h!("arg-org-team-member-rm-org"))]
        org: String,
        #[clap(help = h!("arg-org-team-member-rm-team"))]
        team: String,
        #[clap(help = h!("arg-org-team-member-rm-user"))]
        user: String,
    },
}

impl TeamMemberSubcommand {
    async fn run(self, api: &Forgejo) -> eyre::Result<()> {
        match self {
            TeamMemberSubcommand::List { org, team, page } => {
                list_team_members(api, org, team, page).await?
            }
            TeamMemberSubcommand::Add { org, team, user } => {
                add_user_to_team(api, org, team, user).await?
            }
            TeamMemberSubcommand::Rm { org, team, user } => {
                remove_user_from_team(api, org, team, user).await?
            }
        }
        Ok(())
    }
}

async fn list_team_members(
    api: &Forgejo,
    org: String,
    team: String,
    page: u32,
) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    let (headers, users) = api
        .org_list_team_members(id)
        .page(page)
        .page_size(20)
        .await?;

    let SpecialRender {
        bullet,
        light_grey,
        bright_cyan,
        reset,
        ..
    } = crate::special_render();
    if users.is_empty() {
        ftl_eprintln!("msg-org-team-member-list-no_results");
    } else {
        for user in users {
            let username = user
                .login
                .as_deref()
                .ok_or_eyre("repo does not have full name")?;
            match user.full_name.as_deref().filter(|s| !s.is_empty()) {
                Some(full_name) => println!(
                    "{bullet} {bright_cyan}{full_name}{reset} {light_grey}({username}){reset}"
                ),
                None => println!("{bullet} {bright_cyan}{username}{reset}"),
            }
        }
        let count = headers.x_total_count.unwrap_or_default();
        ftl_eprintln!(
            "msg-org-team-member-list-page_number",
            page,
            total = (count as u64).div_ceil(20),
        );
    }
    Ok(())
}

async fn add_user_to_team(
    api: &Forgejo,
    org: String,
    team: String,
    user: String,
) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    api.org_add_team_member(id, &user).await?;
    ftl_println!("msg-org-team-member-add-success", user, team);
    Ok(())
}

async fn remove_user_from_team(
    api: &Forgejo,
    org: String,
    team: String,
    user: String,
) -> eyre::Result<()> {
    let id = find_team_by_name(api, &org, &team)
        .await?
        .id
        .ok_or_eyre("team does not have id")?;
    api.org_remove_team_member(id, &user).await?;
    ftl_println!("msg-org-team-member-rm-success", user, team);
    Ok(())
}
