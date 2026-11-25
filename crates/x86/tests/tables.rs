//! The crate's own tables checked against iced-x86's `instr_info` answers, which the shipped decoder leaves
//! out: each `Code`'s claim, the integer memory sizes and the general registers' numbers and widths.

use iced_x86::{Code, MemorySize, Register};
use wcontainers_x86::claims::*;

/// Every `Code` has the claim its features and privilege give it.
#[test]
fn every_code_has_its_claim() {
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for code in Code::values() {
        let features: Vec<String> = code
            .cpuid_features()
            .iter()
            .map(|f| format!("{f:?}"))
            .filter(|f| !BASELINE_FEATURES.contains(&f.as_str()))
            .collect();
        let kernel = code.is_privileged() || code == Code::Rdpmc;
        let told = |name: &str| {
            let at = TOLD_FEATURES.iter().position(|&n| n == name).unwrap();
            CLAIM_FIRST_TOLD + at as u8
        };
        let expected = if matches!(code, Code::Lahf | Code::Sahf) {
            told(LAHF_SAHF_FAMILY)
        } else if features
            .iter()
            .all(|f| KERNEL_FEATURES.contains(&f.as_str()))
        {
            if kernel { CLAIM_KERNEL } else { CLAIM_BASELINE }
        } else if let [one] = &features[..]
            && TOLD_FEATURES.contains(&one.as_str())
        {
            told(one)
        } else {
            CLAIM_NEVER
        };
        assert_eq!(wcontainers_x86::claim(code), expected, "{code:?}");
        let name = match expected {
            CLAIM_BASELINE => "baseline",
            CLAIM_KERNEL => "kernel",
            CLAIM_NEVER => "never",
            c => TOLD_FEATURES[wcontainers_x86::told_feature(c).unwrap()],
        };
        *counts.entry(name).or_default() += 1;
    }
    eprintln!("claims: {counts:?}");
    // A few by hand.
    let claim = wcontainers_x86::claim;
    let told = |code| wcontainers_x86::told_feature(claim(code)).map(|at| TOLD_FEATURES[at]);
    assert_eq!(claim(Code::Add_rm64_r64), CLAIM_BASELINE);
    assert_eq!(told(Code::VEX_Vaddps_ymm_ymm_ymmm256), Some("AVX"));
    assert_eq!(told(Code::VEX_Vpaddd_ymm_ymm_ymmm256), Some("AVX2"));
    assert_eq!(told(Code::VEX_Pdep_r64_r64_rm64), Some("BMI2"));
    assert_eq!(told(Code::Popcnt_r64_rm64), Some("POPCNT"));
    assert_eq!(told(Code::Lahf), Some(LAHF_SAHF_FAMILY));
    assert_eq!(told(Code::Xsetbv), Some("XSAVE"));
    assert_eq!(
        claim(Code::EVEX_Vaddps_zmm_k1z_zmm_zmmm512b32_er),
        CLAIM_NEVER
    );
    assert_eq!(claim(Code::Hlt), CLAIM_KERNEL);
    assert_eq!(claim(Code::Rdmsr), CLAIM_KERNEL);
    assert_eq!(claim(Code::Vmlaunch), CLAIM_NEVER);
}

/// Every memory size the crate knows has iced's size.
#[test]
fn memory_sizes_are_iceds() {
    let mut known = 0;
    for size in MemorySize::values() {
        let ours = wcontainers_x86::memory_size_bytes(size);
        if ours != 0 {
            assert_eq!(ours as usize, size.size(), "{size:?}");
            known += 1;
        }
    }
    assert!(known >= 20);
}

/// Every general register's number and width are iced's.
#[test]
fn general_registers_are_iceds() {
    let mut seen = 0;
    for r in Register::values() {
        match wcontainers_x86::general_register(r) {
            Some(wcontainers_x86::General {
                number,
                width,
                high,
            }) => {
                assert!(r.is_gpr(), "{r:?}");
                assert_eq!(usize::from(width), r.size() * 8, "{r:?}");
                assert_eq!(
                    high,
                    matches!(r, Register::AH | Register::CH | Register::DH | Register::BH)
                );
                let full = r.full_register();
                assert_eq!(usize::from(number), full.number(), "{r:?}");
                seen += 1;
            }
            None => assert!(!r.is_gpr(), "{r:?}"),
        }
    }
    assert_eq!(seen, 68);
}

/// Encodings longer than 15 bytes have the CPU's length, and shorter ones iced's.
#[test]
fn lengths_past_fifteen_bytes() {
    let length = |bytes: &[u8]| wcontainers_x86::decode(bytes).map(|(i, n)| (n, i.too_long));
    // The longest legal instruction, 15 bytes, and with a segment prefix before it 16.
    let longest = [
        0xf3, 0x66, 0x67, 0x48, 0xc7, 0x84, 0x24, 1, 2, 3, 4, 5, 6, 7, 8,
    ];
    assert_eq!(length(&longest), Some((15, false)));
    let mut over = vec![0x2e];
    over.extend(longest);
    assert_eq!(length(&over), Some((16, true)));
    // Fifteen operand-size prefixes before a nop: 16 bytes.
    let mut prefixes = vec![0x66; 15];
    prefixes.push(0x90);
    assert_eq!(length(&prefixes), Some((16, true)));
    // Fourteen and a nop: 15 bytes, an instruction.
    assert_eq!(length(&prefixes[1..]), Some((15, false)));
    // A run cut short by the end of the code is no instruction.
    assert_eq!(length(&[0x66; 15]), None);
}
