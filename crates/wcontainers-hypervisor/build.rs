//! Builds the inside's programs for the bare-metal target of the host's architecture and writes
//! `programs.rs`, which holds each program's bytes, into the build's output directory.

use std::{env, fs, path::PathBuf, process::Command};

use wcontainers_inside::build_environment::{CARGO, OUT_DIR, TARGET_ARCH, TARGET_OS};

/// The inside's package.
const INSIDE: &str = "wcontainers-inside";

/// The inside's programs: the floor's test cases.
const PROGRAMS: &[&str] = &["floor-cases"];

/// The bare-metal target of each host architecture the inside is built for.
const TARGETS: &[(&str, &str)] = &[("aarch64", "aarch64-unknown-none")];

/// The cargo profile the inside is built with.
const PROFILE: &str = "release";

/// The system whose SDK may lack [`LATER_CALLS`].
const MACOS: &str = "macos";

/// Cargo's command and options that build the inside.
const BUILD: [&str; 2] = ["build", "--bins"];
const PACKAGE_OPTION: &str = "--package";
const TARGET_OPTION: &str = "--target";
const PROFILE_OPTION: &str = "--profile";
const MANIFEST_OPTION: &str = "--manifest-path";
const TARGET_DIR_OPTION: &str = "--target-dir";

/// The file name of a package's manifest.
const MANIFEST_FILE: &str = "Cargo.toml";

/// Settings of the outer build that would leak into the nested one and build the inside for the host.
const LEAKED: [&str; 5] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET",
    "RUSTC_WRAPPER",
];

/// A program's file name becomes a constant's name with each of these replaced.
const NAME_JOINERS: (char, &str) = ('-', "_");

/// The file the build writes the programs into; the monitor includes it from the output directory.
const PROGRAMS_FILE: &str = "programs.rs";

/// Hypervisor.framework calls of macOS 15 and later that the build's SDK may not declare: the linker leaves
/// them to be found when the program loads, on a system that has them.
const LATER_CALLS: &[&str] = &[
    "hv_gic_config_create",
    "hv_gic_config_set_distributor_base",
    "hv_gic_config_set_redistributor_base",
    "hv_gic_create",
    "hv_gic_get_intid",
    "hv_gic_get_spi_interrupt_range",
    "hv_gic_set_spi",
];

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let inside = manifest.with_file_name(INSIDE);
    println!("cargo::rerun-if-changed={}", inside.display());
    if env::var(TARGET_OS).as_deref() == Ok(MACOS) {
        for call in LATER_CALLS {
            println!("cargo::rustc-link-arg=-Wl,-U,_{call}");
        }
    }
    let out = PathBuf::from(env::var(OUT_DIR).expect("output directory"));
    let arch = env::var(TARGET_ARCH).expect("target architecture");
    let mut programs = String::new();
    if let Some((_, target)) = TARGETS.iter().find(|(a, _)| *a == arch) {
        let target_dir = out.join(INSIDE);
        let mut cargo = Command::new(env::var(CARGO).expect("cargo"));
        cargo
            .args(BUILD)
            .args([PACKAGE_OPTION, INSIDE, TARGET_OPTION, target])
            .args([PROFILE_OPTION, PROFILE])
            .arg(MANIFEST_OPTION)
            .arg(inside.join(MANIFEST_FILE))
            .arg(TARGET_DIR_OPTION)
            .arg(&target_dir);
        for leaked in LEAKED {
            cargo.env_remove(leaked);
        }
        let status = cargo.status().expect("running cargo for the inside");
        assert!(status.success(), "building the inside failed");
        for program in PROGRAMS {
            let path = target_dir.join(target).join(PROFILE).join(program);
            let name = program
                .to_uppercase()
                .replace(NAME_JOINERS.0, NAME_JOINERS.1);
            programs += &format!(
                "/// The inside's `{program}` program.\npub const {name}: &[u8] = include_bytes!({path:?});\n"
            );
        }
    }
    fs::write(out.join(PROGRAMS_FILE), programs).expect("writing the programs");
}
