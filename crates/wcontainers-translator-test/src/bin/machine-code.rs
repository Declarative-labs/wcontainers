//! Writes what the engines run for a test program at full look-ahead, for reading the machine code: the
//! code module (`.wasm`) and wasmtime's compiled form of it with the runner's settings (`.cwasm`, an ELF file
//! for `objdump -d`). Prints each piece's function index (as the module's lookup table gives it), blob offset
//! and the program's symbol there. The program is ARM64's; the rest is written once over the guest.
//!
//! Usage: `machine-code <program> <optimisation> <out directory> [define ...]`, such as
//! `machine-code bench -O2 out SCALE=1 WORKLOAD=2`. `WC_OMIT` (a list of stop, permissions, returns) leaves
//! checks out, as the benchmarks do.

use std::process::Command;
use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise, translate_omitting};
use wcontainers_translator_test::guest::GuestTest;
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
    write::<Arm64>(program, optimisation, out, &defines, omit);
}

fn write<G: GuestTest>(program: &str, optimisation: &str, out: &str, defines: &[&str], omit: Omit) {
    let blob = build_with::<G>(program, optimisation, Library::None, defines);
    let mut translation = translate_omitting::<G>(
        &blob.bytes,
        0..blob.code_bytes,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        wcontainers_translator_test::runner::probe::<G>(),
        G::DEFAULT_OFFER,
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
    let first = first_piece(&translation.wasm);
    for (i, offset) in translation.pieces.iter().enumerate() {
        let address = wcontainers_translator_test::programs::LOAD + offset;
        let name: Vec<&str> = symbols
            .iter()
            .filter(|(a, _)| *a == address)
            .map(|(_, n)| n.as_str())
            .collect();
        println!(
            "function {}: offset {offset:#x} {}",
            first + i as u32,
            name.join(" ")
        );
    }
    println!("wrote {base}.wasm and {base}.cwasm");
}

/// The function index of the first piece: the first function the module's lookup table holds.
fn first_piece(wasm: &[u8]) -> u32 {
    use wasmparser::{ElementItems, Operator, Parser, Payload};
    for payload in Parser::new(0).parse_all(wasm) {
        let Payload::ElementSection(elements) = payload.expect("a valid module") else {
            continue;
        };
        for element in elements {
            let first = match element.expect("an element segment").items {
                ElementItems::Functions(functions) => functions.into_iter().next(),
                ElementItems::Expressions(_, expressions) => {
                    expressions.into_iter().next().map(|expression| {
                        match expression?.get_operators_reader().read()? {
                            Operator::RefFunc { function_index } => Ok(function_index),
                            other => panic!("a lookup table entry of {other:?}"),
                        }
                    })
                }
            };
            if let Some(first) = first {
                return first.expect("a function index");
            }
        }
    }
    panic!("a module with no lookup table entries");
}
