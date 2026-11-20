//! Makes Rust bindings of the pinned Mesa's Gallium headers when `WCONTAINERS_MESA` names its Meson
//! build folder; without it the crate builds without the driver.

use std::env;
use std::path::PathBuf;

#[path = "../wcontainers-nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{RERUN_IF_CHANGED, mesa_flags, source_folder};

const MESA_VARIABLE: &str = "WCONTAINERS_MESA";
const CHECK_CFG: &str = "cargo::rustc-check-cfg=cfg(mesa)";
const CFG: &str = "cargo::rustc-cfg=mesa";
const RERUN_IF_ENV_CHANGED: &str = "cargo::rerun-if-env-changed=";
const RUSTC_ENV: &str = "cargo::rustc-env=";
/// Between a variable and its value in `RUSTC_ENV`.
const ASSIGN: &str = "=";
/// The variable naming the bindings' file for `src/gallium.rs`.
const BINDINGS_VARIABLE: &str = "WCONTAINERS_GALLIUM_BINDINGS";

/// The Gallium headers the driver is written against, and the bindings made of them.
const GALLIUM_HEADER: &str = "bridge/gallium.h";
const BINDINGS: &str = "gallium.rs";
/// What the bindings hold: Gallium's types and constants, and the functions of Mesa's that the
/// driver calls.
const ALLOWED_TYPES: &str = "pipe_.*";
const ALLOWED_FUNCTIONS: &str = "u_init_pipe_screen_caps";
const ALLOWED_CONSTANTS: &str = "PIPE_.*|MESA_SHADER_.*";
/// The Gallium file whose defines the bindings take from Mesa's compile commands.
const FLAGS_FROM: &str = "u_screen.c";
/// How Mesa's compile commands and clang name a define.
const DEFINE_FLAGS: &[&str] = &["-D", "/D"];
const CLANG_DEFINE: &str = "-D";
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
    let defines = mesa_flags(&build, FLAGS_FROM)
        .into_iter()
        .filter_map(|flag| {
            DEFINE_FLAGS
                .iter()
                .find_map(|prefix| flag.strip_prefix(prefix))
                .map(|define| format!("{CLANG_DEFINE}{define}"))
        });
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
    let file = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join(BINDINGS);
    bindings
        .write_to_file(&file)
        .expect("the bindings are written");
    println!("{RUSTC_ENV}{BINDINGS_VARIABLE}{ASSIGN}{}", file.display());
    println!("{CFG}");
}
