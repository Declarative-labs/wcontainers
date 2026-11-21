//! Writes each iced `Code`'s claim from iced's own `instr_info` tables, which the shipped decoder does not
//! carry.

use iced_x86::Code;
use std::fmt::Write;

include!("src/claims.rs");
include!("src/lanes.rs");

/// The parts of iced's `Code` names the lane forms are found by: a VEX code's start, the separator of the
/// mnemonic and the operands, the start of an opcode that tells apart two encodings of one form, and the
/// operand names of the registers, memory and the two together.
const VEX_START: &str = "VEX_V";
const NAME_SEPARATOR: char = '_';
const OPCODE_MAP: &str = "0F";
const XMM: &str = "xmm";
const YMM: &str = "ymm";
const MEMORY: char = 'm';
const M256: &str = "m256";
const YMM_OR_M256: &str = "ymmm256";
/// Each 256-bit operand name and its 128-bit lane's.
const LANE_OPERANDS: &[(&str, &str)] = &[(YMM, XMM), (YMM_OR_M256, "xmmm128"), (M256, "m128")];
/// Cargo's line that rebuilds when the shared lane constants change.
const RERUN_LANES: &str = "cargo::rerun-if-changed=src/lanes.rs\n";
/// The generated lane table's text: its start, the end of its length, a pair's start, between the two, a
/// pair's end, and the table's end.
const LANE_TABLE: &str = "/// Each `Code`'s legacy form for a 128-bit lane (its number), and how its operands map.\npub static LANE_FORMS: [(u16, u8); ";
const LENGTH_END: &str = "] = [";
const PAIR_START: char = '(';
const PAIR_BETWEEN: char = ',';
const PAIR_END: &str = "),";
const TABLE_END: &str = "];\n";

/// A VEX operand's name as its 128-bit lane's in a legacy form.
fn lane_operand(token: &str) -> String {
    LANE_OPERANDS
        .iter()
        .find(|(wide, _)| *wide == token)
        .map_or(token, |(_, lane)| lane)
        .into()
}

/// Whether a legacy operand name takes the lane's operand: the same, or a register-or-memory form of it.
fn takes(legacy: &str, lane: &str) -> bool {
    legacy == lane
        || legacy
            .strip_prefix(XMM)
            .and_then(|rest| rest.strip_prefix(MEMORY))
            .is_some_and(|size| lane == XMM || lane.strip_prefix(MEMORY) == Some(size))
}

/// Each VEX `Code`'s legacy SSE form for one 128-bit lane, by the two codes' names, and how its operands map
/// (`lanes.rs`). A 256-bit form is taken only when its destination is a ymm register or 32 bytes of memory.
fn lane_forms() -> Vec<(Code, u8)> {
    let mut legacy: std::collections::HashMap<String, Vec<(Vec<String>, Code)>> =
        Default::default();
    for code in Code::values() {
        let name = format!("{code:?}");
        let mut tokens = name.split(NAME_SEPARATOR).map(String::from);
        let mnemonic = tokens.next().unwrap();
        legacy
            .entry(mnemonic)
            .or_default()
            .push((tokens.collect(), code));
    }
    Code::values()
        .map(|code| {
            let name = format!("{code:?}");
            let Some(rest) = name.strip_prefix(VEX_START) else {
                return (Code::INVALID, LANE_NONE);
            };
            // A name that ends in its opcode tells apart two encodings of one form.
            let mut tokens: Vec<&str> = rest
                .split(NAME_SEPARATOR)
                .filter(|t| !t.starts_with(OPCODE_MAP))
                .collect();
            let mut mnemonic = tokens.remove(0).to_string();
            mnemonic[..1].make_ascii_uppercase();
            let wide = tokens.iter().any(|t| t.starts_with(YMM) || *t == M256);
            if wide && ![YMM, YMM_OR_M256, M256].contains(&tokens[0]) {
                return (Code::INVALID, LANE_NONE);
            }
            let lane: Vec<String> = tokens.iter().map(|t| lane_operand(t)).collect();
            let without_first_source: Vec<String> = lane
                .iter()
                .enumerate()
                .filter(|(n, _)| *n != 1)
                .map(|(_, t)| t.clone())
                .collect();
            let mut tries = vec![(lane.clone(), LANE_IN_ORDER)];
            if lane.len() > 2 {
                tries.push((without_first_source.clone(), LANE_FROM_FIRST_SOURCE));
                if lane.last().is_some_and(|t| t == XMM) {
                    let mut masked = without_first_source.clone();
                    masked.pop();
                    tries.push((masked, LANE_FROM_FIRST_SOURCE_MASK_LAST));
                }
            }
            let forms = legacy.get(&mnemonic).map(Vec::as_slice).unwrap_or_default();
            tries
                .iter()
                .find_map(|(want, kind)| {
                    forms
                        .iter()
                        .find(|(have, _)| {
                            have.len() == want.len()
                                && have.iter().zip(want).all(|(h, w)| takes(h, w))
                        })
                        .map(|&(_, form)| (form, *kind))
                })
                .unwrap_or((Code::INVALID, LANE_NONE))
        })
        .collect()
}

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
    std::io::Write::write_all(&mut std::io::stdout(), RERUN_LANES.as_bytes()).unwrap();
    let mut out =
        String::from("/// Each `Code`'s claim group, by its number.\npub static GROUPS: [u8; ");
    write!(out, "{}] = [", Code::values().len()).unwrap();
    for code in Code::values() {
        write!(out, "{},", claim(code)).unwrap();
    }
    out.push_str("];\n");
    out.push_str(LANE_TABLE);
    out.push_str(&Code::values().len().to_string());
    out.push_str(LENGTH_END);
    for (form, kind) in lane_forms() {
        out.push(PAIR_START);
        out.push_str(&(form as u16).to_string());
        out.push(PAIR_BETWEEN);
        out.push_str(&kind.to_string());
        out.push_str(PAIR_END);
    }
    out.push_str(TABLE_END);
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("groups.rs");
    std::fs::write(path, out).unwrap();
}
