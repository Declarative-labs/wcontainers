//! The x87 forms that move, exchange, free, classify and load values and read or write the control and status words,
//! as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 1, chapter 8, and vol. 2) gives them; none rounds.
//!
//! ST(i) is physical register (TOP + i) mod 8. A form that pushes onto a register in use, or reads an empty one, has a
//! stack fault: IE and SF set, C1 set for an overflow and cleared for an underflow. Masked, the value it would have
//! read or pushed is the indefinite QNaN; unmasked, nothing but FSW changes, and ES and B mark the exception pending.
//! A waiting form raises #MF before anything else while an exception is pending. Every form but the control forms
//! (FNINIT, FNCLEX, FLDCW, FNSTCW, FNSTSW, FWAIT and the 8087's no-ops) makes FIP its own address.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;
use crate::integer::{Condition, OperandPlace};
use crate::planted::{Bug, is_active};
use crate::{PreciseInstruction, X87};

/// What an x87 form does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum X87Operation {
    /// FNOP.
    NoOperation,
    /// FWAIT.
    Wait,
    /// FENI, FDISI and FSETPM, which the 8087 and 80287 used and later chips ignore.
    LegacyNoOperation,
    /// FNCLEX: the exception flags, SF, ES and B cleared.
    ClearExceptions,
    /// FNINIT.
    Initialize,
    /// FNSTSW to AX or memory.
    StoreStatus,
    /// FNSTCW.
    StoreControl,
    /// FLDCW.
    LoadControl,
    IncrementTop,
    DecrementTop,
    /// FFREE and FFREEP: ST(i) marked empty, then for FFREEP a pop.
    Free { pop: bool },
    /// FXCH.
    Exchange,
    /// FLD ST(i).
    LoadRegister,
    /// FST and FSTP to ST(i).
    StoreRegister { pop: bool },
    /// FSTPNCE: FSTP to ST(i) without the stack fault, an empty ST(0) leaving ST(i) empty.
    StoreRegisterUnchecked,
    /// FCHS.
    ChangeSign,
    /// FABS.
    Absolute,
    /// FLDZ and FLD1.
    LoadConstant(u128),
    /// FCMOVcc.
    MoveIf(Condition),
    /// FXAM.
    Examine,
    /// FLD m80.
    LoadExtended,
    /// FSTP m80.
    StoreExtended,
    /// FILD.
    LoadInteger,
    /// FLD m32 and m64: the value widened exactly; an SNaN quieted with IE, a denormal with DE.
    LoadFloat,
    /// FCOM, FUCOM, FCOMI, FUCOMI, FTST and FICOM, with their pops: C3, C2 and C0 (or ZF, PF and CF) from ST(0)
    /// compared with the source.
    Compare { ordering: Ordering, source: Source, pops: u8, to_flags: bool },
    /// FIST and FISTP: ST(0) rounded to an integer by FCW's rounding control.
    StoreInteger { pop: bool },
    /// FADD to FDIVR and their pops and integer forms: the destination (ST(0), or ST(i) where the form names it
    /// first) and the source combined, the source first for a reversed form, by [`crate::x87_arithmetic`].
    Arithmetic { operation: crate::x87_arithmetic::Arithmetic, source: Source, reverse: bool, pop: bool },
    /// FST and FSTP to memory in single or double precision: ST(0) rounded by FCW's rounding.
    StoreFloat { pop: bool },
    /// FLDPI, FLDL2E, FLDL2T, FLDLG2 and FLDLN2: the constant's significand cut to 64 bits, rounded up under FCW's
    /// rounding where `above_half` says what the bits beyond hold; none of the five is exact there.
    LoadRoundedConstant { truncated: u128, above_half: bool },
    /// FSQRT.
    SquareRoot,
    /// FRNDINT: ST(0) rounded to an integer by FCW's rounding, whatever its precision control.
    RoundToIntegral,
    /// FSCALE.
    Scale,
    /// FXTRACT: ST(0) replaced by its exponent, then its significand pushed.
    Extract,
    /// FPREM and, `nearest`, FPREM1.
    PartialRemainder { nearest: bool },
    /// FBLD.
    LoadDecimal,
    /// FBSTP.
    StoreDecimal,
    /// FNSTENV: the environment stored, then every exception masked.
    StoreEnvironment,
    /// FLDENV.
    LoadEnvironment,
    /// FNSAVE: the environment and the registers stored, then FNINIT.
    SaveState,
    /// FRSTOR.
    RestoreState,
    /// FXSAVE and, `wide`, FXSAVE64.
    SaveExtended { wide: bool },
    /// FXRSTOR and, `wide`, FXRSTOR64.
    RestoreExtended { wide: bool },
}

/// Which NaNs make a comparison invalid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ordering {
    /// Any NaN, as FCOM's.
    Ordered,
    /// An SNaN alone, as FUCOM's.
    Unordered,
}

/// What ST(0) is compared with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Register,
    Single,
    Double,
    Integer,
    Zero,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct X87Form {
    pub instruction: Instruction,
    pub operation: X87Operation,
    pub place: OperandPlace,
    pub code: Code,
}

/// What a condition code the manual leaves undefined holds after the form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodeRule {
    Unchanged,
    Cleared,
}

/// The rules of the undefined condition codes: C1's, and C0's, C2's and C3's together.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndefinedCodes {
    pub c1: CodeRule,
    pub others: CodeRule,
}

/// When FOP and FDP take the form's opcode and memory operand's address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerUpdate {
    /// After every form but the control forms.
    Always,
    /// Only when the form finds an unmasked exception, as CPUID's FDP_EXCPTN_ONLY says of FDP and Intel's chips since
    /// the Pentium 4 do with FOP unless FOP compatibility is on.
    OnUnmaskedException,
}

/// What FXSAVE and FXRSTOR raise for an area not aligned to 16 while AC is set, which the manual leaves to the chip:
/// #GP(0) always, or #AC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MisalignedArea {
    GeneralProtection,
    AlignmentCheck,
}

/// An x87 form's behavior: its undefined condition codes, when it records its opcode and data address, what a
/// store that faults part-way writes, and for FXSAVE and FXRSTOR what a misaligned area raises under AC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct X87Behavior {
    pub codes: UndefinedCodes,
    pub opcode: PointerUpdate,
    pub data: PointerUpdate,
    pub across_fault: StoreAcrossFault,
    pub misaligned_area: MisalignedArea,
}

pub const TOP_SHIFT: u16 = 11;
pub const TOP_MASK: u16 = 0b111 << TOP_SHIFT;
pub const INVALID: u16 = 1 << 0;
pub const DENORMAL: u16 = 1 << 1;
pub const STACK_FAULT: u16 = 1 << 6;
pub const SUMMARY: u16 = 1 << 7;
pub const BUSY: u16 = 1 << 15;
pub const EXCEPTION_FLAGS: u16 = 0x3F;
pub const C0: u16 = 1 << 8;
pub const C1: u16 = 1 << 9;
pub const C2: u16 = 1 << 10;
pub const C3: u16 = 1 << 14;
const SIGN: u128 = 1 << 79;
const EXPONENT_MASK: u128 = 0x7FFF << 64;
const INTEGER_BIT: u64 = 1 << 63;
/// The QNaN a masked invalid operation leaves: negative, all exponent bits, the integer bit and the top fraction bit.
pub const INDEFINITE: u128 = 0xFFFF_C000_0000_0000_0000;
/// FNINIT's FCW.
const CONTROL_INITIAL: u16 = 0x037F;

impl X87Form {
    /// Whether the form is a control form, which leaves FIP, FOP and FDP alone.
    pub const fn is_control(&self) -> bool {
        matches!(
            self.operation,
            X87Operation::Wait
                | X87Operation::LegacyNoOperation
                | X87Operation::ClearExceptions
                | X87Operation::Initialize
                | X87Operation::StoreStatus
                | X87Operation::StoreControl
                | X87Operation::LoadControl
                | X87Operation::StoreEnvironment
                | X87Operation::LoadEnvironment
                | X87Operation::SaveState
                | X87Operation::RestoreState
                | X87Operation::SaveExtended { .. }
                | X87Operation::RestoreExtended { .. }
        )
    }

    /// Whether the form raises #MF while an exception is pending: every form but the FN forms and the 8087's no-ops.
    pub const fn waits(&self) -> bool {
        !matches!(
            self.operation,
            X87Operation::LegacyNoOperation
                | X87Operation::ClearExceptions
                | X87Operation::Initialize
                | X87Operation::StoreStatus
                | X87Operation::StoreControl
                | X87Operation::StoreEnvironment
                | X87Operation::SaveState
                | X87Operation::SaveExtended { .. }
                | X87Operation::RestoreExtended { .. }
        )
    }

    /// The condition codes the manual leaves undefined after the form.
    pub const fn list_undefined_codes(&self) -> u16 {
        match self.operation {
            X87Operation::NoOperation
            | X87Operation::Wait
            | X87Operation::LegacyNoOperation
            | X87Operation::Initialize
            | X87Operation::Examine
            | X87Operation::Compare { .. }
            | X87Operation::PartialRemainder { .. }
            | X87Operation::StoreEnvironment
            | X87Operation::LoadEnvironment
            | X87Operation::SaveState
            | X87Operation::RestoreState
            | X87Operation::SaveExtended { .. }
            | X87Operation::RestoreExtended { .. } => 0,
            X87Operation::StoreInteger { .. } => C0 | C2 | C3,
            X87Operation::ClearExceptions
            | X87Operation::StoreStatus
            | X87Operation::StoreControl
            | X87Operation::LoadControl
            | X87Operation::Free { .. } => C0 | C1 | C2 | C3,
            _ => C0 | C2 | C3,
        }
    }

    /// Whether the form reads or writes memory.
    pub fn has_memory_operand(&self) -> bool {
        self.place != OperandPlace::Register
    }

    /// Whether the form writes its memory operand.
    pub fn writes_memory(&self) -> bool {
        self.has_memory_operand()
            && matches!(
                self.operation,
                X87Operation::StoreStatus
                    | X87Operation::StoreControl
                    | X87Operation::StoreExtended
                    | X87Operation::StoreInteger { .. }
                    | X87Operation::StoreFloat { .. }
                    | X87Operation::StoreDecimal
                    | X87Operation::StoreEnvironment
                    | X87Operation::SaveState
                    | X87Operation::SaveExtended { .. }
            )
    }

    /// The bytes of the memory operand.
    pub fn memory_bytes(&self) -> u64 {
        match self.code {
            Code::Fld_m80fp | Code::Fstp_m80fp | Code::Fbld_m80bcd | Code::Fbstp_m80bcd => 10,
            Code::Fldenv_m14byte | Code::Fnstenv_m14byte => 14,
            Code::Fldenv_m28byte | Code::Fnstenv_m28byte => 28,
            Code::Fnsave_m94byte | Code::Frstor_m94byte => 94,
            Code::Fnsave_m108byte | Code::Frstor_m108byte => 108,
            Code::Fxsave_m512byte | Code::Fxsave64_m512byte | Code::Fxrstor_m512byte | Code::Fxrstor64_m512byte => 512,
            Code::Fild_m64int | Code::Fld_m64fp | Code::Fcom_m64fp | Code::Fcomp_m64fp | Code::Fistp_m64int => 8,
            Code::Fild_m32int | Code::Fld_m32fp | Code::Fcom_m32fp | Code::Fcomp_m32fp | Code::Ficom_m32int
            | Code::Ficomp_m32int | Code::Fist_m32int | Code::Fistp_m32int => 4,
            Code::Fadd_m64fp | Code::Fsub_m64fp | Code::Fsubr_m64fp | Code::Fmul_m64fp | Code::Fdiv_m64fp | Code::Fdivr_m64fp
            | Code::Fst_m64fp | Code::Fstp_m64fp => 8,
            Code::Fadd_m32fp | Code::Fsub_m32fp | Code::Fsubr_m32fp | Code::Fmul_m32fp | Code::Fdiv_m32fp | Code::Fdivr_m32fp
            | Code::Fst_m32fp | Code::Fstp_m32fp | Code::Fiadd_m32int | Code::Fisub_m32int | Code::Fisubr_m32int
            | Code::Fimul_m32int | Code::Fidiv_m32int | Code::Fidivr_m32int => 4,
            _ => 2,
        }
    }
}

impl X87Form {
    /// The index of the register a form that names none compares with: ST(1) for FCOMPP and FUCOMPP.
    pub const fn to_implied_index(&self) -> usize {
        match self.code {
            Code::Fcompp | Code::Fucompp => 1,
            _ => 0,
        }
    }
}

/// The class of an extended value, as the arithmetic sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Zero,
    /// A denormal or a pseudo-denormal.
    Denormal,
    Normal,
    Infinity,
    QuietNaN,
    SignalingNaN,
    /// An unnormal, a pseudo-infinity or a pseudo-NaN, which no operation takes.
    Unsupported,
}

const QUIET: u64 = 1 << 62;

pub fn classify(value: u128) -> Class {
    let exponent = (value & EXPONENT_MASK) >> 64;
    let significand = value as u64;
    let integer = significand & INTEGER_BIT != 0;
    match exponent {
        0 if significand == 0 => Class::Zero,
        0 => Class::Denormal,
        0x7FFF if !integer => Class::Unsupported,
        0x7FFF if significand == INTEGER_BIT => Class::Infinity,
        0x7FFF if significand & QUIET != 0 => Class::QuietNaN,
        0x7FFF => Class::SignalingNaN,
        _ if !integer => Class::Unsupported,
        _ => Class::Normal,
    }
}

/// The magnitude's order key: the exponent, a denormal's taken as 1, then the significand.
fn to_key(value: u128) -> (u128, u64) {
    (((value & EXPONENT_MASK) >> 64).max(1), value as u64)
}

/// The order of two values neither a NaN nor unsupported; the two zeros are equal.
pub fn compare_values(first: u128, second: u128) -> core::cmp::Ordering {
    use core::cmp::Ordering as Order;
    let zero = |value: u128| classify(value) == Class::Zero;
    if zero(first) && zero(second) {
        return Order::Equal;
    }
    let negative = |value: u128| value & SIGN != 0;
    match (negative(first), negative(second)) {
        (false, true) => Order::Greater,
        (true, false) => Order::Less,
        (false, false) => to_key(first).cmp(&to_key(second)),
        (true, true) => to_key(second).cmp(&to_key(first)),
    }
}

/// A single or double value widened to extended exactly, and the exception it raises: IE for an SNaN, quieted; DE
/// for a denormal.
pub fn widen(bits: u64, fraction_bits: u32, exponent_bits: u32) -> (u128, u16) {
    let sign = u128::from(bits >> (fraction_bits + exponent_bits) & 1) << 79;
    let exponent_mask = (1 << exponent_bits) - 1;
    let exponent = bits >> fraction_bits & exponent_mask;
    let fraction = bits & ((1 << fraction_bits) - 1);
    let bias = (1 << (exponent_bits - 1)) - 1;
    let shift = 63 - fraction_bits;
    if exponent == exponent_mask {
        let quiet = fraction >> (fraction_bits - 1) & 1 != 0;
        let significand = INTEGER_BIT | fraction << shift | if fraction != 0 { QUIET } else { 0 };
        let raised = if fraction != 0 && !quiet { INVALID } else { 0 };
        return (sign | EXPONENT_MASK | u128::from(significand), raised);
    }
    if exponent == 0 && fraction == 0 {
        return (sign, 0);
    }
    if exponent == 0 {
        let leading = fraction.leading_zeros() - (64 - fraction_bits);
        let significand = fraction << (shift + leading + 1);
        let biased = u128::from(16383 - bias + 1) - u128::from(leading + 1);
        return (sign | biased << 64 | u128::from(significand), DENORMAL);
    }
    let biased = u128::from(exponent + 16383 - bias);
    (sign | biased << 64 | u128::from(INTEGER_BIT | fraction << shift), 0)
}

/// FCW's rounding control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    Nearest,
    Down,
    Up,
    TowardZero,
}

impl Rounding {
    pub const fn from_control(control: u16) -> Self {
        match control >> 10 & 3 {
            0 => Self::Nearest,
            1 => Self::Down,
            2 => Self::Up,
            _ => Self::TowardZero,
        }
    }
}

/// A finite value rounded to an integer of `bits` bits: the integer, whether it is inexact, and whether its
/// magnitude was rounded up; `None` where it does not fit.
pub fn round_to_integer(value: u128, rounding: Rounding, bits: u32) -> Option<(i64, bool, bool)> {
    let negative = value & SIGN != 0;
    let exponent = i64::try_from(((value & EXPONENT_MASK) >> 64).max(1)).unwrap_or(0) - 16383 - 63;
    let significand = value as u64;
    let (whole, remainder, half): (u128, u128, u128) = if exponent >= 0 {
        if exponent > 64 {
            return (significand == 0).then_some((0, false, false));
        }
        (u128::from(significand) << exponent, 0, 1)
    } else if exponent <= -128 {
        (0, u128::from(significand), u128::MAX)
    } else {
        let shift = (-exponent) as u32;
        let all = u128::from(significand);
        (all >> shift, all & ((1 << shift) - 1), 1 << (shift - 1))
    };
    let inexact = remainder != 0;
    let up = match rounding {
        Rounding::Nearest if is_active(Bug::RoundHalfAwayFromZero) => remainder >= half && half != u128::MAX && inexact,
        Rounding::Nearest => remainder > half || remainder == half && half != u128::MAX && whole & 1 != 0 && inexact,
        Rounding::Down => negative && inexact,
        Rounding::Up => !negative && inexact,
        Rounding::TowardZero => false,
    };
    let magnitude = whole + u128::from(up);
    let limit = 1u128 << (bits - 1);
    if magnitude > limit || magnitude == limit && !negative {
        return None;
    }
    let signed = if negative { -(magnitude as i128) } else { magnitude as i128 };
    Some((signed as i64, inexact, up))
}

/// The row of an x87 form, or `None` for any other form.
pub fn find_x87_form(instruction: Instruction) -> Option<&'static X87Form> {
    X87_FORMS.iter().find(|form| form.instruction == instruction)
}

const RULES: [CodeRule; 2] = [CodeRule::Unchanged, CodeRule::Cleared];
const POINTERS: [PointerUpdate; 2] = [PointerUpdate::Always, PointerUpdate::OnUnmaskedException];
const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// Every combination of the rules the form leaves open: its undefined condition codes, FOP for a form that is not
/// a control form, FDP for one with a memory operand too, and what a store to memory writes when it faults part-way.
pub(crate) fn list_x87_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_x87_form(instruction) else {
        return Vec::new();
    };
    let undefined = form.list_undefined_codes();
    let c1: &[CodeRule] = if undefined & C1 != 0 { &RULES } else { &RULES[..1] };
    let others: &[CodeRule] = if undefined & !C1 != 0 { &RULES } else { &RULES[..1] };
    let codes: Vec<UndefinedCodes> =
        c1.iter().flat_map(|&c1| others.iter().map(move |&others| UndefinedCodes { c1, others })).collect();
    let opcode: &[PointerUpdate] = if form.is_control() { &POINTERS[..1] } else { &POINTERS };
    let data: &[PointerUpdate] = if form.is_control() || !form.has_memory_operand() { &POINTERS[..1] } else { &POINTERS };
    // An environment, a save area or FXSAVE's area is checked whole before any of it is written.
    let image = matches!(form.operation, X87Operation::StoreEnvironment | X87Operation::SaveState | X87Operation::SaveExtended { .. });
    let across: &[StoreAcrossFault] = if form.writes_memory() && !image { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    let area: &[MisalignedArea] = if matches!(form.operation, X87Operation::SaveExtended { .. } | X87Operation::RestoreExtended { .. }) {
        &[MisalignedArea::GeneralProtection, MisalignedArea::AlignmentCheck]
    } else {
        &[MisalignedArea::GeneralProtection]
    };
    let mut behaviors = Vec::new();
    for &across_fault in across {
        for &codes in &codes {
            for &opcode in opcode {
                for &data in data {
                    for &misaligned_area in area {
                        let behavior = X87Behavior { codes, opcode, data, across_fault, misaligned_area };
                        behaviors.push(PreciseInstruction::X87 { instruction, behavior });
                    }
                }
            }
        }
    }
    behaviors
}

/// The x87 registers as a stack.
impl X87 {
    pub const fn top(&self) -> usize {
        ((self.status & TOP_MASK) >> TOP_SHIFT) as usize
    }

    pub const fn to_physical(&self, index: usize) -> usize {
        (self.top() + index) % 8
    }

    pub const fn is_empty(&self, index: usize) -> bool {
        self.tags & (1 << self.to_physical(index)) == 0
    }

    pub const fn read(&self, index: usize) -> u128 {
        self.registers[self.to_physical(index)]
    }

    /// Writes ST(i) and marks it valid.
    pub fn write(&mut self, index: usize, value: u128) {
        let physical = self.to_physical(index);
        self.registers[physical] = value;
        self.tags |= 1 << physical;
    }

    pub fn free(&mut self, index: usize) {
        self.tags &= !(1 << self.to_physical(index));
    }

    pub fn set_top(&mut self, top: usize) {
        self.status = (self.status & !TOP_MASK) | ((top as u16 % 8) << TOP_SHIFT);
    }

    /// Pops: ST(0) marked empty, TOP one up.
    pub fn pop(&mut self) {
        self.free(0);
        self.set_top(self.top() + 1);
    }

    /// Whether the flags would raise an unmasked exception.
    pub const fn is_unmasked(&self, flags: u16) -> bool {
        flags & !self.control & EXCEPTION_FLAGS != 0
    }

    /// Sets the exception flags, and ES and B where one is unmasked.
    pub fn raise(&mut self, flags: u16) {
        self.status |= flags;
        if self.status & !self.control & EXCEPTION_FLAGS != 0 {
            self.status |= SUMMARY | BUSY;
        }
    }

    pub fn set_code(&mut self, code: u16, value: bool) {
        if value { self.status |= code } else { self.status &= !code }
    }
}

/// The class FXAM gives, as C3, C2 and C0.
pub fn examine(value: u128, empty: bool) -> u16 {
    let exponent = (value & EXPONENT_MASK) >> 64;
    let significand = value as u64;
    let fraction = significand & !INTEGER_BIT;
    let integer = significand & INTEGER_BIT != 0;
    let (c3, c2, c0) = match () {
        () if empty => (true, false, true),
        () if exponent == 0 && significand == 0 => (true, false, false),
        () if exponent == 0 => (true, true, false),
        () if exponent == 0x7FFF && !integer => (false, false, false),
        () if exponent == 0x7FFF && fraction == 0 => (false, true, true),
        () if exponent == 0x7FFF => (false, false, true),
        () if !integer => (false, false, false),
        () => (false, true, false),
    };
    (u16::from(c3) * C3) | (u16::from(c2) * C2) | (u16::from(c0) * C0)
}

/// The extended value of a signed integer, exactly.
pub fn convert_integer(value: i64) -> u128 {
    if value == 0 {
        return 0;
    }
    let sign = if value < 0 { SIGN } else { 0 };
    let magnitude = value.unsigned_abs();
    let shift = magnitude.leading_zeros();
    let exponent = u128::from(16383 + 63 - shift);
    sign | exponent << 64 | u128::from(magnitude << shift)
}

/// FNINIT's state, the registers kept.
pub fn initialize(x87: &mut X87) {
    let registers = x87.registers;
    *x87 = X87 { registers, ..crate::X87_INITIAL };
    x87.control = CONTROL_INITIAL;
}

use OperandPlace::{Memory, Register};
use Ordering::{Ordered, Unordered};
use X87Operation::{
    Absolute, ChangeSign, ClearExceptions, DecrementTop, Examine, Exchange, Free, IncrementTop, Initialize,
    Compare, LegacyNoOperation, LoadConstant, LoadControl, LoadExtended, LoadFloat, LoadInteger, LoadRegister, MoveIf,
    NoOperation, StoreInteger,
    StoreControl, StoreExtended, StoreRegister, StoreRegisterUnchecked, StoreStatus, Wait,
};

const fn x87(instruction: Instruction, operation: X87Operation, place: OperandPlace, code: Code) -> X87Form {
    X87Form { instruction, operation, place, code }
}

/// +1.0.
const ONE: u128 = 0x3FFF_8000_0000_0000_0000;

/// Every x87 form modeled.
pub static X87_FORMS: &[X87Form] = &[
    x87(Instruction::Fnop, NoOperation, Register, Code::Fnop),
    x87(Instruction::Fwait, Wait, Register, Code::Wait),
    x87(Instruction::Feni8087Nop, LegacyNoOperation, Register, Code::Fneni),
    x87(Instruction::Fdisi8087Nop, LegacyNoOperation, Register, Code::Fndisi),
    x87(Instruction::Fsetpm287Nop, LegacyNoOperation, Register, Code::Fnsetpm),
    x87(Instruction::Fnclex, ClearExceptions, Register, Code::Fnclex),
    x87(Instruction::Fninit, Initialize, Register, Code::Fninit),
    x87(Instruction::FnstswAx, StoreStatus, Register, Code::Fnstsw_AX),
    x87(Instruction::FnstswMemmem16, StoreStatus, Memory, Code::Fnstsw_m2byte),
    x87(Instruction::FnstcwMemmem16, StoreControl, Memory, Code::Fnstcw_m2byte),
    x87(Instruction::FldcwMemmem16, LoadControl, Memory, Code::Fldcw_m2byte),
    x87(Instruction::Fincstp, IncrementTop, Register, Code::Fincstp),
    x87(Instruction::Fdecstp, DecrementTop, Register, Code::Fdecstp),
    x87(Instruction::FfreeX87, Free { pop: false }, Register, Code::Ffree_sti),
    x87(Instruction::FfreepX87, Free { pop: true }, Register, Code::Ffreep_sti),
    x87(Instruction::FxchSt0X87, Exchange, Register, Code::Fxch_st0_sti),
    x87(Instruction::FxchSt0X87Ddc1, Exchange, Register, Code::Fxch_st0_sti_DDC8),
    x87(Instruction::FxchSt0X87Dfc1, Exchange, Register, Code::Fxch_st0_sti_DFC8),
    x87(Instruction::FldSt0X87, LoadRegister, Register, Code::Fld_sti),
    x87(Instruction::FstX87St0, StoreRegister { pop: false }, Register, Code::Fst_sti),
    x87(Instruction::FstpX87St0, StoreRegister { pop: true }, Register, Code::Fstp_sti),
    x87(Instruction::FstpX87St0Dfd0, StoreRegister { pop: true }, Register, Code::Fstp_sti_DFD0),
    x87(Instruction::FstpX87St0Dfd1, StoreRegister { pop: true }, Register, Code::Fstp_sti_DFD8),
    x87(Instruction::FstpnceX87St0, StoreRegisterUnchecked, Register, Code::Fstpnce_sti),
    x87(Instruction::Fchs, ChangeSign, Register, Code::Fchs),
    x87(Instruction::Fabs, Absolute, Register, Code::Fabs),
    x87(Instruction::Fldz, LoadConstant(0), Register, Code::Fldz),
    x87(Instruction::Fld1, LoadConstant(ONE), Register, Code::Fld1),
    x87(Instruction::FcmovbSt0X87, MoveIf(Condition::Below), Register, Code::Fcmovb_st0_sti),
    x87(Instruction::FcmoveSt0X87, MoveIf(Condition::Zero), Register, Code::Fcmove_st0_sti),
    x87(Instruction::FcmovbeSt0X87, MoveIf(Condition::BelowOrEqual), Register, Code::Fcmovbe_st0_sti),
    x87(Instruction::FcmovuSt0X87, MoveIf(Condition::Parity), Register, Code::Fcmovu_st0_sti),
    x87(Instruction::FcmovnbSt0X87, MoveIf(Condition::NotBelow), Register, Code::Fcmovnb_st0_sti),
    x87(Instruction::FcmovneSt0X87, MoveIf(Condition::NotZero), Register, Code::Fcmovne_st0_sti),
    x87(Instruction::FcmovnbeSt0X87, MoveIf(Condition::NotBelowOrEqual), Register, Code::Fcmovnbe_st0_sti),
    x87(Instruction::FcmovnuSt0X87, MoveIf(Condition::NotParity), Register, Code::Fcmovnu_st0_sti),
    x87(Instruction::Fxam, Examine, Register, Code::Fxam),
    x87(Instruction::FldSt0Memmem80real, LoadExtended, Memory, Code::Fld_m80fp),
    x87(Instruction::FstpMemmem80realSt0, StoreExtended, Memory, Code::Fstp_m80fp),
    x87(Instruction::FildSt0Memmem16int, LoadInteger, Memory, Code::Fild_m16int),
    x87(Instruction::FildSt0Memmem32int, LoadInteger, Memory, Code::Fild_m32int),
    x87(Instruction::FildSt0Memm64int, LoadInteger, Memory, Code::Fild_m64int),
    x87(Instruction::FldSt0Memmem32real, LoadFloat, Memory, Code::Fld_m32fp),
    x87(Instruction::FldSt0Memm64real, LoadFloat, Memory, Code::Fld_m64fp),
    x87(Instruction::FcomSt0X87, compare(Ordered, Source::Register, 0, false), Register, Code::Fcom_st0_sti),
    x87(Instruction::FcomSt0X87Dcd0, compare(Ordered, Source::Register, 0, false), Register, Code::Fcom_st0_sti_DCD0),
    x87(Instruction::FcompSt0X87, compare(Ordered, Source::Register, 1, false), Register, Code::Fcomp_st0_sti),
    x87(Instruction::FcompSt0X87Dcd1, compare(Ordered, Source::Register, 1, false), Register, Code::Fcomp_st0_sti_DCD8),
    x87(Instruction::FcompSt0X87Ded0, compare(Ordered, Source::Register, 1, false), Register, Code::Fcomp_st0_sti_DED0),
    x87(Instruction::Fcompp, compare(Ordered, Source::Register, 2, false), Register, Code::Fcompp),
    x87(Instruction::FucomSt0X87, compare(Unordered, Source::Register, 0, false), Register, Code::Fucom_st0_sti),
    x87(Instruction::FucompSt0X87, compare(Unordered, Source::Register, 1, false), Register, Code::Fucomp_st0_sti),
    x87(Instruction::Fucompp, compare(Unordered, Source::Register, 2, false), Register, Code::Fucompp),
    x87(Instruction::FcomiSt0X87, compare(Ordered, Source::Register, 0, true), Register, Code::Fcomi_st0_sti),
    x87(Instruction::FcomipSt0X87, compare(Ordered, Source::Register, 1, true), Register, Code::Fcomip_st0_sti),
    x87(Instruction::FucomiSt0X87, compare(Unordered, Source::Register, 0, true), Register, Code::Fucomi_st0_sti),
    x87(Instruction::FucomipSt0X87, compare(Unordered, Source::Register, 1, true), Register, Code::Fucomip_st0_sti),
    x87(Instruction::Ftst, compare(Ordered, Source::Zero, 0, false), Register, Code::Ftst),
    x87(Instruction::FcomSt0Memmem32real, compare(Ordered, Source::Single, 0, false), Memory, Code::Fcom_m32fp),
    x87(Instruction::FcomSt0Memm64real, compare(Ordered, Source::Double, 0, false), Memory, Code::Fcom_m64fp),
    x87(Instruction::FcompSt0Memmem32real, compare(Ordered, Source::Single, 1, false), Memory, Code::Fcomp_m32fp),
    x87(Instruction::FcompSt0Memm64real, compare(Ordered, Source::Double, 1, false), Memory, Code::Fcomp_m64fp),
    x87(Instruction::FicomSt0Memmem16int, compare(Ordered, Source::Integer, 0, false), Memory, Code::Ficom_m16int),
    x87(Instruction::FicomSt0Memmem32int, compare(Ordered, Source::Integer, 0, false), Memory, Code::Ficom_m32int),
    x87(Instruction::FicompSt0Memmem16int, compare(Ordered, Source::Integer, 1, false), Memory, Code::Ficomp_m16int),
    x87(Instruction::FicompSt0Memmem32int, compare(Ordered, Source::Integer, 1, false), Memory, Code::Ficomp_m32int),
    x87(Instruction::FistMemmem16intSt0, StoreInteger { pop: false }, Memory, Code::Fist_m16int),
    x87(Instruction::FistMemmem32intSt0, StoreInteger { pop: false }, Memory, Code::Fist_m32int),
    x87(Instruction::FistpMemmem16intSt0, StoreInteger { pop: true }, Memory, Code::Fistp_m16int),
    x87(Instruction::FistpMemmem32intSt0, StoreInteger { pop: true }, Memory, Code::Fistp_m32int),
    x87(Instruction::FistpMemm64intSt0, StoreInteger { pop: true }, Memory, Code::Fistp_m64int),
    arithmetic(Instruction::FaddSt0X87, Add, Source::Register, false, false, Register, Code::Fadd_st0_sti),
    arithmetic(Instruction::FaddX87St0, Add, Source::Register, false, false, Register, Code::Fadd_sti_st0),
    arithmetic(Instruction::FaddpX87St0, Add, Source::Register, false, true, Register, Code::Faddp_sti_st0),
    arithmetic(Instruction::FaddMemmem32real, Add, Source::Single, false, false, Memory, Code::Fadd_m32fp),
    arithmetic(Instruction::FaddMemm64real, Add, Source::Double, false, false, Memory, Code::Fadd_m64fp),
    arithmetic(Instruction::FiaddSt0Memmem16int, Add, Source::Integer, false, false, Memory, Code::Fiadd_m16int),
    arithmetic(Instruction::FiaddSt0Memmem32int, Add, Source::Integer, false, false, Memory, Code::Fiadd_m32int),
    arithmetic(Instruction::FsubSt0X87, Subtract, Source::Register, false, false, Register, Code::Fsub_st0_sti),
    arithmetic(Instruction::FsubX87St0, Subtract, Source::Register, false, false, Register, Code::Fsub_sti_st0),
    arithmetic(Instruction::FsubpX87St0, Subtract, Source::Register, false, true, Register, Code::Fsubp_sti_st0),
    arithmetic(Instruction::FsubSt0Memmem32real, Subtract, Source::Single, false, false, Memory, Code::Fsub_m32fp),
    arithmetic(Instruction::FsubSt0Memm64real, Subtract, Source::Double, false, false, Memory, Code::Fsub_m64fp),
    arithmetic(Instruction::FisubSt0Memmem16int, Subtract, Source::Integer, false, false, Memory, Code::Fisub_m16int),
    arithmetic(Instruction::FisubSt0Memmem32int, Subtract, Source::Integer, false, false, Memory, Code::Fisub_m32int),
    arithmetic(Instruction::FsubrSt0X87, Subtract, Source::Register, true, false, Register, Code::Fsubr_st0_sti),
    arithmetic(Instruction::FsubrX87St0, Subtract, Source::Register, true, false, Register, Code::Fsubr_sti_st0),
    arithmetic(Instruction::FsubrpX87St0, Subtract, Source::Register, true, true, Register, Code::Fsubrp_sti_st0),
    arithmetic(Instruction::FsubrSt0Memmem32real, Subtract, Source::Single, true, false, Memory, Code::Fsubr_m32fp),
    arithmetic(Instruction::FsubrSt0Memm64real, Subtract, Source::Double, true, false, Memory, Code::Fsubr_m64fp),
    arithmetic(Instruction::FisubrSt0Memmem16int, Subtract, Source::Integer, true, false, Memory, Code::Fisubr_m16int),
    arithmetic(Instruction::FisubrSt0Memmem32int, Subtract, Source::Integer, true, false, Memory, Code::Fisubr_m32int),
    arithmetic(Instruction::FmulSt0X87, Multiply, Source::Register, false, false, Register, Code::Fmul_st0_sti),
    arithmetic(Instruction::FmulX87St0, Multiply, Source::Register, false, false, Register, Code::Fmul_sti_st0),
    arithmetic(Instruction::FmulpX87St0, Multiply, Source::Register, false, true, Register, Code::Fmulp_sti_st0),
    arithmetic(Instruction::FmulSt0Memmem32real, Multiply, Source::Single, false, false, Memory, Code::Fmul_m32fp),
    arithmetic(Instruction::FmulSt0Memm64real, Multiply, Source::Double, false, false, Memory, Code::Fmul_m64fp),
    arithmetic(Instruction::FimulSt0Memmem16int, Multiply, Source::Integer, false, false, Memory, Code::Fimul_m16int),
    arithmetic(Instruction::FimulSt0Memmem32int, Multiply, Source::Integer, false, false, Memory, Code::Fimul_m32int),
    arithmetic(Instruction::FdivSt0X87, Divide, Source::Register, false, false, Register, Code::Fdiv_st0_sti),
    arithmetic(Instruction::FdivX87St0, Divide, Source::Register, false, false, Register, Code::Fdiv_sti_st0),
    arithmetic(Instruction::FdivpX87St0, Divide, Source::Register, false, true, Register, Code::Fdivp_sti_st0),
    arithmetic(Instruction::FdivSt0Memmem32real, Divide, Source::Single, false, false, Memory, Code::Fdiv_m32fp),
    arithmetic(Instruction::FdivSt0Memm64real, Divide, Source::Double, false, false, Memory, Code::Fdiv_m64fp),
    arithmetic(Instruction::FidivSt0Memmem16int, Divide, Source::Integer, false, false, Memory, Code::Fidiv_m16int),
    arithmetic(Instruction::FidivSt0Memmem32int, Divide, Source::Integer, false, false, Memory, Code::Fidiv_m32int),
    arithmetic(Instruction::FdivrSt0X87, Divide, Source::Register, true, false, Register, Code::Fdivr_st0_sti),
    arithmetic(Instruction::FdivrX87St0, Divide, Source::Register, true, false, Register, Code::Fdivr_sti_st0),
    arithmetic(Instruction::FdivrpX87St0, Divide, Source::Register, true, true, Register, Code::Fdivrp_sti_st0),
    arithmetic(Instruction::FdivrSt0Memmem32real, Divide, Source::Single, true, false, Memory, Code::Fdivr_m32fp),
    arithmetic(Instruction::FdivrSt0Memm64real, Divide, Source::Double, true, false, Memory, Code::Fdivr_m64fp),
    arithmetic(Instruction::FidivrSt0Memmem16int, Divide, Source::Integer, true, false, Memory, Code::Fidivr_m16int),
    arithmetic(Instruction::FidivrSt0Memmem32int, Divide, Source::Integer, true, false, Memory, Code::Fidivr_m32int),
    x87(Instruction::FstMemmem32realSt0, X87Operation::StoreFloat { pop: false }, Memory, Code::Fst_m32fp),
    x87(Instruction::FstMemm64realSt0, X87Operation::StoreFloat { pop: false }, Memory, Code::Fst_m64fp),
    x87(Instruction::FstpMemmem32realSt0, X87Operation::StoreFloat { pop: true }, Memory, Code::Fstp_m32fp),
    x87(Instruction::FstpMemm64realSt0, X87Operation::StoreFloat { pop: true }, Memory, Code::Fstp_m64fp),
    constant(Instruction::Fldpi, 0x4000_C90F_DAA2_2168_C234, true, Code::Fldpi),
    constant(Instruction::Fldl2e, 0x3FFF_B8AA_3B29_5C17_F0BB, true, Code::Fldl2e),
    constant(Instruction::Fldl2t, 0x4000_D49A_784B_CD1B_8AFE, false, Code::Fldl2t),
    constant(Instruction::Fldlg2, 0x3FFD_9A20_9A84_FBCF_F798, true, Code::Fldlg2),
    constant(Instruction::Fldln2, 0x3FFE_B172_17F7_D1CF_79AB, true, Code::Fldln2),
    x87(Instruction::Fsqrt, X87Operation::SquareRoot, Register, Code::Fsqrt),
    x87(Instruction::Frndint, X87Operation::RoundToIntegral, Register, Code::Frndint),
    x87(Instruction::Fscale, X87Operation::Scale, Register, Code::Fscale),
    x87(Instruction::Fxtract, X87Operation::Extract, Register, Code::Fxtract),
    x87(Instruction::Fprem, X87Operation::PartialRemainder { nearest: false }, Register, Code::Fprem),
    x87(Instruction::Fprem1, X87Operation::PartialRemainder { nearest: true }, Register, Code::Fprem1),
    x87(Instruction::FbldSt0Memmem80dec, X87Operation::LoadDecimal, Memory, Code::Fbld_m80bcd),
    x87(Instruction::FbstpMemmem80decSt0, X87Operation::StoreDecimal, Memory, Code::Fbstp_m80bcd),
    x87(Instruction::FnstenvMemmem14, X87Operation::StoreEnvironment, Memory, Code::Fnstenv_m14byte),
    x87(Instruction::FnstenvMemmem28, X87Operation::StoreEnvironment, Memory, Code::Fnstenv_m28byte),
    x87(Instruction::FldenvMemmem14, X87Operation::LoadEnvironment, Memory, Code::Fldenv_m14byte),
    x87(Instruction::FldenvMemmem28, X87Operation::LoadEnvironment, Memory, Code::Fldenv_m28byte),
    x87(Instruction::FnsaveMemmem94, X87Operation::SaveState, Memory, Code::Fnsave_m94byte),
    x87(Instruction::FnsaveMemmem108, X87Operation::SaveState, Memory, Code::Fnsave_m108byte),
    x87(Instruction::FrstorMemmem94, X87Operation::RestoreState, Memory, Code::Frstor_m94byte),
    x87(Instruction::FrstorMemmem108, X87Operation::RestoreState, Memory, Code::Frstor_m108byte),
    x87(Instruction::FxsaveMemmfpxenv, X87Operation::SaveExtended { wide: false }, Memory, Code::Fxsave_m512byte),
    x87(Instruction::Fxsave64Memmfpxenv, X87Operation::SaveExtended { wide: true }, Memory, Code::Fxsave64_m512byte),
    x87(Instruction::FxrstorMemmfpxenv, X87Operation::RestoreExtended { wide: false }, Memory, Code::Fxrstor_m512byte),
    x87(Instruction::Fxrstor64Memmfpxenv, X87Operation::RestoreExtended { wide: true }, Memory, Code::Fxrstor64_m512byte),
];

const fn arithmetic(
    instruction: Instruction,
    operation: crate::x87_arithmetic::Arithmetic,
    source: Source,
    reverse: bool,
    pop: bool,
    place: OperandPlace,
    code: Code,
) -> X87Form {
    X87Form { instruction, operation: X87Operation::Arithmetic { operation, source, reverse, pop }, place, code }
}

/// A constant's form: its significand cut to 64 bits, and whether the bits beyond lie above a half.
const fn constant(instruction: Instruction, truncated: u128, above_half: bool, code: Code) -> X87Form {
    X87Form { instruction, operation: X87Operation::LoadRoundedConstant { truncated, above_half }, place: OperandPlace::Register, code }
}

use crate::x87_arithmetic::Arithmetic::{Add, Divide, Multiply, Subtract};

const fn compare(ordering: Ordering, source: Source, pops: u8, to_flags: bool) -> X87Operation {
    Compare { ordering, source, pops, to_flags }
}
