//! App state shared by the views and the menu. Signals are cheap handles,
//! so the struct is `Copy`.

use crate::git::diff::FileDiff;
use crate::git::log::{Commit, CommitDetails};
use crate::git::status::Status;
use crate::git::Git;
use div::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The tabs of the main area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Changes,
    History,
    Branches,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Changes, Tab::History, Tab::Branches];

    /// The value the tabs component uses for this tab.
    pub fn value(self) -> &'static str {
        match self {
            Tab::Changes => "changes",
            Tab::History => "history",
            Tab::Branches => "branches",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Changes => "Changes",
            Tab::History => "History",
            Tab::Branches => "Branches",
        }
    }
}

/// A repository in the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub path: PathBuf,
    /// The folder name, shown in the list.
    pub name: String,
}

impl Repo {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        Self { path, name }
    }
}

/// What the list shows under a repository's name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Summary {
    #[default]
    Loading,
    Ready {
        /// The branch, or `None` when HEAD is detached.
        branch: Option<String>,
        changes: usize,
        ahead: u32,
        behind: u32,
    },
    /// git failed, for example because the folder is gone.
    Error(String),
}

impl Summary {
    /// `main · 3 changes · ↑2 ↓1`, `dev · clean`, `detached · clean`.
    pub fn line(&self) -> String {
        match self {
            Summary::Loading => "Loading…".into(),
            Summary::Error(message) => message.clone(),
            Summary::Ready {
                branch,
                changes,
                ahead,
                behind,
            } => {
                let mut parts = vec![branch.clone().unwrap_or_else(|| "detached".into())];
                parts.push(match changes {
                    0 => "clean".into(),
                    1 => "1 change".into(),
                    n => format!("{n} changes"),
                });
                match (ahead, behind) {
                    (0, 0) => {}
                    (a, 0) => parts.push(format!("↑{a}")),
                    (0, b) => parts.push(format!("↓{b}")),
                    (a, b) => parts.push(format!("↑{a} ↓{b}")),
                }
                parts.join(" · ")
            }
        }
    }
}

/// The file whose diff is shown in the Changes tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSelection {
    pub path: String,
    /// The staged side (index against HEAD) rather than the unstaged one.
    pub staged: bool,
    /// Not tracked yet: diffed against nothing, discarded by deleting.
    pub untracked: bool,
}

/// The diff pane's content.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiffState {
    #[default]
    Empty,
    Loading,
    Ready(FileDiff),
    Failed(String),
}

/// Something loaded in the background.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Loadable<T> {
    #[default]
    Empty,
    Loading,
    Ready(T),
    Failed(String),
}

/// The History tab's list: the selected repository's commits from HEAD,
/// loaded a page at a time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// The repository the commits belong to.
    pub repo: Option<PathBuf>,
    pub commits: Vec<Commit>,
    /// The last page was full, so there may be more.
    pub more: bool,
    pub loading: bool,
    pub error: Option<String>,
}

/// A question the confirmation dialog asks before an action that can lose
/// work.
#[derive(Clone)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    /// The label of the button that goes ahead.
    pub action: String,
    pub run: Rc<dyn Fn()>,
}

#[derive(Clone, Copy)]
pub struct State {
    /// The selected tab, as its [`Tab::value`].
    pub tab: Signal<String>,
    /// What the last operation reported, shown in the status bar.
    pub status: Signal<String>,
    /// How git is run. Tests point it at fixture configuration.
    pub git: Signal<Git>,
    /// Where the repository list is saved; `None` keeps it in memory.
    pub store: Signal<Option<PathBuf>>,
    pub repos: Signal<Vec<Repo>>,
    pub summaries: Signal<HashMap<PathBuf, Summary>>,
    pub selected: Signal<Option<PathBuf>>,
    /// The full status of the selected repository, with its path.
    pub detail: Signal<Option<(PathBuf, Status)>>,
    pub file: Signal<Option<FileSelection>>,
    pub diff: Signal<DiffState>,
    /// Show every line of a long diff instead of the first part.
    pub full_diff: Signal<bool>,
    /// A command that changes the selected repository is running.
    pub busy: Signal<bool>,
    /// The commit message being written.
    pub message: Signal<String>,
    /// The open confirmation dialog, if any.
    pub confirm: Signal<Option<Confirm>>,
    pub history: Signal<History>,
    /// The hash of the commit selected in History.
    pub commit: Signal<Option<String>>,
    pub details: Signal<Loadable<CommitDetails>>,
    /// The file of the selected commit whose diff is shown.
    pub commit_file: Signal<Option<String>>,
    pub commit_diff: Signal<DiffState>,
    pub commit_full_diff: Signal<bool>,
}

impl State {
    /// State with the user's git and no saved list.
    pub fn new() -> Self {
        Self::with(Git::new(), None)
    }

    pub fn with(git: Git, store: Option<PathBuf>) -> Self {
        Self {
            tab: create_signal(Tab::Changes.value().to_owned()),
            status: create_signal("Ready".to_owned()),
            git: create_signal(git),
            store: create_signal(store),
            repos: create_signal(Vec::new()),
            summaries: create_signal(HashMap::new()),
            selected: create_signal(None),
            detail: create_signal(None),
            file: create_signal(None),
            diff: create_signal(DiffState::Empty),
            full_diff: create_signal(false),
            busy: create_signal(false),
            message: create_signal(String::new()),
            confirm: create_signal(None),
            history: create_signal(History::default()),
            commit: create_signal(None),
            details: create_signal(Loadable::Empty),
            commit_file: create_signal(None),
            commit_diff: create_signal(DiffState::Empty),
            commit_full_diff: create_signal(false),
        }
    }

    pub fn select_tab(&self, tab: Tab) {
        self.tab.set(tab.value().to_owned());
    }

    pub fn summary(&self, path: &Path) -> Summary {
        self.summaries.get().get(path).cloned().unwrap_or_default()
    }

    /// The selected repository's status, if it is loaded.
    pub fn selected_status(&self) -> Option<Status> {
        let selected = self.selected.get()?;
        self.detail
            .get()
            .filter(|(path, _)| *path == selected)
            .map(|(_, status)| status)
    }

    /// Whether the Commit button can run: something is staged, there is
    /// a message, and no other command is running.
    pub fn can_commit(&self) -> bool {
        !self.busy.get()
            && !self.message.get().trim().is_empty()
            && self
                .selected_status()
                .is_some_and(|status| status.staged().next().is_some())
    }

    /// The selected repository, if any.
    pub fn selected_repo(&self) -> Option<Repo> {
        let selected = self.selected.get()?;
        self.repos
            .get()
            .into_iter()
            .find(|repo| repo.path == selected)
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_lines() {
        let ready = |branch: Option<&str>, changes, ahead, behind| Summary::Ready {
            branch: branch.map(str::to_owned),
            changes,
            ahead,
            behind,
        };
        assert_eq!(ready(Some("main"), 0, 0, 0).line(), "main · clean");
        assert_eq!(ready(Some("dev"), 1, 2, 0).line(), "dev · 1 change · ↑2");
        assert_eq!(ready(None, 3, 2, 1).line(), "detached · 3 changes · ↑2 ↓1");
        assert_eq!(Summary::Loading.line(), "Loading…");
    }
}
