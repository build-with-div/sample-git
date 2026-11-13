//! `git status --porcelain=v2 -z --branch`: branch, upstream, ahead/behind
//! and the changed files.

use super::command::{Access, Error, Git};
use crate::git_args;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: BranchInfo,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchInfo {
    /// The commit HEAD points at; `None` before the first commit.
    pub oid: Option<String>,
    /// The branch name; `None` when HEAD is detached.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

/// One path with changes in the index, the working tree, or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    /// The path before a rename or copy.
    pub orig_path: Option<String>,
    /// The change between HEAD and the index.
    pub staged: Option<Change>,
    /// The change between the index and the working tree.
    pub unstaged: Option<Change>,
    /// Unmerged: both sides changed the file.
    pub conflicted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

impl Change {
    /// The one-letter code shown in the lists.
    pub fn letter(self) -> &'static str {
        match self {
            Change::Added => "A",
            Change::Modified => "M",
            Change::Deleted => "D",
            Change::Renamed => "R",
            Change::Copied => "C",
            Change::TypeChanged => "T",
            Change::Untracked => "?",
            Change::Conflicted => "U",
        }
    }

    fn from_code(code: u8) -> Option<Change> {
        match code {
            b'A' => Some(Change::Added),
            b'M' => Some(Change::Modified),
            b'D' => Some(Change::Deleted),
            b'R' => Some(Change::Renamed),
            b'C' => Some(Change::Copied),
            b'T' => Some(Change::TypeChanged),
            _ => None,
        }
    }
}

impl Status {
    /// Entries with staged changes, in git's (path) order.
    pub fn staged(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.staged.is_some())
    }

    /// Entries with unstaged changes, untracked files and conflicts.
    pub fn unstaged(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.unstaged.is_some())
    }

    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Read the status of `repo`.
pub fn status(git: &Git, repo: &Path) -> Result<Status, Error> {
    let output = git.run(
        repo,
        &git_args!(
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=normal"
        ),
        Access::Read,
    )?;
    parse(&String::from_utf8_lossy(&output))
}

/// Parse `git status --porcelain=v2 -z --branch` output.
pub fn parse(text: &str) -> Result<Status, Error> {
    let mut status = Status::default();
    let mut fields = text.split('\0');
    while let Some(record) = fields.next() {
        if record.is_empty() {
            continue;
        }
        let bad = || Error::Io(format!("unexpected status record: {record:?}"));
        if let Some(header) = record.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            let branch = &mut status.branch;
            match key {
                "branch.oid" if value != "(initial)" => branch.oid = Some(value.to_owned()),
                "branch.head" if value != "(detached)" => branch.head = Some(value.to_owned()),
                "branch.upstream" => branch.upstream = Some(value.to_owned()),
                "branch.ab" => {
                    let mut counts = value.split(' ');
                    branch.ahead = counts
                        .next()
                        .and_then(|ahead| ahead.trim_start_matches('+').parse().ok())
                        .ok_or_else(bad)?;
                    branch.behind = counts
                        .next()
                        .and_then(|behind| behind.trim_start_matches('-').parse().ok())
                        .ok_or_else(bad)?;
                }
                _ => {}
            }
            continue;
        }
        let kind = record.as_bytes()[0];
        match kind {
            b'1' | b'2' => {
                // `1 XY sub mH mI mW hH hI path`, and for 2 also `Xscore`
                // before the path, followed by the original path as the next
                // NUL-separated field.
                let parts = if kind == b'1' { 9 } else { 10 };
                let columns: Vec<&str> = record.splitn(parts, ' ').collect();
                if columns.len() != parts {
                    return Err(bad());
                }
                let xy = columns[1].as_bytes();
                let orig_path = if kind == b'2' {
                    Some(fields.next().ok_or_else(bad)?.to_owned())
                } else {
                    None
                };
                status.entries.push(Entry {
                    path: columns[parts - 1].to_owned(),
                    orig_path,
                    staged: Change::from_code(xy[0]),
                    unstaged: Change::from_code(xy[1]),
                    conflicted: false,
                });
            }
            b'u' => {
                let columns: Vec<&str> = record.splitn(11, ' ').collect();
                if columns.len() != 11 {
                    return Err(bad());
                }
                status.entries.push(Entry {
                    path: columns[10].to_owned(),
                    orig_path: None,
                    staged: None,
                    unstaged: Some(Change::Conflicted),
                    conflicted: true,
                });
            }
            b'?' => status.entries.push(Entry {
                path: record[2..].to_owned(),
                orig_path: None,
                staged: None,
                unstaged: Some(Change::Untracked),
                conflicted: false,
            }),
            // Ignored files are not requested.
            b'!' => {}
            _ => return Err(bad()),
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_branch_with_an_upstream() {
        let text = "# branch.oid 1234abcd\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0";
        let status = parse(text).unwrap();
        assert_eq!(
            status.branch,
            BranchInfo {
                oid: Some("1234abcd".into()),
                head: Some("main".into()),
                upstream: Some("origin/main".into()),
                ahead: 2,
                behind: 1,
            }
        );
        assert!(status.is_clean());
    }

    #[test]
    fn an_unborn_branch_and_a_detached_head() {
        let unborn = parse("# branch.oid (initial)\0# branch.head main\0").unwrap();
        assert_eq!(unborn.branch.oid, None);
        assert_eq!(unborn.branch.head.as_deref(), Some("main"));
        let detached = parse("# branch.oid abc\0# branch.head (detached)\0").unwrap();
        assert_eq!(detached.branch.head, None);
    }

    #[test]
    fn changes_renames_conflicts_and_untracked_files() {
        let text = concat!(
            "# branch.oid abc\0# branch.head main\0",
            "1 M. N... 100644 100644 100644 aaa bbb src/lib.rs\0",
            "1 .M N... 100644 100644 100644 aaa aaa README with spaces.md\0",
            "1 MD N... 100644 100644 000000 aaa bbb gone.txt\0",
            "2 R. N... 100644 100644 100644 aaa aaa R100 new name.rs\0old name.rs\0",
            "u UU N... 100644 100644 100644 100644 a b c both.txt\0",
            "? notes/ünïcode.txt\0",
        );
        let status = parse(text).unwrap();
        let entries = &status.entries;
        assert_eq!(entries.len(), 6);
        assert_eq!(entries[0].staged, Some(Change::Modified));
        assert_eq!(entries[0].unstaged, None);
        assert_eq!(entries[1].path, "README with spaces.md");
        assert_eq!(entries[1].unstaged, Some(Change::Modified));
        assert_eq!(entries[2].staged, Some(Change::Modified));
        assert_eq!(entries[2].unstaged, Some(Change::Deleted));
        assert_eq!(entries[3].path, "new name.rs");
        assert_eq!(entries[3].orig_path.as_deref(), Some("old name.rs"));
        assert_eq!(entries[3].staged, Some(Change::Renamed));
        assert!(entries[4].conflicted);
        assert_eq!(entries[5].path, "notes/ünïcode.txt");
        assert_eq!(entries[5].unstaged, Some(Change::Untracked));
        assert_eq!(status.staged().count(), 3);
        assert_eq!(status.unstaged().count(), 4);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("1 M.\0").is_err());
        assert!(parse("zzz\0").is_err());
    }
}
