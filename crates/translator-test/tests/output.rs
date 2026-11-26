//! Not a check by itself: writes what the translator makes of the benchmark set and the real binaries, so
//! that two commits' output can be compared byte for byte (`same-output.sh` runs it on both and compares).
//! Each module is written as one line of `<directory>/<set>.txt`: its name (set, look-ahead, setting and
//! number), its length and its BLAKE3 hash. One command:
//! `WC_OUTPUT_DIR=<directory> WC_OUTPUT_SETS=bench,busybox nix develop -c cargo test --release -p wcontainers-translator-test --test output -- --ignored --nocapture`
//! (sets: `bench`, which is the benchmark set and the thread module, and the names of the real binaries).
//! `WC_OUTPUT_LOOK_AHEADS` (comma separated labels of [`LOOK_AHEADS`]) limits the look-aheads, all by
//! default. A real binary is translated from all its starts at each look-ahead, and at full and bounded also
//! from each kind of start alone. The benchmark set and [`EVERY_HOST`] are translated for each answer the
//! guest's probe can give, the other binaries for this machine's. Written once over the guest, with one test
//! per guest. The runners' access ordering setting chooses the form; with [`CUT_DESCRIPTION_SETTING`] set,
//! each module is hashed without its description section, whose layout two commits may differ in.

use std::fmt::Write;
use wcontainers_translator::contract::{DESCRIPTION_SECTION, THREAD};
use wcontainers_translator::{
    AccessOrdering, BOUND, LookAhead, Optimise, OrderedAccess, Translation, translate,
};
use wcontainers_translator_test::ordered::{Scan, scan, validity};

/// Set to any value, a module's line is of its bytes without the description section.
const CUT_DESCRIPTION_SETTING: &str = "WC_OUTPUT_CUT_DESCRIPTION";

const LOOK_AHEADS: [(&str, LookAhead); 7] = [
    ("none", LookAhead::None),
    ("b1", LookAhead::Bounded(1)),
    ("b17", LookAhead::Bounded(17)),
    ("b64", LookAhead::Bounded(64)),
    ("bounded", LookAhead::Bounded(BOUND)),
    ("b4096", LookAhead::Bounded(4096)),
    ("full", LookAhead::Full),
];
/// The look-aheads asked for.
fn look_aheads() -> Vec<(&'static str, LookAhead)> {
    let asked = std::env::var("WC_OUTPUT_LOOK_AHEADS").ok();
    LOOK_AHEADS
        .into_iter()
        .filter(|(label, _)| {
            asked
                .as_deref()
                .is_none_or(|a| a.split(',').any(|l| l == *label))
        })
        .collect()
}
const SETTINGS: [Optimise; 2] = [Optimise::Speed, Optimise::Size];

fn line(out: &mut String, name: &str, wasm: &[u8]) {
    let wasm = if std::env::var_os(CUT_DESCRIPTION_SETTING).is_some() {
        without_description(wasm)
    } else {
        wasm.to_vec()
    };
    writeln!(out, "{name} {} {}", wasm.len(), blake3::hash(&wasm)).unwrap();
}

/// The module with its description section left out: the header, then every other section as it is.
fn without_description(wasm: &[u8]) -> Vec<u8> {
    /// The magic and the version: https://webassembly.github.io/spec/core/binary/modules.html#binary-module
    const HEADER_BYTES: usize = 8;
    /// https://webassembly.github.io/spec/core/binary/modules.html#custom-section
    const CUSTOM: u8 = 0;
    let mut kept = wasm[..HEADER_BYTES].to_vec();
    let mut at = HEADER_BYTES;
    while at < wasm.len() {
        let id = wasm[at];
        let (size, size_bytes) = leb128(&wasm[at + 1..]);
        let contents = at + 1 + size_bytes;
        let end = contents + size;
        let (name_length, name_bytes) = leb128(&wasm[contents..]);
        let name = &wasm[contents + name_bytes..][..name_length.min(end - contents - name_bytes)];
        if id != CUSTOM || name != DESCRIPTION_SECTION.as_bytes() {
            kept.extend_from_slice(&wasm[at..end]);
        }
        at = end;
    }
    kept
}

/// An unsigned LEB128 number at the start of `bytes`, and how many bytes it takes.
fn leb128(bytes: &[u8]) -> (usize, usize) {
    /// Each byte's high bit says another follows, below it 7 bits of the number:
    /// https://webassembly.github.io/spec/core/binary/values.html#integers
    const MORE: u8 = 0x80;
    const BITS: u32 = MORE.trailing_zeros();
    let (mut value, mut taken) = (0, 0);
    for (i, &byte) in bytes.iter().enumerate() {
        value |= usize::from(byte & !MORE) << (BITS * i as u32);
        taken = i + 1;
        if byte & MORE == 0 {
            break;
        }
    }
    (value, taken)
}

/// What is done with each module made: its name and its bytes.
type Each<'a> = &'a mut dyn FnMut(&str, &[u8]);

fn lines(each: Each<'_>, name: &str, translation: &Translation) {
    for (i, module) in translation.modules.iter().enumerate() {
        each(&format!("{name}-{i}"), &module.wasm);
    }
}

#[test]
#[ignore]
fn write_output_arm64() {
    arm64::write_output();
}

#[test]
#[ignore]
fn write_output_x86_64() {
    x86_64::write_output();
}

/// Checks every code module of the benchmark set and the real binaries in each ordered form.
///
/// Each module validates, and its functions make only the guest accesses its form allows: no plain load, no
/// plain store unless the form keeps stores plain, every bulk operation between two fences. The sets,
/// look-aheads and probe answers are the output lists' (`WC_OUTPUT_SETS`, default all; `WC_OUTPUT_LOOK_AHEADS`).
#[test]
#[ignore]
fn ordered_modules_make_only_ordered_accesses_x86_64() {
    let kind = OrderedAccess::SequentiallyConsistent;
    let every = std::iter::once("bench")
        .chain(x86_64::sources().map(|source| source.name))
        .collect::<Vec<_>>()
        .join(",");
    let sets = std::env::var("WC_OUTPUT_SETS").unwrap_or(every);
    let mut wrong = Vec::new();
    for ordering in [
        AccessOrdering::Ordered(kind),
        AccessOrdering::LoadsOrdered(kind),
    ] {
        for set in sets.split(',') {
            let (mut modules, mut found) = (0, Scan::default());
            let mut each = |name: &str, wasm: &[u8]| {
                // The thread module's one memory is the thread's state, and it has no form.
                if name == THREAD {
                    return;
                }
                modules += 1;
                if let Err(error) = validity(wasm) {
                    found.wrong.push(format!("{name}: not valid: {error}"));
                }
                let mut scanned = scan(wasm, ordering);
                for line in &mut scanned.wrong {
                    *line = format!("{name}: {line}");
                }
                found.add(scanned);
            };
            x86_64::walk(set, &mut each, ordering);
            eprintln!(
                "{set} {ordering:?}: {modules} modules, {} functions, {} atomic accesses, {} plain stores kept, \
                 {} bulk operations, {} wrong",
                found.functions,
                found.atomic,
                found.plain_stores_kept,
                found.bulk,
                found.wrong.len()
            );
            wrong.extend(
                found
                    .wrong
                    .into_iter()
                    .map(|line| format!("{ordering:?} {line}")),
            );
        }
    }
    for line in wrong.iter().take(SHOWN) {
        eprintln!("{line}");
    }
    assert!(wrong.is_empty(), "{} wrong", wrong.len());
}

/// How many wrong lines the scan prints.
const SHOWN: usize = 50;

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
mod arm64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    pub(super) use wcontainers_translator_test::arm64::binaries::{Binary, sources};
    pub(super) use wcontainers_translator_test::arm64::programs::{Library, Position, build_with};
    pub(super) use wcontainers_translator_test::arm64::runner::{access_ordering, probe};
    use wcontainers_translator_test::arm64::{Cpu, part};

    include!("for_guest/output.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    pub(super) use wcontainers_translator_test::x86_64::binaries::{Binary, sources};
    pub(super) use wcontainers_translator_test::x86_64::programs::{Library, Position, build_with};
    pub(super) use wcontainers_translator_test::x86_64::runner::{access_ordering, probe};
    use wcontainers_translator_test::x86_64::{Cpu, part};

    include!("for_guest/output.rs");
}
