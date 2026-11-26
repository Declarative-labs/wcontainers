//! The inputs of the legacy integer forms, derived from each form's operand kinds as iced-x86 lists them: one
//! skeleton for every shape, so a new form of a modeled shape needs no line here.
//!
//! A form's operands are its operand size, the registers it names and how its bytes lie (or, with a memory operand,
//! the memory operand), the value of each register or immediate it reads, and the flags its family reads.
//!
//! A memory operand is one operand kind, whose values are derived here once for every form that has one: each
//! addressing form the encoding allows, each segment override, each place the access can lie and each code layout,
//! with the critical values of each part.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Encoder, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use itertools::Itertools;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::{IntegerForm, OperandPlace, Operation};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::{
    AccessPlacement, BITNESS, CODE_ADDRESS, Comparison, DATA_PAGES, FlagSet, GENERAL_16, GENERAL_32, GENERAL_64,
    NON_CANONICAL_ADDRESS, OperandSize, OperandSizes, OperandType, OperandValue, Role, Splits,
    ValueSet, to_access_address, to_accumulator, to_data_pages,
};

/// A page whose next page is unmapped, for code that runs over its end.
const FAR_CODE_ADDRESS: u64 = 0x5000;
/// The last page of the lower canonical half.
pub const LAST_CANONICAL_PAGE: u64 = (1 << 47) - PAGE_BYTES;
/// A canonical FS base far from the data, which a base register that wraps around makes up for.
const FS_BASE: u64 = 0x0000_7000_0000_0000;
/// A canonical GS base in the upper half, so that the address wraps past zero.
const GS_BASE: u64 = 0xFFFF_FFFF_FFFF_0000;
/// An address of the source's own kernel pages, and one elsewhere in the upper half.
const KERNEL_ADDRESS: u64 = 0xFFFF_FFFF_FFE0_5000;
const UPPER_HALF_ADDRESS: u64 = 0xFFFF_8000_0000_0000;
/// The last page below 4 GiB.
const BELOW_FOUR_GIB_PAGE: u64 = (1 << u32::BITS) - PAGE_BYTES;
/// An index register's value: at a scale of 2, 4 or 8 it moves the access by a multiple of 0x18, never of 0x100, so
/// a wrong scale reads other bytes.
const INDEX: u64 = 0x18;
/// What a register that 32-bit addressing reads holds above its low half, which that addressing ignores.
const ABOVE_ADDRESS_HALF: u64 = 0xA5A5_A5A5_0000_0000;
/// The prefixes that change neither an instruction's length nor, without FS or GS, its meaning in 64-bit mode.
const NEUTRAL_PREFIXES: [u8; 3] = [0x26, 0x2E, 0x36];
const LEGACY_PREFIXES: [u8; 11] = [0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3];
/// REX.W, which would widen the operand were it the REX that counts.
const PADDING_REX: u8 = 0x48;
/// The REX prefixes, 0x40 to 0x4F.
const REX_NIBBLE: u8 = 0x40;

const fn is_rex(byte: u8) -> bool {
    byte & 0xF0 == REX_NIBBLE
}

const GENERAL_8: [Register; 16] = [
    Register::AL, Register::CL, Register::DL, Register::BL, Register::SPL, Register::BPL,
    Register::SIL, Register::DIL, Register::R8L, Register::R9L, Register::R10L, Register::R11L,
    Register::R12L, Register::R13L, Register::R14L, Register::R15L,
];
/// The 8-bit registers 4 to 7 name without a REX prefix.
const HIGH_BYTES: [Register; 4] = [Register::AH, Register::CH, Register::DH, Register::BH];

/// How the instruction's bytes lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodeLayout {
    PageStart,
    /// Legal prefixes before it, to 15 bytes: as many different ones as there are before any repeats.
    Padded15,
    /// Legal prefixes before it, to 16 bytes: too long.
    Padded16,
    /// REX prefixes before it, to 15 bytes, each ignored, as a REX is unless it comes right before the opcode.
    Padded15Rex,
    /// REX prefixes before it, to 16 bytes: too long.
    Padded16Rex,
    /// 16 bytes, the 16th on an unmapped page.
    Padded16AcrossPage,
    /// Its last byte at the last canonical address.
    EndsAtCanonicalEdge,
    /// Its last byte one past the last canonical address; an instruction of one byte ends at that address.
    AcrossCanonicalEdge,
    /// Its last byte on an unmapped page.
    AcrossPageIntoUnmapped,
    /// Its prefixes and opcode up to the last canonical address, its ModRM byte or immediate past it.
    OpcodeAtCanonicalEdge,
    /// Its prefixes and opcode at the end of a page whose next page is unmapped.
    OpcodeAtPageEnd,
}

pub const LAYOUTS_BEYOND_START: [CodeLayout; 10] = [
    CodeLayout::Padded15,
    CodeLayout::Padded16,
    CodeLayout::Padded15Rex,
    CodeLayout::Padded16Rex,
    CodeLayout::Padded16AcrossPage,
    CodeLayout::EndsAtCanonicalEdge,
    CodeLayout::AcrossCanonicalEdge,
    CodeLayout::AcrossPageIntoUnmapped,
    CodeLayout::OpcodeAtCanonicalEdge,
    CodeLayout::OpcodeAtPageEnd,
];

impl CodeLayout {
    /// Whether the instruction faults while it is fetched or measured, before anything else of the state counts.
    pub const fn faults_before_execution(self) -> bool {
        !matches!(self, Self::PageStart | Self::Padded15 | Self::Padded15Rex | Self::EndsAtCanonicalEdge)
    }
}

/// The register numbers a form without a memory operand names, in operand order, and how its bytes lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Encoding {
    pub registers: [usize; 2],
    pub layout: CodeLayout,
}

/// How a memory operand's address is formed from the parts its encoding gives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Addressing {
    /// A base register and a displacement of the size the value needs: none for 0, but RBP and R13 force one, and
    /// RSP and R12 take a SIB byte.
    Base { base: usize, displacement: i32 },
    /// A base, an index register at a scale, and a displacement.
    BaseIndex {
        base: usize,
        index: usize,
        scale: u32,
        displacement: i32,
    },
    /// An index register at a scale and a 32-bit displacement, with no base.
    Index { index: usize, scale: u32 },
    /// A 32-bit displacement alone, through a SIB byte with neither base nor index.
    Displacement,
    /// The address of the next instruction and a 32-bit displacement.
    InstructionPointer,
    /// MOV's moffs: the address alone, as wide as the address size.
    Offset,
}

/// The width of the address arithmetic: 64 bits, or 32 under the 0x67 prefix, which wraps the sum at 4 GiB and
/// reads only the low halves of the registers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddressSize {
    Bits64,
    Bits32,
}

/// Which segment the access goes through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegmentChoice {
    Default,
    /// FS, with a far base the base register wraps back from.
    Fs,
    /// GS, with a base in the upper half the address wraps past zero from.
    Gs,
    /// An SS override.
    Stack,
    /// A DS override.
    Data,
    /// An ES override.
    Extra,
    /// A CS override.
    Code,
}

/// A memory operand: how its address is formed, the register the form names besides it, where the access lies and
/// what its pages allow, through which segment, with the code laid out how.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Access {
    pub addressing: Addressing,
    pub address_size: AddressSize,
    /// The register in ModRM.reg; never one the address reads.
    pub register: usize,
    pub data: DataPlacement,
    pub segment: SegmentChoice,
    pub layout: CodeLayout,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataPlacement {
    /// Around the boundary of the two data pages.
    Pages {
        placement: AccessPlacement,
        lower: PagePermission,
        upper: PagePermission,
    },
    /// On the source's own kernel pages.
    Kernel,
    /// Elsewhere in the upper half.
    UpperHalf,
    /// Its last byte at the last canonical address, on a writable page.
    EndsAtCanonicalEdge,
    /// Its last byte past the last canonical address.
    AcrossCanonicalEdge,
    /// One byte above the lowest address above the lower canonical half, so not aligned either.
    MisalignedNonCanonical,
    /// Below 4 GiB with bit 31 set, unmapped: where a 32-bit displacement with its sign bit set lies once it is
    /// zero-extended, as 32-bit addressing does.
    ZeroExtendedSignBit,
    /// At the start of the writable page just below 4 GiB, which a 32-bit address summed below zero reaches by
    /// wrapping, where the same sum in 64 bits lies in the kernel's half.
    BelowFourGiB,
}

/// The immediate kinds that share critical values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImmediateClass {
    /// One byte, sign-extended or not.
    Byte,
    /// 16 or 32 bits, as wide as the operand or sign-extended to 64.
    Full,
    /// As wide as the operand, 64 bits included (MOV's B8+r).
    Wide,
    /// A shift or rotate count.
    Count,
    /// A vector form's control: two-bit fields each holding every value, bits alternating, nibbles and halves.
    Selector,
    /// A jump's 8-bit displacement: none, one either way and each extreme.
    Relative8,
    /// A jump's 32-bit displacement.
    Relative32,
    /// CMPcc's predicate: each of the eight, and two with bits above them set, which legacy SSE ignores.
    Predicate,
    /// INT n's vector: 0, 1, those whose gates ring 3 may use (3 and 4), others up to 0xFF, but never Linux's system
    /// call 0x80, which the chip source refuses.
    Vector,
    /// VPTERNLOG's truth table: each operand alone (0xF0, 0xCC, 0xAA), the exclusive or and the majority of the
    /// three, none, all, and each single combination at the ends.
    TernaryTable,
    /// VCVTPS2PH's rounding: each mode, MXCSR's (bit 2), and bits above them set, which it ignores.
    Rounding,
}

/// What each operand of a form is, from its first code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    /// The register-or-memory operand.
    RegisterOrMemory,
    /// A register in ModRM.reg or the opcode.
    Register,
    /// AL, AX, EAX or RAX.
    Accumulator,
    /// CL, a shift or rotate count.
    Count,
    Immediate(ImmediateClass),
    /// The constant 1 of a shift by one.
    One,
    /// A memory operand whose address is the displacement.
    Absolute,
}

fn to_slot(kind: OpCodeOperandKind, form: &IntegerForm) -> Slot {
    use OpCodeOperandKind as Kind;
    match kind {
        // LEA's operand is memory alone, whose address it takes.
        Kind::r8_or_mem | Kind::r16_or_mem | Kind::r32_or_mem | Kind::r64_or_mem | Kind::mem => Slot::RegisterOrMemory,
        Kind::r8_reg | Kind::r16_reg | Kind::r32_reg | Kind::r64_reg => Slot::Register,
        Kind::r32_rm | Kind::r64_rm => Slot::Register,
        Kind::r8_opcode | Kind::r16_opcode | Kind::r32_opcode | Kind::r64_opcode => Slot::Register,
        Kind::al | Kind::ax | Kind::eax | Kind::rax => Slot::Accumulator,
        Kind::cl => Slot::Count,
        Kind::imm8_const_1 => Slot::One,
        Kind::imm8 | Kind::imm8sex16 | Kind::imm8sex32 | Kind::imm8sex64
            if form.operation.is_shift_or_rotate() || matches!(form.operation, Operation::BitTest(_)) =>
        {
            Slot::Immediate(ImmediateClass::Count)
        }
        Kind::imm8 | Kind::imm8sex16 | Kind::imm8sex32 | Kind::imm8sex64 => Slot::Immediate(ImmediateClass::Byte),
        Kind::imm16 | Kind::imm32 | Kind::imm32sex64 if form.codes.iter().any(|code| is_wide_move(*code)) => {
            Slot::Immediate(ImmediateClass::Wide)
        }
        Kind::imm16 | Kind::imm32 | Kind::imm32sex64 => Slot::Immediate(ImmediateClass::Full),
        Kind::imm64 => Slot::Immediate(ImmediateClass::Wide),
        Kind::mem_offs => Slot::Absolute,
        other => panic!("{:?} has an operand of a kind no skeleton covers: {other:?}", form.instruction),
    }
}

fn is_wide_move(code: Code) -> bool {
    code == Code::Mov_r64_imm64
}

fn list_slots(form: &IntegerForm) -> Vec<Slot> {
    form.codes[0].op_code().op_kinds().iter().map(|&kind| to_slot(kind, form)).collect()
}

/// The bytes of a register or register-or-memory operand of the kind; `None` for any other kind.
fn to_kind_width(kind: OpCodeOperandKind) -> Option<usize> {
    use OpCodeOperandKind as Kind;
    Some(match kind {
        Kind::r8_or_mem | Kind::r8_reg | Kind::r8_opcode | Kind::al | Kind::cl => 1,
        Kind::r16_or_mem | Kind::r16_reg | Kind::r16_opcode | Kind::ax => 2,
        Kind::r32_or_mem | Kind::r32_reg | Kind::r32_rm | Kind::r32_opcode | Kind::eax => 4,
        Kind::r64_or_mem | Kind::r64_reg | Kind::r64_rm | Kind::r64_opcode | Kind::rax => 8,
        _ => return None,
    })
}

/// The bytes of each operand of the code, the destination's for an operand of no such width (an address alone,
/// an immediate).
fn list_widths(code: Code) -> Vec<usize> {
    let kinds = code.op_code().op_kinds();
    let destination = kinds.iter().find_map(|&kind| to_kind_width(kind)).unwrap_or(1);
    kinds.iter().map(|&kind| to_kind_width(kind).unwrap_or(destination)).collect()
}

/// The code of the operand size, the form's only code when it has one.
fn to_code(form: &IntegerForm, size: Option<OperandSize>) -> Code {
    match size {
        None | Some(OperandSize::Bits16) => form.codes[0],
        Some(OperandSize::Bits32) => form.codes[1],
        Some(OperandSize::Bits64) => form.codes[2],
    }
}

/// The bytes the form's memory operand reaches at the operand size; 0 for a form without one.
pub fn to_access_bytes(form: &IntegerForm, size: Option<OperandSize>) -> u64 {
    let code = to_code(form, size);
    let slots = list_slots(form);
    let widths = list_widths(code);
    slots
        .iter()
        .zip(widths)
        .find(|(slot, _)| matches!(slot, Slot::RegisterOrMemory | Slot::Absolute) && form.place != OperandPlace::Register)
        .map_or(0, |(_, width)| width as u64)
}

/// Whether the operation moves data rather than computing with it, so that two values that differ in every byte
/// are its critical values.
fn moves_data(operation: Operation) -> bool {
    matches!(operation, Operation::Move | Operation::MoveIf(_) | Operation::Exchange)
}

/// The operand types of an integer form, in the order the build reads them.
pub fn list_operand_types(form: &IntegerForm) -> Vec<OperandType> {
    let slots = list_slots(form);
    let widths = list_widths(form.codes[0]);
    let in_memory = form.place != OperandPlace::Register;
    let mut types = Vec::new();
    match form.codes.len() {
        3 => types.push(OperandType::OperandSize(OperandSizes::All)),
        2 => types.push(OperandType::OperandSize(OperandSizes::WithoutSixtyFour)),
        _ => {}
    }
    let free_registers = slots
        .iter()
        .filter(|slot| match slot {
            Slot::RegisterOrMemory => !in_memory,
            Slot::Register => true,
            _ => false,
        })
        .count();
    types.push(if in_memory {
        // XCHG with memory is locked whether or not it says so.
        let locked = form.place == OperandPlace::LockedMemory || form.operation == Operation::Exchange;
        let splits = if locked { Splits::Excluded } else { Splits::Allowed };
        OperandType::Memory {
            splits,
            offset: slots.contains(&Slot::Absolute),
        }
    } else {
        OperandType::Encoding(free_registers)
    });
    // The operand in memory takes the values a register would; the registers beside it fewer.
    let full = |width: usize| match (moves_data(form.operation), width) {
        (true, _) => ValueSet::Distinct,
        (false, 1) => ValueSet::Byte,
        (false, _) => ValueSet::Arithmetic,
    };
    let values = |width: usize| match (moves_data(form.operation), in_memory) {
        (true, _) | (_, false) => full(width),
        (false, true) => ValueSet::Few,
    };
    for (index, (slot, &width)) in slots.iter().zip(&widths).enumerate() {
        let role = if index == 0 { Role::Destination } else { Role::Source };
        // IMUL with three operands does not read its destination; UD0 and UD1 read nothing.
        let read = form.operation.reads_operands()
            && (index > 0
                || (form.operation.reads_destination()
                    && !(form.operation == Operation::MultiplySigned && slots.len() == 3)));
        match slot {
            Slot::RegisterOrMemory | Slot::Absolute if in_memory && read && form.operation.accesses_memory() => {
                types.push(OperandType::MemoryValue(full(width)));
            }
            Slot::RegisterOrMemory if in_memory => {}
            // A bit test's register is an offset; SHLD and SHRD's source takes fewer values beside their counts.
            Slot::Register if matches!(form.operation, Operation::BitTest(_)) => {
                types.push(OperandType::RegisterValue(role, ValueSet::BitOffset));
            }
            Slot::Register
                if matches!(form.operation, Operation::ShiftLeftDouble | Operation::ShiftRightDouble) =>
            {
                types.push(OperandType::RegisterValue(role, ValueSet::Few));
            }
            Slot::RegisterOrMemory | Slot::Register | Slot::Accumulator if read => {
                types.push(OperandType::RegisterValue(role, values(width)));
            }
            Slot::Count => types.push(OperandType::ShiftCount),
            Slot::Immediate(class) => types.push(OperandType::Immediate(*class)),
            _ => {}
        }
    }
    // The data register is the high half of a dividend, for which fewer values do; CMPXCHG's accumulator is equal
    // to its destination or not; an 8-bit division's dividend is AX, whose quotient's edges are its own.
    let divides_bytes = matches!(form.operation, Operation::DivideWide { .. }) && widths.first() == Some(&1);
    for &number in form.operation.list_implicit_reads() {
        types.push(match (form.operation, number) {
            (Operation::CompareExchange, _) => OperandType::Comparison,
            (_, 0) if divides_bytes => OperandType::RegisterValue(Role::Implicit(number), ValueSet::ByteDividend),
            (_, 0) => OperandType::RegisterValue(Role::Implicit(number), ValueSet::Arithmetic),
            _ => OperandType::RegisterValue(Role::Implicit(number), ValueSet::Few),
        });
    }
    let read_flags = form.operation.reads_flags();
    types.push(OperandType::Flags(if read_flags == Flags::CARRY {
        FlagSet::Carry
    } else if read_flags.is_empty() {
        FlagSet::Unread
    } else {
        FlagSet::Condition(read_flags)
    }));
    types
}

/// Register number pairs: names that need REX, RSP, the high bytes AH and BH (4 and 7 without REX), and one
/// register named twice.
const REGISTER_PAIRS: [[usize; 2]; 4] = [[1, 13], [13, 4], [4, 7], [3, 3]];

pub fn generate_encodings(free_registers: usize) -> Vec<OperandValue> {
    let pairs: &[[usize; 2]] = match free_registers {
        0 => &REGISTER_PAIRS[..1],
        _ => &REGISTER_PAIRS,
    };
    let at_start = pairs.iter().map(|&registers| Encoding {
        registers,
        layout: CodeLayout::PageStart,
    });
    let laid_out = LAYOUTS_BEYOND_START.map(|layout| Encoding {
        registers: REGISTER_PAIRS[0],
        layout,
    });
    at_start.chain(laid_out).map(OperandValue::Encoding).collect()
}

pub fn generate_immediates(class: ImmediateClass) -> Vec<OperandValue> {
    let values: &[i64] = match class {
        ImmediateClass::Byte => &[0, 1, 8, 0x7F, -0x80, -1],
        ImmediateClass::Full => &[0, 1, 8, 0x7FFF, -0x8000, 0x7FFF_FFFF, i32::MIN as i64, -1],
        ImmediateClass::Wide => &[0, 1, 0x7FFF, -0x8000, 0x7FFF_FFFF, 0x8000_0000, i64::MAX, i64::MIN, -1],
        ImmediateClass::Count => &[0, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 63, 64, 0xFF],
        ImmediateClass::Selector => &[0x00, 0x55, 0xAA, 0xFF, 0x1B, 0xE4, 0x4E, 0xB1, 0x0F, 0xF0],
        ImmediateClass::Relative8 => &[0, 1, -1, 0x7F, -0x80],
        ImmediateClass::Relative32 => &[0, 1, -1, 0x7FFF_FFFF, i32::MIN as i64],
        ImmediateClass::Predicate => &[0, 1, 2, 3, 4, 5, 6, 7, 0x0D, 0xFF],
        ImmediateClass::Vector => &[0, 1, 2, 3, 4, 5, 8, 0x0E, 0x20, 0x7F, 0x81, 0xFF],
        ImmediateClass::TernaryTable => &[0xF0, 0xCC, 0xAA, 0x96, 0xE8, 0x00, 0xFF, 0x01, 0x80],
        ImmediateClass::Rounding => &[0, 1, 2, 3, 4, 0xFB, 0xFC],
    };
    values.iter().map(|&value| OperandValue::Immediate(value)).collect()
}

/// The four bases every placement and segment meets, each with the register named besides it: no displacement
/// (RBX), a SIB byte and the lowest 32-bit displacement (RSP, the stack segment), the highest 8-bit displacement
/// (RBP, the stack segment), REX.B and the highest 32-bit displacement (R13).
const BASES: [(Addressing, usize); 4] = [
    (Addressing::Base { base: 3, displacement: 0 }, 1),
    (Addressing::Base { base: 4, displacement: i32::MIN }, 13),
    (Addressing::Base { base: 5, displacement: 0x7F }, 4),
    (Addressing::Base { base: 13, displacement: i32::MAX }, 0),
];

/// Every other addressing form the ModRM and SIB bytes allow, in 64-bit and 32-bit addressing, each with the
/// critical values of its parts: each displacement size and sign, each scale, RSP and R12 as a base (their SIB
/// byte), RBP and R13 (their forced displacement), R12 as an index (index 100 with REX.X), no base, a displacement
/// alone, and the instruction pointer; in 32-bit addressing a sum that wraps at 4 GiB from registers whose upper
/// halves are set.
const ADDRESSING_FORMS: [(Addressing, AddressSize); 26] = {
    use AddressSize::{Bits32, Bits64};
    use Addressing::{Base, BaseIndex, Displacement, Index, InstructionPointer};
    const fn indexed(base: usize, index: usize, scale: u32, displacement: i32) -> Addressing {
        BaseIndex {
            base,
            index,
            scale,
            displacement,
        }
    }
    [
        (Base { base: 3, displacement: -0x80 }, Bits64),
        (Base { base: 3, displacement: 0x1000 }, Bits64),
        (Base { base: 4, displacement: 0 }, Bits64),
        (Base { base: 12, displacement: 0x10 }, Bits64),
        (Base { base: 5, displacement: 0 }, Bits64),
        (Base { base: 13, displacement: 0 }, Bits64),
        (indexed(3, 6, 1, 0), Bits64),
        (indexed(3, 6, 2, 0), Bits64),
        (indexed(3, 6, 4, -0x80), Bits64),
        (indexed(3, 6, 8, i32::MIN), Bits64),
        (indexed(7, 12, 4, 0x10), Bits64),
        (indexed(13, 9, 8, 0), Bits64),
        (indexed(5, 6, 2, 0), Bits64),
        (indexed(4, 9, 2, 0), Bits64),
        (Index { index: 9, scale: 8 }, Bits64),
        (Index { index: 6, scale: 1 }, Bits64),
        (Displacement, Bits64),
        (InstructionPointer, Bits64),
        (Base { base: 3, displacement: i32::MAX }, Bits32),
        (Base { base: 4, displacement: 0 }, Bits32),
        (Base { base: 5, displacement: 0 }, Bits32),
        (Base { base: 13, displacement: 0x10 }, Bits32),
        (indexed(3, 6, 4, 0), Bits32),
        (Index { index: 9, scale: 8 }, Bits32),
        (Displacement, Bits32),
        (InstructionPointer, Bits32),
    ]
};

/// The 32-bit addressing forms whose parts, summed in 64 bits, lie below zero where they reach the page below 4 GiB,
/// so that only a sum wrapped at 4 GiB reaches it: a base with the lowest displacement, a base and index with it, an
/// index with a negative displacement, and the instruction pointer; and a displacement alone, zero-extended there.
const WRAPPING_FORMS: [Addressing; 5] = [
    Addressing::Base { base: 3, displacement: i32::MIN },
    Addressing::BaseIndex {
        base: 3,
        index: 6,
        scale: 4,
        displacement: i32::MIN,
    },
    Addressing::Index { index: 9, scale: 8 },
    Addressing::InstructionPointer,
    Addressing::Displacement,
];

/// Every value of a memory operand: each base at every placement and permission the access can meet and through
/// every segment where the segment changes the outcome, each other addressing form at the start of a writable page,
/// a displacement alone in the kernel's half (sign-extended in 64-bit addressing, zero-extended in 32-bit), and each
/// code layout with a short and the longest encoding. A form whose address is its displacement (MOV's moffs) takes
/// the same with that address alone, 64 bits wide or 32.
pub fn generate_accesses(splits: Splits, offset: bool) -> Vec<OperandValue> {
    let permissions = || {
        let mapped = |data_access| PagePermission::Mapped {
            data_access,
            instruction_fetch: InstructionFetch::Forbidden,
        };
        [
            PagePermission::Unmapped,
            mapped(DataAccess::ReadOnly),
            mapped(DataAccess::ReadWrite),
        ]
    };
    let pages = |placement, lower, upper| DataPlacement::Pages {
        placement,
        lower,
        upper,
    };
    let writable = permissions()[2];
    let start = pages(AccessPlacement::LowerPageStart, writable, PagePermission::Unmapped);
    let non_canonical = pages(AccessPlacement::NonCanonical, PagePermission::Unmapped, PagePermission::Unmapped);
    let mut placements = Vec::new();
    for placement in [
        AccessPlacement::LowerPageStart,
        AccessPlacement::Misaligned,
        AccessPlacement::EndsAtBoundary,
    ] {
        for lower in permissions() {
            placements.push(pages(placement, lower, PagePermission::Unmapped));
        }
    }
    if splits == Splits::Allowed {
        for (lower, upper) in permissions().into_iter().cartesian_product(permissions()) {
            placements.push(pages(AccessPlacement::StraddlesBoundary, lower, upper));
        }
    }
    for upper in permissions() {
        placements.push(pages(AccessPlacement::UpperPageStart, PagePermission::Unmapped, upper));
    }
    placements.extend([
        non_canonical,
        DataPlacement::MisalignedNonCanonical,
        DataPlacement::Kernel,
        DataPlacement::UpperHalf,
        DataPlacement::EndsAtCanonicalEdge,
    ]);
    if splits == Splits::Allowed {
        placements.push(DataPlacement::AcrossCanonicalEdge);
    }
    let bases: Vec<(Addressing, AddressSize, usize)> = if offset {
        vec![(Addressing::Offset, AddressSize::Bits64, 0)]
    } else {
        BASES.iter().map(|&(addressing, register)| (addressing, AddressSize::Bits64, register)).collect()
    };
    let mut accesses: Vec<Access> = Vec::new();
    let mut push = |(addressing, address_size, register): (Addressing, AddressSize, usize), data, segment, layout| {
        let access = Access {
            addressing,
            address_size,
            register,
            data,
            segment,
            layout,
        };
        if !accesses.contains(&access) {
            accesses.push(access);
        }
    };
    let plain = bases[0];
    for &data in &placements {
        push(plain, data, SegmentChoice::Default, CodeLayout::PageStart);
    }
    for &base in &bases {
        push(base, start, SegmentChoice::Default, CodeLayout::PageStart);
        for segment in [
            SegmentChoice::Default,
            SegmentChoice::Stack,
            SegmentChoice::Data,
            SegmentChoice::Extra,
            SegmentChoice::Code,
            SegmentChoice::Fs,
        ] {
            push(base, non_canonical, segment, CodeLayout::PageStart);
        }
        push(base, DataPlacement::MisalignedNonCanonical, SegmentChoice::Default, CodeLayout::PageStart);
        for segment in [
            SegmentChoice::Fs,
            SegmentChoice::Gs,
            SegmentChoice::Stack,
            SegmentChoice::Extra,
            SegmentChoice::Code,
        ] {
            push(base, start, segment, CodeLayout::PageStart);
        }
    }
    let forms: Vec<(Addressing, AddressSize, usize)> = if offset {
        vec![(Addressing::Offset, AddressSize::Bits32, 0)]
    } else {
        ADDRESSING_FORMS.iter().map(|&(addressing, address_size)| (addressing, address_size, 0)).collect()
    };
    for &form in &forms {
        push(form, start, SegmentChoice::Default, CodeLayout::PageStart);
    }
    let wrapping: &[Addressing] = if offset { &[Addressing::Offset] } else { &WRAPPING_FORMS };
    for &addressing in wrapping {
        push((addressing, AddressSize::Bits32, 0), DataPlacement::BelowFourGiB, SegmentChoice::Default, CodeLayout::PageStart);
    }
    if !offset {
        for (address_size, data) in [
            (AddressSize::Bits64, DataPlacement::Kernel),
            (AddressSize::Bits32, DataPlacement::ZeroExtendedSignBit),
        ] {
            push((Addressing::Displacement, address_size, 0), data, SegmentChoice::Default, CodeLayout::PageStart);
        }
    }
    let longest = if offset { plain } else { (BASES[1].0, AddressSize::Bits64, BASES[1].1) };
    for layout in LAYOUTS_BEYOND_START {
        push(plain, start, SegmentChoice::Default, layout);
        push(longest, start, SegmentChoice::Default, layout);
    }
    if !offset {
        let longest_narrow = (Addressing::Base { base: 4, displacement: i32::MAX }, AddressSize::Bits32, 0);
        push(longest_narrow, start, SegmentChoice::Default, CodeLayout::Padded16);
    }
    accesses.into_iter().map(OperandValue::Access).collect()
}

/// The operand values of one integer state, by what they choose.
#[derive(Default)]
struct IntegerChoice {
    size: Option<OperandSize>,
    encoding: Option<Encoding>,
    values: Vec<(Role, u64)>,
    count: Option<u64>,
    immediate: Option<i64>,
    flags: Option<Flags>,
    access: Option<Access>,
    memory_value: Option<u64>,
    comparison: Option<Comparison>,
}

/// Builds the state of an integer form from one value of each of its operand types.
pub fn build_state(form: &IntegerForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut choice = IntegerChoice::default();
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(size)) => choice.size = Some(size),
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => choice.encoding = Some(encoding),
            (OperandType::RegisterValue(role, _), OperandValue::Bits(value)) => choice.values.push((*role, value)),
            (OperandType::ShiftCount, OperandValue::Bits(count)) => choice.count = Some(count),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => choice.immediate = Some(value),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => choice.flags = Some(flags),
            (OperandType::Memory { .. }, OperandValue::Access(access)) => choice.access = Some(access),
            (OperandType::MemoryValue(_), OperandValue::Bits(value)) => choice.memory_value = Some(value),
            (OperandType::Comparison, OperandValue::Comparison(comparison)) => choice.comparison = Some(comparison),
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = to_code(form, choice.size);
    let widths = list_widths(code);
    let slots = list_slots(form);
    let in_memory = form.place != OperandPlace::Register;
    // The register numbers the free slots take, in order.
    let mut numbers: Vec<usize> = match (choice.encoding, choice.access) {
        (Some(encoding), _) => encoding.registers.to_vec(),
        (None, Some(access)) => vec![access.register],
        (None, None) => Vec::new(),
    };
    numbers.reverse();
    let address_registers = choice.access.map(|access| list_address_registers(access.addressing)).unwrap_or_default();
    let named: Vec<usize> = slots
        .iter()
        .scan(numbers.clone(), |numbers, slot| {
            Some(match slot {
                Slot::RegisterOrMemory if !in_memory => numbers.pop(),
                Slot::Register => numbers.pop(),
                _ => None,
            })
        })
        .flatten()
        .chain(address_registers)
        .collect();
    // REX.W, a 64-bit operand, is a REX prefix too, and AH to BH cannot be named with one.
    let needs_rex = named.iter().any(|&number| number >= 8) || widths.contains(&8);
    let to_register = |number: usize, width: usize| match width {
        1 if !needs_rex && (4..8).contains(&number) => HIGH_BYTES[number - 4],
        1 => GENERAL_8[number],
        2 => GENERAL_16[number],
        4 => GENERAL_32[number],
        _ => GENERAL_64[number],
    };
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        flags: choice.flags.unwrap_or(Flags::empty()),
        ..ZERO_PROCESSOR
    };
    let layout = match (choice.access, choice.encoding) {
        (Some(access), _) => access.layout,
        (None, Some(encoding)) => encoding.layout,
        (None, None) => CodeLayout::PageStart,
    };
    // The operands in order: registers, immediates and the memory operand, and the register each value goes to.
    let mut free = numbers;
    let mut operands: Vec<Operand> = Vec::new();
    let mut targets: Vec<(Role, Register)> = Vec::new();
    for (index, (slot, &width)) in slots.iter().zip(&widths).enumerate() {
        let role = if index == 0 { Role::Destination } else { Role::Source };
        match slot {
            Slot::RegisterOrMemory if in_memory => operands.push(Operand::Memory),
            Slot::RegisterOrMemory | Slot::Register => {
                let register = to_register(free.pop().expect("a register for each free slot"), width);
                operands.push(Operand::Register(register));
                targets.push((role, register));
            }
            Slot::Accumulator => {
                let register = to_register(0, width);
                operands.push(Operand::Register(register));
                targets.push((role, register));
            }
            Slot::Count => operands.push(Operand::Register(Register::CL)),
            Slot::Immediate(_) => operands.push(Operand::Immediate(choice.immediate.expect("an immediate"))),
            Slot::One => operands.push(Operand::Immediate(1)),
            Slot::Absolute => operands.push(Operand::Memory),
        }
    }
    for (role, value) in &choice.values {
        if let Role::Implicit(number) = role {
            processor.general_purpose_registers[*number] = *value;
        } else if let Some((_, register)) = targets.iter().find(|(target, _)| target == role) {
            write_view(&mut processor, *register, *value);
        }
    }
    if let Some(count) = choice.count {
        let counter = &mut processor.general_purpose_registers[1];
        *counter = (*counter & !0xFF) | count;
    }
    let mut memory_mutations = Vec::new();
    let memory_operand = in_memory.then(|| {
        let access = choice.access.expect("a memory operand");
        let width = to_access_bytes(form, choice.size);
        let (operand, address) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
        if let Some(value) = choice.memory_value {
            // Only the bytes a state can hold: those in the program's addresses.
            let bytes: Vec<u8> = (0..width)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(value.to_le_bytes())
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment {
                    start: address,
                    bytes: Cow::Owned(bytes),
                });
            }
        }
        operand
    });
    // CMPXCHG's accumulator, from the destination's value once it is in place.
    if let Some(comparison) = choice.comparison {
        let width = widths[0] as u64;
        let destination = match (choice.memory_value, targets.iter().find(|(role, _)| *role == Role::Destination)) {
            (Some(value), _) if in_memory => value,
            (_, Some((_, register))) => read_view(&processor, *register),
            _ => panic!("{:?} compares a destination", form.instruction),
        };
        let accumulator = to_accumulator(comparison, destination, width);
        write_view(&mut processor, to_register(0, widths[0]).full_register(), accumulator);
    }
    let encoded = encode(code, &operands, memory_operand, form.place == OperandPlace::LockedMemory);
    let (code_page, start, bytes) = lay_out(layout, &encoded, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let mut all = vec![
        MemoryDelta::PermissionAssignment {
            addresses: code_page..code_page + PAGE_BYTES,
            permission: executable,
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

#[derive(Clone, Copy)]
enum Operand {
    Register(Register),
    Immediate(i64),
    Memory,
}

/// Writes a value as the register sees it: AH to BH take the value's low byte, the rest of it below and above.
fn write_view(processor: &mut Processor, register: Register, value: u64) {
    let full = &mut processor.general_purpose_registers[register.full_register().number()];
    *full = if HIGH_BYTES.contains(&register) { value.rotate_left(8) } else { value };
}

/// A register's value as the instruction reads it: AH to BH their byte, the others their low bytes.
fn read_view(processor: &Processor, register: Register) -> u64 {
    let full = processor.general_purpose_registers[register.full_register().number()];
    let value = if HIGH_BYTES.contains(&register) { full >> 8 } else { full };
    value & width_mask(register.size())
}

const fn width_mask(bytes: usize) -> u64 {
    if bytes >= size_of::<u64>() { u64::MAX } else { (1 << (bytes * 8)) - 1 }
}

/// The general registers the address reads.
pub fn list_address_registers(addressing: Addressing) -> Vec<usize> {
    match addressing {
        Addressing::Base { base, .. } => vec![base],
        Addressing::BaseIndex { base, index, .. } => vec![base, index],
        Addressing::Index { index, .. } => vec![index],
        Addressing::Displacement | Addressing::InstructionPointer | Addressing::Offset => Vec::new(),
    }
}

/// The address of the access's first byte, which the placement gives.
pub fn to_data_address(data: DataPlacement, access_bytes: u64) -> u64 {
    match data {
        DataPlacement::Pages { placement, .. } => match placement {
            AccessPlacement::NonCanonical => NON_CANONICAL_ADDRESS,
            placement => to_access_address(placement, access_bytes),
        },
        DataPlacement::Kernel => KERNEL_ADDRESS,
        DataPlacement::UpperHalf => UPPER_HALF_ADDRESS,
        DataPlacement::EndsAtCanonicalEdge => LAST_CANONICAL_PAGE + PAGE_BYTES - access_bytes,
        DataPlacement::AcrossCanonicalEdge => LAST_CANONICAL_PAGE + PAGE_BYTES - access_bytes + 1,
        DataPlacement::MisalignedNonCanonical => NON_CANONICAL_ADDRESS + 1,
        DataPlacement::ZeroExtendedSignBit => u64::from(KERNEL_ADDRESS as u32),
        DataPlacement::BelowFourGiB => BELOW_FOUR_GIB_PAGE,
    }
}

/// Maps the data the access reaches and gives the address of its first byte.
fn place_data(data: DataPlacement, access_bytes: u64, memory: &mut Vec<MemoryDelta<'static, X86_64>>) -> u64 {
    let writable_page = |page: u64| {
        [
            MemoryDelta::ValueAssignment {
                start: page,
                bytes: Cow::Borrowed(&DATA_PAGES[..PAGE_BYTES as usize]),
            },
            MemoryDelta::PermissionAssignment {
                addresses: page..page + PAGE_BYTES,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadWrite,
                    instruction_fetch: InstructionFetch::Forbidden,
                },
            },
        ]
    };
    match data {
        DataPlacement::Pages { lower, upper, .. } => memory.extend(to_data_pages(lower, upper)),
        DataPlacement::EndsAtCanonicalEdge | DataPlacement::AcrossCanonicalEdge => {
            memory.extend(writable_page(LAST_CANONICAL_PAGE));
        }
        DataPlacement::BelowFourGiB => memory.extend(writable_page(BELOW_FOUR_GIB_PAGE)),
        DataPlacement::Kernel
        | DataPlacement::UpperHalf
        | DataPlacement::MisalignedNonCanonical
        | DataPlacement::ZeroExtendedSignBit => {}
    }
    to_data_address(data, access_bytes)
}

/// Maps the data the access reaches, sets the segment base and the registers the address reads so that it reaches
/// it, and gives the memory operand to encode and the address of the access's first byte.
///
/// # Panics
///
/// When the addressing form cannot reach the placement: a 32-bit address beyond 4 GiB, a displacement alone or
/// from the instruction pointer beyond 32 bits.
pub fn place_memory_operand(
    access: Access,
    access_bytes: u64,
    processor: &mut Processor,
    mutations: &mut Vec<MemoryDelta<'static, X86_64>>,
) -> (MemoryOperand, u64) {
    let address = place_data(access.data, access_bytes, mutations);
    let segment_base = match access.segment {
        SegmentChoice::Fs => {
            processor.fs_base = FS_BASE;
            FS_BASE
        }
        SegmentChoice::Gs => {
            processor.gs_base = GS_BASE;
            GS_BASE
        }
        _ => 0,
    };
    let target = address.wrapping_sub(segment_base);
    let wide = access.address_size == AddressSize::Bits64;
    let unreachable = || panic!("{access:?} cannot reach {address:#x}");
    if !wide && target > u64::from(u32::MAX) {
        unreachable();
    }
    let registers = if wide { &GENERAL_64 } else { &GENERAL_32 };
    let mut set = |number: usize, value: u64| {
        processor.general_purpose_registers[number] =
            if wide { value } else { ABOVE_ADDRESS_HALF | (value & u64::from(u32::MAX)) };
    };
    let displacement_size = if wide { 8 } else { 4 };
    let mut operand = match access.addressing {
        Addressing::Base { base, displacement } => {
            set(base, target.wrapping_sub(displacement as i64 as u64));
            MemoryOperand::with_base_displ(registers[base], i64::from(displacement))
        }
        Addressing::BaseIndex {
            base,
            index,
            scale,
            displacement,
        } => {
            set(index, INDEX);
            let offset = (displacement as i64 as u64).wrapping_add(INDEX * u64::from(scale));
            set(base, target.wrapping_sub(offset));
            MemoryOperand::with_base_index_scale_displ_size(
                registers[base],
                registers[index],
                scale,
                i64::from(displacement),
                1,
            )
        }
        Addressing::Index { index, scale } => {
            set(index, INDEX);
            let displacement = target.wrapping_sub(INDEX * u64::from(scale));
            let displacement = to_displacement(displacement, wide).unwrap_or_else(unreachable);
            MemoryOperand::with_index_scale_displ_size(registers[index], scale, displacement, displacement_size)
        }
        Addressing::Displacement => {
            to_displacement(target, wide).unwrap_or_else(unreachable);
            MemoryOperand::with_displ(target, displacement_size)
        }
        Addressing::InstructionPointer => {
            // EIP-relative addresses wrap at 4 GiB, so every address below it is in reach.
            let base = if wide { Register::RIP } else { Register::EIP };
            if wide && target.abs_diff(CODE_ADDRESS) >= 1 << 31 {
                unreachable();
            }
            MemoryOperand::with_base_displ(base, target as i64)
        }
        Addressing::Offset => MemoryOperand::with_displ(target, displacement_size),
    };
    operand.segment_prefix = match access.segment {
        SegmentChoice::Default => Register::None,
        SegmentChoice::Fs => Register::FS,
        SegmentChoice::Gs => Register::GS,
        SegmentChoice::Stack => Register::SS,
        SegmentChoice::Data => Register::DS,
        SegmentChoice::Extra => Register::ES,
        SegmentChoice::Code => Register::CS,
    };
    (operand, address)
}

/// The 32-bit displacement that gives the address: sign-extended in 64-bit addressing, any 32 bits in 32-bit.
fn to_displacement(address: u64, wide: bool) -> Option<i64> {
    let low = address as u32;
    if wide {
        (low as i32 as i64 as u64 == address).then_some(low as i32 as i64)
    } else {
        (u64::from(low) == address).then_some(low as i32 as i64)
    }
}

/// The prefix that pads an instruction without changing it: its own segment override, or DS, which 64-bit mode
/// ignores.
pub fn to_padding(memory: Option<MemoryOperand>) -> u8 {
    match memory.map(|operand| operand.segment_prefix) {
        Some(Register::FS) => 0x64,
        Some(Register::GS) => 0x65,
        Some(Register::SS) => 0x36,
        Some(Register::ES) => 0x26,
        Some(Register::CS) => 0x2E,
        _ => 0x3E,
    }
}

fn encode(code: Code, operands: &[Operand], memory: Option<MemoryOperand>, locked: bool) -> Vec<u8> {
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let mut instruction = match operands {
        [] => Encoded::with(code),
        [first] => match *first {
            Operand::Register(register) => Encoded::with1(code, register),
            Operand::Memory => Encoded::with1(code, memory.expect("a memory operand")),
            Operand::Immediate(value) => Encoded::with1(code, value as i32),
        }
        .unwrap_or_else(to_error),
        [first, second] => match (*first, *second) {
            (Operand::Register(first), Operand::Register(second)) => Encoded::with2(code, first, second),
            (Operand::Register(first), Operand::Memory) => {
                Encoded::with2(code, first, memory.expect("a memory operand"))
            }
            (Operand::Memory, Operand::Register(second)) => {
                Encoded::with2(code, memory.expect("a memory operand"), second)
            }
            (Operand::Register(first), Operand::Immediate(value)) if code == Code::Mov_r64_imm64 => {
                Encoded::with2(code, first, value)
            }
            (Operand::Register(first), Operand::Immediate(value)) => Encoded::with2(code, first, to_immediate(code, value)),
            (Operand::Memory, Operand::Immediate(value)) => {
                Encoded::with2(code, memory.expect("a memory operand"), to_immediate(code, value))
            }
            _ => panic!("{code:?} has no such operands"),
        }
        .unwrap_or_else(to_error),
        [first, second, third] => {
            let memory = || memory.expect("a memory operand");
            match (*first, *second, *third) {
                (Operand::Register(first), Operand::Register(second), Operand::Immediate(value)) => {
                    Encoded::with3(code, first, second, to_immediate(code, value))
                }
                (Operand::Register(first), Operand::Memory, Operand::Immediate(value)) => {
                    Encoded::with3(code, first, memory(), to_immediate(code, value))
                }
                (Operand::Memory, Operand::Register(second), Operand::Immediate(value)) => {
                    Encoded::with3(code, memory(), second, to_immediate(code, value))
                }
                (Operand::Register(first), Operand::Register(second), Operand::Register(third)) => {
                    Encoded::with3(code, first, second, third)
                }
                (Operand::Memory, Operand::Register(second), Operand::Register(third)) => {
                    Encoded::with3(code, memory(), second, third)
                }
                _ => panic!("{code:?} has no such operands"),
            }
            .unwrap_or_else(to_error)
        }
        _ => panic!("{code:?} has more operands than an integer skeleton"),
    };
    instruction.set_has_lock_prefix(locked);
    encode_at_code_address(&instruction)
}

/// The bytes of the instruction at the code address, where a state's code starts unless its layout moves it.
pub fn encode_at_code_address(instruction: &Encoded) -> Vec<u8> {
    let mut encoder = Encoder::new(BITNESS);
    encoder
        .encode(instruction, CODE_ADDRESS)
        .unwrap_or_else(|error| panic!("{:?} encodes: {error}", instruction.code()));
    encoder.take_buffer()
}

/// The immediate as the code's operand kind holds it: a byte, or as many bits as the kind has, sign-extended.
fn to_immediate(code: Code, value: i64) -> i32 {
    let kinds = code.op_code().op_kinds();
    let kind = kinds[kinds.len() - 1];
    match kind {
        OpCodeOperandKind::imm16 => value as i16 as i32,
        OpCodeOperandKind::imm8 | OpCodeOperandKind::imm8sex16 | OpCodeOperandKind::imm8sex32
        | OpCodeOperandKind::imm8sex64 => value as i8 as i32,
        _ => value as i32,
    }
}

/// The number of bytes of the instruction's prefixes and opcode, up to its ModRM byte or immediate.
fn count_opcode_bytes(encoded: &[u8]) -> usize {
    let legacy = encoded.iter().take_while(|byte| LEGACY_PREFIXES.contains(byte)).count();
    let rex = usize::from(encoded.get(legacy).is_some_and(|byte| is_rex(*byte)));
    let opcode = legacy + rex;
    let escape = match encoded.get(opcode..opcode + 2) {
        Some([0x0F, 0x38 | 0x3A]) => 2,
        Some([0x0F, _]) => 1,
        _ => 0,
    };
    (opcode + escape + 1).min(encoded.len())
}

/// The code's page, its first address and its bytes for the layout; prefixes pad it with as many different neutral
/// ones as fit before `padding` repeats, so that a padded instruction need not repeat a prefix.
pub fn lay_out(layout: CodeLayout, encoded: &[u8], padding: u8) -> (u64, u64, Vec<u8>) {
    let padded = |length: usize| {
        let count = length - encoded.len();
        let mut bytes: Vec<u8> = NEUTRAL_PREFIXES
            .iter()
            .copied()
            .filter(|prefix| *prefix != padding && padding != 0x64 && padding != 0x65)
            .take(count.saturating_sub(1))
            .collect();
        bytes.resize(count, padding);
        bytes.extend_from_slice(encoded);
        bytes
    };
    // A REX before a legacy prefix or another REX is ignored, so an instruction that starts with neither gets its
    // padding prefix between them.
    let padded_with_rex = |length: usize| {
        let separated = !encoded.first().is_some_and(|first| LEGACY_PREFIXES.contains(first) || is_rex(*first));
        let mut bytes = vec![PADDING_REX; length - encoded.len() - usize::from(separated)];
        if separated {
            bytes.push(padding);
        }
        bytes.extend_from_slice(encoded);
        bytes
    };
    let length = encoded.len() as u64;
    let edge = LAST_CANONICAL_PAGE + PAGE_BYTES;
    let opcode = count_opcode_bytes(encoded);
    match layout {
        CodeLayout::PageStart => (CODE_ADDRESS, CODE_ADDRESS, encoded.to_vec()),
        CodeLayout::Padded15 => (CODE_ADDRESS, CODE_ADDRESS, padded(15)),
        CodeLayout::Padded16 => (CODE_ADDRESS, CODE_ADDRESS, padded(16)),
        CodeLayout::Padded15Rex => (CODE_ADDRESS, CODE_ADDRESS, padded_with_rex(15)),
        CodeLayout::Padded16Rex => (CODE_ADDRESS, CODE_ADDRESS, padded_with_rex(16)),
        CodeLayout::Padded16AcrossPage => (FAR_CODE_ADDRESS, FAR_CODE_ADDRESS + PAGE_BYTES - 15, padded(16)),
        CodeLayout::AcrossPageIntoUnmapped => {
            (FAR_CODE_ADDRESS, FAR_CODE_ADDRESS + PAGE_BYTES - length + 1, encoded.to_vec())
        }
        CodeLayout::OpcodeAtPageEnd => {
            (FAR_CODE_ADDRESS, FAR_CODE_ADDRESS + PAGE_BYTES - opcode as u64, encoded.to_vec())
        }
        CodeLayout::EndsAtCanonicalEdge => (LAST_CANONICAL_PAGE, edge - length, encoded.to_vec()),
        // The bytes past the edge would lie past the program's addresses, where a state holds nothing.
        CodeLayout::AcrossCanonicalEdge => {
            let past = 1.min(encoded.len() - 1);
            (LAST_CANONICAL_PAGE, edge - length + past as u64, encoded[..encoded.len() - past].to_vec())
        }
        CodeLayout::OpcodeAtCanonicalEdge => {
            (LAST_CANONICAL_PAGE, edge - opcode as u64, encoded[..opcode].to_vec())
        }
    }
}
