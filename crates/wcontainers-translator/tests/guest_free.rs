//! The translator is written once against the guest CPU interface: it depends on no CPU crate and its
//! source names none, so everything it knows of a guest comes through `wcontainers_guest_cpu::Guest`. Of the
//! shared abi crate, which also holds ARM64's register block and its offsets, it may name only what is true
//! of every guest: the page size and the page permissions.

use std::path::Path;

/// The crates of this repository the translator may name.
const ALLOWED: [&str; 2] = ["wcontainers_abi", "wcontainers_guest_cpu"];
/// What it may name of the abi crate, always by its full path.
const ABI_ALLOWED: [&str; 2] = ["PAGE_SIZE", "Permissions"];

/// Every `wcontainers_*` crate name in `text` that is not allowed and, in Rust source, every use of the abi
/// crate other than `wcontainers_abi::<an allowed item>`, so that it cannot be named under another name.
fn named(text: &str, rust: bool) -> Vec<String> {
    let normal = text.replace('-', "_");
    let word = |from: &str| -> String {
        from.chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect()
    };
    normal
        .match_indices("wcontainers_")
        .filter_map(|(at, _)| {
            let name = word(&normal[at..]);
            if !ALLOWED.contains(&name.as_str()) {
                return Some(name);
            }
            let rest = &normal[at + name.len()..];
            let item = rest.strip_prefix("::").map(word);
            match (name.as_str(), item) {
                ("wcontainers_abi", Some(item)) if ABI_ALLOWED.contains(&item.as_str()) => None,
                ("wcontainers_abi", _) if rust => {
                    Some(format!("{name}{}", rest.lines().next().unwrap_or("")))
                }
                _ => None,
            }
        })
        .collect()
}

#[test]
fn the_abi_check_finds_what_it_should() {
    let rust = |text| named(text, true);
    assert!(rust("wcontainers_abi::PAGE_SIZE wcontainers_abi::Permissions").is_empty());
    assert!(rust("use wcontainers_abi::Permissions as P;").is_empty());
    assert_eq!(rust("x: wcontainers_abi::RegisterBlock").len(), 1);
    assert_eq!(
        rust("use wcontainers_abi::{PAGE_SIZE, STOP_FLAG_OFFSET};").len(),
        1
    );
    assert_eq!(rust("use wcontainers_abi;").len(), 1);
    assert_eq!(
        rust("use wcontainers_abi as abi;\nfn f(b: abi::RegisterBlock) {}").len(),
        1
    );
    assert_eq!(rust("extern crate wcontainers_abi as abi;").len(), 1);
    assert_eq!(rust("wcontainers_arm64::Arm64").len(), 1);
    assert!(named("wcontainers-abi.workspace = true", false).is_empty());
}

fn sources(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn the_translator_names_no_guest_cpu_crate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    // The package's own name is the one allowed line that names it.
    let dependencies: String = manifest
        .lines()
        .filter(|line| !line.starts_with("name ="))
        .collect::<Vec<_>>()
        .join("\n");
    let mut found: Vec<String> = named(&dependencies, false)
        .into_iter()
        .map(|name| format!("Cargo.toml: {name}"))
        .collect();
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    assert!(files.len() > 5, "the sources were found");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        found.extend(
            named(&text, true)
                .into_iter()
                .map(|name| format!("{}: {name}", file.display())),
        );
    }
    assert!(found.is_empty(), "the translator names {found:?}");
}

#[test]
fn the_check_finds_a_cpu_crate() {
    assert_eq!(
        named(
            "use wcontainers_arm64::Instruction;\nwcontainers-x86 = { path = \"x\" }",
            false
        ),
        ["wcontainers_arm64", "wcontainers_x86"]
    );
    assert!(
        named(
            "use wcontainers_guest_cpu::Guest; wcontainers-abi.workspace = true",
            false
        )
        .is_empty()
    );
}
