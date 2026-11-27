//! The generator's commands over Microsoft's Win32 metadata, each run on the pinned metadata: the census of the
//! graphics namespaces is compared with reports/census.txt (its counts are the census the brief records: Direct3D11 92
//! interfaces and 521 methods, Dxgi 47 and 191, Direct3D12 101 and 389, Direct3D10 55 and 341), `generate` and
//! `once-check` each run to their end on the spike's slice of Direct3D 11 (both glues written), and `layout-check` on
//! Direct3D 11. The input comes from the environment, and the
//! test fails without it:
//!   WCONTAINERS_GENERATOR_METADATA  Windows.Win32.winmd, as fetch-metadata.sh fetches and checks it

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn metadata() -> PathBuf {
    PathBuf::from(std::env::var_os("WCONTAINERS_GENERATOR_METADATA").unwrap_or_else(|| panic!("WCONTAINERS_GENERATOR_METADATA names an input of this test (tests/windows.rs); it fails without it")))
}

fn run(arguments: &[&std::ffi::OsStr]) -> Output {
    let run = Command::new(env!("CARGO_BIN_EXE_api-crate-generator")).args(arguments).output().expect("the generator runs");
    assert!(run.status.success(), "{arguments:?}: the generator failed: {}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    run
}

fn work(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The census of the six graphics namespaces, as reviewed in reports/census.txt.
#[test]
fn census_as_reported() {
    let winmd = metadata();
    let namespaces = ["Windows.Win32.Graphics.Direct3D11", "Windows.Win32.Graphics.Dxgi", "Windows.Win32.Graphics.Dxgi.Common", "Windows.Win32.Graphics.Direct3D", "Windows.Win32.Graphics.Direct3D12", "Windows.Win32.Graphics.Direct3D10"];
    let mut arguments = vec![std::ffi::OsStr::new("census"), winmd.as_os_str()];
    arguments.extend(namespaces.iter().map(std::ffi::OsStr::new));
    let printed = String::from_utf8_lossy(&run(&arguments).stdout).to_string();
    let expected = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("reports").join("census.txt")).unwrap_or_default();
    assert!(printed == expected, "the census differs from reports/census.txt; it printed:\n{printed}");
}

/// `generate` on the slice, then `once-check` on what it wrote, and `layout-check` on Direct3D 11, each to its end.
#[test]
fn the_windows_commands_run() {
    let winmd = metadata();
    let written = work("windows-generated");
    let generated = run(&[std::ffi::OsStr::new("generate"), winmd.as_os_str(), std::ffi::OsStr::new("direct3d-11-slice"), written.as_os_str()]);
    assert!(written.join("crates").join("direct3d-11").join("interface.rs").is_file(), "generate wrote no interface.rs: {}", String::from_utf8_lossy(&generated.stdout));
    run(&[std::ffi::OsStr::new("once-check"), winmd.as_os_str(), written.as_os_str()]);
    let check = work("windows-layout").join("layout.c");
    std::fs::create_dir_all(check.parent().expect("a folder")).expect("the folder made");
    let laid = run(&[std::ffi::OsStr::new("layout-check"), winmd.as_os_str(), std::ffi::OsStr::new("direct3d-11"), check.as_os_str()]);
    assert!(String::from_utf8_lossy(&laid.stdout).starts_with("LAYOUT CHECK: "), "{}", String::from_utf8_lossy(&laid.stdout));
    assert!(check.is_file(), "layout-check wrote no C file");
}
