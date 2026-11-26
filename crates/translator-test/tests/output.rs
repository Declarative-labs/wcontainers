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
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::DESCRIPTION_SECTION;
use wcontainers_translator::{
    AccessOrdering, BOUND, LookAhead, Optimise, OrderedAccess, Translation, translate,
};
use wcontainers_translator_test::binaries::{Binary, sources};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::ordered::{Scan, scan, validity};
use wcontainers_translator_test::programs::{Library, Position, build_with};
use wcontainers_translator_test::runner::{access_ordering, probe};
use wcontainers_x86::X86_64;

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
/// The real binary translated for every probe answer, beside the benchmark set.
const EVERY_HOST: &str = "musl";

/// The hosts a set is translated for, each with its label.
fn hosts<G: GuestTest>(set: &str) -> Vec<(String, G::Host)> {
    G::PROBE_ANSWERS
        .iter()
        .map(|&answer| (format!("h{answer}"), G::host_from_probe(answer)))
        .filter(|(_, host)| set == "bench" || set == EVERY_HOST || *host == probe::<G>())
        .collect()
}

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

/// The thread module's name in the lists.
const THREAD: &str = "thread";

/// What is done with each module made: its name and its bytes.
type Each<'a> = &'a mut dyn FnMut(&str, &[u8]);

fn lines(each: Each<'_>, name: &str, translation: &Translation) {
    for (i, module) in translation.modules.iter().enumerate() {
        each(&format!("{name}-{i}"), &module.wasm);
    }
}

/// The benchmark set: the whole program (as `measure_benchmarks` sizes it) and each workload alone, from
/// address 0, and the thread module, in the form `ordering`.
fn bench<G: GuestTest>(each: Each<'_>, ordering: AccessOrdering) {
    each(
        THREAD,
        &wcontainers_translator::thread_module::<G>(wcontainers_translator::ProgressCount::Absent),
    );
    let mut blobs = vec![build_with::<G>(
        "bench",
        "-O2",
        Library::None,
        &["SCALE=1"],
        Position::Fixed,
    )];
    for k in 1..=8 {
        let workload = format!("WORKLOAD={k}");
        blobs.push(build_with::<G>(
            "bench",
            "-O2",
            Library::None,
            &["SCALE=1", &workload],
            Position::Fixed,
        ));
    }
    for blob in &blobs {
        for (host_label, host) in hosts::<G>("bench") {
            for (label, look_ahead) in look_aheads() {
                for optimise in SETTINGS {
                    let translation = translate::<G>(
                        &blob.bytes,
                        0..blob.code_bytes,
                        blob.link_address,
                        &[0],
                        look_ahead,
                        optimise,
                        wcontainers_translator::ProgressCount::Absent,
                        ordering,
                        host,
                        G::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        each,
                        &format!("{}-{host_label}-{label}-{optimise:?}", blob.name),
                        &translation,
                    );
                }
            }
        }
    }
}

/// A real binary from every start its file offers, in the form `ordering`.
fn binary<G: GuestTest>(each: Each<'_>, name: &str, ordering: AccessOrdering) {
    let source = sources::<G>()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no set named {name}"));
    let binary = Binary::read::<G>(source.name, &std::fs::read(source.file::<G>()).unwrap());
    let all = binary.starts.all();
    let mut start_sets = vec![(String::new(), all, false)];
    for (means, starts) in binary.starts.by_means() {
        let means = format!("-{}", means.replace(' ', "_"));
        start_sets.push((means, starts.iter().copied().collect(), true));
    }
    for (host_label, host) in hosts::<G>(name) {
        for (label, look_ahead) in look_aheads() {
            for (means, starts, alone) in &start_sets {
                if *alone && !matches!(label, "full" | "bounded") || starts.is_empty() {
                    continue;
                }
                for optimise in SETTINGS {
                    let translation = translate::<G>(
                        &binary.image,
                        binary.code.clone(),
                        binary.link_address,
                        starts,
                        look_ahead,
                        optimise,
                        wcontainers_translator::ProgressCount::Absent,
                        ordering,
                        host,
                        G::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        each,
                        &format!("{name}-{host_label}-{label}{means}-{optimise:?}"),
                        &translation,
                    );
                }
            }
        }
    }
}

#[test]
#[ignore]
fn write_output_arm64() {
    write_output::<Arm64>();
}

#[test]
#[ignore]
fn write_output_x86_64() {
    write_output::<X86_64>();
}

fn write_output<G: GuestTest>() {
    let directory = std::env::var("WC_OUTPUT_DIR").expect("WC_OUTPUT_DIR names where to write");
    let sets = std::env::var("WC_OUTPUT_SETS").expect("WC_OUTPUT_SETS names the sets");
    std::fs::create_dir_all(&directory).unwrap();
    for set in sets.split(',') {
        let mut out = String::new();
        let mut each = |name: &str, wasm: &[u8]| line(&mut out, name, wasm);
        walk::<G>(set, &mut each, access_ordering::<G>());
        std::fs::write(format!("{directory}/{set}.txt"), out).unwrap();
        eprintln!("wrote {directory}/{set}.txt");
    }
}

/// Every module of one set (`bench` or a binary's name) in the form `ordering`.
fn walk<G: GuestTest>(set: &str, each: Each<'_>, ordering: AccessOrdering) {
    match set {
        "bench" => bench::<G>(each, ordering),
        name => binary::<G>(each, name, ordering),
    }
}

/// The benchmark set and every real binary, in each ordered form: every code module validates, and every function
/// of it makes only the accesses to the guest's memory its form allows (no plain load, no plain store unless
/// the form keeps stores plain, every bulk operation between two fences). The sets, look-aheads and probe
/// answers are those of the output lists (`WC_OUTPUT_SETS`, default all, and `WC_OUTPUT_LOOK_AHEADS`).
#[test]
#[ignore]
fn ordered_modules_make_only_ordered_accesses_x86_64() {
    let kind = OrderedAccess::SequentiallyConsistent;
    let every = std::iter::once("bench")
        .chain(sources::<X86_64>().map(|source| source.name))
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
            walk::<X86_64>(set, &mut each, ordering);
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
