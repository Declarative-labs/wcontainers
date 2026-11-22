//! Hypervisor.framework refuses a program without the hypervisor entitlement, which cargo's programs lack.

/// Runs a copy of this program signed ad hoc with the entitlement and exits as it did; in that copy, or on a host
/// other than macOS, returns.
pub fn be_signed() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        const SIGNED: &str = "WCONTAINERS_HYPERVISOR_SIGNED";
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
}
