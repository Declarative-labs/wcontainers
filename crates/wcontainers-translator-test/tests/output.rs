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
//! per guest.

use std::fmt::Write;
use wcontainers_arm64::Arm64;
use wcontainers_translator::{BOUND, LookAhead, Optimise, Translation, translate};
use wcontainers_translator_test::binaries::{Binary, sources};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{Library, Position, build_with};
use wcontainers_translator_test::runner::probe;

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
    writeln!(out, "{name} {} {}", wasm.len(), blake3::hash(wasm)).unwrap();
}

fn lines(out: &mut String, name: &str, translation: &Translation) {
    for (i, module) in translation.modules.iter().enumerate() {
        line(out, &format!("{name}-{i}"), &module.wasm);
    }
}

/// The benchmark set: the whole program (as `measure_benchmarks` sizes it) and each workload alone, from
/// address 0, and the thread module.
fn bench<G: GuestTest>(out: &mut String) {
    line(out, "thread", &wcontainers_translator::thread_module::<G>());
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
                        &[0],
                        look_ahead,
                        optimise,
                        host,
                        G::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        out,
                        &format!("{}-{host_label}-{label}-{optimise:?}", blob.name),
                        &translation,
                    );
                }
            }
        }
    }
}

/// A real binary from every start its file offers.
fn binary<G: GuestTest>(out: &mut String, name: &str) {
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
                        starts,
                        look_ahead,
                        optimise,
                        host,
                        G::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        out,
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

fn write_output<G: GuestTest>() {
    let directory = std::env::var("WC_OUTPUT_DIR").expect("WC_OUTPUT_DIR names where to write");
    let sets = std::env::var("WC_OUTPUT_SETS").expect("WC_OUTPUT_SETS names the sets");
    std::fs::create_dir_all(&directory).unwrap();
    for set in sets.split(',') {
        let mut out = String::new();
        match set {
            "bench" => bench::<G>(&mut out),
            name => binary::<G>(&mut out, name),
        }
        std::fs::write(format!("{directory}/{set}.txt"), out).unwrap();
        eprintln!("wrote {directory}/{set}.txt");
    }
}
