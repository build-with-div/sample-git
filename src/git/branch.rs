//! Local and remote branches: list, switch, create and delete.

use super::command::{Access, Error, Git};
use crate::git_args;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    /// `main`, or `origin/main` for a remote branch.
    pub name: String,
    pub remote: bool,
    /// HEAD is on this branch.
    pub current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch no longer exists.
    pub gone: bool,
    pub short_hash: String,
    pub subject: String,
    /// Committer time of the tip, seconds since the Unix epoch.
    pub time: i64,
}

const FORMAT: &str = "--format=%(refname)%00%(refname:short)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(objectname:short)%00%(committerdate:unix)%00%(subject)";

/// Local branches by name, then remote branches by name.
pub fn branches(git: &Git, repo: &Path) -> Result<Vec<Branch>, Error> {
    let output = git.run(
        repo,
        &git_args!(
            "for-each-ref",
            FORMAT,
            "--sort=refname",
            "refs/heads",
            "refs/remotes"
        ),
        Access::Read,
    )?;
    parse(&String::from_utf8_lossy(&output))
}

pub fn parse(text: &str) -> Result<Vec<Branch>, Error> {
    let mut branches = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.splitn(8, '\0').collect();
        let bad = || Error::Io(format!("unexpected ref record: {line:?}"));
        if fields.len() != 8 {
            return Err(bad());
        }
        let refname = fields[0];
        let remote = refname.starts_with("refs/remotes/");
        // `refs/remotes/origin/HEAD` is an alias, not a branch.
        if remote && refname.ends_with("/HEAD") {
            continue;
        }
        let (ahead, behind, gone) = parse_track(fields[4]);
        branches.push(Branch {
            name: fields[1].to_owned(),
            remote,
            current: fields[2] == "*",
            upstream: (!fields[3].is_empty()).then(|| fields[3].to_owned()),
            ahead,
            behind,
            gone,
            short_hash: fields[5].to_owned(),
            time: fields[6].parse().unwrap_or(0),
            subject: fields[7].to_owned(),
        });
    }
    branches.sort_by_key(|branch| branch.remote);
    Ok(branches)
}

/// `ahead 2, behind 1`, `ahead 3`, `gone` or empty.
fn parse_track(track: &str) -> (u32, u32, bool) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.split(", ") {
        if let Some(count) = part.strip_prefix("ahead ") {
            ahead = count.parse().unwrap_or(0);
        } else if let Some(count) = part.strip_prefix("behind ") {
            behind = count.parse().unwrap_or(0);
        }
    }
    (ahead, behind, track == "gone")
}

/// Whether `name` is a valid new branch name.
pub fn is_valid_name(git: &Git, name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && git
            .output(
                None,
                &git_args!("check-ref-format", "--branch", name),
                Access::Read,
                None,
            )
            .is_ok_and(|output| output.code == Some(0))
}

/// Switch to the local branch `name`. git refuses if local changes would be
/// overwritten; the error says so.
pub fn switch(git: &Git, repo: &Path, name: &str) -> Result<(), Error> {
    git.run(
        repo,
        &git_args!("switch", "--no-guess", name),
        Access::Write,
    )
    .map(drop)
}

/// Create the branch `name` at HEAD and switch to it.
pub fn create(git: &Git, repo: &Path, name: &str) -> Result<(), Error> {
    git.run(repo, &git_args!("switch", "-c", name), Access::Write)
        .map(drop)
}

/// Create a local branch tracking the remote branch `remote_name` (such as
/// `origin/feature`) and switch to it.
pub fn track(git: &Git, repo: &Path, remote_name: &str) -> Result<(), Error> {
    git.run(
        repo,
        &git_args!("switch", "--track", remote_name),
        Access::Write,
    )
    .map(drop)
}

/// Delete the local branch `name` if it is merged (`git branch -d`).
pub fn delete(git: &Git, repo: &Path, name: &str) -> Result<(), Error> {
    git.run(repo, &git_args!("branch", "-d", "--", name), Access::Write)
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_and_remote_refs() {
        let rows = [
            [
                "refs/heads/feature",
                "feature",
                " ",
                "origin/feature",
                "gone",
                "abc1234",
                "1700000000",
                "Add a thing",
            ],
            [
                "refs/heads/main",
                "main",
                "*",
                "origin/main",
                "ahead 2, behind 1",
                "def5678",
                "1700000100",
                "Fix: a bug",
            ],
            [
                "refs/remotes/origin/HEAD",
                "origin",
                " ",
                "",
                "",
                "def5678",
                "1700000100",
                "Fix: a bug",
            ],
            [
                "refs/remotes/origin/main",
                "origin/main",
                " ",
                "",
                "",
                "aaa1111",
                "1690000000",
                "Older",
            ],
        ];
        let text: String = rows.iter().map(|row| row.join("\0") + "\n").collect();
        let branches = parse(&text).unwrap();
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "feature");
        assert!(branches[0].gone);
        assert!(!branches[0].current);
        assert_eq!(branches[1].name, "main");
        assert!(branches[1].current);
        assert_eq!((branches[1].ahead, branches[1].behind), (2, 1));
        assert_eq!(branches[1].subject, "Fix: a bug");
        assert!(branches[2].remote);
        assert_eq!(branches[2].name, "origin/main");
        assert_eq!(branches[2].upstream, None);
    }

    #[test]
    fn tracking_text() {
        assert_eq!(parse_track(""), (0, 0, false));
        assert_eq!(parse_track("ahead 3"), (3, 0, false));
        assert_eq!(parse_track("behind 4"), (0, 4, false));
        assert_eq!(parse_track("gone"), (0, 0, true));
    }
}
