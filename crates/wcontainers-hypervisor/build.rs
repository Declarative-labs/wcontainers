//! Builds the inside's programs for the bare-metal target of the host's architecture and writes
//! `programs.rs`, which holds each program's bytes, into the build's output directory.

use std::{env, fs, path::PathBuf, process::Command};

/// The inside's package.
const INSIDE: &str = "wcontainers-inside";

/// The inside's programs: the floor's test cases.
const PROGRAMS: &[&str] = &["floor-cases"];

/// The bare-metal target of each host architecture the inside is built for.
const TARGETS: &[(&str, &str)] = &[("aarch64", "aarch64-unknown-none")];

/// The cargo profile the inside is built with.
const PROFILE: &str = "release";

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
    "hv_vm_config_get_el2_supported",
    "hv_vm_config_set_el2_enabled",
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let inside = manifest.with_file_name(INSIDE);
    println!("cargo::rerun-if-changed={}", inside.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for call in LATER_CALLS {
            println!("cargo::rustc-link-arg=-Wl,-U,_{call}");
        }
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("output directory"));
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target architecture");
    let mut programs = String::new();
    if let Some((_, target)) = TARGETS.iter().find(|(a, _)| *a == arch) {
        let target_dir = out.join(INSIDE);
        let mut cargo = Command::new(env::var("CARGO").expect("cargo"));
        cargo
            .args([
                "build",
                "--package",
                INSIDE,
                "--target",
                target,
                "--profile",
                PROFILE,
                "--bins",
            ])
            .arg("--manifest-path")
            .arg(inside.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target_dir);
        for leaked in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_TARGET",
            "RUSTC_WRAPPER",
        ] {
            cargo.env_remove(leaked);
        }
        let status = cargo.status().expect("running cargo for the inside");
        assert!(status.success(), "building the inside failed");
        for program in PROGRAMS {
            let path = target_dir.join(target).join(PROFILE).join(program);
            let name = program.to_uppercase().replace('-', "_");
            programs += &format!(
                "/// The inside's `{program}` program.\npub const {name}: &[u8] = include_bytes!({path:?});\n"
            );
        }
    }
    fs::write(out.join("programs.rs"), programs).expect("writing programs.rs");
}
