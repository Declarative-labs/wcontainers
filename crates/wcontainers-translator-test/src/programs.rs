//! C programs built as flat blobs, run through the translator and on real ARM64 Linux.

use crate::layout::initial_stack;
use crate::reference::{build as build_reference, container_name};
use crate::runner::{Machine, Stopped, Syscalls};
use std::path::{Path, PathBuf};
use std::process::Command;
use wcontainers_abi::{Permissions, RegisterBlock};

/// Where every blob is linked and loaded.
pub const LOAD: u64 = 0x40_0000;
/// The top of the 1 MiB stack, above the reach of the reference's own randomised heap.
pub const STACK_TOP: u64 = 0x4810_0000;
const STACK_BYTES: u64 = 1 << 20;

/// A flat blob and how much of its start is code.
pub struct Blob {
    pub bytes: Vec<u8>,
    pub code_bytes: u64,
    pub path: PathBuf,
}

fn source_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("programs")
}

fn zig(arguments: &[&str]) {
    let status = Command::new("zig")
        .args(arguments)
        .status()
        .expect("run zig (from the Nix shell)");
    assert!(status.success(), "zig {arguments:?} failed");
}

/// What a program is built against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Library {
    /// Nothing: the program brings its own `_start` and system calls, and uses only general registers.
    None,
    /// The C library (musl, as zig builds it), started by its own `_start`.
    C,
}

/// Build `programs/<name>.c` at an optimisation level, such as "-O2", into a flat blob.
pub fn build(name: &str, optimisation: &str, library: Library) -> Blob {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/programs");
    std::fs::create_dir_all(&out).unwrap();
    let stem = format!("{name}{optimisation}");
    let elf = out.join(format!("{stem}.elf"));
    let source = source_directory().join(format!("{name}.c"));
    let script = source_directory().join("flat.ld");
    let text = |p: &Path| p.to_str().unwrap().to_string();
    let script = format!("-Wl,-T,{}", text(&script));
    let mut arguments = vec![
        "cc",
        "-target",
        "aarch64-linux-musl",
        optimisation,
        "-static",
        "-fno-pic",
        "-no-pie",
        "-fno-sanitize=all",
        &script,
        "-Wl,--build-id=none",
    ];
    if library == Library::None {
        arguments.extend([
            "-ffreestanding",
            "-nostdlib",
            // zig sets the CPU's features itself; this is how it leaves out floating point and vectors.
            "-mcpu=generic-neon-fp_armv8",
            "-fno-stack-protector",
            "-fwrapv",
        ]);
    }
    let (elf_text, source_text) = (text(&elf), text(&source));
    arguments.extend(["-o", &elf_text, &source_text]);
    zig(&arguments);
    let path = out.join(format!("{stem}.bin"));
    let code = out.join(format!("{stem}.text"));
    zig(&["objcopy", "-O", "binary", &text(&elf), &text(&path)]);
    zig(&[
        "objcopy",
        "-O",
        "binary",
        "--only-section=.text",
        &text(&elf),
        &text(&code),
    ]);
    Blob {
        bytes: std::fs::read(&path).unwrap(),
        code_bytes: std::fs::metadata(&code).unwrap().len(),
        path,
    }
}

/// What a program did: its output and exit status.
#[derive(Debug, PartialEq, Eq)]
pub struct Ran {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// Run a blob through the translator at look-ahead none, starting at its first byte.
pub fn translated(blob: &Blob) -> (Ran, Machine) {
    let slots = blob.code_bytes.div_ceil(4) as u32;
    let mut machine = Machine::new(STACK_TOP, LOAD, slots, Syscalls::StandIn);
    let end = LOAD + (blob.bytes.len() as u64).next_multiple_of(4096);
    machine.set_permissions(LOAD..end, Permissions::all());
    machine.set_permissions(
        STACK_TOP - STACK_BYTES..STACK_TOP,
        Permissions::READ | Permissions::WRITE,
    );
    machine.load(&blob.bytes, 0..blob.code_bytes, LOAD, &[0]);
    let (sp, stack) = initial_stack(STACK_TOP);
    machine.write(sp, &stack);
    let mut registers = RegisterBlock::ZERO;
    registers.pc = LOAD;
    registers.sp = sp;
    machine.set_registers(&registers);
    let (stopped, registers) = machine.run();
    let status = match stopped {
        // Linux reports the low 8 bits of the status.
        Stopped::Finished(status) => status & 0xff,
        other => {
            let at = (registers.pc - LOAD) as usize;
            let word = blob
                .bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
            panic!(
                "the program stopped without exiting: {other:?} at {:#x}, word {word:#x?} ({:?})",
                registers.pc,
                word.map(wcontainers_arm64::decode)
            )
        }
    };
    let ran = Ran {
        stdout: machine.stdout().to_vec(),
        stderr: machine.stderr().to_vec(),
        status,
    };
    (ran, machine)
}

/// Run a blob natively on ARM64 Linux in Docker.
pub fn native(blob: &Blob) -> Ran {
    let reference = build_reference();
    let directory = blob.path.parent().unwrap();
    let file = blob.path.file_name().unwrap().to_str().unwrap();
    let output = Command::new("docker")
        .args(["run", "--rm", "--network", "none", "--name"])
        .arg(container_name("program"))
        .arg("-v")
        .arg(format!("{}:/r:ro", reference.display()))
        .arg("-v")
        .arg(format!("{}:/p:ro", directory.display()))
        .args(["alpine:latest", "/r/wcontainers-reference", "blob"])
        .arg(format!("/p/{file}"))
        .arg(format!("{LOAD:#x}"))
        .arg(format!("{STACK_TOP:#x}"))
        .output()
        .expect("run docker");
    Ran {
        stdout: output.stdout,
        stderr: output.stderr,
        status: output.status.code().expect("an exit status"),
    }
}
