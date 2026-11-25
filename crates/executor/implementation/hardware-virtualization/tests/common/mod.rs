//! What the monitor's tests that run a program inside share: the signed copy, the nested build of the program
//! for the bare-metal target, the machine it boots on, settings from the environment and the load.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use wcontainers_executor_hardware_virtualization::{
    Host, Hypervisor, Loaded, Memory, VirtualMachine, load,
};
use wcontainers_executor_hardware_virtualization_inside::build_environment::AARCH64;
use wcontainers_executor_hardware_virtualization_inside::layout::MEMORY_BASE;
use wcontainers_executor_hardware_virtualization_inside::link::Start;
use wcontainers_translator::{BOUND, LookAhead};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{C_DRIVER, OUTPUT_FLAG, ZIG_TARGET_FLAG};
use wcontainers_translator_test::reference::{FEATURES_FLAG, TARGET_FLAG, TARGET_FOLDER_FLAG};

/// Bytes of guest-physical memory a test running one program reserves.
pub const MEMORY: u64 = 1 << 30;

/// The virtual CPUs of a test that needs only one.
pub const ONE_CPU: u64 = 1;

/// The inside's package, which holds the test programs.
const INSIDE: &str = "wcontainers-executor-hardware-virtualization-inside";

/// The inside's folder, beside this package's.
const INSIDE_FOLDER: &str = "hardware-virtualization-inside";

/// The bare-metal target of each host architecture the programs are built for.
const TARGETS: &[(&str, &str)] = &[(AARCH64, "aarch64-unknown-none-softfloat")];

/// The cargo profile the programs are built with.
const PROFILE: &str = wcontainers_translator_test::reference::RELEASE;

/// The inside's feature that builds its test programs which need test-only crates.
const TEST_PROGRAMS: &str = "kernel-test";

/// Jobs the nested build may run.
const JOBS: &str = "4";

/// Settings of the test's environment that would build the bare-metal program for the host.
const LEAKED: [&str; 2] = ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"];

/// What starts a hexadecimal setting.
const HEX_PREFIX: &str = "0x";

/// The radix of a hexadecimal setting.
const HEX_RADIX: u32 = 16;

/// Set in the environment of the signed copy the test runs itself as.
const SIGNED: &str = "WCONTAINERS_HYPERVISOR_TEST_SIGNED";

/// The load averages the system keeps: over 1, 5 and 15 minutes.
const LOAD_AVERAGES: usize = 3;

/// A number from the environment variable `name` (decimal, or hexadecimal after `0x`), or `default`.
pub fn setting(name: &str, default: u64) -> u64 {
    let Ok(text) = std::env::var(name) else {
        return default;
    };
    let parsed = match text.strip_prefix(HEX_PREFIX) {
        Some(hex) => u64::from_str_radix(hex, HEX_RADIX),
        None => text.parse(),
    };
    parsed.unwrap_or_else(|e| panic!("{name}={text}: {e}"))
}

/// Sets the most instructions of a native piece; 1 is look-ahead none.
pub const BOUND_VARIABLE: &str = "WCONTAINERS_NATIVE_BOUND";

/// The look-ahead the monitor translates at: [`BOUND_VARIABLE`]'s instructions per piece at most, the
/// translator's run-time bound when it is unset.
pub fn look_ahead() -> LookAhead {
    match setting(BOUND_VARIABLE, BOUND.into()) {
        0 => panic!("{BOUND_VARIABLE} is at least 1"),
        1 => LookAhead::None,
        n => LookAhead::Bounded(u32::try_from(n).expect("a bound fits u32")),
    }
}

/// KVM needs no signature: nothing to do.
#[cfg(not(target_os = "macos"))]
pub fn be_signed() {}

/// Hypervisor.framework refuses a binary without the hypervisor entitlement, which cargo's test binaries
/// lack: unless this is already the signed copy, the test signs a copy of itself ad hoc with the entitlement,
/// runs that, and exits as it did.
#[cfg(target_os = "macos")]
pub fn be_signed() {
    if std::env::var_os(SIGNED).is_some() {
        return;
    }
    let me = std::env::current_exe().expect("the test's own path");
    let copy = me.with_extension("signed");
    std::fs::copy(&me, &copy).expect("copying the test");
    let entitlements = concat!(env!("CARGO_MANIFEST_DIR"), "/hypervisor.entitlements");
    let signed = Command::new("codesign")
        .args(["-s", "-", "-f", "--entitlements", entitlements])
        .arg(&copy)
        .output()
        .expect("running codesign");
    assert!(
        signed.status.success(),
        "codesign: {}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let status = Command::new(&copy)
        .args(std::env::args_os().skip(1))
        .env(SIGNED, "1")
        .status()
        .expect("running the signed test");
    std::process::exit(status.code().unwrap_or(1));
}

/// Builds `program` of the inside's package for the host's bare-metal target and gives its ELF image.
pub fn build(program: &str) -> Vec<u8> {
    let target = TARGETS
        .iter()
        .find(|(arch, _)| *arch == std::env::consts::ARCH)
        .map(|(_, target)| *target)
        .expect("a bare-metal target for this host");
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(INSIDE);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .with_file_name(INSIDE_FOLDER)
        .join("Cargo.toml");
    let mut cargo = wcontainers_translator_test::reference::cargo();
    cargo
        .args([
            "build",
            "--bin",
            program,
            TARGET_FLAG,
            target,
            "--profile",
            PROFILE,
        ])
        .args([FEATURES_FLAG, TEST_PROGRAMS])
        .args(["--jobs", JOBS])
        .arg("--manifest-path")
        .arg(&manifest)
        .arg(TARGET_FOLDER_FLAG)
        .arg(&target_dir);
    for leaked in LEAKED {
        cargo.env_remove(leaked);
    }
    let status = cargo.status().expect("running cargo for the program");
    assert!(status.success(), "building {program} failed");
    std::fs::read(target_dir.join(target).join(PROFILE).join(program)).expect("the program's image")
}

/// A machine with `memory` bytes of guest-physical memory and `program` loaded for `cpus` virtual CPUs.
pub fn machine(
    program: &[u8],
    memory: u64,
    cpus: u64,
) -> (<Host as Hypervisor>::VirtualMachine, Memory, Loaded, Start) {
    let machine = Host::machine(cpus).expect("making the machine");
    let reserved = Memory::reserve(memory).expect("reserving memory");
    machine
        .map(
            std::ptr::NonNull::new(reserved.host(MEMORY_BASE)).expect("reserved"),
            MEMORY_BASE,
            memory,
        )
        .expect("mapping memory");
    let loaded = load(&reserved, program, cpus).expect("loading the inside");
    let interrupts = machine.interrupts().expect("interrupt numbers");
    let start = Start {
        memory,
        host_page: page_size(),
        timer_interrupt: interrupts.timer.into(),
        first_raised_interrupt: interrupts.first_raised.into(),
        ..Start::default()
    };
    (machine, reserved, loaded, start)
}

pub fn page_size() -> u64 {
    // SAFETY: a plain query.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}

pub fn load_average() -> String {
    let mut loads = [0f64; LOAD_AVERAGES];
    // SAFETY: fills three values.
    unsafe { libc::getloadavg(loads.as_mut_ptr(), loads.len() as i32) };
    format!("{:.1} {:.1} {:.1}", loads[0], loads[1], loads[2])
}

/// The kernel test crate's programs, of which `hello` is the bar's.
pub const KERNEL_TEST_GUEST: &str = "../unicorn/guest";

/// This track's own test programs.
pub const OWN_GUEST: &str = "tests/guest";

/// Whether a program is linked with the C library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CLibrary {
    Without,
    Musl,
}

/// The C compiler's target and the program's extension.
pub const GUEST_TARGET: &str = <wcontainers_arm64::Arm64 as GuestTest>::C_TARGET;
pub const C_SOURCE: &str = "c";
pub const OBJECT: &str = "o";

/// Where [`compile`] puts the program `name`.
pub fn compiled(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// Builds a static ARM64 Linux program from `directory/name.c` with zig's C compiler.
pub fn compile(directory: &str, name: &str, library: CLibrary) -> Vec<u8> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(directory)
        .join(name)
        .with_extension(C_SOURCE);
    let out = compiled(name);
    let object = out.with_extension(OBJECT);
    // Compiled with the C library's headers; linked with its code or without it.
    zig(Command::new("zig")
        .args([C_DRIVER, ZIG_TARGET_FLAG, GUEST_TARGET, "-O2", "-c"])
        .arg(guest_programs_include())
        .arg(OUTPUT_FLAG)
        .arg(&object)
        .arg(&source));
    zig(Command::new("zig")
        .args([C_DRIVER, ZIG_TARGET_FLAG, GUEST_TARGET, "-static"])
        .args((library == CLibrary::Without).then_some("-nostdlib"))
        .arg(OUTPUT_FLAG)
        .arg(&out)
        .arg(&object));
    std::fs::read(&out).expect("the built program")
}

/// The benchmark program of bench.c's workloads.
pub const BENCH: &str = "bench";

/// The benchmark program of calls between two functions.
pub const CALLS: &str = "calls";

/// Where the benchmark programs' sources are, from this package.
pub const PROGRAMS_DIRECTORY: &str = "../../../translator-test/programs";

/// The include flag of the guest's own folder of those programs (`programs/arm64`), whose headers the shared
/// programs include, as the translator's test harness builds them.
fn guest_programs_include() -> String {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PROGRAMS_DIRECTORY)
        .join(<wcontainers_arm64::Arm64 as GuestTest>::NAME);
    format!("-I{}", folder.display())
}

/// Where the speed test places a benchmark program: under the native ceiling, and above the low addresses macOS
/// keeps for itself (it moves a mapping asked for under 0x70_0000_0000 elsewhere).
pub const IMAGE_BASE: u64 = 1 << 40;

/// A freestanding program's compiler flags beside the target. x18 is the platform's own on macOS, which may change it
/// at any time, so no side's code uses it (zig's C compiler takes that as a feature of the processor, and drops
/// `-ffixed-x18`).
const FREESTANDING_FLAGS: [&str; 3] = ["-O2", "-ffreestanding", "-mcpu=generic+reserve_x18"];

/// A freestanding program's linker flags beside the place. Each segment starts a page: the kernel maps a writable
/// segment with no bytes in the file only from the first page boundary after its start (see the status file).
const FREESTANDING_LINK_FLAGS: [&str; 3] = ["-static", "-z", "separate-loadable-segments"];

/// Builds the freestanding static program `name` from `directory/source.c` with `defines`, placed at `base` (the
/// linker's own place when none), and gives its path.
pub fn freestanding(
    directory: &str,
    name: &str,
    source: &str,
    defines: &[String],
    base: Option<u64>,
) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(directory)
        .join(source)
        .with_extension(C_SOURCE);
    let out = compiled(name);
    let object = out.with_extension(OBJECT);
    zig(Command::new("zig")
        .args([C_DRIVER, ZIG_TARGET_FLAG, GUEST_TARGET, "-c"])
        .args(FREESTANDING_FLAGS)
        .arg(guest_programs_include())
        .args(defines.iter().map(|define| format!("-D{define}")))
        .arg(OUTPUT_FLAG)
        .arg(&object)
        .arg(&source));
    zig(Command::new("zig")
        .arg("ld.lld")
        .args(FREESTANDING_LINK_FLAGS)
        .args(base.map(|base| format!("--image-base={base:#x}")))
        .arg(OUTPUT_FLAG)
        .arg(&out)
        .arg(&object));
    out
}

pub fn zig(command: &mut Command) {
    let output = command
        .output()
        .expect("running zig cc; run the test inside `nix develop`");
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
