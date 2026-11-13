//! Committing and discarding from the Changes tab, headless, against
//! fixture repositories.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::{Key, TestApp};
use sample_git::git::log;
use sample_git::{actions, ui, State};
use std::fs;
use std::path::{Path, PathBuf};

fn open(fixture: &Fixture, repo: PathBuf) -> (State, TestApp) {
    let state = State::with(fixture.git.clone(), None);
    let mut ui = TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state));
    actions::add_repo(state, repo);
    ui.run_until_idle();
    (state, ui)
}

fn subjects(fixture: &Fixture, repo: &Path) -> Vec<String> {
    log::log(&fixture.git, repo, 0, 10)
        .unwrap()
        .into_iter()
        .map(|commit| commit.subject)
        .collect()
}

fn disabled(ui: &TestApp, id: &str) -> bool {
    ui.by_id(id).get_attribute("disabled").unwrap().is_some()
}

fn write_message(ui: &mut TestApp, text: &str) {
    let message = ui.by_id("commit-message");
    ui.click(&message);
    ui.type_text(text);
}

#[test]
fn commit_needs_a_message_and_staged_files() {
    let fixture = Fixture::new("commit-button");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "README.md", "# Fixture\n\nChanged.\n");
    let (state, mut ui) = open(&fixture, repo.clone());

    assert!(disabled(&ui, "commit"), "nothing staged, no message");
    assert!(ui.find_text("Commit message").is_some(), "placeholder");
    write_message(&mut ui, "Explain the fixture");
    assert!(
        ui.find_text("Commit message").is_none(),
        "placeholder hidden"
    );
    assert_eq!(state.message.get(), "Explain the fixture");
    assert!(disabled(&ui, "commit"), "nothing staged");

    let stage = ui.by_id("stage-README.md");
    ui.click(&stage);
    assert!(!disabled(&ui, "commit"));

    let commit = ui.by_id("commit");
    ui.click(&commit);
    assert_eq!(
        subjects(&fixture, &repo),
        ["Explain the fixture", "Initial commit"]
    );
    assert_eq!(state.status.get(), "Committed: Explain the fixture");
    assert_eq!(state.message.get(), "");
    let box_text = ui.by_id("commit-message").control_value().unwrap();
    assert_eq!(box_text.unwrap_or_default(), "");
    assert!(ui.find_text("Staged (0)").is_some());
    assert!(ui.find_text("Changes (0)").is_some());
    assert!(disabled(&ui, "commit"));
    assert!(ui.find_text("Commit message").is_some(), "placeholder back");
}

#[test]
fn ctrl_enter_commits_a_multi_line_message() {
    let fixture = Fixture::new("commit-keys");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "notes.txt", "notes\n");
    let (state, mut ui) = open(&fixture, repo.clone());
    let stage_all = ui.by_id("stage-all");
    ui.click(&stage_all);

    write_message(&mut ui, "Add notes");
    ui.press(Key::Enter);
    ui.press(Key::Enter);
    ui.type_text("A body line.");
    assert_eq!(state.message.get(), "Add notes\n\nA body line.");
    ui.press_with(Modifiers::CTRL, Key::Enter);

    assert_eq!(subjects(&fixture, &repo)[0], "Add notes");
    let body = fixture.git(&repo, &["log", "-1", "--format=%B"]);
    assert_eq!(body.trim_end(), "Add notes\n\nA body line.");
    assert_eq!(state.message.get(), "");
}

#[test]
fn discard_asks_first_and_cancel_keeps_the_change() {
    let fixture = Fixture::new("discard-cancel");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "README.md", "# Fixture\n\nChanged.\n");
    let (state, mut ui) = open(&fixture, repo.clone());

    let discard = ui.by_id("discard-README.md");
    ui.click(&discard);
    assert_eq!(
        ui.by_id("confirm-title").text_content().unwrap(),
        "Discard changes to README.md?"
    );
    // The dialog keeps input: the row's Stage button under it does nothing.
    let stage = ui.by_id("stage-README.md");
    ui.click(&stage);
    assert!(ui.find_text("Staged (0)").is_some());

    let cancel = ui.by_id("confirm-cancel");
    ui.click(&cancel);
    assert!(state.confirm.get().is_none());
    assert!(ui.find_text("Discard changes to README.md?").is_none());
    assert_eq!(
        fs::read_to_string(repo.join("README.md")).unwrap(),
        "# Fixture\n\nChanged.\n"
    );

    // With the dialog gone, the row works again.
    let stage = ui.by_id("stage-README.md");
    ui.click(&stage);
    assert!(ui.find_text("Staged (1)").is_some());
}

#[test]
fn discard_restores_the_file_after_confirming() {
    let fixture = Fixture::new("discard-accept");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "README.md", "# Fixture\n\nChanged.\n");
    let (state, mut ui) = open(&fixture, repo.clone());

    let discard = ui.by_id("discard-README.md");
    ui.click(&discard);
    let accept = ui.by_id("confirm-accept");
    assert_eq!(accept.text_content().unwrap(), "Discard changes");
    ui.click(&accept);
    assert_eq!(
        fs::read_to_string(repo.join("README.md")).unwrap(),
        "# Fixture\n\nFirst line.\n"
    );
    assert_eq!(state.status.get(), "Discarded changes to README.md");
    assert!(ui.find_text("Changes (0)").is_some());
}

#[test]
fn an_untracked_file_is_deleted_and_escape_cancels() {
    let fixture = Fixture::new("discard-untracked");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "scratch.txt", "scratch\n");
    let (_, mut ui) = open(&fixture, repo.clone());

    let delete = ui.by_id("discard-scratch.txt");
    assert_eq!(delete.text_content().unwrap(), "Delete");
    ui.click(&delete);
    assert_eq!(
        ui.by_id("confirm-title").text_content().unwrap(),
        "Delete scratch.txt?"
    );
    ui.press(Key::Escape);
    assert!(ui.find_text("Delete scratch.txt?").is_none());
    assert!(repo.join("scratch.txt").exists());

    let delete = ui.by_id("discard-scratch.txt");
    ui.click(&delete);
    let accept = ui.by_id("confirm-accept");
    ui.click(&accept);
    assert!(!repo.join("scratch.txt").exists());
    assert!(ui.find_text("Changes (0)").is_some());
}
