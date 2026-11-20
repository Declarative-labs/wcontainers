//! Builds the Gallium bridge against the pinned Mesa when `WCONTAINERS_MESA` names its Meson build
//! folder; without it the crate builds without the driver.

use std::env;
use std::path::PathBuf;

#[path = "../wcontainers-nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{RERUN_IF_CHANGED, mesa_flags, source_folder};

const MESA_VARIABLE: &str = "WCONTAINERS_MESA";
const CHECK_CFG: &str = "cargo::rustc-check-cfg=cfg(mesa)";
const CFG: &str = "cargo::rustc-cfg=mesa";
const RERUN_IF_ENV_CHANGED: &str = "cargo::rerun-if-env-changed=";

const SCREEN_SOURCE: &str = "bridge/screen.c";
const SCREEN_HEADER: &str = "bridge/screen.h";
const BRIDGE_LIBRARY: &str = "wcontainers_opengl_bridge";
/// The Gallium file whose flags (defines and C standard) the bridge takes from Mesa's compile
/// commands.
const FLAGS_FROM: &str = "u_screen.c";

/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &[
    "include",
    "src",
    "src/gallium/include",
    "src/gallium/auxiliary",
    "src/gallium/drivers",
    "src/util",
];

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    println!("{RERUN_IF_CHANGED}{SCREEN_SOURCE}");
    println!("{RERUN_IF_CHANGED}{SCREEN_HEADER}");
    let roots = [source_folder(&build), build.clone()];
    let includes: Vec<PathBuf> = INCLUDES
        .iter()
        .flat_map(|folder| roots.iter().map(move |root| root.join(folder)))
        .collect();
    let mut bridge = cc::Build::new();
    // Mesa's headers warn under the flags cc adds by default; Mesa builds them without those.
    bridge
        .file(SCREEN_SOURCE)
        .includes(&includes)
        .warnings(false);
    for flag in mesa_flags(&build, FLAGS_FROM) {
        bridge.flag(flag);
    }
    bridge.compile(BRIDGE_LIBRARY);
    println!("{CFG}");
}
