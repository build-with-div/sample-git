//! Fetch, pull and push from the toolbar, headless, against a bare
//! fixture origin and its clones.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::TestApp;
use sample_git::{actions, ui, State};
use std::path::{Path, PathBuf};

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

fn exists(ui: &TestApp, id: &str) -> bool {
    ui.document().element_by_id(id).unwrap().is_some()
}

fn disabled(ui: &TestApp, id: &str) -> bool {
    ui.by_id(id).get_attribute("disabled").unwrap().is_some()
}

fn click(ui: &mut TestApp, id: &str) {
    let element = ui.by_id(id);
    ui.click(&element);
}

/// Commit `file` in `repo` with `subject`.
fn commit(fixture: &Fixture, repo: &Path, file: &str, subject: &str) {
    write(repo, file, &format!("{subject}\n"));
    fixture.git(repo, &["add", file]);
    fixture.git(repo, &["commit", "--quiet", "-m", subject]);
}

/// A commit on origin's main from its other clone, the seed.
fn commit_upstream(fixture: &Fixture, name: &str, subject: &str) {
    let seed = fixture.path(&format!("{name}-seed"));
    commit(fixture, &seed, "theirs.txt", subject);
    fixture.git(&seed, &["push", "--quiet", "origin", "main"]);
}

fn origin_subject(fixture: &Fixture, origin: &Path, branch: &str) -> String {
    fixture
        .git(origin, &["log", "-1", "--format=%s", branch])
        .trim()
        .to_owned()
}

#[test]
fn without_a_remote_the_buttons_are_disabled() {
    let fixture = Fixture::new("remote-none");
    let repo = fixture.repo_with_commit("app");
    let (_state, ui) = open(&fixture, repo);
    assert!(disabled(&ui, "fetch"));
    assert!(disabled(&ui, "pull"));
    assert!(disabled(&ui, "push"));
}

#[test]
fn fetch_then_pull_brings_in_new_commits() {
    let fixture = Fixture::new("remote-pull");
    let (_origin, clone) = fixture.clone_with_origin("app");
    commit_upstream(&fixture, "app", "Theirs");
    let (state, mut ui) = open(&fixture, clone.clone());
    assert!(!disabled(&ui, "fetch"));
    assert!(!disabled(&ui, "pull"));
    assert!(!disabled(&ui, "push"));

    click(&mut ui, "fetch");
    assert_eq!(text(&ui, "status-text"), "Fetched");
    assert_eq!(text(&ui, "repo-summary"), "main · clean · ↓1");
    assert!(!exists(&ui, "cancel-remote"));

    click(&mut ui, "pull");
    assert_eq!(text(&ui, "status-text"), "Pulled origin/main");
    assert!(clone.join("theirs.txt").exists());
    assert_eq!(text(&ui, "repo-summary"), "main · clean");
    assert!(!state.busy.get());
}

#[test]
fn push_sends_local_commits_to_the_upstream() {
    let fixture = Fixture::new("remote-push");
    let (origin, clone) = fixture.clone_with_origin("app");
    commit(&fixture, &clone, "mine.txt", "Mine");
    let (_state, mut ui) = open(&fixture, clone);
    assert_eq!(text(&ui, "repo-summary"), "main · clean · ↑1");

    click(&mut ui, "push");
    assert_eq!(text(&ui, "status-text"), "Pushed main to origin/main");
    assert_eq!(origin_subject(&fixture, &origin, "main"), "Mine");
    assert_eq!(text(&ui, "repo-summary"), "main · clean");
}

#[test]
fn a_branch_without_an_upstream_is_published_after_a_question() {
    let fixture = Fixture::new("remote-publish");
    let (origin, clone) = fixture.clone_with_origin("app");
    fixture.git(&clone, &["switch", "--quiet", "-c", "topic"]);
    commit(&fixture, &clone, "topic.txt", "Topic work");
    let (state, mut ui) = open(&fixture, clone.clone());
    assert!(disabled(&ui, "pull"));

    click(&mut ui, "push");
    assert_eq!(text(&ui, "confirm-title"), "Publish topic?");
    assert_eq!(text(&ui, "confirm-detail"), "git push -u origin topic");
    click(&mut ui, "confirm-cancel");
    let branches = fixture.git(&origin, &["branch", "--list", "topic"]);
    assert_eq!(branches.trim(), "");

    click(&mut ui, "push");
    click(&mut ui, "confirm-accept");
    assert_eq!(state.status.get(), "Published topic to origin");
    assert_eq!(origin_subject(&fixture, &origin, "topic"), "Topic work");
    let upstream = fixture.git(&clone, &["rev-parse", "--abbrev-ref", "topic@{upstream}"]);
    assert_eq!(upstream.trim(), "origin/topic");
    assert!(!disabled(&ui, "pull"));
}

#[test]
fn a_failed_pull_shows_git_output_in_details() {
    let fixture = Fixture::new("remote-diverged");
    let (_origin, clone) = fixture.clone_with_origin("app");
    commit_upstream(&fixture, "app", "Theirs");
    commit(&fixture, &clone, "mine.txt", "Mine");
    fixture.git(&clone, &["fetch", "--quiet"]);
    let (state, mut ui) = open(&fixture, clone);
    assert!(!exists(&ui, "remote-details"));

    click(&mut ui, "pull");
    let status = text(&ui, "status-text");
    assert!(status.contains("fast-forward"), "{status}");
    click(&mut ui, "remote-details");
    assert_eq!(text(&ui, "confirm-title"), "Pull failed");
    let detail = text(&ui, "confirm-detail");
    assert!(
        detail.starts_with("$ git pull --ff-only --progress\n"),
        "{detail}"
    );
    assert!(detail.contains("fast-forward"), "{detail}");
    // A message has one button, which closes it.
    assert!(!exists(&ui, "confirm-cancel"));
    click(&mut ui, "confirm-accept");
    assert!(state.confirm.get().is_none());

    // Details belongs to the failure's message; a newer message hides it.
    assert!(exists(&ui, "remote-details"));
    click(&mut ui, "refresh");
    state.status.set("Ready".to_owned());
    ui.run_until_idle();
    assert!(!exists(&ui, "remote-details"));
}

#[test]
fn cancel_stops_a_running_fetch() {
    let fixture = Fixture::new("remote-cancel");
    let (_origin, clone) = fixture.clone_with_origin("app");
    commit_upstream(&fixture, "app", "Theirs");
    let (state, mut ui) = open(&fixture, clone);

    // div-gap: 29. Input in `TestApp` runs all work to the end, so a click
    // on Fetch would finish before Cancel could be pressed. The action
    // starts the fetch instead, and the click on Cancel lands before it
    // runs.
    actions::fetch(state);
    assert!(state.busy.get());
    assert_eq!(text(&ui, "status-text"), "Fetching…");
    assert!(disabled(&ui, "pull"));
    click(&mut ui, "cancel-remote");
    assert_eq!(text(&ui, "status-text"), "Fetch cancelled");
    assert!(!exists(&ui, "cancel-remote"));
    assert!(!state.busy.get());
    assert_eq!(text(&ui, "repo-summary"), "main · clean");
}
