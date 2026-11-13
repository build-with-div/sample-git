//! The Changes tab, headless, against fixture repositories.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::TestApp;
use sample_git::git::status;
use sample_git::state::DiffState;
use sample_git::ui::diff_view::LINE_LIMIT;
use sample_git::{actions, ui, State};
use std::path::PathBuf;

fn open(fixture: &Fixture, repo: PathBuf) -> (State, TestApp) {
    let state = State::with(fixture.git.clone(), None);
    let mut ui = TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state));
    actions::add_repo(state, repo);
    ui.run_until_idle();
    (state, ui)
}

fn text(ui: &TestApp, id: &str) -> String {
    ui.by_id(id).text_content().unwrap()
}

#[test]
fn clicking_a_file_shows_its_diff() {
    let fixture = Fixture::new("changes-diff");
    let repo = fixture.repo_with_commit("app");
    write(
        &repo,
        "README.md",
        "# Fixture\n\nFirst line.\nAdded line.\n",
    );
    let (state, mut ui) = open(&fixture, repo);

    assert!(ui.find_text("Staged (0)").is_some());
    assert!(ui.find_text("Changes (1)").is_some());
    let row = ui.by_id("file-unstaged-README.md");
    ui.click(&row);
    assert_eq!(text(&ui, "diff-header"), "README.md");
    let lines = text(&ui, "diff-lines");
    assert!(lines.contains("@@ -1,3 +1,4 @@"), "{lines}");
    assert!(lines.contains("+Added line."), "{lines}");
    assert!(lines.contains(" First line."), "{lines}");
    assert!(matches!(state.diff.get(), DiffState::Ready(_)));
}

#[test]
fn staging_moves_a_file_and_its_selection() {
    let fixture = Fixture::new("changes-stage");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "README.md", "changed\n");
    write(&repo, "new.txt", "brand new\n");
    let (state, mut ui) = open(&fixture, repo.clone());

    let row = ui.by_id("file-unstaged-new.txt");
    ui.click(&row);
    assert_eq!(text(&ui, "diff-header"), "new.txt (new file)");
    assert!(text(&ui, "diff-lines").contains("+brand new"));

    let stage = ui.by_id("stage-new.txt");
    ui.click(&stage);
    assert!(ui.find_text("Staged (1)").is_some());
    assert!(ui.find_text("Changes (1)").is_some());
    assert_eq!(state.status.get(), "Staged new.txt");
    // The selection followed the file to the staged list.
    assert_eq!(text(&ui, "diff-header"), "new.txt (staged)");
    assert!(text(&ui, "diff-lines").contains("+brand new"));
    let staged = status::status(&fixture.git, &repo).unwrap();
    assert_eq!(staged.staged().count(), 1);

    let unstage = ui.by_id("unstage-new.txt");
    ui.click(&unstage);
    assert!(ui.find_text("Staged (0)").is_some());
    assert_eq!(text(&ui, "diff-header"), "new.txt (new file)");
    assert!(text(&ui, "diff-lines").contains("+brand new"));
}

#[test]
fn stage_all_and_unstage_all() {
    let fixture = Fixture::new("changes-all");
    let repo = fixture.repo_with_commit("app");
    write(&repo, "a.txt", "a\n");
    write(&repo, "b.txt", "b\n");
    let (_, mut ui) = open(&fixture, repo);
    let stage_all = ui.by_id("stage-all");
    ui.click(&stage_all);
    assert!(ui.find_text("Staged (2)").is_some());
    assert!(ui.find_text("Changes (0)").is_some());
    let unstage_all = ui.by_id("unstage-all");
    ui.click(&unstage_all);
    assert!(ui.find_text("Staged (0)").is_some());
    assert!(ui.find_text("Changes (2)").is_some());
}

#[test]
fn a_long_diff_is_cut_until_asked() {
    let fixture = Fixture::new("changes-long");
    let repo = fixture.repo_with_commit("app");
    let body: String = (0..LINE_LIMIT + 200)
        .map(|n| format!("line {n}\n"))
        .collect();
    write(&repo, "long.txt", &body);
    let (state, mut ui) = open(&fixture, repo);
    let row = ui.by_id("file-unstaged-long.txt");
    ui.click(&row);
    let shown = ui.by_id("diff-lines").text_content().unwrap();
    assert!(shown.contains(&format!("line {}", LINE_LIMIT - 100)));
    assert!(!shown.contains(&format!("line {}", LINE_LIMIT + 100)));
    assert!(text(&ui, "diff-truncated").starts_with(&format!("Showing {LINE_LIMIT} of")));
    let more = ui.by_id("show-full-diff");
    ui.click(&more);
    assert!(state.full_diff.get());
    let shown = ui.by_id("diff-lines").text_content().unwrap();
    assert!(shown.contains(&format!("line {}", LINE_LIMIT + 199)));
    assert!(ui.find_text("Show full diff").is_none());
}
