//! C programs built as flat blobs, run through the translator and on real ARM64 Linux.

use crate::layout::initial_stack;
use crate::reference::{build as build_reference, container_name};
use crate::runner::{Machine, Stopped, Syscalls};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_translator::LookAhead;

/// Where every blob is linked and loaded.
pub const LOAD: u64 = 0x40_0000;
/// The top of the 1 MiB stack, above the reach of the reference's own randomised heap.
pub const STACK_TOP: u64 = 0x4810_0000;
const STACK_BYTES: u64 = 1 << 20;

/// A flat blob and how much of its start is code.
pub struct Blob {
    /// The program and optimisation level, such as "hello-O2".
    pub name: String,
    pub bytes: Vec<u8>,
    pub code_bytes: u64,
}

fn out_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/programs")
}

/// A file name no other build or run, in this process or another, uses.
fn unique(stem: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{stem}.{}.{n}", std::process::id())
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
    build_with(name, optimisation, library, &[])
}

/// [`build`] with preprocessor definitions, such as "SCALE=1".
pub fn build_with(name: &str, optimisation: &str, library: Library, defines: &[&str]) -> Blob {
    let out = out_directory();
    std::fs::create_dir_all(&out).unwrap();
    let mut stem = format!("{name}{optimisation}");
    for define in defines {
        stem.push('-');
        stem.push_str(&define.replace('=', ""));
    }
    let defines: Vec<String> = defines.iter().map(|d| format!("-D{d}")).collect();
    // Tests build the same program at once, so each build works under its own names.
    let own = unique(&stem);
    let elf = out.join(format!("{own}.elf"));
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
    arguments.extend(defines.iter().map(String::as_str));
    let (elf_text, source_text) = (text(&elf), text(&source));
    arguments.extend(["-o", &elf_text, &source_text]);
    zig(&arguments);
    let built = out.join(format!("{own}.bin"));
    let code = out.join(format!("{own}.text"));
    zig(&["objcopy", "-O", "binary", &text(&elf), &text(&built)]);
    zig(&[
        "objcopy",
        "-O",
        "binary",
        "--only-section=.text",
        &text(&elf),
        &text(&code),
    ]);
    let blob = Blob {
        bytes: std::fs::read(&built).unwrap(),
        code_bytes: std::fs::metadata(&code).unwrap().len(),
        name: stem,
    };
    std::fs::remove_file(&built).unwrap();
    std::fs::remove_file(&code).unwrap();
    // The last build of each is kept for looking at.
    std::fs::rename(&elf, out.join(format!("{}.elf", blob.name))).unwrap();
    blob
}

/// What a program did: its output and exit status.
#[derive(Debug, PartialEq, Eq)]
pub struct Ran {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// Where a flat blob and its stack go in guest memory, the same for every runner.
pub struct Placement {
    /// Size of guest memory: everything up to the top of the stack.
    pub memory_bytes: u64,
    /// Page aligned ranges and what they allow: the blob, then the stack.
    pub regions: Vec<(Range<u64>, Permissions)>,
    /// The stack pointer to start with, and the bytes to place from it up to the top of the stack.
    pub sp: u64,
    pub stack: Vec<u8>,
}

pub fn placement(blob: &Blob) -> Placement {
    let end = LOAD + (blob.bytes.len() as u64).next_multiple_of(4096);
    let (sp, stack) = initial_stack(STACK_TOP);
    Placement {
        memory_bytes: STACK_TOP,
        regions: vec![
            (LOAD..end, Permissions::all()),
            (
                STACK_TOP - STACK_BYTES..STACK_TOP,
                Permissions::READ | Permissions::WRITE,
            ),
        ],
        sp,
        stack,
    }
}

/// Run a blob through the translator in wasmtime, starting at its first byte.
pub fn translated(blob: &Blob, look_ahead: LookAhead) -> (Ran, Machine, RegisterBlock) {
    let slots = blob.code_bytes.div_ceil(4) as u32;
    let place = placement(blob);
    let mut machine = Machine::new(place.memory_bytes, LOAD, slots, Syscalls::StandIn);
    for (range, permissions) in place.regions {
        machine.set_permissions(range, permissions);
    }
    machine.load(&blob.bytes, 0..blob.code_bytes, LOAD, &[0], look_ahead);
    machine.write(place.sp, &place.stack);
    let mut registers = RegisterBlock::ZERO;
    registers.pc = LOAD;
    registers.sp = place.sp;
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
    (ran, machine, registers)
}

/// Every byte the guest may reach: the blob's pages, then the stack.
pub fn reachable_memory(machine: &mut Machine, blob: &Blob) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (range, _) in placement(blob).regions {
        let mut part = vec![0; (range.end - range.start) as usize];
        machine.read(range.start, &mut part);
        bytes.extend(part);
    }
    bytes
}

/// Run a blob natively on ARM64 Linux in Docker.
pub fn native(blob: &Blob) -> Ran {
    let reference = build_reference();
    // Every run reads a file of its own that nothing replaces while Docker reads it.
    let directory = out_directory().join("native");
    std::fs::create_dir_all(&directory).unwrap();
    let file = format!("{}.bin", unique(&blob.name));
    std::fs::write(directory.join(&file), &blob.bytes).unwrap();
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
    std::fs::remove_file(directory.join(&file)).unwrap();
    Ran {
        stdout: output.stdout,
        stderr: output.stderr,
        status: output.status.code().expect("an exit status"),
    }
}
