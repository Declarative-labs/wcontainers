//! Records this host's chip: every listed instruction on every state the critical value generator makes.
//!
//! `cargo test -p wcontainers-processor-architecture-recording --test record -- <folder>` writes one file for each
//! instruction into the folder; without a folder it records nothing, so a plain `cargo test` passes by.

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
fn main() {
    use std::path::PathBuf;
    use std::time::Instant;

    use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
    use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
    use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
    use wcontainers_processor_architecture_recording::write_recordings;

    let Some(folder) = std::env::args_os().nth(1).map(PathBuf::from) else {
        println!("no folder given: nothing recorded");
        return;
    };
    #[cfg(target_os = "macos")]
    be_signed();
    const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
        CriticalValueProduct::new(Arm64InputDomain);

    let Ok(mut chip) = SingleStepMachine::new() else {
        panic!("this host's hypervisor makes no single step machine");
    };
    let started = Instant::now();
    if let Err(error) = write_recordings(&mut chip, &GENERATOR, &folder) {
        panic!("writing into {}: {error}", folder.display());
    }
    println!(
        "recorded into {} in {:.3} s, {:.3} s of it in the guest",
        folder.display(),
        started.elapsed().as_secs_f64(),
        chip.time_in_guest().as_secs_f64()
    );
}

#[cfg(not(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux"))))]
fn main() {
    println!("recording needs an ARM64 host with a hypervisor the single step machine drives");
}

/// Runs a copy of this program signed with the hypervisor entitlement, which Hypervisor.framework requires, and
/// exits as it did; in that copy, returns.
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn be_signed() {
    use std::process::Command;

    const SIGNED: &str = "WCONTAINERS_RECORD_SIGNED";
    const ENTITLEMENTS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../executor/implementation/hardware-virtualization/hypervisor.entitlements"
    );
    if std::env::var_os(SIGNED).is_some() {
        return;
    }
    let Ok(me) = std::env::current_exe() else {
        panic!("this program has no path of its own");
    };
    let copy = me.with_extension("signed");
    if let Err(error) = std::fs::copy(&me, &copy) {
        panic!("copying {}: {error}", me.display());
    }
    let signing = Command::new("codesign")
        .args(["-s", "-", "-f", "--entitlements", ENTITLEMENTS])
        .arg(&copy)
        .status();
    assert!(
        signing.is_ok_and(|status| status.success()),
        "codesign failed"
    );
    let Ok(status) = Command::new(&copy)
        .args(std::env::args_os().skip(1))
        .env(SIGNED, "1")
        .status()
    else {
        panic!("running {}", copy.display());
    };
    std::process::exit(status.code().unwrap_or(1));
}
