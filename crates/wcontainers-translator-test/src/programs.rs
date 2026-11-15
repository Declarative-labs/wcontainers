//! Small C programs built as flat blobs, run through the translator and on real ARM64 Linux.

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

/// Build `programs/<name>.c` at an optimisation level, such as "-O2", into a flat blob. Only general
/// registers are used: milestone 0 translates integer instructions.
pub fn build(name: &str, optimisation: &str) -> Blob {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/programs");
    std::fs::create_dir_all(&out).unwrap();
    let stem = format!("{name}{optimisation}");
    let elf = out.join(format!("{stem}.elf"));
    let source = source_directory().join(format!("{name}.c"));
    let script = source_directory().join("flat.ld");
    let text = |p: &Path| p.to_str().unwrap().to_string();
    zig(&[
        "cc",
        "-target",
        "aarch64-linux-musl",
        optimisation,
        "-ffreestanding",
        "-nostdlib",
        "-static",
        "-fno-pic",
        "-no-pie",
        // zig sets the CPU's features itself; this is how it leaves out floating point and vectors.
        "-mcpu=generic-neon-fp_armv8",
        "-fno-stack-protector",
        "-fno-sanitize=all",
        "-fwrapv",
        &format!("-Wl,-T,{}", text(&script)),
        "-Wl,--build-id=none",
        "-o",
        &text(&elf),
        &text(&source),
    ]);
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
    pub output: Vec<u8>,
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
    let mut registers = RegisterBlock::ZERO;
    registers.pc = LOAD;
    registers.sp = STACK_TOP;
    machine.set_registers(&registers);
    let (stopped, registers) = machine.run();
    let status = match stopped {
        Stopped::Finished(status) => status,
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
        output: machine.output().to_vec(),
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
        output: output.stdout,
        status: output.status.code().expect("an exit status"),
    }
}
