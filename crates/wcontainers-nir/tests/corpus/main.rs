//! The NIR route's corpus run (plan 15.3): every SPIR-V shader under `WC_NIR_CORPUS` through each
//! candidate route, each shader in a child process of its own (Mesa aborts on some inputs), one JSON
//! line a shader into `WC_NIR_RESULTS`. Each converted shader is held to lavapipe running the
//! program as given (`compare`). Knobs: `WC_NIR_SHADERS` (how many, a sample drawn with
//! `WC_NIR_SEED`, which also seeds every shader's inputs; all when unset), `WC_NIR_INPUT_SETS` (the
//! vertices or fragments each run, each with inputs of its own), `WC_NIR_JOBS` (children at once) and `WC_NIR_DUMP` (a folder for the
//! routes' intermediate forms). Without `WC_NIR_CORPUS`, or
//! without Mesa, it runs nothing.

#[cfg(mesa)]
mod bgfx;
#[cfg(mesa)]
mod compare;
#[cfg(mesa)]
mod entry;
#[cfg(mesa)]
mod glsl;
#[cfg(mesa)]
mod interface;
#[cfg(mesa)]
mod lavapipe;
#[cfg(mesa)]
mod llvmpipe;
#[cfg(mesa)]
mod needs;
#[cfg(mesa)]
mod run;

#[cfg(not(mesa))]
fn main() {
    eprintln!("corpus: built without Mesa; nothing run");
}

#[cfg(mesa)]
fn main() {
    run::main();
}
