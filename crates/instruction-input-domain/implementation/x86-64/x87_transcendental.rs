//! The inputs of the x87 transcendental forms: each a case of FCW, FSW and the operands together.
//!
//! The cases are every class against every class under FNINIT's FCW; values on both sides of each threshold the
//! manual names (the trigonometric forms' range at 2 to the 63, F2XM1's range at ±1, FYL2XP1's at ±(1 - √2/2), FYL2X's
//! 1 and √2), near the multiples of π/2 the reduction meets, exact results, and results that overflow or are tiny,
//! under each rounding control and under two other precisions, which these forms ignore; the same edges with every
//! exception unmasked; an exception pending; a full stack; and the condition codes set with TOP elsewhere.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_processor_architecture_x86_64::x87_transcendental::Function;

use crate::x87_arithmetic::X87Case;

const ONE: u128 = 0x3FFF_8000_0000_0000_0000;
const NEGATIVE: u128 = 1 << 79;
const SMALLEST_NORMAL: u128 = 0x0001_8000_0000_0000_0000;
const DENORMAL: u128 = 0x0000_4000_0000_0000_0001;
const LARGEST: u128 = 0x7FFE_FFFF_FFFF_FFFF_FFFF;
/// FNINIT's FCW.
const INITIAL_CONTROL: u16 = 0x037F;
/// Every exception unmasked.
const UNMASKED_CONTROL: u16 = 0x0340;
/// An SNaN.
const SIGNALING: u128 = 0xFFFF_A000_0000_0000_5678;

/// 2 to the power, as an 80-bit value.
const fn power(exponent: i32) -> u128 {
    (((exponent + 16383) as u128) << 64) | 1 << 63
}

/// The next value above in magnitude.
const fn above(value: u128) -> u128 {
    value + 1
}

const fn negate(value: u128) -> u128 {
    value ^ NEGATIVE
}

/// Each FCW: the four rounding controls at 64 bits, and rounding to nearest at 24 and 53, every exception masked.
const CONTROLS: [u16; 6] = [0x037F, 0x077F, 0x0B7F, 0x0F7F, 0x007F, 0x027F];

/// The edges of a unary form's argument.
fn list_unary_edges(function: Function) -> Vec<u128> {
    match function {
        Function::TwoToXMinusOne => vec![
            ONE,
            negate(ONE),
            above(ONE),
            negate(above(ONE)),
            0x3FFE_FFFF_FFFF_FFFF_FFFF,
            0xBFFE_FFFF_FFFF_FFFF_FFFF,
            0x3FFE_8000_0000_0000_0000,
            0xBFFD_947A_E147_AE14_7AE1,
            power(-70),
            negate(power(-70)),
            SMALLEST_NORMAL,
            0x4000_8000_0000_0000_0000,
            negate(0x4005_C800_0000_0000_0000),
            0x4005_C800_0000_0000_0000,
        ],
        _ => vec![
            0x3FFE_C90F_DAA2_2168_C235,
            0x3FFF_C90F_DAA2_2168_C235,
            0x4000_C90F_DAA2_2168_C235,
            0x4000_C90F_DAA2_2168_C234,
            0x4001_96CB_E3F9_990E_91A8,
            negate(0x4002_A000_0000_0000_0000),
            0x4020_9502_F900_0000_0000,
            power(62) | 0x1234_5678_9ABC_DEF0,
            0x403D_FFFF_FFFF_FFFF_FFFF,
            negate(0x403D_FFFF_FFFF_FFFF_FFFF),
            power(63),
            negate(power(64)),
            power(-40),
            SMALLEST_NORMAL,
            0x3FFE_C000_0000_0000_0000,
        ],
    }
}

/// The edges of a binary form, x (ST(0)) first, then y (ST(1)).
fn list_binary_edges(function: Function) -> Vec<(u128, u128)> {
    match function {
        Function::ArcTangent => vec![
            (ONE, ONE),
            (negate(ONE), ONE),
            (negate(ONE), negate(ONE)),
            (0x4000_C000_0000_0000_0000, 0x4000_C000_0000_0000_0001),
            (power(1000), power(-16000)),
            (power(-16000), power(16000)),
            (power(64), ONE),
            (ONE, SMALLEST_NORMAL),
            (negate(ONE), power(-70)),
            (0x4000_8000_0000_0000_0000, negate(0x4000_C000_0000_0000_0000)),
            (LARGEST, power(-100)),
            (0x3FFB_CCCC_CCCC_CCCC_CCCD, negate(ONE)),
        ],
        Function::YLogX => vec![
            (ONE, 0x4000_C000_0000_0000_0000),
            (ONE, negate(ONE)),
            (above(ONE), ONE),
            (0x3FFE_FFFF_FFFF_FFFF_FFFF, ONE),
            (0x4000_8000_0000_0000_0000, 0x4000_C000_0000_0000_0001),
            (0x3FFE_8000_0000_0000_0000, ONE),
            (power(100), negate(0x4000_C000_0000_0000_0000)),
            (0x3FFF_B504_F333_F9DE_6484, ONE),
            (0x3FFF_B504_F333_F9DE_6485, ONE),
            (0x4000_ADF8_5458_A2BB_4A9B, ONE),
            (0x4002_A000_0000_0000_0000, negate(power(-16380))),
            (power(1000), power(16000)),
            (power(-16000), power(16370)),
            (DENORMAL, ONE),
        ],
        Function::YLogXPlusOne => vec![
            (power(-70), ONE),
            (negate(power(-70)), ONE),
            (0x3FFD_8000_0000_0000_0000, 0x4000_C000_0000_0000_0000),
            (negate(0x3FFD_8000_0000_0000_0000), ONE),
            (0x3FFD_95F6_1998_0C43_36F7, ONE),
            (0x3FFD_95F6_1998_0C43_36F8, ONE),
            (negate(0x3FFD_95F6_1998_0C43_36F7), ONE),
            (negate(0x3FFD_95F6_1998_0C43_36F8), ONE),
            (0x3FFE_8000_0000_0000_0000, ONE),
            (negate(0x3FFE_FFBE_76C8_B439_5810), ONE),
            (ONE, ONE),
            (0x4000_8000_0000_0000_0000, negate(ONE)),
            (0x3FFB_CCCC_CCCC_CCCC_CCCD, power(-16382)),
            (0x3FFB_CCCC_CCCC_CCCC_CCCD, LARGEST),
        ],
        _ => Vec::new(),
    }
}

fn case(control: u16, destination: u128, source: u128) -> X87Case {
    X87Case { control, status: 0, full: false, mixed: false, destination, source }
}

/// Every case of a transcendental form, the classes given by the arithmetic's own list.
pub fn generate_cases(function: Function, classes: &[u128]) -> Vec<X87Case> {
    let mut cases = Vec::new();
    let edges: Vec<(u128, u128)> = if function.is_binary() {
        for &x in classes {
            for &y in classes {
                cases.push(case(INITIAL_CONTROL, x, y));
            }
        }
        list_binary_edges(function)
    } else {
        cases.extend(classes.iter().map(|&value| case(INITIAL_CONTROL, value, 0)));
        list_unary_edges(function).into_iter().map(|value| (value, 0)).collect()
    };
    for &control in &CONTROLS {
        cases.extend(edges.iter().map(|&(x, y)| case(control, x, y)));
    }
    cases.extend(edges.iter().map(|&(x, y)| case(UNMASKED_CONTROL, x, y)));
    let (x, y) = edges[0];
    let unmasked: Vec<(u128, u128)> = if function.is_binary() {
        vec![(SIGNALING, ONE), (DENORMAL, ONE), (ONE, DENORMAL), (0, ONE), (negate(ONE), ONE), (classes[9], 0), (classes[14], ONE)]
    } else {
        vec![(SIGNALING, 0), (DENORMAL, 0), (classes[9], 0), (classes[14], 0), (power(-16382) | 0x1234, 0)]
    };
    cases.extend(unmasked.iter().map(|&(x, y)| case(UNMASKED_CONTROL, x, y)));
    cases.push(X87Case { control: 0x037E, status: 0x8081, ..case(INITIAL_CONTROL, x, y) });
    cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, x, y) });
    cases.push(X87Case { status: 0x4700 | 5 << 11, full: true, ..case(UNMASKED_CONTROL, x, y) });
    cases.push(X87Case { status: 0x4720 | 5 << 11, full: true, ..case(INITIAL_CONTROL, power(63), y) });
    cases
}
