//! Real Linux binaries of the guest's CPU translated at scale: the code and start addresses of each, what the translator
//! makes of them, and whether the engines take it. Each test is written once over the guest and run for each.

use std::io::Write;
use std::time::Duration;
use std::time::Instant;
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{BR_TABLE_TARGETS, LookAhead, Means, Optimise, Translation};
use wcontainers_translator_test::binaries::{Binary, sources};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::boundaries;
use wcontainers_translator_test::runner::{Machine, Syscalls, probe};
use wcontainers_x86::X86_64;

/// Where the binaries' images are placed in guest memory.
const LOAD: u64 = 0x10_0000;

/// The binaries named in `WC_BINARIES` (comma separated), or all of them.
fn chosen<G: GuestTest>() -> Vec<wcontainers_translator_test::binaries::Source> {
    let names = std::env::var("WC_BINARIES").unwrap_or_default();
    sources::<G>()
        .into_iter()
        .filter(|s| names.is_empty() || names.split(',').any(|n| n == s.name))
        .collect()
}

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

/// A binary translated at full look-ahead from every start its file offers, for a program told `offer`.
fn translated<G: GuestTest>(binary: &Binary, optimise: Optimise, offer: G::Offer) -> Translation {
    wcontainers_translator::translate::<G>(
        &binary.image,
        binary.code.clone(),
        binary.link_address,
        &binary.starts.all(),
        LookAhead::Full,
        optimise,
        wcontainers_translator::ProgressCount::Absent,
        probe::<G>(),
        offer,
    )
    .expect("translate")
}

/// What wasmtime makes of a translation: compile time, instantiation time and bytes of machine code.
fn load_in_wasmtime<G: GuestTest>(
    binary: &Binary,
    translation: Translation,
) -> (Duration, Duration, usize) {
    let memory = (LOAD + binary.image.len() as u64).next_multiple_of(1 << 16);
    // The lookup table covers the code alone.
    let slots = ((binary.code.end - binary.code.start) / G::ALIGN) as u32;
    let table_base = LOAD + binary.code.start;
    let mut machine = Machine::<G>::new(memory, table_base, slots, Syscalls::StandIn);
    machine.load_translation(&binary.image, binary.code.clone(), LOAD, translation)
}

/// Chrome makes the modules of a translation and their instances, each function compiled at its first call or
/// all at once (`flags`).
fn load_in_chrome<G: GuestTest>(
    name: &str,
    binary: &Binary,
    translation: &Translation,
    flags: &str,
) -> Result<wcontainers_translator_test::browser::Loaded, String> {
    let wasm: Vec<&[u8]> = translation.modules.iter().map(|m| &m.wasm[..]).collect();
    let code = binary.code.clone();
    wcontainers_translator_test::browser::load_modules::<G>(
        name,
        &binary.image,
        LOAD,
        code,
        &wasm,
        flags,
    )
}

/// busybox and the C library, whole: in both settings every module is valid and loads in wasmtime and in
/// Chrome, and every piece is counted once by the means that found it.
fn small_binaries_translate_and_load<G: GuestTest>() {
    for source in sources::<G>()
        .iter()
        .filter(|s| matches!(s.name, "busybox" | "musl"))
    {
        let binary = Binary::read::<G>(source.name, &std::fs::read(source.file::<G>()).unwrap());
        assert_eq!(binary.unwind_errors, 0, "{}", binary.name);
        let starts = binary.starts.all();
        for optimise in [Optimise::Speed, Optimise::Size] {
            let name = format!("{} for {optimise:?}", binary.name);
            let translation = translated::<G>(&binary, optimise, G::Offer::default());
            let pieces = translation.pieces().count() as u64;
            assert_eq!(translation.found.iter().sum::<u64>(), pieces, "{name}");
            assert_eq!(
                translation.found[Means::Given as usize],
                starts.len() as u64,
                "{name}"
            );
            for module in &translation.modules {
                validate(&module.wasm);
            }
            let loaded = load_in_chrome::<G>(&name, &binary, &translation, "");
            assert!(loaded.is_ok(), "{name}: Chrome refused: {:?}", loaded.err());
            let (compiled, _, machine_code) = load_in_wasmtime::<G>(&binary, translation);
            eprintln!(
                "{name}: {pieces} pieces, wasmtime compiled it in {:.2} s to {machine_code} bytes",
                compiled.as_secs_f64()
            );
        }
    }
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

/// An unwind table entry that cannot be read (here a function's entry pointing to no CIE) loses that entry
/// alone, counted and warned; the entries after it are still read. The binary is the first of the list whose
/// unwind tables describe enough functions (x86-64's musl describes only its assembly's few).
fn a_bad_unwind_entry_loses_only_itself<G: GuestTest>() {
    use object::{Object, ObjectSection};
    let (source, bytes, functions) = sources::<G>()
        .into_iter()
        .map(|source| {
            let bytes = std::fs::read(source.file::<G>()).unwrap();
            let functions = unwind_functions(&bytes);
            (source, bytes, functions)
        })
        .find(|(_, _, functions)| functions.len() > UNWIND_FUNCTIONS_NEEDED)
        .expect("a binary whose unwind tables describe enough functions");
    let whole = Binary::read::<G>(source.name, &bytes);
    let file = object::File::parse(&*bytes).unwrap();
    let (section_at, _) = file
        .section_by_name(".eh_frame")
        .unwrap()
        .file_range()
        .unwrap();
    // The CIE pointer follows the entry's 4 byte length.
    let at = section_at as usize + functions[functions.len() / 2] + 4;
    let mut broken = bytes.clone();
    broken[at..at + 4].copy_from_slice(&0x7fff_fff0u32.to_le_bytes());
    let binary = Binary::read::<G>(&format!("{} with a bad CIE pointer", source.name), &broken);
    assert_eq!(binary.unwind_errors, 1);
    assert!(binary.starts.unwind.is_subset(&whole.starts.unwind));
    assert!(
        binary.starts.unwind.len() + 1 >= whole.starts.unwind.len(),
        "{} of {} unwind starts read",
        binary.starts.unwind.len(),
        whole.starts.unwind.len()
    );
}

/// Not a check: each binary's code and starts, what `more` measures of it, its translation at full look-ahead for
/// each list of `offers`, the lookup array that covers it, and what the engines make of it. One command:
/// `WC_BINARIES=busybox,musl nix develop -c cargo test --release -p wcontainers-translator-test --test binaries measure_binaries_arm64 -- --ignored --nocapture`
/// (`measure_binaries_x86_64` for x86-64; all nine when `WC_BINARIES` is not set; each is taken from its Docker
/// image once). `WC_BINARIES_DUMP` names a directory to write each module to, for looking into;
/// `WC_BINARIES_ENGINES=0` leaves the engines out; `WC_OPTIMISE` (`speed` or `size`) translates for one setting
/// only, not both.
fn measure_binaries<G: GuestTest>(offers: &[(&str, G::Offer)], more: impl Fn(&Binary)) {
    for source in chosen::<G>() {
        let file = std::fs::read(source.file::<G>()).unwrap();
        let started = Instant::now();
        let binary = Binary::read::<G>(source.name, &file);
        let reading = started.elapsed().as_secs_f64();
        let starts = binary.starts.all();
        let code = &binary.image[binary.code.start as usize..binary.code.end as usize];
        let means: Vec<String> = binary
            .starts
            .by_means()
            .iter()
            .map(|(what, set)| format!("{what} {}", set.len()))
            .collect();
        println!(
            "{}: image {} bytes, code {} bytes ({} gzipped, {} instructions), read in {reading:.2} s; {} starts ({}); {} unwind table entries not read",
            binary.name,
            binary.image.len(),
            code.len(),
            gzipped(code),
            boundaries::<G>(code).len(),
            starts.len(),
            means.join(", "),
            binary.unwind_errors
        );
        more(&binary);
        for (optimise, &(told, offer)) in settings()
            .into_iter()
            .flat_map(|o| offers.iter().map(move |offer| (o, offer)))
        {
            let name = format!("{} for {optimise:?}{told}", binary.name);
            let started = Instant::now();
            let translation = translated::<G>(&binary, optimise, offer);
            let seconds = started.elapsed().as_secs_f64();
            let pieces = translation.pieces().count();
            let found: Vec<String> = Means::ALL
                .iter()
                .map(|&m| format!("{} {}", m.name(), translation.found[m as usize]))
                .collect();
            let bytes: usize = translation.modules.iter().map(|m| m.wasm.len()).sum();
            let compressed: usize = translation.modules.iter().map(|m| gzipped(&m.wasm)).sum();
            println!(
                "{}: translated in {seconds:.2} s ({:.2} MB of code a second); {} pieces ({}); {} modules, {bytes} bytes ({:.1} a byte of code), {compressed} gzipped ({:.1} a gzipped byte)",
                name,
                code.len() as f64 / seconds / 1e6,
                pieces,
                found.join(", "),
                translation.modules.len(),
                bytes as f64 / code.len() as f64,
                compressed as f64 / gzipped(code) as f64,
            );
            // Places in the code that may take each slow path.
            let sites: Vec<String> = Reason::ALL
                .iter()
                .map(|&r| {
                    (
                        r,
                        translation
                            .modules
                            .iter()
                            .map(|m| m.sites[r as usize])
                            .sum::<u64>(),
                    )
                })
                .filter(|&(_, n)| n > 0)
                .map(|(r, n)| format!("{} {n}", r.name()))
                .collect();
            println!(
                "{name}: places that may take a slow path: {}",
                sites.join(", ")
            );
            let (array, touched) = lookup_array::<G>(&binary, &translation);
            println!(
                "{name}: lookup array {array} bytes, {touched} of them in pages that hold a piece's slot"
            );
            for (i, module) in translation.modules.iter().enumerate() {
                validate(&module.wasm);
                if let Ok(directory) = std::env::var("WC_BINARIES_DUMP") {
                    let path = format!("{directory}/{}-{optimise:?}-{i}.wasm", binary.name);
                    std::fs::write(path, &module.wasm).unwrap();
                }
            }
            if std::env::var("WC_BINARIES_ENGINES").is_ok_and(|v| v == "0") {
                continue;
            }
            for (what, flags) in [
                ("compiled at first call", ""),
                ("compiled at once", "--no-wasm-lazy-compilation"),
            ] {
                // What Chrome's processes hold after making the modules and their instances, beyond what
                // they held before; compiled at once less compiled at first call is the compiled code.
                match load_in_chrome::<G>(&name, &binary, &translation, flags) {
                    Ok(loaded) => println!(
                        "{}: Chrome, {what}, made its modules in {:.2} s and their instances in {:.3} s; its processes grew by {:.1} MB",
                        name,
                        loaded.compiling / 1e3,
                        loaded.instantiating / 1e3,
                        loaded.memory as f64 / 1e6
                    ),
                    Err(error) => println!("{}: Chrome, {what}, refused: {error}", binary.name),
                }
            }
            let (compiled, instantiated, machine_code) =
                load_in_wasmtime::<G>(&binary, translation);
            println!(
                "{}: wasmtime compiled it in {:.2} s to {machine_code} bytes of machine code, instantiated it in {:.3} s",
                name,
                compiled.as_secs_f64(),
                instantiated.as_secs_f64(),
            );
        }
    }
}

/// Bytes of the lookup array over a binary's code (one 4-byte slot per `ALIGN` bytes), and of those the bytes in
/// pages of the array that hold at least one piece's slot: what it costs where untouched pages cost nothing.
fn lookup_array<G: GuestTest>(binary: &Binary, translation: &Translation) -> (u64, u64) {
    let slot = size_of::<u32>() as u64;
    let array = (binary.code.end - binary.code.start) / G::ALIGN * slot;
    let pages: std::collections::BTreeSet<u64> = translation
        .pieces()
        .map(|p| (p - binary.code.start) / G::ALIGN * slot / wcontainers_abi::PAGE_SIZE)
        .collect();
    (array, pages.len() as u64 * wcontainers_abi::PAGE_SIZE)
}

/// x86-64's switch tables in a binary, found by decoding its code from its first byte on: jumps through a
/// register; of those, the tables `X86_64::jump_table` recognises (each target's offset from the table, 4 bytes,
/// as position-independent code has them) and of these the ones whose targets span more `br_table` targets than
/// an engine takes; and jumps through a table of 8-byte absolute entries (`jmp [table + index * 8]`, or such a
/// load into the register the next instruction jumps through), which the translator leaves to the lookup.
fn x86_64_switch_tables(binary: &Binary) {
    use iced_x86::{Code, OpKind, Register};
    use wcontainers_guest_cpu::Guest;
    let code = &binary.image[binary.code.start as usize..binary.code.end as usize];
    let starts: Vec<u64> = boundaries::<X86_64>(code)
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
    small_binaries_translate_and_load::<Arm64>();
}

#[test]
fn a_bad_unwind_entry_loses_only_itself_arm64() {
    a_bad_unwind_entry_loses_only_itself::<Arm64>();
}

#[test]
#[ignore]
fn measure_binaries_arm64() {
    measure_binaries::<Arm64>(&[("", wcontainers_arm64::Offer::default())], |_| {});
}

#[test]
fn small_binaries_translate_and_load_x86_64() {
    small_binaries_translate_and_load::<X86_64>();
}

#[test]
fn a_bad_unwind_entry_loses_only_itself_x86_64() {
    a_bad_unwind_entry_loses_only_itself::<X86_64>();
}

/// Each binary for programs told the default list and told the x86-64-v3 list, with its switch tables.
#[test]
#[ignore]
fn measure_binaries_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    measure_binaries::<X86_64>(
        &[("", wcontainers_x86::Offer::default()), (", told v3", v3)],
        x86_64_switch_tables,
    );
}
