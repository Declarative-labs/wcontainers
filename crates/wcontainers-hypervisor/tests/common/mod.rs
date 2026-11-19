//! What the monitor's tests that run a program inside share: the signed copy, the nested build of the program
//! for the bare-metal target, the machine it boots on, settings from the environment and the load.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use wcontainers_hypervisor::{Host, Hypervisor, Loaded, Memory, VirtualMachine, load};
use wcontainers_inside::layout::MEMORY_BASE;
use wcontainers_inside::link::Start;

/// The inside's package, which holds the test programs.
const INSIDE: &str = "wcontainers-inside";

/// The bare-metal target of each host architecture the programs are built for.
const TARGETS: &[(&str, &str)] = &[("aarch64", "aarch64-unknown-none")];

/// The cargo profile the programs are built with.
const PROFILE: &str = "release";

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

/// Hypervisor.framework refuses a binary without the hypervisor entitlement, which cargo's test binaries
/// lack: unless this is already the signed copy, the test signs a copy of itself ad hoc with the entitlement,
/// runs that, and exits as it did.
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
        .with_file_name(INSIDE)
        .join("Cargo.toml");
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args([
            "build",
            "--bin",
            program,
            "--target",
            target,
            "--profile",
            PROFILE,
        ])
        .args(["--jobs", JOBS])
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
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
    let machine = Host::machine().expect("making the machine");
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
