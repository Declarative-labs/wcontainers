//! Writes what the engines run for a test program at full look-ahead, for reading the machine code: the
//! code module (`.wasm`) and wasmtime's compiled form of it with the runner's settings (`.cwasm`, an ELF file
//! for `objdump -d`). Prints each piece's function index in the module, blob offset and the program's symbol
//! there. Written once over the guest, named by its `GuestTest::NAME`.
//!
//! Usage: `machine-code <guest> <program> <optimisation> <library> <out directory> [define ...]`, the library
//! `none` or `c`, such as `machine-code x86_64 bench -O2 none out SCALE=1 WORKLOAD=2`. `WC_OMIT` (a list of
//! stop-check, permission-check, return-check, depth-check) leaves those checks out, as the benchmarks do;
//! `WC_PROGRESS_COUNT=kept` keeps the progress count.
//! `machine-code <module.wasm> <out.cwasm>` compiles a module made by hand from one of those, to compare
//! another emission's machine code.

use object::{Object, ObjectSymbol};
use std::path::Path;
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::first_piece_function;
use wcontainers_translator::{LookAhead, Omit, Optimise, ProgressCount, translate_omitting};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{LOAD, Library, Position, build_with, elf_path};
use wcontainers_translator_test::runner::{precompile, probe};
use wcontainers_x86::X86_64;

/// The extension of a code module.
const MODULE_EXTENSION: &str = "wasm";
/// The extension of wasmtime's compiled form of a module.
const COMPILED_EXTENSION: &str = "cwasm";
/// The variable listing the checks to leave out, by the names below.
const OMIT_VARIABLE: &str = "WC_OMIT";
const OMIT_STOP: &str = "stop-check";
const OMIT_PERMISSIONS: &str = "permission-check";
const OMIT_RETURNS: &str = "return-check";
const OMIT_DEPTH: &str = "depth-check";
/// The variable that keeps the progress count when it is [`PROGRESS_KEPT`].
const PROGRESS_VARIABLE: &str = "WC_PROGRESS_COUNT";
const PROGRESS_KEPT: &str = "kept";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [module, out] = arguments.as_slice()
        && Path::new(module).extension() == Some(MODULE_EXTENSION.as_ref())
    {
        let wasm = std::fs::read(module).expect("the module");
        std::fs::write(out, precompile(&wasm)).unwrap();
        return;
    }
    let [guest, program, optimisation, library, out, defines @ ..] = arguments.as_slice() else {
        panic!(
            "usage: machine-code <guest> <program> <optimisation> <library> <out directory> [define ...]"
        );
    };
    let library = match library.as_str() {
        "none" => Library::None,
        "c" => Library::C,
        other => panic!("no library {other}: none or c"),
    };
    let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
    let omitted = std::env::var(OMIT_VARIABLE).unwrap_or_default();
    let omit = Omit {
        stop: omitted.contains(OMIT_STOP),
        permissions: omitted.contains(OMIT_PERMISSIONS),
        returns: omitted.contains(OMIT_RETURNS),
        depth: omitted.contains(OMIT_DEPTH),
    };
    let progress = match std::env::var(PROGRESS_VARIABLE) {
        Ok(kept) if kept == PROGRESS_KEPT => ProgressCount::Kept,
        _ => ProgressCount::Absent,
    };
    let build = (
        program.as_str(),
        optimisation.as_str(),
        library,
        out.as_str(),
    );
    match guest.as_str() {
        Arm64::NAME => write::<Arm64>(build, &defines, (omit, progress)),
        X86_64::NAME => write::<X86_64>(build, &defines, (omit, progress)),
        other => panic!("no guest {other}: {} or {}", Arm64::NAME, X86_64::NAME),
    }
}

fn write<G: GuestTest>(
    (program, optimisation, library, out): (&str, &str, Library, &str),
    defines: &[&str],
    (omit, progress): (Omit, ProgressCount),
) {
    let blob = build_with::<G>(program, optimisation, library, defines, Position::Fixed);
    let mut translation = translate_omitting::<G>(
        &blob.bytes,
        0..blob.code_bytes,
        blob.link_address,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        progress,
        probe::<G>(),
        G::Offer::default(),
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
        let function = first_piece_function::<G>() as usize + i;
        println!("function {function}: offset {offset:#x} {names:?}");
    }
    println!("wrote {} and {}", module.display(), compiled.display());
}
