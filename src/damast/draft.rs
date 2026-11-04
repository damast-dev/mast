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
        let pr = api
            .repo_get_pull_request(repo.owner(), repo.name(), id)
            .await?;
        let title = pr.title.ok_or_eyre("pr does not have title")?;
        let (new_title, done) = match self {
            DraftCommand::Ready { .. } => (strip(&title), "ready for review"),
            DraftCommand::Draft { .. } => (mark(&title), "a draft"),
        };
        if new_title == title {
            println!("#{id} is already {done}");
            return Ok(());
        }
        crate::issues::edit_title(repo, api, id, Some(new_title)).await?;
        println!("#{id} is now {done}");
        Ok(())
    }
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
