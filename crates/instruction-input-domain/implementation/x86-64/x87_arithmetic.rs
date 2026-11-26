//! The inputs of the x87 arithmetic, FST to memory, the rounded constants, FSQRT and FRNDINT: each a case of FCW,
//! FSW and the operands' values together.
//!
//! The cases are every class against every class under FNINIT's FCW; the operation's rounding edges (ties at each
//! precision's last bit with an even and an odd last bit, bits beyond a tie, a sticky bit alone, a borrow, overflow,
//! results tiny before rounding that round to the smallest normal or not, exact and inexact) under each of the twelve
//! combinations of precision and rounding control; the same edges with every exception unmasked; an exception
//! pending; and the condition codes set with TOP elsewhere.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_processor_architecture_x86_64::x87::Source;
use wcontainers_processor_architecture_x86_64::x87_arithmetic::Arithmetic;

use crate::OperandValue;
use crate::x87::EMPTY;

/// What the cases of a form vary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum X87CaseSet {
    /// FADD to FDIVR with the source of this kind, of these bytes in memory.
    Arithmetic { operation: Arithmetic, source: Source, bytes: u64 },
    /// FST to memory of these bytes.
    Store { bytes: u64 },
    /// FSQRT and FRNDINT.
    Unary { root: bool },
    /// The rounded constants.
    Constant,
}

/// One case: FCW, FSW (TOP among it), whether every register but the operands' is in use, and the destination's and
/// source's values, the source's in memory as its bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct X87Case {
    pub control: u16,
    pub status: u16,
    pub full: bool,
    pub destination: u128,
    pub source: u128,
}

const ONE: u128 = 0x3FFF_8000_0000_0000_0000;
const LARGEST: u128 = 0x7FFE_FFFF_FFFF_FFFF_FFFF;
const SMALLEST_NORMAL: u128 = 0x0001_8000_0000_0000_0000;
const NEGATIVE: u128 = 1 << 79;
/// FNINIT's FCW: every exception masked, 64-bit precision, rounding to nearest.
const INITIAL_CONTROL: u16 = 0x037F;
/// Every exception unmasked.
const UNMASKED_CONTROL: u16 = 0x0340;

/// Every class FXAM tells apart, each sign of zero and infinity, the formats no operation takes, and empty.
const CLASSES: [u128; 16] = [
    EMPTY,
    0,
    NEGATIVE,
    ONE,
    0xBFFF_C000_0000_0000_0000,
    LARGEST,
    SMALLEST_NORMAL,
    0x0000_0000_0000_0000_0001,
    0x0000_8000_0000_0000_0000,
    0x7FFF_8000_0000_0000_0000,
    0xFFFF_8000_0000_0000_0000,
    0x7FFF_C000_0000_0000_1234,
    0xFFFF_A000_0000_0000_5678,
    0xFFFF_C000_0000_0000_0000,
    0x4000_4000_0000_0000_0000,
    0x7FFF_0000_0000_0000_0001,
];

/// 2 to the power, as an 80-bit value.
const fn power(exponent: i32) -> u128 {
    (((exponent + 16383) as u128) << 64) | 1 << 63
}

/// One plus 2 to the power.
const fn one_plus(exponent: i32) -> u128 {
    ONE | 1 << (63 + exponent)
}

/// Each FCW: the four rounding controls at each of the three precisions, every exception masked.
fn list_controls() -> Vec<u16> {
    [0x0000, 0x0200, 0x0300].iter().flat_map(|precision| (0..4).map(move |rounding| 0x007F | precision | rounding << 10)).collect()
}

/// The register operands' rounding edges of the operation, the destination first.
fn list_edges(operation: Arithmetic) -> Vec<(u128, u128)> {
    let negate = |value: u128| value ^ NEGATIVE;
    match operation {
        Arithmetic::Add | Arithmetic::Subtract => {
            let sign = |value: u128| if operation == Arithmetic::Subtract { negate(value) } else { value };
            vec![
                (ONE, sign(power(-24))),
                (one_plus(-23), sign(power(-24))),
                (ONE, sign(power(-24) | 1 << 30)),
                (ONE, sign(power(-53))),
                (one_plus(-52), sign(power(-53))),
                (ONE, sign(power(-64))),
                (one_plus(-63), sign(power(-64))),
                (ONE, sign(power(-100))),
                (ONE, sign(negate(power(-100)))),
                (LARGEST, sign(LARGEST)),
                (negate(LARGEST), sign(negate(LARGEST))),
                (SMALLEST_NORMAL | 1 << 62, sign(negate(SMALLEST_NORMAL))),
                (SMALLEST_NORMAL | 0x7FF, sign(negate(SMALLEST_NORMAL))),
                (SMALLEST_NORMAL, sign(negate(0x0000_0000_0000_0000_0001))),
                (one_plus(-30), sign(negate(one_plus(-30)))),
            ]
        }
        Arithmetic::Multiply => vec![
            (one_plus(-63), one_plus(-63)),
            (one_plus(-12), one_plus(-12)),
            (0x3FFF_C000_0000_0000_0000, one_plus(-23)),
            (0x3FFF_C000_0000_0000_0000, one_plus(-52)),
            (0x4000_C000_0000_0000_0000, 0x3FFD_AAAA_AAAA_AAAA_AAAB),
            (power(16000), power(1000)),
            (negate(power(16000)), power(1000)),
            (power(-16000), power(-1000)),
            (SMALLEST_NORMAL, power(-1)),
            (SMALLEST_NORMAL | 1, power(-1)),
            (SMALLEST_NORMAL, 0x3FFE_FFFF_FFFF_FFFF_FFFF),
            (negate(SMALLEST_NORMAL), 0x3FFE_FFFF_FFFF_FFFF_FFFF),
            (0x0000_4000_0000_0000_0001, 0x3FFF_C000_0000_0000_0000),
        ],
        Arithmetic::Divide => vec![
            (ONE, 0x4000_C000_0000_0000_0000),
            (0x4000_8000_0000_0000_0000, 0x4000_C000_0000_0000_0000),
            (ONE, 0x4001_E000_0000_0000_0000),
            (negate(ONE), 0x4000_C000_0000_0000_0000),
            (ONE, one_plus(-63)),
            (LARGEST, power(-1)),
            (SMALLEST_NORMAL, power(1)),
            (SMALLEST_NORMAL, 0x4000_C000_0000_0000_0000),
            (ONE, LARGEST),
            (SMALLEST_NORMAL, one_plus(-63)),
            (SMALLEST_NORMAL | 0xFF, 0x3FFF_FFFF_FFFF_FFFF_FFFF),
            (0x0000_4000_0000_0000_0001, 0x4000_C000_0000_0000_0000),
        ],
    }
}

/// A memory source's values: the format's classes and edges, or integers at each end and about zero.
fn list_memory_sources(source: Source, bytes: u64) -> Vec<u128> {
    match (source, bytes) {
        (Source::Integer, 2) => [0u16, 1, 0xFFFF, 3, 0x7FFF, 0x8000, 0x1234].map(u128::from).to_vec(),
        (Source::Integer, _) => [0u32, 1, 0xFFFF_FFFF, 3, 0x7FFF_FFFF, 0x8000_0000, 0x0123_4567].map(u128::from).to_vec(),
        (_, 4) => [
            0u32, 0x8000_0000, 0x3F80_0000, 0xBFC0_0000, 1, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000,
            0x7FC0_0001, 0x7FA0_0001, 0x3380_0000, 0x3F80_0001, 0x4040_0000,
        ]
        .map(u128::from)
        .to_vec(),
        _ => [
            0u64,
            1 << 63,
            0x3FF0_0000_0000_0000,
            0xBFF8_0000_0000_0000,
            1,
            0x000F_FFFF_FFFF_FFFF,
            0x0010_0000_0000_0000,
            0x7FEF_FFFF_FFFF_FFFF,
            0x7FF0_0000_0000_0000,
            0xFFF0_0000_0000_0000,
            0x7FF8_0000_0000_0001,
            0x7FF4_0000_0000_0001,
            0x3CA0_0000_0000_0000,
            0x3FF0_0000_0000_0001,
            0x4008_0000_0000_0000,
        ]
        .map(u128::from)
        .to_vec(),
    }
}

/// The destinations a memory source meets at each rounding control: values whose sum, product or quotient with
/// the memory edges lands at a precision's last bit or beyond it.
const MEMORY_EDGE_DESTINATIONS: [u128; 5] = [ONE, 0x3FFF_FFFF_FFFF_FFFF_FFFF, one_plus(-63), SMALLEST_NORMAL, LARGEST];

fn case(control: u16, destination: u128, source: u128) -> X87Case {
    X87Case { control, status: 0, full: false, destination, source }
}

/// Every case of the set.
pub fn generate_cases(set: X87CaseSet) -> Vec<OperandValue> {
    let controls = list_controls();
    let mut cases = Vec::new();
    match set {
        X87CaseSet::Arithmetic { operation, source, bytes } => {
            let in_memory = source != Source::Register;
            let sources = if in_memory { list_memory_sources(source, bytes) } else { CLASSES.to_vec() };
            let destinations = &CLASSES[..];
            for &destination in destinations {
                for &value in &sources {
                    cases.push(case(INITIAL_CONTROL, destination, value));
                }
            }
            let edges: Vec<(u128, u128)> = if in_memory {
                let edge_sources: Vec<u128> = sources.iter().copied().skip(sources.len() - 4).collect();
                MEMORY_EDGE_DESTINATIONS.iter().flat_map(|&destination| edge_sources.iter().map(move |&value| (destination, value))).collect()
            } else {
                list_edges(operation)
            };
            for &control in &controls {
                cases.extend(edges.iter().map(|&(destination, value)| case(control, destination, value)));
            }
            cases.extend(edges.iter().map(|&(destination, value)| case(UNMASKED_CONTROL, destination, value)));
            let (snan, denormal) = (CLASSES[12], CLASSES[7]);
            let unmasked: Vec<(u128, u128)> = if in_memory {
                sources.iter().map(|&value| (ONE, value)).collect()
            } else {
                vec![(ONE, snan), (CLASSES[9], CLASSES[10]), (ONE, denormal), (ONE, 0), (0, 0), (ONE, CLASSES[14])]
            };
            cases.extend(unmasked.iter().map(|&(destination, value)| case(UNMASKED_CONTROL, destination, value)));
            let normal = if in_memory { sources[2] } else { 0x4000_C000_0000_0000_0000 };
            cases.push(X87Case { control: 0x037E, status: 0x8081, ..case(INITIAL_CONTROL, ONE, normal) });
            cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, ONE, normal) });
            cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, 0x3FFD_AAAA_AAAA_AAAA_AAAB, normal) });
        }
        X87CaseSet::Store { bytes } => {
            let (fraction_bits, exponent_bits) = if bytes == 4 { (23, 8) } else { (52, 11) };
            let bias = (1i32 << (exponent_bits - 1)) - 1;
            // A value at the format's last bit and a half beyond it, the format's largest and past it, its smallest
            // normal and denormal and between them.
            let tie = ONE | 1 << (63 - fraction_bits - 1);
            let odd_tie = ONE | 1 << (63 - fraction_bits) | 1 << (63 - fraction_bits - 1);
            let above = tie | 1;
            let largest = power(bias) | (u128::from(u64::MAX >> 1) & !((1u128 << (63 - fraction_bits)) - 1));
            let past_largest = power(bias) | (u128::from(u64::MAX >> 1) & !((1u128 << (63 - fraction_bits - 1)) - 1));
            let smallest = power(1 - bias);
            let smallest_denormal = power(1 - bias - fraction_bits);
            let between = power(-bias) | 0x5555_5555_5555_5555;
            let edges = [tie, odd_tie, above, largest, past_largest, power(bias + 1), smallest, smallest_denormal, between, power(-bias - fraction_bits - 2)];
            cases.extend(CLASSES.iter().map(|&value| case(INITIAL_CONTROL, value, 0)));
            for &control in controls.iter().filter(|control| *control & 0x0300 == 0x0300) {
                cases.extend(edges.iter().flat_map(|&value| [case(control, value, 0), case(control, value ^ NEGATIVE, 0)]));
            }
            cases.extend(edges.iter().map(|&value| case(UNMASKED_CONTROL, value, 0)));
            cases.extend([CLASSES[7], CLASSES[12], CLASSES[14]].map(|value| case(UNMASKED_CONTROL, value, 0)));
            cases.push(X87Case { control: 0x037E, status: 0x8081, ..case(INITIAL_CONTROL, ONE, 0) });
            cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, tie, 0) });
        }
        X87CaseSet::Unary { root } => {
            cases.extend(CLASSES.iter().map(|&value| case(INITIAL_CONTROL, value, 0)));
            let edges: Vec<u128> = if root {
                vec![0x4000_8000_0000_0000_0000, 0x4000_C000_0000_0000_0000, one_plus(-63), 0x3FFF_FFFF_FFFF_FFFF_FFFF, LARGEST, SMALLEST_NORMAL, 0x0000_0000_0000_0000_0001, 0x0000_0000_0000_0000_0002]
            } else {
                vec![0x3FFE_8000_0000_0000_0000, 0x3FFF_C000_0000_0000_0000, 0x4000_A000_0000_0000_0000, 0xC000_A000_0000_0000_0000, 0x3FFD_C000_0000_0000_0000, 0x403D_FFFF_FFFF_FFFF_FFFF, one_plus(-63), power(62) | 1]
            };
            for &control in &controls {
                cases.extend(edges.iter().map(|&value| case(control, value, 0)));
            }
            cases.extend(edges.iter().map(|&value| case(UNMASKED_CONTROL, value, 0)));
            cases.extend([CLASSES[4], CLASSES[7], CLASSES[12], CLASSES[14]].map(|value| case(UNMASKED_CONTROL, value, 0)));
            cases.push(X87Case { control: 0x037E, status: 0x8081, ..case(INITIAL_CONTROL, ONE, 0) });
            cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, 0x3FFF_C000_0000_0000_0000, 0) });
        }
        X87CaseSet::Constant => {
            for &control in &controls {
                cases.push(case(control, EMPTY, 0));
            }
            cases.push(X87Case { full: true, ..case(INITIAL_CONTROL, EMPTY, 0) });
            cases.push(X87Case { full: true, ..case(UNMASKED_CONTROL, EMPTY, 0) });
            cases.push(X87Case { control: 0x037E, status: 0x8081, ..case(INITIAL_CONTROL, EMPTY, 0) });
            cases.push(X87Case { status: 0x4720 | 5 << 11, ..case(INITIAL_CONTROL, EMPTY, 0) });
        }
    }
    cases.into_iter().map(OperandValue::X87Case).collect()
}
