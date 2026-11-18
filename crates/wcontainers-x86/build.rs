//! Writes each iced `Code`'s claim group from iced's own `instr_info` tables, which the shipped decoder does
//! not carry.

use iced_x86::Code;
use std::fmt::Write;

include!("src/claims.rs");

/// The group of one `Code`, by its features against the claim lists: kernel only when the CPU allows it only
/// at CPL 0 (and `rdpmc`, which Linux leaves to the kernel by keeping CR4.PCE clear) and the CPU presented
/// has it by default; an instruction of a feature not presented is one a CPU without it does not have,
/// kernel only or not.
fn group(code: Code) -> u8 {
    let kernel = code.is_privileged() || code == Code::Rdpmc;
    let names: Vec<String> = code
        .cpuid_features()
        .iter()
        .map(|f| format!("{f:?}"))
        .collect();
    let all_in = |lists: &[&[&str]]| {
        names
            .iter()
            .all(|n| lists.iter().any(|list| list.contains(&n.as_str())))
    };
    if all_in(&[DEFAULT_FEATURES, KERNEL_FEATURES]) {
        if kernel { GROUP_KERNEL } else { GROUP_DEFAULT }
    } else if all_in(&[DEFAULT_FEATURES, AVX_FEATURES]) {
        GROUP_AVX
    } else {
        GROUP_NEVER
    }
}

fn main() {
    println!("cargo::rerun-if-changed=src/claims.rs");
    let mut out =
        String::from("/// Each `Code`'s claim group, by its number.\npub static GROUPS: [u8; ");
    write!(out, "{}] = [", Code::values().len()).unwrap();
    for code in Code::values() {
        write!(out, "{},", group(code)).unwrap();
    }
    out.push_str("];\n");
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("groups.rs");
    std::fs::write(path, out).unwrap();
}
