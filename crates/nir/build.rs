//! Links the pinned Mesa's compiler libraries and the bridge to them when `WCONTAINERS_MESA` names the
//! Meson build folder `mesa/build` made; without it the crate builds without the parts that read NIR.
//! Without the `standalone` feature only the bridge is built: Mesa is linked by the program the
//! crate is linked into (the OpenGL driver inside Mesa's own library).

use std::env;
use std::path::{Path, PathBuf};

#[path = "mesa/meson_build.rs"]
mod meson_build;
use meson_build::{
    ASSIGN, CFG, CHECK_CFG, LINK_ARG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED,
    RUSTC_ENV, mesa_flags, source_folder,
};

const LINK_SEARCH: &str = "cargo::rustc-link-search=native=";
const LINK_STATIC: &str = "cargo::rustc-link-lib=static=";
const LINK_DYNAMIC: &str = "cargo::rustc-link-lib=dylib=";

/// Set by cargo when the `standalone` feature is on.
const STANDALONE_FEATURE: &str = "CARGO_FEATURE_STANDALONE";
const BRIDGE_SOURCE: &str = "bridge/bridge.c";
/// The SPIR-V steps: Mesa's SPIR-V reader and candidate A's Zink compiler.
const SPIRV_SOURCE: &str = "bridge/spirv.c";
const BRIDGE_HEADER: &str = "bridge/bridge.h";
/// Candidate B's flattening of NIR.
const FLAT_SOURCE: &str = "bridge/flat.c";
const FLAT_HEADER: &str = "bridge/flat.h";
/// The GLSL front (C++, Mesa's GLSL compiler) and what the bridge's files share.
const GLSL_SOURCE: &str = "bridge/glsl.cpp";
const PRIVATE_HEADER: &str = "bridge/bridge_private.h";
const GLSL_LIBRARY: &str = "wcontainers_nir_glsl";
const BRIDGE_LIBRARY: &str = "wcontainers_nir_bridge";
/// The file whose flags (defines and C standard) the bridge takes from Mesa's compile commands.
const FLAGS_FROM: &str = "nir_serialize.c";
/// The C++ file of Mesa's GLSL compiler whose flags the GLSL front takes.
const CPP_FLAGS_FROM: &str = "standalone.cpp";

/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &[
    "include",
    "src",
    "src/compiler",
    "src/compiler/nir",
    "src/compiler/glsl",
    "src/compiler/spirv",
    "src/gallium/include",
    "src/gallium/auxiliary",
    "src/gallium/drivers/zink",
    "src/gallium/drivers/zink/nir_to_spirv",
    "src/mesa",
    "src/mapi",
    "src/util",
    "src/util/format",
];

/// The static libraries, by their folder in Mesa's build, in link order (users before what they use).
const LIBRARIES: &[(&str, &str)] = &[
    ("src/gallium/drivers/zink", "zink"),
    ("src/compiler/glsl", "glsl_standalone"),
    ("src/compiler/glsl", "glsl"),
    ("src/compiler/glsl", "glsl_util"),
    ("src/compiler/glsl/glcpp", "glcpp_standalone"),
    ("src/compiler/glsl/glcpp", "glcpp"),
    ("src/mesa", "mesa"),
    ("src/compiler/spirv", "vtn"),
    ("src/compiler/nir", "nir"),
    ("src/compiler", "compiler"),
    ("src/util", "mesa_util"),
    ("src/util/blake3", "blake3"),
    ("src/c11/impl", "mesa_util_c11"),
];

/// What those libraries take from the system, beyond the dependencies Meson found for Mesa.
const SYSTEM_LIBRARIES: &[&str] = &["stdc++", "m", "dl"];

/// Mesa's dependencies, by Meson's names, that the compiler libraries use; their link arguments
/// are read from Meson's introspection of the build.
const MESA_DEPENDENCIES: &[&str] = &["zlib", "threads"];
const INTROSPECTED_DEPENDENCIES: &str = "meson-info/intro-dependencies.json";
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
/// The converter's own sources (this crate's Rust and bridge, and the naga whose module it makes),
/// hashed into the converter's version, and the variable that hands the hash to the crate.
const CONVERTER_SOURCES: &[&str] = &["src", "bridge", "../../third_party/naga/src"];
const SOURCES_VARIABLE: &str = "WCONTAINERS_NIR_SOURCES";
const SEARCH_FLAG: &str = "-L";
const LIBRARY_FLAG: &str = "-l";
const LIBRARY_PREFIX: &str = "lib";
const STATIC_EXTENSION: &str = "a";

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    let build = &build;
    println!("{RERUN_IF_CHANGED}{BRIDGE_SOURCE}");
    println!("{RERUN_IF_CHANGED}{BRIDGE_HEADER}");
    println!("{RERUN_IF_CHANGED}{FLAT_SOURCE}");
    println!("{RERUN_IF_CHANGED}{FLAT_HEADER}");
    println!("{RERUN_IF_CHANGED}{GLSL_SOURCE}");
    println!("{RERUN_IF_CHANGED}{SPIRV_SOURCE}");
    println!("{RERUN_IF_CHANGED}{PRIVATE_HEADER}");
    let roots = [source_folder(build), build.clone()];
    let includes: Vec<PathBuf> = INCLUDES
        .iter()
        .flat_map(|folder| roots.iter().map(move |root| root.join(folder)))
        .collect();

    let standalone = env::var_os(STANDALONE_FEATURE).is_some();
    let mut bridge = cc::Build::new();
    // Mesa's headers warn under the flags cc adds by default; Mesa builds them without those.
    bridge
        .file(BRIDGE_SOURCE)
        .file(FLAT_SOURCE)
        .includes(&includes)
        .warnings(false);
    if standalone {
        bridge.file(SPIRV_SOURCE);
    }
    for flag in mesa_flags(build, FLAGS_FROM) {
        bridge.flag(flag);
    }
    bridge.compile(BRIDGE_LIBRARY);
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
    if !standalone {
        return;
    }

    let mut glsl = cc::Build::new();
    glsl.cpp(true)
        .file(GLSL_SOURCE)
        .includes(&includes)
        .warnings(false);
    for flag in mesa_flags(build, CPP_FLAGS_FROM) {
        glsl.flag(flag);
    }
    glsl.compile(GLSL_LIBRARY);

    for (folder, name) in LIBRARIES {
        println!("{LINK_SEARCH}{}", build.join(folder).display());
        println!("{LINK_STATIC}{name}");
    }
    mesa_dependencies(build);
    for name in SYSTEM_LIBRARIES {
        println!("{LINK_DYNAMIC}{name}");
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

/// Links what Meson found for the dependencies the compiler libraries use.
fn mesa_dependencies(build: &Path) {
    #[derive(serde::Deserialize)]
    struct Dependency {
        name: String,
        link_args: Vec<String>,
    }
    let path = build.join(INTROSPECTED_DEPENDENCIES);
    println!("{RERUN_IF_CHANGED}{}", path.display());
    let text = std::fs::read_to_string(&path).expect("Mesa's build has Meson's introspection");
    let found: Vec<Dependency> = serde_json::from_str(&text).expect("introspection is JSON");
    for dependency in found
        .iter()
        .filter(|d| MESA_DEPENDENCIES.contains(&d.name.as_str()))
    {
        for arg in &dependency.link_args {
            let file = Path::new(arg);
            if let Some(dir) = arg.strip_prefix(SEARCH_FLAG) {
                println!("{LINK_SEARCH}{dir}");
            } else if let Some(name) = arg.strip_prefix(LIBRARY_FLAG) {
                println!("{LINK_DYNAMIC}{name}");
            } else if file.is_absolute() {
                let stem = file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .expect("a library file");
                let name = stem.strip_prefix(LIBRARY_PREFIX).unwrap_or(stem);
                let dir = file.parent().expect("a library file has a folder");
                println!("{LINK_SEARCH}{}", dir.display());
                if file.extension().is_some_and(|e| e == STATIC_EXTENSION) {
                    println!("{LINK_STATIC}{name}");
                } else {
                    println!("{LINK_DYNAMIC}{name}");
                }
            } else {
                println!("{LINK_ARG}{arg}");
            }
        }
    }
}
