//! Lists the baked files of each folder of `samples/`, so that every file is compiled and checked.
//!
//! For each folder it writes `<folder>.rs` into the build's output: a module for each file and `SAMPLES`, every
//! file's `SAMPLE` in the order of the file names, which the crate includes where it names the folder's types.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

const SAMPLES: &str = "samples";
const RUST_EXTENSION: &str = "rs";

fn main() {
    println!("cargo::rerun-if-changed={SAMPLES}");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLES);
    for folder in list_sorted(&samples) {
        println!("cargo::rerun-if-changed={}", folder.display());
        let mut listing = String::new();
        let mut names = Vec::new();
        for file in list_sorted(&folder) {
            assert!(
                file.extension()
                    .is_some_and(|extension| extension == RUST_EXTENSION),
                "{} is not a baked file",
                file.display()
            );
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                panic!("{} has no file name in UTF-8", file.display());
            };
            let name = stem.replace('-', "_");
            assert!(
                name.chars().all(|character| character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '_'),
                "{} must be named in lower case, digits and hyphens",
                file.display()
            );
            writeln!(
                listing,
                "#[path = {:?}]\nmod {name};",
                file.display().to_string()
            )
            .expect("writing to a string succeeds");
            names.push(name);
        }
        let entries: Vec<String> = names.iter().map(|name| format!("{name}::SAMPLE")).collect();
        writeln!(
            listing,
            "\n/// Every baked sample of the folder, in the order of the file names.\n\
             pub const SAMPLES: &[Sample] = &[{}];",
            entries.join(", ")
        )
        .expect("writing to a string succeeds");
        let Some(folder_name) = folder.file_name() else {
            panic!("{} has no name", folder.display());
        };
        let listing_file = output.join(folder_name).with_extension(RUST_EXTENSION);
        fs::write(listing_file, listing).expect("the listing is writable");
    }
}

/// Returns the entries of a folder, sorted by name.
fn list_sorted(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        panic!("{} is not readable", folder.display());
    };
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("a folder entry is readable").path())
        .collect();
    paths.sort();
    paths
}
