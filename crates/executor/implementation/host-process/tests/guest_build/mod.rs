//! Guest programs built as static ARM64 Linux programs with zig's C compiler.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

use wcontainers_arm64::Arm64;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{C_DRIVER, OUTPUT_FLAG, ZIG_TARGET_FLAG};

/// The translator's test programs, whose guest folder holds headers the virtual-machine executor's programs include.
pub const PROGRAMS: &str = "../../../translator-test/programs";

/// A freestanding program's compiler flags beside the target and its defines.
const FREESTANDING_FLAGS: [&str; 2] = ["-O2", "-ffreestanding"];

/// A freestanding program's linker flags: each segment starts a page, as the virtual-machine executor's speed test
/// links them.
const FREESTANDING_LINK_FLAGS: [&str; 3] = ["-static", "-z", "separate-loadable-segments"];

fn include() -> String {
    format!(
        "-I{}",
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(PROGRAMS)
            .join(<Arm64 as GuestTest>::NAME)
            .display()
    )
}

fn source(directory: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(directory)
        .join(name)
        .with_extension("c")
}

fn out(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wchp-{name}"))
}

fn zig(commands: &mut [Command]) {
    for command in commands {
        let output = command
            .output()
            .expect("running zig cc; run the test inside `nix develop`");
        assert!(
            output.status.success(),
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Builds the static ARM64 Linux program `directory/name.c` with zig's C compiler, linked with musl or with no C
/// library, once per test process.
pub fn compile(directory: &str, name: &str, with_libc: bool) -> Vec<u8> {
    static BUILT: OnceLock<Mutex<std::collections::BTreeMap<String, Vec<u8>>>> = OnceLock::new();
    let built = BUILT.get_or_init(Mutex::default);
    let key = format!("{directory}/{name}");
    if let Some(image) = built
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return image.clone();
    }
    let source = source(directory, name);
    let out = out(name);
    let object = out.with_extension("o");
    let include = include();
    let target = [C_DRIVER, ZIG_TARGET_FLAG, <Arm64 as GuestTest>::C_TARGET];
    // Compiled with the C library's headers, linked with its code or without it.
    let mut compile = Command::new("zig");
    compile
        .args(target)
        .args(["-O2", "-c", &include, OUTPUT_FLAG])
        .arg(&object)
        .arg(&source);
    let mut link = Command::new("zig");
    link.args(target)
        .arg("-static")
        .args((!with_libc).then_some("-nostdlib"))
        .arg(OUTPUT_FLAG)
        .arg(&out)
        .arg(&object);
    zig(&mut [compile, link]);
    let image = std::fs::read(&out).expect("the built program");
    built
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, image.clone());
    image
}

/// Builds the freestanding static program `name` from `directory/source.c` with `defines` and the compiler's `flags`
/// (no C library), and gives its path.
pub fn freestanding(
    directory: &str,
    source_name: &str,
    name: &str,
    defines: &[String],
    flags: &[&str],
) -> PathBuf {
    let out = out(name);
    let object = out.with_extension("o");
    let mut compile = Command::new("zig");
    compile
        .args([
            C_DRIVER,
            ZIG_TARGET_FLAG,
            <Arm64 as GuestTest>::C_TARGET,
            "-c",
        ])
        .args(FREESTANDING_FLAGS)
        .args(flags)
        .arg(include())
        .args(defines.iter().map(|define| format!("-D{define}")))
        .arg(OUTPUT_FLAG)
        .arg(&object)
        .arg(source(directory, source_name));
    let mut link = Command::new("zig");
    link.arg("ld.lld")
        .args(FREESTANDING_LINK_FLAGS)
        .arg(OUTPUT_FLAG)
        .arg(&out)
        .arg(&object);
    zig(&mut [compile, link]);
    out
}
