//! Not a check by itself: writes what the translator makes of the benchmark set and the real binaries, so
//! that two commits' output can be compared byte for byte (`same-output.sh` runs it on both and compares).
//! Each module is written as one line of `<directory>/<set>.txt`: its name (set, look-ahead, setting and
//! number), its length and its BLAKE3 hash. One command:
//! `WC_OUTPUT_DIR=<directory> WC_OUTPUT_SETS=bench,busybox nix develop -c cargo test --release -p wcontainers-translator-test --test output -- --ignored --nocapture`
//! (sets: `bench`, which is the benchmark set and the thread module, and the names of the real binaries).

use std::fmt::Write;
use wcontainers_translator::{BOUND, LookAhead, Optimise, Translation, translate};
use wcontainers_translator_test::binaries::{Binary, SOURCES};
use wcontainers_translator_test::programs::{Library, build_with};
use wcontainers_translator_test::runner::probe;

const LOOK_AHEADS: [(&str, LookAhead); 2] = [
    ("full", LookAhead::Full),
    ("bounded", LookAhead::Bounded(BOUND)),
];
const SETTINGS: [Optimise; 2] = [Optimise::Speed, Optimise::Size];

fn line(out: &mut String, name: &str, wasm: &[u8]) {
    writeln!(out, "{name} {} {}", wasm.len(), blake3::hash(wasm)).unwrap();
}

fn lines(out: &mut String, name: &str, translation: &Translation) {
    for (i, module) in translation.modules.iter().enumerate() {
        line(out, &format!("{name}-{i}"), &module.wasm);
    }
}

/// The benchmark set: the whole program (as `measure_benchmarks` sizes it) and each workload alone, from
/// address 0, and the thread module.
fn bench(out: &mut String) {
    line(out, "thread", &wcontainers_translator::thread_module());
    let mut blobs = vec![build_with("bench", "-O2", Library::None, &["SCALE=1"])];
    for k in 1..=8 {
        let workload = format!("WORKLOAD={k}");
        blobs.push(build_with(
            "bench",
            "-O2",
            Library::None,
            &["SCALE=1", &workload],
        ));
    }
    for blob in &blobs {
        for (label, look_ahead) in LOOK_AHEADS {
            for optimise in SETTINGS {
                let translation = translate(
                    &blob.bytes,
                    0..blob.code_bytes,
                    &[0],
                    look_ahead,
                    optimise,
                    probe(),
                )
                .unwrap();
                lines(
                    out,
                    &format!("{}-{label}-{optimise:?}", blob.name),
                    &translation,
                );
            }
        }
    }
}

/// A real binary from every start its file offers.
fn binary(out: &mut String, name: &str) {
    let source = SOURCES
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no set named {name}"));
    let binary = Binary::read(source.name, &std::fs::read(source.file()).unwrap());
    let starts = binary.starts.all();
    for (label, look_ahead) in LOOK_AHEADS {
        for optimise in SETTINGS {
            let translation = translate(
                &binary.image,
                binary.code.clone(),
                &starts,
                look_ahead,
                optimise,
                probe(),
            )
            .unwrap();
            lines(out, &format!("{name}-{label}-{optimise:?}"), &translation);
        }
    }
}

#[test]
#[ignore]
fn write_output() {
    let directory = std::env::var("WC_OUTPUT_DIR").expect("WC_OUTPUT_DIR names where to write");
    let sets = std::env::var("WC_OUTPUT_SETS").expect("WC_OUTPUT_SETS names the sets");
    std::fs::create_dir_all(&directory).unwrap();
    for set in sets.split(',') {
        let mut out = String::new();
        match set {
            "bench" => bench(&mut out),
            name => binary(&mut out, name),
        }
        std::fs::write(format!("{directory}/{set}.txt"), out).unwrap();
        eprintln!("wrote {directory}/{set}.txt");
    }
}
