//! What the build scripts of the crates built on Mesa read from Mesa's Meson build folder; included
//! by each of them with `#[path]`.

use std::path::{Path, PathBuf};

/// Cargo's directives, as every build script of the workspace names them.
#[allow(
    dead_code,
    reason = "the build scripts on Mesa use only some of the shared names"
)]
#[path = "../../executor/implementation/hardware-virtualization-inside/build_environment.rs"]
mod build_environment;
#[allow(
    unused_imports,
    reason = "only the build scripts that link Mesa themselves use it"
)]
pub use build_environment::LINK_ARG;
pub use build_environment::{RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED};
#[allow(
    dead_code,
    reason = "only the build scripts that hand values to their crate use it"
)]
pub const RUSTC_ENV: &str = "cargo::rustc-env=";
/// Between a variable and its value in `RUSTC_ENV`.
#[allow(
    dead_code,
    reason = "only the build scripts that hand values to their crate use it"
)]
pub const ASSIGN: &str = "=";
/// The variable naming the Mesa build folder, and the cfg set when it does.
pub const MESA_VARIABLE: &str = "WCONTAINERS_MESA";
pub const CHECK_CFG: &str = "cargo::rustc-check-cfg=cfg(mesa)";
pub const CFG: &str = "cargo::rustc-cfg=mesa";
/// Meson's record of the build, which names the source folder it was made from.
const MESON_INFO: &str = "meson-info/meson-info.json";
/// Mesa's own compile commands.
const COMPILE_COMMANDS: &str = "compile_commands.json";
/// How GCC and Clang, and the Visual C++ compiler, spell a define and the language standard.
const DEFINE_FLAGS: &[&str] = &["-D", "/D"];
const STANDARD_FLAGS: &[&str] = &["-std=", "/std:"];
/// How Clang spells a define, for bindgen.
const CLANG_DEFINE: &str = "-D";
const QUOTE: char = '"';
const QUOTE_TEXT: &str = "\"";
const ESCAPED_QUOTE: &str = "\\\"";

/// The source folder Meson made the build from.
#[allow(
    dead_code,
    reason = "only the build scripts that compile C against Mesa call it"
)]
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
#[allow(
    dead_code,
    reason = "only the build scripts that compile C against Mesa call it"
)]
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

/// The defines Mesa compiles `file` with, spelled as Clang takes them (for bindgen).
#[allow(dead_code, reason = "only the build scripts that run bindgen call it")]
pub fn mesa_defines(build: &Path, file: &str) -> Vec<String> {
    mesa_flags(build, file)
        .into_iter()
        .filter_map(|flag| {
            DEFINE_FLAGS
                .iter()
                .find_map(|prefix| flag.strip_prefix(prefix))
                .map(|define| format!("{CLANG_DEFINE}{define}"))
        })
        .collect()
}
