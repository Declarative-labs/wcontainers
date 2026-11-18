//! The translator's build identity: a hash of the sources of every local crate the emitted code depends
//! on (the translator, every guest and what they stand on) and of the id of every other crate they
//! depend on, so a change to any of them changes the cache's keys.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand, Package, PackageId};

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

/// Whether a package depends on the named one to build its code (not only its tests).
fn builds_on(package: &Package, name: &str) -> bool {
    package
        .dependencies
        .iter()
        .any(|d| d.name == name && d.kind != DependencyKind::Development)
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let metadata = MetadataCommand::new()
        .manifest_path(manifest.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .other_options(["--locked".to_owned(), "--offline".to_owned()])
        .exec()
        .expect("cargo metadata");
    let root = metadata.workspace_root.as_std_path();
    // A crate added to the workspace or removed from it changes the lock.
    println!(
        "cargo:rerun-if-changed={}",
        root.join("Cargo.lock").display()
    );
    for member in metadata.workspace_packages() {
        println!("cargo:rerun-if-changed={}", member.manifest_path);
    }
    // The translator, and every guest: a crate built on the guest interface that does not stand on
    // the translator.
    let from: Vec<&PackageId> = metadata
        .workspace_packages()
        .into_iter()
        .filter(|p| {
            p.name.as_str() == "wcontainers-translator"
                || (builds_on(p, "wcontainers-guest-cpu")
                    && !builds_on(p, "wcontainers-translator"))
        })
        .map(|p| &p.id)
        .collect();
    let resolve = metadata.resolve.as_ref().expect("a resolved graph");
    let nodes: HashMap<&PackageId, _> = resolve.nodes.iter().map(|n| (&n.id, n)).collect();
    let mut seen = BTreeSet::new();
    let mut work = from;
    while let Some(id) = work.pop() {
        if !seen.insert(id) {
            continue;
        }
        for dependency in &nodes[id].deps {
            let built_from = dependency
                .dep_kinds
                .iter()
                .any(|k| k.kind != DependencyKind::Development);
            if built_from {
                work.push(&dependency.pkg);
            }
        }
    }
    let mut hasher = blake3::Hasher::new();
    for id in seen {
        let package = &metadata[id];
        if package.source.is_some() {
            hasher.update(id.repr.as_bytes());
            continue;
        }
        let directory = package
            .manifest_path
            .parent()
            .expect("a manifest's directory")
            .as_std_path();
        let mut paths = vec![package.manifest_path.clone().into_std_path_buf()];
        files(&directory.join("src"), &mut paths);
        paths.sort();
        for path in paths {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path.strip_prefix(root).expect("inside the workspace");
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(&std::fs::read(&path).expect("a source file"));
        }
        // A source file added.
        println!("cargo:rerun-if-changed={}", directory.join("src").display());
    }
    println!(
        "cargo:rustc-env=WCONTAINERS_TRANSLATOR_BUILD={}",
        hasher.finalize().to_hex()
    );
}
