//! Writes each iced `Code`'s claim group from iced's own `instr_info` tables, which the shipped decoder does
//! not carry.

use iced_x86::Code;
use std::fmt::Write;

include!("src/claims.rs");

/// The group of one `Code`: kernel only when the CPU allows it only at CPL 0 (and `rdpmc`, which Linux leaves
/// to the kernel by keeping CR4.PCE clear), else by its features against the claim lists.
fn group(code: Code) -> u8 {
    if code.is_privileged() || code == Code::Rdpmc {
        return GROUP_KERNEL;
    }
    let names: Vec<String> = code
        .cpuid_features()
        .iter()
        .map(|f| format!("{f:?}"))
        .collect();
    let in_list = |list: &[&str], name: &String| list.iter().any(|n| n == name);
    if names.iter().all(|n| in_list(DEFAULT_FEATURES, n)) {
        GROUP_DEFAULT
    } else if names
        .iter()
        .all(|n| in_list(DEFAULT_FEATURES, n) || in_list(AVX_FEATURES, n))
    {
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
