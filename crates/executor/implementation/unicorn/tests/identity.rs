//! The identification registers and hardware capability words compared with real ARM64 Linux.
//!
//! The reference runs on a chip with more features than the one the kernel presents, so a field
//! Linux lets a program see
//! (<https://docs.kernel.org/arch/arm64/cpu-feature-registers.html#:~:text=List%20of%20registers%20with%20visible%20features>)
//! may say less here than there, and a capability may be missing here; every field Linux hides
//! shows its safe value, the same on every chip, so it must be equal; and a read is undefined in
//! the same places.

use std::collections::BTreeMap;
use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

/// One identification register's (CRm, op2).
type Place = (u8, u8);

/// The registers whose whole value names the processor rather than its features.
///
/// MIDR_EL1 and REVIDR_EL1, which Linux answers with the processor's own.
const NAMING_THE_PROCESSOR: [Place; 2] = [(0, 0), (0, 6)];

/// The `AT_HWCAP` bit Linux sets on every processor: CPUID, as it answers the registers' reads.
const ON_EVERY_PROCESSOR: u64 = 1 << 11;

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

/// What a program printed: each register's value (`None` where the read is undefined) and each
/// capability word.
struct Seen {
    registers: BTreeMap<Place, Option<u64>>,
    capabilities: BTreeMap<String, u64>,
}

fn parse(text: &str) -> Seen {
    let mut seen = Seen {
        registers: BTreeMap::new(),
        capabilities: BTreeMap::new(),
    };
    for line in text.lines() {
        let (name, value) = line.split_once(' ').expect("a name and a value");
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
            seen.capabilities.insert(
                name.to_string(),
                u64::from_str_radix(value, 16).expect("a value"),
            );
        }
    }
    seen
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
    for (name, ours) in &ours_seen.capabilities {
        let theirs = theirs_seen
            .capabilities
            .get(name)
            .copied()
            .unwrap_or_default();
        if name == "AT_HWCAP" && (ours ^ theirs) & ON_EVERY_PROCESSOR != 0 {
            differences.push(format!(
                "{name}: ours {ours:016x}, theirs {theirs:016x} differ in CPUID"
            ));
        }
        if ours & !theirs != 0 {
            differences.push(format!(
                "{name}: ours {ours:016x} has bits {:016x} theirs {theirs:016x} lacks",
                ours & !theirs
            ));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
