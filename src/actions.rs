//! What the buttons and menu items do. Git runs on the blocking pool;
//! results come back to the UI thread in `spawn_local` tasks, which write
//! them into the state's signals.

use crate::git::{repo, status, Error, Git};
use crate::state::{Repo, State, Summary};
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

fn summarize(result: Result<status::Status, Error>) -> Summary {
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
                set_summary(state, path, summarize(result));
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
        set_summary(state, path, summarize(result));
    })
    .detach();
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
