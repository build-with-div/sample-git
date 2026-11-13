//! What the buttons and menu items do. Git runs on the blocking pool;
//! results come back to the UI thread in `spawn_local` tasks, which write
//! them into the state's signals.

use crate::git::diff::{self, Source};
use crate::git::{repo, status, Error, Git};
use crate::state::{DiffState, FileSelection, Repo, State, Summary};
use crate::store;
use div::task::{spawn_blocking, spawn_local};
use std::path::{Path, PathBuf};

/// How deep "Scan folder" looks, and how many repositories it adds at most.
pub const SCAN_DEPTH: usize = 3;
pub const SCAN_LIMIT: usize = 500;
/// Status refreshes that run at once.
const PARALLEL_REFRESHES: usize = 4;

/// Whether two paths name the same repository (case-insensitive on
/// Windows, where git and the file dialog may disagree on case).
pub fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

fn summarize(result: &Result<status::Status, Error>) -> Summary {
    match result {
        Ok(status) => Summary::Ready {
            branch: status.branch.head.clone(),
            changes: status.entries.len(),
            ahead: status.branch.ahead,
            behind: status.branch.behind,
        },
        Err(error) => Summary::Error(error.summary()),
    }
}

fn set_summary(state: State, path: PathBuf, summary: Summary) {
    state.summaries.update(|summaries| {
        summaries.insert(path, summary);
    });
}

/// Load the saved list, select the first repository and refresh them all.
pub fn load_saved(state: State) {
    let Some(file) = state.store.get() else {
        return;
    };
    let repos: Vec<Repo> = store::load(&file).into_iter().map(Repo::new).collect();
    if state.selected.get().is_none() {
        state
            .selected
            .set(repos.first().map(|repo| repo.path.clone()));
    }
    state.repos.set(repos);
    refresh_all(state);
}

fn save(state: State) {
    let Some(file) = state.store.get() else {
        return;
    };
    let paths: Vec<PathBuf> = state
        .repos
        .get()
        .into_iter()
        .map(|repo| repo.path)
        .collect();
    if let Err(error) = store::save(&file, &paths) {
        state.status.set(format!(
            "Could not save the list to {}: {error}",
            file.display()
        ));
    }
}

/// Refresh the summary of every listed repository, a few at a time.
pub fn refresh_all(state: State) {
    let git = state.git.get();
    let paths: Vec<PathBuf> = state
        .repos
        .get()
        .into_iter()
        .map(|repo| repo.path)
        .collect();
    spawn_local(async move {
        for chunk in paths.chunks(PARALLEL_REFRESHES) {
            let tasks: Vec<_> = chunk
                .iter()
                .map(|path| {
                    let (git, path) = (git.clone(), path.clone());
                    spawn_blocking(move || {
                        let result = status::status(&git, &path);
                        (path, result)
                    })
                })
                .collect();
            for task in tasks {
                let (path, result) = task.await;
                apply_status(state, path, result);
            }
        }
    })
    .detach();
}

/// Refresh one repository's summary.
pub fn refresh(state: State, path: PathBuf) {
    let git = state.git.get();
    spawn_local(async move {
        let probe = path.clone();
        let result = spawn_blocking(move || status::status(&git, &probe)).await;
        apply_status(state, path, result);
    })
    .detach();
}

/// Record a status: the list's summary, and for the selected repository
/// the Changes tab's lists and diff.
fn apply_status(state: State, path: PathBuf, result: Result<status::Status, Error>) {
    set_summary(state, path.clone(), summarize(&result));
    if state.selected.get().as_deref() != Some(path.as_path()) {
        return;
    }
    match result {
        Ok(status) => {
            // The selected file, if it is still in the list it was in.
            let file = state.file.get().and_then(|file| {
                let entry = status
                    .entries
                    .iter()
                    .find(|entry| entry.path == file.path)?;
                let change = if file.staged {
                    entry.staged
                } else {
                    entry.unstaged
                }?;
                Some(FileSelection {
                    untracked: change == status::Change::Untracked,
                    ..file
                })
            });
            state.detail.set(Some((path, status)));
            match file {
                Some(file) => {
                    if state.file.get().as_ref() != Some(&file) {
                        state.file.set(Some(file));
                    }
                    load_diff(state);
                }
                None => {
                    state.file.set(None);
                    state.diff.set(DiffState::Empty);
                }
            }
        }
        Err(_) => state.detail.set(None),
    }
}

/// The selected repository changed: forget the old file and load the new
/// repository's status.
pub fn selection_changed(state: State, selected: Option<PathBuf>) {
    state.file.set(None);
    state.diff.set(DiffState::Empty);
    state.detail.set(None);
    if let Some(path) = selected {
        refresh(state, path);
    }
}

/// Show the diff of `file` in the selected repository.
pub fn select_file(state: State, file: FileSelection) {
    state.full_diff.set(false);
    state.file.set(Some(file));
    load_diff(state);
}

fn load_diff(state: State) {
    let (Some(repo), Some(file)) = (state.selected.get(), state.file.get()) else {
        state.diff.set(DiffState::Empty);
        return;
    };
    if !matches!(state.diff.get(), DiffState::Ready(_)) {
        state.diff.set(DiffState::Loading);
    }
    let git = state.git.get();
    spawn_local(async move {
        let source = match (file.staged, file.untracked) {
            (true, _) => Source::Staged,
            (false, true) => Source::Untracked,
            (false, false) => Source::Unstaged,
        };
        let path = file.path.clone();
        let result = spawn_blocking(move || diff::diff_file(&git, &repo, &path, &source)).await;
        // Drop the answer if the user picked another file meanwhile.
        if state.file.get().as_ref() != Some(&file) {
            return;
        }
        state.diff.set(match result {
            Ok(diff) => DiffState::Ready(diff),
            Err(error) => DiffState::Failed(error.summary()),
        });
    })
    .detach();
}

/// Run `operation` on the selected repository on the blocking pool, with
/// the busy flag set, then refresh. Errors go to the status bar.
fn mutate(
    state: State,
    done: impl Into<String>,
    operation: impl FnOnce(&Git, &Path) -> Result<(), Error> + Send + 'static,
) {
    let Some(repo) = state.selected.get() else {
        return;
    };
    if state.busy.get() {
        return;
    }
    let done = done.into();
    let git = state.git.get();
    state.busy.set(true);
    spawn_local(async move {
        let path = repo.clone();
        let result = spawn_blocking(move || operation(&git, &path)).await;
        state.busy.set(false);
        state.status.set(match result {
            Ok(()) => done,
            Err(error) => error.summary(),
        });
        refresh(state, repo);
    })
    .detach();
}

/// Follow a staged or unstaged file to the other list.
fn move_selection(state: State, paths: Option<&[String]>, staged: bool) {
    if let Some(file) = state.file.get() {
        if paths.is_none_or(|paths| paths.contains(&file.path)) {
            state.file.set(Some(FileSelection {
                staged,
                untracked: false,
                ..file
            }));
        }
    }
}

pub fn stage(state: State, path: String) {
    let paths = vec![path.clone()];
    move_selection(state, Some(&paths), true);
    mutate(state, format!("Staged {path}"), move |git, repo| {
        repo::stage(git, repo, &paths)
    });
}

pub fn unstage(state: State, path: String) {
    let paths = vec![path.clone()];
    move_selection(state, Some(&paths), false);
    mutate(state, format!("Unstaged {path}"), move |git, repo| {
        repo::unstage(git, repo, &paths)
    });
}

pub fn stage_all(state: State) {
    move_selection(state, None, true);
    mutate(state, "Staged all changes", |git, repo| {
        repo::stage_all(git, repo)
    });
}

pub fn unstage_all(state: State) {
    move_selection(state, None, false);
    mutate(state, "Unstaged all changes", |git, repo| {
        repo::unstage_all(git, repo)
    });
}

/// Add the repository containing `folder`, select it and refresh it. A
/// folder outside any repository is reported in the status bar.
pub fn add_repo(state: State, folder: PathBuf) {
    let git = state.git.get();
    spawn_local(async move {
        let probe = folder.clone();
        let top = spawn_blocking(move || repo::toplevel(&git, &probe)).await;
        match top {
            Ok(Some(top)) => {
                let name = Repo::new(top.clone()).name;
                if insert(state, top.clone()) {
                    state.status.set(format!("Added {name}"));
                    save(state);
                    refresh(state, top.clone());
                } else {
                    state.status.set(format!("{name} is already in the list"));
                }
                state
                    .selected
                    .set(Some(existing(state, &top).unwrap_or(top)));
            }
            Ok(None) => state
                .status
                .set(format!("{} is not in a git repository", folder.display())),
            Err(error) => state.status.set(error.summary()),
        }
    })
    .detach();
}

/// The listed path equal to `path`, as it is spelled in the list.
fn existing(state: State, path: &Path) -> Option<PathBuf> {
    state
        .repos
        .get()
        .into_iter()
        .map(|repo| repo.path)
        .find(|listed| same_path(listed, path))
}

/// Add `path` unless it is listed; returns whether it was added.
fn insert(state: State, path: PathBuf) -> bool {
    if existing(state, &path).is_some() {
        return false;
    }
    state.repos.update(|repos| repos.push(Repo::new(path)));
    true
}

/// Add every repository found under `root`.
pub fn scan_folder(state: State, root: PathBuf) {
    state.status.set(format!("Scanning {}…", root.display()));
    spawn_local(async move {
        let found = spawn_blocking(move || repo::scan(&root, SCAN_DEPTH, SCAN_LIMIT)).await;
        let total = found.len();
        let added: Vec<PathBuf> = found
            .into_iter()
            .filter(|path| insert(state, path.clone()))
            .collect();
        state.status.set(match (total, added.len()) {
            (0, _) => "No repositories found".to_owned(),
            (total, new) => format!(
                "Found {total} {} ({new} new)",
                if total == 1 {
                    "repository"
                } else {
                    "repositories"
                }
            ),
        });
        if !added.is_empty() {
            save(state);
            if state.selected.get().is_none() {
                state.selected.set(added.first().cloned());
            }
            for path in added {
                refresh(state, path);
            }
        }
    })
    .detach();
}

/// Take `path` off the list. Nothing on disk changes.
pub fn remove_repo(state: State, path: &Path) {
    let repos: Vec<Repo> = state
        .repos
        .get()
        .into_iter()
        .filter(|repo| !same_path(&repo.path, path))
        .collect();
    if state
        .selected
        .get()
        .is_some_and(|selected| same_path(&selected, path))
    {
        state
            .selected
            .set(repos.first().map(|repo| repo.path.clone()));
    }
    let name = Repo::new(path.to_owned()).name;
    state.repos.set(repos);
    state.summaries.update(|summaries| {
        summaries.remove(path);
    });
    state.status.set(format!("Removed {name} from the list"));
    save(state);
}

/// Ask for a folder and add its repository.
pub fn pick_and_add(state: State) {
    spawn_local(async move {
        match div::dialog::pick_folder()
            .title("Add a repository")
            .pick()
            .await
        {
            Ok(Some(folder)) => add_repo(state, folder),
            Ok(None) => {}
            Err(error) => state
                .status
                .set(format!("The folder dialog failed: {error}")),
        }
    })
    .detach();
}

/// Ask for a folder and add the repositories under it.
pub fn pick_and_scan(state: State) {
    spawn_local(async move {
        match div::dialog::pick_folder()
            .title("Scan a folder for repositories")
            .pick()
            .await
        {
            Ok(Some(folder)) => scan_folder(state, folder),
            Ok(None) => {}
            Err(error) => state
                .status
                .set(format!("The folder dialog failed: {error}")),
        }
    })
    .detach();
}

/// Refresh the selected repository every `period` while the app runs.
pub fn refresh_selected_every(state: State, period: std::time::Duration) {
    spawn_local(async move {
        let mut interval = div::task::time::interval(period);
        loop {
            interval.tick().await;
            if let Some(path) = state.selected.get() {
                refresh(state, path);
            }
        }
    })
    .detach();
}

/// The git used before the window opens, to warn about a missing or old
/// git in the status bar.
pub fn check_git(state: State, git: Git) {
    spawn_local(async move {
        let version = spawn_blocking(move || git.version()).await;
        match version {
            Ok(version) if version >= crate::git::MIN_VERSION => {}
            Ok((major, minor)) => state.status.set(format!(
                "git {major}.{minor} is older than {}.{}; some views may fail",
                crate::git::MIN_VERSION.0,
                crate::git::MIN_VERSION.1
            )),
            Err(error) => state.status.set(error.summary()),
        }
    })
    .detach();
}
