//! Our own instruction type. Meanings are written against it, never against the decoder's types, so the
//! decoder can be replaced.

/// Operand width of an integer instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Width {
    W32,
    X64,
}

impl Width {
    pub fn bits(self) -> u32 {
        match self {
            Width::W32 => 32,
            Width::X64 => 64,
        }
    }
}

/// A general register operand, with register number 31 already resolved to what the encoding means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    /// x0 to x30.
    X(u8),
    Zero,
    Sp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

/// How a register operand is widened before use: unsigned or signed, from 8, 16, 32 or 64 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extend {
    pub signed: bool,
    pub from_bits: u8,
}

/// The second source of an arithmetic or logical instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    /// Already shifted into place.
    Imm(u64),
    Shifted {
        rm: Reg,
        shift: Shift,
        amount: u8,
    },
    Extended {
        rm: Reg,
        extend: Extend,
        amount: u8,
    },
}

/// An ARM64 condition code, 0 to 15 (eq, ne, cs, cc, mi, pl, vs, vc, hi, ls, ge, lt, gt, le, al, nv).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cond(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logic {
    And,
    Or,
    Xor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveWide {
    /// movz
    Zero,
    /// movn
    Not,
    /// movk
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bitfield {
    /// sbfm
    Signed,
    /// bfm
    Insert,
    /// ubfm
    Unsigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// csel
    Plain,
    /// csinc
    Increment,
    /// csinv
    Invert,
    /// csneg
    Negate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Rbit,
    Rev16,
    Rev32,
    Rev,
    Clz,
    Cls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
    Udiv,
    Sdiv,
    Lslv,
    Lsrv,
    Asrv,
    Rorv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulAdd {
    Madd,
    Msub,
    Smaddl,
    Smsubl,
    Umaddl,
    Umsubl,
    Smulh,
    Umulh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchRegister {
    Br,
    Blr,
    Ret,
}

/// Where a load or store goes. Offsets are in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    /// base + offset
    Offset { base: Reg, offset: i64 },
    /// base += offset, then use base
    PreIndex { base: Reg, offset: i64 },
    /// use base, then base += offset
    PostIndex { base: Reg, offset: i64 },
    /// base + (extend(index) << shift)
    Indexed {
        base: Reg,
        index: Reg,
        extend: Extend,
        shift: u8,
    },
    /// this instruction's address + offset
    Literal { offset: i64 },
}

/// The shape of an AdvSIMD operand: the element size in bytes (1, 2, 4 or 8), and whether the whole 128
/// bits are used. When not, only the low 64 bits are read and the top half of a result is cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrangement {
    pub element: u8,
    pub whole: bool,
}

/// An AdvSIMD data processing operation. Each works on elements of the instruction's `element` size, the
/// size Arm's `size` field names: for long operations that is the narrow source, for narrowing ones the
/// narrow result. `Signed` and `Unsigned` pairs are one variant with a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimdOp {
    // Integer, every element of `rn` with the same element of `rm`.
    And,
    /// bic
    AndNot,
    Or,
    /// orn
    OrNot,
    Xor,
    /// bsl: the destination selects, bit by bit, from `rn` (set) or `rm` (clear).
    Select,
    /// bit: `rm` selects where `rn` is inserted into the destination.
    InsertIfTrue,
    /// bif: the inverse of `rm` selects where `rn` is inserted.
    InsertIfFalse,
    Add,
    Sub,
    Mul,
    /// mla and mls: the destination plus or minus the product.
    MulAdd,
    MulSub,
    /// pmul: carry-less multiply of bytes.
    PolyMul,
    /// The compares give all ones where they hold, zero where not.
    Equal,
    Greater {
        signed: bool,
    },
    GreaterOrEqual {
        signed: bool,
    },
    /// cmtst: the two have a set bit in common.
    Test,
    /// shadd, srhadd and shsub: half the sum or difference, rounded up when `rounding`.
    HalvingAdd {
        signed: bool,
        rounding: bool,
    },
    HalvingSub {
        signed: bool,
    },
    SaturatingAdd {
        signed: bool,
    },
    SaturatingSub {
        signed: bool,
    },
    /// sshl, srshl, sqshl, sqrshl and the unsigned ones: shift by the signed low byte of `rm`, right when
    /// negative.
    ShiftByRegister {
        signed: bool,
        rounding: bool,
        saturating: bool,
    },
    Max {
        signed: bool,
    },
    Min {
        signed: bool,
    },
    AbsDiff {
        signed: bool,
    },
    /// saba: the destination plus the absolute difference.
    AbsDiffAccumulate {
        signed: bool,
    },
    /// The pairwise operations work on the elements of `rn` then `rm` taken two by two.
    AddPairwise,
    MaxPairwise {
        signed: bool,
    },
    MinPairwise {
        signed: bool,
    },
    /// sqdmulh and sqrdmulh: the high half of twice the product, saturated.
    DoublingMulHigh {
        rounding: bool,
    },
    // Integer, on one register.
    /// rev16, rev32, rev64: reverse the elements in each group of this many bytes.
    Reverse {
        group: u8,
    },
    /// saddlp, sadalp: add neighbouring pairs into elements twice as wide, adding to the destination when
    /// `accumulate`.
    AddLongPairwise {
        signed: bool,
        accumulate: bool,
    },
    /// suqadd (signed) and usqadd: add `rn`, of the other signedness, to the destination, saturating.
    SaturatingAccumulate {
        signed: bool,
    },
    CountLeadingSign,
    CountLeadingZeros,
    CountBits,
    Not,
    ReverseBits,
    SaturatingAbs,
    SaturatingNegate,
    Abs,
    Negate,
    /// cmgt, cmge, cmeq, cmle and cmlt with zero, by condition code.
    CompareZero(Cond),
    /// xtn
    Narrow,
    /// sqxtn, uqxtn, sqxtun: saturate from `from_signed` to `to_signed`.
    SaturatingNarrow {
        from_signed: bool,
        to_signed: bool,
    },
    /// shll: widen and shift left by the element's width in bits.
    ShiftLeftLongByWidth,
    /// urecpe and ursqrte on 32 bit fixed point.
    ReciprocalEstimateInteger,
    ReciprocalSqrtEstimateInteger,
    // Integer shifts by an immediate (`shift`).
    /// sshr, ssra, srshr, srsra and the unsigned ones.
    ShiftRight {
        signed: bool,
        rounding: bool,
        accumulate: bool,
    },
    ShiftLeft,
    /// sqshl, uqshl and sqshlu by an immediate.
    SaturatingShiftLeft {
        from_signed: bool,
        to_signed: bool,
    },
    /// shrn and rshrn.
    ShiftRightNarrow {
        rounding: bool,
    },
    /// sqshrn, sqrshrn, uqshrn, uqrshrn, sqshrun, sqrshrun.
    SaturatingShiftRightNarrow {
        from_signed: bool,
        to_signed: bool,
        rounding: bool,
    },
    /// sshll and ushll.
    ShiftLeftLong {
        signed: bool,
    },
    /// sri and sli: shift, keeping the destination's bits the shift leaves out.
    ShiftRightInsert,
    ShiftLeftInsert,
    // Long, wide and narrow on three registers.
    AddLong {
        signed: bool,
    },
    SubLong {
        signed: bool,
    },
    /// saddw: the wide `rn` plus the widened `rm`.
    AddWide {
        signed: bool,
    },
    SubWide {
        signed: bool,
    },
    /// addhn and raddhn: the high half of the wide sum.
    AddNarrowHigh {
        rounding: bool,
    },
    SubNarrowHigh {
        rounding: bool,
    },
    AbsDiffLong {
        signed: bool,
    },
    AbsDiffAccumulateLong {
        signed: bool,
    },
    MulLong {
        signed: bool,
    },
    MulAddLong {
        signed: bool,
    },
    MulSubLong {
        signed: bool,
    },
    /// sqdmull, sqdmlal, sqdmlsl.
    DoublingMulLong,
    DoublingMulAddLong,
    DoublingMulSubLong,
    /// pmull: carry-less multiply of bytes, or of 64 bit elements, to twice the width.
    PolyMulLong,
    // Floating point, elementwise.
    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,
    FloatMax,
    FloatMin,
    FloatMaxNumber,
    FloatMinNumber,
    /// fmulx: as fmul, but zero times infinity is two with the sign of the product.
    FloatMulExtended,
    /// fmla and fmls: fused.
    FloatMulAdd,
    FloatMulSub,
    FloatAbsDiff,
    FloatEqual,
    FloatGreaterOrEqual,
    FloatGreater,
    FloatAbsGreaterOrEqual,
    FloatAbsGreater,
    /// frecps: 2 - a * b, fused; frsqrts: (3 - a * b) / 2.
    FloatReciprocalStep,
    FloatReciprocalSqrtStep,
    FloatAddPairwise,
    FloatMaxPairwise,
    FloatMinPairwise,
    FloatMaxNumberPairwise,
    FloatMinNumberPairwise,
    FloatAbs,
    FloatNegate,
    FloatSqrt,
    FloatRound(Rounding),
    /// fcmgt, fcmge, fcmeq, fcmle and fcmlt with zero, by condition code.
    FloatCompareZero(Cond),
    /// fcvt*s and fcvt*u, with `shift` fraction bits.
    FloatToInteger {
        signed: bool,
        rounding: Rounding,
    },
    /// scvtf and ucvtf, with `shift` fraction bits.
    IntegerToFloat {
        signed: bool,
    },
    /// fcvtn and fcvtxn (round to odd): to half the width.
    FloatNarrow {
        odd: bool,
    },
    /// fcvtl: to twice the width.
    FloatWiden,
    FloatReciprocalEstimate,
    FloatReciprocalSqrtEstimate,
    FloatReciprocalExponent,
    // Across all elements, to one element in the low bits of the destination.
    AddAcross,
    /// saddlv, uaddlv: the sum, twice as wide.
    AddLongAcross {
        signed: bool,
    },
    MaxAcross {
        signed: bool,
    },
    MinAcross {
        signed: bool,
    },
    FloatMaxAcross,
    FloatMinAcross,
    FloatMaxNumberAcross,
    FloatMinNumberAcross,
    /// The scalar pairwise faddp: the sum of the two elements.
    FloatAddAcross,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permute {
    Zip1,
    Zip2,
    Unzip1,
    Unzip2,
    Transpose1,
    Transpose2,
}

/// The shape of an AdvSIMD data processing instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Element size in bytes, as Arm's `size` field names it.
    pub element: u8,
    /// Arm's `Q`: the whole 128 bits, or for long and narrow operations the upper half.
    pub q: bool,
    /// One element only, the rest of the destination cleared.
    pub scalar: bool,
}

/// What a vector immediate does with its register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorImmediate {
    /// movi, mvni and fmov: the register becomes the immediate.
    Move,
    /// orr: the register is ored with it.
    Or,
    /// bic: the register is anded with its inverse.
    Clear,
}

/// A system register that unprivileged code may read or write and that we answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemRegister {
    Nzcv,
    TpidrEl0,
    /// Floating point control: rounding mode, flush to zero, default not-a-number, alternative half
    /// precision.
    Fpcr,
    /// Floating point status: the cumulative exception flags and the saturation flag.
    Fpsr,
    /// The virtual counter, ticking at [`COUNTER_FREQUENCY`].
    Counter,
    /// The counter's frequency, [`COUNTER_FREQUENCY`].
    CounterFrequency,
    /// The physical counter, which Linux keeps from EL0.
    PhysicalCounter,
    /// A register that reads as a fixed value and ignores writes or cannot be written: the cache type, the
    /// zeroing block size, the read-only thread pointer and the debug communication channel with no debugger.
    Fixed(u64),
    /// An identity register, which EL1 traps, with the value Linux answers for it.
    Identity(u64),
}

/// The counter frequency we present, in Hz: one tick a nanosecond.
pub const COUNTER_FREQUENCY: u64 = 1_000_000_000;

/// What a program is told its processor has, beyond the base every program gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Offer {
    pub kept_registers: KeptRegisters,
    pub addresses: Addresses,
}

impl Offer {
    /// Every offer, in the order of their keys.
    pub const ALL: [Offer; 4] = [
        Offer {
            kept_registers: KeptRegisters::AnsweredInPlace,
            addresses: Addresses::FourGib,
        },
        Offer {
            kept_registers: KeptRegisters::Trapped,
            addresses: Addresses::FourGib,
        },
        Offer {
            kept_registers: KeptRegisters::AnsweredInPlace,
            addresses: Addresses::Windows,
        },
        Offer {
            kept_registers: KeptRegisters::Trapped,
            addresses: Addresses::Windows,
        },
    ];
}

/// The addresses a program's data accesses reach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Addresses {
    /// The 4 GiB from zero, each address its own place in the guest memory.
    #[default]
    FourGib,
    /// The 2^48 bytes of a 48-bit virtual address, through the space's window table (the translator's contract).
    Windows,
}

/// What EL0 gets from the system registers EL1 may keep from it: the counter registers, as
/// [CNTKCTL_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CNTKCTL-EL1--Counter-timer-Kernel-Control-Register) sets them for the program, the identity registers, DAIF, DAIFSet and
/// DAIFClr, SCXTNUM_EL0, the timers and the activity monitors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeptRegisters {
    /// As a program sees them under Linux, answered in the translation: CNTVCT_EL0 reads the runner's clock and
    /// CNTFRQ_EL0 its frequency (EL0VCTEN set), an identity register reads what Linux answers, and the others are
    /// undefined.
    #[default]
    AnsweredInPlace,
    /// Every access EL1 traps raises the trap, and the kernel answers it: the counter from its own clock, which replay
    /// needs, with EL0VCTEN and EL0PCTEN clear.
    Trapped,
}

pub use crate::thread_state::{FPCR_BITS, FPSR_BITS};

/// What a single instruction atomic does to memory with the register's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicOp {
    Add,
    /// ldclr: and with the inverse.
    Clear,
    Xor,
    /// ldset: or.
    Set,
    SignedMax,
    SignedMin,
    UnsignedMax,
    UnsignedMin,
    Swap,
}

/// How a floating point value is rounded to an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// To nearest, ties to even.
    Nearest,
    /// To nearest, ties away from zero.
    Away,
    /// Towards minus infinity.
    Floor,
    /// Towards plus infinity.
    Ceiling,
    /// Towards zero.
    Zero,
    /// As FPCR says.
    Current,
    /// As FPCR says, raising inexact when the value changes (frintx).
    CurrentExact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatUnary {
    Move,
    Abs,
    Negate,
    Sqrt,
    Round(Rounding),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatBinary {
    Add,
    Sub,
    Mul,
    Div,
    Max,
    Min,
    /// The number when one side is a quiet not-a-number.
    MaxNumber,
    MinNumber,
    /// The product, negated.
    NegatedMul,
}

/// One decoded instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
    AddSub {
        width: Width,
        subtract: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        operand: Operand,
    },
    AddSubCarry {
        width: Width,
        subtract: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    Logical {
        width: Width,
        op: Logic,
        /// The second source is inverted first (bic, orn, eon).
        invert: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        operand: Operand,
    },
    MoveWide {
        width: Width,
        op: MoveWide,
        rd: Reg,
        imm: u16,
        shift: u8,
    },
    Bitfield {
        width: Width,
        op: Bitfield,
        rd: Reg,
        rn: Reg,
        immr: u8,
        imms: u8,
    },
    Extract {
        width: Width,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        lsb: u8,
    },
    /// adr, or adrp when `page` is set.
    PcRelative {
        rd: Reg,
        page: bool,
        offset: i64,
    },
    /// ccmp, or ccmn when `negate` is set. `operand` is an immediate or a plain register.
    CondCompare {
        width: Width,
        negate: bool,
        rn: Reg,
        operand: Operand,
        nzcv: u8,
        cond: Cond,
    },
    CondSelect {
        width: Width,
        op: Select,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        cond: Cond,
    },
    Unary {
        width: Width,
        op: Unary,
        rd: Reg,
        rn: Reg,
    },
    Binary {
        width: Width,
        op: Binary,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    /// The long and high forms are always 64 bit; `width` is the width of the plain forms.
    MulAdd {
        width: Width,
        op: MulAdd,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Reg,
    },
    /// b, or bl when `link` is set.
    Branch {
        link: bool,
        offset: i64,
    },
    BranchCond {
        cond: Cond,
        offset: i64,
    },
    CompareBranch {
        width: Width,
        nonzero: bool,
        rt: Reg,
        offset: i64,
    },
    TestBranch {
        nonzero: bool,
        rt: Reg,
        bit: u8,
        offset: i64,
    },
    BranchRegister {
        op: BranchRegister,
        rn: Reg,
    },
    /// Integer load of `bytes` bytes. `signed` names the width the value is sign extended to; otherwise
    /// it is zero extended.
    Load {
        bytes: u8,
        signed: Option<Width>,
        rt: Reg,
        address: Address,
    },
    Store {
        bytes: u8,
        rt: Reg,
        address: Address,
    },
    /// Two loads of `bytes` each from consecutive addresses; ldpsw sign extends.
    LoadPair {
        bytes: u8,
        signed: bool,
        rt: Reg,
        rt2: Reg,
        address: Address,
    },
    StorePair {
        bytes: u8,
        rt: Reg,
        rt2: Reg,
        address: Address,
    },
    /// Load of `bytes` (1, 2, 4, 8 or 16) into the low bytes of SIMD and floating point register `rt`; the
    /// rest of the register is cleared.
    LoadVector {
        bytes: u8,
        rt: u8,
        address: Address,
    },
    /// Store of the low `bytes` of SIMD and floating point register `rt`.
    StoreVector {
        bytes: u8,
        rt: u8,
        address: Address,
    },
    /// Two vector loads of `bytes` (4, 8 or 16) each from consecutive addresses.
    LoadVectorPair {
        bytes: u8,
        rt: u8,
        rt2: u8,
        address: Address,
    },
    StoreVectorPair {
        bytes: u8,
        rt: u8,
        rt2: u8,
        address: Address,
    },
    /// AdvSIMD modified immediate. `imm` is the 64 bit pattern, repeated in both halves when `whole`;
    /// otherwise the top half is cleared.
    VectorImmediate {
        op: VectorImmediate,
        rd: u8,
        imm: u64,
        whole: bool,
    },
    /// AdvSIMD data processing. `index` names an element of `rm` used for every element (by element
    /// forms); `shift` is a shift or a number of fraction bits.
    Simd {
        op: SimdOp,
        shape: Shape,
        rd: u8,
        rn: u8,
        rm: u8,
        index: Option<u8>,
        shift: u8,
    },
    /// zip, uzp and trn.
    Permute {
        op: Permute,
        element: u8,
        whole: bool,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    /// tbl and tbx: bytes of `registers` consecutive registers from `rn`, picked by the bytes of `rm`. Out of
    /// range picks give zero, or with `extend` keep the destination's byte.
    Table {
        extend: bool,
        registers: u8,
        whole: bool,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    /// ld1 to ld4 and st1 to st4, multiple structures, one lane, or one structure to every lane
    /// (`replicate`).
    SimdMemory {
        load: bool,
        /// Elements per structure, 1 to 4, spread over that many registers.
        structure: u8,
        /// Registers in all, from `rt` upwards (wrapping at 31).
        registers: u8,
        element: u8,
        whole: bool,
        lane: Option<u8>,
        replicate: bool,
        rt: u8,
        rn: Reg,
        /// After the access, the base moves by the bytes accessed (`Zero`) or by a register.
        post: Option<Reg>,
    },
    /// dup (general): every element becomes the low bits of a general register.
    DuplicateGeneral {
        arrangement: Arrangement,
        rd: u8,
        rn: Reg,
    },
    /// dup (element): every element becomes element `index` of `rn`.
    DuplicateElement {
        arrangement: Arrangement,
        index: u8,
        rd: u8,
        rn: u8,
    },
    /// dup (scalar): element `index` of `rn` becomes the low bits of `rd`, the rest cleared.
    ScalarElement {
        bytes: u8,
        index: u8,
        rd: u8,
        rn: u8,
    },
    /// umov, smov and fmov to a general register: element `index` of `bytes` bytes, zero extended, or sign
    /// extended to the width given.
    MoveToGeneral {
        bytes: u8,
        index: u8,
        signed: Option<Width>,
        rd: Reg,
        rn: u8,
    },
    /// fmov from a general register: the low `bytes` (4 or 8) of `rd` become them, the rest is cleared.
    MoveFromGeneral {
        bytes: u8,
        rd: u8,
        rn: Reg,
    },
    /// ins (general): element `index` of `bytes` bytes becomes the low bits of a general register; the
    /// other elements are kept.
    InsertGeneral {
        bytes: u8,
        index: u8,
        rd: u8,
        rn: Reg,
    },
    /// ins (element): element `to` of `rd` becomes element `from` of `rn`.
    InsertElement {
        bytes: u8,
        to: u8,
        from: u8,
        rd: u8,
        rn: u8,
    },
    /// ext: bytes from `index` of `rm`:`rn` joined, `rn` lowest.
    ExtractVector {
        whole: bool,
        index: u8,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    /// crc32 and crc32c: the checksum in `rn` updated with the low `bytes` of `rm`.
    Crc32 {
        castagnoli: bool,
        bytes: u8,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    /// A single instruction atomic: memory at `rn` becomes `op` of itself and `rs`; `rt` gets the old value.
    Atomic {
        op: AtomicOp,
        bytes: u8,
        rs: Reg,
        rt: Reg,
        rn: Reg,
    },
    /// cas: if memory at `rn` equals `rs`, it becomes `rt`; `rs` gets the old value. With `pair`, `rs` and
    /// `rt` name even registers whose next register holds the second half.
    CompareSwap {
        bytes: u8,
        pair: bool,
        rs: Reg,
        rt: Reg,
        rn: Reg,
    },
    /// ldxr and ldxp, plain and acquiring: load and remember the address and value.
    LoadExclusive {
        bytes: u8,
        pair: bool,
        rt: Reg,
        rt2: Reg,
        rn: Reg,
    },
    /// stxr and stxp, plain and releasing: store if the address is still remembered and memory still
    /// holds the value loaded; `rs` gets 0 if stored, 1 if not.
    StoreExclusive {
        bytes: u8,
        pair: bool,
        rs: Reg,
        rt: Reg,
        rt2: Reg,
        rn: Reg,
    },
    /// ldar and stlr: a plain access at `rn` that must be aligned.
    LoadAcquire {
        bytes: u8,
        rt: Reg,
        rn: Reg,
    },
    StoreRelease {
        bytes: u8,
        rt: Reg,
        rn: Reg,
    },
    /// clrex: forget the remembered address.
    ClearExclusive,
    /// dc zva: zero the 64 byte block holding the address in `rn`.
    ZeroBlock {
        rn: Reg,
    },
    /// dc cvac, civac: cache maintenance, which needs only that the address may be read.
    CleanCache {
        rn: Reg,
    },
    /// ic ivau: the code at the address in `rn` may have changed.
    CodeChanged {
        rn: Reg,
    },
    /// Scalar floating point on `size` bytes (4 or 8): one source.
    Float {
        op: FloatUnary,
        size: u8,
        rd: u8,
        rn: u8,
    },
    /// fcvt: between half (2), single (4) and double (8) precision.
    FloatConvert {
        from: u8,
        to: u8,
        rd: u8,
        rn: u8,
    },
    FloatBinary {
        op: FloatBinary,
        size: u8,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    /// fmadd and its negations: `rd = (±ra) + (±rn) * rm`, fused, each negation applied to the operand.
    FloatMulAdd {
        negate_addend: bool,
        negate_product: bool,
        size: u8,
        rd: u8,
        rn: u8,
        rm: u8,
        ra: u8,
    },
    /// fcmp and fcmpe: with zero when `rm` is `None`. `signaling` raises invalid for quiet not-a-numbers too.
    FloatCompare {
        signaling: bool,
        size: u8,
        rn: u8,
        rm: Option<u8>,
    },
    /// fccmp and fccmpe: compare if the condition holds, else the flags become `nzcv`.
    FloatCondCompare {
        signaling: bool,
        size: u8,
        rn: u8,
        rm: u8,
        nzcv: u8,
        cond: Cond,
    },
    FloatSelect {
        size: u8,
        rd: u8,
        rn: u8,
        rm: u8,
        cond: Cond,
    },
    /// fcvt*s and fcvt*u: a float of `size` bytes, times 2 to the `fraction_bits`, rounded, saturated.
    FloatToInteger {
        size: u8,
        width: Width,
        signed: bool,
        rounding: Rounding,
        fraction_bits: u8,
        rd: Reg,
        rn: u8,
    },
    /// scvtf and ucvtf: an integer divided by 2 to the `fraction_bits`.
    IntegerToFloat {
        size: u8,
        width: Width,
        signed: bool,
        fraction_bits: u8,
        rd: u8,
        rn: Reg,
    },
    /// svc
    Syscall,
    /// brk, with its comment field.
    Breakpoint {
        comment: u16,
    },
    /// Hints, barriers and prefetches: nothing happens in a single thread.
    Nothing,
    ReadSystem {
        register: SystemRegister,
        rt: Reg,
    },
    WriteSystem {
        register: SystemRegister,
        rt: Reg,
    },
    /// An access to a register or PSTATE field EL1 keeps from EL0, which Linux answers with illegal instruction:
    /// DAIF, DAIFSet and DAIFClr, SCXTNUM_EL0, the timers, the activity monitors and an identity register Linux
    /// gives no value.
    KeptFromEl0,
    /// Not an instruction of the CPU we present.
    Undefined,
    /// An instruction of a feature the CPU we present does not offer (an instruction of the CPU we present
    /// is always translated).
    Unsupported,
}

/// Where control can go after an instruction, as far as the instruction itself says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flow {
    /// The next instruction can run after this one (including after a call returns).
    pub falls_through: bool,
    /// A direct jump or call target, in bytes from this instruction.
    pub target: Option<i64>,
}

/// The one memory access of a load or store: where, how many bytes in all, and whether it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryAccess {
    pub address: Address,
    pub bytes: u32,
    pub write: bool,
}

impl Instruction {
    pub fn memory_access(&self) -> Option<MemoryAccess> {
        use Instruction as I;
        let (address, bytes, write) = match *self {
            I::Load { bytes, address, .. } | I::LoadVector { bytes, address, .. } => {
                (address, u32::from(bytes), false)
            }
            I::Store { bytes, address, .. } | I::StoreVector { bytes, address, .. } => {
                (address, u32::from(bytes), true)
            }
            I::LoadPair { bytes, address, .. } | I::LoadVectorPair { bytes, address, .. } => {
                (address, 2 * u32::from(bytes), false)
            }
            I::StorePair { bytes, address, .. } | I::StoreVectorPair { bytes, address, .. } => {
                (address, 2 * u32::from(bytes), true)
            }
            I::Atomic { bytes, rn, .. }
            | I::StoreRelease { bytes, rn, .. }
            | I::CompareSwap {
                bytes,
                pair: false,
                rn,
                ..
            }
            | I::StoreExclusive {
                bytes,
                pair: false,
                rn,
                ..
            } => (
                Address::Offset {
                    base: rn,
                    offset: 0,
                },
                u32::from(bytes),
                true,
            ),
            I::CompareSwap { bytes, rn, .. } | I::StoreExclusive { bytes, rn, .. } => (
                Address::Offset {
                    base: rn,
                    offset: 0,
                },
                2 * u32::from(bytes),
                true,
            ),
            I::LoadAcquire { bytes, rn, .. }
            | I::LoadExclusive {
                bytes,
                pair: false,
                rn,
                ..
            } => (
                Address::Offset {
                    base: rn,
                    offset: 0,
                },
                u32::from(bytes),
                false,
            ),
            I::LoadExclusive { bytes, rn, .. } => (
                Address::Offset {
                    base: rn,
                    offset: 0,
                },
                2 * u32::from(bytes),
                false,
            ),
            I::SimdMemory {
                load,
                structure,
                registers,
                element,
                whole,
                lane,
                replicate,
                rn,
                post,
                ..
            } => {
                let bytes = if lane.is_some() || replicate {
                    u32::from(structure) * u32::from(element)
                } else {
                    u32::from(registers) * if whole { 16 } else { 8 }
                };
                // Post-indexed by the size is a step of the base; by a register it is not known here.
                let address = match post {
                    Some(Reg::Zero) => Address::PostIndex {
                        base: rn,
                        offset: bytes.into(),
                    },
                    _ => Address::Offset {
                        base: rn,
                        offset: 0,
                    },
                };
                (address, bytes, !load)
            }
            _ => return None,
        };
        Some(MemoryAccess {
            address,
            bytes,
            write,
        })
    }

    pub fn flow(&self) -> Flow {
        use Instruction as I;
        let (falls_through, target) = match *self {
            I::Branch { link, offset } => (link, Some(offset)),
            I::BranchCond { cond, offset } => (cond.0 < 14, Some(offset)),
            I::CompareBranch { offset, .. } | I::TestBranch { offset, .. } => (true, Some(offset)),
            I::BranchRegister { op, .. } => (op == BranchRegister::Blr, None),
            I::Breakpoint { .. } | I::KeptFromEl0 | I::Undefined | I::Unsupported => (false, None),
            _ => (true, None),
        };
        Flow {
            falls_through,
            target,
        }
    }
}
