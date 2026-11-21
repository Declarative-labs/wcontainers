//! The kernel keeps to its boundary: it builds for WebAssembly, the GPL stand-in never enters its
//! dependency tree, and its copy of Linux's definitions is current.

use std::path::Path;
use std::process::Command;

use wcontainers_kernel_test::generate;

fn cargo() -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(generate::workspace_root());
    command
}

#[test]
fn kernel_builds_for_wasm32() {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("wasm-check");
    let output = cargo()
        .args([
            "build",
            "-p",
            "wcontainers-kernel-linux",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .arg("--target-dir")
        .arg(&target)
        .output()
        .expect("running cargo");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn tree(package: &str, target: &str) -> String {
    let output = cargo()
        .args([
            "tree",
            "-p",
            package,
            "-e",
            "normal,build",
            "--prefix",
            "none",
        ])
        .args(["--target", target])
        .output()
        .expect("running cargo tree");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn unicorn_stays_out_of_the_shipped_crates() {
    // The check can see unicorn where it is.
    assert!(tree("wcontainers-kernel-test", "all").contains("unicorn-engine"));
    for package in ["wcontainers-kernel-linux", "wcontainers-abi"] {
        for target in ["wasm32-unknown-unknown", "all"] {
            let tree = tree(package, target);
            assert!(tree.starts_with(package), "{tree}");
            assert!(
                !tree.contains("unicorn"),
                "{package} on {target} depends on unicorn:\n{tree}"
            );
        }
    }
}

#[test]
fn generated_files_are_current() {
    for file in generate::generate() {
        let current = std::fs::read_to_string(&file.path).unwrap_or_default();
        assert!(
            current == file.contents,
            "{} is out of date; run `cargo run -p wcontainers-kernel-test --bin generate-linux`",
            file.path.display()
        );
    }
}
