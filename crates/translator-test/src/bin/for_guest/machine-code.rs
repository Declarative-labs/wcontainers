// Written once for both guests; each guest's module in `machine-code.rs` includes it.

pub(super) fn write(
    (program, optimisation, library, out): (&str, &str, &str, &str),
    defines: &[&str],
    (omit, progress): (Omit, ProgressCount),
) {
    let library = match library {
        "none" => Library::None,
        "c" => Library::C,
        other => panic!("no library {other}: none or c"),
    };
    let blob = build_with(program, optimisation, library, defines, Position::Fixed);
    let mut translation = translate_omitting::<Cpu>(
        &blob.bytes,
        0..blob.code_bytes,
        blob.link_address,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        progress,
        wcontainers_translator::AccessOrdering::Unordered,
        probe(),
        <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
        omit,
    )
    .expect("translation");
    // A test program is far below every engine limit, so its translation is one module.
    let translation = translation.modules.remove(0);
    std::fs::create_dir_all(out).unwrap();
    let base = Path::new(out).join(&blob.name);
    let module = base.with_added_extension(MODULE_EXTENSION);
    let compiled = base.with_added_extension(COMPILED_EXTENSION);
    std::fs::write(&module, &translation.wasm).unwrap();
    std::fs::write(&compiled, precompile(&translation.wasm)).unwrap();

    // The program's symbols, to name the pieces.
    let elf = std::fs::read(elf_path(&blob.name)).expect("the linked program");
    let elf = object::File::parse(&*elf).expect("an ELF file");
    for (i, offset) in translation.pieces.iter().enumerate() {
        let address = LOAD + offset;
        let names: Vec<&str> = elf
            .symbols()
            .filter(|symbol| symbol.address() == address)
            .filter_map(|symbol| symbol.name().ok())
            .filter(|name| !name.is_empty())
            .collect();
        let function = first_piece_function::<Cpu>() as usize + i;
        println!("function {function}: offset {offset:#x} {names:?}");
    }
    println!("wrote {} and {}", module.display(), compiled.display());
}
