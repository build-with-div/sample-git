//! The repository list, headless, against fixture repositories.

mod common;

use common::{write, Fixture};
use div::prelude::*;
use div::testing::TestApp;
use sample_git::state::Summary;
use sample_git::{actions, store, ui, State};

fn mount(state: State) -> TestApp {
    TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state))
}

fn state_for(fixture: &Fixture) -> State {
    State::with(fixture.git.clone(), Some(fixture.path("repos.txt")))
}

#[test]
fn adding_a_repository_lists_selects_and_saves_it() {
    let fixture = Fixture::new("ui-add");
    let repo = fixture.repo_with_commit("alpha");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let state = state_for(&fixture);
    let mut ui = mount(state);

    // A folder inside the repository adds the repository itself.
    actions::add_repo(state, repo.join("src"));
    ui.run_until_idle();

    assert_eq!(state.repos.get().len(), 1);
    assert!(ui.find_text("alpha").is_some());
    assert!(ui.find_text("main · clean").is_some());
    assert_eq!(
        ui.by_id("repo-title").text_content().unwrap(),
        "alpha",
        "the new repository is selected"
    );
    assert_eq!(state.status.get(), "Added alpha");
    let saved = store::load(&fixture.path("repos.txt"));
    assert_eq!(saved.len(), 1);
    assert!(actions::same_path(&saved[0], &repo));

    // Adding it again keeps one entry.
    actions::add_repo(state, repo.clone());
    ui.run_until_idle();
    assert_eq!(state.repos.get().len(), 1);
    assert_eq!(state.status.get(), "alpha is already in the list");
}

#[test]
fn a_dirty_repository_shows_its_change_count() {
    let fixture = Fixture::new("ui-dirty");
    let repo = fixture.repo_with_commit("beta");
    write(&repo, "README.md", "changed\n");
    write(&repo, "new.txt", "new\n");
    let state = state_for(&fixture);
    let mut ui = mount(state);
    actions::add_repo(state, repo);
    ui.run_until_idle();
    assert!(ui.find_text("main · 2 changes").is_some());
    assert_eq!(
        ui.by_id("repo-summary").text_content().unwrap(),
        "main · 2 changes"
    );
}

#[test]
fn a_plain_folder_is_reported_not_added() {
    let fixture = Fixture::new("ui-plain");
    let plain = fixture.path("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let state = state_for(&fixture);
    let mut ui = mount(state);
    actions::add_repo(state, plain.clone());
    ui.run_until_idle();
    assert!(state.repos.get().is_empty());
    assert!(state.status.get().ends_with("is not in a git repository"));
    assert!(ui
        .find_text("No repositories yet. Add one, or scan a folder for them.")
        .is_some());
}

#[test]
fn scanning_adds_every_repository_and_clicking_selects_one() {
    let fixture = Fixture::new("ui-scan");
    fixture.repo_with_commit("work/one");
    fixture.repo_with_commit("work/group/two");
    let state = state_for(&fixture);
    let mut ui = mount(state);
    actions::scan_folder(state, fixture.path("work"));
    ui.run_until_idle();
    let names: Vec<String> = state.repos.get().into_iter().map(|r| r.name).collect();
    // In path order: work/group/two sorts before work/one.
    assert_eq!(names, vec!["two", "one"]);
    assert_eq!(state.status.get(), "Found 2 repositories (2 new)");
    assert_eq!(ui.by_id("repo-title").text_content().unwrap(), "two");

    let one = ui.by_id("repo-one");
    ui.click(&one);
    assert_eq!(ui.by_id("repo-title").text_content().unwrap(), "one");

    // Scanning again adds nothing.
    actions::scan_folder(state, fixture.path("work"));
    ui.run_until_idle();
    assert_eq!(state.status.get(), "Found 2 repositories (0 new)");
}

#[test]
fn removing_takes_a_repository_off_the_list_only() {
    let fixture = Fixture::new("ui-remove");
    let repo = fixture.repo_with_commit("gamma");
    let state = state_for(&fixture);
    let mut ui = mount(state);
    actions::add_repo(state, repo.clone());
    ui.run_until_idle();

    let remove = ui.by_id("remove-gamma");
    ui.click(&remove);
    assert!(state.repos.get().is_empty());
    assert_eq!(state.selected.get(), None);
    assert!(store::load(&fixture.path("repos.txt")).is_empty());
    assert!(repo.join(".git").exists(), "nothing on disk is touched");
    assert_eq!(
        ui.by_id("repo-title").text_content().unwrap(),
        "No repository selected"
    );
}

#[test]
fn the_saved_list_loads_at_start_and_reports_missing_folders() {
    let fixture = Fixture::new("ui-load");
    let repo = fixture.repo_with_commit("delta");
    let gone = fixture.path("gone");
    store::save(&fixture.path("repos.txt"), &[repo.clone(), gone.clone()]).unwrap();
    let state = state_for(&fixture);
    let mut ui = mount(state);
    actions::load_saved(state);
    ui.run_until_idle();
    assert_eq!(state.repos.get().len(), 2);
    assert_eq!(state.selected.get(), Some(repo.clone()));
    assert!(matches!(state.summary(&repo), Summary::Ready { .. }));
    assert!(matches!(state.summary(&gone), Summary::Error(_)));
}
