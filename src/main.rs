// Release builds open no console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use div::prelude::*;
use div::window::WindowSpec;
use sample_git::{menu, ui, State};

fn main() -> Result<(), Error> {
    let state = State::new();
    App::new("build.div.samples.git")
        .name("Sample Git")
        .menu(move || menu::app_menu(state))
        .window(
            WindowSpec::new("main")
                .title("Sample Git")
                .size(1200.0, 760.0)
                .min_size(960.0, 600.0),
            move || ui::root(state),
        )
        .run()
}
