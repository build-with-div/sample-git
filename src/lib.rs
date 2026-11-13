//! Sample Git: a small local git client built on Div.
//!
//! `main.rs` opens the window; the views, menu and state live here so the
//! headless tests in `tests/` can mount them.

pub mod actions;
pub mod git;
pub mod menu;
pub mod state;
pub mod store;
pub mod ui;

pub use state::{State, Tab};
