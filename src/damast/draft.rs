//! Draft pull requests: `mast pr create --draft`, `mast pr ready`, `mast pr draft`.
//!
//! Damast and Forgejo both store "draft" as a work-in-progress title prefix, so this works
//! against plain Forgejo too: it only edits the title.

use clap::Subcommand;
use eyre::OptionExt;
use forgejo_api::{Forgejo, structs::CreatePullRequestOption};

use crate::{issues::IssueId, repo::RepoName};

/// Prefix written when marking a draft (Forgejo's default first `WORK_IN_PROGRESS_PREFIXES` entry).
const PREFIX: &str = "WIP:";
// shortcut: only Forgejo's default prefixes are recognised; read the server's list if one gets exposed.
const KNOWN_PREFIXES: [&str; 2] = ["WIP:", "[WIP]"];

#[derive(Subcommand, Clone, Debug)]
pub enum DraftCommand {
    /// Mark a draft pull request as ready for review (removes the WIP: title prefix)
    Ready {
        /// The pull request (defaults to the one for the current branch)
        pr: Option<IssueId>,
    },
    /// Convert a pull request to a draft (adds the WIP: title prefix)
    Draft {
        /// The pull request (defaults to the one for the current branch)
        pr: Option<IssueId>,
    },
}

impl DraftCommand {
    pub fn pr(&self) -> Option<&IssueId> {
        match self {
            DraftCommand::Ready { pr } | DraftCommand::Draft { pr } => pr.as_ref(),
        }
    }

    pub async fn run(self, repo: &RepoName, api: &Forgejo, id: i64) -> eyre::Result<()> {
        let want_draft = matches!(self, DraftCommand::Draft { .. });
        let done = if want_draft {
            "a draft"
        } else {
            "ready for review"
        };
        let pr = api
            .repo_get_pull_request(repo.owner(), repo.name(), id)
            .await?;
        let title = pr.title.ok_or_eyre("pr does not have title")?;
        let Some(new_title) = plan(&title, pr.draft, want_draft)? else {
            println!("#{id} is already {done}");
            return Ok(());
        };
        crate::issues::edit_title(repo, api, id, Some(new_title)).await?;
        // The server decides with its own WORK_IN_PROGRESS_PREFIXES; mast only knows the defaults.
        let now = api
            .repo_get_pull_request(repo.owner(), repo.name(), id)
            .await?
            .draft;
        if now.is_some_and(|d| d != want_draft) {
            crate::issues::edit_title(repo, api, id, Some(title)).await?;
            eyre::bail!(
                "#{id} did not become {done}: the server uses other WORK_IN_PROGRESS_PREFIXES than {KNOWN_PREFIXES:?}; \
                 change the prefix with `mast pr edit {id} title`"
            );
        }
        println!("#{id} is now {done}");
        Ok(())
    }
}

/// The new title to get to the wanted state, or `None` if the PR is already there.
/// `server_draft` is the PR's `draft` field, which the server computes from its own prefixes.
fn plan(title: &str, server_draft: Option<bool>, want_draft: bool) -> eyre::Result<Option<String>> {
    if server_draft.unwrap_or_else(|| is_draft(title)) == want_draft {
        return Ok(None);
    }
    let new_title = if want_draft {
        mark(title)
    } else {
        strip(title)
    };
    if new_title == title {
        eyre::bail!(
            "the title starts with a draft prefix mast does not know (it knows {KNOWN_PREFIXES:?}); \
             remove it with `mast pr edit <pr> title`"
        );
    }
    Ok(Some(new_title))
}

fn prefix_len(title: &str) -> Option<usize> {
    KNOWN_PREFIXES
        .iter()
        .find(|p| {
            title
                .get(..p.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(p))
        })
        .map(|p| p.len())
}

pub fn is_draft(title: &str) -> bool {
    prefix_len(title).is_some()
}

/// The title with the draft prefix, unless it already has one.
pub fn mark(title: &str) -> String {
    if is_draft(title) {
        title.to_owned()
    } else {
        format!("{PREFIX} {}", title.trim())
    }
}

/// The title without its draft prefix.
pub fn strip(title: &str) -> String {
    match prefix_len(title) {
        Some(n) => title[n..].trim().to_owned(),
        None => title.to_owned(),
    }
}

/// Hook for `pr create --draft` (API path).
pub fn mark_option(opt: CreatePullRequestOption, draft: bool) -> CreatePullRequestOption {
    if !draft {
        return opt;
    }
    CreatePullRequestOption {
        title: opt.title.as_deref().map(mark),
        ..opt
    }
}

/// Hook for `pr create --draft --agit`.
pub fn mark_if(title: String, draft: bool) -> String {
    if draft { mark(&title) } else { title }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn titles() {
        assert!(is_draft("WIP: x"));
        assert!(is_draft("wip: x"));
        assert!(is_draft("[wip] x"));
        assert!(!is_draft("x WIP:"));
        assert!(!is_draft("WI"));
        assert!(!is_draft("WIÜ: x"));

        assert_eq!(mark("x"), "WIP: x");
        assert_eq!(mark("wip: x"), "wip: x");
        assert_eq!(mark("[WIP] x"), "[WIP] x");

        assert_eq!(strip("WIP: x"), "x");
        assert_eq!(strip("[wip]  x"), "x");
        assert_eq!(strip("x"), "x");

        assert_eq!(mark_if("x".into(), false), "x");
        assert_eq!(mark_if("x".into(), true), "WIP: x");
    }

    #[test]
    fn plans() {
        assert_eq!(
            plan("x", Some(false), true).unwrap().as_deref(),
            Some("WIP: x")
        );
        assert_eq!(
            plan("WIP: x", Some(true), false).unwrap().as_deref(),
            Some("x")
        );
        assert_eq!(plan("WIP: x", Some(true), true).unwrap(), None);
        assert_eq!(plan("x", Some(false), false).unwrap(), None);
        // the server's flag wins over mast's prefix list
        assert_eq!(plan("WIP: x", Some(false), false).unwrap(), None);
        assert!(
            plan("Draft: x", Some(true), false).is_err(),
            "custom prefix mast cannot strip"
        );
        assert_eq!(plan("Draft: x", Some(true), true).unwrap(), None);
        // older servers without the field: fall back to the known prefixes
        assert_eq!(plan("[wip] x", None, false).unwrap().as_deref(), Some("x"));
    }

    #[test]
    fn option() {
        let opt = CreatePullRequestOption {
            assignee: None,
            assignees: None,
            base: None,
            body: None,
            due_date: None,
            head: None,
            labels: None,
            milestone: None,
            title: Some("Feature".into()),
        };
        assert_eq!(
            mark_option(opt.clone(), false).title.as_deref(),
            Some("Feature")
        );
        assert_eq!(
            mark_option(opt, true).title.as_deref(),
            Some("WIP: Feature")
        );
    }

    #[test]
    fn cli() {
        let app = crate::App::try_parse_from(["mast", "pr", "ready", "7"]).unwrap();
        assert!(format!("{app:?}").contains("Ready"));
        let app = crate::App::try_parse_from(["mast", "pr", "draft"]).unwrap();
        assert!(format!("{app:?}").contains("Draft"));
        let app = crate::App::try_parse_from(["mast", "pr", "create", "--draft", "Title"]).unwrap();
        assert!(format!("{app:?}").contains("draft: true"));
        assert!(crate::App::try_parse_from(["mast", "pr", "create", "--draft", "--web"]).is_err());
    }
}
