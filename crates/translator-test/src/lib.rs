//! Proving the translator against real Linux of each guest CPU: a wasmtime runner, a browser runner, the
//! native reference runner (in Docker, or over SSH on a machine of the guest's CPU), the random case
//! descriptions and the comparison. Written once over a guest's test part (`guest::GuestTest`).

pub mod binaries;
pub mod browser;
pub mod changes;
pub mod guest;
pub mod jobs;
pub mod layer1;
pub mod layer2;
pub mod layer3;
pub mod layout;
pub mod mixed;
pub mod ordered;
pub mod programs;
pub mod progress;
pub mod reference;
pub mod runner;
pub mod seeded;
pub mod state_memory;

/// What a run on the guest's CPU itself, not through a translation, is called in the tools' commands and
/// reports.
pub const NATIVE: &str = "native";
