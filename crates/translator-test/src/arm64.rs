//! The harness with ARM64 as its guest: each module of the harness, written once, compiled for it.

/// The guest every module here is compiled for.
pub type Cpu = wcontainers_arm64::Arm64;

#[path = "binaries.rs"]
pub mod binaries;
#[path = "browser.rs"]
pub mod browser;
#[path = "changes.rs"]
pub mod changes;
#[path = "guest/mod.rs"]
pub mod guest;
#[path = "layer1.rs"]
pub mod layer1;
#[path = "layer2.rs"]
pub mod layer2;
#[path = "layer3.rs"]
pub mod layer3;
#[path = "mixed.rs"]
pub mod mixed;
/// The ARM64 guest's own part of the harness: its names, forms, state and programs.
#[path = "guest/arm64.rs"]
pub mod part;
#[path = "programs.rs"]
pub mod programs;
#[path = "progress.rs"]
pub mod progress;
#[path = "runner.rs"]
pub mod runner;
