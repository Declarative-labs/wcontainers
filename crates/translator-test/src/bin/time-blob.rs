//! Times flat blobs natively on the reference, and translated at full look-ahead in wasmtime and in Chrome on
//! the machine it runs on, so a test program built where the compilers are can be timed on another machine.
//!
//! Usage: `time-blob dump <guest> <program> <optimisation> <out file> [define ...]` builds a program (no
//! library) and writes its blob, its code size and its link address first as eight little-endian bytes each;
//! `time-blob <how> <guest>
//! <blob file> <runs>`, where `how` is `native` (on the guest's reference), `wasmtime` or `chrome` (translated
//! for speed with every check), runs it that many times and prints the status, the best time the guest ran
//! and the output; `time-blob rounds <guest> <rounds> <blob file> ...` runs each blob natively, in wasmtime
//! and in Chrome in turn, once each a round, so that a change of the machine's load falls on all three, and
//! prints every time with the load before it, then each one's best and its times native.

use wcontainers_translator::{LookAhead, Omit, Optimise};

/// The two words before a dumped blob: its code size and its link address.
const HEADER_WORD: usize = size_of::<u64>();
/// The commands.
const DUMP: &str = "dump";
use wcontainers_translator_test::NATIVE;
const WASMTIME: &str = "wasmtime";
const CHROME: &str = "chrome";
const ROUNDS: &str = "rounds";
/// The order of each round.
const EACH_ROUND: [&str; 3] = [NATIVE, WASMTIME, CHROME];
/// The translation every timed run uses.
const HOW: (LookAhead, Optimise) = (LookAhead::Full, Optimise::Speed);

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        [DUMP, guest, program, optimisation, out, defines @ ..]
            if *guest == wcontainers_translator_test::x86_64::part::NAME =>
        {
            x86_64::dump(program, optimisation, out, defines)
        }
        [DUMP, guest, program, optimisation, out, defines @ ..]
            if *guest == wcontainers_translator_test::arm64::part::NAME =>
        {
            arm64::dump(program, optimisation, out, defines)
        }
        [ROUNDS, guest, count, files @ ..]
            if *guest == wcontainers_translator_test::x86_64::part::NAME =>
        {
            x86_64::rounds(count.parse().unwrap(), files)
        }
        [ROUNDS, guest, count, files @ ..]
            if *guest == wcontainers_translator_test::arm64::part::NAME =>
        {
            arm64::rounds(count.parse().unwrap(), files)
        }
        [how, guest, file, runs] if *guest == wcontainers_translator_test::x86_64::part::NAME => {
            x86_64::time(how, file, runs.parse().unwrap())
        }
        [how, guest, file, runs] if *guest == wcontainers_translator_test::arm64::part::NAME => {
            arm64::time(how, file, runs.parse().unwrap())
        }
        _ => panic!(
            "usage: time-blob dump <guest> <program> <optimisation> <out file> [define ...], time-blob <native|wasmtime|chrome> <guest> <blob file> <runs>, or time-blob rounds <guest> <rounds> <blob file> ..."
        ),
    }
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Blob, LOAD, Library, Position, STACK_BYTES, build_with, load, native, native_seconds,
        translated_omitting,
    };

    include!("for_guest/time-blob.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Blob, LOAD, Library, Position, STACK_BYTES, build_with, load, native, native_seconds,
        translated_omitting,
    };

    include!("for_guest/time-blob.rs");
}
