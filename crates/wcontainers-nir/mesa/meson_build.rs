//! What the build scripts of the crates built on Mesa read from Mesa's Meson build folder; included
//! by each of them with `#[path]`.

use std::path::{Path, PathBuf};

pub const RERUN_IF_CHANGED: &str = "cargo::rerun-if-changed=";
/// Meson's record of the build, which names the source folder it was made from.
const MESON_INFO: &str = "meson-info/meson-info.json";
/// Mesa's own compile commands.
const COMPILE_COMMANDS: &str = "compile_commands.json";
/// How GCC and Clang, and the Visual C++ compiler, spell a define and the language standard.
const DEFINE_FLAGS: &[&str] = &["-D", "/D"];
const STANDARD_FLAGS: &[&str] = &["-std=", "/std:"];
const QUOTE: char = '"';
const QUOTE_TEXT: &str = "\"";
const ESCAPED_QUOTE: &str = "\\\"";

/// The source folder Meson made the build from.
pub fn source_folder(build: &Path) -> PathBuf {
    #[derive(serde::Deserialize)]
    struct Info {
        directories: Directories,
    }
    #[derive(serde::Deserialize)]
    struct Directories {
        source: PathBuf,
    }
    let path = build.join(MESON_INFO);
    let text = std::fs::read_to_string(&path).expect("Mesa's build has Meson's record");
    let info: Info = serde_json::from_str(&text).expect("Meson's record is JSON");
    info.directories.source
}

/// The defines and language standard Mesa compiled `file` with, from its compile commands.
pub fn mesa_flags(build: &Path, file: &str) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Command {
        file: String,
        command: String,
    }
    let path = build.join(COMPILE_COMMANDS);
    println!("{RERUN_IF_CHANGED}{}", path.display());
    let text = std::fs::read_to_string(&path).expect("Mesa's build has its compile commands");
    let commands: Vec<Command> = serde_json::from_str(&text).expect("compile commands are JSON");
    let command = commands
        .iter()
        .find(|c| c.file.ends_with(file))
        .unwrap_or_else(|| panic!("Mesa's build compiles {file}"));
    command
        .command
        .split_whitespace()
        .map(unquoted)
        .filter(|a| {
            DEFINE_FLAGS
                .iter()
                .chain(STANDARD_FLAGS)
                .any(|f| a.starts_with(f))
        })
        .collect()
}

/// An argument of a Visual C++ command as the compiler receives it: Meson quotes every one and
/// escapes the quotes inside.
fn unquoted(argument: &str) -> String {
    match argument
        .strip_prefix(QUOTE)
        .and_then(|a| a.strip_suffix(QUOTE))
    {
        Some(inner) => inner.replace(ESCAPED_QUOTE, QUOTE_TEXT),
        None => argument.to_string(),
    }
}
