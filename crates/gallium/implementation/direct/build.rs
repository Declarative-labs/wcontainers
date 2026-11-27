// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Compiles the glue's C (flat.c, serve.c; and the hostile C caller, caller.c, with the `hostile` feature) against the pinned Mesa's Gallium headers when `WCONTAINERS_MESA` names Mesa's Meson
//! build folder; without it the crate builds without its C and the `mesa` cfg is not set.

use std::env;
use std::path::PathBuf;

#[path = "../../../nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{ASSIGN, CFG, CHECK_CFG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, RUSTC_ENV, mesa_flags, source_folder};

/// The glue's C and its header.
const FLAT_SOURCE: &str = "flat.c";
const FLAT_HEADER: &str = "flat.h";
const SERVE_SOURCE: &str = "serve.c";
/// The hostile C caller the direct crate's own test drives, built with the `hostile` feature only.
const CALLER_SOURCE: &str = "caller.c";
const HOSTILE_FEATURE: &str = "CARGO_FEATURE_HOSTILE";
const GLUE_LIBRARY: &str = "wcontainers_gallium_direct_glue";
/// Mesa's libraries the hostile caller's reference links (by folder in Mesa's build), and how cargo is told.
const REFERENCE_LIBRARIES: &[(&str, &str)] = &[("src/gallium/auxiliary", "gallium"), ("src/util", "mesa_util")];
const LINK_SEARCH: &str = "cargo::rustc-link-search=native=";
const LINK_STATIC: &str = "cargo::rustc-link-lib=static=";
/// The Gallium file whose defines and C standard the glue's C takes from Mesa's compile commands.
const FLAGS_FROM: &str = "u_screen.c";
/// The variable that carries the glue's compile flags to tests/layouts.rs, and what separates them.
const CLANG_ARGS: &str = "WC_GALLIUM_CLANG_ARGS";
const ARGUMENT_SEPARATOR: &str = "\u{1f}";
/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &["include", "src", "src/gallium/include", "src/gallium/auxiliary", "src/util"];

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        // Said, not silent: without Mesa the glue's C, the layout comparison and the hostile caller are not built.
        println!("cargo:warning={MESA_VARIABLE} names no Mesa build: the glue's C, tests/layouts.rs and tests/served.rs are not built");
        return;
    };
    println!("{RERUN_IF_CHANGED}{FLAT_SOURCE}");
    println!("{RERUN_IF_CHANGED}{FLAT_HEADER}");
    println!("{RERUN_IF_CHANGED}{SERVE_SOURCE}");
    println!("{RERUN_IF_CHANGED}{CALLER_SOURCE}");
    let roots = [source_folder(&build), build.clone()];
    let includes: Vec<PathBuf> = INCLUDES.iter().flat_map(|folder| roots.iter().map(move |root| root.join(folder))).collect();
    let mut glue = cc::Build::new();
    // Mesa's headers warn under the flags cc adds by default; Mesa builds them without those.
    glue.file(FLAT_SOURCE).file(SERVE_SOURCE).includes(&includes).warnings(false);
    let hostile = env::var_os(HOSTILE_FEATURE).is_some();
    if hostile {
        glue.file(CALLER_SOURCE);
    }
    let flags = mesa_flags(&build, FLAGS_FROM);
    for flag in &flags {
        glue.flag(flag);
    }
    // The same flags for the layouts test's clang (the independent count of Mesa's fields), one separator apart.
    let clang_args: Vec<String> = includes.iter().map(|folder| format!("-I{}", folder.display())).chain(flags).collect();
    println!("{RUSTC_ENV}{CLANG_ARGS}{ASSIGN}{}", clang_args.join(ARGUMENT_SEPARATOR));
    glue.compile(GLUE_LIBRARY);
    if hostile {
        // The caller's reference for the screen's capabilities: Mesa's u_init_pipe_screen_caps (Gallium's auxiliary
        // library) and what it calls of Mesa's utilities, linked after the glue.
        for (folder, library) in REFERENCE_LIBRARIES {
            println!("{LINK_SEARCH}{}", build.join(folder).display());
            println!("{LINK_STATIC}{library}");
        }
    }
    println!("{CFG}");
}
