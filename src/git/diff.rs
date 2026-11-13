//! Unified diffs: running `git diff` for one file and parsing the output.

use super::command::{Access, Error, Git};
use std::ffi::OsStr;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDiff {
    /// The path after the change (`+++ b/…`), or before it for a deletion.
    pub path: String,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The `@@ -a,b +c,d @@ context` line.
    pub header: String,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    /// The line number in the old file, for context and removed lines.
    pub old: Option<u32>,
    /// The line number in the new file, for context and added lines.
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
    /// `\ No newline at end of file`.
    NoNewline,
}

impl FileDiff {
    /// The number of lines in all hunks, headers included.
    pub fn line_count(&self) -> usize {
        self.hunks.iter().map(|hunk| hunk.lines.len() + 1).sum()
    }

    pub fn is_empty(&self) -> bool {
        !self.binary && self.hunks.is_empty()
    }
}

/// Which side of a file to diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Working tree against the index.
    Unstaged,
    /// Index against HEAD.
    Staged,
    /// A file git does not track yet, shown as added.
    Untracked,
    /// The change a commit made, against its first parent.
    Commit(String),
}

const COMMON: [&str; 4] = [
    "-c",
    "core.quotepath=false",
    "--no-pager",
    "--literal-pathspecs",
];

/// The diff of `path` in `repo`.
pub fn diff_file(git: &Git, repo: &Path, path: &str, source: &Source) -> Result<FileDiff, Error> {
    let mut args: Vec<&OsStr> = COMMON.iter().map(OsStr::new).collect();
    let tail = ["--no-color", "--no-ext-diff"];
    match source {
        Source::Unstaged => args.push(OsStr::new("diff")),
        Source::Staged => args.extend(["diff", "--cached"].map(OsStr::new)),
        Source::Untracked => args.extend(["diff", "--no-index"].map(OsStr::new)),
        Source::Commit(hash) => {
            args.extend(["show", "--format=", "-m", "--first-parent"].map(OsStr::new));
            args.push(OsStr::new(hash));
        }
    }
    args.extend(tail.map(OsStr::new));
    args.push(OsStr::new("--"));
    if *source == Source::Untracked {
        args.push(OsStr::new("/dev/null"));
    }
    args.push(OsStr::new(path));
    let output = git.output(Some(repo), &args, Access::Read, None)?;
    // `--no-index` exits with 1 when the files differ, as `diff` does.
    let ok = output.code == Some(0) || (*source == Source::Untracked && output.code == Some(1));
    if !ok {
        return Err(output.failure(&args));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = parse(&text)?;
    Ok(match files.len() {
        0 => FileDiff {
            path: path.to_owned(),
            ..FileDiff::default()
        },
        _ => files.swap_remove(0),
    })
}

/// Parse `git diff` output into one [`FileDiff`] per file.
pub fn parse(text: &str) -> Result<Vec<FileDiff>, Error> {
    let mut files: Vec<FileDiff> = Vec::new();
    // Lines left in the current hunk, old and new side.
    let mut remaining = (0_u32, 0_u32);
    let mut next = (0_u32, 0_u32);
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let in_hunk = remaining.0 > 0 || remaining.1 > 0;
        if in_hunk {
            let file = files.last_mut().expect("a hunk belongs to a file");
            let hunk = file.hunks.last_mut().expect("lines belong to a hunk");
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (LineKind::Added, &line[1..]),
                Some(b'-') => (LineKind::Removed, &line[1..]),
                Some(b' ') => (LineKind::Context, &line[1..]),
                // An empty context line whose space was stripped.
                None => (LineKind::Context, ""),
                Some(b'\\') => (LineKind::NoNewline, line),
                _ => return Err(Error::Io(format!("unexpected diff line: {line:?}"))),
            };
            let (old, new) = match kind {
                LineKind::Context => {
                    remaining.0 = remaining.0.saturating_sub(1);
                    remaining.1 = remaining.1.saturating_sub(1);
                    next.0 += 1;
                    next.1 += 1;
                    (Some(next.0 - 1), Some(next.1 - 1))
                }
                LineKind::Added => {
                    remaining.1 = remaining.1.saturating_sub(1);
                    next.1 += 1;
                    (None, Some(next.1 - 1))
                }
                LineKind::Removed => {
                    remaining.0 = remaining.0.saturating_sub(1);
                    next.0 += 1;
                    (Some(next.0 - 1), None)
                }
                LineKind::NoNewline => (None, None),
            };
            hunk.lines.push(Line {
                kind,
                old,
                new,
                text: text.to_owned(),
            });
            continue;
        }
        if line.starts_with("diff --git ") || line.starts_with("diff --cc ") {
            files.push(FileDiff::default());
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(file) = files.last_mut() {
                if path != "/dev/null" {
                    file.path = strip_side(path).to_owned();
                }
            }
        } else if let Some(path) = line.strip_prefix("--- ") {
            if let Some(file) = files.last_mut() {
                if path != "/dev/null" && file.path.is_empty() {
                    file.path = strip_side(path).to_owned();
                }
            }
        } else if line.starts_with("Binary files ") && line.ends_with(" differ") {
            if let Some(file) = files.last_mut() {
                file.binary = true;
            }
        } else if line.starts_with("@@") {
            let (old, new) = parse_hunk_header(line)
                .ok_or_else(|| Error::Io(format!("unexpected hunk header: {line:?}")))?;
            let file = files
                .last_mut()
                .ok_or_else(|| Error::Io("a hunk before any file".into()))?;
            remaining = (old.1, new.1);
            next = (old.0, new.0);
            file.hunks.push(Hunk {
                header: line.to_owned(),
                lines: Vec::new(),
            });
        } else if line.starts_with('\\') {
            // `\ No newline at end of file` right after a hunk's last line.
            if let Some(hunk) = files.last_mut().and_then(|file| file.hunks.last_mut()) {
                hunk.lines.push(Line {
                    kind: LineKind::NoNewline,
                    old: None,
                    new: None,
                    text: line.to_owned(),
                });
            }
        }
    }
    Ok(files)
}

fn strip_side(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

/// `@@ -3,7 +3,8 @@ fn main()` → `((3, 7), (3, 8))`. A missing count is 1.
fn parse_hunk_header(line: &str) -> Option<((u32, u32), (u32, u32))> {
    let mut parts = line.split(' ');
    parts.next()?;
    let old = parse_range(parts.next()?.strip_prefix('-')?)?;
    let new = parse_range(parts.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

fn parse_range(range: &str) -> Option<(u32, u32)> {
    match range.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((range.parse().ok()?, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODIFIED: &str = "diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,4 @@
 use std::io;
+use std::fs;

 fn main() {
--- old comment
@@ -10 +11 @@ fn tail()
-x
+y
\\ No newline at end of file
";

    #[test]
    fn a_modified_file_with_two_hunks() {
        let files = parse(MODIFIED).unwrap();
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "src/lib.rs");
        assert_eq!(file.hunks.len(), 2);
        let first = &file.hunks[0].lines;
        assert_eq!(first.len(), 5);
        assert_eq!(first[0].old, Some(1));
        assert_eq!(first[0].new, Some(1));
        assert_eq!(first[1].kind, LineKind::Added);
        assert_eq!(first[1].new, Some(2));
        assert_eq!(first[1].old, None);
        assert_eq!(first[2].text, "");
        // A removed line whose text starts with "-- " is not a file header.
        assert_eq!(first[4].kind, LineKind::Removed);
        assert_eq!(first[4].text, "-- old comment");
        assert_eq!(first[4].old, Some(4));
        let second = &file.hunks[1];
        assert_eq!(second.header, "@@ -10 +11 @@ fn tail()");
        assert_eq!(second.lines[0].old, Some(10));
        assert_eq!(second.lines[1].new, Some(11));
        assert_eq!(second.lines[2].kind, LineKind::NoNewline);
    }

    #[test]
    fn new_deleted_and_binary_files() {
        let text = "diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+one
+two
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone
diff --git a/logo.png b/logo.png
Binary files a/logo.png and b/logo.png differ
";
        let files = parse(text).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[0].hunks[0].lines[1].new, Some(2));
        assert_eq!(files[1].path, "old.txt");
        assert_eq!(files[1].hunks[0].lines[0].kind, LineKind::Removed);
        assert!(files[2].binary);
        assert_eq!(files[2].line_count(), 0);
    }

    #[test]
    fn crlf_lines_keep_their_text() {
        let text = "diff --git a/w.txt b/w.txt\r\n--- a/w.txt\r\n+++ b/w.txt\r\n@@ -1 +1 @@\r\n-a\r\n+b\r\n";
        let files = parse(text).unwrap();
        assert_eq!(files[0].hunks[0].lines[1].text, "b");
    }
}
