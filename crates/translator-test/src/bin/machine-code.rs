//! Writes what the engines run for a test program at full look-ahead, for reading the machine code: the
//! code module (`.wasm`) and wasmtime's compiled form of it with the runner's settings (`.cwasm`, an ELF file
//! for `objdump -d`). Prints each piece's function index in the module, blob offset and the program's symbol
//! there. Written once for both guests, each named by its `part::NAME`.
//!
//! Usage: `machine-code <guest> <program> <optimisation> <library> <out directory> [define ...]`, the library
//! `none` or `c`, such as `machine-code x86_64 bench -O2 none out SCALE=1 WORKLOAD=2`. `WC_OMIT` (a list of
//! stop-check, permission-check, return-check, depth-check) leaves those checks out, as the benchmarks do;
//! `WC_PROGRESS_COUNT=kept` keeps the progress count.
//! `machine-code <module.wasm> <out.cwasm>` compiles a module made by hand from one of those, to compare
//! another emission's machine code.

use object::{Object, ObjectSymbol};
use std::path::Path;
use wcontainers_translator::contract::first_piece_function;
use wcontainers_translator::{LookAhead, Omit, Optimise, ProgressCount, translate_omitting};

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
        std::fs::write(out, arm64::precompile(&wasm)).unwrap();
        return;
    }
    let [guest, program, optimisation, library, out, defines @ ..] = arguments.as_slice() else {
        panic!(
            "usage: machine-code <guest> <program> <optimisation> <library> <out directory> [define ...]"
        );
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
        library.as_str(),
        out.as_str(),
    );
    match guest.as_str() {
        wcontainers_translator_test::arm64::part::NAME => {
            arm64::write(build, &defines, (omit, progress))
        }
        wcontainers_translator_test::x86_64::part::NAME => {
            x86_64::write(build, &defines, (omit, progress))
        }
        other => panic!(
            "no guest {other}: {} or {}",
            wcontainers_translator_test::arm64::part::NAME,
            wcontainers_translator_test::x86_64::part::NAME
        ),
    }
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
mod arm64 {
    use super::*;
    use wcontainers_translator_test::arm64::Cpu;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        LOAD, Library, Position, build_with, elf_path,
    };
    pub(super) use wcontainers_translator_test::arm64::runner::{precompile, probe};

    include!("for_guest/machine-code.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_translator_test::x86_64::Cpu;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        LOAD, Library, Position, build_with, elf_path,
    };
    pub(super) use wcontainers_translator_test::x86_64::runner::{precompile, probe};

    include!("for_guest/machine-code.rs");
}
