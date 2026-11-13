//! What the buttons and menu items do. Git runs on the blocking pool;
//! results come back to the UI thread in `spawn_local` tasks, which write
//! them into the state's signals.

use crate::git::diff::{self, Source};
use crate::git::remote::{self, Progress};
use crate::git::{branch, log, repo, status, Cancel, Error, Git};
use crate::state::{
    Branches, Confirm, DiffState, FileSelection, History, Loadable, RemoteFailure, RemoteOp, Repo,
    State, Summary,
};
use crate::store;
use div::task::{spawn_blocking, spawn_local};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// How deep "Scan folder" looks, and how many repositories it adds at most.
pub const SCAN_DEPTH: usize = 3;
pub const SCAN_LIMIT: usize = 500;
/// Commits per History page.
pub const HISTORY_PAGE: usize = 200;
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
            // HEAD moved (a commit, a switch, a pull, or work outside the
            // app): reload History from the top.
            let history = state.history.get();
            let head = status.branch.oid.as_deref();
            let stale = history.repo.as_deref() != Some(path.as_path())
                || (!history.loading
                    && history.commits.first().map(|commit| commit.hash.as_str()) != head);
            if stale {
                load_history(state, path.clone(), false);
            }
            // Branches are cheap to list, and a refresh may follow work
            // outside the app, so every status reloads them.
            load_branches(state, path.clone());
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
    state.history.set(History::default());
    clear_commit(state);
    state.branches.set(Branches::default());
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

/// Load a page of `path`'s history: the first page, replacing the list, or
/// the next one after it.
fn load_history(state: State, path: PathBuf, append: bool) {
    let mut history = state.history.get();
    if append && (history.loading || !history.more) {
        return;
    }
    if !append {
        history = History {
            repo: Some(path.clone()),
            ..History::default()
        };
    }
    history.loading = true;
    let skip = history.commits.len();
    state.history.set(history);
    let git = state.git.get();
    spawn_local(async move {
        let probe = path.clone();
        let page = spawn_blocking(move || log::log(&git, &probe, skip, HISTORY_PAGE)).await;
        let mut history = state.history.get();
        // Another repository was selected meanwhile.
        if history.repo.as_deref() != Some(path.as_path()) {
            return;
        }
        history.loading = false;
        match page {
            Ok(page) => {
                history.more = page.len() == HISTORY_PAGE;
                history.commits.extend(page);
                history.error = None;
            }
            Err(error) => history.error = Some(error.summary()),
        }
        let selected = state.commit.get();
        let kept = selected.is_none_or(|hash| history.commits.iter().any(|c| c.hash == hash));
        state.history.set(history);
        if !kept {
            clear_commit(state);
        }
    })
    .detach();
}

/// Load the next page of History.
pub fn load_more(state: State) {
    if let Some(path) = state.selected.get() {
        load_history(state, path, true);
    }
}

fn clear_commit(state: State) {
    state.commit.set(None);
    state.details.set(Loadable::Empty);
    state.commit_file.set(None);
    state.commit_diff.set(DiffState::Empty);
}

/// Show `hash`'s message, metadata and changed files.
pub fn select_commit(state: State, hash: String) {
    let Some(repo) = state.selected.get() else {
        return;
    };
    if state.commit.get().as_ref() == Some(&hash) {
        return;
    }
    clear_commit(state);
    state.commit.set(Some(hash.clone()));
    state.details.set(Loadable::Loading);
    let git = state.git.get();
    spawn_local(async move {
        let wanted = hash.clone();
        let result = spawn_blocking(move || log::details(&git, &repo, &wanted)).await;
        if state.commit.get().as_ref() != Some(&hash) {
            return;
        }
        state.details.set(match result {
            Ok(details) => Loadable::Ready(details),
            Err(error) => Loadable::Failed(error.summary()),
        });
    })
    .detach();
}

/// Show the selected commit's diff of `path`.
pub fn select_commit_file(state: State, path: String) {
    let (Some(repo), Some(hash)) = (state.selected.get(), state.commit.get()) else {
        return;
    };
    state.commit_full_diff.set(false);
    state.commit_file.set(Some(path.clone()));
    state.commit_diff.set(DiffState::Loading);
    let git = state.git.get();
    spawn_local(async move {
        let (wanted, source) = (path.clone(), Source::Commit(hash.clone()));
        let result = spawn_blocking(move || diff::diff_file(&git, &repo, &wanted, &source)).await;
        if state.commit.get().as_ref() != Some(&hash)
            || state.commit_file.get().as_ref() != Some(&path)
        {
            return;
        }
        state.commit_diff.set(match result {
            Ok(diff) => DiffState::Ready(diff),
            Err(error) => DiffState::Failed(error.summary()),
        });
    })
    .detach();
}

/// List `path`'s branches. The old list stays up while the same
/// repository reloads, so a refresh does not flicker.
fn load_branches(state: State, path: PathBuf) {
    if state.branches.get().repo.as_deref() != Some(path.as_path()) {
        state.branches.set(Branches {
            repo: Some(path.clone()),
            loading: true,
            ..Branches::default()
        });
    }
    let git = state.git.get();
    spawn_local(async move {
        let probe = path.clone();
        let result = spawn_blocking(move || {
            Ok::<_, Error>((
                branch::branches(&git, &probe)?,
                remote::remotes(&git, &probe)?,
            ))
        })
        .await;
        let mut branches = state.branches.get();
        // Another repository was selected meanwhile.
        if branches.repo.as_deref() != Some(path.as_path()) {
            return;
        }
        branches.loading = false;
        match result {
            Ok((list, remotes)) => {
                branches.list = list;
                branches.remotes = remotes;
                branches.error = None;
            }
            Err(error) => branches.error = Some(error.summary()),
        }
        if state.branches.get() != branches {
            state.branches.set(branches);
        }
    })
    .detach();
}

/// Switch the selected repository to the local branch `name`. git refuses
/// when local changes would be overwritten, and says why in the status bar.
pub fn switch_branch(state: State, name: String) {
    mutate(state, format!("Switched to {name}"), move |git, repo| {
        branch::switch(git, repo, &name)
    });
}

/// Create the branch typed into "New branch" at HEAD, switch to it and
/// clear the box.
pub fn create_branch(state: State) {
    if !state.can_create_branch() {
        return;
    }
    let name = state.new_branch.get().trim().to_owned();
    mutate_then(
        state,
        format!("Created and switched to {name}"),
        {
            let name = name.clone();
            move |git, repo| {
                if !branch::is_valid_name(git, &name) {
                    return Err(Error::Io(format!("\"{name}\" is not a valid branch name")));
                }
                branch::create(git, repo, &name)
            }
        },
        move || state.new_branch.set(String::new()),
    );
}

/// Create a local branch tracking the remote branch `name` (such as
/// `origin/feature`) and switch to it.
pub fn check_out_remote(state: State, name: String) {
    let local = name.split_once('/').map_or(name.as_str(), |(_, rest)| rest);
    let done = format!("Created {local} tracking {name}");
    mutate(state, done, move |git, repo| {
        branch::track(git, repo, &name)
    });
}

/// Ask, then delete the local branch `name` if it is merged.
pub fn ask_delete_branch(state: State, name: String) {
    state.confirm.set(Some(Confirm {
        title: format!("Delete branch {name}?"),
        body: "git deletes the branch only if it is merged into its upstream \
               branch, or into the current branch when it has none. An \
               unmerged branch is kept, and the status bar says why."
            .to_owned(),
        detail: None,
        action: "Delete branch".to_owned(),
        destructive: true,
        run: Some(Rc::new(move || delete_branch(state, name.clone()))),
    }));
}

fn delete_branch(state: State, name: String) {
    mutate(state, format!("Deleted branch {name}"), move |git, repo| {
        branch::delete(git, repo, &name)
    });
}

/// Which remote command [`run_remote`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteCommand {
    Fetch,
    Pull,
    Push,
}

impl RemoteCommand {
    fn running(self) -> &'static str {
        match self {
            RemoteCommand::Fetch => "Fetching",
            RemoteCommand::Pull => "Pulling",
            RemoteCommand::Push => "Pushing",
        }
    }

    fn name(self) -> &'static str {
        match self {
            RemoteCommand::Fetch => "Fetch",
            RemoteCommand::Pull => "Pull",
            RemoteCommand::Push => "Push",
        }
    }
}

/// `git fetch --all --prune`.
pub fn fetch(state: State) {
    if state.can_fetch() {
        run_remote(state, RemoteCommand::Fetch, None, "Fetched".to_owned());
    }
}

/// `git pull --ff-only` from the current branch's upstream.
pub fn pull(state: State) {
    let Some(status) = state.selected_status() else {
        return;
    };
    if let (true, Some(upstream)) = (state.can_pull(), status.branch.upstream) {
        run_remote(
            state,
            RemoteCommand::Pull,
            None,
            format!("Pulled {upstream}"),
        );
    }
}

/// `git push` of the current branch to its upstream. A branch without one
/// is published to the first remote (`origin` first) after a question.
pub fn push(state: State) {
    let Some(status) = state.selected_status() else {
        return;
    };
    let Some(branch) = status.branch.head else {
        return;
    };
    if !state.can_push() {
        return;
    }
    if let Some(upstream) = status.branch.upstream {
        let done = format!("Pushed {branch} to {upstream}");
        run_remote(state, RemoteCommand::Push, None, done);
        return;
    }
    let Some(remote) = state.remotes().into_iter().next() else {
        return;
    };
    state.confirm.set(Some(Confirm {
        title: format!("Publish {branch}?"),
        body: format!(
            "{branch} has no upstream branch yet. Push it to {remote} and track \
             {remote}/{branch} from now on?"
        ),
        detail: Some(format!("git push -u {remote} {branch}")),
        action: "Publish branch".to_owned(),
        destructive: false,
        run: Some(Rc::new(move || {
            let done = format!("Published {branch} to {remote}");
            let target = Some((remote.clone(), branch.clone()));
            run_remote(state, RemoteCommand::Push, target, done);
        })),
    }));
}

/// Stop the running fetch, pull or push.
pub fn cancel_remote(state: State) {
    if let Some(op) = state.remote_op.get() {
        op.cancel.cancel();
    }
}

/// Show the full output of the fetch, pull or push that failed.
pub fn show_remote_failure(state: State) {
    if let Some(failure) = state.shown_failure() {
        state.confirm.set(Some(Confirm {
            title: failure.title,
            body: failure.status,
            detail: Some(failure.log),
            action: "Close".to_owned(),
            destructive: false,
            run: None,
        }));
    }
}

/// Run a fetch, pull or push on the selected repository with progress in
/// the status bar and a Cancel button, then refresh. A failure keeps git's
/// whole output for Details.
fn run_remote(
    state: State,
    command: RemoteCommand,
    set_upstream: Option<(String, String)>,
    done: String,
) {
    let Some(repo) = state.selected.get() else {
        return;
    };
    if state.busy.get() {
        return;
    }
    let git = state.git.get();
    let cancel = Cancel::new();
    state.busy.set(true);
    state.progress.set(String::new());
    state.remote_op.set(Some(RemoteOp {
        label: command.running().to_owned(),
        cancel: cancel.clone(),
    }));
    // Progress comes from the blocking thread.
    let progress = state.progress.sender();
    spawn_local(async move {
        let path = repo.clone();
        let result = spawn_blocking(move || {
            let on_progress = |update: Progress| progress.send(update.line);
            match command {
                RemoteCommand::Fetch => remote::fetch(&git, &path, &cancel, on_progress),
                RemoteCommand::Pull => remote::pull(&git, &path, &cancel, on_progress),
                RemoteCommand::Push => {
                    let target = set_upstream
                        .as_ref()
                        .map(|(remote, branch)| (remote.as_str(), branch.as_str()));
                    remote::push(&git, &path, target, &cancel, on_progress)
                }
            }
        })
        .await;
        state.busy.set(false);
        state.remote_op.set(None);
        match result {
            Ok(_) => state.status.set(done),
            Err(Error::Cancelled) => state.status.set(format!("{} cancelled", command.name())),
            Err(error) => {
                let summary = error.summary();
                let log = match &error {
                    Error::Failed {
                        command, stderr, ..
                    } => format!("$ {command}\n{}", stderr.trim_end()),
                    other => other.summary(),
                };
                state.remote_failure.set(Some(RemoteFailure {
                    status: summary.clone(),
                    title: format!("{} failed", command.name()),
                    log,
                }));
                state.status.set(summary);
            }
        }
        refresh(state, repo);
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
    mutate_then(state, done, operation, || {});
}

/// [`mutate`], running `succeeded` on the UI thread if the operation worked.
fn mutate_then(
    state: State,
    done: impl Into<String>,
    operation: impl FnOnce(&Git, &Path) -> Result<(), Error> + Send + 'static,
    succeeded: impl FnOnce() + 'static,
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
        match result {
            Ok(()) => {
                state.status.set(done);
                succeeded();
            }
            Err(error) => state.status.set(error.summary()),
        }
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

/// Commit what is staged with the message being written, then clear it.
/// Hooks and signing run as they would on the command line.
pub fn commit(state: State) {
    if !state.can_commit() {
        return;
    }
    let message = state.message.get();
    let subject = message.lines().next().unwrap_or_default().trim().to_owned();
    mutate_then(
        state,
        format!("Committed: {subject}"),
        move |git, repo| repo::commit(git, repo, &message),
        move || state.message.set(String::new()),
    );
}

/// Ask, then throw away the unstaged changes to `path`; an untracked file
/// is deleted.
pub fn ask_discard(state: State, path: String, untracked: bool) {
    let (title, body, action) = if untracked {
        (
            format!("Delete {path}?"),
            "This file is not tracked by git, so it will be deleted. This cannot be undone.",
            "Delete file",
        )
    } else {
        (
            format!("Discard changes to {path}?"),
            "The changes to this file that are not staged will be lost. This cannot be undone.",
            "Discard changes",
        )
    };
    state.confirm.set(Some(Confirm {
        title,
        body: body.to_owned(),
        detail: None,
        action: action.to_owned(),
        destructive: true,
        run: Some(Rc::new(move || discard(state, path.clone(), untracked))),
    }));
}

fn discard(state: State, path: String, untracked: bool) {
    let done = if untracked {
        format!("Deleted {path}")
    } else {
        format!("Discarded changes to {path}")
    };
    mutate(state, done, move |git, repo| {
        repo::discard(git, repo, &path, untracked)
    });
}

/// Close the confirmation dialog without doing anything.
pub fn cancel_confirm(state: State) {
    state.confirm.set(None);
}

/// Close the dialog and run its action, if it has one.
pub fn accept_confirm(state: State) {
    if let Some(confirm) = state.confirm.get() {
        state.confirm.set(None);
        if let Some(run) = confirm.run {
            run();
        }
    }
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
