//! The History tab, headless, against fixture repositories.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::TestApp;
use sample_git::actions::{self, HISTORY_PAGE};
use sample_git::git::{log, Access};
use sample_git::state::Loadable;
use sample_git::{ui, State, Tab};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn open(fixture: &Fixture, repo: PathBuf) -> (State, TestApp) {
    let state = State::with(fixture.git.clone(), None);
    let mut ui = TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state));
    actions::add_repo(state, repo);
    ui.run_until_idle();
    let history = ui.by_text("History");
    ui.click(&history);
    assert_eq!(state.tab.get(), Tab::History.value());
    (state, ui)
}

fn text(ui: &TestApp, id: &str) -> String {
    ui.by_id(id).text_content().unwrap()
}

fn shorts(fixture: &Fixture, repo: &Path) -> Vec<String> {
    log::log(&fixture.git, repo, 0, 10)
        .unwrap()
        .into_iter()
        .map(|commit| commit.short)
        .collect()
}

#[test]
fn a_commit_shows_its_message_files_and_diffs() {
    let fixture = Fixture::new("history-details");
    let repo = fixture.repo_with_commit("app");
    write(
        &repo,
        "README.md",
        "# Fixture\n\nFirst line.\nSecond line.\n",
    );
    write(&repo, "notes.txt", "notes\n");
    fixture.git(&repo, &["add", "-A"]);
    fixture.git(
        &repo,
        &[
            "commit",
            "--quiet",
            "-m",
            "Change the readme",
            "-m",
            "And add a file.",
        ],
    );
    let (state, mut ui) = open(&fixture, repo.clone());
    let [newest, root] = <[String; 2]>::try_from(shorts(&fixture, &repo)).unwrap();

    // Newest first, with hash and author under the subject.
    let list = text(&ui, "commit-list");
    let (first, second) = (
        list.find("Change the readme").unwrap(),
        list.find("Initial commit").unwrap(),
    );
    assert!(first < second, "{list}");
    let row = text(&ui, &format!("commit-{newest}"));
    assert!(
        row.contains(&format!("{newest} · Fixture Author · ")),
        "{row}"
    );
    assert!(text(&ui, "commit-note").contains("Select a commit"));

    let row = ui.by_id(&format!("commit-{newest}"));
    ui.click(&row);
    assert!(matches!(state.details.get(), Loadable::Ready(_)));
    assert_eq!(text(&ui, "commit-subject"), "Change the readme");
    assert_eq!(text(&ui, "commit-body"), "And add a file.");
    assert_eq!(
        text(&ui, "commit-author"),
        "Fixture Author <fixture@example.com>"
    );
    assert_eq!(text(&ui, "commit-parents"), root);
    assert!(text(&ui, "commit-details").contains("Changed files (2)"));

    let file = ui.by_id("commit-file-notes.txt");
    ui.click(&file);
    assert_eq!(text(&ui, "commit-diff-header"), "notes.txt");
    assert!(text(&ui, "commit-diff-lines").contains("+notes"));
    let file = ui.by_id("commit-file-README.md");
    ui.click(&file);
    let lines = text(&ui, "commit-diff-lines");
    assert!(lines.contains("+Second line."), "{lines}");
    assert!(!lines.contains("+notes"), "{lines}");

    // The root commit has no parents and lists its one file.
    let row = ui.by_id(&format!("commit-{root}"));
    ui.click(&row);
    assert_eq!(text(&ui, "commit-parents"), "none (root commit)");
    assert!(text(&ui, "commit-details").contains("Changed files (1)"));
    assert!(state.commit_file.get().is_none());
}

#[test]
fn history_loads_a_page_at_a_time() {
    let fixture = Fixture::new("history-pages");
    let repo = fixture.init("long");
    let total = HISTORY_PAGE + 5;
    // One `git fast-import` is much faster than hundreds of commits.
    let mut stream = String::new();
    for n in 1..=total {
        let message = format!("Commit {n:04}");
        stream.push_str(&format!(
            "commit refs/heads/main\nmark :{n}\ncommitter Fixture Author <fixture@example.com> {} +0000\ndata {}\n{message}\n",
            1_700_000_000 + n,
            message.len()
        ));
        if n > 1 {
            stream.push_str(&format!("from :{}\n", n - 1));
        }
        stream.push('\n');
    }
    fixture
        .git
        .run_with_input(
            &repo,
            &[OsStr::new("fast-import"), OsStr::new("--quiet")],
            Access::Write,
            Some(stream.as_bytes()),
        )
        .unwrap();

    let (state, mut ui) = open(&fixture, repo);
    assert_eq!(state.history.get().commits.len(), HISTORY_PAGE);
    assert!(ui.find_text(&format!("Commit {total:04}")).is_some());
    assert!(ui.find_text("Commit 0001").is_none());

    // Scroll to the end of the list, as a user would, to reach the button.
    let list = ui.by_id("commit-list");
    let end = list.scroll_height().unwrap() as f64;
    list.scroll_to(0.0, end).unwrap();
    ui.run_until_idle();
    let more = ui.by_id("load-more");
    ui.click(&more);
    assert_eq!(state.history.get().commits.len(), total);
    assert!(ui.find_text("Commit 0001").is_some());
    assert!(ui.find_text("Load more").is_none());
}

#[test]
fn a_new_commit_appears_at_the_top() {
    let fixture = Fixture::new("history-follows");
    let repo = fixture.repo_with_commit("app");
    let (state, mut ui) = open(&fixture, repo.clone());
    assert_eq!(state.history.get().commits.len(), 1);

    write(&repo, "notes.txt", "notes\n");
    let refresh = ui.by_id("refresh");
    ui.click(&refresh);
    state.select_tab(Tab::Changes);
    ui.run_until_idle();
    let stage = ui.by_id("stage-notes.txt");
    ui.click(&stage);
    let message = ui.by_id("commit-message");
    ui.click(&message);
    ui.type_text("Add notes");
    let commit = ui.by_id("commit");
    ui.click(&commit);

    let history = state.history.get();
    assert_eq!(history.commits.len(), 2);
    assert_eq!(history.commits[0].subject, "Add notes");
    state.select_tab(Tab::History);
    ui.run_until_idle();
    let list = text(&ui, "commit-list");
    assert!(list.find("Add notes").unwrap() < list.find("Initial commit").unwrap());
}

#[test]
fn a_repository_without_commits_says_so() {
    let fixture = Fixture::new("history-empty");
    let repo = fixture.init("empty");
    let (state, ui) = open(&fixture, repo);
    assert!(state.history.get().commits.is_empty());
    assert_eq!(text(&ui, "history-note"), "No commits yet.");
}
