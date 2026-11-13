//! Opening repositories, staging, discarding and committing.

use super::command::{Access, Error, Git};
use crate::git_args;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// The top-level directory of the repository containing `path`, or `None`
/// when `path` is not inside a working tree.
pub fn toplevel(git: &Git, path: &Path) -> Result<Option<PathBuf>, Error> {
    let args = git_args!("rev-parse", "--show-toplevel");
    let output = git.output(Some(path), &args, Access::Read, None)?;
    if output.code != Some(0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let top = text.trim();
    Ok((!top.is_empty()).then(|| PathBuf::from(top)))
}

fn with_paths<'a>(head: &'a [&'a str], paths: &'a [String]) -> Vec<&'a OsStr> {
    let mut args: Vec<&OsStr> = head.iter().map(OsStr::new).collect();
    args.push(OsStr::new("--"));
    args.extend(paths.iter().map(OsStr::new));
    args
}

/// Stage `paths`: changes, new files and deletions.
pub fn stage(git: &Git, repo: &Path, paths: &[String]) -> Result<(), Error> {
    let args = with_paths(&["--literal-pathspecs", "add", "-A"], paths);
    git.run(repo, &args, Access::Write).map(drop)
}

/// Stage everything.
pub fn stage_all(git: &Git, repo: &Path) -> Result<(), Error> {
    git.run(repo, &git_args!("add", "-A"), Access::Write)
        .map(drop)
}

/// Unstage `paths`, keeping the working tree. Works before the first
/// commit, where there is no HEAD to restore from.
pub fn unstage(git: &Git, repo: &Path, paths: &[String]) -> Result<(), Error> {
    let args = with_paths(&["--literal-pathspecs", "reset", "-q"], paths);
    git.run(repo, &args, Access::Write).map(drop)
}

/// Unstage everything.
pub fn unstage_all(git: &Git, repo: &Path) -> Result<(), Error> {
    git.run(repo, &git_args!("reset", "-q"), Access::Write)
        .map(drop)
}

/// Throw away the unstaged changes to `path`. A tracked file goes back to
/// its staged (or committed) content; an untracked file or folder is
/// deleted. The app asks before calling this.
pub fn discard(git: &Git, repo: &Path, path: &str, untracked: bool) -> Result<(), Error> {
    let paths = [path.to_owned()];
    let args = if untracked {
        with_paths(&["--literal-pathspecs", "clean", "-f", "-d", "-q"], &paths)
    } else {
        with_paths(&["--literal-pathspecs", "restore", "--worktree"], &paths)
    };
    git.run(repo, &args, Access::Write).map(drop)
}

/// Commit the staged changes with `message`. Hooks and signing run as
/// they would from a terminal.
pub fn commit(git: &Git, repo: &Path, message: &str) -> Result<(), Error> {
    git.run_with_input(
        repo,
        &git_args!("commit", "--quiet", "-F", "-"),
        Access::Write,
        Some(message.as_bytes()),
    )
    .map(drop)
}

/// Folders skipped by [`scan`]: dependency and build output trees.
const SKIP: [&str; 2] = ["node_modules", "target"];

/// Repositories under `root`, up to `depth` levels down, sorted. A
/// repository's own subfolders are not searched, nor hidden folders and
/// [`SKIP`]. Stops after `limit` results.
pub fn scan(root: &Path, depth: usize, limit: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_into(root, depth, limit, &mut found);
    found.sort();
    found
}

fn scan_into(dir: &Path, depth: usize, limit: usize, found: &mut Vec<PathBuf>) {
    if found.len() >= limit {
        return;
    }
    if dir.join(".git").exists() {
        found.push(dir.to_owned());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.starts_with('.') && !SKIP.contains(&name.as_ref())
        })
        .map(|entry| entry.path())
        .collect();
    children.sort();
    for child in children {
        scan_into(&child, depth - 1, limit, found);
    }
}
