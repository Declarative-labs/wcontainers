// Written once for both guests; each guest's module in `binaries.rs` includes it.

/// The binaries named in `WC_BINARIES` (comma separated), or all of them.
pub(super) fn chosen() -> Vec<harness::binaries::Source> {
    let names = std::env::var("WC_BINARIES").unwrap_or_default();
    sources()
        .into_iter()
        .filter(|s| names.is_empty() || names.split(',').any(|n| n == s.name))
        .collect()
}
/// A binary translated at full look-ahead from every start its file offers, for a program told `offer`.
pub(super) fn translated(
    binary: &Binary,
    optimise: Optimise,
    offer: <Cpu as wcontainers_guest_cpu::Guest>::Offer,
) -> Translation {
    wcontainers_translator::translate::<Cpu>(
        &binary.image,
        binary.code.clone(),
        binary.link_address,
        &binary.starts.all(),
        LookAhead::Full,
        optimise,
        wcontainers_translator::ProgressCount::Absent,
        wcontainers_translator::AccessOrdering::Unordered,
        probe(),
        offer,
    )
    .expect("translate")
}
/// What wasmtime makes of a translation: compile time, instantiation time and bytes of machine code.
pub(super) fn load_in_wasmtime(
    binary: &Binary,
    translation: Translation,
) -> (Duration, Duration, usize) {
    let memory = (LOAD + binary.image.len() as u64).next_multiple_of(1 << 16);
    // The lookup table covers the code alone.
    let slots = ((binary.code.end - binary.code.start) / Cpu::ALIGN) as u32;
    let table_base = LOAD + binary.code.start;
    let mut machine = Machine::new(memory, table_base, slots, Syscalls::StandIn);
    machine.load_translation(&binary.image, binary.code.clone(), LOAD, translation)
}
/// Chrome makes the modules of a translation and their instances, each function compiled at its first call or
/// all at once (`flags`).
pub(super) fn load_in_chrome(
    name: &str,
    binary: &Binary,
    translation: &Translation,
    flags: &str,
) -> Result<harness::browser::Loaded, String> {
    let wasm: Vec<&[u8]> = translation.modules.iter().map(|m| &m.wasm[..]).collect();
    let code = binary.code.clone();
    harness::browser::load_modules(name, &binary.image, LOAD, code, &wasm, flags)
}
/// busybox and the C library, whole: in both settings every module is valid and loads in wasmtime and in
/// Chrome, and every piece is counted once by the means that found it.
pub(super) fn small_binaries_translate_and_load() {
    for source in sources()
        .iter()
        .filter(|s| matches!(s.name, "busybox" | "musl"))
    {
        let binary = Binary::read(source.name, &std::fs::read(source.file()).unwrap());
        assert_eq!(binary.unwind_errors, 0, "{}", binary.name);
        let starts = binary.starts.all();
        for optimise in [Optimise::Speed, Optimise::Size] {
            let name = format!("{} for {optimise:?}", binary.name);
            let translation = translated(
                &binary,
                optimise,
                <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
            );
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
            let loaded = load_in_chrome(&name, &binary, &translation, "");
            assert!(loaded.is_ok(), "{name}: Chrome refused: {:?}", loaded.err());
            let (compiled, _, machine_code) = load_in_wasmtime(&binary, translation);
            eprintln!(
                "{name}: {pieces} pieces, wasmtime compiled it in {:.2} s to {machine_code} bytes",
                compiled.as_secs_f64()
            );
        }
    }
}
/// An unwind table entry that cannot be read (here a function's entry pointing to no CIE) loses that entry
/// alone, counted and warned; the entries after it are still read. The binary is the first of the list whose
/// unwind tables describe enough functions (x86-64's musl describes only its assembly's few).
pub(super) fn a_bad_unwind_entry_loses_only_itself() {
    use object::{Object, ObjectSection};
    let (source, bytes, functions) = sources()
        .into_iter()
        .map(|source| {
            let bytes = std::fs::read(source.file()).unwrap();
            let functions = unwind_functions(&bytes);
            (source, bytes, functions)
        })
        .find(|(_, _, functions)| functions.len() > UNWIND_FUNCTIONS_NEEDED)
        .expect("a binary whose unwind tables describe enough functions");
    let whole = Binary::read(source.name, &bytes);
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
    let binary = Binary::read(&format!("{} with a bad CIE pointer", source.name), &broken);
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
pub(super) fn measure_binaries(
    offers: &[(&str, <Cpu as wcontainers_guest_cpu::Guest>::Offer)],
    more: impl Fn(&Binary),
) {
    for source in chosen() {
        let file = std::fs::read(source.file()).unwrap();
        let started = Instant::now();
        let binary = Binary::read(source.name, &file);
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
            boundaries(code).len(),
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
            let translation = translated(&binary, optimise, offer);
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
            let (array, touched) = lookup_array(&binary, &translation);
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
                match load_in_chrome(&name, &binary, &translation, flags) {
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
            let (compiled, instantiated, machine_code) = load_in_wasmtime(&binary, translation);
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
pub(super) fn lookup_array(binary: &Binary, translation: &Translation) -> (u64, u64) {
    let slot = size_of::<u32>() as u64;
    let array = (binary.code.end - binary.code.start) / Cpu::ALIGN * slot;
    let pages: std::collections::BTreeSet<u64> = translation
        .pieces()
        .map(|p| (p - binary.code.start) / Cpu::ALIGN * slot / wcontainers_abi::PAGE_SIZE)
        .collect();
    (array, pages.len() as u64 * wcontainers_abi::PAGE_SIZE)
}
