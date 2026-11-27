//! Hands the crate what it reads of the pinned Mesa's build when `WCONTAINERS_MESA` names the Meson build folder
//! `mesa/build` made: the converter's version and, with the `standalone` feature, the reference renderers' files.
//! The bridge over Mesa's compiler and its link are built by wcontainers-mesa-compiler-direct, the crate that calls
//! it; without `WCONTAINERS_MESA` the crate builds without the parts that read NIR.

use std::env;
use std::path::{Path, PathBuf};

#[path = "mesa/meson_build.rs"]
mod meson_build;
use meson_build::{
    ASSIGN, CFG, CHECK_CFG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, RUSTC_ENV,
};

/// Set by cargo when the `standalone` feature is on.
const STANDALONE_FEATURE: &str = "CARGO_FEATURE_STANDALONE";
/// What `mesa/build` records of the Mesa it built (the commit and the options), and the variable
/// that hands it to the crate, part of the converter's version.
const MADE_FROM: &str = "wcontainers-made-from";
const MESA_BUILD_VARIABLE: &str = "WCONTAINERS_MESA_MADE_FROM";
/// Lavapipe, the corpus run's Vulkan reference, by its Meson target, and the variable naming its file.
const LAVAPIPE_TARGET: &str = "vulkan_lvp";
const LAVAPIPE_VARIABLE: &str = "WCONTAINERS_LAVAPIPE";
/// Mesa's EGL, through which the corpus run reaches llvmpipe, its OpenGL reference.
const EGL_TARGET: &str = "EGL";
const EGL_VARIABLE: &str = "WCONTAINERS_EGL";
const INTROSPECTED_TARGETS: &str = "meson-info/intro-targets.json";
/// Between the parts of the version.
const VERSION_PARTS: &str = "+";
/// The converter's own sources (this crate's Rust, the bridge over Mesa's compiler in the direct crate that
/// builds it, and the naga whose module it makes), hashed into the converter's version, and the variable that
/// hands the hash to the crate.
const CONVERTER_SOURCES: &[&str] = &[
    "src",
    "../mesa-compiler/implementation/direct/bridge",
    "../../third_party/naga/src",
];
const SOURCES_VARIABLE: &str = "WCONTAINERS_NIR_SOURCES";

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    let build = &build;
    let made_from = build.join(MADE_FROM);
    println!("{RERUN_IF_CHANGED}{}", made_from.display());
    let recorded =
        std::fs::read_to_string(&made_from).expect("the Mesa build records what it built");
    let commit = recorded
        .lines()
        .next()
        .expect("the record starts with the commit");
    let options = blake3::hash(recorded.as_bytes()).to_hex();
    println!("{RUSTC_ENV}{MESA_BUILD_VARIABLE}{ASSIGN}{commit}{VERSION_PARTS}{options}");
    for folder in CONVERTER_SOURCES {
        println!("{RERUN_IF_CHANGED}{folder}");
    }
    println!(
        "{RUSTC_ENV}{SOURCES_VARIABLE}{ASSIGN}{}",
        converter_sources_hash()
    );
    println!("{CFG}");
    if env::var_os(STANDALONE_FEATURE).is_none() {
        return;
    }
    println!(
        "{RUSTC_ENV}{LAVAPIPE_VARIABLE}{ASSIGN}{}",
        target_file(build, LAVAPIPE_TARGET).display()
    );
    println!(
        "{RUSTC_ENV}{EGL_VARIABLE}{ASSIGN}{}",
        target_file(build, EGL_TARGET).display()
    );
}

/// BLAKE3 of every file under `CONVERTER_SOURCES`, each path then its bytes, in path order.
fn converter_sources_hash() -> String {
    let mut files = Vec::new();
    let mut folders: Vec<PathBuf> = CONVERTER_SOURCES.iter().map(PathBuf::from).collect();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("a converter source folder") {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update(file.to_string_lossy().as_bytes());
        hasher.update(&std::fs::read(&file).expect("a converter source file"));
    }
    hasher.finalize().to_hex().to_string()
}

/// The file Meson builds for a target, from its introspection of the build.
fn target_file(build: &Path, name: &str) -> PathBuf {
    #[derive(serde::Deserialize)]
    struct Target {
        name: String,
        filename: Vec<PathBuf>,
    }
    let path = build.join(INTROSPECTED_TARGETS);
    println!("{RERUN_IF_CHANGED}{}", path.display());
    let text = std::fs::read_to_string(&path).expect("Mesa's build has Meson's introspection");
    let targets: Vec<Target> = serde_json::from_str(&text).expect("introspection is JSON");
    targets
        .into_iter()
        .find(|t| t.name == name)
        .and_then(|t| t.filename.into_iter().next())
        .unwrap_or_else(|| panic!("Mesa's build has no target {name}"))
}
