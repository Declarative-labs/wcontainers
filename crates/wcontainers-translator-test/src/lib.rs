//! Proving the translator against real ARM64 Linux: a wasmtime runner, a browser runner, the native
//! reference runner in Docker, the random case descriptions and the comparison.

pub mod browser;
pub mod layer1;
pub mod layout;
pub mod programs;
pub mod reference;
pub mod runner;
