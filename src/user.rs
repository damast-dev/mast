use clap::{Args, Subcommand};
use eyre::{Context, ContextCompat, OptionExt};
use forgejo_api::Forgejo;

use crate::{
    SpecialRender, ftl_bail, ftl_ensure, ftl_eyre, ftl_println, h, lh, localization::AsFluent,
    repo::RepoInfo,
};

use std::borrow::Cow;

#[derive(Args, Clone, Debug)]
pub struct UserCommand {
    #[clap(help = h!("arg-remote"))]
    #[clap(long, short = 'R')]
    remote: Option<String>,
    #[clap(subcommand)]
    command: UserSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum UserSubcommand {
    #[clap(about = h!("cmd-user-search"))]
    Search {
        #[clap(help = h!("arg-user-search-query"))]
        query: String,
        #[clap(long, short)]
        page: Option<usize>,
    },
    #[clap(about = h!("cmd-user-view"))]
    View {
        #[clap(help = h!("arg-user-view-user"), long_help = lh!("arg-user-view-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-browse"))]
    Browse {
        #[clap(help = h!("arg-user-browse-user"), long_help = lh!("arg-user-browse-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-follow"))]
    Follow {
        #[clap(help = h!("arg-user-follow-user"))]
        user: String,
    },
    #[clap(about = h!("cmd-user-unfollow"))]
    Unfollow {
        #[clap(help = h!("arg-user-unfollow-user"))]
        user: String,
    },
    #[clap(about = h!("cmd-user-following"))]
    Following {
        #[clap(help = h!("arg-user-following-user"), long_help = lh!("arg-user-following-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-followers"))]
    Followers {
        #[clap(help = h!("arg-user-followers-user"), long_help = lh!("arg-user-followers-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-block"))]
    Block {
        #[clap(help = h!("arg-user-block-user"))]
        user: String,
    },
    #[clap(about = h!("cmd-user-unblock"))]
    Unblock {
        #[clap(help = h!("arg-user-unblock-user"))]
        user: String,
    },
    #[clap(about = h!("cmd-user-repos"))]
    Repos {
        #[clap(help = h!("arg-user-repos-user"), long_help = lh!("arg-user-repos-user"))]
        user: Option<String>,

        #[clap(help = h!("arg-user-repos-starred"))]
        #[clap(long)]
        starred: bool,

        #[clap(help = h!("arg-user-repos-sort"))]
        #[clap(long)]
        sort: Option<RepoSortOrder>,

        #[clap(help = h!("arg-user-repos-page"))]
        #[clap(long, default_value_t = 1)]
        page: u32,
    },
    #[clap(about = h!("cmd-user-orgs"))]
    Orgs {
        #[clap(help = h!("arg-user-orgs-user"), long_help = lh!("arg-user-orgs-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-activity"))]
    Activity {
        #[clap(help = h!("arg-user-orgs-user"), long_help = lh!("arg-user-orgs-user"))]
        user: Option<String>,
    },
    #[clap(about = h!("cmd-user-edit"))]
    #[clap(subcommand)]
    Edit(EditCommand),

    #[clap(about = h!("cmd-user-key"))]
    #[clap(subcommand)]
    Key(KeyCommand),

    #[clap(about = h!("cmd-user-gpg"))]
    #[clap(subcommand)]
    Gpg(GpgCommand),
}

#[derive(Subcommand, Clone, Debug)]
pub enum EditCommand {
    #[clap(about = h!("cmd-user-edit-bio"))]
    Bio {
        #[clap(help = h!("arg-user-edit-bio-content"))]
        content: Option<String>,
    },
    #[clap(about = h!("cmd-user-edit-name"))]
    Name {
        #[clap(help = h!("arg-user-edit-name-name"))]
        #[clap(group = "arg")]
        name: Option<String>,

        #[clap(help = h!("arg-user-edit-name-unset"))]
        #[clap(long, short, group = "arg")]
        unset: bool,
    },
    #[clap(about = h!("cmd-user-edit-pronouns"))]
    Pronouns {
        #[clap(help = h!("arg-user-edit-pronouns-pronouns"))]
        #[clap(group = "arg")]
        pronouns: Option<String>,

        #[clap(help = h!("arg-user-edit-pronouns-unset"))]
        #[clap(long, short, group = "arg")]
        unset: bool,
    },
    #[clap(about = h!("cmd-user-edit-location"))]
    Location {
        #[clap(help = h!("arg-user-edit-location-location"))]
        #[clap(group = "arg")]
        location: Option<String>,

        #[clap(help = h!("arg-user-edit-location-unset"))]
        #[clap(long, short, group = "arg")]
        unset: bool,
    },
    #[clap(about = h!("cmd-user-edit-activity"))]
    Activity {
        #[clap(help = h!("arg-user-edit-activity-visibility"))]
        #[clap(long, short)]
        visibility: VisibilitySetting,
    },
    #[clap(about = h!("cmd-user-edit-email"))]
    Email {
        #[clap(help = h!("arg-user-edit-email-visibility"))]
        #[clap(long, short)]
        visibility: Option<VisibilitySetting>,

        #[clap(help = h!("arg-user-edit-email-add"))]
        #[clap(long, short)]
        add: Vec<String>,

        #[clap(help = h!("arg-user-edit-email-rm"))]
        #[clap(long, short)]
        rm: Vec<String>,
    },
    #[clap(about = h!("cmd-user-edit-website"))]
    Website {
        #[clap(help = h!("arg-user-edit-website-url"))]
        #[clap(group = "arg")]
        url: Option<String>,

        #[clap(help = h!("arg-user-edit-website-unset"))]
        #[clap(long, short, group = "arg")]
        unset: bool,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum KeyCommand {
    #[clap(about = h!("cmd-user-key-list"))]
    List {
        #[clap(help = h!("arg-user-key-list-verbose"))]
        #[clap(short, long)]
        verbose: bool,
    },
    #[clap(about = h!("cmd-user-key-view"))]
    View {
        #[clap(help = h!("arg-user-key-view-id"))]
        id: i64,
    },
    #[clap(about = h!("cmd-user-key-delete"))]
    Delete {
        #[clap(help = h!("arg-user-key-delete-id"))]
        id: i64,
    },
    #[clap(about = h!("cmd-user-key-upload"))]
    Upload {
        #[clap(help = h!("arg-user-key-upload-keyfile"))]
        keyfile: Option<String>,

        #[clap(help = h!("arg-user-key-upload-title"))]
        #[clap(short, long)]
        title: Option<String>,

        #[clap(help = h!("arg-user-key-upload-force"))]
        #[clap(short, long)]
        force: bool,

        #[clap(help = h!("arg-user-key-upload-read_only"))]
        #[clap(short, long)]
        read_only: bool,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum GpgCommand {
    #[clap(about = h!("cmd-user-gpg-list"))]
    List {
        #[clap(help = h!("arg-user-gpg-list-verbose"))]
        #[clap(short, long)]
        verbose: bool,
    },

    #[clap(about = h!("cmd-user-gpg-view"))]
    View {
        #[clap(help = h!("arg-user-gpg-view-id"))]
        id: i64,
    },
    #[clap(about = h!("cmd-user-gpg-delete"))]
    Delete {
        #[clap(help = h!("arg-user-gpg-delete-id"))]
        id: i64,

        #[clap(help = h!("arg-user-gpg-delete-force"))]
        #[clap(short, long)]
        force: bool,
    },
    #[clap(about = h!("cmd-user-gpg-upload"))]
    Upload {
        #[clap(help = h!("arg-user-gpg-upload-key"))]
        key: String,

        #[clap(help = h!("arg-user-gpg-upload-no_verify"))]
        #[clap(short, long)]
        no_verify: bool,
    },
    #[clap(about = h!("cmd-user-gpg-verify"), long_about = lh!("cmd-user-gpg-verify"))]
    Verify {
        #[clap(help = h!("arg-user-gpg-verify-id"))]
        id: i64,
    },
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum VisibilitySetting {
    Hidden,
    Public,
}

impl UserCommand {
    pub async fn run(self, keys: &mut crate::KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        let repo = RepoInfo::get_current(host_name, None, self.remote.as_deref(), keys)?;
        let api = keys.get_api(repo.host_url()).await?;
        match self.command {
            UserSubcommand::Search { query, page } => user_search(&api, &query, page).await?,
            UserSubcommand::View { user } => view_user(&api, user.as_deref()).await?,
            UserSubcommand::Browse { user } => {
                browse_user(&api, repo.host_url(), user.as_deref()).await?
            }
            UserSubcommand::Follow { user } => follow_user(&api, &user).await?,
            UserSubcommand::Unfollow { user } => unfollow_user(&api, &user).await?,
            UserSubcommand::Following { user } => list_following(&api, user.as_deref()).await?,
            UserSubcommand::Followers { user } => list_followers(&api, user.as_deref()).await?,
            UserSubcommand::Block { user } => block_user(&api, &user).await?,
            UserSubcommand::Unblock { user } => unblock_user(&api, &user).await?,
            UserSubcommand::Repos {
                user,
                starred,
                sort,
                page,
            } => list_repos(&api, user.as_deref(), starred, sort, page).await?,
            UserSubcommand::Orgs { user } => list_orgs(&api, user.as_deref()).await?,
            UserSubcommand::Activity { user } => list_activity(&api, user.as_deref()).await?,
            UserSubcommand::Edit(cmd) => match cmd {
                EditCommand::Bio { content } => edit_bio(&api, content).await?,
                EditCommand::Name { name, unset } => edit_name(&api, name, unset).await?,
                EditCommand::Pronouns { pronouns, unset } => {
                    edit_pronouns(&api, pronouns, unset).await?
                }
                EditCommand::Location { location, unset } => {
                    edit_location(&api, location, unset).await?
                }
                EditCommand::Activity { visibility } => edit_activity(&api, visibility).await?,
                EditCommand::Email {
                    visibility,
                    add,
                    rm,
                } => edit_email(&api, visibility, add, rm).await?,
                EditCommand::Website { url, unset } => edit_website(&api, url, unset).await?,
            },
            UserSubcommand::Key(cmd) => match cmd {
                KeyCommand::List { verbose } => list_keys(&api, verbose).await?,
                KeyCommand::View { id } => view_key(&api, id).await?,
                KeyCommand::Delete { id } => delete_key(&api, id).await?,
                KeyCommand::Upload {
                    keyfile,
                    title,
                    force,
                    read_only,
                } => upload_key(&api, keyfile, title, force, read_only).await?,
            },
            UserSubcommand::Gpg(cmd) => match cmd {
                GpgCommand::List { verbose } => list_gpg(&api, verbose).await?,
                GpgCommand::View { id } => view_gpg(&api, id).await?,
                GpgCommand::Delete { id, force } => delete_gpg(&api, id, force).await?,
                GpgCommand::Upload { key, no_verify } => upload_gpg(&api, key, no_verify).await?,
                GpgCommand::Verify { id } => verify_gpg(&api, id).await?,
            },
        }
        Ok(())
    }
}

async fn user_search(api: &Forgejo, query: &str, page: Option<usize>) -> eyre::Result<()> {
    let page = page.unwrap_or(1);
    if page == 0 {
        ftl_println!("msg-user-search-page_zero");
    }
    let query = forgejo_api::structs::UserSearchQuery {
        q: Some(query.to_owned()),
        ..Default::default()
    };
    let result = api.user_search(query).await?;
    let users = result.data.ok_or_eyre("search did not return data")?;
    let ok = result.ok.ok_or_eyre("search did not return ok")?;
    if !ok {
        ftl_println!("msg-user-search-fail");
        return Ok(());
    }
    if users.is_empty() {
        ftl_println!("msg-user-search-none");
        println!();
    } else {
        let SpecialRender {
            bullet,
            bold,
            reset,
            ..
        } = *crate::special_render();
        let page_start = (page - 1) * 20;
        let total_pages = users.len().div_ceil(20);
        if page_start >= users.len() {
            ftl_println!("msg-user-search-page_too_high", total_pages);
        } else {
            for user in users.iter().skip(page_start).take(20) {
                let username = user
                    .login
                    .as_deref()
                    .ok_or_eyre("user does not have name")?;
                println!("{bullet} {bold}{username}{reset}");
            }
            ftl_println!(
                "msg-user-search-footer",
                first_index = page_start + 1,
                last_index = (page_start + 20).min(users.len()),
                total_results = users.len(),
                page,
                total_pages,
                more = (users.len() > 20).ftl(),
            );
        }
    }
    Ok(())
}

async fn view_user(api: &Forgejo, user: Option<&str>) -> eyre::Result<()> {
    let user_data = match user {
        Some(user) => api.user_get(user).await?,
        None => api.user_get_current().await?,
    };
    let username = user_data
        .login
        .as_deref()
        .ok_or_eyre("user has no username")?;
    let followers = user_data.followers_count.unwrap_or_default();
    let following = user_data.following_count.unwrap_or_default();

    ftl_println!(
        "msg-user-view-header",
        username,
        pronouns = user_data.pronouns.as_deref().filter(|s| !s.is_empty()),
        followers,
        following,
        website = user_data.website.as_deref().filter(|s| !s.is_empty()),
        email = user_data.email.as_deref().filter(|s| !s.is_empty()),
    );

    if let Some(desc) = user_data.description.as_deref()
        && !desc.is_empty()
    {
        println!("\n{}\n", crate::markdown(desc));
    }

    let joined = user_data
        .created
        .ok_or_eyre("user does not have join date")?;
    ftl_println!("msg-user-view-joined_on", joined = joined.ftl());

    Ok(())
}

async fn browse_user(api: &Forgejo, host_url: &url::Url, user: Option<&str>) -> eyre::Result<()> {
    let username = match user {
        Some(user) => user.to_owned(),
        None => {
            let myself = api.user_get_current().await?;
            myself
                .login
                .ok_or_eyre("authenticated user does not have login")?
        }
    };
    // `User` doesn't have an `html_url` field, so we gotta construct the user
    // page url ourselves
    let mut url = host_url.clone();
    url.path_segments_mut()
        .map_err(|_| eyre::eyre!("invalid host url"))?
        .push(&username);
    open::that_detached(url.as_str()).wrap_err("Failed to open URL")?;

    Ok(())
}

async fn follow_user(api: &Forgejo, username: &str) -> eyre::Result<()> {
    api.user_current_put_follow(username).await?;
    ftl_println!("msg-user-follow-success", username);
    Ok(())
}

async fn unfollow_user(api: &Forgejo, username: &str) -> eyre::Result<()> {
    api.user_current_delete_follow(username).await?;
    ftl_println!("msg-user-unfollow-success", username);
    Ok(())
}

async fn list_following(api: &Forgejo, user: Option<&str>) -> eyre::Result<()> {
    let following = match user {
        Some(user) => api.user_list_following(user).all().await?,
        None => api.user_current_list_following().all().await?,
    };

    if following.is_empty() {
        match user {
            Some(name) => ftl_println!("msg-user-following-none-other", name),
            None => ftl_println!("msg-user-following-none-self"),
        }
    } else {
        match user {
            Some(name) => ftl_println!("msg-user-following-other", name),
            None => ftl_println!("msg-user-following-self"),
        }
        let SpecialRender { bullet, .. } = *crate::special_render();

        for followed in following {
            let username = followed
                .login
                .as_deref()
                .ok_or_eyre("user does not have username")?;
            println!("{bullet} {username}");
        }
    }

    Ok(())
}

async fn list_followers(api: &Forgejo, user: Option<&str>) -> eyre::Result<()> {
    let (_, followers) = match user {
        Some(user) => api.user_list_followers(user).await?,
        None => api.user_current_list_followers().await?,
    };

    if followers.is_empty() {
        match user {
            Some(user) => ftl_println!("msg-user-followers-none-other", user),
            None => ftl_println!("msg-user-followers-none-self"),
        }
    } else {
        match user {
            Some(user) => ftl_println!("msg-user-followers-other", user),
            None => ftl_println!("msg-user-followers-self"),
        }
        let SpecialRender { bullet, .. } = *crate::special_render();

        for follower in followers {
            let username = follower
                .login
                .as_deref()
                .ok_or_eyre("user does not have username")?;
            println!("{bullet} {username}");
        }
    }

    Ok(())
}

async fn block_user(api: &Forgejo, user: &str) -> eyre::Result<()> {
    api.user_block_user(user).await?;
    ftl_println!("msg-user-block-success", user);
    Ok(())
}

async fn unblock_user(api: &Forgejo, user: &str) -> eyre::Result<()> {
    api.user_unblock_user(user).await?;
    ftl_println!("msg-user-unblock-success", user);
    Ok(())
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum RepoSortOrder {
    #[default]
    Name,
    Modified,
    Created,
    Stars,
    Forks,
}

async fn list_repos(
    api: &Forgejo,
    user: Option<&str>,
    starred: bool,
    sort: Option<RepoSortOrder>,
    page: u32,
) -> eyre::Result<()> {
    let (headers, mut repos) = if starred {
        match user {
            Some(user) => api.user_list_starred(user).page(page).page_size(50).await?,
            None => {
                api.user_current_list_starred()
                    .page(page)
                    .page_size(50)
                    .await?
            }
        }
    } else {
        match user {
            Some(user) => api.user_list_repos(user).page(page).page_size(50).await?,
            None => {
                let query = forgejo_api::structs::UserCurrentListReposQuery {
                    ..Default::default()
                };
                api.user_current_list_repos(query)
                    .page(page)
                    .page_size(50)
                    .await?
            }
        }
    };

    if repos.is_empty() {
        if starred {
            match user {
                Some(user) => ftl_println!("msg-user-repos-none-starred-other", user),
                None => ftl_println!("msg-user-repos-none-starred-self"),
            }
        } else {
            match user {
                Some(user) => ftl_println!("msg-user-repos-none-other", user),
                None => ftl_println!("msg-user-repos-none-self"),
            }
        };
    } else {
        let sort_fn: fn(
            &forgejo_api::structs::Repository,
            &forgejo_api::structs::Repository,
        ) -> std::cmp::Ordering = match sort.unwrap_or_default() {
            RepoSortOrder::Name => |a, b| a.full_name.cmp(&b.full_name),
            RepoSortOrder::Modified => |a, b| b.updated_at.cmp(&a.updated_at),
            RepoSortOrder::Created => |a, b| b.created_at.cmp(&a.created_at),
            RepoSortOrder::Stars => |a, b| b.stars_count.cmp(&a.stars_count),
            RepoSortOrder::Forks => |a, b| b.forks_count.cmp(&a.forks_count),
        };
        repos.sort_unstable_by(sort_fn);

        let SpecialRender { bullet, .. } = *crate::special_render();
        for repo in &repos {
            let name = repo
                .full_name
                .as_deref()
                .ok_or_eyre("repo does not have name")?;
            println!("{bullet} {name}");
        }

        let page_start = (page - 1) * 50;
        let total_items = match headers.x_total_count {
            Some(t) => t as usize,
            None => repos.len(),
        };
        let total_pages = total_items.div_ceil(50);

        ftl_println!(
            "msg-user-search-footer",
            first_index = page_start + 1,
            last_index = (page_start + 20).min(repos.len() as u32),
            total_results = repos.len(),
            page,
            total_pages,
            more = (repos.len() > 20).ftl(),
        );
    }

    Ok(())
}

async fn list_orgs(api: &Forgejo, user: Option<&str>) -> eyre::Result<()> {
    let mut orgs = match user {
        Some(user) => api.org_list_user_orgs(user).await?,
        None => api.org_list_current_user_orgs().await?,
    };

    if orgs.is_empty() {
        match user {
            Some(user) => ftl_println!("msg-user-orgs-none-other", user),
            None => println!("msg-user-orgs-none-self"),
        }
    } else {
        orgs.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        let SpecialRender { bullet, dash, .. } = *crate::special_render();
        for org in &orgs {
            let name = org.name.as_deref().ok_or_eyre("org does not have name")?;
            let full_name = org
                .full_name
                .as_deref()
                .ok_or_eyre("org does not have name")?;
            if !full_name.is_empty() {
                println!("{bullet} {name} {dash} \"{full_name}\"");
            } else {
                println!("{bullet} {name}");
            }
        }
        ftl_println!("msg-user-orgs-count", organizations = orgs.len());
    }
    Ok(())
}

async fn list_activity(api: &Forgejo, user: Option<&str>) -> eyre::Result<()> {
    let user = match user {
        Some(s) => s.to_owned(),
        None => {
            let myself = api.user_get_current().await?;
            myself.login.ok_or_eyre("current user does not have name")?
        }
    };
    let query = forgejo_api::structs::UserListActivityFeedsQuery {
        only_performed_by: Some(true),
        ..Default::default()
    };
    let (_, feed) = api.user_list_activity_feeds(&user, query).await?;

    for activity in feed {
        print_activity(&activity)?;
    }
    Ok(())
}

pub fn print_activity(activity: &forgejo_api::structs::Activity) -> eyre::Result<()> {
    let actor = activity
        .act_user
        .as_ref()
        .ok_or_eyre("activity does not have actor")?;
    let actor = actor
        .login
        .as_deref()
        .ok_or_eyre("actor does not have name")?;
    let op_type = activity
        .op_type
        .as_ref()
        .ok_or_eyre("activity does not have op type")?;

    // do not add ? to these. they are here to make each branch smaller
    let repo = activity
        .repo
        .as_ref()
        .ok_or_eyre("activity does not have repo");
    let content = activity
        .content
        .as_deref()
        .ok_or_eyre("activity does not have content");
    let ref_name = activity
        .ref_name
        .as_deref()
        .ok_or_eyre("repo does not have full name");

    fn get_repo_name(repo: &forgejo_api::structs::Repository) -> eyre::Result<&str> {
        repo.full_name
            .as_deref()
            .ok_or_eyre("repo does not have full name")
    }
    // The first item of the returned tuple is the ID of the issue. It isn't
    // parsed to a number, since it only ever gets printed out.
    // string -> number -> string seems like a waste
    //
    // The second item is the "associated content" with the issue-related
    // activity. When opening or closing an issue, it's the name of the issue.
    // When commenting, it's a snippet of the comment's content.
    fn issue_content(content: &str) -> eyre::Result<(Cow<'_, str>, Cow<'_, str>)> {
        Ok(serde_json::from_str::<(Cow<'_, str>, Cow<'_, str>)>(
            content,
        )?)
    }

    use forgejo_api::structs::ActivityOpType;
    match op_type {
        ActivityOpType::CreateRepo => {
            let repo = repo?;
            let repo_name = get_repo_name(repo)?;
            if let Some(parent) = &repo.parent {
                let parent_repo_name = get_repo_name(parent)?;
                ftl_println!(
                    "msg-activity-created_fork",
                    actor,
                    parent_repo_name,
                    repo_name,
                );
            } else if repo.mirror.is_some_and(|b| b) {
                ftl_println!("msg-activity-created_mirror", actor, repo_name);
            } else {
                ftl_println!("msg-activity-created_repo", actor, repo_name);
            }
        }
        ActivityOpType::RenameRepo => {
            let old_name = content?;
            let new_name = get_repo_name(repo?)?;
            ftl_println!("msg-activity-renamed_repo", actor, old_name, new_name);
        }
        ActivityOpType::StarRepo => {
            let repo_name = get_repo_name(repo?)?;
            ftl_println!("msg-activity-starred_repo", actor, repo_name);
        }
        ActivityOpType::WatchRepo => {
            let repo_name = get_repo_name(repo?)?;
            ftl_println!("msg-activity-watched_repo", actor, repo_name);
        }
        ActivityOpType::CommitRepo => {
            let repo_name = get_repo_name(repo?)?;
            let ref_name = ref_name?;
            let branch = ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name);
            if !content?.is_empty() {
                ftl_println!("msg-activity-pushed_commit", actor, branch, repo_name);
            }
        }
        ActivityOpType::CreateIssue => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-created_issue", actor, repo_name, number);
        }
        ActivityOpType::CreatePullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-created_pr", actor, repo_name, number);
        }
        ActivityOpType::TransferRepo => {
            let old_name = content?;
            let new_name = get_repo_name(repo?)?;
            ftl_println!("msg-activity-transferred_repo", actor, old_name, new_name);
        }
        ActivityOpType::PushTag => {
            let repo_name = get_repo_name(repo?)?;
            let ref_name = ref_name?;
            let tag_name = ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name);
            ftl_println!("msg-activity-pushed_tag", actor, tag_name, repo_name);
        }
        ActivityOpType::CommentIssue => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-commented_issue", actor, repo_name, number);
        }
        ActivityOpType::MergePullRequest | ActivityOpType::AutoMergePullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-merged_pr", actor, repo_name, number);
        }
        ActivityOpType::CloseIssue => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-closed_issue", actor, repo_name, number);
        }
        ActivityOpType::ReopenIssue => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-reopened_issue", actor, repo_name, number);
        }
        ActivityOpType::ClosePullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-closed_pr", actor, repo_name, number);
        }
        ActivityOpType::ReopenPullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-reopened_pr", actor, repo_name, number);
        }
        ActivityOpType::DeleteTag => {
            let repo_name = get_repo_name(repo?)?;
            let ref_name = ref_name?;
            let tag_name = ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name);
            ftl_println!("msg-activity-deleted_tag", actor, tag_name, repo_name);
        }
        ActivityOpType::DeleteBranch => {
            let repo_name = get_repo_name(repo?)?;
            let ref_name = ref_name?;
            let branch = ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name);
            ftl_println!("msg-activity-deleted_branch", actor, branch, repo_name);
        }
        ActivityOpType::MirrorSyncPush => {}
        ActivityOpType::MirrorSyncCreate => {}
        ActivityOpType::MirrorSyncDelete => {}
        ActivityOpType::ApprovePullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-approved_pr", actor, repo_name, number);
        }
        ActivityOpType::RejectPullRequest => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-rejected_pr", actor, repo_name, number);
        }
        ActivityOpType::CommentPull => {
            let repo_name = get_repo_name(repo?)?;
            let (number, _) = issue_content(content?)?;
            ftl_println!("msg-activity-commented_pr", actor, repo_name, number);
        }
        ActivityOpType::PublishRelease => {
            let repo_name = get_repo_name(repo?)?;
            let release_name = content?;
            ftl_println!("msg-activity-deleted_tag", actor, release_name, repo_name);
        }
        ActivityOpType::PullReviewDismissed => {}
        ActivityOpType::PullRequestReadyForReview => {}
    }

    Ok(())
}

fn default_settings_opt() -> forgejo_api::structs::UserSettingsOptions {
    forgejo_api::structs::UserSettingsOptions {
        description: None,
        diff_view_style: None,
        enable_repo_unit_hints: None,
        full_name: None,
        hide_activity: None,
        hide_email: None,
        language: None,
        location: None,
        pronouns: None,
        hide_pronouns: None,
        theme: None,
        website: None,
    }
}

async fn edit_bio(api: &Forgejo, new_bio: Option<String>) -> eyre::Result<()> {
    let new_bio = match new_bio {
        Some(s) => s,
        None => {
            let mut bio = api
                .user_get_current()
                .await?
                .description
                .unwrap_or_default();
            crate::editor(&mut bio, Some("md")).await?;
            bio
        }
    };
    let opt = forgejo_api::structs::UserSettingsOptions {
        description: Some(new_bio),
        ..default_settings_opt()
    };
    api.update_user_settings(opt).await?;
    Ok(())
}

async fn edit_name(api: &Forgejo, new_name: Option<String>, unset: bool) -> eyre::Result<()> {
    match (new_name, unset) {
        (Some(_), true) => unreachable!(),
        (Some(name), false) if !name.is_empty() => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                full_name: Some(name),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        (None, true) => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                full_name: Some(String::new()),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        _ => ftl_println!("msg-user-edit-name-removal_hint"),
    }
    Ok(())
}

async fn edit_pronouns(
    api: &Forgejo,
    new_pronouns: Option<String>,
    unset: bool,
) -> eyre::Result<()> {
    match (new_pronouns, unset) {
        (Some(_), true) => unreachable!(),
        (Some(pronouns), false) if !pronouns.is_empty() => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                pronouns: Some(pronouns),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        (None, true) => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                pronouns: Some(String::new()),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        _ => ftl_println!("msg-user-edit-pronouns-removal_hint"),
    }
    Ok(())
}

async fn edit_location(
    api: &Forgejo,
    new_location: Option<String>,
    unset: bool,
) -> eyre::Result<()> {
    match (new_location, unset) {
        (Some(_), true) => unreachable!(),
        (Some(location), false) if !location.is_empty() => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                location: Some(location),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        (None, true) => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                location: Some(String::new()),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        _ => ftl_println!("msg-user-edit-location-removal_hint"),
    }
    Ok(())
}

async fn edit_activity(api: &Forgejo, visibility: VisibilitySetting) -> eyre::Result<()> {
    let opt = forgejo_api::structs::UserSettingsOptions {
        hide_activity: Some(visibility == VisibilitySetting::Hidden),
        ..default_settings_opt()
    };
    api.update_user_settings(opt).await?;
    Ok(())
}

async fn edit_email(
    api: &Forgejo,
    visibility: Option<VisibilitySetting>,
    add: Vec<String>,
    rm: Vec<String>,
) -> eyre::Result<()> {
    if let Some(vis) = visibility {
        let opt = forgejo_api::structs::UserSettingsOptions {
            hide_activity: Some(vis == VisibilitySetting::Hidden),
            ..default_settings_opt()
        };
        api.update_user_settings(opt).await?;
    }
    if !add.is_empty() {
        let opt = forgejo_api::structs::CreateEmailOption { emails: Some(add) };
        api.user_add_email(opt).await?;
    }
    if !rm.is_empty() {
        let opt = forgejo_api::structs::DeleteEmailOption { emails: Some(rm) };
        api.user_delete_email(opt).await?;
    }
    Ok(())
}

async fn edit_website(api: &Forgejo, new_url: Option<String>, unset: bool) -> eyre::Result<()> {
    match (new_url, unset) {
        (Some(_), true) => unreachable!(),
        (Some(url), false) if !url.is_empty() => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                website: Some(url),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        (None, true) => {
            let opt = forgejo_api::structs::UserSettingsOptions {
                website: Some(String::new()),
                ..default_settings_opt()
            };
            api.update_user_settings(opt).await?;
        }
        _ => ftl_println!("msg-user-edit-website-removal_hint"),
    }
    Ok(())
}

async fn list_keys(api: &Forgejo, verbose: bool) -> eyre::Result<()> {
    let SpecialRender {
        bold,
        bright_cyan,
        reset,
        ..
    } = *crate::special_render();

    let keys = api.user_current_list_keys(Default::default()).all().await?;

    ftl_println!("msg-user-key-list-count", keys = keys.len());

    let id_length = keys
        .iter()
        // Compute number of digits in the ID using the logarithm
        .map(|k| std::cmp::max(k.id.unwrap_or(0), 1).ilog10() + 1)
        .max()
        .unwrap_or(0) as usize;

    let title_length = keys
        .iter()
        .map(|k| k.title.as_ref().map(String::len).unwrap_or(0))
        .max()
        .unwrap_or(0);

    for key in keys {
        let id = key.id.unwrap_or(0);

        if verbose {
            println!();
            ftl_println!("msg-user-key-list-header", id);
            print_key(&key, 4);
        } else {
            let title = crate::DisplayOptional(key.title, "?");
            let fingerprint = crate::DisplayOptional(key.fingerprint, "?");

            println!(
                "{bold}{id: >id_length$} {bright_cyan}{title: <title_length$}{reset} {fingerprint}"
            );
        }
    }

    Ok(())
}

async fn view_key(api: &Forgejo, id: i64) -> eyre::Result<()> {
    let key = api.user_current_get_key(id).await?;
    print_key(&key, 0);

    Ok(())
}

fn print_key(key: &forgejo_api::structs::PublicKey, indent: usize) {
    let SpecialRender {
        bright_red, reset, ..
    } = *crate::special_render();

    let indent = " ".repeat(indent);
    let unknown_value = format!("{bright_red}?{reset}");

    print!("{indent}");
    ftl_println!(
        "msg-user-key-list-title",
        title = key.title.as_deref().unwrap_or(&unknown_value),
    );

    print!("{indent}");
    ftl_println!(
        "msg-user-key-list-created_at",
        created_at = key.created_at.as_ref().map(|ts| ts.ftl()),
    );

    print!("{indent}");
    ftl_println!(
        "msg-user-key-list-type",
        key_type = key.key_type.as_deref().unwrap_or(&unknown_value),
    );

    print!("{indent}");
    ftl_println!(
        "msg-user-key-list-fingerprint",
        fingerprint = key.fingerprint.as_deref().unwrap_or(&unknown_value),
    );

    if let Some(key) = &key.key {
        println!("\n{indent}{key}");
    }
}

async fn delete_key(api: &Forgejo, id: i64) -> eyre::Result<()> {
    api.user_current_delete_key(id).await?;
    ftl_println!("msg-user-key-delete-success", id);
    Ok(())
}

async fn upload_key(
    api: &Forgejo,
    file: Option<String>,
    title: Option<String>,
    force: bool,
    read_only: bool,
) -> eyre::Result<()> {
    use tokio::io::AsyncReadExt;

    let is_stdin = matches!(file.as_deref(), Some("-"));

    let file = if let Some(file) = file {
        std::path::PathBuf::from(file)
    } else {
        let ssh_dir = directories::UserDirs::new()
            .ok_or_else(|| ftl_eyre!("msg-user-key-upload-home_not_found"))?
            .home_dir()
            .join(".ssh");

        let mut dirstream = tokio::fs::read_dir(ssh_dir).await?;

        loop {
            let Some(entry) = dirstream.next_entry().await? else {
                ftl_bail!("msg-user-key-upload-home_not_found");
            };

            if !entry.file_type().await?.is_file() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("id_") || !name.ends_with(".pub") {
                continue;
            }

            let path = entry.path();
            ftl_ensure!(
                crate::ftl_prompt_bool!(
                    default false;
                    "msg-user-key-upload-confirm_key_file_prompt",
                    path = path.to_string_lossy(),
                )?,
                "msg-user-key-upload-file_unconfirmed",
            );

            break path;
        }
    };

    ftl_ensure!(
        force || is_stdin || file.extension().map(|e| e == "pub").unwrap_or_default(),
        "msg-user-key-upload-unexpected_extension",
        path = file.to_string_lossy(),
    );

    let content = if is_stdin {
        let mut key_content = String::new();
        tokio::io::stdin().read_to_string(&mut key_content).await?;
        key_content
    } else {
        tokio::fs::read_to_string(&file).await?
    };

    // Private keys start with
    // -----BEGIN OPENSSH PRIVATE KEY-----
    //
    // Public keys are one-line and start with "ssh-", so we check for that.
    let trimmed = content.trim();
    ftl_ensure!(
        force || (trimmed.starts_with("ssh-") && !trimmed.contains('\n')),
        "msg-user-key-upload-invalid_key",
        path = file.to_string_lossy(),
    );

    let title = if let Some(title) = title {
        title
    } else {
        let Some(guess) = trimmed.split(' ').next_back() else {
            ftl_bail!("msg-user-key-upload-no_title");
        };

        ftl_ensure!(
            crate::ftl_prompt_bool!(
                default false;
                "msg-user-key-upload-confirm_key_title_prompt",
                title = guess,
            )?,
            "msg-user-key-upload-title_unconfirmed",
        );

        guess.to_string()
    };

    let body = forgejo_api::structs::CreateKeyOption {
        key: content,
        read_only: Some(read_only),
        title,
    };

    let key = api.user_current_post_key(body).await?;
    ftl_println!("msg-user-key-upload-success");
    println!();
    print_key(&key, 0);

    Ok(())
}

async fn list_gpg(api: &Forgejo, verbose: bool) -> eyre::Result<()> {
    let SpecialRender {
        bold,
        bright_cyan,
        reset,
        ..
    } = *crate::special_render();

    let keys = api.user_current_list_gpg_keys().all().await?;

    let id_length = keys
        .iter()
        // Compute number of digits in the ID using the logarithm
        .map(|k| std::cmp::max(k.id.unwrap_or(0), 1).ilog10() + 1)
        .max()
        .unwrap_or(0) as usize;

    let keyid_length = keys
        .iter()
        .map(|k| k.key_id.as_ref().map(String::len).unwrap_or(0))
        .max()
        .unwrap_or(0);

    ftl_println!("msg-user-gpg-list-count", keys = keys.len());
    for key in keys {
        let id = key.id.unwrap_or(0);
        if verbose {
            ftl_println!("msg-user-gpg-list-header", id);
            print_gpg(&key, 4);
        } else {
            let keyid = crate::DisplayOptional(key.key_id, "?");
            println!("{bold}{id: >id_length$} {bright_cyan}{keyid: <keyid_length$}{reset}");
        }
    }

    Ok(())
}

async fn view_gpg(api: &Forgejo, id: i64) -> eyre::Result<()> {
    let key = api.user_current_get_gpg_key(id).await?;
    print_gpg(&key, 0);

    Ok(())
}

fn print_gpg(key: &forgejo_api::structs::GPGKey, indent_depth: usize) {
    let SpecialRender {
        bright_red, reset, ..
    } = *crate::special_render();

    let indent = " ".repeat(indent_depth);
    let unknown_value = format!("{bright_red}?{reset}");

    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-key_id",
        key_id = key.key_id.as_deref().unwrap_or(&unknown_value),
    );
    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-can_sign",
        can_sign = key.can_sign.unwrap_or_default().ftl(),
    );
    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-can_encrypt_comms",
        can_encrypt_comms = key.can_encrypt_comms.unwrap_or_default().ftl(),
    );
    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-can_encrypt_storage",
        can_encrypt_storage = key.can_encrypt_storage.unwrap_or_default().ftl(),
    );
    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-can_certify",
        can_certify = key.can_certify.unwrap_or_default().ftl(),
    );
    print!("{indent}");
    ftl_println!(
        "msg-user-gpg-list-verified",
        verified = key.verified.unwrap_or_default().ftl(),
    );

    for email in key.emails.as_deref().unwrap_or_default() {
        if let forgejo_api::structs::GPGKeyEmail {
            email: Some(email),
            verified,
        } = email
        {
            let verified = verified.unwrap_or_default();
            print!("{indent}");
            ftl_println!("msg-user-gpg-list-email", email, verified = verified.ftl());
        }
    }

    if let Some(key) = key.public_key.as_ref() {
        println!("\n{indent}{key}");
    }

    for subkey in key.subkeys.as_deref().unwrap_or(&[]) {
        println!();
        print!("{indent}");
        ftl_println!("msg-user-gpg-list-subkey", id = key.id);
        print_gpg(subkey, indent_depth + 4);
    }
}

async fn delete_gpg(api: &Forgejo, id: i64, force: bool) -> eyre::Result<()> {
    ftl_ensure!(
        force || crate::ftl_prompt_bool!(default false; "msg-user-gpg-delete-confirmation_prompt")?,
        "msg-user-gpg-delete-unconfirmed",
    );

    api.user_current_delete_gpg_key(id).await?;
    println!("Key with ID {id} deleted successfully.");

    Ok(())
}

async fn upload_gpg(api: &Forgejo, key_name: String, no_verify: bool) -> eyre::Result<()> {
    ftl_println!("msg-user-gpg-upload-exporting");
    let key_output = tokio::process::Command::new("gpg")
        .arg("--export")
        .arg("--armor")
        .arg(&key_name)
        .stderr(std::process::Stdio::inherit())
        .output()
        .await?;

    ftl_ensure!(
        key_output.status.success(),
        "msg-user-gpg-upload-export_failed",
        status_code = key_output.status.code(),
    );

    eyre::ensure!(!key_output.stdout.is_empty(), "No such key found!");

    let key = String::from_utf8(key_output.stdout).context("Couldn't convert key to string.")?;

    let signature = if no_verify {
        None
    } else {
        Some(gpg_verify_token(api, &key_name).await?)
    };

    let form = forgejo_api::structs::CreateGPGKeyOption {
        armored_public_key: key,
        armored_signature: signature,
    };
    let key = api.user_current_post_gpg_key(form).await?;

    ftl_println!("msg-user-gpg-upload-success");
    println!();
    print_gpg(&key, 0);

    Ok(())
}

async fn gpg_verify_token(api: &Forgejo, key_name: &str) -> eyre::Result<String> {
    use tokio::io::AsyncWriteExt;

    ftl_println!("msg-user-gpg-verify-fetching_token");
    let token = api.get_verification_token().await?;

    ftl_println!("msg-user-gpg-verify-signing_token", key_name);
    let mut child = tokio::process::Command::new("gpg")
        .arg("--armor")
        .arg("--default-key")
        .arg(key_name)
        .arg("--detach-sig")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().context("Failed to open GPG stdin")?;
    let writer = tokio::spawn(async move { stdin.write_all(token.as_bytes()).await });

    let output = child.wait_with_output().await?;
    writer.await??;

    ftl_ensure!(
        output.status.success(),
        "msg-user-gpg-upload-signing_failed",
        status_code = output.status.code(),
    );

    Ok(String::from_utf8(output.stdout)?)
}

async fn verify_gpg(api: &Forgejo, id: i64) -> eyre::Result<()> {
    let key = api.user_current_get_gpg_key(id).await?;

    let Some(key_id) = &key.key_id else {
        eyre::bail!("API didn't return a key ID!");
    };

    ftl_println!("msg-user-gpg-verify-key_to_verify");
    print_gpg(&key, 0);

    let token = gpg_verify_token(api, key_id).await?;

    let option = forgejo_api::structs::VerifyGPGKeyOption {
        armored_signature: Some(token),
        key_id: key_id.clone(),
    };
    api.user_verify_gpg_key(option).await?;

    ftl_println!("msg-user-gpg-verify-success");

    Ok(())
}
