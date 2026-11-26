//! Proving the translator against real Linux of each guest CPU: a wasmtime runner, a browser runner, the
//! native reference runner (in Docker, or over SSH on a machine of the guest's CPU), the random case
//! descriptions and the comparison. Written once and compiled for each guest, in `arm64` and `x86_64`.

#[allow(
    clippy::duplicate_mod,
    reason = "each guest's module compiles the same harness files, the form of a harness with no trait over its guest"
)]
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
pub mod arm64;
pub mod guest_memory;
pub mod jobs;
pub mod layout;
pub mod ordered;
pub mod reference;
pub mod seeded;
pub mod state_memory;
#[allow(
    clippy::duplicate_mod,
    reason = "each guest's module compiles the same harness files, the form of a harness with no trait over its guest"
)]
pub mod x86_64;

/// What a run on the guest's CPU itself, not through a translation, is called in the tools' commands and
/// reports.
pub const NATIVE: &str = "native";
