//! The translator is written once against the guest CPU interface: it depends on no CPU crate and its
//! source names none, so everything it knows of a guest comes through `wcontainers_guest_cpu::Guest`.

use std::path::Path;

/// The crates of this repository the translator may name.
const ALLOWED: [&str; 2] = ["wcontainers_abi", "wcontainers_guest_cpu"];

/// Every `wcontainers_*` crate name in `text` that is not allowed.
fn named(text: &str) -> Vec<String> {
    let normal = text.replace('-', "_");
    normal
        .match_indices("wcontainers_")
        .map(|(at, _)| {
            normal[at..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|name| !ALLOWED.contains(&name.as_str()))
        .collect()
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
    let mut found: Vec<String> = named(&dependencies)
        .into_iter()
        .map(|name| format!("Cargo.toml: {name}"))
        .collect();
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    assert!(files.len() > 5, "the sources were found");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        found.extend(
            named(&text)
                .into_iter()
                .map(|name| format!("{}: {name}", file.display())),
        );
    }
    assert!(found.is_empty(), "the translator names {found:?}");
}

#[test]
fn the_check_finds_a_cpu_crate() {
    assert_eq!(
        named("use wcontainers_arm64::Instruction;\nwcontainers-x86 = { path = \"x\" }"),
        ["wcontainers_arm64", "wcontainers_x86"]
    );
    assert!(named("use wcontainers_guest_cpu::Guest; wcontainers-abi.workspace = true").is_empty());
}
