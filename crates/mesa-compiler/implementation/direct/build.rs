//! Builds our C bridge over Mesa's compiler (bridge/, moved here from crates/nir) and links the pinned Mesa's
//! compiler libraries when `WCONTAINERS_MESA` names the Meson build folder crates/nir/mesa/build made; without it
//! nothing is built and the crate's calls are left for the program it is linked into to resolve. Without the
//! `standalone` feature only the bridge is built: Mesa is linked by the program the crate is linked into (the
//! OpenGL driver inside Mesa's own library). Taken from crates/nir/build.rs as graphics-implementations
//! 7e53ee79e has it, less what only crates/nir hands its own code (the converter's version, the reference
//! renderers' files).

use std::env;
use std::path::{Path, PathBuf};

#[path = "../../../nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{CFG, CHECK_CFG, LINK_ARG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, mesa_flags, source_folder};

/// The variable naming SPIRV-Tools' development folder (its headers, and the pkg-config file naming its library
/// folder), whose validator the bridge runs on every module before Mesa's reader (bridge/spirv.c).
const SPIRV_TOOLS_VARIABLE: &str = "WCONTAINERS_SPIRV_TOOLS";
const SPIRV_TOOLS_LIBRARY: &str = "SPIRV-Tools";
const SPIRV_TOOLS_PKG_CONFIG: &str = "lib/pkgconfig/SPIRV-Tools.pc";
const LIBRARY_DIRECTORY_LINE: &str = "libdir=";
const INCLUDE_FOLDER: &str = "include";

const LINK_SEARCH: &str = "cargo::rustc-link-search=native=";
const LINK_STATIC: &str = "cargo::rustc-link-lib=static=";
const LINK_DYNAMIC: &str = "cargo::rustc-link-lib=dylib=";

/// Set by cargo when the `standalone` feature is on.
const STANDALONE_FEATURE: &str = "CARGO_FEATURE_STANDALONE";
const BRIDGE_SOURCE: &str = "bridge/bridge.c";
/// The SPIR-V step: Mesa's SPIR-V reader.
const SPIRV_SOURCE: &str = "bridge/spirv.c";
const BRIDGE_HEADER: &str = "bridge/bridge.h";
/// Candidate B's flattening of NIR.
const FLAT_SOURCE: &str = "bridge/flat.c";
const FLAT_HEADER: &str = "bridge/flat.h";
/// The GLSL front (C++, Mesa's GLSL compiler) and what the bridge's files share.
const GLSL_SOURCE: &str = "bridge/glsl.cpp";
const PRIVATE_HEADER: &str = "bridge/bridge_private.h";
/// The single steps, which only the bridge's own files call.
const STEPS_HEADER: &str = "bridge/steps.h";
const GLSL_LIBRARY: &str = "wcontainers_mesa_compiler_glsl";
const BRIDGE_LIBRARY: &str = "wcontainers_mesa_compiler_bridge";
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
    "src/mesa",
    "src/mapi",
    "src/util",
    "src/util/format",
];

/// The static libraries, by their folder in Mesa's build, in link order (users before what they use).
const LIBRARIES: &[(&str, &str)] = &[
    // Not Mesa's OpenGL library: the stand-ins give what the compiler takes of it, and ld.bfd would
    // take its state tracker too.
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
const RENAMED_BLAKE3: &str = "wcontainers_mesa_compiler_blake3";
const RENAMED_PREFIX: &str = "wcontainers_mesa_compiler_";
const INNER_PREFIX: &str = "blake3_";
const OUTER_PREFIXES: &[&str] = &["blake3_hasher_", "blake3_version"];
/// BLAKE3's x86-64 assembly defines each function a second time with a leading underscore (the
/// name macOS gives C functions), and the crate's copy of it does too.
const ASSEMBLY_UNDERSCORE: char = '_';
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
    println!("{RERUN_IF_CHANGED}{STEPS_HEADER}");
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
        bridge.file(SPIRV_SOURCE).include(spirv_tools().join(INCLUDE_FOLDER));
    }
    for flag in mesa_flags(build, FLAGS_FROM) {
        bridge.flag(flag);
    }
    bridge.compile(BRIDGE_LIBRARY);
    if standalone {
        // After the bridge, which calls it: SPIRV-Tools' validator, statically.
        println!("{LINK_SEARCH}{}", spirv_tools_libraries().display());
        println!("{LINK_STATIC}{SPIRV_TOOLS_LIBRARY}");
    }
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
            let bare = name.trim_start_matches(ASSEMBLY_UNDERSCORE);
            bare.starts_with(INNER_PREFIX) && !OUTER_PREFIXES.iter().any(|p| bare.starts_with(p))
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
        let name = Path::new(member)
            .file_name()
            .expect("a member names a file");
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

/// SPIRV-Tools' development folder, which the build needs where the SPIR-V steps are built.
fn spirv_tools() -> PathBuf {
    println!("{RERUN_IF_ENV_CHANGED}{SPIRV_TOOLS_VARIABLE}");
    env::var_os(SPIRV_TOOLS_VARIABLE)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{SPIRV_TOOLS_VARIABLE} names SPIRV-Tools' development folder (nixpkgs#spirv-tools.dev)"))
}

/// The folder of SPIRV-Tools' libraries, as its pkg-config file names it.
fn spirv_tools_libraries() -> PathBuf {
    let file = spirv_tools().join(SPIRV_TOOLS_PKG_CONFIG);
    let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    text.lines()
        .find_map(|line| line.strip_prefix(LIBRARY_DIRECTORY_LINE))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{} names its library folder", file.display()))
}
