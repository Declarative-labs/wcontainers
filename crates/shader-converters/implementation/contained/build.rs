//! Checks that the committed module (`converters.wasm`) was built from the converters' sources as they are, by the
//! hash of those sources it was recorded with (`converters.blake3`), and gives that hash to the crates that depend on
//! this one (`DEP_WCS_CONTAINED_SOURCES`). The module itself is built by the linked implementation's build script,
//! natively with `WASI_SDK_PATH` set; the sources and their hash are that script's.

use std::path::{Path, PathBuf};

/// The folder of the implementation that holds the converters' sources, beside this one.
const LINKED: &str = "../linked";

/// The module's recorded hash of its sources.
const MODULE_SOURCES: &str = "converters.blake3";

/// How the module is built again after its sources change.
const REBUILD: &str = "build wcontainers-shader-converters-linked natively with WASI_SDK_PATH naming wasi-sdk 25, \
                       which says what to copy";

/// Left out of sources before they are hashed, so every checkout reads the same.
const CARRIAGE_RETURN: u8 = b'\r';

fn main() {
    let linked = Path::new(LINKED);
    let root = linked.join("dxbc-spirv");
    let dxil = linked.join("dxil-spirv");
    // The linked build script's list, read from beside it.
    let cpp = [
        (root.join("dxbc"), true),
        (root.join("ir"), true),
        (root.join("util"), true),
        (dxil.clone(), false),
        (dxil.join("bc"), true),
        (dxil.join("debug"), true),
        (dxil.join("util"), true),
        (dxil.join("opcodes"), true),
        (dxil.join("third_party/glslang-spirv"), true),
        (dxil.join("third_party/bc-decoder"), true),
        (
            dxil.join("third_party/spirv-headers/include/spirv/unified1"),
            true,
        ),
        (linked.join("cpp"), true),
        (linked.join("converter-patches"), true),
        (linked.join("build.rs"), false),
    ];
    let mut hasher = blake3::Hasher::new();
    hash_files(&mut hasher, linked, &cpp);
    let sources = hasher.finalize().to_hex().to_string();
    let recorded = std::fs::read_to_string(MODULE_SOURCES).unwrap_or_default();
    assert!(
        recorded.trim() == sources,
        "the converters' sources changed since converters.wasm was built: {REBUILD}"
    );
    println!("cargo:sources={sources}");
    println!("cargo:rerun-if-changed={MODULE_SOURCES}");
    println!("cargo:rerun-if-changed={LINKED}");
}

/// Hashes the files under each `(path, recursive)` (a path may be one file) by their name below `base` and their
/// contents, carriage returns left out, as the linked build script does. A directory listed without its
/// subdirectories (a submodule's top) gives only its files with an extension, as its sources all have one: git's files
/// there have none, among them the submodule's `.git` link, which depends on where the checkout keeps git's data.
fn hash_files(hasher: &mut blake3::Hasher, base: &Path, paths: &[(PathBuf, bool)]) {
    for file in list_files(paths) {
        let name = file
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let bytes: Vec<u8> = std::fs::read(&file)
            .unwrap()
            .into_iter()
            .filter(|&b| b != CARRIAGE_RETURN)
            .collect();
        hasher.update(name.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
}

/// Returns the files under each `(path, recursive)`, sorted, as `hash_files` takes them.
fn list_files(paths: &[(PathBuf, bool)]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for (path, recursive) in paths {
        if path.is_file() {
            files.push(path.clone());
            continue;
        }
        let mut stack = vec![path.clone()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if *recursive {
                        stack.push(path);
                    }
                } else if *recursive || path.extension().is_some() {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}
