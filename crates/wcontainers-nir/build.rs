//! Links the pinned Mesa's compiler libraries and the bridge to them when `WCONTAINERS_MESA` names the
//! Meson build folder `mesa/build` made; without it the crate builds without the parts that read NIR.

use std::env;
use std::path::{Path, PathBuf};

const MESA_VARIABLE: &str = "WCONTAINERS_MESA";
const CHECK_CFG: &str = "cargo::rustc-check-cfg=cfg(mesa)";
const CFG: &str = "cargo::rustc-cfg=mesa";
const RERUN_IF_ENV_CHANGED: &str = "cargo::rerun-if-env-changed=";
const RERUN_IF_CHANGED: &str = "cargo::rerun-if-changed=";
const LINK_SEARCH: &str = "cargo::rustc-link-search=native=";
const LINK_STATIC: &str = "cargo::rustc-link-lib=static=";
const LINK_DYNAMIC: &str = "cargo::rustc-link-lib=dylib=";

const BRIDGE_SOURCE: &str = "bridge/bridge.c";
const BRIDGE_HEADER: &str = "bridge/bridge.h";
/// Candidate B's flattening of NIR.
const FLAT_SOURCE: &str = "bridge/flat.c";
const FLAT_HEADER: &str = "bridge/flat.h";
/// The GLSL front (C++, Mesa's GLSL compiler) and what the bridge's files share.
const GLSL_SOURCE: &str = "bridge/glsl.cpp";
const PRIVATE_HEADER: &str = "bridge/bridge_private.h";
const GLSL_LIBRARY: &str = "wcontainers_nir_glsl";
const BRIDGE_LIBRARY: &str = "wcontainers_nir_bridge";
/// Meson's record of the build, which names the source folder it was made from.
const MESON_INFO: &str = "meson-info/meson-info.json";
/// Mesa's own compile commands, and the file whose flags (defines and C standard) the bridge takes.
const COMPILE_COMMANDS: &str = "compile_commands.json";
const FLAGS_FROM: &str = "nir_to_spirv.c";
/// The C++ file of Mesa's GLSL compiler whose flags the GLSL front takes.
const CPP_FLAGS_FROM: &str = "standalone.cpp";
const DEFINE_FLAG: &str = "-D";
const STANDARD_FLAG: &str = "-std=";

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
const LINK_ARG: &str = "cargo::rustc-link-arg=";
/// What `mesa/build` records of the Mesa it built (the commit and the options), and the variable
/// that hands it to the crate, part of the converter's version.
const MADE_FROM: &str = "wcontainers-made-from";
const MESA_BUILD_VARIABLE: &str = "WCONTAINERS_MESA_MADE_FROM";
const RUSTC_ENV: &str = "cargo::rustc-env=";
/// Lavapipe, the corpus run's Vulkan reference, by its Meson target, and the variable naming its file.
const LAVAPIPE_TARGET: &str = "vulkan_lvp";
const LAVAPIPE_VARIABLE: &str = "WCONTAINERS_LAVAPIPE";
const INTROSPECTED_TARGETS: &str = "meson-info/intro-targets.json";
/// Between a variable and its value in `RUSTC_ENV`, and between the parts of the version.
const ASSIGN: &str = "=";
const VERSION_PARTS: &str = "+";
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
    println!("{RERUN_IF_CHANGED}{PRIVATE_HEADER}");
    let roots = [source_folder(build), build.clone()];
    let includes: Vec<PathBuf> = INCLUDES
        .iter()
        .flat_map(|folder| roots.iter().map(move |root| root.join(folder)))
        .collect();

    let mut bridge = cc::Build::new();
    let flags = mesa_flags(build, FLAGS_FROM);
    // Mesa's headers warn under the flags cc adds by default; Mesa builds them without those.
    bridge
        .file(BRIDGE_SOURCE)
        .file(FLAT_SOURCE)
        .includes(&includes)
        .warnings(false);
    for flag in &flags {
        bridge.flag(flag);
    }
    bridge.compile(BRIDGE_LIBRARY);
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
    let made_from = build.join(MADE_FROM);
    println!("{RERUN_IF_CHANGED}{}", made_from.display());
    let recorded = std::fs::read_to_string(&made_from).expect("mesa/build records what it built");
    let commit = recorded
        .lines()
        .next()
        .expect("the record starts with the commit");
    let options = blake3::hash(recorded.as_bytes()).to_hex();
    println!("{RUSTC_ENV}{MESA_BUILD_VARIABLE}{ASSIGN}{commit}{VERSION_PARTS}{options}");
    println!(
        "{RUSTC_ENV}{LAVAPIPE_VARIABLE}{ASSIGN}{}",
        target_file(build, LAVAPIPE_TARGET).display()
    );
    println!("{CFG}");
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

/// The source folder Meson made the build from.
fn source_folder(build: &Path) -> PathBuf {
    #[derive(serde::Deserialize)]
    struct Info {
        directories: Directories,
    }
    #[derive(serde::Deserialize)]
    struct Directories {
        source: PathBuf,
    }
    let path = build.join(MESON_INFO);
    let text = std::fs::read_to_string(&path).expect("Mesa's build has Meson's record");
    let info: Info = serde_json::from_str(&text).expect("Meson's record is JSON");
    info.directories.source
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

/// The defines and language standard Mesa compiled `file` with, from its compile commands.
fn mesa_flags(build: &Path, file: &str) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Command {
        file: String,
        command: String,
    }
    let path = build.join(COMPILE_COMMANDS);
    println!("{RERUN_IF_CHANGED}{}", path.display());
    let text = std::fs::read_to_string(&path).expect("Mesa's build has its compile commands");
    let commands: Vec<Command> = serde_json::from_str(&text).expect("compile commands are JSON");
    let command = commands
        .iter()
        .find(|c| c.file.ends_with(file))
        .unwrap_or_else(|| panic!("Mesa's build compiles {file}"));
    command
        .command
        .split_whitespace()
        .filter(|a| a.starts_with(DEFINE_FLAG) || a.starts_with(STANDARD_FLAG))
        .map(String::from)
        .collect()
}
