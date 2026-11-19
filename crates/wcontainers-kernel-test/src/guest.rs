//! Building the small static ARM64 test programs in `guest/` with zig's C compiler.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the test programs' sources are.
pub fn source_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("guest")
}

/// Builds `guest/<name>.c` into `out/<name>` as a static ARM64 Linux program. `link_flags` go to the
/// link step only, so a program linked with `-nostdlib` still has the C library's headers.
pub fn build(name: &str, link_flags: &[&str], out: &Path) -> PathBuf {
    std::fs::create_dir_all(out).expect("creating the output directory");
    let source = source_directory().join(format!("{name}.c"));
    // Tests build in parallel, so each build writes its own files and renames the program into place.
    let unique = format!("{}-{:?}", std::process::id(), std::thread::current().id())
        .replace(|c: char| !c.is_ascii_alphanumeric(), "");
    let object = out.join(format!("{name}.{unique}.o"));
    let built = out.join(format!("{name}.{unique}"));
    let program = out.join(name);
    zig(Command::new("zig")
        .args(["cc", "-target", "aarch64-linux-musl", "-O2", "-c", "-o"])
        .arg(&object)
        .arg(&source));
    zig(Command::new("zig")
        .args(["cc", "-target", "aarch64-linux-musl", "-static", "-o"])
        .arg(&built)
        .arg(&object)
        .args(link_flags));
    std::fs::remove_file(&object).expect("removing the object file");
    // A program already in place with the same bytes stays, so a container another test is starting
    // from it (through Docker's shared directory) never sees its name replaced.
    let same = std::fs::read(&program)
        .is_ok_and(|old| old == std::fs::read(&built).expect("reading the built program"));
    if same {
        std::fs::remove_file(&built).expect("removing the duplicate program");
    } else {
        std::fs::rename(&built, &program).expect("moving the program into place");
    }
    program
}

fn zig(command: &mut Command) {
    let output = command
        .output()
        .expect("running zig cc; run the tests inside `nix develop`");
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
