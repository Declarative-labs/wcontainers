//! Real Linux binaries of the guest's CPU translated at scale: the code and start addresses of each, what the translator
//! makes of them, and whether the engines take it. Each test is written once over the guest and run for each.

use std::io::Write;
use std::time::Duration;
use std::time::Instant;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{BR_TABLE_TARGETS, LookAhead, Means, Optimise, Translation};
use wcontainers_x86::X86_64;

/// Where the binaries' images are placed in guest memory.
const LOAD: u64 = 0x10_0000;

/// The settings named in `WC_OPTIMISE` (`speed` or `size`), or both.
fn settings() -> Vec<Optimise> {
    match std::env::var("WC_OPTIMISE").as_deref() {
        Ok("speed") => vec![Optimise::Speed],
        Ok("size") => vec![Optimise::Size],
        _ => vec![Optimise::Speed, Optimise::Size],
    }
}

fn gzipped(bytes: &[u8]) -> usize {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap().len()
}

fn validate(wasm: &[u8]) {
    use wasmparser::{Validator, WasmFeatures};
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::RELAXED_SIMD
        | WasmFeatures::GC;
    Validator::new_with_features(features)
        .validate_all(wasm)
        .expect("a valid module");
}

/// The unwind tables' function entries of a file, by their offsets in the section.
fn unwind_functions(bytes: &[u8]) -> Vec<usize> {
    use gimli::UnwindSection;
    use object::{Object, ObjectSection};
    let file = object::File::parse(bytes).unwrap();
    let Some(section) = file.section_by_name(".eh_frame") else {
        return Vec::new();
    };
    let data = section.data().unwrap();
    let bases = gimli::BaseAddresses::default().set_eh_frame(section.address());
    let mut entries = gimli::EhFrame::new(data, gimli::LittleEndian).entries(&bases);
    let mut functions = Vec::new();
    while let Some(entry) = entries.next().unwrap() {
        if let gimli::CieOrFde::Fde(partial) = entry {
            functions.push(partial.offset());
        }
    }
    functions
}

/// How many functions a binary's unwind tables must describe for the test of a bad entry to break one among
/// them.
const UNWIND_FUNCTIONS_NEEDED: usize = 10;

/// x86-64's switch tables in a binary, found by decoding its code from its first byte on: jumps through a
/// register; of those, the tables `X86_64::jump_table` recognises (each target's offset from the table, 4 bytes,
/// as position-independent code has them) and of these the ones whose targets span more `br_table` targets than
/// an engine takes; and jumps through a table of 8-byte absolute entries (`jmp [table + index * 8]`, or such a
/// load into the register the next instruction jumps through), which the translator leaves to the lookup.
fn x86_64_switch_tables(binary: &x86_64::Binary) {
    use iced_x86::{Code, OpKind, Register};
    use wcontainers_guest_cpu::Guest;
    let code = &binary.image[binary.code.start as usize..binary.code.end as usize];
    let starts: Vec<u64> = x86_64::boundaries(code)
        .into_iter()
        .map(|o| o + binary.code.start)
        .collect();
    let decode = |at: u64| wcontainers_x86::decode(&binary.image[at as usize..]).map(|(i, _)| i);
    let before = |o: u64| {
        let k = starts.binary_search(&o).ok()?.checked_sub(1)?;
        Some((starts[k], decode(starts[k])?))
    };
    // An operand read from a table of 8-byte words by a scaled index.
    let from_word_table = |i: &iced_x86::Instruction, n: u32| {
        i.op_kind(n) == OpKind::Memory
            && i.memory_index() != Register::None
            && u64::from(i.memory_index_scale()) == size_of::<u64>() as u64
    };
    let (mut through_register, mut recognised, mut over_limit, mut absolute) =
        (0u64, 0u64, 0u64, 0u64);
    for &at in &starts {
        let Some(i) = decode(at) else { continue };
        let ins = &i.iced;
        match ins.code() {
            Code::Jmp_rm64 if ins.op_kind(0) == OpKind::Register => {
                through_register += 1;
                let register = ins.op_register(0);
                let reg = register.number() as u8;
                let loaded = before(at).is_some_and(|(_, p)| {
                    p.iced.code() == Code::Mov_r64_rm64
                        && p.iced.op_register(0) == register
                        && from_word_table(&p.iced, 1)
                });
                if loaded {
                    absolute += 1;
                }
                if let Some(table) =
                    X86_64::jump_table(at, reg, &binary.image, binary.code.clone(), before)
                {
                    recognised += 1;
                    let (lo, hi) = (table.targets.iter().min(), table.targets.iter().max());
                    if let (Some(lo), Some(hi)) = (lo, hi)
                        && (hi - lo) / X86_64::ALIGN + 1 > BR_TABLE_TARGETS
                    {
                        over_limit += 1;
                    }
                }
            }
            Code::Jmp_rm64 if from_word_table(ins, 0) => absolute += 1,
            _ => {}
        }
    }
    println!(
        "{}: switch tables found decoding from the first byte: {through_register} jumps through a register, {recognised} tables recognised, {over_limit} of them over the limit of {BR_TABLE_TARGETS} targets; {absolute} jumps through a table of absolute entries",
        binary.name
    );
}

#[test]
fn small_binaries_translate_and_load_arm64() {
    arm64::small_binaries_translate_and_load();
}

#[test]
fn a_bad_unwind_entry_loses_only_itself_arm64() {
    arm64::a_bad_unwind_entry_loses_only_itself();
}

#[test]
#[ignore]
fn measure_binaries_arm64() {
    arm64::measure_binaries(&[("", ())], |_| {});
}

#[test]
fn small_binaries_translate_and_load_x86_64() {
    x86_64::small_binaries_translate_and_load();
}

#[test]
fn a_bad_unwind_entry_loses_only_itself_x86_64() {
    x86_64::a_bad_unwind_entry_loses_only_itself();
}

/// Each binary for programs told the default list and told the x86-64-v3 list, with its switch tables.
#[test]
#[ignore]
fn measure_binaries_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    x86_64::measure_binaries(
        &[("", wcontainers_x86::Offer::default()), (", told v3", v3)],
        x86_64_switch_tables,
    );
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
mod arm64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    use wcontainers_translator_test::arm64 as harness;
    use wcontainers_translator_test::arm64::Cpu;
    pub(super) use wcontainers_translator_test::arm64::binaries::{Binary, sources};
    pub(super) use wcontainers_translator_test::arm64::programs::boundaries;
    pub(super) use wcontainers_translator_test::arm64::runner::{Machine, Syscalls, probe};

    include!("for_guest/binaries.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    use wcontainers_translator_test::x86_64 as harness;
    use wcontainers_translator_test::x86_64::Cpu;
    pub(super) use wcontainers_translator_test::x86_64::binaries::{Binary, sources};
    pub(super) use wcontainers_translator_test::x86_64::programs::boundaries;
    pub(super) use wcontainers_translator_test::x86_64::runner::{Machine, Syscalls, probe};

    include!("for_guest/binaries.rs");
}
