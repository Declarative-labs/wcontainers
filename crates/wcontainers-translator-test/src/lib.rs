//! Proving the translator against real ARM64 Linux: a wasmtime runner, a browser runner, the native
//! reference runner in Docker, the random case descriptions and the comparison.

pub mod binaries;
pub mod browser;
pub mod changes;
pub mod jobs;
pub mod layer1;
pub mod layer2;
pub mod layer3;
pub mod layout;
pub mod mixed;
pub mod programs;
pub mod progress;
pub mod reference;
pub mod runner;
pub mod seeded;
