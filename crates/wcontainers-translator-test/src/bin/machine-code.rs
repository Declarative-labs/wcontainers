//! Writes what the engines run for a test program at full look-ahead, for reading the machine code: the
//! code module (`.wasm`) and wasmtime's compiled form of it with the runner's settings (`.cwasm`, an ELF file
//! for `objdump -d`). Prints each piece's function index (counting the six imported hooks and the
//! dispatcher and the unwinder before the pieces), blob offset and the program's symbol there.
//!
//! Usage: `machine-code <program> <optimisation> <out directory> [define ...]`, such as
//! `machine-code bench -O2 out SCALE=1 WORKLOAD=2`. `WC_OMIT` (a list of stop, permissions, returns) leaves
//! checks out, as the benchmarks do.

use std::process::Command;
use wcontainers_translator::{LookAhead, Omit, Optimise, translate_omitting};
use wcontainers_translator_test::programs::{Library, build_with};
use wcontainers_translator_test::runner::precompile;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [program, optimisation, out, defines @ ..] = arguments.as_slice() else {
        panic!("usage: machine-code <program> <optimisation> <out directory> [define ...]");
    };
    let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
    let omitted = std::env::var("WC_OMIT").unwrap_or_default();
    let omit = Omit {
        stop: omitted.contains("stop"),
        permissions: omitted.contains("permissions"),
        returns: omitted.contains("returns"),
        depth: omitted.contains("depth"),
    };
    let blob = build_with(program, optimisation, Library::None, &defines);
    let mut translation = translate_omitting::<wcontainers_arm64::Arm64>(
        &blob.bytes,
        0..blob.code_bytes,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        wcontainers_translator::ProgressCount::Absent,
        wcontainers_translator_test::runner::probe(),
        (),
        omit,
    )
    .expect("translation");
    // A test program is far below every engine limit, so its translation is one module.
    let translation = translation.modules.remove(0);
    std::fs::create_dir_all(out).unwrap();
    let base = format!("{out}/{}", blob.name);
    std::fs::write(format!("{base}.wasm"), &translation.wasm).unwrap();
    std::fs::write(format!("{base}.cwasm"), precompile(&translation.wasm)).unwrap();

    // The program's symbols, to name the pieces.
    let elf = format!(
        "{}/../../target/programs/{}.elf",
        env!("CARGO_MANIFEST_DIR"),
        blob.name
    );
    let symbols = Command::new("nm").arg(&elf).output().expect("nm");
    let symbols: Vec<(u64, String)> = String::from_utf8_lossy(&symbols.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let address = u64::from_str_radix(parts.next()?, 16).ok()?;
            Some((address, parts.nth(1)?.to_string()))
        })
        .collect();
    for (i, offset) in translation.pieces.iter().enumerate() {
        let address = wcontainers_translator_test::programs::LOAD + offset;
        let name: Vec<&str> = symbols
            .iter()
            .filter(|(a, _)| *a == address)
            .map(|(_, n)| n.as_str())
            .collect();
        println!("function {}: offset {offset:#x} {}", 8 + i, name.join(" "));
    }
    println!("wrote {base}.wasm and {base}.cwasm");
}
