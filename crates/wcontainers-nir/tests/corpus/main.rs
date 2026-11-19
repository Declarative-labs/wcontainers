//! The NIR route's corpus run (plan 15.3): every SPIR-V shader under `WC_NIR_CORPUS` through each
//! candidate route, each shader in a child process of its own (Mesa aborts on some inputs), one JSON
//! line a shader into `WC_NIR_RESULTS`. Knobs: `WC_NIR_SHADERS` (how many, a sample drawn with
//! `WC_NIR_SEED`; all when unset), `WC_NIR_JOBS` (children at once) and `WC_NIR_DUMP` (a folder for the
//! routes' intermediate forms). Without `WC_NIR_CORPUS`, or
//! without Mesa, it runs nothing.

#[cfg(mesa)]
mod entry;
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
