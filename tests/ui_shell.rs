//! Headless tests of the window layout.

use div::prelude::*;
use div::testing::TestApp;
use sample_git::{ui, State, Tab};

fn mount(state: State) -> TestApp {
    TestApp::new(LogicalSize::new(1200.0, 760.0)).mount(move || ui::root(state))
}

#[test]
fn the_shell_shows_its_empty_states() {
    let ui = mount(State::new());
    assert!(ui.find_text("No repository selected").is_some());
    assert!(ui.find_text("Select a repository in the list.").is_some());
    assert!(ui
        .find_text("No repositories yet. Add one, or scan a folder for them.")
        .is_some());
    assert!(ui.find_text("Ready").is_some());
}

#[test]
fn clicking_a_tab_selects_it() {
    let state = State::new();
    let mut ui = mount(state);
    let history = ui.by_text("History");
    ui.click(&history);
    assert_eq!(state.tab.get(), Tab::History.value());
    let selected = history.get_attribute("aria-selected").unwrap();
    assert_eq!(selected.as_deref(), Some("true"));
}

#[test]
fn the_sidebar_is_240px_wide() {
    let ui = mount(State::new());
    let sidebar = ui.by_id("sidebar").bounding_rect().unwrap().unwrap();
    assert_eq!(sidebar.width, 240.0);
    let main = ui.by_id("main").bounding_rect().unwrap().unwrap();
    assert_eq!(main.x, 240.0);
    assert_eq!(main.width, 960.0);
}
