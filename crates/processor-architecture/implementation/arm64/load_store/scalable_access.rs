//! The SVE contiguous loads and stores at a 128-bit vector length, with the faults and choices of the parent module:
//! LD1, LDNT1, LD2 to LD4, LD1R, LD1RQ, LDR and their stores and prefetches, as Arm's pseudocode loops over their
//! elements (for example [LD1W (scalar plus scalar)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LD1W--scalar-plus-scalar---Contiguous-load-unsigned-words-to-vector--scalar-index--)).
//!
//! An inactive element is not accessed: a load gives it zero, a store leaves its memory alone, and neither faults on it.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use super::{AccessLayout, STACK_POINTER_ALIGNMENT, find_fault, to_fault_address};
use crate::behavior::{FaultAddressTag, MisalignedStackPointer};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, RN};
use crate::memory::read_bytes_into;
use crate::operand::RegisterThirtyOne;
use crate::registers::{is_general_purpose, read_register};
use crate::scalable::{PG, VECTOR_BITS, ZD, is_active, read_element, read_predicate};
use crate::{
    AccessDirection, Exception, Instruction, PREDICATE_REGISTERS, PagePermission, Processor,
    VECTOR_REGISTERS,
};

/// The register offset of the scalar-plus-scalar forms.
pub const RM: Field = Field::new(16, 5);
/// The element size of ST1B, ST1H and ST1W, a byte shifted left by it.
pub const STORE_SIZE: Field = Field::new(21, 2);
pub const IMMEDIATE4: [Field; 1] = [Field::new(16, 4)];
pub const IMMEDIATE6: [Field; 1] = [Field::new(16, 6)];
/// The immediate of LDR and STR, imm9h:imm9l.
pub const IMMEDIATE9: [Field; 2] = [Field::new(16, 6), Field::new(10, 3)];
/// The predicate LDR and STR (predicate) move.
pub const PT: Field = Field::new(0, 4);
/// The bytes of a predicate at a 128-bit vector length.
const PREDICATE_BYTES: u64 = (VECTOR_BITS / u8::BITS / u8::BITS) as u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessKind {
    Load,
    Store,
    /// A prefetch, which changes nothing a state shows and never faults.
    Prefetch,
}

/// The size of a register's elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementSize {
    Bits(u32),
    /// A byte shifted left by the field, at least the memory's element.
    Field(Field),
}

/// What an access adds to its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Offset {
    /// The register at bits 20 to 16 times the memory's element size; XZR is unallocated.
    Register,
    /// A signed or unsigned immediate times the unit.
    Immediate {
        fields: &'static [Field],
        signed: bool,
        unit: OffsetUnit,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OffsetUnit {
    /// The bytes the access moves: MUL VL.
    Transfer,
    Bytes(u64),
}

/// What the elements a load reads fill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Replication {
    /// Each element from its own memory.
    None,
    /// LD1R: one element read, in every active element.
    Element,
    /// LD1RQ: the elements of 16 bytes, in each 128 bits of the vector.
    Quadword,
}

/// A modeled SVE contiguous access.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalableAccess {
    pub kind: AccessKind,
    /// The bytes of one element in memory.
    pub memory_bytes: u32,
    pub element: ElementSize,
    /// Whether a load sign-extends each element.
    pub signed: bool,
    /// The registers moved, consecutive from Zt, wrapping after Z31.
    pub registers: u32,
    pub offset: Offset,
    pub replication: Replication,
    /// Whether a governing predicate chooses the elements; LDR and STR move every byte.
    pub predicated: bool,
    /// Whether the register moved is the predicate Pt, as LDR and STR (predicate) move.
    pub predicate_register: bool,
}

/// What an SVE store writes of the active elements below a fault.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableStoreAcrossFault {
    WritesNothing,
    WritesBytesBelowFault,
    /// The elements of each vector's length of the access, counted from its first byte, that ends at or below the
    /// fault, as a Neoverse V2 does for ST3B.
    WritesWholeVectorsBelowFault,
    /// Every element before the one that faults, in the order the elements are numbered, as a Neoverse V2 does for a
    /// scatter.
    WritesElementsBeforeFault,
}

/// The choices a chip makes for an SVE access, as for the other loads and stores.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalableAccessChoices {
    pub misaligned_stack_pointer: MisalignedStackPointer,
    pub across_fault: Option<ScalableStoreAcrossFault>,
    pub fault_address_tag: FaultAddressTag,
}

impl ScalableAccess {
    /// The esize of the register's elements in bits, none where the size field is smaller than the memory's element.
    pub fn to_element_bits(self, word: u32) -> Option<u32> {
        match self.element {
            ElementSize::Bits(bits) => Some(bits),
            ElementSize::Field(field) => {
                let size = if crate::planted::is(3616) && self.memory_bytes == 4 {
                    2
                } else {
                    field.read(word)
                };
                let bits = u8::BITS << size;
                (bits >= self.memory_bytes * u8::BITS).then_some(bits)
            }
        }
    }

    /// The bytes the encoding moves at most, and what it adds to its base's value to find the first, given the
    /// registers; an unallocated element size is laid out as the memory's element, so that its state places the base.
    pub fn to_access_layout<LevelSpecific>(
        self,
        word: u32,
        processor: &Processor<LevelSpecific>,
    ) -> Option<AccessLayout> {
        let esize = self
            .to_element_bits(word)
            .unwrap_or(self.memory_bytes * u8::BITS);
        let bytes = self.to_transfer_bytes(esize);
        let offset = match self.offset {
            Offset::Register => {
                read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                    .wrapping_mul(u64::from(self.memory_bytes))
            }
            Offset::Immediate {
                fields,
                signed,
                unit,
            } => {
                let width: u32 = fields.iter().map(|field| field.width).sum();
                let raw = fields.iter().fold(0_u64, |value, field| {
                    value << field.width | u64::from(field.read(word))
                });
                let immediate = if signed {
                    (((raw << (u64::BITS - width)) as i64) >> (u64::BITS - width)) as u64
                } else {
                    raw
                };
                let unit = match unit {
                    OffsetUnit::Transfer => bytes,
                    OffsetUnit::Bytes(unit) => unit,
                };
                immediate.wrapping_mul(unit)
            }
        };
        Some(AccessLayout { bytes, offset })
    }

    /// The bytes the access moves when every element is active.
    fn to_transfer_bytes(self, esize: u32) -> u64 {
        if self.predicate_register {
            return PREDICATE_BYTES;
        }
        let elements = u64::from(VECTOR_BITS / esize);
        match self.replication {
            Replication::Element => u64::from(self.memory_bytes),
            Replication::None | Replication::Quadword => {
                elements * u64::from(self.memory_bytes) * u64::from(self.registers)
            }
        }
    }
}

/// The access of each modeled SVE contiguous load, store and prefetch, none for every other instruction.
pub const fn to_scalable_access(instruction: Instruction<Infallible>) -> Option<ScalableAccess> {
    use AccessKind::{Load, Prefetch, Store};
    use Instruction as I;
    const fn access(
        kind: AccessKind,
        memory_bytes: u32,
        element: ElementSize,
        signed: bool,
        registers: u32,
        offset: Offset,
    ) -> ScalableAccess {
        ScalableAccess {
            kind,
            memory_bytes,
            element,
            signed,
            registers,
            offset,
            replication: Replication::None,
            predicated: true,
            predicate_register: false,
        }
    }
    const fn bits(bits: u32) -> ElementSize {
        ElementSize::Bits(bits)
    }
    const REGISTER: Offset = Offset::Register;
    const VL: Offset = Offset::Immediate {
        fields: &IMMEDIATE4,
        signed: true,
        unit: OffsetUnit::Transfer,
    };
    const fn load(memory_bytes: u32, esize: u32, signed: bool, offset: Offset) -> ScalableAccess {
        access(Load, memory_bytes, bits(esize), signed, 1, offset)
    }
    const fn structure(
        kind: AccessKind,
        memory_bytes: u32,
        registers: u32,
        offset: Offset,
    ) -> ScalableAccess {
        access(
            kind,
            memory_bytes,
            bits(memory_bytes * u8::BITS),
            false,
            registers,
            offset,
        )
    }
    const fn store(memory_bytes: u32, offset: Offset) -> ScalableAccess {
        access(
            Store,
            memory_bytes,
            ElementSize::Field(STORE_SIZE),
            false,
            1,
            offset,
        )
    }
    const fn replicate(memory_bytes: u32, esize: u32, signed: bool) -> ScalableAccess {
        let mut access = access(
            Load,
            memory_bytes,
            bits(esize),
            signed,
            1,
            Offset::Immediate {
                fields: &IMMEDIATE6,
                signed: false,
                unit: OffsetUnit::Bytes(memory_bytes as u64),
            },
        );
        access.replication = Replication::Element;
        access
    }
    const fn quadword(memory_bytes: u32, offset: Offset) -> ScalableAccess {
        let mut access = structure(Load, memory_bytes, 1, offset);
        access.replication = Replication::Quadword;
        access
    }
    const QUADWORD_IMMEDIATE: Offset = Offset::Immediate {
        fields: &IMMEDIATE4,
        signed: true,
        unit: OffsetUnit::Bytes(16),
    };
    const fn whole(kind: AccessKind, predicate_register: bool) -> ScalableAccess {
        let unit = if predicate_register {
            PREDICATE_BYTES
        } else {
            (VECTOR_BITS / u8::BITS) as u64
        };
        let mut access = access(
            kind,
            1,
            bits(u8::BITS),
            false,
            1,
            Offset::Immediate {
                fields: &IMMEDIATE9,
                signed: true,
                unit: OffsetUnit::Bytes(unit),
            },
        );
        access.predicated = false;
        access.predicate_register = predicate_register;
        access
    }
    const fn prefetch(memory_bytes: u32, offset: Offset) -> ScalableAccess {
        access(
            Prefetch,
            memory_bytes,
            bits(memory_bytes * u8::BITS),
            false,
            1,
            offset,
        )
    }
    const PREFETCH_VL: Offset = Offset::Immediate {
        fields: &IMMEDIATE6,
        signed: true,
        unit: OffsetUnit::Transfer,
    };
    Some(match instruction {
        I::Ld1b => load(1, 8, false, REGISTER),
        I::Ld1b16 => load(1, 16, false, REGISTER),
        I::Ld1b32 => load(1, 32, false, REGISTER),
        I::Ld1b64 => load(1, 64, false, REGISTER),
        I::Ld1sw64 => load(4, 64, true, REGISTER),
        I::Ld1h => load(2, 16, false, REGISTER),
        I::Ld1h32 => load(2, 32, false, REGISTER),
        I::Ld1h64 => load(2, 64, false, REGISTER),
        I::Ld1sh64 => load(2, 64, true, REGISTER),
        I::Ld1sh32 => load(2, 32, true, REGISTER),
        I::Ld1w => load(4, 32, false, REGISTER),
        I::Ld1w64 => load(4, 64, false, REGISTER),
        I::Ld1sb64 => load(1, 64, true, REGISTER),
        I::Ld1sb32 => load(1, 32, true, REGISTER),
        I::Ld1sb16 => load(1, 16, true, REGISTER),
        I::Ld1d => load(8, 64, false, REGISTER),
        I::Ld1bImmediate => load(1, 8, false, VL),
        I::Ld1bImmediate16 => load(1, 16, false, VL),
        I::Ld1bImmediate32 => load(1, 32, false, VL),
        I::Ld1bImmediate64 => load(1, 64, false, VL),
        I::Ld1swImmediate64 => load(4, 64, true, VL),
        I::Ld1hImmediate => load(2, 16, false, VL),
        I::Ld1hImmediate32 => load(2, 32, false, VL),
        I::Ld1hImmediate64 => load(2, 64, false, VL),
        I::Ld1shImmediate64 => load(2, 64, true, VL),
        I::Ld1shImmediate32 => load(2, 32, true, VL),
        I::Ld1wImmediate => load(4, 32, false, VL),
        I::Ld1wImmediate64 => load(4, 64, false, VL),
        I::Ld1sbImmediate64 => load(1, 64, true, VL),
        I::Ld1sbImmediate32 => load(1, 32, true, VL),
        I::Ld1sbImmediate16 => load(1, 16, true, VL),
        I::Ld1dImmediate => load(8, 64, false, VL),
        I::Ldnt1bZRR => load(1, 8, false, REGISTER),
        I::Ldnt1hZRR => load(2, 16, false, REGISTER),
        I::Ldnt1wZRR => load(4, 32, false, REGISTER),
        I::Ldnt1dZRR => load(8, 64, false, REGISTER),
        I::Ldnt1bZRI => load(1, 8, false, VL),
        I::Ldnt1hZRI => load(2, 16, false, VL),
        I::Ldnt1wZRI => load(4, 32, false, VL),
        I::Ldnt1dZRI => load(8, 64, false, VL),
        I::Ld2b => structure(Load, 1, 2, REGISTER),
        I::Ld3b => structure(Load, 1, 3, REGISTER),
        I::Ld4b => structure(Load, 1, 4, REGISTER),
        I::Ld2h => structure(Load, 2, 2, REGISTER),
        I::Ld3h => structure(Load, 2, 3, REGISTER),
        I::Ld4h => structure(Load, 2, 4, REGISTER),
        I::Ld2w => structure(Load, 4, 2, REGISTER),
        I::Ld3w => structure(Load, 4, 3, REGISTER),
        I::Ld4w => structure(Load, 4, 4, REGISTER),
        I::Ld2d => structure(Load, 8, 2, REGISTER),
        I::Ld3d => structure(Load, 8, 3, REGISTER),
        I::Ld4d => structure(Load, 8, 4, REGISTER),
        I::Ld2bImmediate => structure(Load, 1, 2, VL),
        I::Ld3bImmediate => structure(Load, 1, 3, VL),
        I::Ld4bImmediate => structure(Load, 1, 4, VL),
        I::Ld2hImmediate => structure(Load, 2, 2, VL),
        I::Ld3hImmediate => structure(Load, 2, 3, VL),
        I::Ld4hImmediate => structure(Load, 2, 4, VL),
        I::Ld2wImmediate => structure(Load, 4, 2, VL),
        I::Ld3wImmediate => structure(Load, 4, 3, VL),
        I::Ld4wImmediate => structure(Load, 4, 4, VL),
        I::Ld2dImmediate => structure(Load, 8, 2, VL),
        I::Ld3dImmediate => structure(Load, 8, 3, VL),
        I::Ld4dImmediate => structure(Load, 8, 4, VL),
        I::St1bZPBr => store(1, REGISTER),
        I::St1hZPBr => store(2, REGISTER),
        I::St1wZPBr => store(4, REGISTER),
        I::St1d => structure(Store, 8, 1, REGISTER),
        I::St1bZPBi => store(1, VL),
        I::St1hZPBi => store(2, VL),
        I::St1wZPBi => store(4, VL),
        I::St1dImmediate => structure(Store, 8, 1, VL),
        I::Stnt1bZRR => structure(Store, 1, 1, REGISTER),
        I::Stnt1hZRR => structure(Store, 2, 1, REGISTER),
        I::Stnt1wZRR => structure(Store, 4, 1, REGISTER),
        I::Stnt1dZRR => structure(Store, 8, 1, REGISTER),
        I::Stnt1bZRI => structure(Store, 1, 1, VL),
        I::Stnt1hZRI => structure(Store, 2, 1, VL),
        I::Stnt1wZRI => structure(Store, 4, 1, VL),
        I::Stnt1dZRI => structure(Store, 8, 1, VL),
        I::St2b => structure(Store, 1, 2, REGISTER),
        I::St3b => structure(Store, 1, 3, REGISTER),
        I::St4b => structure(Store, 1, 4, REGISTER),
        I::St2h => structure(Store, 2, 2, REGISTER),
        I::St3h => structure(Store, 2, 3, REGISTER),
        I::St4h => structure(Store, 2, 4, REGISTER),
        I::St2w => structure(Store, 4, 2, REGISTER),
        I::St3w => structure(Store, 4, 3, REGISTER),
        I::St4w => structure(Store, 4, 4, REGISTER),
        I::St2d => structure(Store, 8, 2, REGISTER),
        I::St3d => structure(Store, 8, 3, REGISTER),
        I::St4d => structure(Store, 8, 4, REGISTER),
        I::St2bImmediate => structure(Store, 1, 2, VL),
        I::St3bImmediate => structure(Store, 1, 3, VL),
        I::St4bImmediate => structure(Store, 1, 4, VL),
        I::St2hImmediate => structure(Store, 2, 2, VL),
        I::St3hImmediate => structure(Store, 2, 3, VL),
        I::St4hImmediate => structure(Store, 2, 4, VL),
        I::St2wImmediate => structure(Store, 4, 2, VL),
        I::St3wImmediate => structure(Store, 4, 3, VL),
        I::St4wImmediate => structure(Store, 4, 4, VL),
        I::St2dImmediate => structure(Store, 8, 2, VL),
        I::St3dImmediate => structure(Store, 8, 3, VL),
        I::St4dImmediate => structure(Store, 8, 4, VL),
        I::Ld1rbImmediate => replicate(1, 8, false),
        I::Ld1rbImmediate16 => replicate(1, 16, false),
        I::Ld1rbImmediate32 => replicate(1, 32, false),
        I::Ld1rbImmediate64 => replicate(1, 64, false),
        I::Ld1rsw => replicate(4, 64, true),
        I::Ld1rhImmediate => replicate(2, 16, false),
        I::Ld1rhImmediate32 => replicate(2, 32, false),
        I::Ld1rhImmediate64 => replicate(2, 64, false),
        I::Ld1rshImmediate64 => replicate(2, 64, true),
        I::Ld1rshImmediate32 => replicate(2, 32, true),
        I::Ld1rwImmediate => replicate(4, 32, false),
        I::Ld1rwImmediate64 => replicate(4, 64, false),
        I::Ld1rsbImmediate64 => replicate(1, 64, true),
        I::Ld1rsbImmediate32 => replicate(1, 32, true),
        I::Ld1rsbImmediate16 => replicate(1, 16, true),
        I::Ld1rd => replicate(8, 64, false),
        I::Ld1rqb8 => quadword(1, REGISTER),
        I::Ld1rqh16 => quadword(2, REGISTER),
        I::Ld1rqwW => quadword(4, REGISTER),
        I::Ld1rqd64 => quadword(8, REGISTER),
        I::Ld1rqbImmediate8 => quadword(1, QUADWORD_IMMEDIATE),
        I::Ld1rqhImmediate16 => quadword(2, QUADWORD_IMMEDIATE),
        I::Ld1rqwWImmediate => quadword(4, QUADWORD_IMMEDIATE),
        I::Ld1rqdImmediate64 => quadword(8, QUADWORD_IMMEDIATE),
        I::LdrZXI => whole(Load, false),
        I::LdrPXI => whole(Load, true),
        I::StrZXI => whole(Store, false),
        I::StrPXI => whole(Store, true),
        I::PrfbPRR => prefetch(1, REGISTER),
        I::PrfhPRR => prefetch(2, REGISTER),
        I::PrfwPRR => prefetch(4, REGISTER),
        I::PrfdPRR => prefetch(8, REGISTER),
        I::PrfbPRI => prefetch(1, PREFETCH_VL),
        I::PrfhPRI => prefetch(2, PREFETCH_VL),
        I::PrfwPRI => prefetch(4, PREFETCH_VL),
        I::PrfdPRI => prefetch(8, PREFETCH_VL),
        _ => return None,
    })
}

/// Applies the access encoded in the word; changes no register when it returns an exception.
pub fn simulate_scalable_access<LevelSpecific, Architecture>(
    access: ScalableAccess,
    choices: ScalableAccessChoices,
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let Some(esize) = access.to_element_bits(word) else {
        return Err(Exception::UndefinedInstruction);
    };
    let Some(layout) = access.to_access_layout(word, processor) else {
        return Err(Exception::UndefinedInstruction);
    };
    let number = RN.read(word);
    let base = read_register(processor, number, RegisterThirtyOne::StackPointer);
    if access.kind == AccessKind::Prefetch && !crate::planted::is(2046) {
        return Ok(());
    }
    if !is_general_purpose(number)
        && !base.is_multiple_of(STACK_POINTER_ALIGNMENT)
        && choices.misaligned_stack_pointer == MisalignedStackPointer::Faults
    {
        return Err(Exception::StackPointerAlignmentFault);
    }
    let offset = if crate::planted::is(2037) {
        layout.offset.wrapping_mul(4)
    } else if crate::planted::is(2043) && layout.offset >> (u64::BITS - 1) != 0 {
        layout.offset.wrapping_add(1 << 13)
    } else if crate::planted::is(2042) {
        layout.offset.wrapping_mul(16) / u64::from(access.memory_bytes)
    } else {
        layout.offset
    };
    let address = base.wrapping_add(offset);
    let elements = VECTOR_BITS / esize;
    let predicate = if access.predicated {
        read_predicate(processor, word, PG)
    } else {
        u16::MAX
    };
    let active = |element: u32| {
        !access.predicated
            || is_active(predicate, element, esize)
            || crate::planted::any(&[2039, 2045])
    };
    let memory_bytes = u64::from(access.memory_bytes);
    // Each element and register moved, in the order the pseudocode moves them, with the address of its bytes.
    let mut moves: Vec<(u32, u32, u64)> = Vec::new();
    match access.replication {
        Replication::Element => {
            if (0..elements).any(active) || crate::planted::is(2041) {
                moves.push((0, 0, address));
            }
        }
        Replication::None | Replication::Quadword => {
            let registers = if access.predicate_register {
                1
            } else {
                access.registers
            };
            let count = if access.predicate_register {
                PREDICATE_BYTES as u32
            } else {
                elements
            };
            for element in 0..count {
                for register in 0..registers {
                    let place = if crate::planted::is(2038) {
                        u64::from(register * elements + element)
                    } else {
                        u64::from(element * registers + register)
                    };
                    if active(element) {
                        moves.push((
                            element,
                            register,
                            address.wrapping_add(place * memory_bytes),
                        ));
                    }
                }
            }
        }
    }
    let tag = choices.fault_address_tag;
    let direction = match access.kind {
        AccessKind::Store => AccessDirection::Write,
        AccessKind::Load | AccessKind::Prefetch => AccessDirection::Read,
    };
    let fault = moves.iter().enumerate().find_map(|(place, (_, _, at))| {
        find_fault(memory, to_untagged_address(*at), memory_bytes, direction)
            .map(|(location, status)| (place, location, status, *at))
    });
    match access.kind {
        AccessKind::Store => {
            let stored = |element: u32, register: u32| -> Vec<u8> {
                let value = if access.predicate_register {
                    u128::from(
                        processor.predicate_registers[PT.read(word) as usize % PREDICATE_REGISTERS]
                            >> (element * u8::BITS)
                            & 0xFF,
                    )
                } else {
                    let register = if crate::planted::is(2040) {
                        0
                    } else {
                        register
                    };
                    let vector = processor.vector_registers
                        [((ZD.read(word) + register) % VECTOR_REGISTERS as u32) as usize];
                    read_element(vector, element, esize)
                };
                value.to_le_bytes()[..memory_bytes as usize].to_vec()
            };
            let written = fault.map_or(moves.len(), |(place, ..)| place);
            let across = choices.across_fault;
            let writes_below =
                fault.is_none() || across == Some(ScalableStoreAcrossFault::WritesBytesBelowFault);
            // The end of the vector's length of the access that holds the byte.
            let vector_bytes = u64::from(VECTOR_BITS / u8::BITS);
            let to_part_end = |at: u64| {
                let start = to_untagged_address(address);
                let into = to_untagged_address(at).wrapping_sub(start);
                start.wrapping_add((into / vector_bytes + 1) * vector_bytes)
            };
            for (place, &(element, register, at)) in moves[..written].iter().enumerate() {
                let whole = match fault {
                    Some((_, location, ..)) => {
                        across == Some(ScalableStoreAcrossFault::WritesWholeVectorsBelowFault)
                            && to_part_end(at) <= location
                    }
                    None => false,
                };
                if (writes_below || whole) && !(crate::planted::is(2044) && place > 0) {
                    mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                        start: to_untagged_address(at),
                        bytes: Cow::Owned(stored(element, register)),
                    });
                }
            }
            if let Some((place, location, status, at)) = fault {
                let (element, register, _) = moves[place];
                let below = location.wrapping_sub(to_untagged_address(at)) as usize;
                if writes_below && below > 0 {
                    mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                        start: to_untagged_address(at),
                        bytes: Cow::Owned(stored(element, register)[..below].to_vec()),
                    });
                }
                return Err(Exception::DataAbort {
                    status,
                    access: direction,
                    address: to_fault_address(at, location, tag),
                });
            }
            Ok(())
        }
        AccessKind::Load => {
            if let Some((_, location, status, at)) = fault {
                return Err(Exception::DataAbort {
                    status,
                    access: direction,
                    address: to_fault_address(at, location, tag),
                });
            }
            let extend = |bytes: &[u8]| -> u128 {
                let mut buffer = [0_u8; size_of::<u128>()];
                buffer[..bytes.len()].copy_from_slice(bytes);
                let value = u128::from_le_bytes(buffer);
                let bits = bytes.len() as u32 * u8::BITS;
                let value = if access.signed {
                    (((value << (u128::BITS - bits)) as i128) >> (u128::BITS - bits)) as u128
                } else {
                    value
                };
                value & (u128::MAX >> (u128::BITS - esize))
            };
            if access.predicate_register {
                let mut bytes = [0_u8; PREDICATE_BYTES as usize];
                read_bytes_into(memory, to_untagged_address(address), &mut bytes);
                processor.predicate_registers[PT.read(word) as usize] = u16::from_le_bytes(bytes);
                return Ok(());
            }
            let mut results = [0_u128; 4];
            if crate::planted::is(2035) {
                let kept = (0..elements)
                    .filter(|element| !active(*element))
                    .fold(0_u128, |mask, element| {
                        mask | (u128::MAX >> (u128::BITS - esize)) << (element * esize)
                    });
                results[0] = processor.vector_registers[ZD.read(word) as usize] & kept;
            }
            for &(element, register, at) in &moves {
                let mut bytes = [0_u8; size_of::<u64>()];
                let bytes = &mut bytes[..memory_bytes as usize];
                read_bytes_into(memory, to_untagged_address(at), bytes);
                let value = extend(bytes);
                match access.replication {
                    Replication::Element => {
                        for element in (0..elements).filter(|element| active(*element)) {
                            results[0] |= value << (element * esize);
                        }
                    }
                    Replication::None | Replication::Quadword => {
                        results[register as usize] |= value << (element * esize);
                    }
                }
            }
            for (register, result) in results.iter().enumerate().take(access.registers as usize) {
                let number = (ZD.read(word) + register as u32) % VECTOR_REGISTERS as u32;
                processor.vector_registers[number as usize] = *result;
            }
            Ok(())
        }
        AccessKind::Prefetch => Ok(()),
    }
}
