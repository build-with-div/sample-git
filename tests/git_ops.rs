//! The git layer against real git and scratch repositories.

mod common;

use common::{write, Fixture};
use sample_git::git::diff::{self, LineKind, Source};
use sample_git::git::status::{self, Change};
use sample_git::git::{branch, log, remote, repo, Cancel, Error, MIN_VERSION};
use std::path::Path;

fn paths(status: &status::Status, staged: bool) -> Vec<String> {
    let entries: Vec<_> = if staged {
        status.staged().collect()
    } else {
        status.unstaged().collect()
    };
    entries
        .into_iter()
        .map(|entry| entry.path.clone())
        .collect()
}

#[test]
fn git_is_new_enough() {
    let fixture = Fixture::new("version");
    assert!(fixture.git.version().unwrap() >= MIN_VERSION);
}

#[test]
fn toplevel_finds_the_repository_and_rejects_other_folders() {
    let fixture = Fixture::new("toplevel");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "src/main.rs", "fn main() {}\n");
    let found = repo::toplevel(&fixture.git, &repo.join("src"))
        .unwrap()
        .unwrap();
    assert!(same_path(&found, &repo), "{found:?} vs {repo:?}");
    let plain = fixture.path("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(repo::toplevel(&fixture.git, &plain).unwrap(), None);
}

fn same_path(a: &Path, b: &Path) -> bool {
    let normal = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
    normal(a) == normal(b)
}

#[test]
fn status_stage_unstage_and_commit() {
    let fixture = Fixture::new("status");
    let repo = fixture.repo_with_commit("app");
    let git = &fixture.git;

    write(&repo, "README.md", "# Fixture\n\nChanged line.\n");
    write(&repo, "new file.txt", "hello\n");
    let status = status::status(git, &repo).unwrap();
    assert_eq!(status.branch.head.as_deref(), Some("main"));
    assert_eq!(paths(&status, false), vec!["README.md", "new file.txt"]);
    assert_eq!(status.entries[1].unstaged, Some(Change::Untracked));

    repo::stage(git, &repo, &["new file.txt".into()]).unwrap();
    let status = status::status(git, &repo).unwrap();
    assert_eq!(paths(&status, true), vec!["new file.txt"]);
    assert_eq!(status.entries[1].staged, Some(Change::Added));

    repo::unstage(git, &repo, &["new file.txt".into()]).unwrap();
    assert_eq!(
        paths(&status::status(git, &repo).unwrap(), true),
        Vec::<String>::new()
    );

    repo::stage_all(git, &repo).unwrap();
    repo::commit(git, &repo, "Change the readme\n\nAnd add a file.").unwrap();
    let status = status::status(git, &repo).unwrap();
    assert!(status.is_clean());

    let commits = log::log(git, &repo, 0, 10).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].subject, "Change the readme");
    assert_eq!(commits[0].author, "Fixture Author");
    let details = log::details(git, &repo, &commits[0].hash).unwrap();
    // `2026-11-13 14:05`, in local time.
    assert_eq!(details.date.len(), 16, "{}", details.date);
    assert_eq!(details.message, "Change the readme\n\nAnd add a file.");
    let files: Vec<_> = details
        .files
        .iter()
        .map(|f| (f.change, f.path.as_str()))
        .collect();
    assert_eq!(
        files,
        vec![
            (Change::Modified, "README.md"),
            (Change::Added, "new file.txt")
        ]
    );
    // The root commit lists its files against the empty tree.
    let root = log::details(git, &repo, &commits[1].hash).unwrap();
    assert_eq!(root.files.len(), 1);
    assert_eq!(root.commit.parents.len(), 0);
}

#[test]
fn an_unborn_branch_has_no_history_and_can_unstage() {
    let fixture = Fixture::new("unborn");
    let repo = fixture.init("empty");
    let git = &fixture.git;
    assert_eq!(log::log(git, &repo, 0, 10).unwrap(), Vec::new());
    write(&repo, "a.txt", "a\n");
    repo::stage_all(git, &repo).unwrap();
    let status = status::status(git, &repo).unwrap();
    assert_eq!(status.branch.oid, None);
    assert_eq!(paths(&status, true), vec!["a.txt"]);
    repo::unstage_all(git, &repo).unwrap();
    assert_eq!(
        paths(&status::status(git, &repo).unwrap(), false),
        vec!["a.txt"]
    );
}

#[test]
fn diffs_of_unstaged_staged_untracked_and_committed_files() {
    let fixture = Fixture::new("diff");
    let repo = fixture.repo_with_commit("app");
    let git = &fixture.git;

    write(
        &repo,
        "README.md",
        "# Fixture\n\nFirst line.\nSecond line.\n",
    );
    let unstaged = diff::diff_file(git, &repo, "README.md", &Source::Unstaged).unwrap();
    assert_eq!(unstaged.path, "README.md");
    let added: Vec<_> = unstaged.hunks[0]
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::Added)
        .map(|line| (line.new, line.text.as_str()))
        .collect();
    assert_eq!(added, vec![(Some(4), "Second line.")]);

    repo::stage_all(git, &repo).unwrap();
    let staged = diff::diff_file(git, &repo, "README.md", &Source::Staged).unwrap();
    assert_eq!(staged.hunks, unstaged.hunks);
    let nothing = diff::diff_file(git, &repo, "README.md", &Source::Unstaged).unwrap();
    assert!(nothing.is_empty());

    write(&repo, "notes/new [1].txt", "one\ntwo\n");
    let untracked = diff::diff_file(git, &repo, "notes/new [1].txt", &Source::Untracked).unwrap();
    let lines: Vec<_> = untracked.hunks[0].lines.iter().map(|l| l.kind).collect();
    assert_eq!(lines, vec![LineKind::Added, LineKind::Added]);

    std::fs::write(repo.join("logo.bin"), [0_u8, 1, 2, 0, 255]).unwrap();
    let binary = diff::diff_file(git, &repo, "logo.bin", &Source::Untracked).unwrap();
    assert!(binary.binary);

    repo::commit(git, &repo, "Add a line").unwrap();
    let head = log::log(git, &repo, 0, 1).unwrap().remove(0);
    let committed = diff::diff_file(git, &repo, "README.md", &Source::Commit(head.hash)).unwrap();
    assert_eq!(committed.hunks, unstaged.hunks);
}

#[test]
fn discard_restores_tracked_files_and_deletes_untracked_ones() {
    let fixture = Fixture::new("discard");
    let repo = fixture.repo_with_commit("app");
    let git = &fixture.git;
    write(&repo, "README.md", "scribbles\n");
    write(&repo, "junk.txt", "junk\n");
    repo::discard(git, &repo, "README.md", false).unwrap();
    repo::discard(git, &repo, "junk.txt", true).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.join("README.md")).unwrap(),
        "# Fixture\n\nFirst line.\n"
    );
    assert!(!repo.join("junk.txt").exists());
    assert!(status::status(git, &repo).unwrap().is_clean());
}

#[test]
fn branches_create_switch_and_delete() {
    let fixture = Fixture::new("branch");
    let repo = fixture.repo_with_commit("app");
    let git = &fixture.git;
    assert!(branch::is_valid_name(git, "feature/one"));
    assert!(!branch::is_valid_name(git, "bad name"));
    assert!(!branch::is_valid_name(git, "-x"));

    branch::create(git, &repo, "feature").unwrap();
    let names: Vec<_> = branch::branches(git, &repo)
        .unwrap()
        .into_iter()
        .map(|b| (b.name, b.current))
        .collect();
    assert_eq!(
        names,
        vec![("feature".into(), true), ("main".into(), false)]
    );

    branch::switch(git, &repo, "main").unwrap();
    // A merged branch deletes; deleting the current branch fails.
    branch::delete(git, &repo, "feature").unwrap();
    let error = branch::delete(git, &repo, "main").unwrap_err();
    assert!(matches!(error, Error::Failed { .. }), "{error:?}");

    // An unmerged branch is refused by `branch -d`.
    branch::create(git, &repo, "wip").unwrap();
    write(&repo, "wip.txt", "wip\n");
    repo::stage_all(git, &repo).unwrap();
    repo::commit(git, &repo, "WIP").unwrap();
    branch::switch(git, &repo, "main").unwrap();
    let error = branch::delete(git, &repo, "wip").unwrap_err();
    assert!(error.summary().contains("not fully merged"), "{error:?}");
}

#[test]
fn fetch_pull_and_push_through_a_local_origin() {
    let fixture = Fixture::new("remote");
    let (origin, clone) = fixture.clone_with_origin("app");
    let git = &fixture.git;
    assert_eq!(remote::remotes(git, &clone).unwrap(), vec!["origin"]);

    // Someone else pushes to origin.
    let other = fixture.path("other");
    fixture.git(
        &fixture.dir,
        &[
            "clone",
            "--quiet",
            &origin.to_string_lossy(),
            &other.to_string_lossy(),
        ],
    );
    write(&other, "theirs.txt", "theirs\n");
    fixture.git(&other, &["add", "-A"]);
    fixture.git(&other, &["commit", "--quiet", "-m", "Theirs"]);
    fixture.git(&other, &["push", "--quiet"]);

    let cancel = Cancel::new();
    remote::fetch(git, &clone, &cancel, |_| {}).unwrap();
    let status = status::status(git, &clone).unwrap();
    assert_eq!((status.branch.ahead, status.branch.behind), (0, 1));

    remote::pull(git, &clone, &cancel, |_| {}).unwrap();
    assert!(clone.join("theirs.txt").exists());

    write(&clone, "mine.txt", "mine\n");
    repo::stage_all(git, &clone).unwrap();
    repo::commit(git, &clone, "Mine").unwrap();
    let mut phases = Vec::new();
    remote::push(git, &clone, None, &cancel, |p| phases.push(p.phase)).unwrap();
    let status = status::status(git, &clone).unwrap();
    assert_eq!((status.branch.ahead, status.branch.behind), (0, 0));
    assert_eq!(
        fixture
            .git(&origin, &["log", "-1", "--format=%s", "main"])
            .trim(),
        "Mine"
    );

    // A new branch has no upstream until it is pushed with -u.
    branch::create(git, &clone, "topic").unwrap();
    assert_eq!(status::status(git, &clone).unwrap().branch.upstream, None);
    remote::push(git, &clone, Some(("origin", "topic")), &cancel, |_| {}).unwrap();
    assert_eq!(
        status::status(git, &clone)
            .unwrap()
            .branch
            .upstream
            .as_deref(),
        Some("origin/topic")
    );
}

#[test]
fn pull_refuses_to_merge_diverged_branches() {
    let fixture = Fixture::new("diverged");
    let (origin, clone) = fixture.clone_with_origin("app");
    let git = &fixture.git;
    let other = fixture.path("other");
    fixture.git(
        &fixture.dir,
        &[
            "clone",
            "--quiet",
            &origin.to_string_lossy(),
            &other.to_string_lossy(),
        ],
    );
    write(&other, "theirs.txt", "theirs\n");
    fixture.git(&other, &["add", "-A"]);
    fixture.git(&other, &["commit", "--quiet", "-m", "Theirs"]);
    fixture.git(&other, &["push", "--quiet"]);
    write(&clone, "mine.txt", "mine\n");
    repo::stage_all(git, &clone).unwrap();
    repo::commit(git, &clone, "Mine").unwrap();

    let error = remote::pull(git, &clone, &Cancel::new(), |_| {}).unwrap_err();
    assert!(matches!(error, Error::Failed { .. }), "{error:?}");
    // No merge commit was made.
    assert_eq!(log::log(git, &clone, 0, 10).unwrap()[0].subject, "Mine");
}

#[test]
fn a_cancelled_command_reports_cancelled() {
    let fixture = Fixture::new("cancel");
    let (_, clone) = fixture.clone_with_origin("app");
    let cancel = Cancel::new();
    cancel.cancel();
    // Cancelled before it starts, the command does not run.
    let result = remote::fetch(&fixture.git, &clone, &cancel, |_| {});
    assert_eq!(result, Err(Error::Cancelled));
}

#[test]
fn scan_finds_repositories_and_skips_dependency_folders() {
    let fixture = Fixture::new("scan");
    fixture.init("work/a");
    fixture.init("work/nested/b");
    fixture.init("work/a/inner");
    fixture.init("work/node_modules/dep");
    fixture.init("work/.hidden/c");
    fixture.init("work/one/two/three/too-deep");
    let found = repo::scan(&fixture.path("work"), 3, 100);
    let names: Vec<_> = found
        .iter()
        .map(|p| {
            p.strip_prefix(fixture.path("work"))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(names, vec!["a", "nested/b"]);
}
