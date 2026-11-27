//! The inputs of the modeled x86-64 forms, and one state for each form not yet modeled: its sample encoding at the
//! start of a page.
//!
//! Every state holds the instruction on a page of its own and, for a form that reads or writes memory, the pages its
//! access reaches with permissions that are operands. Each data byte holds the complement of its address's low byte,
//! so that no loaded value equals an address near it. iced-x86 encodes each form with its operands.
//!
//! A state's RFLAGS holds only bits a ring 3 program can set (see the architecture's `Flags`); the critical values
//! of the flags a form does not read are none and all of them, AC included, so that every misaligned access is
//! tried with alignment checking on and off.

#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, Register};
use itertools::Itertools;

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::avx::find_avx_form;
use wcontainers_processor_architecture_x86_64::branch::find_branch_form;
use wcontainers_processor_architecture_x86_64::extension::find_extension_form;
use wcontainers_processor_architecture_x86_64::fixed::find_fixed_form;
use wcontainers_processor_architecture_x86_64::mmx::find_mmx_form;
use wcontainers_processor_architecture_x86_64::stack::find_stack_form;
use wcontainers_processor_architecture_x86_64::x87::find_x87_form;
use wcontainers_processor_architecture_x86_64::evex::find_evex_form;
use wcontainers_processor_architecture_x86_64::integer::find_integer_form;
use wcontainers_processor_architecture_x86_64::float::find_float_form;
use wcontainers_processor_architecture_x86_64::vector::find_vector_form;
use wcontainers_processor_architecture_x86_64::sse::find_sse_form;
use wcontainers_processor_architecture_x86_64::avx512_integer::find_evex_integer_form;
use wcontainers_processor_architecture_x86_64::opmask::find_opmask_form;
use wcontainers_processor_architecture_x86_64::string::find_string_form;
use wcontainers_processor_architecture_x86_64::system::find_system_form;
use wcontainers_processor_architecture_x86_64::mmx_convert::find_mmx_convert_form;
use wcontainers_processor_architecture_x86_64::descriptor::find_descriptor_form;
use wcontainers_processor_architecture_x86_64::reciprocal::find_reciprocal_form;
use wcontainers_processor_architecture_x86_64::segment::find_segment_form;
use wcontainers_processor_architecture_x86_64::optional::find_optional_form;
use wcontainers_processor_architecture_x86_64::xsave::find_state_form;
use wcontainers_processor_architecture_x86_64::amx::find_amx_form;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, Instruction, InstructionFetch, PRIVILEGED_FORMS, PagePermission, Processor, SAMPLE_ENCODINGS,
    STACK_POINTER, VECTOR_REGISTERS, X86_64, ZERO_PROCESSOR,
};

pub mod amx;
pub mod avx;
pub mod avx512_integer;
pub mod branch;
pub mod compatibility;
pub mod descriptor;
pub mod evex;
pub mod extension;
pub mod fixed;
pub mod float;
pub mod integer;
pub mod mmx;
pub mod mmx_convert;
pub mod opmask;
pub mod optional;
pub mod reciprocal;
pub mod segment;
pub mod sse;
pub mod stack;
pub mod string;
pub mod system;
pub mod vector;
mod vex_upper;
mod vnni;
pub mod x87;
pub mod x87_arithmetic;
pub mod xsave;
pub mod x87_transcendental;
#[cfg(test)]
mod tests;

pub use integer::{
    Access, AddressSize, Addressing, CodeLayout, DataPlacement, Encoding, ImmediateClass, LAYOUTS_BEYOND_START,
    SegmentChoice, to_data_address,
};

/// The first page above the page at zero, which stays unmapped as on Linux.
/// Returns the upper halves every state's YMM registers start with: each register's complement, its halves swapped, so
/// that a form that keeps them, and one that zeroes them, is seen to.
pub(crate) fn fill_upper_vector_halves() -> [u128; VECTOR_REGISTERS] {
    array::from_fn(|number| (!(number as u128)).rotate_left(u64::BITS))
}

pub const CODE_ADDRESS: u64 = PAGE_BYTES;
pub const LOWER_DATA_ADDRESS: u64 = CODE_ADDRESS + PAGE_BYTES;
pub const UPPER_DATA_ADDRESS: u64 = LOWER_DATA_ADDRESS + PAGE_BYTES;
/// The lowest address above the lower canonical half.
const NON_CANONICAL_ADDRESS: u64 = 1 << 63;
const BITNESS: u32 = 64;
const DATA_PAGES_BYTES: usize = 2 * PAGE_BYTES as usize;
/// The carry out of bit 3 that AF reports.
const NIBBLE_EDGE: u64 = 1 << 4;
/// What a register holds above the width a narrower form reads of it.
const ABOVE_LOW_HALF: u64 = 0xA5A5_A5A5_0000_0000;
/// What an index or count register holds above its low byte, which the form ignores.
const ABOVE_INDEX: u64 = 0xA5A5_A5A5_A5A5_A500;
/// RAX's bits above AX when AX is an 8-bit division's dividend.
const BYTE_DIVIDEND_UPPER: u64 = 0xA5A5_A5A5_A5A5_0000;

const GENERAL_64: [Register; 16] = [
    Register::RAX, Register::RCX, Register::RDX, Register::RBX, Register::RSP, Register::RBP,
    Register::RSI, Register::RDI, Register::R8, Register::R9, Register::R10, Register::R11,
    Register::R12, Register::R13, Register::R14, Register::R15,
];
const GENERAL_32: [Register; 16] = [
    Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESP, Register::EBP,
    Register::ESI, Register::EDI, Register::R8D, Register::R9D, Register::R10D, Register::R11D,
    Register::R12D, Register::R13D, Register::R14D, Register::R15D,
];
const GENERAL_16: [Register; 16] = [
    Register::AX, Register::CX, Register::DX, Register::BX, Register::SP, Register::BP,
    Register::SI, Register::DI, Register::R8W, Register::R9W, Register::R10W, Register::R11W,
    Register::R12W, Register::R13W, Register::R14W, Register::R15W,
];
pub(crate) const VECTOR: [Register; 16] = [
    Register::XMM0, Register::XMM1, Register::XMM2, Register::XMM3, Register::XMM4,
    Register::XMM5, Register::XMM6, Register::XMM7, Register::XMM8, Register::XMM9,
    Register::XMM10, Register::XMM11, Register::XMM12, Register::XMM13, Register::XMM14,
    Register::XMM15,
];
pub(crate) const VECTOR_256: [Register; 16] = [
    Register::YMM0, Register::YMM1, Register::YMM2, Register::YMM3, Register::YMM4,
    Register::YMM5, Register::YMM6, Register::YMM7, Register::YMM8, Register::YMM9,
    Register::YMM10, Register::YMM11, Register::YMM12, Register::YMM13, Register::YMM14,
    Register::YMM15,
];

/// The bytes of both data pages, from the lower page's first address.
static DATA_PAGES: [u8; DATA_PAGES_BYTES] = {
    let mut bytes = [0; DATA_PAGES_BYTES];
    let mut offset = 0;
    while offset < DATA_PAGES_BYTES {
        bytes[offset] = !((LOWER_DATA_ADDRESS as usize + offset) as u8);
        offset += 1;
    }
    bytes
};

#[derive(Clone, Copy, Default, Debug)]
pub struct X86_64InputDomain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandType {
    OperandSize(OperandSizes),
    /// The general register that has a role in the form.
    Register(Role),
    /// The value of that register.
    RegisterValue(Role, ValueSet),
    /// The value a memory operand holds where the access lies, as wide as the access.
    MemoryValue(ValueSet),
    /// The value of the vector register that has a role in the form, or of the memory its source reads.
    VectorValue(Role, vector::VectorValues),
    /// The destination and source of a floating-point form, together: their rotations and the operation's
    /// exception pairs.
    FloatOperands(float::FloatOperands),
    /// MXCSR: the rounding mode, the exception masks, DAZ and FTZ, and the exception flags already set, as far as
    /// the form reads them.
    Mxcsr(float::MxcsrSet),
    Flags(FlagSet),
    /// Where the first byte the form reads or writes lies, around the boundary of the two data pages.
    AccessPlacement(Splits),
    DataPagePermission(DataPage),
    /// Whether CMPXCHG finds the accumulator's value in memory.
    Comparison,
    CpuidLeaf,
    CpuidSubleaf,
    /// The bits above 32 of RAX and RCX, which CPUID ignores.
    CpuidUpperHalves,
    /// CL, below RCX's other bits.
    ShiftCount,
    /// The registers a form without a memory operand names, and how its bytes lie; it names this many registers.
    Encoding(usize),
    /// A memory operand: how its address is formed and the register named besides, where the access lies, through
    /// which segment, with the code laid out how. `offset` for a form whose address is its displacement (MOV's
    /// moffs).
    Memory { splits: Splits, offset: bool },
    Immediate(ImmediateClass),
    /// The address size of a jump that counts with RCX, or with ECX under the 0x67 prefix.
    AddressSize,
    /// The sample encoding of a form not yet modeled, at its page's start with every register as the domain fills it.
    SampleEncoding,
    /// An operand of a form of [`sse`].
    Sse(sse::SseOperand),
    /// The sources, immediate and MXCSR of an AVX form that takes them together.
    AvxData(avx::AvxDataSet),
    /// The value of the MMX (or XMM or general) register that has a role in an MMX form, or of the memory its source
    /// reads.
    MmxValue(Role, vector::VectorValues),
    /// The x87 state MMX shares: its stack, tags, control and status words and pointers.
    X87Environment,
    /// ST(i)'s index in an x87 form.
    StackRegister,
    /// The x87 state around an x87 form's operands.
    X87Stack,
    /// The value of ST(0) (the destination) or ST(i) (the source) of an x87 form, or of its memory operand.
    X87Value(Role, x87::X87ValueSet),
    /// The segment register MOV reads.
    SegmentRegister,
    /// An EVEX register form's three registers and how its bytes lie.
    EvexEncoding,
    /// The first source register of an EVEX form with a memory operand.
    EvexRegister,
    /// The opmask register an EVEX form with a memory operand names, and its value, for a form of this many
    /// elements.
    EvexMask(usize),
    /// Whether an EVEX form's memory source is one element broadcast.
    EvexBroadcast,
    /// The two sources of a floating-point EVEX form and MXCSR; for a register form also the opmask and the
    /// embedded rounding or exception suppression.
    EvexFloatData(evex::EvexFloatData),
    /// The destination's old value, the two sources and the immediate of an integer EVEX form; for a register form
    /// also the opmask.
    EvexIntegerData(evex::EvexIntegerData),
    /// The destination's old value and the two sources of a fused EVEX form and MXCSR; for a register form also
    /// the opmask and the embedded rounding.
    EvexFusedData(evex::EvexFusedData),
    /// The source of a conversion, the first source of a scalar one, and MXCSR; for a register form also the opmask
    /// and the embedded rounding or exception suppression.
    EvexConvertData(evex::EvexConvertData),
    /// The two operands of a compare, its predicate and MXCSR; for a register form also the opmask and exception
    /// suppression.
    EvexCompareData(evex::EvexCompareData),
    /// The vector VCOMPRESS or VEXPAND moves elements of and what memory holds before; for a register form also the
    /// opmask.
    EvexMoveData(evex::EvexMoveData),
    /// The element (VSCALEF: both elements) of VGETEXP to VSCALEF, the immediate and MXCSR; for a register form also
    /// the opmask and the embedded control.
    EvexSpecialData(evex::EvexSpecialData),
    /// The scale, indices, opmask and data vector of an EVEX gather or scatter.
    EvexGatherData(evex::EvexGatherData),
    /// The registers an opmask form names, this many, and how its bytes lie.
    OpmaskEncoding(usize),
    /// The value of the opmask register of the form's operand of that index, `bits` of it read.
    OpmaskValue { operand: usize, bits: u32 },
    /// The vector registers an EVEX form names, which of its `codes` encodes it, and, for a form without a memory
    /// operand, how its bytes lie.
    EvexIntegerEncoding { codes: usize, in_memory: bool },
    /// The writemask, merging and, where `zeroing`, zeroing.
    Writemask { zeroing: bool },
    /// Whether an EVEX form's memory operand is one lane broadcast.
    LaneBroadcast,
    /// The value of a ZMM register that has a role in the form (the first source's is `Implicit(1)`), or of the
    /// memory its last source reads.
    ZmmValue(Role, avx512_integer::ZmmValues),
    /// Where a string form's source and destination elements lie, for a form that reaches each, of `bytes` each.
    StringLayout { source: bool, destination: bool, bytes: u64 },
    /// A repeated string form's count.
    StringCount,
    /// The accumulator or the elements a string form stores or compares.
    StringData(string::StringDataSet),
    /// Where the stack access of PUSHF, POPF or ENTER lies, of `bytes`, or how the code lies.
    SystemLayout { bytes: u64 },
    /// The flags PUSHF pushes.
    SystemFlags,
    /// The image POPF pops.
    PoppedFlags,
    /// ENTER's frame size and nesting level.
    EnterOperands,
    /// The source lanes of a conversion between MMX and XMM registers, or MASKMOVQ's mask.
    ConvertSource(mmx_convert::ConvertSet),
    /// FCW, FSW and the operands of an x87 arithmetic form together.
    X87Case(x87_arithmetic::X87CaseSet),
    /// The selector LAR to VERW read.
    Selector,
    /// The four lanes RCPPS to RSQRTSS read.
    ReciprocalSource,
    /// The segment register MOV writes and the selector together.
    SegmentLoad,
    /// The selector a segment load takes.
    SegmentSelector,
    /// The far pointer a far CALL, JMP or RET takes.
    FarPointer,
    /// The slots IRET pops.
    IretFrame,
    /// Where the far CALL pushes.
    CallStack,
    /// Where IRET's stack access of `bytes` lies.
    FrameLayout { bytes: u64 },
    /// The hypercall number VMCALL reads in RAX.
    HypercallNumber,
    /// PKRU with EAX, ECX and EDX for RDPKRU and WRPKRU.
    KeyRights,
    /// The registers a form of the `optional` family names, which of its `codes` encodes it (the operand or
    /// address size), and how its bytes lie; VEX names only the low 16 vector registers.
    OptionalEncoding { vex: bool, codes: usize },
    /// The vector a form of the `optional` family reads, or the memory it reads.
    OptionalVector(optional::VectorSet),
    /// MXCSR for a form of the `optional` family; a half-precision form ignores DAZ and FTZ.
    OptionalMxcsr { half: bool },
    /// The segment base RDFSBASE or RDGSBASE reads.
    OptionalBase,
    /// Where MOVDIR64B's destination, or UMONITOR's address, lies.
    OptionalPlace { block: bool },
    /// XBEGIN's fallback and how its code lies.
    OptionalFallback,
    /// Where an XSAVE form's area lies, for a form that reads it or writes it.
    XsaveArea { read: bool },
    /// EDX:EAX's request of an XSAVE form.
    XsaveRequest,
    /// What the XSAVE components hold and which of them XINUSE marks, with MXCSR.
    RegisterContents,
    /// The XSTATE_BV XSAVE and XSAVEOPT find in the area.
    OldStateBitmap,
    /// The header and image XRSTOR reads.
    RestoredImage,
    /// XGETBV's ECX.
    ControlIndex,
    /// UMWAIT's or TPAUSE's source register and its value.
    WaitSource,
    /// UMWAIT's or TPAUSE's deadline in EDX:EAX.
    WaitDeadline,
    /// What XSAVEOPT's modified optimization knows before it runs.
    XrstorTracking,
    /// The registers XRSTOR restores over and what XINUSE says of them.
    RestoredOver,
    /// JCXZ's operand size, code address, displacement and RCX together.
    CountJump,
    /// XCR0 and IA32_XFD with TILECFG before, for an AMX form.
    TileState(amx::ConfigurationSet),
    /// The tiles an AMX form names, this many.
    TileNames(usize),
    /// Where an AMX form's memory operand lies, for a form that writes it, and for one that moves tile rows.
    TilePlacement { write: bool, rows: bool },
    /// The 64 bytes LDTILECFG reads.
    TileConfigurationImage,
    /// What the tiles hold.
    TileContents(amx::ContentSet),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandSizes {
    All,
    /// 16 and 64 bits, the sizes PUSH has in 64-bit mode.
    WithoutThirtyTwo,
    /// 32 and 64 bits, the sizes SYSEXIT has.
    WithoutSixteen,
    /// 16 and 32 bits, the sizes a `z` operand has (MOVSXD's).
    WithoutSixtyFour,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandSize {
    Bits16,
    Bits32,
    Bits64,
}

impl OperandSize {
    pub const fn to_bytes(self) -> u64 {
        match self {
            Self::Bits16 => 2,
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Destination,
    Source,
    Base,
    /// A register the form reads besides its operands, by number: the accumulator or the data register.
    Implicit(usize),
    /// The register of the form's operand at this index, for a form with several register sources.
    Operand(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueSet {
    /// Every edge of a 16-, 32- and 64-bit sum.
    Arithmetic,
    /// The edges of an 8-bit sum, above bits that a write of 8 bits must keep.
    Byte,
    /// Fewer edges, for forms with many other operands.
    Few,
    /// Two values that differ in every byte.
    Distinct,
    /// A bit offset: inside the operand, at each edge of it and past them both ways, and as far as it reaches.
    BitOffset,
    /// AX as an 8-bit division's dividend: every edge of a 16-, 32- and 64-bit sum, and the dividends whose quotient
    /// by 1 or by -1 is the least or the greatest signed byte, or one past it.
    ByteDividend,
    /// An index or count read from a register's low bits: each side of 8, 16, 32 and 64, the largest byte, and some
    /// with every byte above the lowest set.
    Index,
    /// BEXTR's control: a start and a length each at 0, 1 and each side of 32 and 64, and once with bits above them.
    BitField,
    /// PDEP's and PEXT's mask: none, one bit at each end of 32 and 64 bits, alternating bits and bytes, and all.
    Mask,
    /// FLDCW's source: FNINIT's, none, all, every defined bit, and some of each field.
    ControlWord,
    /// A single-precision number of each class: each zero, one, a negative, the denormal ends, the smallest and
    /// largest normal, each infinity, a QNaN and an SNaN, and two.
    SingleFloat,
    /// The same in double precision.
    DoubleFloat,
    /// A near branch's target: canonical in the lower half, the first address past it, canonical in the upper half,
    /// and the last address of the lower half.
    Target,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagSet {
    /// The form reads no flag: none, or all of them.
    Unread,
    Carry,
    /// The flags a condition reads: every subset of them, and all flags.
    Condition(Flags),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Splits {
    Allowed,
    /// A locked access across a cache line is a split lock, which the host may slow down or turn into #AC.
    Excluded,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataPage {
    Lower,
    Upper,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessPlacement {
    LowerPageStart,
    /// One byte above the lower page's start.
    Misaligned,
    EndsAtBoundary,
    StraddlesBoundary,
    UpperPageStart,
    NonCanonical,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comparison {
    Equal,
    Differs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandValue {
    OperandSize(OperandSize),
    Register(usize),
    /// A register's value, a CPUID input or a shift count.
    Bits(u64),
    Vector(u128),
    Vectors { destination: u128, source: u128 },
    Flags(Flags),
    AccessPlacement(AccessPlacement),
    PagePermission(PagePermission),
    Comparison(Comparison),
    Encoding(Encoding),
    Immediate(i64),
    Access(Access),
    Mxcsr(u32),
    AddressSize(AddressSize),
    SampleEncoding,
    Sse(sse::SseValue),
    AvxData(avx::AvxData),
    X87(mmx::X87Environment),
    X87Stack(x87::X87Stack),
    EvexEncoding(evex::EvexEncoding),
    EvexMask(evex::EvexMask),
    Broadcast(bool),
    /// Two ZMM values, lowest 128 bits first, an opmask (k0 where the form's own operand gives it), an embedded
    /// control and MXCSR.
    EvexFloatData {
        first: [u128; 4],
        second: [u128; 4],
        mask: evex::EvexMask,
        control: evex::EmbeddedControl,
        mxcsr: u32,
    },
    /// Three ZMM values, an immediate (0 for a form without one) and an opmask (k0 where the form's own operand
    /// gives it).
    EvexIntegerData {
        destination: [u128; 4],
        first: [u128; 4],
        second: [u128; 4],
        immediate: u8,
        mask: evex::EvexMask,
    },
    /// Two ZMM values, a predicate, an opmask, an embedded control and MXCSR.
    EvexCompareData {
        first: [u128; 4],
        second: [u128; 4],
        immediate: u8,
        mask: evex::EvexMask,
        control: evex::EmbeddedControl,
        mxcsr: u32,
    },
    /// Three ZMM values, an opmask, an embedded control and MXCSR.
    EvexFusedData {
        destination: [u128; 4],
        first: [u128; 4],
        second: [u128; 4],
        mask: evex::EvexMask,
        control: evex::EmbeddedControl,
        mxcsr: u32,
    },
    /// A gather's or scatter's scale, indices (each index element's value), opmask and data vector.
    EvexGatherData {
        scale: u8,
        indices: [u128; 4],
        mask: evex::EvexMask,
        values: [u128; 4],
    },
    OpmaskEncoding(opmask::OpmaskEncoding),
    EvexIntegerEncoding(avx512_integer::EvexIntegerEncoding),
    Writemask(avx512_integer::Writemask),
    LaneBroadcast(bool),
    Zmm([u128; 4]),
    StringLayout(string::StringLayout),
    /// Two values of a string form: the accumulator and the destination element, or the source and destination
    /// elements.
    StringData { first: u64, second: u64 },
    SystemLayout(system::SystemLayout),
    EnterOperands { allocation: u16, level: u8 },
    X87Case(x87_arithmetic::X87Case),
    FarPointer { offset: u64, selector: u16 },
    SegmentLoad { segment: usize, selector: u16 },
    KeyRights { pkru: u32, accumulator: u64, counter: u64, data: u64 },
    IretFrame(segment::IretFrame),
    CallStack(segment::CallStack),
    OptionalPlace(optional::Place),
    XsaveArea(xsave::AreaPlacement),
    RegisterContents(xsave::RegisterContents),
    /// The XSTATE_BV written over the area's header, or `None` to keep the data page's bytes.
    OldStateBitmap(Option<u64>),
    RestoredImage(xsave::RestoredImage),
    /// EDX:EAX's request of an XSAVE form, which decides how far its access reaches.
    XsaveRequest(u64),
    /// The area of the last XRSTOR and the components unmodified since, or none.
    XrstorTracking(Option<(u64, u64)>),
    WaitSource { register: usize, value: u64 },
    CountJump(compatibility::CountJump),
    TileState(amx::TileState),
    /// The destination's or the one tile's name, then the sources'.
    TileNames([u8; 3]),
    TilePlacement(amx::Placement),
    /// An index into [`amx::list_images`].
    TileConfigurationImage(usize),
    TileContents(amx::Contents),
}

impl InstructionInputDomain<X86_64> for X86_64InputDomain {
    type OperandType = OperandType;
    type OperandValue = OperandValue;
    type OperandTypes<'domain> = vec::IntoIter<OperandType>;
    type CriticalValues<'domain> = vec::IntoIter<OperandValue>;

    /// The operands of the form in Intel's order, then the state it reads besides them; for a form not yet modeled
    /// its sample encoding.
    fn list_operand_types(&self, instruction: Instruction) -> vec::IntoIter<OperandType> {
        use OperandType::{
            AccessPlacement, CpuidLeaf, CpuidSubleaf, CpuidUpperHalves, DataPagePermission, OperandSize, Register,
        };
        const LOWER: OperandType = DataPagePermission(DataPage::Lower);
        const UPPER: OperandType = DataPagePermission(DataPage::Upper);
        const UNREAD: OperandType = OperandType::Flags(FlagSet::Unread);
        if let Some(form) = find_integer_form(instruction) {
            return integer::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_extension_form(instruction) {
            return extension::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_fixed_form(instruction) {
            return fixed::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_mmx_form(instruction) {
            return mmx::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_x87_form(instruction) {
            return x87::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_stack_form(instruction) {
            return stack::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_vector_form(instruction) {
            return vector::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_avx_form(instruction) {
            return avx::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_float_form(instruction) {
            return float::list_operand_types(form).into_iter();
        }
        if instruction == Instruction::JcxzRelbrb {
            return compatibility::list_operand_types().into_iter();
        }
        if let Some(form) = find_branch_form(instruction) {
            return branch::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_sse_form(instruction) {
            return sse::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_evex_form(instruction) {
            return evex::list_operand_types(&form).into_iter();
        }
        if let Some(form) = find_opmask_form(instruction) {
            return opmask::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_evex_integer_form(instruction) {
            return avx512_integer::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_string_form(instruction) {
            return string::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_system_form(instruction) {
            return system::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_mmx_convert_form(instruction) {
            return mmx_convert::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_descriptor_form(instruction) {
            return descriptor::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_reciprocal_form(instruction) {
            return reciprocal::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_segment_form(instruction) {
            return segment::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_optional_form(instruction) {
            return optional::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_state_form(instruction) {
            return xsave::list_operand_types(form).into_iter();
        }
        if let Some(form) = find_amx_form(instruction) {
            return amx::list_operand_types(form).into_iter();
        }
        if let Some(codes) = find_privileged_codes(instruction) {
            let sizes = (codes.len() > 1).then_some(OperandSize(OperandSizes::WithoutSixteen));
            return sizes.into_iter().chain([OperandType::Encoding(0), UNREAD]).collect::<Vec<_>>().into_iter();
        }
        let operand_types: &[OperandType] = match instruction {
            Instruction::PushGprv50 => &[
                OperandSize(OperandSizes::WithoutThirtyTwo),
                Register(Role::Source),
                AccessPlacement(Splits::Allowed),
                LOWER,
                UPPER,
                UNREAD,
            ],
            Instruction::Ud2 | Instruction::Int3 => &[UNREAD],
            Instruction::Cpuid => &[CpuidLeaf, CpuidSubleaf, CpuidUpperHalves],
            _ if SAMPLE_ENCODINGS[instruction as usize].is_empty() => &[],
            _ => &[OperandType::SampleEncoding],
        };
        Vec::from(operand_types).into_iter()
    }

    fn generate_critical_values(&self, operand_type: OperandType) -> vec::IntoIter<OperandValue> {
        let bits = |values: &[u64]| values.iter().copied().map(OperandValue::Bits).collect::<Vec<_>>();
        let values: Vec<OperandValue> = match operand_type {
            OperandType::OperandSize(OperandSizes::All) => {
                [OperandSize::Bits16, OperandSize::Bits32, OperandSize::Bits64]
                    .map(OperandValue::OperandSize)
                    .to_vec()
            }
            OperandType::OperandSize(OperandSizes::WithoutThirtyTwo) => {
                [OperandSize::Bits16, OperandSize::Bits64]
                    .map(OperandValue::OperandSize)
                    .to_vec()
            }
            OperandType::OperandSize(OperandSizes::WithoutSixteen) => {
                [OperandSize::Bits32, OperandSize::Bits64]
                    .map(OperandValue::OperandSize)
                    .to_vec()
            }
            OperandType::OperandSize(OperandSizes::WithoutSixtyFour) => {
                [OperandSize::Bits16, OperandSize::Bits32]
                    .map(OperandValue::OperandSize)
                    .to_vec()
            }
            // RAX, RSP and R13, which needs REX.
            OperandType::Register(Role::Destination | Role::Source | Role::Operand(_)) => {
                [0, STACK_POINTER, 13].map(OperandValue::Register).to_vec()
            }
            OperandType::Register(Role::Base | Role::Implicit(_)) => {
                [3, STACK_POINTER, 5, 13].map(OperandValue::Register).to_vec()
            }
            // The values whose carry or borrow chain ends at each side of the low nibble's edge, which AF reports and
            // tells apart from a carry or borrow at bit 3 or 5: adding 1 to 7 carries out of bit 2 alone and to 0xF
            // (its low nibble F, bit 4 clear) out of bit 3 alone; subtracting 1 from 8 borrows from bit 3 alone and
            // from 0x10 from bit 4 alone, as negating them does; 8 plus 8 and 0x10 less 8 carry or borrow at bit 4
            // alone.
            OperandType::RegisterValue(_, set) | OperandType::MemoryValue(set) if set != ValueSet::Arithmetic => {
                match set {
                    ValueSet::Byte => [0, 1, NIBBLE_EDGE / 2 - 1, NIBBLE_EDGE / 2, NIBBLE_EDGE - 1, NIBBLE_EDGE, 0x7F, 0x80, 0xFF]
                        .map(|byte| OperandValue::Bits(0xA5A5_A5A5_A5A5_A500 | byte))
                        .to_vec(),
                    ValueSet::Few => bits(&[0, 1, 0x8000_0000, u64::MAX]),
                    ValueSet::Index => bits(&[0, 1, 7, 8, 15, 16, 31, 32, 63, 64, 0xFF, ABOVE_INDEX | 1, ABOVE_INDEX | 31, ABOVE_INDEX | 64]),
                    ValueSet::BitField => {
                        let edges = [0, 1, 31, 32, 63, 64];
                        let mut values: Vec<u64> =
                            edges.iter().flat_map(|&start| [0, 1, 31, 32, 64].map(|length| start | (length << 8))).collect();
                        values.push(ABOVE_INDEX & !0xFFFF | 4 | (8 << 8));
                        bits(&values)
                    }
                    ValueSet::ControlWord => bits(&[0x037F, 0x0000, 0xFFFF, 0x1F7F, 0x0C7E, 0x027F, 0x0040]),
                    ValueSet::Target => bits(&[CODE_ADDRESS + 0x123, 1 << 47, 0xFFFF_8000_0000_0000, (1 << 47) - 1]),
                    ValueSet::SingleFloat => bits(&[
                        0, 0x8000_0000, 0x3F80_0000, 0xBFC0_0000, 1, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0x7F80_0000,
                        0xFF80_0000, 0x7FC0_0001, 0x7FA0_0001, 0x4000_0000,
                    ]),
                    ValueSet::DoubleFloat => bits(&[
                        0,
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
                        0x4000_0000_0000_0000,
                    ]),
                    ValueSet::Mask => bits(&[
                        0,
                        1,
                        0x8000_0000,
                        0x8000_0000_0000_0000,
                        0x5555_5555_5555_5555,
                        0xAAAA_AAAA_AAAA_AAAA,
                        0x00FF_00FF_00FF_00FF,
                        u64::MAX,
                    ]),
                    ValueSet::BitOffset => bits(&[0, 1, 15, 16, 31, 32, 63, 64, u64::MAX, -64i64 as u64, i64::MAX as u64]),
                    ValueSet::ByteDividend => {
                        let mut values = self.generate_critical_values(OperandType::MemoryValue(ValueSet::Arithmetic)).collect::<Vec<_>>();
                        // -128, 128, -129 and 127 in AX, above bits a division of bytes keeps.
                        values.extend(bits(&[0xFF80, 0x0080, 0xFF7F, 0x007F].map(|dividend| BYTE_DIVIDEND_UPPER | dividend)));
                        values
                    }
                    _ => bits(&[0x0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210]),
                }
            }
            OperandType::RegisterValue(..) | OperandType::MemoryValue(_) => bits(&[
                0,
                1,
                NIBBLE_EDGE / 2 - 1,
                NIBBLE_EDGE / 2,
                NIBBLE_EDGE - 1,
                NIBBLE_EDGE,
                0x7FFF,
                0x8000,
                0xFFFF,
                0x7FFF_FFFF,
                0x8000_0000,
                0xFFFF_FFFF,
                i64::MAX as u64,
                i64::MIN as u64,
                u64::MAX,
            ]),
            OperandType::VectorValue(_, values) => vector::generate_vectors(values),
            OperandType::FloatOperands(operands) => float::generate_operands(operands),
            OperandType::Mxcsr(set) => float::generate_mxcsr_values(set),
            OperandType::Flags(FlagSet::Unread) => {
                [Flags::empty(), Flags::all()].map(OperandValue::Flags).to_vec()
            }
            OperandType::Flags(FlagSet::Carry) => [
                Flags::empty(),
                Flags::CARRY,
                Flags::all().difference(Flags::CARRY),
                Flags::all(),
            ]
            .map(OperandValue::Flags)
            .to_vec(),
            OperandType::Flags(FlagSet::Condition(read)) => read
                .iter()
                .powerset()
                .map(|subset| subset.into_iter().collect::<Flags>())
                .chain([Flags::all()])
                .map(OperandValue::Flags)
                .collect(),
            OperandType::AccessPlacement(splits) => {
                let all = [
                    AccessPlacement::LowerPageStart,
                    AccessPlacement::Misaligned,
                    AccessPlacement::EndsAtBoundary,
                    AccessPlacement::StraddlesBoundary,
                    AccessPlacement::UpperPageStart,
                    AccessPlacement::NonCanonical,
                ];
                all.into_iter()
                    .filter(|placement| {
                        splits == Splits::Allowed || *placement != AccessPlacement::StraddlesBoundary
                    })
                    .map(OperandValue::AccessPlacement)
                    .collect()
            }
            OperandType::DataPagePermission(_) => {
                let mapped = |data_access| PagePermission::Mapped {
                    data_access,
                    instruction_fetch: InstructionFetch::Forbidden,
                };
                [
                    PagePermission::Unmapped,
                    mapped(DataAccess::ReadOnly),
                    mapped(DataAccess::ReadWrite),
                ]
                .map(OperandValue::PagePermission)
                .to_vec()
            }
            OperandType::Comparison => [Comparison::Equal, Comparison::Differs]
                .map(OperandValue::Comparison)
                .to_vec(),
            // Basic, extended and hypervisor leaves, and one above the basic and the extended range.
            OperandType::CpuidLeaf => bits(&[
                0,
                1,
                7,
                0xD,
                0x4000_0000,
                0x7FFF_FFFF,
                0x8000_0000,
                0x8000_0001,
                0x8000_0008,
                0x8FFF_FFFF,
            ]),
            OperandType::CpuidSubleaf => bits(&[0, 1, u64::from(u32::MAX)]),
            OperandType::CpuidUpperHalves => bits(&[0, 0xFFFF_FFFF_0000_0000]),
            OperandType::ShiftCount => bits(&[0, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 63, 64, 0xFF]),
            OperandType::Encoding(registers) => integer::generate_encodings(registers),
            OperandType::Memory { splits, offset } => integer::generate_accesses(splits, offset),
            OperandType::Immediate(class) => integer::generate_immediates(class),
            OperandType::AddressSize => [AddressSize::Bits64, AddressSize::Bits32].map(OperandValue::AddressSize).to_vec(),
            OperandType::SampleEncoding => vec![OperandValue::SampleEncoding],
            OperandType::Sse(operand) => sse::generate_values(operand),
            OperandType::AvxData(set) => avx::generate_data(set),
            OperandType::MmxValue(_, values) => mmx::generate_mmx_values(values),
            OperandType::X87Environment => mmx::X87_ENVIRONMENTS.map(OperandValue::X87).to_vec(),
            OperandType::StackRegister => x87::STACK_INDICES.map(OperandValue::Register).to_vec(),
            OperandType::SegmentRegister => stack::SEGMENT_NUMBERS.map(OperandValue::Register).to_vec(),
            OperandType::X87Stack => x87::X87_STACKS.map(OperandValue::X87Stack).to_vec(),
            OperandType::X87Value(_, set) => x87::generate_x87_values(set),
            OperandType::EvexEncoding => evex::generate_encodings(),
            OperandType::EvexRegister => evex::generate_first_sources(),
            OperandType::EvexMask(set) => evex::generate_masks(set),
            OperandType::EvexBroadcast => vec![OperandValue::Broadcast(false), OperandValue::Broadcast(true)],
            OperandType::EvexFloatData(data) => evex::generate_float_data(data),
            OperandType::EvexIntegerData(data) => evex::generate_integer_data(data),
            OperandType::EvexFusedData(data) => evex::generate_fused_data(data),
            OperandType::EvexConvertData(data) => evex::generate_convert_data(data),
            OperandType::EvexCompareData(data) => evex::generate_compare_data(data),
            OperandType::EvexMoveData(data) => evex::generate_move_data(data),
            OperandType::EvexSpecialData(data) => evex::generate_special_data(data),
            OperandType::EvexGatherData(data) => evex::generate_gather_data(data),
            OperandType::OpmaskEncoding(_) => opmask::generate_encodings(),
            OperandType::OpmaskValue { bits, .. } => opmask::generate_values(bits),
            OperandType::EvexIntegerEncoding { codes, in_memory } => avx512_integer::generate_encodings(codes, in_memory),
            OperandType::Writemask { zeroing } => avx512_integer::generate_maskings(zeroing),
            OperandType::LaneBroadcast => [false, true].map(OperandValue::LaneBroadcast).to_vec(),
            OperandType::ZmmValue(_, values) => avx512_integer::generate_zmm_values(values),
            OperandType::StringLayout { source, destination, bytes } => string::generate_layouts(source, destination, bytes),
            OperandType::StringCount => string::generate_counts(),
            OperandType::StringData(set) => string::generate_data(set),
            OperandType::SystemLayout { bytes } => system::generate_layouts(bytes),
            OperandType::SystemFlags => system::generate_flags(),
            OperandType::PoppedFlags => system::generate_popped(),
            OperandType::EnterOperands => system::generate_enter_operands(),
            OperandType::ConvertSource(set) => mmx_convert::generate_sources(set),
            OperandType::X87Case(set) => x87_arithmetic::generate_cases(set),
            OperandType::Selector => descriptor::generate_selectors(),
            OperandType::ReciprocalSource => reciprocal::generate_sources(),
            OperandType::SegmentLoad => segment::generate_segment_loads(),
            OperandType::SegmentSelector => segment::generate_selectors(),
            OperandType::FarPointer => segment::generate_far_pointers(),
            OperandType::IretFrame => segment::generate_frames(),
            OperandType::CallStack => segment::generate_call_stacks(),
            OperandType::FrameLayout { bytes } => segment::generate_frame_layouts(bytes),
            OperandType::HypercallNumber => system::generate_hypercall_numbers(),
            OperandType::KeyRights => system::generate_key_rights(),
            OperandType::OptionalEncoding { vex, codes } => optional::generate_encodings(vex, codes),
            OperandType::OptionalVector(set) => optional::generate_vectors(set),
            OperandType::OptionalMxcsr { half } => optional::generate_mxcsr_values(half),
            OperandType::OptionalBase => optional::generate_bases(),
            OperandType::OptionalPlace { block } => optional::generate_places(block),
            OperandType::OptionalFallback => optional::generate_fallbacks(),
            OperandType::XsaveArea { read } => xsave::generate_areas(read),
            OperandType::XsaveRequest => xsave::generate_requests(),
            OperandType::RegisterContents => xsave::generate_contents(),
            OperandType::OldStateBitmap => xsave::generate_old_bitmaps(),
            OperandType::RestoredImage => xsave::generate_images(),
            OperandType::ControlIndex => xsave::generate_control_indices(),
            OperandType::WaitSource => xsave::generate_wait_sources(),
            OperandType::WaitDeadline => xsave::generate_deadlines(),
            OperandType::XrstorTracking => xsave::generate_trackings(),
            OperandType::RestoredOver => xsave::generate_restored_over(),
            OperandType::CountJump => compatibility::generate_cases(),
            OperandType::TileState(set) => amx::generate_states(set),
            OperandType::TileNames(count) => amx::generate_names(count),
            OperandType::TilePlacement { write, rows } => amx::generate_placements(write, rows),
            OperandType::TileConfigurationImage => amx::generate_images(),
            OperandType::TileContents(set) => amx::generate_contents(set),
        };
        values.into_iter()
    }

    /// Every register no operand names holds the complement of its number, so that none holds zero; a VEX form's
    /// state also holds ZMM's bits above 255 and ZMM16 to ZMM31, which no VEX form names (see `vex_upper`).
    ///
    /// A register named twice holds the value of its last register value operand; a base register holds what puts
    /// the access at its placement, whatever else names it. XINUSE marks every component out of its initial
    /// configuration besides those the form's operands mark.
    ///
    /// # Panics
    ///
    /// When the operands are not one value of each operand type the form lists, in its order.
    fn build_state(&self, instruction: Instruction, operands: &[OperandValue]) -> ExecutionState<'static, X86_64> {
        let mut state = vex_upper::fill_upper_bits(self.build_form_state(instruction, operands));
        wcontainers_processor_architecture_x86_64::xsave::mark_components_in_use(&mut state.processor);
        state
    }
}

impl X86_64InputDomain {
    /// The state as the form's module builds it.
    fn build_form_state(&self, instruction: Instruction, operands: &[OperandValue]) -> ExecutionState<'static, X86_64> {
        let typed: Vec<(OperandType, OperandValue)> = self
            .list_operand_types(instruction)
            .zip(operands.iter().copied())
            .collect();
        if !is_modeled(instruction) {
            assert!(typed.len() == operands.len(), "one value for each operand of {instruction:?}");
            return build_sample_state(instruction);
        }
        assert!(
            !typed.is_empty() && typed.len() == operands.len(),
            "one value for each operand of {instruction:?}, a modeled form"
        );
        if let Some(form) = find_integer_form(instruction) {
            return integer::build_state(form, &typed);
        }
        if let Some(form) = find_extension_form(instruction) {
            return extension::build_state(form, &typed);
        }
        if let Some(form) = find_fixed_form(instruction) {
            return fixed::build_state(form, &typed);
        }
        if let Some(form) = find_mmx_form(instruction) {
            return mmx::build_state(form, &typed);
        }
        if let Some(form) = find_x87_form(instruction) {
            return x87::build_state(form, &typed);
        }
        if let Some(form) = find_stack_form(instruction) {
            return stack::build_state(form, &typed);
        }
        if let Some(form) = find_vector_form(instruction) {
            return vector::build_state(form, &typed);
        }
        if let Some(form) = find_avx_form(instruction) {
            return avx::build_state(form, &typed);
        }
        if let Some(form) = find_float_form(instruction) {
            return float::build_state(form, &typed);
        }
        if instruction == Instruction::JcxzRelbrb {
            return compatibility::build_state(&typed);
        }
        if let Some(form) = find_branch_form(instruction) {
            return branch::build_state(form, &typed);
        }
        if let Some(form) = find_sse_form(instruction) {
            return sse::build_state(form, &typed);
        }
        if let Some(form) = find_evex_form(instruction) {
            return evex::build_state(&form, &typed);
        }
        if let Some(form) = find_opmask_form(instruction) {
            return opmask::build_state(form, &typed);
        }
        if let Some(form) = find_evex_integer_form(instruction) {
            return avx512_integer::build_state(form, &typed);
        }
        if let Some(form) = find_string_form(instruction) {
            return string::build_state(form, &typed);
        }
        if let Some(form) = find_system_form(instruction) {
            return system::build_state(form, &typed);
        }
        if let Some(form) = find_mmx_convert_form(instruction) {
            return mmx_convert::build_state(form, &typed);
        }
        if let Some(form) = find_descriptor_form(instruction) {
            return descriptor::build_state(form, &typed);
        }
        if let Some(form) = find_reciprocal_form(instruction) {
            return reciprocal::build_state(form, &typed);
        }
        if let Some(form) = find_segment_form(instruction) {
            return segment::build_state(form, &typed);
        }
        if let Some(form) = find_optional_form(instruction) {
            return optional::build_state(form, &typed);
        }
        if let Some(form) = find_state_form(instruction) {
            return xsave::build_state(form, &typed);
        }
        if let Some(form) = find_amx_form(instruction) {
            return amx::build_state(form, &typed);
        }
        let mut choice = Choice::default();
        for (operand_type, operand) in typed {
            choice.take(operand_type, operand);
        }
        let mut processor = Processor {
            general_purpose_registers: array::from_fn(|number| !(number as u64)),
            instruction_pointer: CODE_ADDRESS,
            vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
            ..ZERO_PROCESSOR
        };
        processor.flags = choice.flags;
        for (role, value) in &choice.values {
            let number = choice.to_register(*role);
            processor.general_purpose_registers[number] = *value;
        }
        if let Some(leaf) = choice.leaf {
            let upper = choice.upper_halves;
            processor.general_purpose_registers[0] = upper | leaf;
            processor.general_purpose_registers[1] = upper | choice.subleaf;
        }
        let mut memory_mutations = Vec::new();
        if let Some(placement) = choice.placement {
            let access_bytes = choice.size.to_bytes();
            let address = to_access_address(placement, access_bytes);
            processor.general_purpose_registers[STACK_POINTER] = address.wrapping_add(access_bytes);
            memory_mutations.extend(to_data_pages(choice.lower, choice.upper));
        }
        let code = encode(instruction, &choice);
        let (code_page, start, bytes) = integer::lay_out(choice.layout, &code, integer::to_padding(None));
        processor.instruction_pointer = start;
        let readable_and_executable = PagePermission::Mapped {
            data_access: DataAccess::ReadOnly,
            instruction_fetch: InstructionFetch::Allowed,
        };
        let mut all = vec![
            MemoryDelta::PermissionAssignment {
                addresses: code_page..code_page + PAGE_BYTES,
                permission: readable_and_executable,
            },
            MemoryDelta::ValueAssignment {
                start,
                bytes: Cow::Owned(bytes),
            },
        ];
        all.append(&mut memory_mutations);
        ExecutionState {
            processor,
            memory_mutations: all,
        }
    }
}

/// The operand values of one state, by what they choose.
struct Choice {
    size: OperandSize,
    source: usize,
    values: Vec<(Role, u64)>,
    flags: Flags,
    placement: Option<AccessPlacement>,
    lower: PagePermission,
    upper: PagePermission,
    leaf: Option<u64>,
    subleaf: u64,
    upper_halves: u64,
    layout: CodeLayout,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            size: OperandSize::Bits64,
            source: 0,
            values: Vec::new(),
            flags: Flags::empty(),
            placement: None,
            lower: PagePermission::Unmapped,
            upper: PagePermission::Unmapped,
            leaf: None,
            subleaf: 0,
            upper_halves: 0,
            layout: CodeLayout::PageStart,
        }
    }
}

impl Choice {
    fn take(&mut self, operand_type: OperandType, operand: OperandValue) {
        match (operand_type, operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(size)) => self.size = size,
            (OperandType::Register(Role::Source), OperandValue::Register(number)) => self.source = number,
            (OperandType::RegisterValue(role, _), OperandValue::Bits(value)) => self.values.push((role, value)),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => self.flags = flags,
            (OperandType::AccessPlacement(_), OperandValue::AccessPlacement(placement)) => {
                self.placement = Some(placement);
            }
            (OperandType::DataPagePermission(DataPage::Lower), OperandValue::PagePermission(permission)) => {
                self.lower = permission;
            }
            (OperandType::DataPagePermission(DataPage::Upper), OperandValue::PagePermission(permission)) => {
                self.upper = permission;
            }
            (OperandType::CpuidLeaf, OperandValue::Bits(leaf)) => self.leaf = Some(leaf),
            (OperandType::CpuidSubleaf, OperandValue::Bits(subleaf)) => self.subleaf = subleaf,
            (OperandType::CpuidUpperHalves, OperandValue::Bits(upper)) => self.upper_halves = upper,
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => self.layout = encoding.layout,
            _ => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }

    fn to_register(&self, role: Role) -> usize {
        match role {
            Role::Source => self.source,
            Role::Destination | Role::Base | Role::Implicit(_) | Role::Operand(_) => {
                panic!("no form of this path names a {role:?} register")
            }
        }
    }

    fn to_general(&self, number: usize) -> Register {
        match self.size {
            OperandSize::Bits16 => GENERAL_16[number],
            OperandSize::Bits32 => GENERAL_32[number],
            OperandSize::Bits64 => GENERAL_64[number],
        }
    }

    fn to_code(&self, [bits16, bits32, bits64]: [Code; 3]) -> Code {
        match self.size {
            OperandSize::Bits16 => bits16,
            OperandSize::Bits32 => bits32,
            OperandSize::Bits64 => bits64,
        }
    }
}

/// The form's bytes with the chosen operands, at the code address.
fn encode(instruction: Instruction, choice: &Choice) -> Vec<u8> {
    let encoded = match instruction {
        Instruction::PushGprv50 => Encoded::with1(
            choice.to_code([Code::Push_r16, Code::Push_r16, Code::Push_r64]),
            choice.to_general(choice.source),
        ),
        Instruction::Ud2 => Ok(Encoded::with(Code::Ud2)),
        Instruction::Int3 => Ok(Encoded::with(Code::Int3)),
        Instruction::Cpuid => Ok(Encoded::with(Code::Cpuid)),
        other => match find_privileged_codes(other) {
            Some(codes) => Ok(Encoded::with(match choice.size {
                OperandSize::Bits64 => codes[codes.len() - 1],
                _ => codes[0],
            })),
            None => panic!("{other:?} is not a modeled form"),
        },
    };
    let encoded = encoded.unwrap_or_else(|error| panic!("{instruction:?} takes its operands: {error}"));
    integer::encode_at_code_address(&encoded)
}

/// Whether the domain models the form's operands.
fn is_modeled(instruction: Instruction) -> bool {
    find_integer_form(instruction).is_some()
        || find_extension_form(instruction).is_some()
        || find_fixed_form(instruction).is_some()
        || find_mmx_form(instruction).is_some()
        || find_x87_form(instruction).is_some()
        || find_stack_form(instruction).is_some()
        || find_vector_form(instruction).is_some()
        || find_avx_form(instruction).is_some()
        || find_float_form(instruction).is_some()
        || find_branch_form(instruction).is_some()
        || find_sse_form(instruction).is_some()
        || find_evex_form(instruction).is_some()
        || find_privileged_codes(instruction).is_some()
        || find_opmask_form(instruction).is_some()
        || find_evex_integer_form(instruction).is_some()
        || find_string_form(instruction).is_some()
        || find_system_form(instruction).is_some()
        || find_mmx_convert_form(instruction).is_some()
        || find_descriptor_form(instruction).is_some()
        || find_reciprocal_form(instruction).is_some()
        || find_segment_form(instruction).is_some()
        || find_optional_form(instruction).is_some()
        || find_state_form(instruction).is_some()
        || find_amx_form(instruction).is_some()
        || matches!(instruction, Instruction::PushGprv50 | Instruction::Ud2 | Instruction::Int3 | Instruction::Cpuid)
}

/// The state of a form not yet modeled: its sample encoding at the code page's start, executable, and every register
/// as a modeled form's state fills those it does not name; JCXZ, which has no encoding, gets an empty code page.
fn build_sample_state(instruction: Instruction) -> ExecutionState<'static, X86_64> {
    let processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let readable_and_executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    ExecutionState {
        processor,
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: CODE_ADDRESS..CODE_ADDRESS + PAGE_BYTES,
                permission: readable_and_executable,
            },
            MemoryDelta::ValueAssignment {
                start: CODE_ADDRESS,
                bytes: Cow::Borrowed(SAMPLE_ENCODINGS[instruction as usize]),
            },
        ],
    }
}

/// The codes of a form of `PRIVILEGED_FORMS`, 32 bits first.
fn find_privileged_codes(instruction: Instruction) -> Option<&'static [Code]> {
    PRIVILEGED_FORMS.iter().find(|(form, _)| *form == instruction).map(|(_, codes)| *codes)
}

/// The complement of the address's low byte inside the data pages, zero elsewhere.
pub fn to_data_byte(address: u64) -> u8 {
    if (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).contains(&address) {
        !(address as u8)
    } else {
        0
    }
}

/// The data pages' bytes and permissions.
fn to_data_pages(lower: PagePermission, upper: PagePermission) -> [MemoryDelta<'static, X86_64>; 3] {
    [
        MemoryDelta::ValueAssignment {
            start: LOWER_DATA_ADDRESS,
            bytes: Cow::Borrowed(&DATA_PAGES),
        },
        MemoryDelta::PermissionAssignment {
            addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS,
            permission: lower,
        },
        MemoryDelta::PermissionAssignment {
            addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES,
            permission: upper,
        },
    ]
}

fn to_access_address(placement: AccessPlacement, access_bytes: u64) -> u64 {
    match placement {
        AccessPlacement::LowerPageStart => LOWER_DATA_ADDRESS,
        AccessPlacement::Misaligned => LOWER_DATA_ADDRESS + 1,
        AccessPlacement::EndsAtBoundary => UPPER_DATA_ADDRESS - access_bytes,
        AccessPlacement::StraddlesBoundary => UPPER_DATA_ADDRESS - access_bytes / 2,
        AccessPlacement::UpperPageStart => UPPER_DATA_ADDRESS,
        AccessPlacement::NonCanonical => NON_CANONICAL_ADDRESS,
    }
}

/// The accumulator CMPXCHG compares with the value found: equal to it or not in the operand's bytes, and set above
/// them, which a narrower form must leave as it was or clear.
pub const fn to_accumulator(comparison: Comparison, found: u64, access_bytes: u64) -> u64 {
    let compared = match comparison {
        Comparison::Equal => found,
        Comparison::Differs => found ^ 1,
    };
    (compared & to_mask(access_bytes)) | (ABOVE_LOW_HALF & !to_mask(access_bytes))
}

const fn to_mask(length: u64) -> u64 {
    if length >= 8 { u64::MAX } else { (1 << (length * 8)) - 1 }
}
