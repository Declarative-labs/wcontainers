//! Layer 3: real compiled programs. Csmith's random C programs and the hand-picked programs, built with clang
//! (zig's) or gcc (Alpine's, in Docker) at an optimisation level, as flat blobs of fixed position or
//! position independent code, run on Linux and through the translator at none and full.

use crate::programs::{Blob, Ran, flatten, native, out_directory, source_directory, unique, zig};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The Docker image with gcc and musl, made once from Alpine.
const GCC_IMAGE: &str = "wc-translator-gcc";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compiler {
    Clang,
    Gcc,
}

/// How the code is generated: for the fixed address it is linked and loaded at, or position independent
/// (`-fPIE`, a static position independent program that relocates itself at start), loaded away from where
/// it is linked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Fixed,
    Independent,
}

pub const LEVELS: [&str; 6] = ["-O0", "-O1", "-O2", "-O3", "-Os", "-Oz"];

/// Csmith's headers, copied where Docker can read them.
fn csmith_include() -> PathBuf {
    static INCLUDE: OnceLock<PathBuf> = OnceLock::new();
    INCLUDE
        .get_or_init(|| {
            let csmith = Command::new("sh")
                .args(["-c", "dirname $(dirname $(command -v csmith))"])
                .output()
                .expect("find csmith (from the Nix shell)");
            let root = String::from_utf8(csmith.stdout).unwrap();
            let from = Path::new(root.trim()).join("include");
            let to = out_directory().join("layer3/include");
            std::fs::create_dir_all(&to).unwrap();
            for entry in std::fs::read_dir(&from).expect("csmith's headers") {
                let entry = entry.unwrap();
                // The Nix store's files are read only, and so are their copies: replace them.
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_file() {
                    let _ = std::fs::remove_file(&target);
                    std::fs::copy(entry.path(), &target).unwrap();
                }
            }
            to
        })
        .clone()
}

/// Make the gcc image if it is not there yet.
fn gcc_image() {
    static MADE: OnceLock<()> = OnceLock::new();
    MADE.get_or_init(|| {
        let present = Command::new("docker")
            .args(["image", "inspect", GCC_IMAGE])
            .output()
            .expect("run docker")
            .status
            .success();
        if present {
            return;
        }
        let mut child = Command::new("docker")
            .args(["build", "-q", "-t", GCC_IMAGE, "-"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("run docker build");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"FROM alpine:latest\nRUN apk add --no-cache gcc musl-dev\n")
            .unwrap();
        assert!(
            child.wait().unwrap().success(),
            "the gcc image did not build"
        );
    });
}

/// Write a Csmith program for a seed; its name is `csmith-<seed>`.
pub fn csmith(seed: u64) -> PathBuf {
    let directory = out_directory().join("layer3");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("csmith-{seed}.c"));
    let status = Command::new("csmith")
        .args([
            "--seed",
            &seed.to_string(),
            "--max-funcs",
            "8",
            "--max-block-depth",
            "4",
            "-o",
        ])
        .arg(&path)
        .status()
        .expect("run csmith (from the Nix shell)");
    assert!(status.success(), "csmith failed for seed {seed}");
    path
}

/// Build a C file with the C library into a flat blob.
pub fn build(source: &Path, compiler: Compiler, optimisation: &str, position: Position) -> Blob {
    let stem = source.file_stem().unwrap().to_str().unwrap();
    let name = format!(
        "{stem}-{}{optimisation}{}",
        match compiler {
            Compiler::Clang => "clang",
            Compiler::Gcc => "gcc",
        },
        match position {
            Position::Fixed => "",
            Position::Independent => "-pic",
        }
    );
    let out = out_directory();
    let own = unique(&name);
    let elf = out.join(format!("{own}.elf"));
    let script = source_directory().join("flat.ld");
    let include = csmith_include();
    let code: &[&str] = match position {
        Position::Fixed => &["-static", "-fno-pic", "-no-pie"],
        Position::Independent => &["-static-pie", "-fPIE", "-Wl,-z,norelro"],
    };
    match compiler {
        Compiler::Clang => {
            let (script, include, elf, source) = (
                format!("-Wl,-T,{}", script.display()),
                format!("-I{}", include.display()),
                elf.display().to_string(),
                source.display().to_string(),
            );
            let mut arguments = vec!["cc", "-target", "aarch64-linux-musl", optimisation];
            arguments.extend(code);
            arguments.extend([
                "-fno-sanitize=all",
                "-w",
                &include,
                &script,
                "-Wl,--build-id=none",
                "-o",
                &elf,
                &source,
            ]);
            zig(&arguments);
        }
        Compiler::Gcc => {
            gcc_image();
            let source_directory = source.parent().unwrap();
            let status = Command::new("docker")
                .args(["run", "--rm", "--network", "none", "--name"])
                .arg(crate::reference::container_name("gcc"))
                .arg("-v")
                .arg(format!("{}:/s:ro", source_directory.display()))
                .arg("-v")
                .arg(format!("{}:/i:ro", include.display()))
                .arg("-v")
                .arg(format!("{}:/l:ro", script.parent().unwrap().display()))
                .arg("-v")
                .arg(format!("{}:/o", out.display()))
                .args([GCC_IMAGE, "gcc", optimisation, "-w"])
                .args(code)
                .args(["-I/i", "-T/l/flat.ld", "-Wl,--build-id=none", "-o"])
                .arg(format!("/o/{own}.elf"))
                .arg(format!(
                    "/s/{}",
                    source.file_name().unwrap().to_str().unwrap()
                ))
                .status()
                .expect("run gcc in docker");
            assert!(status.success(), "gcc failed for {name}");
        }
    }
    let mut blob = flatten(&elf, &own, name);
    if position == Position::Independent {
        blob.load = crate::programs::MOVED;
    }
    blob
}

/// The native run, or `None` when the program does not end in time (Csmith's programs may not).
pub fn native_in_time(blob: &Blob) -> Option<Ran> {
    let ran = native(blob);
    // The reference's own limit: SIGALRM, as Docker reports it.
    (ran.status != 128 + 14).then_some(ran)
}
