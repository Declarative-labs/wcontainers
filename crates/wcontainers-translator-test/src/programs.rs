//! C programs built as flat blobs, run through the translator and on real ARM64 Linux.

use crate::layout::initial_stack;
use crate::reference::{build as build_reference, container_name};
use crate::runner::{Machine, Stopped, Syscalls};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use wcontainers_abi::{Exit, Permissions, RegisterBlock};
use wcontainers_translator::{LookAhead, Omit, Optimise};

/// Where every blob is linked, and loaded unless it is moved.
pub const LOAD: u64 = 0x40_0000;
/// Where a position independent blob is loaded, away from where it is linked.
pub const MOVED: u64 = 0x2345_6000;
/// The top of the stack, above the reach of the reference's own randomised heap.
pub const STACK_TOP: u64 = 0x4810_0000;
/// The stack a blob gets unless it asks for more.
pub const STACK_BYTES: u64 = 1 << 20;

/// A flat blob and how much of its start is code.
pub struct Blob {
    /// The program and optimisation level, such as "hello-O2".
    pub name: String,
    pub bytes: Vec<u8>,
    pub code_bytes: u64,
    /// Where it is placed in guest memory.
    pub load: u64,
    /// Where in it its program's ELF program headers are, and how many: the runners tell the program.
    pub program_headers: Option<(u64, u64)>,
    /// How much stack it gets, below [`STACK_TOP`].
    pub stack_bytes: u64,
}

impl Blob {
    /// Where the program headers are in guest memory, and how many.
    pub fn headers_in_memory(&self) -> Option<(u64, u64)> {
        self.program_headers
            .map(|(offset, count)| (self.load + offset, count))
    }
}

pub(crate) fn out_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/programs")
}

/// A file name no other build or run, in this process or another, uses.
pub(crate) fn unique(stem: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{stem}.{}.{n}", std::process::id())
}

pub(crate) fn source_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("programs")
}

pub(crate) fn zig(arguments: &[&str]) {
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

/// Build `programs/<name>.c` (or `.cpp`, C++ with its library; `.rs`, Rust with its standard library; `.S`,
/// assembly) at an optimisation level, such as "-O2", into a flat blob.
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
    let source = ["c", "cpp", "rs", "S"]
        .iter()
        .map(|extension| source_directory().join(format!("{name}.{extension}")))
        .find(|source| source.exists())
        .expect("a program of that name");
    let script = source_directory().join("flat.ld");
    let text = |p: &Path| p.to_str().unwrap().to_string();
    if source.extension().unwrap() == "rs" {
        rust(&source, &elf, &script, optimisation);
        return flatten(&elf, &own, stem);
    }
    let script = format!("-Wl,-T,{}", text(&script));
    let driver = if source.extension().unwrap() == "cpp" {
        "c++"
    } else {
        "cc"
    };
    let mut arguments = vec![
        driver,
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
    flatten(&elf, &own, stem)
}

/// Build a Rust program with its standard library, statically, linked with `script`.
fn rust(source: &Path, elf: &Path, script: &Path, optimisation: &str) {
    let level = optimisation.trim_start_matches("-O");
    let status = Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--target",
            "aarch64-unknown-linux-musl",
        ])
        .args(["-C", &format!("opt-level={level}")])
        .args(["-C", "panic=unwind", "-C", "relocation-model=static"])
        .args(["-C", "linker-flavor=ld.lld", "-C", "linker=rust-lld"])
        .args([
            "-C",
            "link-self-contained=yes",
            "-C",
            "link-arg=--build-id=none",
        ])
        .arg("-C")
        .arg(format!("link-arg=-T{}", script.display()))
        .arg("-o")
        .arg(elf)
        .arg(source)
        .status()
        .expect("run rustc (from the Nix shell)");
    assert!(status.success(), "rustc {} failed", source.display());
}

/// The flat blob of a linked program, `own` being the unique name its build works under. The ELF file is
/// kept, under the blob's name, for looking at.
pub(crate) fn flatten(elf: &Path, own: &str, name: String) -> Blob {
    let out = out_directory();
    let text = |p: &Path| p.to_str().unwrap().to_string();
    let built = out.join(format!("{own}.bin"));
    let code = out.join(format!("{own}.text"));
    zig(&["objcopy", "-O", "binary", &text(elf), &text(&built)]);
    zig(&[
        "objcopy",
        "-O",
        "binary",
        "--only-section=.text",
        &text(elf),
        &text(&code),
    ]);
    // The program headers go after the image, as the image does not hold them.
    let file = std::fs::read(elf).unwrap();
    let parsed =
        object::read::elf::ElfFile64::<object::Endianness>::parse(&*file).expect("an ELF file");
    let headers = parsed.elf_program_headers();
    let mut bytes = std::fs::read(&built).unwrap();
    bytes.resize(bytes.len().next_multiple_of(8), 0);
    let at = bytes.len() as u64;
    bytes.extend_from_slice(object::pod::bytes_of_slice(headers));
    let blob = Blob {
        bytes,
        code_bytes: std::fs::metadata(&code).unwrap().len(),
        load: LOAD,
        program_headers: Some((at, headers.len() as u64)),
        name,
        stack_bytes: STACK_BYTES,
    };
    std::fs::remove_file(&built).unwrap();
    std::fs::remove_file(&code).unwrap();
    std::fs::rename(elf, out.join(format!("{}.elf", blob.name))).unwrap();
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
    let end = blob.load + (blob.bytes.len() as u64).next_multiple_of(4096);
    let (sp, stack) = initial_stack(STACK_TOP, blob.headers_in_memory());
    Placement {
        memory_bytes: STACK_TOP,
        regions: vec![
            (blob.load..end, Permissions::all()),
            (
                STACK_TOP - blob.stack_bytes..STACK_TOP,
                Permissions::READ | Permissions::WRITE,
            ),
        ],
        sp,
        stack,
    }
}

/// Run a blob through the translator in wasmtime, starting at its first byte, translated for speed.
pub fn translated(blob: &Blob, look_ahead: LookAhead) -> (Ran, Machine, RegisterBlock) {
    translated_for(blob, (look_ahead, Optimise::Speed))
}

/// [`translated`] for speed or for size.
pub fn translated_for(blob: &Blob, how: (LookAhead, Optimise)) -> (Ran, Machine, RegisterBlock) {
    let (ran, machine, registers, _) = translated_omitting(blob, how, Omit::default());
    (ran, machine, registers)
}

/// [`translated`] without some checks, also giving how long the guest ran (translating and compiling not
/// included).
pub fn translated_omitting(
    blob: &Blob,
    how: (LookAhead, Optimise),
    omit: Omit,
) -> (Ran, Machine, RegisterBlock, Duration) {
    run_blob(blob, how, omit, false)
}

/// [`translated_for`] with the thread stopping at every stop check it reaches and carrying on each time.
pub fn translated_stopping_everywhere(
    blob: &Blob,
    how: (LookAhead, Optimise),
) -> (Ran, Machine, RegisterBlock) {
    let (ran, machine, registers, _) = run_blob(blob, how, Omit::default(), true);
    (ran, machine, registers)
}

fn run_blob(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    stop_everywhere: bool,
) -> (Ran, Machine, RegisterBlock, Duration) {
    let mut machine = ready_for(blob, look_ahead, optimise, omit);
    if stop_everywhere {
        machine.stop_at_every_check();
    }
    let started = Instant::now();
    let (stopped, registers) = machine.run();
    let ran_for = started.elapsed();
    let ran = ran(blob, &machine, stopped, &registers);
    (ran, machine, registers, ran_for)
}

/// A machine with a blob placed, translated and ready to run from its first byte.
pub fn ready(blob: &Blob, look_ahead: LookAhead, omit: Omit) -> Machine {
    ready_for(blob, look_ahead, Optimise::Speed, omit)
}

/// [`ready`], translated for `optimise`.
pub fn ready_for(blob: &Blob, look_ahead: LookAhead, optimise: Optimise, omit: Omit) -> Machine {
    ready_on(blob, (look_ahead, optimise), omit, crate::runner::probe())
}

/// [`ready`], translated for a host with these facts instead of the probe's.
pub fn ready_on(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    host: wcontainers_arm64::Host,
) -> Machine {
    ready_with(blob, host, |machine| {
        machine.optimise_for(optimise);
        machine.load_omitting(
            &blob.bytes,
            0..blob.code_bytes,
            blob.load,
            &[0],
            look_ahead,
            omit,
        );
        // The reference stays at none throughout.
        if look_ahead == LookAhead::None {
            machine.translate_at_run_time(|| LookAhead::None);
        }
    })
}

/// A machine with the blob's memory and permissions, its code put in place by `load`, and the stack and
/// registers ready to run from its first byte.
pub fn ready_with(
    blob: &Blob,
    host: wcontainers_arm64::Host,
    load: impl FnOnce(&mut Machine),
) -> Machine {
    // The table covers the whole blob, every page of it that may run code, so code the program writes runs.
    let slots = (blob.bytes.len() as u64).div_ceil(4) as u32;
    let place = placement(blob);
    let mut machine = Machine::new(place.memory_bytes, blob.load, slots, Syscalls::StandIn);
    machine.set_host(host);
    for (range, permissions) in place.regions {
        machine.set_permissions(range, permissions);
    }
    load(&mut machine);
    machine.write(place.sp, &place.stack);
    let mut registers = RegisterBlock::ZERO;
    registers.pc = blob.load;
    registers.sp = place.sp;
    machine.set_registers(&registers);
    machine
}

/// What a program did, from why its thread stopped. Panics if it stopped without exiting or faulting.
pub fn ran(blob: &Blob, machine: &Machine, stopped: Stopped, registers: &RegisterBlock) -> Ran {
    // Docker reports a process killed by a signal as 128 plus the signal's number.
    let status = match stopped {
        // Linux reports the low 8 bits of the status.
        Stopped::Finished(status) => status & 0xff,
        Stopped::Exit(Exit::MemoryFault { .. }) => 128 + 11,
        Stopped::Exit(Exit::AlignmentFault { .. }) => 128 + 7,
        Stopped::Exit(Exit::IllegalInstruction) => 128 + 4,
        Stopped::Exit(Exit::Breakpoint { .. }) => 128 + 5,
        other => {
            let at = registers.pc.wrapping_sub(blob.load) as usize;
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
    Ran {
        stdout: machine.stdout().to_vec(),
        stderr: machine.stderr().to_vec(),
        status,
    }
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

/// The shortest of `runs` native runs of a blob on ARM64 Linux in Docker, in seconds, each timed by the
/// reference from making a child process to its end (so neither the container's nor the reference's start-up
/// is counted).
pub fn native_seconds(blob: &Blob, runs: u32) -> f64 {
    let reference = build_reference();
    let directory = out_directory().join("native");
    std::fs::create_dir_all(&directory).unwrap();
    let file = format!("{}.bin", unique(&blob.name));
    std::fs::write(directory.join(&file), &blob.bytes).unwrap();
    let script = format!(
        "/r/wcontainers-reference time {runs} /p/{file} {:#x} {STACK_TOP:#x} {:#x} {} > /dev/null",
        blob.load,
        blob.stack_bytes,
        headers_arguments(blob).join(" ")
    );
    let output = Command::new("docker")
        .args(["run", "--rm", "--network", "none", "--name"])
        .arg(container_name("timing"))
        .arg("-v")
        .arg(format!("{}:/r:ro", reference.display()))
        .arg("-v")
        .arg(format!("{}:/p:ro", directory.display()))
        .args(["alpine:latest", "sh", "-c", &script])
        .output()
        .expect("run docker");
    std::fs::remove_file(directory.join(&file)).unwrap();
    let times: Vec<f64> = String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|l| l.parse().expect("a time from the reference"))
        .collect();
    assert_eq!(times.len(), runs as usize, "native timing runs");
    times.into_iter().fold(f64::INFINITY, f64::min)
}

/// The reference's arguments for where a blob's program headers are and how many.
fn headers_arguments(blob: &Blob) -> Vec<String> {
    blob.headers_in_memory()
        .map(|(at, count)| vec![format!("{at:#x}"), count.to_string()])
        .unwrap_or_default()
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
        .arg(format!("{:#x}", blob.load))
        .arg(format!("{STACK_TOP:#x}"))
        .arg(format!("{:#x}", blob.stack_bytes))
        .args(headers_arguments(blob))
        .output()
        .expect("run docker");
    std::fs::remove_file(directory.join(&file)).unwrap();
    let status = output.status.code().expect("an exit status");
    // The runner's own failure is no result of the guest's: it fails the case, never compares.
    assert!(
        !(status == RUNNER_FAILED
            && output
                .stderr
                .windows(RUNNER_FAILED_SAYS.len())
                .any(|w| w == RUNNER_FAILED_SAYS)),
        "the reference runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ran {
        stdout: output.stdout,
        stderr: output.stderr,
        status,
    }
}

/// How the reference runner ends when it fails itself, and what it writes on stderr before.
const RUNNER_FAILED: i32 = 101;
const RUNNER_FAILED_SAYS: &[u8] = b"wcontainers-reference: ";
