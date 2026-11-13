//! The Branches tab, headless, against fixture repositories.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::{Key, TestApp};
use sample_git::{actions, ui, State, Tab};
use std::path::{Path, PathBuf};

fn open(fixture: &Fixture, repo: PathBuf) -> (State, TestApp) {
    let state = State::with(fixture.git.clone(), None);
    let mut ui = TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state));
    actions::add_repo(state, repo);
    ui.run_until_idle();
    let tab = ui.by_text("Branches");
    ui.click(&tab);
    assert_eq!(state.tab.get(), Tab::Branches.value());
    (state, ui)
}

fn text(ui: &TestApp, id: &str) -> String {
    ui.by_id(id).text_content().unwrap()
}

fn exists(ui: &TestApp, id: &str) -> bool {
    ui.document().element_by_id(id).unwrap().is_some()
}

fn head(fixture: &Fixture, repo: &Path) -> String {
    fixture
        .git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned()
}

fn local_branches(fixture: &Fixture, repo: &Path) -> Vec<String> {
    fixture
        .git(
            repo,
            &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        )
        .lines()
        .map(str::to_owned)
        .collect()
}

fn click(ui: &mut TestApp, id: &str) {
    let element = ui.by_id(id);
    ui.click(&element);
}

#[test]
fn local_branches_show_the_current_one_and_switch() {
    let fixture = Fixture::new("branches-switch");
    let repo = fixture.repo_with_commit("app");
    fixture.git(&repo, &["branch", "topic"]);
    let (state, mut ui) = open(&fixture, repo.clone());

    assert_eq!(text(&ui, "local-branches-header"), "Local branches (2)");
    let main = text(&ui, "branch-main");
    assert!(main.contains("current"), "{main}");
    assert!(main.contains("Initial commit"), "{main}");
    assert_eq!(text(&ui, "tracking-main"), "no upstream");
    assert!(!exists(&ui, "switch-main"));
    assert!(!exists(&ui, "delete-main"));
    assert!(!text(&ui, "branch-topic").contains("current"));
    assert!(text(&ui, "new-branch-base").starts_with("from main at "));
    assert!(text(&ui, "no-remote-branches").contains("No remote branches"));

    click(&mut ui, "switch-topic");
    assert_eq!(head(&fixture, &repo), "topic");
    assert_eq!(state.status.get(), "Switched to topic");
    assert!(text(&ui, "branch-topic").contains("current"));
    assert!(exists(&ui, "switch-main"));
    assert!(!exists(&ui, "switch-topic"));
    assert_eq!(text(&ui, "repo-summary"), "topic · clean");
}

#[test]
fn a_switch_that_would_lose_changes_is_refused() {
    let fixture = Fixture::new("branches-refused");
    let repo = fixture.repo_with_commit("app");
    fixture.git(&repo, &["switch", "--quiet", "-c", "other"]);
    write(&repo, "README.md", "# Other\n");
    fixture.git(&repo, &["commit", "--quiet", "-am", "Other readme"]);
    fixture.git(&repo, &["switch", "--quiet", "main"]);
    write(&repo, "README.md", "# Local edit\n");
    let (state, mut ui) = open(&fixture, repo.clone());

    click(&mut ui, "switch-other");
    assert_eq!(head(&fixture, &repo), "main");
    let status = state.status.get();
    assert!(status.contains("would be overwritten"), "{status}");
    assert!(text(&ui, "branch-main").contains("current"));
}

#[test]
fn a_new_branch_starts_at_head_and_is_checked_out() {
    let fixture = Fixture::new("branches-create");
    let repo = fixture.repo_with_commit("app");
    let (state, mut ui) = open(&fixture, repo.clone());
    let disabled = |ui: &TestApp| {
        ui.by_id("create-branch")
            .get_attribute("disabled")
            .unwrap()
            .is_some()
    };
    assert!(disabled(&ui));

    // An invalid name is reported and changes nothing.
    click(&mut ui, "new-branch-name");
    ui.type_text("bad name");
    assert!(!disabled(&ui));
    click(&mut ui, "create-branch");
    assert_eq!(
        state.status.get(),
        "\"bad name\" is not a valid branch name"
    );
    assert_eq!(local_branches(&fixture, &repo), ["main"]);

    // Enter in the box creates the branch and clears the box.
    state.new_branch.set(String::new());
    ui.run_until_idle();
    click(&mut ui, "new-branch-name");
    ui.type_text("feature/login");
    ui.press(Key::Enter);
    assert_eq!(head(&fixture, &repo), "feature/login");
    assert_eq!(state.status.get(), "Created and switched to feature/login");
    assert_eq!(state.new_branch.get(), "");
    let input = ui.by_id("new-branch-name");
    assert_eq!(input.control_value().unwrap().unwrap_or_default(), "");
    assert!(disabled(&ui));
    assert!(text(&ui, "branch-feature/login").contains("current"));
    assert_eq!(text(&ui, "local-branches-header"), "Local branches (2)");
}

#[test]
fn deleting_a_branch_asks_first_and_keeps_unmerged_work() {
    let fixture = Fixture::new("branches-delete");
    let repo = fixture.repo_with_commit("app");
    fixture.git(&repo, &["branch", "done"]);
    fixture.git(&repo, &["switch", "--quiet", "-c", "wip"]);
    write(&repo, "wip.txt", "wip\n");
    fixture.git(&repo, &["add", "wip.txt"]);
    fixture.git(&repo, &["commit", "--quiet", "-m", "Unfinished"]);
    fixture.git(&repo, &["switch", "--quiet", "main"]);
    let (state, mut ui) = open(&fixture, repo.clone());

    // Cancel keeps the branch.
    click(&mut ui, "delete-wip");
    assert_eq!(text(&ui, "confirm-title"), "Delete branch wip?");
    click(&mut ui, "confirm-cancel");
    assert!(state.confirm.get().is_none());
    assert_eq!(local_branches(&fixture, &repo), ["done", "main", "wip"]);

    // git refuses to delete unmerged work, and says why.
    click(&mut ui, "delete-wip");
    click(&mut ui, "confirm-accept");
    let status = state.status.get();
    assert!(status.contains("not fully merged"), "{status}");
    assert_eq!(local_branches(&fixture, &repo), ["done", "main", "wip"]);
    assert!(exists(&ui, "branch-wip"));

    // A merged branch goes.
    click(&mut ui, "delete-done");
    click(&mut ui, "confirm-accept");
    assert_eq!(state.status.get(), "Deleted branch done");
    assert_eq!(local_branches(&fixture, &repo), ["main", "wip"]);
    assert!(!exists(&ui, "branch-done"));
}

#[test]
fn a_remote_branch_is_checked_out_as_a_tracking_branch() {
    let fixture = Fixture::new("branches-remote");
    let (_origin, clone) = fixture.clone_with_origin("app");
    fixture.git(&clone, &["push", "--quiet", "origin", "main:feature"]);
    fixture.git(&clone, &["fetch", "--quiet"]);
    let (state, mut ui) = open(&fixture, clone.clone());

    assert_eq!(text(&ui, "remote-branches-header"), "Remote branches (2)");
    assert_eq!(text(&ui, "tracking-main"), "origin/main");
    assert_eq!(text(&ui, "tracked-origin/main"), "tracked by main");
    assert!(!exists(&ui, "checkout-origin/main"));
    assert!(!exists(&ui, "no-remote-branches"));

    click(&mut ui, "checkout-origin/feature");
    assert_eq!(head(&fixture, &clone), "feature");
    assert_eq!(
        state.status.get(),
        "Created feature tracking origin/feature"
    );
    assert!(text(&ui, "branch-feature").contains("current"));
    assert_eq!(text(&ui, "tracking-feature"), "origin/feature");
    assert_eq!(text(&ui, "tracked-origin/feature"), "tracked by feature");
}
