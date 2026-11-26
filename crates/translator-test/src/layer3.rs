//! Layer 3: real compiled programs. Csmith's random C programs and the hand-picked programs, built with clang
//! (zig's) or gcc (Alpine's, in Docker) at an optimisation level, as flat blobs of fixed position or
//! position independent code, run on Linux and through the translator at none and full.

use super::part;
use super::programs::{
    Blob, C_DRIVER, ELF_EXTENSION, FLAT_SCRIPT, INCLUDE_FLAG, LINK_SCRIPT, NO_BUILD_ID,
    NO_SANITIZERS, OUTPUT_FLAG, Ran, SIGNALLED, ZIG_TARGET_FLAG, flatten, native, native_program,
    out_directory, source_directory, unique, zig,
};
use crate::reference::{self, DOCKER, REFERENCE_CONTAINER, container_name, mount, mount_writable};
use linux_raw_sys::general::SIGALRM;
use sha2::Digest;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use wcontainers_docker::PLATFORM_OPTION;

/// Where a container sees a program's source folder, the include folder, the link script's folder and the
/// folder of built programs.
const SOURCE_MOUNT: &str = "/s";
const INCLUDE_MOUNT: &str = "/i";
const SCRIPT_MOUNT: &str = "/l";
const OUT_MOUNT: &str = "/o";
/// The compiler's flag that turns warnings off (Csmith's programs set many off).
const NO_WARNINGS: &str = "-w";
/// gcc's command, in its image.
const GCC: &str = "gcc";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compiler {
    Clang,
    Gcc,
}

pub use super::programs::Position;

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

/// The gcc image of the guest's platform, made if it is not there yet.
fn gcc_image() -> String {
    static MADE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let dockerfile = format!(
        "FROM {}\nRUN apk add --no-cache gcc musl-dev\n",
        reference::IMAGE
    );
    // Named by its recipe, so a change of the recipe makes a new image.
    let recipe = format!("{:x}", sha2::Sha256::digest(&dockerfile));
    let image = format!("wc-x86-gcc-{}-{}", part::NAME, &recipe[..12]);
    // Another guest's failed build must not fail this one's.
    let mut made = MADE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if made.contains(&image) {
        return image;
    }
    let present = Command::new(DOCKER)
        .args(["image", "inspect", &image])
        .output()
        .expect("run docker")
        .status
        .success();
    if !present {
        let mut child = Command::new(DOCKER)
            .args([
                "build",
                "-q",
                PLATFORM_OPTION,
                part::REFERENCE.docker_platform,
                "-t",
                &image,
                "-",
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("run docker build");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(dockerfile.as_bytes())
            .unwrap();
        assert!(
            child.wait().unwrap().success(),
            "the gcc image did not build"
        );
    }
    made.push(image.clone());
    image
}

/// Write a Csmith program for a seed; its name is `csmith-<seed>`.
pub fn csmith(seed: u64) -> PathBuf {
    let directory = out_directory().join("layer3");
    std::fs::create_dir_all(&directory).unwrap();
    let name = format!("csmith-{seed}.c");
    let path = directory.join(&name);
    // Tests run the same case at once, so each writes its own file and moves it into place whole, and none
    // compiles a file another is writing.
    let own = directory.join(unique(&name));
    let status = Command::new("csmith")
        .args([
            "--seed",
            &seed.to_string(),
            "--max-funcs",
            "8",
            "--max-block-depth",
            "4",
            OUTPUT_FLAG,
        ])
        .arg(&own)
        .status()
        .expect("run csmith (from the Nix shell)");
    assert!(status.success(), "csmith failed for seed {seed}");
    std::fs::rename(&own, &path).unwrap();
    path
}

/// Build a C file with the C library into a flat blob.
pub fn build(source: &Path, compiler: Compiler, optimisation: &str, position: Position) -> Blob {
    let (elf, own, name) = compile(source, compiler, optimisation, position, true);
    flatten(&elf, &own, name)
}

/// Build a C file as an ordinary static Linux executable and run it on the reference, so a blob's fault can
/// be told to be the program's own.
pub fn ordinary(source: &Path, compiler: Compiler, optimisation: &str, position: Position) -> Ran {
    let (elf, ..) = compile(source, compiler, optimisation, position, false);
    let executable = std::fs::read(&elf).unwrap();
    std::fs::remove_file(&elf).unwrap();
    native_program(&executable)
}

/// Compile and link a C file, for a flat blob (with `flat.ld`) or as an ordinary executable. Gives the ELF
/// file, its own unique stem and the program's name.
fn compile(
    source: &Path,
    compiler: Compiler,
    optimisation: &str,
    position: Position,
    flat: bool,
) -> (PathBuf, String, String) {
    let stem = source.file_stem().unwrap().to_str().unwrap();
    let name = format!(
        "{stem}-{}{optimisation}{}",
        match compiler {
            Compiler::Clang => "clang",
            Compiler::Gcc => GCC,
        },
        position.suffix()
    );
    let out = out_directory();
    let own = unique(&name);
    let elf = out.join(&own).with_added_extension(ELF_EXTENSION);
    let script = source_directory().join(FLAT_SCRIPT);
    let include = csmith_include();
    let code = position.flags();
    match compiler {
        Compiler::Clang => {
            let (script, include, elf, source) = (
                [LINK_SCRIPT, &script.display().to_string()].concat(),
                [INCLUDE_FLAG, &include.display().to_string()].concat(),
                elf.display().to_string(),
                source.display().to_string(),
            );
            let mut arguments = vec![C_DRIVER, ZIG_TARGET_FLAG, part::C_TARGET, optimisation];
            arguments.extend(code);
            arguments.extend([NO_SANITIZERS, NO_WARNINGS, &include]);
            arguments.push(super::programs::LINK_ONLY_WHAT_IS_REACHED);
            if flat {
                arguments.push(&script);
            }
            arguments.extend([NO_BUILD_ID, OUTPUT_FLAG, &elf, &source]);
            zig(&arguments);
        }
        Compiler::Gcc => {
            let image = gcc_image();
            let source_directory = source.parent().unwrap();
            let mut command = Command::new(DOCKER);
            command
                .args(REFERENCE_CONTAINER)
                .arg(container_name(GCC))
                .arg(PLATFORM_OPTION)
                .arg(part::REFERENCE.docker_platform);
            mount(&mut command, source_directory, SOURCE_MOUNT);
            mount(&mut command, &include, INCLUDE_MOUNT);
            mount(&mut command, script.parent().unwrap(), SCRIPT_MOUNT);
            mount_writable(&mut command, &out, OUT_MOUNT);
            command
                .args([image.as_str(), GCC, optimisation, NO_WARNINGS])
                .args(code)
                .arg([INCLUDE_FLAG, INCLUDE_MOUNT].concat());
            if flat {
                command.arg([LINK_SCRIPT, &inside(SCRIPT_MOUNT, &script)].concat());
            }
            let status = command
                .args([NO_BUILD_ID, OUTPUT_FLAG])
                .arg(inside(OUT_MOUNT, &elf))
                .arg(inside(SOURCE_MOUNT, source))
                .status()
                .expect("run gcc in docker");
            assert!(status.success(), "gcc failed for {name}");
        }
    }
    (elf, own, name)
}

/// Where a container that mounts the host file's folder at `mount` sees that file.
fn inside(mount: &str, file: &Path) -> String {
    let inside = Path::new(mount).join(file.file_name().unwrap());
    inside.to_str().unwrap().to_owned()
}

/// The native run, or `None` when the program does not end in time (Csmith's programs may not).
pub fn native_in_time(blob: &Blob) -> Option<Ran> {
    let ran = native(blob);
    // The reference's own limit: SIGALRM, as `SIGNALLED` plus its number.
    (ran.status != SIGNALLED + SIGALRM as i32).then_some(ran)
}

/// What layer 3 multiplies its seed by to get the number of its first Csmith case.
pub const LAYER3_CASE_STRIDE: u64 = 1_000_003;

/// The source, compiler, level and position of Csmith case `number`.
pub fn csmith_case(number: u64) -> (std::path::PathBuf, Compiler, &'static str, Position) {
    let compiler = if number.is_multiple_of(2) {
        Compiler::Clang
    } else {
        Compiler::Gcc
    };
    let level = LEVELS[(number / 2 % LEVELS.len() as u64) as usize];
    // Each compiler at each level, then the other position.
    let compilers_and_levels = 2 * LEVELS.len() as u64;
    let position = if (number / compilers_and_levels).is_multiple_of(2) {
        Position::Fixed
    } else {
        Position::Independent
    };
    (csmith(number), compiler, level, position)
}
