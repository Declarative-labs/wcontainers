//! Times flat blobs natively on the reference, and translated at full look-ahead in wasmtime and in Chrome on
//! the machine it runs on, so a test program built where the compilers are can be timed on another machine.
//!
//! Usage: `time-blob dump <guest> <program> <optimisation> <out file> [define ...]` builds a program (no
//! library) and writes its blob, its code size first as eight little-endian bytes; `time-blob <how> <guest>
//! <blob file> <runs>`, where `how` is `native` (on the guest's reference), `wasmtime` or `chrome` (translated
//! for speed with every check), runs it that many times and prints the status, the best time the guest ran
//! and the output; `time-blob rounds <guest> <rounds> <blob file> ...` runs each blob natively, in wasmtime
//! and in Chrome in turn, once each a round, so that a change of the machine's load falls on all three, and
//! prints every time with the load before it, then each one's best and its times native.

use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{
    Blob, LOAD, Library, Position, STACK_BYTES, build_with, load, native, native_seconds,
    translated_omitting,
};
use wcontainers_x86::X86_64;

const CODE_SIZE: usize = size_of::<u64>();
/// The commands.
const DUMP: &str = "dump";
const NATIVE: &str = "native";
const WASMTIME: &str = "wasmtime";
const CHROME: &str = "chrome";
const ROUNDS: &str = "rounds";
/// The order of each round.
const EACH_ROUND: [&str; 3] = [NATIVE, WASMTIME, CHROME];
/// The translation every timed run uses.
const HOW: (LookAhead, Optimise) = (LookAhead::Full, Optimise::Speed);

fn dump<G: GuestTest>(program: &str, optimisation: &str, out: &str, defines: &[&str]) {
    let blob = build_with::<G>(
        program,
        optimisation,
        Library::None,
        defines,
        Position::Fixed,
    );
    let mut bytes = blob.code_bytes.to_le_bytes().to_vec();
    bytes.extend(&blob.bytes);
    std::fs::write(out, bytes).unwrap();
}

fn read(file: &str) -> Blob {
    let bytes = std::fs::read(file).unwrap();
    let (size, rest) = bytes.split_at(CODE_SIZE);
    Blob {
        name: std::path::Path::new(file)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        bytes: rest.to_vec(),
        code_bytes: u64::from_le_bytes(size.try_into().unwrap()),
        load: LOAD,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    }
}

/// One timed run: the output, the status and the seconds the guest ran.
fn once<G: GuestTest>(how: &str, blob: &Blob) -> (Vec<u8>, i32, f64) {
    match how {
        NATIVE => {
            let ran = native::<G>(blob);
            (ran.stdout, ran.status, native_seconds::<G>(blob, 1))
        }
        WASMTIME => {
            let (ran, _, _, time) = translated_omitting::<G>(blob, HOW, Omit::default());
            (ran.stdout, ran.status, time.as_secs_f64())
        }
        CHROME => {
            let report = browser::translated_with::<G>(blob, HOW, Omit::default());
            let seconds = report.runs.iter().copied().fold(f64::INFINITY, f64::min);
            (report.ran.stdout, report.ran.status, seconds)
        }
        _ => panic!("time natively, in wasmtime or in Chrome, not {how}"),
    }
}

fn time<G: GuestTest>(how: &str, file: &str, runs: u32) {
    let blob = read(file);
    let mut best = f64::INFINITY;
    let mut outcome = None;
    for _ in 0..runs {
        let (out, status, seconds) = once::<G>(how, &blob);
        best = best.min(seconds);
        outcome = Some((out, status));
    }
    let (out, status) = outcome.expect("one run at least");
    let out = String::from_utf8_lossy(&out);
    println!("{file} {how}: status {status}, best of {runs} {best:.3} s, output {out:?}");
}

fn rounds<G: GuestTest>(rounds: u32, files: &[&str]) {
    for file in files {
        let blob = read(file);
        let mut best = [f64::INFINITY; EACH_ROUND.len()];
        let mut outputs = Vec::new();
        for round in 0..rounds {
            for (how, best) in EACH_ROUND.iter().zip(&mut best) {
                let load = load();
                let (out, status, seconds) = once::<G>(how, &blob);
                println!(
                    "{file} round {round} {how}: {seconds:.3} s, status {status} (load {load})"
                );
                *best = best.min(seconds);
                outputs.push((out, status));
            }
        }
        assert!(
            outputs.windows(2).all(|pair| pair[0] == pair[1]),
            "{file}: every run gives the native output"
        );
        let [native, ..] = best;
        let each: String = EACH_ROUND
            .iter()
            .zip(best)
            .map(|(how, best)| format!(" {how} {best:.3} s ({:.2});", best / native))
            .collect();
        println!("{file}: best of {rounds}:{each} times native in brackets");
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        [DUMP, guest, program, optimisation, out, defines @ ..] if *guest == X86_64::NAME => {
            dump::<X86_64>(program, optimisation, out, defines)
        }
        [DUMP, guest, program, optimisation, out, defines @ ..] if *guest == Arm64::NAME => {
            dump::<Arm64>(program, optimisation, out, defines)
        }
        [ROUNDS, guest, count, files @ ..] if *guest == X86_64::NAME => {
            rounds::<X86_64>(count.parse().unwrap(), files)
        }
        [ROUNDS, guest, count, files @ ..] if *guest == Arm64::NAME => {
            rounds::<Arm64>(count.parse().unwrap(), files)
        }
        [how, guest, file, runs] if *guest == X86_64::NAME => {
            time::<X86_64>(how, file, runs.parse().unwrap())
        }
        [how, guest, file, runs] if *guest == Arm64::NAME => {
            time::<Arm64>(how, file, runs.parse().unwrap())
        }
        _ => panic!(
            "usage: time-blob dump <guest> <program> <optimisation> <out file> [define ...], time-blob <native|wasmtime|chrome> <guest> <blob file> <runs>, or time-blob rounds <guest> <rounds> <blob file> ..."
        ),
    }
}
