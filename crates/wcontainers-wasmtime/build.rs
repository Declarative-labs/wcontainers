//! The translator's build identity: a hash of the sources of every crate the emitted code depends on
//! and of their locked wasm-encoder, so a change to any of them changes the cache's keys.

use std::path::{Path, PathBuf};

/// The crates whose sources decide the emitted code.
const CRATES: [&str; 4] = [
    "wcontainers-translator",
    "wcontainers-guest-cpu",
    "wcontainers-arm64",
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

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let crates = manifest.parent().expect("the crates directory");
    let mut hasher = blake3::Hasher::new();
    for name in CRATES {
        let root = crates.join(name);
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
