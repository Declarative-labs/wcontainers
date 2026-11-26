//! RCPPS, RCPSS, RSQRTPS and RSQRTSS, whose results the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 2) bounds only by a relative error of 1.5 times 2 to the -12, so that a chip's own tables decide them.
//!
//! Each lane's result is a significand of 12 bits from a table the chip keeps, read by the source's top 11
//! significand bits for RCP and by its exponent's parity and top 10 significand bits for RSQRT; its exponent is 253
//! less the source's, or for RSQRT the half of 380 less it, rounded down. A result below the smallest normal is a
//! zero; a denormal source is a zero, so its result is an infinity; an infinity gives a zero; a NaN is quieted; RSQRT of
//! a number below zero is the indefinite QNaN, and of -0 an infinity of its sign. No form raises an exception of
//! MXCSR or sets a flag, nor reads DAZ or FTZ.

use iced_x86::Code;

use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};

/// A chip's tables of RCP's and RSQRT's significands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReciprocalTable {
    pub name: &'static str,
    pub reciprocal: &'static [u16; 2048],
    /// The even exponents' entries first.
    pub root: &'static [u16; 2048],
}

impl core::fmt::Debug for ReciprocalTable {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name)
    }
}

/// The tables of an Intel Xeon Platinum 8488C, which an Intel Core i7-8750H shares on every input (an earlier probe).
pub static XEON_PLATINUM_8488C_RECIPROCALS: ReciprocalTable = ReciprocalTable {
    name: "XEON_PLATINUM_8488C_RECIPROCALS",
    reciprocal: &crate::reciprocal_recording::RECIPROCAL,
    root: &crate::reciprocal_recording::RECIPROCAL_ROOT,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReciprocalForm {
    pub instruction: Instruction,
    pub root: bool,
    pub packed: bool,
    pub place: OperandPlace,
    pub code: Code,
}

impl ReciprocalForm {
    pub const fn memory_bytes(&self) -> u64 {
        if self.packed { 16 } else { 4 }
    }
}

pub fn find_reciprocal_form(instruction: Instruction) -> Option<&'static ReciprocalForm> {
    RECIPROCAL_FORMS.iter().find(|form| form.instruction == instruction)
}

const SIGN: u32 = 1 << 31;
const EXPONENT_ONES: u32 = 0xFF;
const FRACTION: u32 = (1 << 23) - 1;
const QUIET: u32 = 1 << 22;
const INFINITY: u32 = 0x7F80_0000;
const INDEFINITE: u32 = 0xFFC0_0000;

/// One lane's result.
pub fn compute(table: &ReciprocalTable, root: bool, source: u32) -> u32 {
    let sign = source & SIGN;
    let exponent = source >> 23 & EXPONENT_ONES;
    let fraction = source & FRACTION;
    if exponent == EXPONENT_ONES && fraction != 0 {
        return source | QUIET;
    }
    if exponent == 0 {
        return sign | INFINITY;
    }
    if root && sign != 0 {
        return INDEFINITE;
    }
    if exponent == EXPONENT_ONES {
        return sign;
    }
    let (biased, entry) = if root {
        let parity = if is_active(Bug::RootParitySwapped) { 1 - (exponent & 1) } else { exponent & 1 };
        ((380 - exponent) / 2, table.root[(parity << 10 | fraction >> 13) as usize])
    } else {
        let index = (fraction >> 12) as usize;
        let index = if is_active(Bug::ReciprocalTableNeighbor) { (index + 1) % 2048 } else { index };
        if exponent >= 253 {
            return sign;
        }
        (253 - exponent, table.reciprocal[index])
    };
    sign | biased << 23 | u32::from(entry) << 11
}

const fn reciprocal(instruction: Instruction, root: bool, packed: bool, place: OperandPlace, code: Code) -> ReciprocalForm {
    ReciprocalForm { instruction, root, packed, place, code }
}

use OperandPlace::{Memory, Register};

pub static RECIPROCAL_FORMS: &[ReciprocalForm] = &[
    reciprocal(Instruction::RcppsXmmpsXmmps, false, true, Register, Code::Rcpps_xmm_xmmm128),
    reciprocal(Instruction::RcppsXmmpsMemps, false, true, Memory, Code::Rcpps_xmm_xmmm128),
    reciprocal(Instruction::RcpssXmmssXmmss, false, false, Register, Code::Rcpss_xmm_xmmm32),
    reciprocal(Instruction::RcpssXmmssMemss, false, false, Memory, Code::Rcpss_xmm_xmmm32),
    reciprocal(Instruction::RsqrtpsXmmpsXmmps, true, true, Register, Code::Rsqrtps_xmm_xmmm128),
    reciprocal(Instruction::RsqrtpsXmmpsMemps, true, true, Memory, Code::Rsqrtps_xmm_xmmm128),
    reciprocal(Instruction::RsqrtssXmmssXmmss, true, false, Register, Code::Rsqrtss_xmm_xmmm32),
    reciprocal(Instruction::RsqrtssXmmssMemss, true, false, Memory, Code::Rsqrtss_xmm_xmmm32),
];
