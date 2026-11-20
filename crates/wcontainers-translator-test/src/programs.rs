//! C programs built as flat blobs, run through the translator and on real Linux of the guest's CPU.

use crate::guest::GuestTest;
use crate::layout::initial_stack;
use crate::reference::{BlobRan, BlobRequest, RUNNER_EXECUTABLE, with_reference};
use crate::runner::{Fault, Machine, Stopped, Syscalls};
use linux_raw_sys::general::{SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_translator::{LookAhead, Omit, Optimise};

/// Where every blob is linked, and loaded unless it is moved.
pub const LOAD: u64 = 0x40_0000;
/// Where a position independent blob is loaded, away from where it is linked.
pub const MOVED: u64 = 0x2345_6000;
/// The top of the stack, above the reach of the reference's own randomised heap.
pub const STACK_TOP: u64 = 0x4810_0000;
/// The stack a blob gets unless it asks for more.
pub const STACK_BYTES: u64 = 1 << 20;

/// How the code is generated: for the fixed address it is linked and loaded at, or position independent
/// (`-fPIE`, a static position independent program that relocates itself at start), loaded away from where
/// it is linked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Fixed,
    Independent,
}

/// The C compiler's flags for code at a fixed address.
const FIXED_C_FLAGS: [&str; 3] = ["-static", "-fno-pic", "-no-pie"];
/// The C compiler's flags for position independent code.
const INDEPENDENT_C_FLAGS: [&str; 3] = ["-static-pie", "-fPIE", "-Wl,-z,norelro"];
/// rustc's flags for code at a fixed address.
const FIXED_RUST_FLAGS: [&str; 2] = ["-C", "relocation-model=static"];
/// rustc's flags for position independent code.
const INDEPENDENT_RUST_FLAGS: [&str; 4] =
    ["-C", "relocation-model=pie", "-C", "link-arg=-znorelro"];
/// What the name of a blob of position independent code ends with.
const INDEPENDENT_SUFFIX: &str = "-pic";

impl Position {
    /// The C compiler's flags for it.
    pub fn flags(self) -> &'static [&'static str] {
        match self {
            Position::Fixed => &FIXED_C_FLAGS,
            Position::Independent => &INDEPENDENT_C_FLAGS,
        }
    }

    /// rustc's flags for it.
    fn rust_flags(self) -> &'static [&'static str] {
        match self {
            Position::Fixed => &FIXED_RUST_FLAGS,
            Position::Independent => &INDEPENDENT_RUST_FLAGS,
        }
    }

    /// What a blob's name ends with.
    pub fn suffix(self) -> &'static str {
        match self {
            Position::Fixed => "",
            Position::Independent => INDEPENDENT_SUFFIX,
        }
    }

    /// Where a blob of it is loaded.
    pub fn load(self) -> u64 {
        match self {
            Position::Fixed => LOAD,
            Position::Independent => MOVED,
        }
    }
}

/// A flat blob and how much of its start is code.
pub struct Blob {
    /// The program and optimisation level, such as "hello-O2".
    pub name: String,
    pub bytes: Vec<u8>,
    pub code_bytes: u64,
    /// Where it is placed in guest memory.
    pub load: u64,
    /// Where its first byte is linked, when its code names its own addresses absolutely (see
    /// `wcontainers_translator::translate`).
    pub link_address: Option<u64>,
    /// The blob offset of its program's `main`, from the ELF's symbols.
    pub main: Option<u64>,
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

/// The folder of the guest's own program sources and headers, `programs/<guest>`.
fn guest_directory<G: GuestTest>() -> PathBuf {
    source_directory().join(G::NAME)
}

/// The source of a program: `<name>.c` (or `.cpp`, `.rs`, `.S`) in the guest's own folder, else among the
/// shared ones.
pub fn source<G: GuestTest>(name: &str) -> PathBuf {
    [guest_directory::<G>(), source_directory()]
        .iter()
        .flat_map(|directory| {
            ["c", "cpp", "rs", "S"].map(|extension| directory.join(format!("{name}.{extension}")))
        })
        .find(|source| source.exists())
        .unwrap_or_else(|| panic!("no program {name} for {}", G::NAME))
}

/// The longest one zig command may take. A build takes seconds; past this it is stuck, not slow.
const ZIG_LIMIT: Duration = Duration::from_secs(600);

/// Run zig with a cache of this checkout's own, so no zig process of another checkout holds its locks, and
/// fail with a message when it does not end.
pub(crate) fn zig(arguments: &[&str]) {
    use wait_timeout::ChildExt;
    let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/zig-cache");
    let mut child = Command::new("zig")
        .args(arguments)
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .spawn()
        .expect("run zig (from the Nix shell)");
    let Some(status) = child.wait_timeout(ZIG_LIMIT).expect("wait for zig") else {
        let _ = child.kill();
        let _ = child.wait();
        panic!("zig {arguments:?} did not end in {ZIG_LIMIT:?}");
    };
    assert!(status.success(), "zig {arguments:?} failed");
}

/// An ordinary static executable of a C program for zig's target `c_target` (such as "x86_64-linux-musl"),
/// built at -O1 with the C library; `name` names its files while it is built.
pub fn executable(c_target: &str, name: &str, source: &str) -> Vec<u8> {
    let own = unique(name);
    std::fs::create_dir_all(out_directory()).unwrap();
    let (c, elf) = (
        out_directory().join(format!("{own}.c")),
        out_directory().join(format!("{own}.elf")),
    );
    std::fs::write(&c, source).unwrap();
    let (c_path, elf_path) = (c.display().to_string(), elf.display().to_string());
    zig(&[
        "cc", "-target", c_target, "-O1", "-static", "-o", &elf_path, &c_path,
    ]);
    let bytes = std::fs::read(&elf).unwrap();
    std::fs::remove_file(&c).unwrap();
    std::fs::remove_file(&elf).unwrap();
    bytes
}

/// Check that `run` runs ordinary executables for `c_target` as Linux starts a program: hello world's output
/// (its name a blob's) and status, a write through a null pointer ended by SIGSEGV, and a loop that does not
/// end ended by the time limit's SIGALRM.
pub fn check_ordinary_programs(c_target: &str, mut run: impl FnMut(&[u8]) -> Ran) {
    let hello = std::fs::read_to_string(source_directory().join("hello.c")).unwrap();
    for (name, source, stdout, status) in [
        (
            "hello",
            hello.as_str(),
            &b"hello, world\n1 blob beef ! -1234567890123\n"[..],
            3,
        ),
        (
            "null",
            "int main(void) { *(volatile int *)0 = 1; return 0; }",
            b"",
            128 + 11,
        ),
        (
            "forever",
            "int main(void) { volatile int n = 0; for (;;) n++; }",
            b"",
            128 + 14,
        ),
    ] {
        let ran = run(&executable(c_target, name, source));
        assert_eq!(
            (ran.stdout.as_slice(), ran.status),
            (stdout, status),
            "{name} on the reference, stderr {:?}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }
}

/// What a program is built against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Library {
    /// Nothing: the program brings its own `_start` and system calls, and uses only general registers.
    None,
    /// The C library (musl, as zig builds it), started by its own `_start`.
    C,
}

/// Build a program (see [`source`]: C, C++ with its library, Rust with its standard library, or assembly) at
/// an optimisation level, such as "-O2", into a flat blob. The guest's folder is on the include path.
pub fn build<G: GuestTest>(name: &str, optimisation: &str, library: Library) -> Blob {
    build_with::<G>(name, optimisation, library, &[], Position::Fixed)
}

/// [`build`] with preprocessor definitions, such as "SCALE=1", for the code's position.
pub fn build_with<G: GuestTest>(
    name: &str,
    optimisation: &str,
    library: Library,
    defines: &[&str],
    position: Position,
) -> Blob {
    let out = out_directory();
    std::fs::create_dir_all(&out).unwrap();
    let mut stem = format!("{name}{optimisation}{}", position.suffix());
    for define in defines {
        stem.push('-');
        stem.push_str(&define.replace('=', ""));
    }
    let defines: Vec<String> = defines.iter().map(|d| format!("-D{d}")).collect();
    // Tests build the same program at once, so each build works under its own names.
    let own = unique(&stem);
    let elf = out.join(format!("{own}.elf"));
    let source = source::<G>(name);
    let script = source_directory().join("flat.ld");
    let text = |p: &Path| p.to_str().unwrap().to_string();
    if source.extension().unwrap() == "rs" {
        rust::<G>(&source, &elf, &script, optimisation, position);
        return flatten(&elf, &own, stem);
    }
    let script = format!("-Wl,-T,{}", text(&script));
    let include = format!("-I{}", text(&guest_directory::<G>()));
    let driver = if source.extension().unwrap() == "cpp" {
        "c++"
    } else {
        "cc"
    };
    let mut arguments = vec![driver, "-target", G::C_TARGET, optimisation];
    arguments.extend(position.flags());
    arguments.extend([
        "-fno-sanitize=all",
        &script,
        &include,
        "-Wl,--build-id=none",
    ]);
    if library == Library::None {
        arguments.extend(["-ffreestanding", "-nostdlib"]);
        arguments.extend(G::GENERAL_ONLY);
        arguments.extend(["-fno-stack-protector", "-fwrapv"]);
    }
    arguments.extend(defines.iter().map(String::as_str));
    let (elf_text, source_text) = (text(&elf), text(&source));
    arguments.extend(["-o", &elf_text, &source_text]);
    zig(&arguments);
    flatten(&elf, &own, stem)
}

/// Build a Rust program with its standard library, statically, linked with `script`.
fn rust<G: GuestTest>(
    source: &Path,
    elf: &Path,
    script: &Path,
    optimisation: &str,
    position: Position,
) {
    let level = optimisation.trim_start_matches("-O");
    let status = Command::new("rustc")
        .args(["--edition", "2024", "--target", G::REFERENCE.rust_target])
        .args(["-C", &format!("opt-level={level}")])
        .args(["-C", "panic=unwind"])
        .args(position.rust_flags())
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

/// The symbol of a program's main function.
const MAIN: &str = "main";
/// The section of a program's instructions.
const TEXT_SECTION: &str = ".text";

/// The flat blob of a linked program, `own` being the unique name its build works under, loaded at `MOVED`
/// if the linker made it position independent. The ELF file is kept, under the blob's name, for looking at.
pub(crate) fn flatten(elf: &Path, own: &str, name: String) -> Blob {
    use object::read::elf::FileHeader;
    use object::{Object, ObjectSection, ObjectSymbol};
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
    let position = if parsed.elf_header().e_type(parsed.endian()) == object::elf::ET_DYN {
        Position::Independent
    } else {
        Position::Fixed
    };
    // The image starts with its instructions.
    let text = parsed
        .section_by_name(TEXT_SECTION)
        .expect("a text section")
        .address();
    let main = parsed
        .symbols()
        .find(|s| s.name() == Ok(MAIN))
        .map(|s| s.address() - text);
    let mut bytes = std::fs::read(&built).unwrap();
    bytes.resize(bytes.len().next_multiple_of(8), 0);
    let at = bytes.len() as u64;
    bytes.extend_from_slice(object::pod::bytes_of_slice(headers));
    let blob = Blob {
        bytes,
        code_bytes: std::fs::metadata(&code).unwrap().len(),
        load: position.load(),
        link_address: (position == Position::Fixed).then_some(text),
        main,
        program_headers: Some((at, headers.len() as u64)),
        name,
        stack_bytes: STACK_BYTES,
    };
    std::fs::remove_file(&built).unwrap();
    std::fs::remove_file(&code).unwrap();
    std::fs::rename(elf, out.join(format!("{}.elf", blob.name))).unwrap();
    blob
}

/// The offsets of the instructions in a range of code, decoding forward from its start; where nothing
/// decodes, the next slot.
pub fn boundaries<G: GuestTest>(code: &[u8]) -> Vec<u64> {
    let mut at = 0;
    let mut all = Vec::new();
    while at < code.len() {
        all.push(at as u64);
        at += G::decode(&code[at..]).map_or(G::ALIGN as usize, |(_, length)| usize::from(length));
    }
    all
}

/// What a program did: its output and exit status.
#[derive(Debug, PartialEq, Eq)]
pub struct Ran {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

/// The page after the first page of a blob: the data of the hand-made programs whose code fits one page.
pub const PAGE_AFTER_LOAD: u64 = LOAD + PAGE_SIZE;

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

pub fn placement<G: GuestTest>(blob: &Blob) -> Placement {
    let end = blob.load + (blob.bytes.len() as u64).next_multiple_of(PAGE_SIZE);
    let (sp, stack) = initial_stack(STACK_TOP, blob.headers_in_memory(), G::AUXILIARY);
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
pub fn translated<G: GuestTest>(blob: &Blob, look_ahead: LookAhead) -> (Ran, Machine<G>, G::Block) {
    translated_for::<G>(blob, (look_ahead, Optimise::Speed))
}

/// [`translated`] for speed or for size.
pub fn translated_for<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
) -> (Ran, Machine<G>, G::Block) {
    let (ran, machine, registers, _) = translated_omitting::<G>(blob, how, Omit::default());
    (ran, machine, registers)
}

/// [`translated`] without some checks, also giving how long the guest ran (translating and compiling not
/// included).
pub fn translated_omitting<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
    omit: Omit,
) -> (Ran, Machine<G>, G::Block, Duration) {
    run_blob::<G>(blob, how, omit, false)
}

/// [`translated_for`] with the thread stopping at every stop check it reaches and carrying on each time.
pub fn translated_stopping_everywhere<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
) -> (Ran, Machine<G>, G::Block) {
    let (ran, machine, registers, _) = run_blob::<G>(blob, how, Omit::default(), true);
    (ran, machine, registers)
}

fn run_blob<G: GuestTest>(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    stop_everywhere: bool,
) -> (Ran, Machine<G>, G::Block, Duration) {
    let mut machine = ready_for::<G>(blob, look_ahead, optimise, omit);
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
pub fn ready<G: GuestTest>(blob: &Blob, look_ahead: LookAhead, omit: Omit) -> Machine<G> {
    ready_for::<G>(blob, look_ahead, Optimise::Speed, omit)
}

/// [`ready`], translated for `optimise`.
pub fn ready_for<G: GuestTest>(
    blob: &Blob,
    look_ahead: LookAhead,
    optimise: Optimise,
    omit: Omit,
) -> Machine<G> {
    ready_on::<G>(
        blob,
        (look_ahead, optimise),
        omit,
        crate::runner::probe::<G>(),
    )
}

/// [`ready`], translated for a host with these facts instead of the probe's.
pub fn ready_on<G: GuestTest>(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    host: G::Host,
) -> Machine<G> {
    ready_with::<G>(blob, host, |machine| {
        machine.optimise_for(optimise);
        machine.load_omitting(
            &blob.bytes,
            0..blob.code_bytes,
            (blob.load, blob.link_address),
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
pub fn ready_with<G: GuestTest>(
    blob: &Blob,
    host: G::Host,
    load: impl FnOnce(&mut Machine<G>),
) -> Machine<G> {
    // The table covers the whole blob, every page of it that may run code, so code the program writes runs.
    let slots = (blob.bytes.len() as u64).div_ceil(G::ALIGN) as u32;
    let place = placement::<G>(blob);
    let mut machine = Machine::new(place.memory_bytes, blob.load, slots, Syscalls::StandIn);
    machine.set_host(host);
    for (range, permissions) in place.regions {
        machine.set_permissions(range, permissions);
    }
    load(&mut machine);
    machine.write(place.sp, &place.stack);
    machine.set_registers(&G::initial_block(blob.load, place.sp));
    machine
}

/// What a program did, from why its thread stopped. Panics if it stopped without exiting or faulting.
pub fn ran<G: GuestTest>(
    blob: &Blob,
    machine: &Machine<G>,
    stopped: Stopped,
    registers: &G::Block,
) -> Ran {
    let finished = |status| Ran {
        stdout: machine.stdout().to_vec(),
        stderr: machine.stderr().to_vec(),
        status,
    };
    let signal = match stopped {
        // Linux reports the low 8 bits of the status.
        Stopped::Finished(status) => return finished(i32::from(status as u8)),
        Stopped::Exit(Exit::MemoryFault { .. }) => SIGSEGV,
        Stopped::Exit(Exit::AlignmentFault { .. }) => SIGBUS,
        Stopped::Exit(Exit::IllegalInstruction) => SIGILL,
        Stopped::Exit(Exit::Breakpoint { .. }) | Stopped::Fault(Fault::SingleStep) => SIGTRAP,
        Stopped::Fault(Fault::DivideError | Fault::FloatException(_)) => SIGFPE,
        Stopped::Fault(Fault::GeneralProtection | Fault::Overflow) => SIGSEGV,
        Stopped::Fault(Fault::Stack) => SIGBUS,
        other => {
            let pc = u64::from_le_bytes(
                zerocopy::IntoBytes::as_bytes(registers)[G::PC_OFFSET..][..8]
                    .try_into()
                    .unwrap(),
            );
            let at = (pc.wrapping_sub(blob.load) as usize).min(blob.bytes.len());
            let code = &blob.bytes[at..(at + G::MAX_LENGTH as usize).min(blob.bytes.len())];
            panic!(
                "the program stopped without exiting: {other:?} at {pc:#x}, bytes {code:02x?} ({})",
                G::disassemble(code)
            )
        }
    };
    // A process killed by a signal is reported as a shell does, 128 plus the signal's number.
    finished(128 + signal as i32)
}

/// Every byte the guest may reach: the blob's pages, then the stack.
pub fn reachable_memory<G: GuestTest>(machine: &mut Machine<G>, blob: &Blob) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (range, _) in placement::<G>(blob).regions {
        let mut part = vec![0; (range.end - range.start) as usize];
        machine.read(range.start, &mut part);
        bytes.extend(part);
    }
    bytes
}

/// The shortest of `runs` native runs of a blob on the reference, in seconds, each timed by the reference
/// from making a child process to its end (so the reference's start-up is not counted).
/// Linux's file of load averages.
const LINUX_LOAD: &str = "/proc/loadavg";
/// macOS's tool and name for them.
const SYSCTL: &str = "sysctl";
const MACOS_LOAD: &str = "vm.loadavg";
/// sysctl's switch for the value alone.
const VALUE_ONLY: &str = "-n";

/// This machine's load averages, recorded beside every time.
pub fn load() -> String {
    if cfg!(target_os = "linux") {
        return std::fs::read_to_string(LINUX_LOAD)
            .expect("the load averages")
            .trim()
            .to_string();
    }
    let output = Command::new(SYSCTL)
        .args([VALUE_ONLY, MACOS_LOAD])
        .output()
        .expect("sysctl");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn native_seconds<G: GuestTest>(blob: &Blob, runs: u32) -> f64 {
    on_reference::<G>(blob, runs.max(1)).seconds
}

/// Run a blob natively on the reference.
pub fn native<G: GuestTest>(blob: &Blob) -> Ran {
    let ran = on_reference::<G>(blob, 0);
    Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    }
}

/// A blob run natively on the reference, with the signal, `si_code`, pc and general registers of the signal
/// that ended it, if one did (`BlobRan::stop`).
pub fn native_with_stop<G: GuestTest>(blob: &Blob) -> (Ran, Option<(u32, u32, Vec<u64>)>) {
    let ran = on_reference::<G>(blob, 0);
    let stop = ran.stop;
    let ran = Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    };
    (ran, stop)
}

fn on_reference<G: GuestTest>(blob: &Blob, runs: u32) -> BlobRan {
    let request = BlobRequest {
        bytes: &blob.bytes,
        load: blob.load,
        stack_top: STACK_TOP,
        stack_bytes: blob.stack_bytes,
        headers: blob.headers_in_memory(),
        auxiliary: G::AUXILIARY,
        stdin: &[],
        runs,
    };
    runner_did_not_fail(with_reference(&G::REFERENCE, |reference| {
        reference.blob(&request)
    }))
}

/// Run an ordinary static executable on the reference, as Linux starts a program.
pub fn native_program<G: GuestTest>(executable: &[u8]) -> Ran {
    let ran = runner_did_not_fail(with_reference(&G::REFERENCE, |reference| {
        reference.program(executable)
    }));
    Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    }
}

/// The result of a blob or a program. The runner's own failure, in its child or before, is no result of
/// the guest's: it fails the case, never compares.
fn runner_did_not_fail(ran: Result<BlobRan, String>) -> BlobRan {
    let ran = ran.unwrap_or_else(|e| panic!("the reference runner failed: {e}"));
    let says = format!("{RUNNER_EXECUTABLE}{RUNNER_FAILED_AFTER_NAME}");
    assert!(
        !(ran.status == RUNNER_FAILED
            && ran.stderr.windows(says.len()).any(|w| w == says.as_bytes())),
        "the reference runner failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    ran
}

/// How the reference runner ends when it fails itself, and what it writes on stderr before.
const RUNNER_FAILED: i32 = 101;
const RUNNER_FAILED_AFTER_NAME: &str = ": ";
