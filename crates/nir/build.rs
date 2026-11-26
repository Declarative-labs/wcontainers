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
    // Not Mesa's OpenGL library: the stand-ins give what the compiler takes of it, and ld.bfd would
    // take its state tracker too.
    ("src/gallium/drivers/zink", "zink"),
    ("src/compiler/glsl", "glsl_standalone"),
    ("src/compiler/glsl", "glsl"),
    ("src/compiler/glsl", "glsl_util"),
    ("src/compiler/glsl/glcpp", "glcpp_standalone"),
    ("src/compiler/glsl/glcpp", "glcpp"),
    ("src/compiler/spirv", "vtn"),
    ("src/compiler/nir", "nir"),
    ("src/compiler", "compiler"),
    ("src/util", "mesa_util"),
    ("src/util/blake3", "blake3"),
    ("src/c11/impl", "mesa_util_c11"),
];

/// Mesa's BLAKE3 library. Its inner C functions (`blake3_hash_many_neon`,
/// `blake3_compress_in_place_portable`, ...) have the names the blake3 crate's own C and assembly
/// code exports, so it is linked from a copy in which they carry `RENAMED_PREFIX`, their calls among
/// themselves renamed with them. Its outer functions, which Mesa's other libraries call and the crate
/// does not define, keep their names.
const MESA_BLAKE3: &str = "blake3";
const RENAMED_BLAKE3: &str = "wcontainers_nir_mesa_blake3";
const RENAMED_PREFIX: &str = "wcontainers_nir_mesa_";
const INNER_PREFIX: &str = "blake3_";
const OUTER_PREFIXES: &[&str] = &["blake3_hasher_", "blake3_version"];
/// The tools that list an archive's symbols and rename an object's, and the variables that may name
/// others.
const NM: &str = "nm";
const NM_VARIABLE: &str = "NM";
const OBJCOPY: &str = "objcopy";
const OBJCOPY_VARIABLE: &str = "OBJCOPY";
/// Every defined global symbol of an archive, one line each, its name first.
const NM_ARGUMENTS: &[&str] = &["--defined-only", "--extern-only", "--format=posix"];
const REDEFINE_FLAG: &str = "--redefine-syms=";
const RENAMES_FILE: &str = "mesa_blake3_renames.txt";
const RENAMED_OBJECTS: &str = "mesa_blake3_objects";
/// The archiver, which lists an archive's members and makes a new archive (with its index).
const AR: &str = "ar";
const AR_VARIABLE: &str = "AR";
const AR_LIST: &str = "t";
const AR_CREATE: &str = "crs";
const TARGET_ENV_VARIABLE: &str = "CARGO_CFG_TARGET_ENV";
const MSVC_ENV: &str = "msvc";
const OUT_DIR_VARIABLE: &str = "OUT_DIR";

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

    // The Visual C++ tools have no nm or objcopy; there Mesa's BLAKE3 is linked as Meson built it.
    let msvc = env::var(TARGET_ENV_VARIABLE).is_ok_and(|e| e == MSVC_ENV);
    for (folder, name) in LIBRARIES {
        if *name == MESA_BLAKE3 && !msvc {
            let renamed = renamed_blake3(&build.join(folder));
            println!("{LINK_SEARCH}{}", renamed.display());
            println!("{LINK_STATIC}{RENAMED_BLAKE3}");
            continue;
        }
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

/// Writes the copy of Mesa's BLAKE3 library in `folder` whose inner functions carry
/// `RENAMED_PREFIX` into the build script's output folder, and returns that folder.
fn renamed_blake3(folder: &Path) -> PathBuf {
    let archive = folder.join(format!("{LIBRARY_PREFIX}{MESA_BLAKE3}.{STATIC_EXTENSION}"));
    println!("{RERUN_IF_CHANGED}{}", archive.display());
    let out = PathBuf::from(env::var_os(OUT_DIR_VARIABLE).expect("cargo names the output folder"));
    let listed = std::process::Command::new(tool(NM_VARIABLE, NM))
        .args(NM_ARGUMENTS)
        .arg(&archive)
        .output()
        .expect("nm runs");
    assert!(listed.status.success(), "nm lists {}", archive.display());
    let mut inner: Vec<&str> = std::str::from_utf8(&listed.stdout)
        .expect("symbol names are text")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| {
            name.starts_with(INNER_PREFIX) && !OUTER_PREFIXES.iter().any(|p| name.starts_with(p))
        })
        .collect();
    inner.sort_unstable();
    inner.dedup();
    assert!(
        !inner.is_empty(),
        "Mesa's BLAKE3 defines its inner functions"
    );
    let renames: String = inner
        .iter()
        .map(|name| format!("{name} {RENAMED_PREFIX}{name}\n"))
        .collect();
    let renames_file = out.join(RENAMES_FILE);
    std::fs::write(&renames_file, renames).expect("the renames are written");
    // Meson writes thin archives, which objcopy cannot copy: each member object is renamed on its
    // own and the copies are archived anew.
    let members = std::process::Command::new(tool(AR_VARIABLE, AR))
        .arg(AR_LIST)
        .arg(&archive)
        .output()
        .expect("ar runs");
    assert!(members.status.success(), "ar lists {}", archive.display());
    let objects = out.join(RENAMED_OBJECTS);
    std::fs::create_dir_all(&objects).expect("the folder of renamed objects is made");
    let mut renamed_objects = Vec::new();
    for (index, member) in std::str::from_utf8(&members.stdout)
        .expect("member names are text")
        .lines()
        .enumerate()
    {
        let source = folder.join(member);
        let name = Path::new(member).file_name().expect("a member names a file");
        let copy = objects.join(format!("{index}-{}", name.to_string_lossy()));
        let status = std::process::Command::new(tool(OBJCOPY_VARIABLE, OBJCOPY))
            .arg(format!("{REDEFINE_FLAG}{}", renames_file.display()))
            .arg(&source)
            .arg(&copy)
            .status()
            .expect("objcopy runs");
        assert!(status.success(), "objcopy renames {}", source.display());
        renamed_objects.push(copy);
    }
    let renamed = out.join(format!(
        "{LIBRARY_PREFIX}{RENAMED_BLAKE3}.{STATIC_EXTENSION}"
    ));
    // `ar` adds to an archive that is there, so a rebuild starts from none.
    let _ = std::fs::remove_file(&renamed);
    let status = std::process::Command::new(tool(AR_VARIABLE, AR))
        .arg(AR_CREATE)
        .arg(&renamed)
        .args(&renamed_objects)
        .status()
        .expect("ar runs");
    assert!(status.success(), "ar archives the renamed objects");
    out
}

/// The tool a variable names, else its usual name.
fn tool(variable: &str, name: &str) -> std::ffi::OsString {
    println!("{RERUN_IF_ENV_CHANGED}{variable}");
    env::var_os(variable).unwrap_or_else(|| name.into())
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
