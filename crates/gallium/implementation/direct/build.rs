// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Compiles the glue's C (flat.c) against the pinned Mesa's Gallium headers when `WCONTAINERS_MESA` names Mesa's Meson
//! build folder; without it the crate builds without its C and the `mesa` cfg is not set.

use std::env;
use std::path::PathBuf;

#[path = "../../../nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{CFG, CHECK_CFG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, mesa_flags, source_folder};

/// The glue's C and its header.
const FLAT_SOURCE: &str = "flat.c";
const FLAT_HEADER: &str = "flat.h";
const GLUE_LIBRARY: &str = "wcontainers_gallium_direct_glue";
/// The Gallium file whose defines and C standard the glue's C takes from Mesa's compile commands.
const FLAGS_FROM: &str = "u_screen.c";
/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &["include", "src", "src/gallium/include", "src/gallium/auxiliary", "src/util"];

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    println!("{RERUN_IF_CHANGED}{FLAT_SOURCE}");
    println!("{RERUN_IF_CHANGED}{FLAT_HEADER}");
    let roots = [source_folder(&build), build.clone()];
    let includes: Vec<PathBuf> = INCLUDES.iter().flat_map(|folder| roots.iter().map(move |root| root.join(folder))).collect();
    let mut glue = cc::Build::new();
    // Mesa's headers warn under the flags cc adds by default; Mesa builds them without those.
    glue.file(FLAT_SOURCE).includes(&includes).warnings(false);
    for flag in mesa_flags(&build, FLAGS_FROM) {
        glue.flag(flag);
    }
    glue.compile(GLUE_LIBRARY);
    println!("{CFG}");
}
