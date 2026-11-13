//! Fetch, pull and push, with git's progress lines.

use super::command::{Access, Cancel, Error, Git};
use crate::git_args;
use std::path::Path;

/// One progress update, from a line such as
/// `Receiving objects:  45% (912/2026), 1.20 MiB | 2.00 MiB/s`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    /// `Receiving objects`, without a `remote: ` prefix.
    pub phase: String,
    pub percent: Option<u8>,
    /// The whole line, trimmed.
    pub line: String,
}

pub fn parse_progress(line: &str) -> Option<Progress> {
    let line = line.trim();
    let body = line.strip_prefix("remote: ").unwrap_or(line);
    let (phase, rest) = body.split_once(':')?;
    if phase.is_empty() || phase.contains("fatal") || phase.contains("error") {
        return None;
    }
    let percent = rest
        .trim_start()
        .split_once('%')
        .and_then(|(number, _)| number.trim().parse::<u8>().ok());
    Some(Progress {
        phase: phase.trim().to_owned(),
        percent,
        line: line.to_owned(),
    })
}

/// The repository's remotes, `origin` first.
pub fn remotes(git: &Git, repo: &Path) -> Result<Vec<String>, Error> {
    let output = git.run(repo, &git_args!("remote"), Access::Read)?;
    let mut remotes: Vec<String> = String::from_utf8_lossy(&output)
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    remotes.sort_by_key(|name| name != "origin");
    Ok(remotes)
}

/// `git fetch --prune` from every remote.
pub fn fetch(
    git: &Git,
    repo: &Path,
    cancel: &Cancel,
    on_progress: impl FnMut(Progress),
) -> Result<String, Error> {
    stream(
        git,
        repo,
        &git_args!("fetch", "--all", "--prune", "--progress"),
        cancel,
        on_progress,
    )
}

/// `git pull --ff-only`: never creates a merge commit. A diverged branch
/// fails with git's message.
pub fn pull(
    git: &Git,
    repo: &Path,
    cancel: &Cancel,
    on_progress: impl FnMut(Progress),
) -> Result<String, Error> {
    stream(
        git,
        repo,
        &git_args!("pull", "--ff-only", "--progress"),
        cancel,
        on_progress,
    )
}

/// `git push` of the current branch to its upstream, or, with
/// `set_upstream`, `git push -u <remote> <branch>`. Never forces.
pub fn push(
    git: &Git,
    repo: &Path,
    set_upstream: Option<(&str, &str)>,
    cancel: &Cancel,
    on_progress: impl FnMut(Progress),
) -> Result<String, Error> {
    match set_upstream {
        Some((remote, branch)) => stream(
            git,
            repo,
            &git_args!("push", "--progress", "-u", remote, branch),
            cancel,
            on_progress,
        ),
        None => stream(
            git,
            repo,
            &git_args!("push", "--progress"),
            cancel,
            on_progress,
        ),
    }
}

fn stream(
    git: &Git,
    repo: &Path,
    args: &[&std::ffi::OsStr],
    cancel: &Cancel,
    mut on_progress: impl FnMut(Progress),
) -> Result<String, Error> {
    git.run_streaming(repo, args, cancel, |line| {
        if let Some(progress) = parse_progress(line) {
            on_progress(progress);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines() {
        let progress =
            parse_progress("Receiving objects:  45% (912/2026), 1.20 MiB | 2.00 MiB/s").unwrap();
        assert_eq!(progress.phase, "Receiving objects");
        assert_eq!(progress.percent, Some(45));
        let remote = parse_progress("remote: Counting objects: 100% (5/5), done.").unwrap();
        assert_eq!(remote.phase, "Counting objects");
        assert_eq!(remote.percent, Some(100));
        let plain = parse_progress("From ../origin").map(|p| p.percent);
        assert_eq!(plain, None);
        assert_eq!(parse_progress("fatal: unable to access"), None);
        let no_percent = parse_progress("remote: Enumerating objects: 5, done.").unwrap();
        assert_eq!(no_percent.percent, None);
    }
}
