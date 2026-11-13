//! History: `git log` pages and one commit's details.

use super::command::{Access, Error, Git};
use super::status::Change;
use crate::git_args;
use std::ffi::OsStr;
use std::path::Path;

/// One row of the history list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    /// Author time, seconds since the Unix epoch.
    pub time: i64,
    pub subject: String,
}

/// A commit with its full message and the files it changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitDetails {
    pub commit: Commit,
    pub committer: String,
    pub commit_time: i64,
    /// The author date in local time, `2026-11-13 14:05`.
    pub date: String,
    pub message: String,
    pub files: Vec<ChangedFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub change: Change,
    pub path: String,
    pub orig_path: Option<String>,
}

const UNIT: char = '\u{1f}';
const FORMAT: &str = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s";

/// Up to `count` commits of HEAD, skipping the first `skip`. An unborn
/// branch has none.
pub fn log(git: &Git, repo: &Path, skip: usize, count: usize) -> Result<Vec<Commit>, Error> {
    let skip = format!("--skip={skip}");
    let count = format!("--max-count={count}");
    let args = git_args!("log", "-z", FORMAT, &skip, &count, "HEAD", "--");
    let output = git.output(Some(repo), &args, Access::Read, None)?;
    if output.code != Some(0) {
        // Before the first commit HEAD does not resolve: no history.
        let head = git.output(
            Some(repo),
            &git_args!("rev-parse", "--verify", "--quiet", "HEAD"),
            Access::Read,
            None,
        )?;
        if head.code != Some(0) {
            return Ok(Vec::new());
        }
        return Err(output.failure(&args));
    }
    parse_log(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_log(text: &str) -> Result<Vec<Commit>, Error> {
    text.split('\0')
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .map(parse_commit)
        .collect()
}

fn parse_commit(record: &str) -> Result<Commit, Error> {
    let fields: Vec<&str> = record.splitn(7, UNIT).collect();
    let bad = || Error::Io(format!("unexpected log record: {record:?}"));
    if fields.len() != 7 {
        return Err(bad());
    }
    Ok(Commit {
        hash: fields[0].to_owned(),
        short: fields[1].to_owned(),
        parents: fields[2].split_whitespace().map(str::to_owned).collect(),
        author: fields[3].to_owned(),
        email: fields[4].to_owned(),
        time: fields[5].parse().map_err(|_| bad())?,
        subject: fields[6].to_owned(),
    })
}

/// Details of `hash`: message, committer and changed files (against the
/// first parent; a root commit against the empty tree).
pub fn details(git: &Git, repo: &Path, hash: &str) -> Result<CommitDetails, Error> {
    let format = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%cn%x1f%ct%x1f%ad%x1f%B";
    let output = git.run(
        repo,
        &git_args!(
            "show",
            "-s",
            "--no-color",
            "--date=format-local:%Y-%m-%d %H:%M",
            format,
            hash,
            "--"
        ),
        Access::Read,
    )?;
    let text = String::from_utf8_lossy(&output);
    let fields: Vec<&str> = text.splitn(11, UNIT).collect();
    let bad = || Error::Io(format!("unexpected commit record: {text:?}"));
    if fields.len() != 11 {
        return Err(bad());
    }
    let commit = parse_commit(&fields[..7].join(&UNIT.to_string()))?;
    let files = git.run(
        repo,
        &[
            OsStr::new("-c"),
            OsStr::new("core.quotepath=false"),
            OsStr::new("show"),
            OsStr::new("--format="),
            OsStr::new("-z"),
            OsStr::new("--name-status"),
            OsStr::new("-M"),
            OsStr::new("-m"),
            OsStr::new("--first-parent"),
            OsStr::new(hash),
            OsStr::new("--"),
        ],
        Access::Read,
    )?;
    Ok(CommitDetails {
        commit,
        committer: fields[7].to_owned(),
        commit_time: fields[8].parse().map_err(|_| bad())?,
        date: fields[9].to_owned(),
        message: fields[10].trim_end().to_owned(),
        files: parse_name_status(&String::from_utf8_lossy(&files))?,
    })
}

/// `--name-status -z`: `M\0path\0`, `R100\0old\0new\0`.
pub fn parse_name_status(text: &str) -> Result<Vec<ChangedFile>, Error> {
    let mut files = Vec::new();
    let mut fields = text.split('\0').map(|field| field.trim_start_matches('\n'));
    while let Some(code) = fields.next() {
        if code.is_empty() {
            continue;
        }
        let bad = || Error::Io(format!("unexpected name-status record: {code:?}"));
        let change = match code.as_bytes()[0] {
            b'A' => Change::Added,
            b'M' => Change::Modified,
            b'D' => Change::Deleted,
            b'R' => Change::Renamed,
            b'C' => Change::Copied,
            b'T' => Change::TypeChanged,
            b'U' => Change::Conflicted,
            _ => return Err(bad()),
        };
        let first = fields.next().ok_or_else(bad)?.to_owned();
        let file = if matches!(change, Change::Renamed | Change::Copied) {
            ChangedFile {
                change,
                path: fields.next().ok_or_else(bad)?.to_owned(),
                orig_path: Some(first),
            }
        } else {
            ChangedFile {
                change,
                path: first,
                orig_path: None,
            }
        };
        files.push(file);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_records() {
        let text = "aaaa\u{1f}aa\u{1f}bbbb cccc\u{1f}Ana Lima\u{1f}ana@example.com\u{1f}1700000000\u{1f}Merge: things\0\nbbbb\u{1f}bb\u{1f}\u{1f}Bo\u{1f}bo@example.com\u{1f}1690000000\u{1f}Initial commit\0";
        let commits = parse_log(text).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, vec!["bbbb", "cccc"]);
        assert_eq!(commits[0].subject, "Merge: things");
        assert_eq!(commits[1].parents, Vec::<String>::new());
        assert_eq!(commits[1].time, 1_690_000_000);
    }

    #[test]
    fn name_status_with_a_rename() {
        let files = parse_name_status("M\0src/a.rs\0R087\0old.rs\0new.rs\0A\0ünï.txt\0").unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[1].change, Change::Renamed);
        assert_eq!(files[1].orig_path.as_deref(), Some("old.rs"));
        assert_eq!(files[1].path, "new.rs");
        assert_eq!(files[2].path, "ünï.txt");
    }
}
