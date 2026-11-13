//! The git layer: runs the user's `git` and parses its output. It does not
//! depend on Div, and every function blocks, so the UI calls it from
//! background tasks.

pub mod branch;
pub mod command;
pub mod diff;
pub mod log;
pub mod remote;
pub mod repo;
pub mod status;

pub use command::{Access, Cancel, Error, Git, MIN_VERSION};
