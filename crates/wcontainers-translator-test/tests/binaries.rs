//! Real Linux binaries of the guest's CPU translated at scale: the code and start addresses of each, what the translator
//! makes of them, and whether the engines take it. Each test is written once over the guest and run for each.

use std::io::Write;
use std::time::Duration;
use std::time::Instant;
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Means, Optimise, Translation};
use wcontainers_translator_test::binaries::{Binary, sources};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::boundaries;
use wcontainers_translator_test::runner::{Machine, Syscalls, probe};

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

/// A binary translated at full look-ahead from every start its file offers.
fn translated<G: GuestTest>(binary: &Binary, optimise: Optimise) -> Translation {
    wcontainers_translator::translate::<G>(
        &binary.image,
        binary.code.clone(),
        &binary.starts.all(),
        LookAhead::Full,
        optimise,
        probe::<G>(),
        G::Offer::default(),
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
            let translation = translated::<G>(&binary, optimise);
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

/// An unwind table entry that cannot be read (here a function's entry pointing to no CIE) loses that entry
/// alone, counted and warned; the entries after it are still read.
fn a_bad_unwind_entry_loses_only_itself<G: GuestTest>() {
    use gimli::UnwindSection;
    use object::{Object, ObjectSection};
    let source = sources::<G>()
        .into_iter()
        .find(|s| s.name == "musl")
        .unwrap();
    let bytes = std::fs::read(source.file::<G>()).unwrap();
    let whole = Binary::read::<G>(source.name, &bytes);
    let file = object::File::parse(&*bytes).unwrap();
    let section = file.section_by_name(".eh_frame").unwrap();
    let (section_at, _) = section.file_range().unwrap();
    let data = section.data().unwrap();
    let bases = gimli::BaseAddresses::default().set_eh_frame(section.address());
    let mut entries = gimli::EhFrame::new(data, gimli::LittleEndian).entries(&bases);
    let mut functions = Vec::new();
    while let Some(entry) = entries.next().unwrap() {
        if let gimli::CieOrFde::Fde(partial) = entry {
            functions.push(partial.offset());
        }
    }
    assert!(
        functions.len() > 10,
        "musl's unwind tables describe {} functions",
        functions.len()
    );
    // The CIE pointer follows the entry's 4 byte length.
    let at = section_at as usize + functions[functions.len() / 2] + 4;
    let mut broken = bytes.clone();
    broken[at..at + 4].copy_from_slice(&0x7fff_fff0u32.to_le_bytes());
    let binary = Binary::read::<G>("musl with a bad CIE pointer", &broken);
    assert_eq!(binary.unwind_errors, 1);
    assert!(binary.starts.unwind.is_subset(&whole.starts.unwind));
    assert!(
        binary.starts.unwind.len() + 1 >= whole.starts.unwind.len(),
        "{} of {} unwind starts read",
        binary.starts.unwind.len(),
        whole.starts.unwind.len()
    );
}

/// Not a check: each binary's code and starts, its translation at full look-ahead, and what wasmtime makes
/// of it. One command:
/// `WC_BINARIES=busybox,musl nix develop -c cargo test --release -p wcontainers-translator-test --test binaries measure_binaries_arm64 -- --ignored --nocapture`
/// (all nine when `WC_BINARIES` is not set; each is taken from its Docker image once). `WC_BINARIES_DUMP`
/// names a directory to write each module to, for looking into; `WC_BINARIES_ENGINES=0` leaves the engines
/// out; `WC_OPTIMISE` (`speed` or `size`) translates for one setting only, not both.
fn measure_binaries<G: GuestTest>() {
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
        for optimise in settings() {
            let name = format!("{} for {optimise:?}", binary.name);
            let started = Instant::now();
            let translation = translated::<G>(&binary, optimise);
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
    measure_binaries::<Arm64>();
}
