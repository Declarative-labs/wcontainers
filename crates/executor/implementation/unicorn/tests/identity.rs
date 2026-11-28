//! The identification registers and hardware capability words compared with real ARM64 Linux.
//!
//! The reference runs on a chip with more features than the one the kernel presents, so a field
//! Linux lets a program see
//! (<https://docs.kernel.org/arch/arm64/cpu-feature-registers.html#:~:text=List%20of%20registers%20with%20visible%20features>)
//! may say less here than there; every field Linux hides shows its safe value, the same on every
//! chip, so it must be equal; and a read is undefined in the same places.
//!
//! The capability words and /proc/cpuinfo's Features must say exactly what the registers say: the
//! guest derives both from the fields it read, as Linux does, and on each side they must be equal
//! to what that side's kernel gave. The reference shows that derivation agrees with Linux itself.

use std::collections::BTreeMap;
use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

/// One identification register's (CRm, op2).
type Place = (u8, u8);

/// The registers whose whole value names the processor rather than its features.
///
/// MIDR_EL1 and REVIDR_EL1, which Linux answers with the processor's own.
const NAMING_THE_PROCESSOR: [Place; 2] = [(0, 0), (0, 6)];

/// The `AT_HWCAP` bit no identification register gives: EVTSTRM, the timer's event stream.
///
/// <https://docs.kernel.org/arch/arm64/elf_hwcaps.html#:~:text=HWCAP%5FEVTSTRM>. The reference's
/// kernel may set it; this kernel presents no event stream, so it is 0 here.
const EVENT_STREAM: u64 = 1 << 2;

/// Each register's fields Linux lets a program see, by lowest bit, each 4 bits wide.
const VISIBLE_FIELDS: &[(Place, &[u32])] = &[
    // ID_AA64PFR0_EL1: FP, AdvSIMD, SVE, DIT.
    ((4, 0), &[16, 20, 32, 48]),
    // ID_AA64PFR1_EL1: BT, SSBS, MTE, SME.
    ((4, 1), &[0, 4, 8, 24]),
    // ID_AA64PFR2_EL1: FPMR.
    ((4, 2), &[32]),
    // ID_AA64ZFR0_EL1: SVEver, AES, BitPerm, BF16, B16B16, SHA3, SM4, I8MM, F32MM, F64MM.
    ((4, 4), &[0, 4, 16, 20, 24, 32, 40, 44, 52, 56]),
    // ID_AA64SMFR0_EL1: every field.
    ((4, 5), &[32, 36, 40, 44, 48, 52, 56, 60]),
    // ID_AA64FPFR0_EL1: F8E5M2, F8E4M3 and the four FP8 operations.
    ((4, 7), &[0, 28]),
    // ID_AA64ISAR0_EL1: AES, SHA1, SHA2, CRC32, Atomic, RDM, SHA3, SM3, SM4, DP, FHM, TS, RNDR.
    ((6, 0), &[4, 8, 12, 16, 20, 28, 32, 36, 40, 44, 48, 52, 60]),
    // ID_AA64ISAR1_EL1: DPB, APA, API, JSCVT, FCMA, LRCPC, GPA, GPI, FRINTTS, SB, SPECRES, BF16,
    // DGH, I8MM, XS, LS64.
    (
        (6, 1),
        &[0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60],
    ),
    // ID_AA64ISAR2_EL1: WFxT, RPRES, GPA3, APA3, MOPS, BC, RPRFM, CSSC, LUT.
    ((6, 2), &[0, 4, 8, 12, 16, 20, 48, 52, 56]),
    // ID_AA64ISAR3_EL1: FAMINMAX, LSFE, FPRCVT.
    ((6, 3), &[4, 16, 28]),
    // ID_AA64MMFR0_EL1: ECV.
    ((7, 0), &[60]),
    // ID_AA64MMFR1_EL1: AFP.
    ((7, 1), &[44]),
    // ID_AA64MMFR2_EL1: AT.
    ((7, 2), &[32]),
    // ID_AA64MMFR3_EL1: S1POE.
    ((7, 3), &[16]),
];

/// What a program printed: each register's value, each capability word and each Features line.
///
/// A register is `None` where its read is undefined; the words are `AT_HWCAP`, `derived
/// AT_HWCAP`, ...; the lines `cpuinfo Features`, `cpuinfo same`, `derived Features`.
struct Seen {
    registers: BTreeMap<Place, Option<u64>>,
    capabilities: BTreeMap<String, u64>,
    features: BTreeMap<String, String>,
}

/// The capability words the guest prints, given and derived.
const WORDS: [&str; 4] = [
    "AT_HWCAP",
    "AT_HWCAP2",
    "derived AT_HWCAP",
    "derived AT_HWCAP2",
];
/// The lines about /proc/cpuinfo's Features the guest prints.
const FEATURE_LINES: [&str; 3] = ["cpuinfo Features", "cpuinfo same", "derived Features"];

fn parse(text: &str) -> Seen {
    let mut seen = Seen {
        registers: BTreeMap::new(),
        capabilities: BTreeMap::new(),
        features: BTreeMap::new(),
    };
    for line in text.lines() {
        if let Some(name) = FEATURE_LINES
            .iter()
            .find(|name| line.starts_with(&format!("{name} ")) || line == **name)
        {
            let value = line[name.len()..].trim_start_matches(' ');
            seen.features.insert(name.to_string(), value.to_string());
            continue;
        }
        let (name, value) = line.rsplit_once(' ').expect("a name and a value");
        if let Some(place) = name.strip_prefix("S3_0_C0_C") {
            let (crm, op2) = place.split_once('_').expect("CRm and op2");
            let value = match value {
                "undefined" => None,
                hex => Some(u64::from_str_radix(hex, 16).expect("a value")),
            };
            seen.registers.insert(
                (crm.parse().expect("CRm"), op2.parse().expect("op2")),
                value,
            );
        } else {
            assert!(WORDS.contains(&name), "an unknown line: {line}");
            seen.capabilities.insert(
                name.to_string(),
                u64::from_str_radix(value, 16).expect("a value"),
            );
        }
    }
    seen
}

/// Returns why one side's capability words and Features line differ from what its registers say.
///
/// `event_stream` is the EVTSTRM bit that side's kernel may give besides.
fn capability_differences(side: &str, seen: &Seen, event_stream: u64) -> Vec<String> {
    let word = |name: &str| seen.capabilities.get(name).copied();
    let line = |name: &str| seen.features.get(name).map(String::as_str);
    let mut differences = Vec::new();
    for (given, derived, extra) in [
        ("AT_HWCAP", "derived AT_HWCAP", event_stream),
        ("AT_HWCAP2", "derived AT_HWCAP2", 0),
    ] {
        let (given_word, derived_word) = (word(given), word(derived));
        let expected = derived_word.map(|derived| derived | (given_word.unwrap_or(0) & extra));
        if given_word.is_none() || given_word != expected {
            differences.push(format!(
                "{side} {given}: given {given_word:016x?}, the registers say {expected:016x?}"
            ));
        }
    }
    if line("cpuinfo same") != Some("yes") {
        differences.push(format!(
            "{side}: the processors' Features lines are not all the same"
        ));
    }
    let (features, derived) = (line("cpuinfo Features"), line("derived Features"));
    if features.is_none() || features != derived {
        differences.push(format!(
            "{side} /proc/cpuinfo Features: {features:?}, the registers say {derived:?}"
        ));
    }
    differences
}

/// The field of `value` at `low`.
fn field(value: u64, low: u32) -> u64 {
    value >> low & 0xF
}

/// Why our read of `place` is not what Linux on a chip with our features gives, if it is not.
fn difference(place: Place, ours: Option<u64>, theirs: Option<u64>) -> Option<String> {
    let (ours, theirs) = match (ours, theirs) {
        (None, None) => return None,
        (Some(ours), Some(theirs)) => (ours, theirs),
        _ => return Some(format!("ours {ours:x?}, theirs {theirs:x?}")),
    };
    if NAMING_THE_PROCESSOR.contains(&place) {
        return None;
    }
    let visible = VISIBLE_FIELDS
        .iter()
        .find(|(listed, _)| *listed == place)
        .map_or(&[][..], |(_, fields)| *fields);
    let wrong: Vec<String> = (0..64)
        .step_by(4)
        .filter(|low| {
            let (our, their) = (field(ours, *low), field(theirs, *low));
            if visible.contains(low) {
                // A feature present here is present there; a field of 0xF means none for FP
                // and AdvSIMD, where 0 is the first level.
                let level = |value: u64| {
                    if matches!((place, low), ((4, 0), 16 | 20)) {
                        (value + 1) & 0xF
                    } else {
                        value
                    }
                };
                level(our) > level(their)
            } else {
                our != their
            }
        })
        .map(|low| {
            format!(
                "bits {low} to {}: ours {:x}, theirs {:x}{}",
                low + 3,
                field(ours, low),
                field(theirs, low),
                if visible.contains(&low) {
                    " (visible: more here)"
                } else {
                    " (hidden: its safe value)"
                }
            )
        })
        .collect();
    (!wrong.is_empty()).then(|| {
        format!(
            "ours {ours:016x}, theirs {theirs:016x}: {}",
            wrong.join("; ")
        )
    })
}

#[test]
fn identification_registers_and_capabilities_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("identity", &[], &out);
    let (ours, theirs) = run_both(&program, &[]);
    assert_eq!(ours.outcome.exit_code, theirs.exit_code, "exit codes");
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
    let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
    eprintln!("ours:\n{ours_text}\ntheirs:\n{theirs_text}");
    let (ours_seen, theirs_seen) = (parse(&ours_text), parse(&theirs_text));
    assert_eq!(
        ours_seen.registers.keys().collect::<Vec<_>>(),
        theirs_seen.registers.keys().collect::<Vec<_>>(),
        "registers read"
    );
    let mut differences: Vec<String> = ours_seen
        .registers
        .iter()
        .filter_map(|(place, ours)| {
            difference(*place, *ours, theirs_seen.registers[place])
                .map(|why| format!("S3_0_C0_C{}_{}: {why}", place.0, place.1))
        })
        .collect();
    // Ours exactly as its registers say, with no event stream; theirs as its registers say, which
    // checks the guest's derivation against Linux.
    differences.extend(capability_differences("ours", &ours_seen, 0));
    differences.extend(capability_differences("theirs", &theirs_seen, EVENT_STREAM));
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

/// With several processors, /proc/cpuinfo has one block for each, all with the first's Features.
///
/// The test above checks the first's line; the guest exits 0 when this holds.
#[test]
fn every_processor_shows_the_same_features() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for parallelism in [2, 5] {
        let parallelism = std::num::NonZeroUsize::new(parallelism).expect("some parallelism");
        assert_eq!(
            wcontainers_executor_unicorn::driver::program_exit_code(
                "cpuinfosame",
                &[],
                &out,
                parallelism
            ),
            0,
            "parallelism {parallelism}"
        );
    }
}
