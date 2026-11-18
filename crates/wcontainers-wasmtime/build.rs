//! The translator's build identity: a hash of the sources of every workspace crate the emitted code
//! depends on (the translator, every guest and what they stand on) and of the locked entry of every
//! other crate they depend on, so a change to any of them changes the cache's keys.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item};

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

fn read(path: &Path) -> DocumentMut {
    println!("cargo:rerun-if-changed={}", path.display());
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The package names of a workspace crate's normal and build dependencies (what its built code is
/// made from; its tests' dependencies are not).
fn dependencies(manifest: &DocumentMut) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for section in ["dependencies", "build-dependencies"] {
        let Some(table) = manifest.get(section).and_then(Item::as_table_like) else {
            continue;
        };
        for (key, item) in table.iter() {
            let package = item
                .as_table_like()
                .and_then(|t| t.get("package"))
                .and_then(Item::as_str)
                .unwrap_or(key);
            names.insert(package.to_owned());
        }
    }
    names
}

/// One locked package: its name and version, whether it is a workspace crate (no source), its name,
/// version, source and checksum together, and what it depends on as the lock names them.
struct Locked {
    name: String,
    version: String,
    workspace: bool,
    entry: String,
    dependencies: Vec<String>,
}

fn locked(lock: &DocumentMut) -> Vec<Locked> {
    let packages = lock
        .get("package")
        .and_then(Item::as_array_of_tables)
        .expect("the lock's packages");
    packages
        .iter()
        .map(|package| {
            let text = |key: &str| package.get(key).and_then(Item::as_str).map(str::to_owned);
            Locked {
                name: text("name").expect("a locked name"),
                version: text("version").expect("a locked version"),
                workspace: package.get("source").is_none(),
                entry: ["name", "version", "source", "checksum"]
                    .map(|key| text(key).unwrap_or_default())
                    .join(" "),
                dependencies: package
                    .get("dependencies")
                    .and_then(Item::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// The locked package a lock's dependency names: "name", "name version" or "name version (source)".
fn resolve(packages: &[Locked], named: &str) -> usize {
    let mut words = named.split(' ');
    let name = words.next().expect("a name");
    let version = words.next();
    packages
        .iter()
        .position(|p| p.name == name && version.is_none_or(|v| v == p.version))
        .unwrap_or_else(|| panic!("{named} is in the lock"))
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let crates = manifest.parent().expect("the crates directory");
    // A crate added to the workspace or removed from it changes the lock.
    let lock = locked(&read(&crates.join("../Cargo.lock")));
    let mut manifests = BTreeMap::new();
    for entry in std::fs::read_dir(crates)
        .expect("the crates directory")
        .flatten()
    {
        let path = entry.path().join("Cargo.toml");
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            manifests.insert(name, read(&path));
        }
    }
    // The translator, and every guest: a crate built on the guest interface that does not stand on
    // the translator.
    let mut from: Vec<&str> = vec!["wcontainers-translator"];
    for (name, document) in &manifests {
        let needs = dependencies(document);
        if name != "wcontainers-translator"
            && needs.contains("wcontainers-guest-cpu")
            && !needs.contains("wcontainers-translator")
        {
            from.push(name);
        }
    }
    let mut seen = BTreeSet::new();
    let mut work: Vec<usize> = from.iter().map(|name| resolve(&lock, name)).collect();
    while let Some(index) = work.pop() {
        if !seen.insert(index) {
            continue;
        }
        let package = &lock[index];
        let built_from = package
            .workspace
            .then(|| dependencies(&manifests[&package.name]));
        for named in &package.dependencies {
            let dependency = resolve(&lock, named);
            if built_from
                .as_ref()
                .is_none_or(|names| names.contains(&lock[dependency].name))
            {
                work.push(dependency);
            }
        }
    }
    let mut hasher = blake3::Hasher::new();
    for index in seen {
        let package = &lock[index];
        if !package.workspace {
            hasher.update(package.entry.as_bytes());
            continue;
        }
        let root = crates.join(&package.name);
        let mut paths = vec![root.join("Cargo.toml")];
        files(&root.join("src"), &mut paths);
        paths.sort();
        for path in paths {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path.strip_prefix(crates).expect("inside the crates");
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(&std::fs::read(&path).expect("a source file"));
        }
        // A source file added.
        println!("cargo:rerun-if-changed={}", root.join("src").display());
    }
    println!(
        "cargo:rustc-env=WCONTAINERS_TRANSLATOR_BUILD={}",
        hasher.finalize().to_hex()
    );
}
