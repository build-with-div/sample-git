// Release builds open no console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use div::prelude::*;
use div::window::WindowSpec;
use sample_git::git::Git;
use sample_git::{actions, menu, store, ui, State};
use std::time::Duration;

fn main() -> Result<(), Error> {
    let state = State::with(Git::new(), store::default_path());
    App::new("build.div.samples.git")
        .name("Sample Git")
        .menu(move || menu::app_menu(state))
        .window(
            WindowSpec::new("main")
                .title("Sample Git")
                .size(1200.0, 760.0)
                .min_size(960.0, 600.0),
            move || {
                actions::check_git(state, state.git.get());
                actions::load_saved(state);
                // div-gap: 21. No window-activation event to refresh on.
                actions::refresh_selected_every(state, Duration::from_secs(30));
                ui::root(state)
            },
        )
        .run()
}
