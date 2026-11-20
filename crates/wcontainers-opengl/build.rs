//! Makes Rust bindings of the pinned Mesa's Gallium headers when `WCONTAINERS_MESA` names its Meson
//! build folder; without it the crate builds without the driver.

use std::env;
use std::path::PathBuf;

#[path = "../wcontainers-nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{
    CFG, CHECK_CFG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, mesa_defines,
    source_folder,
};

/// The Gallium headers the driver is written against, and the bindings made of them in `OUT_DIR`
/// (`src/gallium.rs` includes them).
const GALLIUM_HEADER: &str = "bridge/gallium.h";
const BINDINGS: &str = "gallium.rs";
/// What the bindings hold: Gallium's types and constants, and the functions of Mesa's that the
/// driver calls.
const ALLOWED_TYPES: &str = "pipe_.*|util_format_description|blitter_context";
const ALLOWED_FUNCTIONS: &str = "u_init_pipe_screen_caps|u_upload_create_default|u_upload_destroy|\
     util_format_description|util_copy_framebuffer_state|util_blitter_(create|destroy|blit|is_blit_supported)";
const ALLOWED_CONSTANTS: &str = "PIPE_.*|MESA_SHADER_.*|MESA_PRIM_.*";
/// The Gallium file whose defines the bindings take from Mesa's compile commands.
const FLAGS_FROM: &str = "u_screen.c";
const CLANG_INCLUDE: &str = "-I";

/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &[
    "include",
    "src",
    "src/gallium/include",
    "src/gallium/auxiliary",
    "src/util",
];

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    println!("{RERUN_IF_CHANGED}{GALLIUM_HEADER}");
    let roots = [source_folder(&build), build.clone()];
    let includes = INCLUDES
        .iter()
        .flat_map(|folder| roots.iter().map(move |root| root.join(folder)))
        .map(|folder| format!("{CLANG_INCLUDE}{}", folder.display()));
    let defines = mesa_defines(&build, FLAGS_FROM);
    let bindings = bindgen::Builder::default()
        .header(GALLIUM_HEADER)
        .clang_args(includes.chain(defines))
        .allowlist_type(ALLOWED_TYPES)
        .allowlist_function(ALLOWED_FUNCTIONS)
        .allowlist_var(ALLOWED_CONSTANTS)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        .layout_tests(false)
        .generate()
        .expect("bindgen reads Mesa's Gallium headers");
    let file =
        PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets the output folder")).join(BINDINGS);
    bindings
        .write_to_file(&file)
        .expect("the bindings are written");
    println!("{CFG}");
}
