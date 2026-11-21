//! Writes each iced `Code`'s claim from iced's own `instr_info` tables, which the shipped decoder does not
//! carry.

use iced_x86::Code;
use std::fmt::Write;

include!("src/claims.rs");

/// The claim of one `Code`, by its features against the lists: kernel only when the CPU allows it only at CPL 0
/// (and `rdpmc`, which Linux leaves to the kernel by keeping CR4.PCE clear) and every x86-64 CPU has it; a told
/// feature's when its features are the baseline's and that one (an instruction of it on a CPU without it is
/// one that CPU does not have, kernel only or not); else never offered.
fn claim(code: Code) -> u8 {
    let kernel = code.is_privileged() || code == Code::Rdpmc;
    if matches!(code, Code::Lahf | Code::Sahf) {
        return told(LAHF_SAHF_FAMILY);
    }
    let names: Vec<String> = code
        .cpuid_features()
        .iter()
        .map(|f| format!("{f:?}"))
        .filter(|n| !BASELINE_FEATURES.contains(&n.as_str()))
        .collect();
    match &names[..] {
        names if names.iter().all(|n| KERNEL_FEATURES.contains(&n.as_str())) => {
            if kernel {
                CLAIM_KERNEL
            } else {
                CLAIM_BASELINE
            }
        }
        [name] if TOLD_FEATURES.contains(&name.as_str()) => told(name),
        names => {
            assert!(
                !names.iter().all(|n| TOLD_FEATURES.contains(&n.as_str())),
                "{code:?} needs more than one told feature: {names:?}"
            );
            CLAIM_NEVER
        }
    }
}

fn told(name: &str) -> u8 {
    let at = TOLD_FEATURES
        .iter()
        .position(|&n| n == name)
        .expect("a told feature");
    CLAIM_FIRST_TOLD + u8::try_from(at).expect("a claim byte")
}

fn main() {
    println!("cargo::rerun-if-changed=src/claims.rs");
    let mut out =
        String::from("/// Each `Code`'s claim group, by its number.\npub static GROUPS: [u8; ");
    write!(out, "{}] = [", Code::values().len()).unwrap();
    for code in Code::values() {
        write!(out, "{},", claim(code)).unwrap();
    }
    out.push_str("];\n");
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("groups.rs");
    std::fs::write(path, out).unwrap();
}
