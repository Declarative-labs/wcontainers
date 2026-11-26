//! The inputs of the ARM64 instructions.
//!
//! Every state holds the instruction on a page of its own and, for an instruction that reads or writes memory,
//! two data pages right above it whose permissions are operands. Each data byte holds the complement of its
//! address's low byte, so that no loaded value equals an address near it, or for an instruction that may write
//! memory, all zeros or all ones in some states.
//!
//! Each critical value comes from the kind of the operand, never from the instruction it belongs to. Every piece of
//! state an instruction may write without reading it takes all zeros and all ones before it, whatever the other
//! operands hold: NZCV, each cumulative flag of FPSR alone too, the data pages, and its destination registers, by
//! the register each register number names.

#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;
use core::convert::Infallible;

use itertools::Itertools;

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::branch::{BranchOperation, to_branch_operation};
use wcontainers_processor_architecture_arm64::cryptography::to_cryptography_operation;
use wcontainers_processor_architecture_arm64::encoding::{Field, IMM16, RD, RDVL_IMM6, RN, RT};
use wcontainers_processor_architecture_arm64::floating_point::scalable_elements::Conversion;
use wcontainers_processor_architecture_arm64::floating_point::{
    ALTERNATIVE_HALF, DEFAULT_NAN, DIVIDE_BY_ZERO, FLUSH_HALF_TO_ZERO, FLUSH_TO_ZERO,
    FloatingPointOperation, INEXACT, INPUT_DENORMAL, INVALID_OPERATION, OVERFLOW, Precision,
    ROUNDING_MODE_BITS, ROUNDING_MODE_SHIFT, UNDERFLOW, to_floating_point_operation,
};
use wcontainers_processor_architecture_arm64::load_store::scalable_access::to_scalable_access;
use wcontainers_processor_architecture_arm64::load_store::scalable_element_access::to_element_access;
use wcontainers_processor_architecture_arm64::load_store::{
    AccessLayout, AtomicOperation, Transfer,
};
use wcontainers_processor_architecture_arm64::memory_tagging::TagAccessInstruction;
use wcontainers_processor_architecture_arm64::operand::{
    Operand, OperandAccess, RegisterThirtyOne, RegisterWidth, Signedness,
};
use wcontainers_processor_architecture_arm64::pointer_authentication::reads_link_register;
use wcontainers_processor_architecture_arm64::scalable_floating_point::{Lanes, VectorRole};
use wcontainers_processor_architecture_arm64::simd::to_vector_operation;
use wcontainers_processor_architecture_arm64::sve::{
    GeneralValues, PredicateValues, ScalableOperand, VectorValues, to_scalable_instruction,
};
use wcontainers_processor_architecture_arm64::system_instruction::{
    SYSTEM_INSTRUCTION, to_system_instruction_bytes,
};
use wcontainers_processor_architecture_arm64::system_register::SYSTEM_REGISTER;
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, DocumentedInstruction, GENERAL_PURPOSE_REGISTERS,
    INSTRUCTION_SIZE, Instruction, InstructionClass, InstructionFetch, LoadStoreInstruction,
    PagePermission, Processor, StructureInstruction, User, VECTOR_REGISTERS, VIRTUAL_ADDRESS_BITS,
    ZERO_PROCESSOR, to_implicit_inputs, writes_condition_flags,
};

pub mod advanced_simd;
mod memory_tagging;
mod scalable;
mod scalable_element_access;
mod structure;
pub mod sve_remaining;
#[cfg(test)]
mod tests;

/// The 4 KiB translation granule the executors use.
const PAGE_BYTES: u64 = 1 << 12;
/// The first page above the page at zero, which stays unmapped as on Linux.
const CODE_ADDRESS: u64 = PAGE_BYTES;
const LOWER_DATA_ADDRESS: u64 = CODE_ADDRESS + PAGE_BYTES;
const UPPER_DATA_ADDRESS: u64 = LOWER_DATA_ADDRESS + PAGE_BYTES;
/// A field this narrow gets every value it can hold.
const EXHAUSTIVE_FIELD_BITS: u32 = 4;
/// The width of immr and imms in a bitmask immediate, above which a 64-bit encoding has N.
const BITMASK_FIELD_BITS: u32 = 6;
/// The imms of a bitmask immediate whose element is two bits: the smallest element, the run of one bit.
const SMALLEST_ELEMENT_IMMS: u32 = 0b11_1100;
/// A field this narrow can name any bit of a register, so it gets the highest bit of each element size.
const BIT_NUMBER_FIELD_BITS: u32 = 6;
/// The highest bit of a byte, a halfword, a word and a doubleword.
const ELEMENT_TOP_BITS: [u32; 4] = [u8::BITS - 1, u16::BITS - 1, u32::BITS - 1, u64::BITS - 1];
/// The second ordinary register beside X0, which is also the link register a branch with link writes.
const LINK_REGISTER: u32 = 30;
/// What an address may hold above the 48 bits a program can use: nothing; a tag in the top byte, both nibbles set and
/// different, so that a fault address shows which it keeps; bits 48 to 55; and the upper half's bits.
const ADDRESS_HIGH_BITS: [u64; 4] = [
    0,
    0xA5 << ADDRESS_TAG_SHIFT,
    0xFF << VIRTUAL_ADDRESS_BITS,
    u64::MAX << VIRTUAL_ADDRESS_BITS,
];
/// Where an address's tag starts, the top byte that TBI0 leaves out.
const ADDRESS_TAG_SHIFT: u32 = u64::BITS - u8::BITS;
/// FPSR's cumulative saturation flag, which no scalar floating-point instruction sets: every state that gives FPSR no
/// value of its own holds it.
const SATURATION: u64 = 1 << 27;
/// FPSR's cumulative flags, each of which an instruction only ever sets.
const STICKY_FLAGS: [u64; 7] = [
    INVALID_OPERATION,
    DIVIDE_BY_ZERO,
    OVERFLOW,
    UNDERFLOW,
    INEXACT,
    INPUT_DENORMAL,
    SATURATION,
];
/// The vector register beside V0, which holds zero where no operand names it, as X30 does.
const SECOND_VECTOR_REGISTER: usize = 1;
/// The predicate beside P0, which holds zero in the state of an SVE instruction, as V1 does.
const SECOND_PREDICATE_REGISTER: usize = 1;
/// The bytes FEAT_LSE2 lets an unaligned ordered or atomic access lie anywhere inside.
const SINGLE_COPY_ATOMIC_BYTES: u64 = 16;
/// Half of an SVE vector's bytes at a 128-bit vector length.
const HALF_VECTOR_BYTES: u64 = 8;
/// A doubleword's bytes: a boundary the alignment rules name besides an access's own size and 16 bytes.
const DOUBLEWORD_BYTES: u64 = u64::BITS as u64 / u8::BITS as u64;
/// Ordinary registers, as many as an instruction names, so that every register operand can name one of its own: even
/// numbers, so that a pair's second register, the next number, is none of them.
const DISTINCT_REGISTER_NUMBERS: [u32; 4] = [0, 2, 4, 6];
/// A governing predicate of an SVE instruction on vectors: none, all and alternate elements of each size, the first
/// alone, the bits of no element wider than a byte, the top byte's bit, and the upper half.
const GOVERNING_PREDICATES: [u16; 9] = [
    0x0000, 0xFFFF, 0x5555, 0x1111, 0x0101, 0xAAAA, 0x0001, 0x8000, 0xFF00,
];
/// A predicate a predicate instruction reads: besides the governing ones, the last element of each size alone, the
/// bits of no element wider than a halfword or a word, each half, and the middle elements.
const OPERAND_PREDICATES: [u16; 16] = [
    0x0000, 0xFFFF, 0x0001, 0x8000, 0x0100, 0x1000, 0x4000, 0x5555, 0xAAAA, 0x1111, 0x0101, 0xEEEE,
    0xFEFE, 0x00FF, 0xFF00, 0x0FF0,
];
/// FFR before an instruction that reads it: [monotonic](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/WRFFR--Write-the-first-fault-register-#:~:text=monotonic) values, the only ones a state can hold, since
/// writing any other leaves FFR unpredictable.
const FIRST_FAULT_VALUES: [u16; 6] = [0x0000, 0x0001, 0x0003, 0x00FF, 0x7FFF, 0xFFFF];
/// The small values a WHILE's counter and limit take, so that two of them lie each element count apart and one more or
/// less.
const SMALL_BOUNDS: [u64; 11] = [0, 1, 2, 3, 4, 5, 8, 9, 15, 16, 17];
/// What lies above the low word of a 32-bit value a WHILE or a saturating step reads, which it must leave out.
const UPPER_WORD_ONES: u64 = 0xFFFF_FFFF << u32::BITS;
/// How far inside a range's end a saturating step's register lies: at it, one, a vector's bytes and a large count in.
const SATURATION_DISTANCES: [u64; 4] = [0, 1, 16, 256];

/// An MRS reaches one of the registers its field names: each the state holds is an operand.
const SYSTEM_REGISTER_READ: [OperandType; 6] = [
    OperandType::SystemRegister(SYSTEM_REGISTER),
    OperandType::RegisterNumber(RT),
    OperandType::ConditionFlags,
    OperandType::FloatingPointControl(ControlReads::Conversion),
    OperandType::StatusFlags,
    OperandType::SoftwareThreadId,
];
/// An MSR (register) writes one of them from its register's value.
const SYSTEM_REGISTER_WRITE: [OperandType; 7] = [
    OperandType::SystemRegister(SYSTEM_REGISTER),
    OperandType::RegisterNumber(RT),
    OperandType::RegisterValue {
        field: RT,
        width: RegisterWidth::Bits64,
        thirty_one: RegisterThirtyOne::ZeroRegister,
    },
    OperandType::ConditionFlags,
    OperandType::FloatingPointControl(ControlReads::Conversion),
    OperandType::StatusFlags,
    OperandType::SoftwareThreadId,
];

/// The option bit of SRLS.
const SCOPED_RELEASE_OPTION: Field = Field::new(5, 1);

#[derive(Clone, Copy, Default, Debug)]
pub struct Arm64InputDomain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandType {
    /// The register an encoding field names.
    RegisterNumber(Field),
    /// The value of the register an encoding field names, read at the width.
    RegisterValue {
        field: Field,
        width: RegisterWidth,
        thirty_one: RegisterThirtyOne,
    },
    /// The value of a 64-bit register an instruction takes as an address, a branch target or a pointer: also with bits
    /// 48 to 55 set, in the upper half and tagged.
    Address {
        field: Field,
        thirty_one: RegisterThirtyOne,
    },
    /// The value of a register an instruction takes as an address without naming it: X30 of RETAA and XPACLRI.
    ImplicitAddress(u32),
    UnsignedImmediate(Field),
    SignedImmediate(Field),
    /// The amount of a shifted register, which is unallocated from the register's width up.
    ShiftAmount {
        field: Field,
        width: RegisterWidth,
    },
    /// N, immr and imms; a field of 12 bits has no N.
    BitmaskImmediate(Field),
    /// A signed offset split over fields, the most significant first.
    ProgramCounterOffset(&'static [Field]),
    /// The value of a register a compare and swap compares with the `bytes` bytes it reads, or of the pair of
    /// registers from it.
    Comparand {
        field: Field,
        bytes: u64,
        pair: bool,
    },
    /// An unsigned immediate split over fields, the most significant first.
    SplitImmediate(&'static [Field]),
    ConditionFlags,
    /// A SIMD and floating-point register an encoding field names, none of which has a special role.
    VectorRegisterNumber(Field),
    /// The first register of a list of consecutive SIMD and floating-point registers, which wraps after V31.
    VectorListNumber(Field),
    /// The low `precision` bits of the SIMD and floating-point register an encoding field names.
    FloatingPointValue {
        field: Field,
        precision: Precision,
    },
    /// FPCR, as an instruction of these precisions reads it.
    FloatingPointControl(ControlReads),
    /// Where the first byte an instruction reads or writes lies, around the boundary of the two data pages and of
    /// 8 and 16 aligned bytes.
    AccessPlacement,
    /// The top byte of the base register of a load or store.
    AddressTag,
    DataPagePermission(DataPage),
    /// Where the instruction lies in its page, for an instruction that reads its own address.
    CodePlacement,
    /// The value of a general-purpose register an instruction converts to floating point of the precision.
    ConvertedInteger {
        field: Field,
        width: RegisterWidth,
        precision: Precision,
    },
    /// The low `precision` bits of the SIMD and floating-point register an instruction converts to an integer of the
    /// width.
    ConvertedFloatingPoint {
        field: Field,
        precision: Precision,
        width: RegisterWidth,
    },
    /// NZCV before an instruction that writes it without reading it.
    WrittenConditionFlags,
    /// FPSR before an instruction that may raise its cumulative flags.
    StatusFlags,
    /// What the data pages hold before an instruction that may write them.
    MemoryFill,
    /// The system register an MRS or MSR names, by the encoding its field holds.
    SystemRegister(Field),
    /// TPIDR_EL0, which an MRS or MSR may read or write.
    SoftwareThreadId,
    /// The low `bytes` bytes of the SIMD and floating-point register a store moves.
    VectorValue {
        field: Field,
        bytes: u32,
    },
    /// The low `bytes` bytes of the register `place` after the first of a list a store moves.
    ListedVectorValue {
        field: Field,
        place: u32,
        bytes: u32,
    },
    /// The whole word of an instruction whose operands no model reads: a word LLVM decodes as the instruction, or its
    /// fixed bits where the list has none.
    Word(u32),
    /// An operand of an Advanced SIMD instruction of the architecture's `advanced_simd` module.
    AdvancedSimd(advanced_simd::SimdOperand),
    /// The SVE predicate register an encoding field names.
    PredicateRegisterNumber(Field),
    /// The predicate that chooses the elements an SVE operation on vectors acts on.
    GoverningPredicate(Field),
    /// A predicate a predicate instruction reads.
    PredicateValue(Field),
    /// The Z register after the one an encoding field names, the second of a pair.
    NextVectorValue(Field),
    /// A Z register whose elements are shift amounts or indices.
    ShiftAmountVector(Field),
    /// The pattern of an SVE count, each of whose values names its own case.
    Pattern(Field),
    /// The value of a register a WHILE compares.
    RegisterBound(Field),
    /// The value of a register a saturating count steps.
    RegisterNearSaturation(Field),
    /// FFR before an instruction that reads it.
    FirstFaultValue,
    /// FFR before an instruction that writes it without reading it.
    WrittenFirstFault,
    /// The Z registers an SVE operand names, `registers` consecutive ones from the field's.
    ScalableVectorNumber {
        field: Field,
        registers: u32,
    },
    /// The whole Z register `offset` registers after the one the field names, wrapping after Z31.
    ScalableVectorValue {
        field: Field,
        offset: u32,
        values: ScalableValues,
    },
    /// The P register an SVE operand names.
    PredicateNumber(Field),
    /// The value of the P register the field names.
    ScalablePredicateValue(Field),
    /// An immediate split over fields, the most significant first, which takes every value it can hold.
    EveryValue(&'static [Field]),
    /// The Z register of offsets of an SVE access, its elements of `element_bits` holding offsets of `offset_bits`,
    /// in elements of memory where scaled.
    ElementOffsets {
        field: Field,
        element_bits: u32,
        memory_bytes: u32,
        offset_bits: u32,
        scaled: bool,
    },
    /// The Z register of addresses of an SVE access, its elements of `element_bits` each an address.
    ElementAddresses {
        field: Field,
        element_bits: u32,
        memory_bytes: u32,
    },
    /// Where an SVE store of two or four registers lies: also across the boundary inside a vector.
    ScalableStructurePlacement,
    /// Where an SVE access of elements of more than one byte lies: also across the boundary inside an element.
    ElementPlacement,
    /// An operand of an SVE instruction of the architecture's `sve_remaining` module that has a kind of its own.
    Remaining(sve_remaining::RemainingOperand),
}

/// What the elements of a Z register an SVE instruction reads hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableValues {
    /// Bits, as a vector register's.
    Integer,
    /// The bits of one register of a structure store: fewer than `Integer`, since each register stores its own bytes.
    Structure,
    /// Floating-point values in the lanes given, or where none, in the lanes of every element size, of which an
    /// instruction takes those of the size its size field gives.
    FloatingPoint {
        lanes: Option<Lanes>,
        role: VectorRole,
    },
    /// What a conversion reads, with the bounds and roundings of what it converts to.
    Converted(Conversion),
}

/// What every byte of the data pages holds before the instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryFill {
    /// The complement of the byte's address's low byte, so that no loaded value equals an address near it.
    Pattern,
    Zeros,
    Ones,
}

/// Which of FPCR's flush controls an instruction can read, besides the rounding mode and default NaN.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlReads {
    /// FZ, for single and double precision.
    SingleOrDouble,
    /// FZ16, and FZ, which should be left out.
    Half,
    /// FZ, FZ16, which a conversion should leave out, and AHP.
    Conversion,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataPage {
    Lower,
    Upper,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessPlacement {
    LowerPageStart,
    /// One byte past the lower page's start: misaligned for every access of two bytes or more, inside 16 aligned
    /// bytes for every access of 15 bytes or fewer.
    ByteAfterLowerPageStart,
    /// Half of the access on each side of the first 8 aligned bytes' end: inside 16 aligned bytes, across 8.
    AcrossEightBytes,
    /// Half of the access on each side of the first 16 aligned bytes' end.
    AcrossSixteenBytes,
    EndsAtBoundary,
    StraddlesBoundary,
    UpperPageStart,
    /// Across the boundary half a vector below the middle of the access, inside a vector of a structure whose middle
    /// is a vector's end.
    StraddlesInsideVector,
    /// Across the boundary half an element below the middle of an access of elements, inside the element there.
    StraddlesInsideElement,
}

/// Whether a compared register holds what the memory it is compared with holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comparison {
    Equal,
    /// Equal in the bytes compared, with ones in every bit above them.
    EqualInComparedBytes,
    /// Equal in every byte compared but the highest, which is the highest of the second register of a pair.
    DiffersInHighestByte,
    Unequal,
}

/// Where an instruction lies in its page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodePlacement {
    PageStart,
    /// The last instruction of the page, whose next instruction is on the page after.
    PageEnd,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandValue {
    RegisterNumber(u32),
    /// A register's value, or an immediate's bits.
    Bits(u64),
    ConditionFlags(ConditionFlags),
    AccessPlacement(AccessPlacement),
    PagePermission(PagePermission),
    Comparison(Comparison),
    CodePlacement(CodePlacement),
    MemoryFill(MemoryFill),
    /// The low bytes of a SIMD and floating-point register.
    Vector(u128),
    Predicate(u16),
    /// A whole Z register, the lanes its values lie in.
    ScalableVector {
        bits: u128,
        lanes: Lanes,
    },
}

impl Arm64InputDomain {
    /// Whether an operand's critical value is one the word gives its operand: a floating-point source of an SVE
    /// instruction whose size field sizes its elements takes only the values of that size.
    pub fn is_value_of_word(
        &self,
        instruction: Instruction<Infallible>,
        operand_type: OperandType,
        value: OperandValue,
        word: u32,
    ) -> bool {
        scalable::is_value_of_word(instruction, operand_type, value, word)
            && sve_remaining::is_value_of_word(instruction, operand_type, value, word)
    }

    /// The numbers of ordinary registers of the operand's kind enough for every register operand of an instruction to
    /// name its own, which the critical values alone do not give: one register's value is a critical value only
    /// beside the others'. None for an operand that names no register.
    pub fn generate_distinct_register_numbers(
        &self,
        operand_type: OperandType,
    ) -> impl Iterator<Item = OperandValue> {
        let numbers: &[u32] = match operand_type {
            OperandType::RegisterNumber(_)
            | OperandType::VectorRegisterNumber(_)
            | OperandType::PredicateRegisterNumber(_)
            | OperandType::ScalableVectorNumber { .. }
            | OperandType::PredicateNumber(_)
            | OperandType::VectorListNumber(_) => &DISTINCT_REGISTER_NUMBERS,
            _ => &[],
        };
        numbers.iter().copied().map(OperandValue::RegisterNumber)
    }
}

impl InstructionInputDomain<Arm64<User>> for Arm64InputDomain {
    type OperandType = OperandType;
    type OperandValue = OperandValue;
    type OperandTypes<'domain> = vec::IntoIter<OperandType>;
    type CriticalValues<'domain> = vec::IntoIter<OperandValue>;

    /// The operands of the assembly syntax in its order, then the state the instruction reads besides them; for an
    /// instruction whose operands no model reads, its whole word.
    fn list_operand_types(
        &self,
        instruction: Instruction<Infallible>,
    ) -> vec::IntoIter<OperandType> {
        use OperandType::{RegisterNumber, SignedImmediate, UnsignedImmediate};
        if let Some(operand_types) = advanced_simd::list_operand_types(instruction) {
            return operand_types.into_iter();
        }
        if let Some(operand_types) = sve_remaining::list_operand_types(instruction) {
            return operand_types.into_iter();
        }
        if let Some(scalable) = to_scalable_instruction(instruction) {
            return to_scalable_operand_types(scalable.operands).into_iter();
        }
        if let Some(access) = to_element_access(instruction) {
            return scalable_element_access::list_operand_types(access).into_iter();
        }
        if let Some(operand_types) = scalable::list_scalable_operand_types(instruction) {
            return operand_types.into_iter();
        }
        if let Some(structure) = StructureInstruction::new(instruction) {
            return structure::to_structure_operand_types(structure).into_iter();
        }
        if let Some(access) = TagAccessInstruction::new(instruction) {
            return memory_tagging::to_tag_access_operand_types(access).into_iter();
        }
        // SRLS's one operand, the bit at 5 that selects STRM, which the list leaves without a meaning.
        if instruction == Instruction::SrlsHiHints {
            return vec![UnsignedImmediate(SCOPED_RELEASE_OPTION)].into_iter();
        }
        if DocumentedInstruction::new(instruction).is_some()
            || LoadStoreInstruction::new(instruction).is_some()
        {
            return to_listed_operand_types(instruction).into_iter();
        }
        let operand_types = match instruction {
            Instruction::Mrs => SYSTEM_REGISTER_READ.to_vec(),
            Instruction::MsrRegister => SYSTEM_REGISTER_WRITE.to_vec(),
            Instruction::Brk => vec![UnsignedImmediate(IMM16)],
            Instruction::Rdvl => vec![RegisterNumber(RD), SignedImmediate(RDVL_IMM6)],
            Instruction::MrsCtrEl0 => vec![RegisterNumber(RT)],
            // The hint number as CRm and op2, and X30, which hint 7 (XPACLRI) strips.
            Instruction::Hint => vec![
                UnsignedImmediate(Field::new(8, 4)),
                UnsignedImmediate(Field::new(5, 3)),
                OperandType::ImplicitAddress(LINK_REGISTER),
            ],
            // op1, CRm and op2, and the flags CFINV, XAFLAG and AXFLAG read.
            Instruction::MsrImmediate => vec![
                UnsignedImmediate(Field::new(16, 3)),
                UnsignedImmediate(Field::new(8, 4)),
                UnsignedImmediate(Field::new(5, 3)),
                OperandType::ConditionFlags,
            ],
            // Every encoding of op1, CRn, CRm and op2, and the register that holds an address where it reaches one.
            Instruction::Sys => vec![
                OperandType::SystemRegister(SYSTEM_INSTRUCTION),
                RegisterNumber(RT),
                OperandType::AccessPlacement,
                OperandType::AddressTag,
                OperandType::DataPagePermission(DataPage::Lower),
                OperandType::DataPagePermission(DataPage::Upper),
                OperandType::MemoryFill,
            ],
            Instruction::Sysl => vec![
                OperandType::SystemRegister(SYSTEM_INSTRUCTION),
                RegisterNumber(RT),
            ],
            _ => vec![OperandType::Word(to_example_word(instruction))],
        };
        operand_types.into_iter()
    }

    fn generate_critical_values(&self, operand_type: OperandType) -> vec::IntoIter<OperandValue> {
        let values: Vec<OperandValue> = match operand_type {
            // Two ordinary registers, so that one can appear twice, the second also the link register; and the
            // zero register or stack pointer.
            OperandType::RegisterNumber(_) => [0, LINK_REGISTER, GENERAL_PURPOSE_REGISTERS as u32]
                .map(OperandValue::RegisterNumber)
                .to_vec(),
            OperandType::RegisterValue { width, .. } => to_register_values(width),
            OperandType::Address { .. } | OperandType::ImplicitAddress(_) => {
                let mut values = to_register_values(RegisterWidth::Bits64);
                // The lowest bit past the addressable ones alone, besides the high bits a data address takes.
                let lowest_past = 1 << VIRTUAL_ADDRESS_BITS;
                values.extend(
                    ADDRESS_HIGH_BITS[1..]
                        .iter()
                        .chain([&lowest_past])
                        .map(|high| OperandValue::Bits(high | CODE_ADDRESS)),
                );
                values
            }
            OperandType::ConvertedInteger {
                width, precision, ..
            } => to_converted_integer_values(width, precision),
            OperandType::ConvertedFloatingPoint {
                precision, width, ..
            } => to_converted_floating_point_values(precision, width),
            // Every encoding the field can hold, each op0, op1, CRn, CRm and op2 with every other.
            OperandType::SystemRegister(field) => (0..1_u32 << field.width)
                .map(|encoding| OperandValue::Bits(u64::from(encoding)))
                .collect(),
            OperandType::Word(word) => [OperandValue::Bits(u64::from(word))].to_vec(),
            OperandType::AdvancedSimd(operand) => advanced_simd::generate_critical_values(operand),
            OperandType::Remaining(operand) => sve_remaining::generate_critical_values(operand),
            OperandType::SoftwareThreadId => to_register_values(RegisterWidth::Bits64),
            OperandType::VectorValue { bytes, .. }
            | OperandType::ListedVectorValue { bytes, .. } => to_vector_values(bytes),
            // The register holding zero, and the last, after which the list wraps.
            OperandType::VectorListNumber(_) => [0, 1, (VECTOR_REGISTERS - 1) as u32]
                .map(OperandValue::RegisterNumber)
                .to_vec(),
            OperandType::WrittenConditionFlags => [ConditionFlags::empty(), ConditionFlags::all()]
                .map(OperandValue::ConditionFlags)
                .to_vec(),
            // None, all, and each alone.
            OperandType::StatusFlags => [0, STICKY_FLAGS.iter().fold(0, |all, flag| all | flag)]
                .into_iter()
                .chain(STICKY_FLAGS)
                .map(OperandValue::Bits)
                .collect(),
            OperandType::MemoryFill => [MemoryFill::Pattern, MemoryFill::Zeros, MemoryFill::Ones]
                .map(OperandValue::MemoryFill)
                .to_vec(),
            OperandType::VectorRegisterNumber(_) | OperandType::PredicateRegisterNumber(_) => {
                [0, 1].map(OperandValue::RegisterNumber).to_vec()
            }
            OperandType::GoverningPredicate(_) => {
                GOVERNING_PREDICATES.map(OperandValue::Predicate).to_vec()
            }
            OperandType::PredicateValue(_) => {
                OPERAND_PREDICATES.map(OperandValue::Predicate).to_vec()
            }
            OperandType::FirstFaultValue => {
                FIRST_FAULT_VALUES.map(OperandValue::Predicate).to_vec()
            }
            OperandType::WrittenFirstFault => {
                [u16::MIN, u16::MAX].map(OperandValue::Predicate).to_vec()
            }
            OperandType::NextVectorValue(_) => to_vector_values(size_of::<u128>() as u32),
            OperandType::ShiftAmountVector(_) => {
                let mut values = to_vector_values(size_of::<u128>() as u32);
                values.extend(
                    to_shift_amount_vectors()
                        .into_iter()
                        .map(OperandValue::Vector),
                );
                values
            }
            OperandType::Pattern(field) => (0..1 << field.width).map(OperandValue::Bits).collect(),
            OperandType::RegisterBound(_) => to_bound_values(),
            OperandType::RegisterNearSaturation(_) => to_saturation_values(),
            OperandType::FloatingPointValue { precision, .. } => {
                to_floating_point_values(precision)
            }
            OperandType::FloatingPointControl(reads) => to_control_values(reads),
            OperandType::ShiftAmount { field, width } => {
                let bits = u64::from(width.to_bits());
                with_each_bit_alone(field.width, [0, 1, bits - 1, bits])
            }
            OperandType::BitmaskImmediate(field) => {
                let all_ones = (1 << BITMASK_FIELD_BITS) - 1;
                let to_bits = |values: Vec<OperandValue>| -> Vec<u32> {
                    values
                        .into_iter()
                        .map(|value| match value {
                            OperandValue::Bits(bits) => bits as u32,
                            _ => unreachable!("bits"),
                        })
                        .collect()
                };
                let elements = to_bits(with_each_bit_alone(
                    BITMASK_FIELD_BITS,
                    [
                        0,
                        1,
                        u64::from(SMALLEST_ELEMENT_IMMS),
                        all_ones - 1,
                        all_ones,
                    ],
                ));
                let rotations = to_bits(with_each_bit_alone(BITMASK_FIELD_BITS, [0, 1, all_ones]));
                let n_values: &[u32] = if field.width > 2 * BITMASK_FIELD_BITS {
                    &[0, 1]
                } else {
                    &[0]
                };
                let mut values = Vec::new();
                for n in n_values {
                    for &immr in &rotations {
                        for &imms in &elements {
                            let bits = (n << (2 * BITMASK_FIELD_BITS))
                                | (immr << BITMASK_FIELD_BITS)
                                | imms;
                            values.push(OperandValue::Bits(u64::from(bits)));
                        }
                    }
                }
                values
            }
            OperandType::UnsignedImmediate(field) | OperandType::SignedImmediate(field)
                if field.width <= EXHAUSTIVE_FIELD_BITS =>
            {
                (0..1 << field.width).map(OperandValue::Bits).collect()
            }
            OperandType::UnsignedImmediate(field) if field.width <= BIT_NUMBER_FIELD_BITS => {
                with_each_bit_alone(field.width, to_bit_numbers(field.width))
            }
            OperandType::UnsignedImmediate(field) => {
                with_each_bit_alone(field.width, [0, 1, low_ones(field.width)])
            }
            OperandType::SignedImmediate(field) => to_signed_critical_values(field.width),
            OperandType::SplitImmediate(fields) => {
                let width: u32 = fields.iter().map(|field| field.width).sum();
                with_each_bit_alone(
                    width,
                    to_bit_numbers(width).into_iter().chain([low_ones(width)]),
                )
            }
            OperandType::ProgramCounterOffset(fields) => {
                to_signed_critical_values(fields.iter().map(|field| field.width).sum())
            }
            OperandType::ConditionFlags => ConditionFlags::all()
                .iter()
                .powerset()
                .map(|subset| OperandValue::ConditionFlags(subset.into_iter().collect()))
                .collect(),
            OperandType::AccessPlacement => [
                AccessPlacement::LowerPageStart,
                AccessPlacement::ByteAfterLowerPageStart,
                AccessPlacement::AcrossEightBytes,
                AccessPlacement::AcrossSixteenBytes,
                AccessPlacement::EndsAtBoundary,
                AccessPlacement::StraddlesBoundary,
                AccessPlacement::UpperPageStart,
            ]
            .map(OperandValue::AccessPlacement)
            .to_vec(),
            OperandType::ScalableStructurePlacement => [
                AccessPlacement::LowerPageStart,
                AccessPlacement::ByteAfterLowerPageStart,
                AccessPlacement::AcrossEightBytes,
                AccessPlacement::AcrossSixteenBytes,
                AccessPlacement::EndsAtBoundary,
                AccessPlacement::StraddlesBoundary,
                AccessPlacement::UpperPageStart,
                AccessPlacement::StraddlesInsideVector,
            ]
            .map(OperandValue::AccessPlacement)
            .to_vec(),
            OperandType::ElementPlacement => [
                AccessPlacement::LowerPageStart,
                AccessPlacement::ByteAfterLowerPageStart,
                AccessPlacement::AcrossEightBytes,
                AccessPlacement::AcrossSixteenBytes,
                AccessPlacement::EndsAtBoundary,
                AccessPlacement::StraddlesBoundary,
                AccessPlacement::UpperPageStart,
                AccessPlacement::StraddlesInsideElement,
            ]
            .map(OperandValue::AccessPlacement)
            .to_vec(),
            OperandType::ElementOffsets {
                element_bits,
                memory_bytes,
                offset_bits,
                scaled,
                ..
            } => scalable_element_access::to_offset_values(
                element_bits,
                memory_bytes,
                offset_bits,
                scaled,
            ),
            OperandType::ElementAddresses {
                element_bits,
                memory_bytes,
                ..
            } => scalable_element_access::to_address_values(element_bits, memory_bytes),
            OperandType::Comparand { bytes, pair, .. } => {
                let register_bytes = if pair { bytes / 2 } else { bytes };
                let above = (register_bytes < u64::from(u64::BITS / u8::BITS))
                    .then_some(Comparison::EqualInComparedBytes);
                // A pair's highest byte is its second register's; a single byte has no byte below its highest.
                let highest =
                    (pair || register_bytes > 1).then_some(Comparison::DiffersInHighestByte);
                [
                    Some(Comparison::Equal),
                    above,
                    highest,
                    Some(Comparison::Unequal),
                ]
                .into_iter()
                .flatten()
                .map(OperandValue::Comparison)
                .collect()
            }
            OperandType::AddressTag => ADDRESS_HIGH_BITS.map(OperandValue::Bits).to_vec(),
            OperandType::CodePlacement => [CodePlacement::PageStart, CodePlacement::PageEnd]
                .map(OperandValue::CodePlacement)
                .to_vec(),
            // Two registers, the second holding zero, as the vector registers; and for a list, the last, whose next is Z0.
            OperandType::ScalableVectorNumber { registers, .. } => [0, 1]
                .into_iter()
                .chain((registers > 1).then_some(VECTOR_REGISTERS as u32 - 1))
                .map(OperandValue::RegisterNumber)
                .collect(),
            OperandType::ScalableVectorValue { values, .. } => {
                scalable::to_scalable_vector_values(values)
            }
            OperandType::PredicateNumber(_) => [0, 1].map(OperandValue::RegisterNumber).to_vec(),
            OperandType::ScalablePredicateValue(_) => scalable::to_predicate_values(),
            OperandType::EveryValue(fields) => {
                let width: u32 = fields.iter().map(|field| field.width).sum();
                (0..1 << width).map(OperandValue::Bits).collect()
            }
            OperandType::DataPagePermission(_) => {
                let mapped = |data_access| PagePermission::Mapped {
                    data_access,
                    instruction_fetch: InstructionFetch::Forbidden,
                };
                [
                    PagePermission::Unmapped,
                    mapped(DataAccess::Forbidden),
                    mapped(DataAccess::ReadOnly),
                    mapped(DataAccess::ReadWrite),
                ]
                .map(OperandValue::PagePermission)
                .to_vec()
            }
        };
        values.into_iter()
    }

    /// Every register no operand names holds the complement of its number, and so does the stack pointer, as number
    /// 31; but X30 and V1 hold zero, so that a register an instruction only writes holds all ones before it (X0, V0)
    /// in some states and zero (X30, V1) in others.
    ///
    /// A register named twice holds the value of its last register value operand; the zero register holds none.
    ///
    /// # Panics
    ///
    /// When the operands are not one value of each operand type the instruction lists, in its order.
    fn build_state(
        &self,
        instruction: Instruction<Infallible>,
        operands: &[OperandValue],
    ) -> ExecutionState<'static, Arm64<User>> {
        let typed: Vec<(OperandType, OperandValue)> = self
            .list_operand_types(instruction)
            .zip(operands.iter().copied())
            .collect();
        assert_eq!(
            typed.len(),
            operands.len(),
            "one value for each operand of {instruction:?}"
        );
        let word = encode_word(instruction, typed.iter().copied());
        let mut processor = Processor {
            general_purpose_registers: array::from_fn(|number| {
                if number == LINK_REGISTER as usize {
                    0
                } else {
                    !(number as u64)
                }
            }),
            stack_pointer: !(GENERAL_PURPOSE_REGISTERS as u64),
            program_counter: CODE_ADDRESS,
            vector_registers: array::from_fn(|number| {
                if number == SECOND_VECTOR_REGISTER {
                    0
                } else {
                    !(number as u128)
                }
            }),
            floating_point_status: SATURATION,
            ..ZERO_PROCESSOR
        };
        // The P registers of an SVE instruction as its Z registers are: each the complement of its number, but P1 zero.
        if to_scalable_instruction(instruction).is_some()
            || typed
                .iter()
                .any(|(operand_type, _)| matches!(operand_type, OperandType::PredicateNumber(_)))
        {
            processor.predicate_registers = array::from_fn(|number| {
                if number == SECOND_PREDICATE_REGISTER {
                    0
                } else {
                    !(number as u16)
                }
            });
        }
        let mut floating_point_values = Vec::new();
        let mut vector_values = Vec::new();
        let mut scalable_values = Vec::new();
        let mut predicate_values = Vec::new();
        let mut listed_vector_values = Vec::new();
        let mut placement = None;
        let mut tag = 0;
        let mut comparand = None;
        let mut fill = MemoryFill::Pattern;
        let mut lower = PagePermission::Unmapped;
        let mut upper = PagePermission::Unmapped;
        let mut register_values = Vec::new();
        let mut simd_values = Vec::new();
        let mut remaining_values = Vec::new();
        let mut next_vector_values = Vec::new();
        for (operand_type, operand) in typed {
            match operand_type {
                OperandType::GoverningPredicate(field) | OperandType::PredicateValue(field) => {
                    let OperandValue::Predicate(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    predicate_values.push((field, value));
                }
                OperandType::FirstFaultValue | OperandType::WrittenFirstFault => {
                    let OperandValue::Predicate(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.first_fault_register = value;
                }
                OperandType::NextVectorValue(field) => {
                    let OperandValue::Vector(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    next_vector_values.push((field, value));
                }
                OperandType::ShiftAmountVector(field) => {
                    let OperandValue::Vector(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    vector_values.push((field, size_of::<u128>() as u32, value));
                }
                OperandType::RegisterBound(field) | OperandType::RegisterNearSaturation(field) => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    register_values.push((field, RegisterThirtyOne::ZeroRegister, value));
                }
                OperandType::FloatingPointValue { field, precision }
                | OperandType::ConvertedFloatingPoint {
                    field, precision, ..
                } => {
                    let OperandValue::Bits(bits) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    floating_point_values.push((field, precision, bits));
                }
                OperandType::FloatingPointControl(_) => {
                    let OperandValue::Bits(bits) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.floating_point_control = bits;
                }
                OperandType::StatusFlags => {
                    let OperandValue::Bits(bits) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.floating_point_status = bits;
                }
                OperandType::MemoryFill => {
                    let OperandValue::MemoryFill(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    fill = chosen;
                }
                OperandType::SoftwareThreadId => {
                    let OperandValue::Bits(bits) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.software_thread_id = bits;
                }
                OperandType::VectorValue { field, bytes } => {
                    let OperandValue::Vector(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    vector_values.push((field, bytes, value));
                }
                OperandType::AdvancedSimd(simd_operand) => {
                    simd_values.push((simd_operand, operand))
                }
                OperandType::Remaining(remaining) => remaining_values.push((remaining, operand)),
                OperandType::ScalableVectorValue { field, offset, .. } => {
                    let (OperandValue::Vector(value)
                    | OperandValue::ScalableVector { bits: value, .. }) = operand
                    else {
                        reject_mismatch(operand_type, operand)
                    };
                    scalable_values.push((field, offset, value));
                }
                OperandType::ElementOffsets { field, .. }
                | OperandType::ElementAddresses { field, .. } => {
                    let OperandValue::Vector(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    scalable_values.push((field, 0, value));
                }
                OperandType::ScalablePredicateValue(field) => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    predicate_values.push((field, value as u16));
                }
                OperandType::ListedVectorValue {
                    field,
                    place,
                    bytes,
                } => {
                    let OperandValue::Vector(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    listed_vector_values.push((field, place, bytes, value));
                }
                // `encode_word` has put them in the word.
                OperandType::ScalableVectorNumber { .. }
                | OperandType::PredicateNumber(_)
                | OperandType::EveryValue(_)
                | OperandType::RegisterNumber(_)
                | OperandType::VectorRegisterNumber(_)
                | OperandType::PredicateRegisterNumber(_)
                | OperandType::Pattern(_)
                | OperandType::VectorListNumber(_)
                | OperandType::UnsignedImmediate(_)
                | OperandType::SignedImmediate(_)
                | OperandType::ShiftAmount { .. }
                | OperandType::BitmaskImmediate(_)
                | OperandType::ProgramCounterOffset(_)
                | OperandType::SplitImmediate(_)
                | OperandType::SystemRegister(_)
                | OperandType::Word(_) => {}
                OperandType::RegisterValue {
                    field, thirty_one, ..
                }
                | OperandType::Address { field, thirty_one } => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    register_values.push((field, thirty_one, value));
                }
                OperandType::ConvertedInteger { field, .. } => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    register_values.push((field, RegisterThirtyOne::ZeroRegister, value));
                }
                OperandType::ImplicitAddress(number) => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.general_purpose_registers[number as usize] = value;
                }
                OperandType::ConditionFlags | OperandType::WrittenConditionFlags => {
                    let OperandValue::ConditionFlags(flags) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.condition_flags = flags;
                }
                OperandType::Comparand { field, pair, .. } => {
                    let OperandValue::Comparison(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    comparand = Some((field, chosen, pair));
                }
                OperandType::AddressTag => {
                    let OperandValue::Bits(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    tag = chosen;
                }
                OperandType::CodePlacement => {
                    let OperandValue::CodePlacement(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.program_counter = match chosen {
                        CodePlacement::PageStart => CODE_ADDRESS,
                        CodePlacement::PageEnd => CODE_ADDRESS + PAGE_BYTES - INSTRUCTION_SIZE,
                    };
                }
                OperandType::AccessPlacement
                | OperandType::ScalableStructurePlacement
                | OperandType::ElementPlacement => {
                    let OperandValue::AccessPlacement(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    placement = Some(chosen);
                }
                OperandType::DataPagePermission(page) => {
                    let OperandValue::PagePermission(permission) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    match page {
                        DataPage::Lower => lower = permission,
                        DataPage::Upper => upper = permission,
                    }
                }
            }
        }
        // Register values go in once every register field is encoded, a floating-point value below the rest of its
        // register.
        for (field, precision, bits) in floating_point_values {
            let register = &mut processor.vector_registers[field.read(word) as usize];
            let mask = u128::MAX >> (u128::BITS - precision.to_bits());
            *register = (*register & !mask) | u128::from(bits);
        }
        for (field, bytes, value) in vector_values {
            let register = &mut processor.vector_registers[field.read(word) as usize];
            let mask = u128::MAX >> (u128::BITS - bytes * u8::BITS);
            *register = (*register & !mask) | (value & mask);
        }
        for (simd_operand, value) in simd_values {
            advanced_simd::apply(simd_operand, value, word, &mut processor);
        }
        for (remaining, value) in remaining_values {
            sve_remaining::apply(remaining, value, word, &mut processor);
        }
        for (field, value) in next_vector_values {
            let number = (field.read(word) as usize + 1) % VECTOR_REGISTERS;
            processor.vector_registers[number] = value;
        }
        for (field, value) in predicate_values {
            if let Some(register) = processor
                .predicate_registers
                .get_mut(field.read(word) as usize)
            {
                *register = value;
            }
        }
        for (field, offset, value) in scalable_values {
            let number = (field.read(word) + offset) as usize % VECTOR_REGISTERS;
            processor.vector_registers[number] = value;
        }
        for (field, place, bytes, value) in listed_vector_values {
            let number = (field.read(word) + place) as usize % VECTOR_REGISTERS;
            let mask = u128::MAX >> (u128::BITS - bytes * u8::BITS);
            let register = &mut processor.vector_registers[number];
            *register = (*register & !mask) | (value & mask);
        }
        for (field, thirty_one, value) in register_values {
            match processor
                .general_purpose_registers
                .get_mut(field.read(word) as usize)
            {
                Some(register) => *register = value,
                None if thirty_one == RegisterThirtyOne::StackPointer => {
                    processor.stack_pointer = value;
                }
                None => {}
            }
        }
        let readable_and_executable = PagePermission::Mapped {
            data_access: DataAccess::ReadOnly,
            instruction_fetch: InstructionFetch::Allowed,
        };
        // The word goes last, so that consecutive states with the same data pages share every change before it.
        let mut memory_mutations = vec![MemoryDelta::PermissionAssignment {
            addresses: CODE_ADDRESS..CODE_ADDRESS + PAGE_BYTES,
            permission: readable_and_executable,
        }];
        let layout = LoadStoreInstruction::new(instruction)
            .and_then(|load_store| load_store.to_access_layout(word, &processor))
            .or_else(|| {
                to_scalable_access(instruction)
                    .and_then(|access| access.to_access_layout(word, &processor))
            })
            .or_else(|| {
                to_element_access(instruction)
                    .map(|access| access.to_access_layout(word, &processor))
            })
            .or_else(|| {
                let bytes = StructureInstruction::new(instruction)?.to_access_bytes(word)?;
                Some(AccessLayout { bytes, offset: 0 })
            })
            .or_else(|| Some(TagAccessInstruction::new(instruction)?.to_access_layout(word)))
            .or_else(|| {
                let bytes = (instruction == Instruction::Sys)
                    .then(|| to_system_instruction_bytes(SYSTEM_INSTRUCTION.read(word)))??;
                Some(AccessLayout { bytes, offset: 0 })
            });
        // SYS takes its address from Rt.
        let base_field = if instruction == Instruction::Sys {
            RT
        } else {
            RN
        };
        if let (Some(placement), Some(layout)) = (placement, layout) {
            let address = to_placed_address(instruction, placement, layout.bytes);
            let base = address.wrapping_sub(layout.offset).wrapping_add(tag);
            match to_element_access(instruction).filter(|access| access.has_vector_base()) {
                Some(access) => {
                    scalable_element_access::place_vector_base(access, word, base, &mut processor);
                }
                None => match processor
                    .general_purpose_registers
                    .get_mut(base_field.read(word) as usize)
                {
                    Some(register) => *register = base,
                    None => processor.stack_pointer = base,
                },
            }
            // A comparand that is also the base keeps the base's value, which places the access.
            if let Some((field, comparison, pair)) = comparand {
                let held = (0..layout.bytes).fold(0_u128, |value, offset| {
                    let byte = to_data_byte(address.wrapping_add(offset), fill);
                    value | (u128::from(byte) << (offset * u64::from(u8::BITS)))
                });
                let (registers, register_bytes) = if pair {
                    (2, layout.bytes / 2)
                } else {
                    (1, layout.bytes)
                };
                let register_bits = (register_bytes * u64::from(u8::BITS)) as u32;
                let above = u64::MAX.checked_shl(register_bits).unwrap_or(0);
                let highest_byte = 0xFF_u64 << (register_bits - u8::BITS);
                for index in 0..registers {
                    let part = (held >> (index * register_bits)) as u64 & !above;
                    let number = field.read(word) + index;
                    if number == RN.read(word) {
                        continue;
                    }
                    if let Some(register) =
                        processor.general_purpose_registers.get_mut(number as usize)
                    {
                        *register = match comparison {
                            Comparison::Equal => part,
                            Comparison::EqualInComparedBytes => part | above,
                            Comparison::DiffersInHighestByte if index + 1 == registers => {
                                part ^ highest_byte
                            }
                            Comparison::DiffersInHighestByte => part,
                            Comparison::Unequal => !part,
                        };
                    }
                }
            }
            let data: &'static [u8; DATA_BYTES] = match fill {
                MemoryFill::Pattern => &PATTERN_DATA,
                MemoryFill::Zeros => &ZERO_DATA,
                MemoryFill::Ones => &ONE_DATA,
            };
            memory_mutations.extend([
                MemoryDelta::ValueAssignment {
                    start: LOWER_DATA_ADDRESS,
                    bytes: Cow::Borrowed(data),
                },
                MemoryDelta::PermissionAssignment {
                    addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS,
                    permission: lower,
                },
                MemoryDelta::PermissionAssignment {
                    addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES,
                    permission: upper,
                },
            ]);
        }
        memory_mutations.push(MemoryDelta::ValueAssignment {
            start: processor.program_counter,
            bytes: Cow::Owned(word.to_le_bytes().to_vec()),
        });
        ExecutionState {
            processor,
            memory_mutations,
        }
    }
}

/// The instruction's word, with the fields its operands of those types encode; other operands leave it alone.
///
/// # Panics
///
/// When an operand's value is not one of its type.
pub fn encode_word(
    instruction: Instruction<Infallible>,
    operands: impl IntoIterator<Item = (OperandType, OperandValue)>,
) -> u32 {
    let should_be = instruction
        .to_record()
        .map_or(0, |record| record.should_be.value);
    let mut word = instruction.to_fixed_bits().value | should_be;
    // An Advanced SIMD operand whose bits depend on the rest of the word goes in once the rest is there.
    let mut deferred = Vec::new();
    for (operand_type, operand) in operands {
        match (operand_type, operand) {
            (
                OperandType::RegisterNumber(field)
                | OperandType::VectorRegisterNumber(field)
                | OperandType::PredicateRegisterNumber(field)
                | OperandType::ScalableVectorNumber { field, .. }
                | OperandType::PredicateNumber(field)
                | OperandType::VectorListNumber(field),
                OperandValue::RegisterNumber(number),
            ) => word |= field.to_bits(number),
            (OperandType::Pattern(field), OperandValue::Bits(bits)) => {
                word |= field.to_bits(bits as u32)
            }
            (
                OperandType::UnsignedImmediate(field)
                | OperandType::SignedImmediate(field)
                | OperandType::ShiftAmount { field, .. }
                | OperandType::BitmaskImmediate(field)
                | OperandType::SystemRegister(field),
                OperandValue::Bits(bits),
            ) => word |= field.to_bits(bits as u32),
            (OperandType::Word(_), OperandValue::Bits(bits)) => word = bits as u32,
            (OperandType::AdvancedSimd(simd_operand), _) => deferred.push((simd_operand, operand)),
            (OperandType::Remaining(remaining), _) => {
                word |= sve_remaining::encode(remaining, operand)
            }
            (
                OperandType::ProgramCounterOffset(fields)
                | OperandType::SplitImmediate(fields)
                | OperandType::EveryValue(fields),
                OperandValue::Bits(bits),
            ) => {
                let mut remaining = bits;
                for field in fields.iter().rev() {
                    word |= field.to_bits(remaining as u32);
                    remaining >>= field.width;
                }
            }
            (
                OperandType::RegisterNumber(_)
                | OperandType::VectorRegisterNumber(_)
                | OperandType::PredicateRegisterNumber(_)
                | OperandType::Pattern(_)
                | OperandType::ScalableVectorNumber { .. }
                | OperandType::PredicateNumber(_)
                | OperandType::EveryValue(_)
                | OperandType::VectorListNumber(_)
                | OperandType::UnsignedImmediate(_)
                | OperandType::SignedImmediate(_)
                | OperandType::ShiftAmount { .. }
                | OperandType::BitmaskImmediate(_)
                | OperandType::ProgramCounterOffset(_)
                | OperandType::SplitImmediate(_)
                | OperandType::SystemRegister(_)
                | OperandType::Word(_),
                _,
            ) => reject_mismatch(operand_type, operand),
            _ => {}
        }
    }
    let rest = word;
    for (simd_operand, operand) in deferred {
        word |= advanced_simd::encode(simd_operand, operand, rest);
    }
    word
}

/// A word LLVM decodes as the instruction, or its fixed and should-be bits where the list has none.
fn to_example_word(instruction: Instruction<Infallible>) -> u32 {
    let record = instruction.to_record();
    record.and_then(|record| record.example).unwrap_or_else(|| {
        instruction.to_fixed_bits().value | record.map_or(0, |record| record.should_be.value)
    })
}

/// The byte a data page holds at the address.
const fn to_data_byte(address: u64, fill: MemoryFill) -> u8 {
    match fill {
        MemoryFill::Pattern => !(address as u8),
        MemoryFill::Zeros => u8::MIN,
        MemoryFill::Ones => u8::MAX,
    }
}

const DATA_BYTES: usize = (UPPER_DATA_ADDRESS + PAGE_BYTES - LOWER_DATA_ADDRESS) as usize;

/// The bytes of both data pages filled so, which every state borrows instead of holding its own copy.
const fn to_data(fill: MemoryFill) -> [u8; DATA_BYTES] {
    let mut data = [0; DATA_BYTES];
    let mut index = 0;
    while index < DATA_BYTES {
        data[index] = to_data_byte(LOWER_DATA_ADDRESS + index as u64, fill);
        index += 1;
    }
    data
}

static PATTERN_DATA: [u8; DATA_BYTES] = to_data(MemoryFill::Pattern);
static ZERO_DATA: [u8; DATA_BYTES] = to_data(MemoryFill::Zeros);
static ONE_DATA: [u8; DATA_BYTES] = to_data(MemoryFill::Ones);

/// Of the low `bytes` bytes of a register: zero, all ones, each byte its own place counted from one with its top bit
/// clear and set, so that a store that moves bytes out of order or from the wrong part of the register shows it; and
/// every byte 0x7F or 0x80, the largest positive and the most negative element of every size.
fn to_vector_values(bytes: u32) -> Vec<OperandValue> {
    let mask = u128::MAX >> (u128::BITS - bytes * u8::BITS);
    let counted = (0..bytes).fold(0_u128, |value, place| {
        value | (u128::from(place + 1) << (place * u8::BITS))
    });
    let top_bits = u128::from_le_bytes([1 << (u8::BITS - 1); size_of::<u128>()]);
    let largest_positive = !top_bits;
    let mut values = [
        0,
        mask,
        counted,
        counted | (top_bits & mask),
        largest_positive & mask,
        top_bits & mask,
    ]
    .to_vec();
    // A whole register's lanes of each element size: each the most negative, each the largest positive, and the two
    // alternating, so that every lane holds each end of the signed range beside one of the other sign.
    if bytes == size_of::<u128>() as u32 {
        for element_bits in ELEMENT_TOP_BITS.map(|top| top + 1) {
            let lanes = u128::BITS / element_bits;
            let fill = |lane: &dyn Fn(u32) -> u128| {
                (0..lanes).fold(0, |value, place| {
                    value | (lane(place) << (place * element_bits))
                })
            };
            let most_negative = 1_u128 << (element_bits - 1);
            let largest = most_negative - 1;
            for pattern in [
                fill(&|_| most_negative),
                fill(&|_| largest),
                fill(&|place| {
                    if place % 2 == 0 {
                        largest
                    } else {
                        most_negative
                    }
                }),
                fill(&|place| {
                    if place % 2 == 0 {
                        most_negative
                    } else {
                        largest
                    }
                }),
            ] {
                if !values.contains(&pattern) {
                    values.push(pattern);
                }
            }
        }
    }
    values.into_iter().map(OperandValue::Vector).collect()
}

/// Zero, one and the highest bit of each element size, for a field this wide that can name a bit; in a field of six
/// bits each also with the top bit set, the same bits of a doubleword's upper word, which a field whose top bit an
/// encoding requires (a 32-bit fixed-point scale) or that names a bit of either word (TBZ's b5:b40) needs.
fn to_bit_numbers(width: u32) -> Vec<u64> {
    let low = [0, 1].into_iter().chain(ELEMENT_TOP_BITS.map(u64::from));
    let upper_word = (width == BIT_NUMBER_FIELD_BITS).then_some(1 << (width - 1));
    low.clone()
        .chain(
            upper_word
                .into_iter()
                .flat_map(move |top| low.clone().map(move |bit| bit | top)),
        )
        .collect()
}

/// Zero, one, all ones, the largest and the most negative value of a signed field this wide, and each bit alone.
fn to_signed_critical_values(width: u32) -> Vec<OperandValue> {
    let all_ones = low_ones(width);
    let sign_bit = 1 << (width - 1);
    with_each_bit_alone(width, [0, 1, all_ones, sign_bit - 1, sign_bit])
}

/// The values that fit a field this wide, with each of its bits alone, in increasing order.
fn with_each_bit_alone(width: u32, values: impl IntoIterator<Item = u64>) -> Vec<OperandValue> {
    let largest = low_ones(width);
    values
        .into_iter()
        .chain((0..width).map(|bit| 1 << bit))
        .filter(|value| *value <= largest)
        .sorted_unstable()
        .dedup()
        .map(OperandValue::Bits)
        .collect()
}

/// The values in their order, each once.
fn to_distinct_bits(values: Vec<u64>) -> Vec<OperandValue> {
    let mut distinct: Vec<u64> = Vec::new();
    for value in values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    distinct.into_iter().map(OperandValue::Bits).collect()
}

/// Ones in the lowest `bits` bits.
const fn low_ones(bits: u32) -> u64 {
    u64::MAX >> (u64::BITS - bits)
}

fn reject_mismatch(operand_type: OperandType, operand: OperandValue) -> ! {
    panic!("{operand:?} is no value of {operand_type:?}")
}

/// The operands the list gives the instruction in the order of its syntax, each register followed by its value
/// where the instruction reads it, then what it reads besides them.
fn to_listed_operand_types(instruction: Instruction<Infallible>) -> Vec<OperandType> {
    let Some(record) = instruction.to_record() else {
        panic!("{instruction:?} is listed")
    };
    let mut operand_types = Vec::new();
    let mut destination = None;
    let mut addresses_data = false;
    let mut reads_own_address = false;
    let mut precisions = Vec::new();
    // A compare and swap compares its first register, or first pair of registers, with the bytes it reads.
    let load_store = LoadStoreInstruction::new(instruction);
    let mut compares = load_store.is_some_and(|load_store| {
        matches!(
            load_store.to_operation().transfer,
            Transfer::Atomic(AtomicOperation::CompareAndSwap) | Transfer::CompareAndSwapPair
        )
    });
    let writes_memory = load_store.is_some_and(|load_store| {
        matches!(
            load_store.to_operation().transfer,
            Transfer::Store
                | Transfer::StoreExclusive
                | Transfer::Atomic(_)
                | Transfer::CompareAndSwapPair
        )
    });
    // A conversion between an integer and floating point reads the one kind of register and writes the other.
    let converted_precision = record.operands.iter().find_map(|operand| match *operand {
        Operand::FloatingPointRegister { precision, .. } => Some(precision),
        _ => None,
    });
    let converted_width = record.operands.iter().find_map(|operand| match *operand {
        Operand::GeneralPurposeRegister { width, .. } => Some(width),
        _ => None,
    });
    // A branch to a register's value, and pointer authentication, take a register as an address.
    let takes_address = matches!(
        record.class,
        InstructionClass::PointerAuthentication | InstructionClass::MemoryTagging
    ) || matches!(
        to_branch_operation(instruction),
        Some(
            BranchOperation::BranchToRegister
                | BranchOperation::BranchWithLinkToRegister
                | BranchOperation::Return
        )
    );
    let (from_integer, to_integer) = match to_floating_point_operation(instruction) {
        Some(FloatingPointOperation::ConvertFromInteger { .. }) => (converted_precision, None),
        Some(
            FloatingPointOperation::ConvertToInteger { .. }
            | FloatingPointOperation::ConvertToJavaScriptInteger,
        ) => (None, converted_width),
        _ => (None, None),
    };
    for operand in record.operands {
        match *operand {
            Operand::GeneralPurposeRegister {
                number,
                width,
                thirty_one,
                access,
            } => {
                operand_types.push(OperandType::RegisterNumber(number));
                let value = if let (true, Some(load_store)) = (compares, load_store) {
                    compares = false;
                    OperandType::Comparand {
                        field: number,
                        bytes: u64::from(load_store.to_operation().bytes),
                        pair: false,
                    }
                } else if let Some(precision) = from_integer {
                    OperandType::ConvertedInteger {
                        field: number,
                        width,
                        precision,
                    }
                } else if takes_address && width == RegisterWidth::Bits64 {
                    OperandType::Address {
                        field: number,
                        thirty_one,
                    }
                } else {
                    OperandType::RegisterValue {
                        field: number,
                        width,
                        thirty_one,
                    }
                };
                match access {
                    OperandAccess::Read => operand_types.push(value),
                    OperandAccess::Written => destination = Some(value),
                }
            }
            Operand::GeneralPurposeRegisterPair { number, width } => {
                operand_types.push(OperandType::RegisterNumber(number));
                operand_types.push(match (compares, load_store) {
                    (true, Some(load_store)) => {
                        compares = false;
                        OperandType::Comparand {
                            field: number,
                            bytes: 2 * u64::from(load_store.to_operation().bytes),
                            pair: true,
                        }
                    }
                    _ => OperandType::RegisterValue {
                        field: number,
                        width,
                        thirty_one: RegisterThirtyOne::ZeroRegister,
                    },
                });
            }
            Operand::SplitImmediate(fields) => {
                operand_types.push(OperandType::SplitImmediate(fields));
            }
            Operand::ShiftedRegister {
                number,
                shift_type,
                amount,
                width,
                ..
            } => operand_types.extend([
                OperandType::RegisterNumber(number),
                OperandType::UnsignedImmediate(shift_type),
                OperandType::ShiftAmount {
                    field: amount,
                    width,
                },
                OperandType::RegisterValue {
                    field: number,
                    width,
                    thirty_one: RegisterThirtyOne::ZeroRegister,
                },
            ]),
            Operand::ExtendedRegister {
                number,
                extend_type,
                amount,
                ..
            } => operand_types.extend([
                OperandType::RegisterNumber(number),
                OperandType::UnsignedImmediate(extend_type),
                OperandType::UnsignedImmediate(amount),
                OperandType::RegisterValue {
                    field: number,
                    width: RegisterWidth::Bits64,
                    thirty_one: RegisterThirtyOne::ZeroRegister,
                },
            ]),
            Operand::ArithmeticImmediate { value, shift } => operand_types.extend([
                OperandType::UnsignedImmediate(shift),
                OperandType::UnsignedImmediate(value),
            ]),
            Operand::BitmaskImmediate { field, .. } => {
                operand_types.push(OperandType::BitmaskImmediate(field));
            }
            Operand::WideImmediateShift { field, .. }
            | Operand::UnsignedImmediate(field)
            | Operand::Condition(field) => {
                operand_types.push(OperandType::UnsignedImmediate(field))
            }
            Operand::SignedImmediate(field) => {
                operand_types.push(OperandType::SignedImmediate(field))
            }
            Operand::ProgramCounterOffset { fields, .. } => {
                operand_types.push(OperandType::ProgramCounterOffset(fields));
                reads_own_address = true;
            }
            Operand::AddressBase { number, .. } => {
                operand_types.push(OperandType::RegisterNumber(number));
                // Only a modeled load or store places its access; a hint that names a base reads nothing there.
                addresses_data = load_store.is_some();
            }
            Operand::SplitAddressOffset { fields, .. } => {
                operand_types.push(OperandType::ProgramCounterOffset(fields));
            }
            Operand::AddressOffset {
                field, signedness, ..
            } => operand_types.push(match signedness {
                Signedness::Unsigned => OperandType::UnsignedImmediate(field),
                Signedness::Signed => OperandType::SignedImmediate(field),
            }),
            Operand::RegisterOffset {
                number,
                signed,
                doubleword,
                shift,
            } => {
                operand_types.push(OperandType::RegisterNumber(number));
                operand_types.extend(
                    [signed, doubleword, Some(shift)]
                        .into_iter()
                        .flatten()
                        .map(OperandType::UnsignedImmediate),
                );
                operand_types.push(OperandType::RegisterValue {
                    field: number,
                    width: RegisterWidth::Bits64,
                    thirty_one: RegisterThirtyOne::ZeroRegister,
                });
            }
            Operand::FloatingPointRegister {
                number,
                precision,
                access,
            } => {
                operand_types.push(OperandType::VectorRegisterNumber(number));
                precisions.push(precision);
                if access == OperandAccess::Read {
                    operand_types.push(match to_integer {
                        Some(width) => OperandType::ConvertedFloatingPoint {
                            field: number,
                            precision,
                            width,
                        },
                        // An Advanced SIMD scalar takes the register's bits as an integer.
                        None if to_vector_operation(instruction).is_some()
                            || to_cryptography_operation(instruction).is_some() =>
                        {
                            OperandType::VectorValue {
                                field: number,
                                bytes: precision.to_bits() / u8::BITS,
                            }
                        }
                        None => OperandType::FloatingPointValue {
                            field: number,
                            precision,
                        },
                    });
                }
            }
            Operand::VectorRegister { number, access } => {
                operand_types.push(OperandType::VectorRegisterNumber(number));
                if access == OperandAccess::Read {
                    // A load or store moves its access's bytes; any other instruction may read the whole register.
                    let bytes = load_store.map_or(size_of::<u128>() as u32, |load_store| {
                        load_store.to_operation().bytes
                    });
                    operand_types.push(OperandType::VectorValue {
                        field: number,
                        bytes,
                    });
                }
            }
            Operand::Ignored(_) => {}
            Operand::Unmodeled => {
                panic!("{instruction:?} has an operand without critical values: {operand:?}")
            }
        }
    }
    let implicit = to_implicit_inputs(instruction);
    if implicit.floating_point_control {
        let half = precisions.contains(&Precision::Half);
        let other = precisions
            .iter()
            .any(|precision| *precision != Precision::Half);
        operand_types.push(OperandType::FloatingPointControl(match (half, other) {
            (true, true) => ControlReads::Conversion,
            (true, false) => ControlReads::Half,
            (false, _) => ControlReads::SingleOrDouble,
        }));
    }
    if let (true, Some(destination)) = (implicit.destination, destination) {
        operand_types.push(destination);
    }
    if implicit.condition_flags {
        operand_types.push(OperandType::ConditionFlags);
    } else if writes_condition_flags(instruction) {
        operand_types.push(OperandType::WrittenConditionFlags);
    }
    if implicit.floating_point_control {
        operand_types.push(OperandType::StatusFlags);
    }
    if addresses_data {
        operand_types.extend([
            OperandType::AccessPlacement,
            OperandType::AddressTag,
            OperandType::DataPagePermission(DataPage::Lower),
            OperandType::DataPagePermission(DataPage::Upper),
        ]);
        if writes_memory {
            operand_types.push(OperandType::MemoryFill);
        }
    }
    if reads_own_address || implicit.program_counter {
        operand_types.push(OperandType::CodePlacement);
    }
    if reads_link_register(instruction) {
        operand_types.push(OperandType::ImplicitAddress(LINK_REGISTER));
    }
    operand_types
}

/// Of a precision: both zeros; the smallest and largest subnormal and the smallest normal of both signs, so that a
/// product of two of them is tiny with either sign; one, the next value up, and half of the gap between them, which
/// an addition rounds at; 2.5, -1.5 and -0.5, which round to an integral value at a tie, the last between -1 and 0;
/// two to the 31, past a 32-bit integer; the largest normal; both infinities; two quiet NaNs with different payloads
/// and signs, and a signaling one with a third; the first quiet one and the signaling one have payload bits at the
/// top of the fraction, which a conversion to a narrower precision keeps, and at its bottom, which it drops.
fn to_floating_point_values(precision: Precision) -> Vec<OperandValue> {
    to_floating_point_bits(precision)
        .into_iter()
        .map(OperandValue::Bits)
        .collect()
}

fn to_floating_point_bits(precision: Precision) -> Vec<u64> {
    let fraction_bits = precision.to_fraction_bits();
    let exponent_ones = (1_u64 << precision.to_exponent_bits()) - 1;
    let bias = exponent_ones >> 1;
    let sign = 1 << (precision.to_bits() - 1);
    let fraction_mask = (1 << fraction_bits) - 1;
    let with_exponent = |exponent: u64| exponent << fraction_bits;
    let infinity = with_exponent(exponent_ones);
    let quiet = 1 << (fraction_bits - 1);
    // Two to the 31, where the precision's exponents reach it.
    let two_to_31 = (bias + u64::from(u32::BITS - 1) < exponent_ones)
        .then(|| with_exponent(bias + u64::from(u32::BITS - 1)));
    [
        Some(0),
        Some(sign),
        Some(1),
        Some(fraction_mask),
        Some(with_exponent(1)),
        Some(with_exponent(bias)),
        Some(with_exponent(bias) | 1),
        Some(with_exponent(bias - u64::from(fraction_bits) - 1)),
        Some(with_exponent(bias + 1) | (1 << (fraction_bits - 2))),
        Some(sign | with_exponent(bias) | (1 << (fraction_bits - 1))),
        Some(sign | with_exponent(bias - 1)),
        Some(sign | with_exponent(1)),
        two_to_31,
        Some(with_exponent(exponent_ones - 1) | fraction_mask),
        Some(infinity),
        Some(sign | infinity),
        Some(infinity | quiet | (quiet >> 1) | 1),
        Some(sign | infinity | quiet | 2),
        Some(infinity | (quiet >> 2) | 3),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The values of a precision, and for a conversion to an integer of the width, both signs of two to the width less
/// one and of two to the width, each with the values next to it, and two to the width less one, less a half, where
/// the precision holds them: the bounds of the integer's range and the roundings at them.
fn to_converted_floating_point_values(
    precision: Precision,
    width: RegisterWidth,
) -> Vec<OperandValue> {
    let fraction_bits = precision.to_fraction_bits();
    let exponent_ones = (1_u64 << precision.to_exponent_bits()) - 1;
    let bias = exponent_ones >> 1;
    let sign = 1 << (precision.to_bits() - 1);
    let mut values = to_floating_point_bits(precision);
    for power in [width.to_bits() - 1, width.to_bits()] {
        let exponent = bias + u64::from(power);
        if exponent >= exponent_ones {
            continue;
        }
        let bound = exponent << fraction_bits;
        for magnitude in [bound - 1, bound, bound + 1] {
            values.extend([magnitude, sign | magnitude]);
        }
        // Two to the power less a half: one less than the bound in the last place of an exponent one lower.
        if power == width.to_bits() - 1 && fraction_bits >= power {
            let fraction = (1 << fraction_bits) - (1 << (fraction_bits - power));
            values.push(((exponent - 1) << fraction_bits) | fraction);
        }
    }
    to_distinct_bits(values)
}

/// The values of a register of the width, and for a conversion to the precision, both signs of the integers just past
/// its significand that round: two to the significand's bits plus one and plus three, each a tie between two values,
/// and the next power's plus one and plus three, below and above a tie.
fn to_converted_integer_values(width: RegisterWidth, precision: Precision) -> Vec<OperandValue> {
    let significand_bits = precision.to_fraction_bits() + 1;
    let mut values: Vec<u64> = to_register_values(width)
        .into_iter()
        .map(|value| match value {
            OperandValue::Bits(bits) => bits,
            _ => unreachable!("a register value is bits"),
        })
        .collect();
    for power in [significand_bits, significand_bits + 1] {
        if power + 1 >= width.to_bits() {
            continue;
        }
        for odd in [1, 3] {
            let integer = (1_u64 << power) + odd;
            values.extend([integer, integer.wrapping_neg() & width.to_mask()]);
        }
    }
    to_distinct_bits(values)
}

/// Every rounding mode, with and without default NaN, with each flush control the instruction reads set alone:
/// FZ16 and FZ for a half-precision instruction, and AHP alone and with FZ for a conversion.
fn to_control_values(reads: ControlReads) -> Vec<OperandValue> {
    let flushes: &[u64] = match reads {
        ControlReads::SingleOrDouble => &[0, FLUSH_TO_ZERO],
        ControlReads::Half => &[0, FLUSH_HALF_TO_ZERO, FLUSH_TO_ZERO],
        ControlReads::Conversion => &[
            0,
            FLUSH_TO_ZERO,
            FLUSH_HALF_TO_ZERO,
            ALTERNATIVE_HALF,
            ALTERNATIVE_HALF | FLUSH_TO_ZERO,
        ],
    };
    let mut values = Vec::new();
    for mode in 0..1 << ROUNDING_MODE_BITS {
        for default_nan in [0, DEFAULT_NAN] {
            for flush in flushes {
                values.push(OperandValue::Bits(
                    (mode << ROUNDING_MODE_SHIFT) | default_nan | flush,
                ));
            }
        }
    }
    values
}

/// Zero, one and all ones; the sign bits of a byte and of a halfword, each with the bit above it clear; the sign
/// bit of a word alone, which a sign-extended word never is; the top: a doubleword's sign bit alone, or for a word
/// read from its 64-bit register, only the upper half, which the word does not see; and the largest positive value
/// of the width, which overflows with one, as the top does with itself.
fn to_register_values(width: RegisterWidth) -> Vec<OperandValue> {
    let narrow_signs = (1 << (u8::BITS - 1)) | (1 << (u16::BITS - 1));
    let word_sign = 1 << (u32::BITS - 1);
    let top = match width {
        RegisterWidth::Bits32 => !RegisterWidth::Bits32.to_mask(),
        RegisterWidth::Bits64 => 1 << (u64::BITS - 1),
    };
    let largest_positive = width.to_mask() >> 1;
    [
        0,
        1,
        narrow_signs,
        word_sign,
        top,
        u64::MAX,
        largest_positive,
    ]
    .map(OperandValue::Bits)
    .to_vec()
}

/// The address of the access's first byte, for an access of elements across the boundary inside an element by the size
/// of its elements.
fn to_placed_address(
    instruction: Instruction<Infallible>,
    placement: AccessPlacement,
    access_bytes: u64,
) -> u64 {
    match to_element_access(instruction) {
        Some(access) if placement == AccessPlacement::StraddlesInsideElement => {
            UPPER_DATA_ADDRESS - access_bytes / 2 - u64::from(access.memory_bytes) / 2
        }
        _ => to_access_address(placement, access_bytes),
    }
}

fn to_access_address(placement: AccessPlacement, access_bytes: u64) -> u64 {
    match placement {
        AccessPlacement::LowerPageStart => LOWER_DATA_ADDRESS,
        AccessPlacement::ByteAfterLowerPageStart => LOWER_DATA_ADDRESS + 1,
        AccessPlacement::AcrossEightBytes => {
            LOWER_DATA_ADDRESS + DOUBLEWORD_BYTES - access_bytes / 2
        }
        AccessPlacement::AcrossSixteenBytes => {
            LOWER_DATA_ADDRESS + SINGLE_COPY_ATOMIC_BYTES - access_bytes / 2
        }
        AccessPlacement::EndsAtBoundary => UPPER_DATA_ADDRESS - access_bytes,
        AccessPlacement::StraddlesBoundary => UPPER_DATA_ADDRESS - access_bytes / 2,
        AccessPlacement::UpperPageStart => UPPER_DATA_ADDRESS,
        AccessPlacement::StraddlesInsideVector => {
            UPPER_DATA_ADDRESS - access_bytes / 2 - HALF_VECTOR_BYTES
        }
        // An access of elements places it by its element's size, `to_placed_address`.
        AccessPlacement::StraddlesInsideElement => UPPER_DATA_ADDRESS - access_bytes / 2,
    }
}

/// The operand types of an SVE instruction's operands, each register followed by its value where it is read.
fn to_scalable_operand_types(operands: &[ScalableOperand]) -> Vec<OperandType> {
    let mut operand_types = Vec::new();
    for operand in operands {
        match *operand {
            ScalableOperand::ElementSize(field)
            | ScalableOperand::Selector(field)
            | ScalableOperand::UnsignedImmediate(field) => {
                operand_types.push(OperandType::UnsignedImmediate(field));
            }
            ScalableOperand::SignedImmediate(field) => {
                operand_types.push(OperandType::SignedImmediate(field));
            }
            ScalableOperand::SplitImmediate(fields) => {
                operand_types.push(OperandType::SplitImmediate(fields));
            }
            ScalableOperand::Pattern(field) => operand_types.push(OperandType::Pattern(field)),
            ScalableOperand::Bitmask(field) => {
                operand_types.push(OperandType::BitmaskImmediate(field));
            }
            ScalableOperand::Vector {
                field,
                values,
                pair,
            } => {
                operand_types.push(OperandType::VectorRegisterNumber(field));
                match values {
                    VectorValues::Written => {}
                    VectorValues::Data => operand_types.push(OperandType::VectorValue {
                        field,
                        bytes: size_of::<u128>() as u32,
                    }),
                    VectorValues::ShiftAmounts => {
                        operand_types.push(OperandType::ShiftAmountVector(field));
                    }
                }
                if pair {
                    operand_types.push(OperandType::NextVectorValue(field));
                }
            }
            ScalableOperand::Predicate { field, values } => {
                operand_types.push(OperandType::PredicateRegisterNumber(field));
                match values {
                    PredicateValues::Written => {}
                    PredicateValues::Governing => {
                        operand_types.push(OperandType::GoverningPredicate(field));
                    }
                    PredicateValues::Operand => {
                        operand_types.push(OperandType::PredicateValue(field));
                    }
                }
            }
            ScalableOperand::General {
                field,
                thirty_one,
                values,
            } => {
                operand_types.push(OperandType::RegisterNumber(field));
                match values {
                    None => {}
                    Some(GeneralValues::Plain) => operand_types.push(OperandType::RegisterValue {
                        field,
                        width: RegisterWidth::Bits64,
                        thirty_one,
                    }),
                    Some(GeneralValues::Bound) => {
                        operand_types.push(OperandType::RegisterBound(field));
                    }
                    Some(GeneralValues::Saturating) => {
                        operand_types.push(OperandType::RegisterNearSaturation(field));
                    }
                }
            }
            ScalableOperand::ConditionFlags => operand_types.push(OperandType::ConditionFlags),
            ScalableOperand::WrittenConditionFlags => {
                operand_types.push(OperandType::WrittenConditionFlags);
            }
            ScalableOperand::FirstFault => operand_types.push(OperandType::FirstFaultValue),
            ScalableOperand::WrittenFirstFault => {
                operand_types.push(OperandType::WrittenFirstFault);
            }
        }
    }
    operand_types
}

/// Of each element size, lanes of shift amounts and indices: zero, one, each side of the size, twice it, the same
/// negative, and the ends of a signed byte; as many lanes to a register as the size has, the amounts taking turns.
fn to_shift_amount_vectors() -> Vec<u128> {
    let mut vectors = Vec::new();
    for top in ELEMENT_TOP_BITS {
        let bits = top + 1;
        let size = i128::from(bits);
        let amounts = [
            0,
            1,
            size - 1,
            size,
            size + 1,
            2 * size,
            -1,
            -(size - 1),
            -size,
            -(size + 1),
            i128::from(i8::MAX),
            i128::from(i8::MIN),
        ];
        let lanes = (u128::BITS / bits) as usize;
        let mask = u128::MAX >> (u128::BITS - bits);
        for chunk in 0..amounts.len().div_ceil(lanes) {
            let vector = (0..lanes).fold(0_u128, |vector, lane| {
                let amount = amounts[(chunk * lanes + lane) % amounts.len()];
                vector | ((amount as u128 & mask) << (lane as u32 * bits))
            });
            vectors.push(vector);
        }
    }
    vectors
}

/// The values a WHILE compares: small ones, each side of the ends of the signed and unsigned 64-bit ranges, and the
/// same of 32 bits with ones above the word.
fn to_bound_values() -> Vec<OperandValue> {
    let near_ends = |signed_max: u64, signed_min: u64, unsigned_max: u64| {
        [
            signed_max - 16,
            signed_max - 1,
            signed_max,
            signed_min,
            signed_min + 1,
            unsigned_max - 16,
            unsigned_max - 1,
            unsigned_max,
        ]
    };
    let doubleword =
        SMALL_BOUNDS
            .into_iter()
            .chain(near_ends(i64::MAX as u64, i64::MIN as u64, u64::MAX));
    let word = SMALL_BOUNDS
        .into_iter()
        .chain(near_ends(
            u64::from(i32::MAX as u32),
            u64::from(i32::MIN as u32),
            u64::from(u32::MAX),
        ))
        .map(|value| value | UPPER_WORD_ONES);
    to_distinct_bits(doubleword.chain(word).collect())
}

/// The values a saturating count steps: at and a little inside each end of the signed and unsigned 64-bit ranges, and
/// the same of 32 bits with ones above the word.
fn to_saturation_values() -> Vec<OperandValue> {
    let inside = |high: u64, low: u64| {
        SATURATION_DISTANCES
            .into_iter()
            .flat_map(move |distance| [high - distance, low + distance])
    };
    let doubleword = inside(i64::MAX as u64, i64::MIN as u64).chain(inside(u64::MAX, 0));
    let word = inside(u64::from(i32::MAX as u32), u64::from(i32::MIN as u32))
        .chain(inside(u64::from(u32::MAX), 0))
        .map(|value| value | UPPER_WORD_ONES);
    to_distinct_bits(doubleword.chain(word).collect())
}
