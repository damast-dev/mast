use std::fmt::Display;
use std::str::FromStr;

use clap::{Args, Subcommand, ValueEnum};
use eyre::OptionExt;
use forgejo_api::{
    Forgejo,
    structs::{CreateMilestoneOptionState, Milestone, StateType},
};

use crate::{
    ftl_eyre, ftl_println, h,
    localization::AsFluent,
    repo::{RepoArg, RepoInfo, RepoName},
};

#[derive(Args, Clone, Debug)]
pub struct MilestoneCommand {
    #[clap(help = h!("arg-remote"))]
    #[clap(long, short = 'R', global = true)]
    remote: Option<String>,
    #[clap(long, short, global = true)]
    repo: Option<RepoArg>,
    #[clap(subcommand)]
    command: MilestoneSubcommand,
}

/// A miletone's state as accepted by the API for filtering.
/// This is needed because forgejo_api uses strings
#[derive(ValueEnum, Copy, Clone, Debug, Default)]
pub enum MilestoneState {
    // Open being the default matches the API, which also defaults to open when we don't explicitly
    // provide a state.
    #[default]
    Open,
    Closed,
    All,
}

impl MilestoneState {
    pub fn to_api_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

/// We need to implement that display state so that clap can show possible values in the help
/// menu.
impl Display for MilestoneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_api_str())
    }
}

/// A miletone's state as accepted by the API for creation.
/// This is needed so we can use ValueEnum
#[derive(ValueEnum, Copy, Clone, Debug, Default)]
pub enum CreateMilestoneState {
    #[default]
    Open,
    Closed,
}

impl CreateMilestoneState {
    pub fn to_api_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

/// We need to implement that display state so that clap can show possible values in the help
/// menu.
impl Display for CreateMilestoneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_api_str())
    }
}

impl From<CreateMilestoneState> for CreateMilestoneOptionState {
    fn from(state: CreateMilestoneState) -> Self {
        match state {
            CreateMilestoneState::Open => CreateMilestoneOptionState::Open,
            CreateMilestoneState::Closed => CreateMilestoneOptionState::Closed,
        }
    }
}

#[derive(Subcommand, Clone, Debug)]
pub enum MilestoneSubcommand {
    #[clap(about = h!("cmd-milestone-search"))]
    Search {
        #[clap(help = h!("arg-milestone-search-state"))]
        #[clap(long, default_value_t)]
        state: MilestoneState,

        #[clap(help = h!("arg-milestone-search-title"))]
        #[clap(long)]
        title: Option<String>,
    },

    #[clap(about = h!("cmd-milestone-create"))]
    Create {
        #[clap(help = h!("arg-milestone-create-title"))]
        title: String,

        #[clap(long)]
        #[clap(help = h!("arg-milestone-create-description"))]
        description: Option<Option<String>>,

        #[clap(long)]
        #[clap( help = h!("arg-milestone-create-state"))]
        state: CreateMilestoneState,
    },

    #[clap(about = h!("cmd-milestone-view"))]
    View {
        #[clap(help = h!("arg-milestone-view-title"))]
        title: String,
    },

    #[clap(about = h!("cmd-milestone-edit"))]
    Edit {
        #[clap(help = h!("arg-milestone-edit-title"))]
        title: String,

        #[clap(long)]
        #[clap(help = h!("arg-milestone-edit-description"))]
        description: Option<String>,

        #[clap(long)]
        #[clap( help = h!("arg-milestone-edit-state"))]
        state: Option<CreateMilestoneState>,

        #[clap(long)]
        #[clap(help = h!("arg-milestone-edit-new_title"))]
        new_title: Option<String>,
    },

    #[clap(about = h!("cmd-milestone-delete"))]
    Delete {
        #[clap(help = h!("arg-milestone-delete-title"))]
        title: String,
    },
}

impl MilestoneCommand {
    pub async fn run(self, keys: &mut crate::KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        use MilestoneSubcommand::*;
        let repo =
            RepoInfo::get_current(host_name, self.repo.as_ref(), self.remote.as_deref(), keys)?;
        let api = keys.get_api(repo.host_url()).await?;
        let repo = repo
            .name()
            .ok_or_else(|| ftl_eyre!("msg-repo-name_needed"))?;

        match self.command {
            Search { state, title } => search_milestones(repo, &api, state, title).await,
            Create {
                title,
                description,
                state,
            } => create_milestone(repo, &api, title, description, state).await,
            View { title } => view_milestone(repo, &api, title).await,
            Edit {
                title,
                description,
                state,
                new_title,
            } => edit_milestone(repo, &api, title, description, state, new_title).await,
            Delete { title } => delete_milestone(repo, &api, title).await,
        }
    }
}

async fn search_milestones(
    repo: &RepoName,
    api: &Forgejo,
    state: MilestoneState,
    name: Option<String>,
) -> eyre::Result<()> {
    let res = api
        .issue_get_milestones_list(
            repo.owner(),
            repo.name(),
            forgejo_api::structs::IssueGetMilestonesListQuery {
                state: Some(state.to_api_str().to_owned()),
                name,
            },
        )
        .all()
        .await?;
    ftl_println!("msg-milestone-search-total", milestones = res.len());
    for milestone in res {
        let title = milestone
            .title
            .ok_or_eyre("milestone does not have title")?;
        let open = milestone
            .open_issues
            .ok_or_eyre("milestone does not have open issues")?;
        let closed = milestone
            .closed_issues
            .ok_or_eyre("milestone does not have closed issues")?;
        let state = milestone
            .state
            .ok_or_eyre("milestone does not have state")?;

        let state = match state {
            StateType::Open => "open",
            StateType::Closed => "closed",
        };

        ftl_println!("msg-milestone-search-entry", title, open, closed, state);
    }
    Ok(())
}

async fn create_milestone(
    repo: &RepoName,
    api: &Forgejo,
    title: String,
    description: Option<Option<String>>,
    state: CreateMilestoneState,
) -> eyre::Result<()> {
    let description = match description {
        Some(Some(description)) => Some(description),
        Some(None) => {
            let mut s = String::new();
            crate::editor(&mut s, Some("md")).await?;
            Some(s)
        }
        None => None,
    };

    let milestone = api
        .issue_create_milestone(
            repo.owner(),
            repo.name(),
            forgejo_api::structs::CreateMilestoneOption {
                description,
                due_on: None, // TODO
                state: Some(state.into()),
                title: Some(title),
            },
        )
        .await?;

    show_milestone(&milestone)?;
    Ok(())
}

pub fn show_milestone(milestone: &Milestone) -> eyre::Result<()> {
    let title = milestone
        .title
        .as_ref()
        .ok_or_eyre("milestone does not have title")?;
    let open = milestone
        .open_issues
        .ok_or_eyre("milestone does not have open issues")?;
    let closed = milestone
        .closed_issues
        .ok_or_eyre("milestone does not have closed issues")?;
    let state = milestone
        .state
        .ok_or_eyre("milestone does not have state")?;
    let due_on = milestone.due_on;
    let created_at = milestone
        .created_at
        .ok_or_eyre("milestone does not have creation date")?;
    let description = milestone.description.as_deref();
    let updated_at = milestone
        .updated_at
        .ok_or_eyre("milestone does not have updated_at")?;

    let state = match state {
        StateType::Open => "open",
        StateType::Closed => "closed",
    };

    let description = match description {
        Some("") | None => None,
        Some(x) => Some(x),
    };

    ftl_println!(
        "msg-milestone-view",
        title,
        open,
        closed,
        state,
        due_on = due_on.map(AsFluent::ftl),
        created_at = created_at.ftl(),
        description,
        updated_at = updated_at.ftl(),
    );
    Ok(())
}

async fn view_milestone(repo: &RepoName, api: &Forgejo, name: String) -> eyre::Result<()> {
    let milestone = if let Ok(id) = i64::from_str(&name) {
        api.issue_get_milestone(repo.owner(), repo.name(), id)
            .await?
    } else {
        // TODO: this could be `issue_get_milestone` as well, but due to the OpenAPI spec being
        // inaccurate, the types for that don't work out.  See:
        // https://codeberg.org/forgejo/forgejo/issues/13789
        let (_, milestones) = api
            .issue_get_milestones_list(
                repo.owner(),
                repo.name(),
                forgejo_api::structs::IssueGetMilestonesListQuery {
                    state: None,
                    name: Some(name),
                },
            )
            .await?;
        milestones
            .into_iter()
            .next()
            .ok_or_else(|| ftl_eyre!("msg-milestone-not_found"))?
    };

    show_milestone(&milestone)?;
    Ok(())
}

pub async fn get_milestone_id(repo: &RepoName, api: &Forgejo, name: String) -> eyre::Result<i64> {
    if let Ok(id) = i64::from_str(&name) {
        return Ok(id);
    }

    let (_, milestones) = api
        .issue_get_milestones_list(
            repo.owner(),
            repo.name(),
            forgejo_api::structs::IssueGetMilestonesListQuery {
                state: None,
                name: Some(name),
            },
        )
        .await?;

    milestones
        .into_iter()
        .next()
        .ok_or_else(|| ftl_eyre!("msg-milestone-not_found"))?
        .id
        .ok_or_eyre("milestone does not have id")
}

async fn edit_milestone(
    repo: &RepoName,
    api: &Forgejo,
    name: String,
    description: Option<String>,
    state: Option<CreateMilestoneState>,
    new_title: Option<String>,
) -> eyre::Result<()> {
    let id = get_milestone_id(repo, api, name).await?;

    let milestone = api
        .issue_edit_milestone(
            repo.owner(),
            repo.name(),
            id,
            forgejo_api::structs::EditMilestoneOption {
                description,
                due_on: None, // TODO
                state: state.map(|s| s.to_api_str().to_string()),
                title: new_title,
            },
        )
        .await?;

    show_milestone(&milestone)?;

    Ok(())
}

async fn delete_milestone(repo: &RepoName, api: &Forgejo, name: String) -> eyre::Result<()> {
    let id = get_milestone_id(repo, api, name).await?;
    api.issue_delete_milestone(repo.owner(), repo.name(), id)
        .await?;
    ftl_println!("msg-milestone-deleted");
    Ok(())
}
