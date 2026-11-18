//! The crate's own tables checked against iced-x86's `instr_info` answers, which the shipped decoder leaves
//! out: each `Code`'s claim group, the integer memory sizes and the general registers' numbers and widths.

use iced_x86::{Code, MemorySize, Register};
use wcontainers_x86::claims::*;

/// Every `Code` is in the group its features and privilege give it.
#[test]
fn every_code_is_in_its_claim_group() {
    let mut counts = [0usize; 4];
    for code in Code::values() {
        let features: Vec<String> = code
            .cpuid_features()
            .iter()
            .map(|f| format!("{f:?}"))
            .collect();
        let all_in = |lists: &[&[&str]]| {
            features
                .iter()
                .all(|f| lists.iter().any(|list| list.contains(&f.as_str())))
        };
        let kernel = code.is_privileged() || code == Code::Rdpmc;
        let expected = if kernel && all_in(&[DEFAULT_FEATURES, KERNEL_FEATURES]) {
            GROUP_KERNEL
        } else if !kernel && all_in(&[DEFAULT_FEATURES]) {
            GROUP_DEFAULT
        } else if all_in(&[DEFAULT_FEATURES, AVX_FEATURES]) {
            GROUP_AVX
        } else {
            GROUP_NEVER
        };
        assert_eq!(wcontainers_x86::claim_group(code), expected, "{code:?}");
        counts[usize::from(expected)] += 1;
    }
    eprintln!("claim groups (default, AVX, never, kernel): {counts:?}");
    // A few by hand.
    assert_eq!(
        wcontainers_x86::claim_group(Code::Add_rm64_r64),
        GROUP_DEFAULT
    );
    assert_eq!(
        wcontainers_x86::claim_group(Code::VEX_Vaddps_ymm_ymm_ymmm256),
        GROUP_AVX
    );
    assert_eq!(
        wcontainers_x86::claim_group(Code::EVEX_Vaddps_zmm_k1z_zmm_zmmm512b32_er),
        GROUP_NEVER
    );
    assert_eq!(wcontainers_x86::claim_group(Code::Hlt), GROUP_KERNEL);
    assert_eq!(wcontainers_x86::claim_group(Code::Rdmsr), GROUP_KERNEL);
    assert_eq!(wcontainers_x86::claim_group(Code::Vmlaunch), GROUP_NEVER);
    assert_eq!(wcontainers_x86::claim_group(Code::Xsetbv), GROUP_AVX);
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
