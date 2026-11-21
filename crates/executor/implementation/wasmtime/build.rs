//! The translator's build identity: a hash of the files of every workspace crate the emitted code
//! depends on (the translator, every guest and what they stand on) and of the locked entry of every
//! other crate they depend on, so a change to any of them changes the cache's keys. The workspace comes
//! from `cargo metadata --no-deps` and the rest from the lock file, so nothing is fetched.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use build_rs::{input, output};
use cargo_lock::Lockfile;
use cargo_metadata::{DependencyKind, MetadataCommand, Package};

/// The variable the build identity is handed to the library in.
const TRANSLATOR_BUILD_VARIABLE: &str = "WCONTAINERS_TRANSLATOR_BUILD";

/// The translator's package.
const TRANSLATOR: &str = "wcontainers-translator";

/// The guest interface's package, which every guest is built on.
const GUEST_INTERFACE: &str = "wcontainers-guest-cpu";

/// A package's directories that hold only targets never built into a dependent, and build output.
const NOT_BUILT_INTO_DEPENDENTS: [&str; 4] = ["tests", "benches", "examples", "target"];

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

/// Whether a workspace crate depends on the named package to build its code (not only its tests).
fn builds_on(package: &Package, name: &str) -> bool {
    package
        .dependencies
        .iter()
        .any(|d| d.name == name && d.kind != DependencyKind::Development)
}

fn main() {
    let metadata = MetadataCommand::new()
        .manifest_path(input::cargo_manifest_path())
        .no_deps()
        .exec()
        .expect("cargo metadata");
    let root = metadata.workspace_root.as_std_path();
    let lock_path = root.join("Cargo.lock");
    // A crate added to the workspace or removed from it changes the lock.
    output::rerun_if_changed(&lock_path);
    let lock = Lockfile::load(&lock_path).expect("the lock file");
    let members: HashMap<&str, &Package> = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| (p.name.as_str(), p))
        .collect();
    for member in members.values() {
        output::rerun_if_changed(&member.manifest_path);
    }
    let local = |name: &str| {
        lock.packages
            .iter()
            .position(|p| p.source.is_none() && p.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} is in the lock"))
    };
    // The translator, and every guest: a crate built on the guest interface that does not stand on
    // the translator.
    let mut work: Vec<usize> = members
        .values()
        .filter(|p| {
            p.name.as_str() == TRANSLATOR
                || (builds_on(p, GUEST_INTERFACE) && !builds_on(p, TRANSLATOR))
        })
        .map(|p| local(p.name.as_str()))
        .collect();
    let mut seen = BTreeSet::new();
    while let Some(index) = work.pop() {
        if !seen.insert(index) {
            continue;
        }
        let package = &lock.packages[index];
        let member = package
            .source
            .is_none()
            .then(|| members[package.name.as_str()]);
        for dependency in &package.dependencies {
            if member.is_some_and(|m| !builds_on(m, dependency.name.as_str())) {
                continue;
            }
            let found = lock
                .packages
                .iter()
                .position(|p| dependency.matches(p))
                .unwrap_or_else(|| panic!("{} is in the lock", dependency.name));
            work.push(found);
        }
    }
    let mut hasher = blake3::Hasher::new();
    for index in seen {
        let package = &lock.packages[index];
        if let Some(source) = &package.source {
            let checksum = package.checksum.as_ref().map(ToString::to_string);
            let entry = format!(
                "{} {} {source} {}",
                package.name,
                package.version,
                checksum.unwrap_or_default()
            );
            hasher.update(entry.as_bytes());
            continue;
        }
        // Every file of the package's directory, cargo's bound of what a package is built from (its
        // sources and whatever they include), but for the directories it never builds into a
        // dependent. Each entry is watched (a directory with every file in it); an entry added
        // beside the manifest is reached only through a changed source, which is watched.
        let member = members[package.name.as_str()];
        let directory = member
            .manifest_path
            .parent()
            .expect("a manifest's directory")
            .as_std_path();
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(directory)
            .expect("a package's directory")
            .flatten()
        {
            let path = entry.path();
            if NOT_BUILT_INTO_DEPENDENTS.contains(&entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            output::rerun_if_changed(&path);
            if path.is_dir() {
                files(&path, &mut paths);
            } else {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).expect("inside the workspace");
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(&std::fs::read(&path).expect("a package's file"));
        }
    }
    output::rustc_env(
        TRANSLATOR_BUILD_VARIABLE,
        hasher.finalize().to_hex().as_str(),
    );
}
