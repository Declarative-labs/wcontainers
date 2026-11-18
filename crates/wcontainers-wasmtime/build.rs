//! The translator's build identity: a hash of the sources of every crate the emitted code depends on
//! and of their locked wasm-encoder, so a change to any of them changes the cache's keys.

use std::path::{Path, PathBuf};

/// The translator and what every guest stands on.
const BASE: [&str; 3] = [
    "wcontainers-translator",
    "wcontainers-guest-cpu",
    "wcontainers-abi",
];

fn files(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, into);
        } else {
            into.push(path);
        }
    }
}

/// The names of a workspace crate's normal dependencies.
fn dependencies(manifest: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(manifest).expect("a crate manifest");
    let document: toml_edit::DocumentMut = text.parse().expect("a crate manifest parses");
    document
        .get("dependencies")
        .and_then(|item| item.as_table_like())
        .map(|table| table.iter().map(|(name, _)| name.to_owned()).collect())
        .unwrap_or_default()
}

/// Every guest of the workspace: a crate built on the guest interface that the translator does not
/// stand on and that does not stand on the translator.
fn guests(crates: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(crates)
        .expect("the crates directory")
        .flatten()
    {
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let needs = dependencies(&manifest);
        let has = |name: &str| needs.iter().any(|n| n == name);
        if !BASE.contains(&name.as_str())
            && has("wcontainers-guest-cpu")
            && !has("wcontainers-translator")
        {
            names.push(name);
        }
    }
    names.sort();
    names
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let crates = manifest.parent().expect("the crates directory");
    // A crate added or removed changes the guests.
    println!("cargo:rerun-if-changed={}", crates.display());
    let mut hasher = blake3::Hasher::new();
    let names = BASE
        .iter()
        .map(|name| (*name).to_owned())
        .chain(guests(crates));
    for name in names {
        let root = crates.join(&name);
        let mut paths = vec![root.join("Cargo.toml")];
        files(&root.join("src"), &mut paths);
        paths.sort();
        for path in paths {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path.strip_prefix(crates).expect("inside the crates");
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(&std::fs::read(&path).expect("a source file"));
        }
        println!("cargo:rerun-if-changed={}", root.join("src").display());
    }
    let lock = crates.join("../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let lock = std::fs::read_to_string(&lock).expect("the workspace's Cargo.lock");
    for package in lock.split("[[package]]") {
        if package.contains("\nname = \"wasm-encoder\"\n") {
            hasher.update(package.as_bytes());
        }
    }
    println!(
        "cargo:rustc-env=WCONTAINERS_TRANSLATOR_BUILD={}",
        hasher.finalize().to_hex()
    );
}
