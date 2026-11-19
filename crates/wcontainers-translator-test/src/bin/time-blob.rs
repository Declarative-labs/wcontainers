//! Times flat blobs translated at full look-ahead in wasmtime on the machine it runs on, so a test program
//! built where the compilers are can be timed on another machine.
//!
//! Usage: `time-blob dump <guest> <program> <optimisation> <out file> [define ...]` builds a program (no
//! library) and writes its blob, its code size first as eight little-endian bytes; `time-blob time <guest>
//! <blob file> <runs>` translates it for speed, runs it that many times and prints the status, the best time the
//! guest ran and the output.

use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{
    Blob, LOAD, Library, STACK_BYTES, build_with, translated_omitting,
};
use wcontainers_x86::X86_64;

const CODE_SIZE: usize = size_of::<u64>();
/// The two commands.
const DUMP: &str = "dump";
const TIME: &str = "time";

fn dump<G: GuestTest>(program: &str, optimisation: &str, out: &str, defines: &[&str]) {
    let blob = build_with::<G>(program, optimisation, Library::None, defines);
    let mut bytes = blob.code_bytes.to_le_bytes().to_vec();
    bytes.extend(&blob.bytes);
    std::fs::write(out, bytes).unwrap();
}

fn run<G: GuestTest>(file: &str, runs: u32) {
    let bytes = std::fs::read(file).unwrap();
    let (size, rest) = bytes.split_at(CODE_SIZE);
    let blob = Blob {
        name: file.into(),
        bytes: rest.to_vec(),
        code_bytes: u64::from_le_bytes(size.try_into().unwrap()),
        load: LOAD,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    };
    let mut best = f64::INFINITY;
    let mut outcome = None;
    for _ in 0..runs {
        let (ran, _, _, time) =
            translated_omitting::<G>(&blob, (LookAhead::Full, Optimise::Speed), Omit::default());
        best = best.min(time.as_secs_f64());
        outcome = Some((ran.stdout, ran.status));
    }
    let (out, status) = outcome.expect("one run at least");
    let out = String::from_utf8_lossy(&out);
    println!("{file}: status {status}, best of {runs} {best:.3} s, output {out:?}");
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
        [TIME, guest, file, runs] if *guest == X86_64::NAME => {
            run::<X86_64>(file, runs.parse().unwrap())
        }
        [TIME, guest, file, runs] if *guest == Arm64::NAME => {
            run::<Arm64>(file, runs.parse().unwrap())
        }
        _ => panic!(
            "usage: time-blob dump <guest> <program> <optimisation> <out file> [define ...], or time-blob time <guest> <blob file> <runs>"
        ),
    }
}
