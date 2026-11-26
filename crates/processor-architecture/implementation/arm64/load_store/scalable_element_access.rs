//! The SVE accesses whose elements each find their own address, at a 128-bit vector length: the gathers, scatters and
//! prefetches of vector addresses, the non-temporal accesses of a vector plus a scalar, and the first-fault and
//! non-fault contiguous loads, as Arm's pseudocode loops over their elements (for example
//! [LDFF1W (scalar plus vector)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LDFF1W--scalar-plus-vector---Gather-load-first-fault-unsigned-words-to-vector--vector-index--)
//! and [ST1W (scalar plus vector)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/ST1W--scalar-plus-vector---Scatter-store-words-from-a-vector--vector-index--)).
//!
//! An inactive element is not accessed. A first-fault load faults only on its first active element; on a later one,
//! and a non-fault load on every one, the access is suppressed instead, and FFR is cleared from that element on.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use super::scalable_access::{AccessKind, ScalableStoreAcrossFault};
use super::{AccessLayout, STACK_POINTER_ALIGNMENT, find_fault, to_fault_address};
use crate::behavior::{FaultAddressTag, MisalignedStackPointer};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, RN};
use crate::memory::read_bytes_into;
use crate::operand::RegisterThirtyOne;
use crate::planted::{PlantedBug, PlantedBy};
use crate::registers::{is_general_purpose, read_register};
use crate::scalable::{PG, VECTOR_BITS, ZD, ZM, ZN, is_active, read_element, read_predicate};
use crate::{
    AccessDirection, Exception, Instruction, PagePermission, PreciseInstruction, Processor,
};

/// The register Xm of the scalar-plus-scalar and vector-plus-scalar forms.
pub const RM: Field = Field::new(16, 5);
/// The signed immediate of LDNF1, in vectors.
pub const IMMEDIATE4: Field = Field::new(16, 4);
/// The unsigned immediate of the vector-plus-immediate forms, in elements of memory.
pub const IMMEDIATE5: Field = Field::new(16, 5);
/// The prefetch operation, which changes nothing a state shows.
pub const PREFETCH_OPERATION: Field = Field::new(0, 4);
/// Whether a gather's or prefetch's 32-bit offsets are sign-extended (xs).
pub const LOAD_EXTEND: Option<Field> = Some(Field::new(22, 1));
/// Whether a scatter's 32-bit offsets are sign-extended (xs).
pub const STORE_EXTEND: Option<Field> = Some(Field::new(14, 1));

/// How a load treats a fault on an element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Faulting {
    /// Every access may fault, and a store or prefetch.
    Every,
    /// LDFF1: the first active element may fault; a later one is suppressed.
    AfterFirst,
    /// LDNF1: every element is suppressed.
    Never,
}

/// Where each element's address comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Addressing {
    /// Xn or SP, plus Xm and the element's number, times the memory's element.
    ScalarPlusScalar,
    /// Xn or SP, plus the signed immediate times the elements and the element's number, times the memory's element.
    ScalarPlusImmediate,
    /// Xn or SP, plus the low `offset_bits` of each element of Zm, sign-extended where the field holds one, and times
    /// the memory's element where scaled.
    ScalarPlusVector {
        offset_bits: u32,
        extension: Option<Field>,
        scaled: bool,
    },
    /// Each element of Zn, plus the unsigned immediate times the memory's element.
    VectorPlusImmediate,
    /// Each element of Zn, plus Xm or zero.
    VectorPlusScalar,
}

impl Addressing {
    /// Whether the base is a general-purpose register or SP, the number Rn names.
    pub const fn has_scalar_base(self) -> bool {
        matches!(
            self,
            Self::ScalarPlusScalar | Self::ScalarPlusImmediate | Self::ScalarPlusVector { .. }
        )
    }
}

/// A modeled SVE access of elements that each find their own address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ElementAccess {
    pub kind: AccessKind,
    pub faulting: Faulting,
    /// The bytes of one element in memory.
    pub memory_bytes: u32,
    pub element_bits: u32,
    /// Whether a load sign-extends each element.
    pub signed: bool,
    pub addressing: Addressing,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LDFF1W--scalar-plus-vector---Gather-load-first-fault-unsigned-words-to-vector--vector-index--#:~:text=CONSTRAINED%20UNPREDICTABLE)
/// value of an inactive element at or after one whose FFR bit is clear, after a first-fault or non-fault load that
/// suppresses no access: zero, which the choice of what it read gives too, or what the register held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnknownElement {
    Zero,
    Merge,
}

/// The choices a chip makes for an access of elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ElementAccessChoices {
    pub misaligned_stack_pointer: MisalignedStackPointer,
    pub across_fault: Option<ScalableStoreAcrossFault>,
    pub fault_address_tag: FaultAddressTag,
    pub unknown_element: Option<UnknownElement>,
}

impl ElementAccess {
    pub const fn elements(self) -> u32 {
        VECTOR_BITS / self.element_bits
    }

    /// Whether a fault of an element may be suppressed, as a first-fault or non-fault load does.
    pub const fn suppresses_faults(self) -> bool {
        !matches!(self.faulting, Faulting::Every)
    }

    /// Whether the base is Zn.
    pub const fn has_vector_base(self) -> bool {
        !self.addressing.has_scalar_base()
    }

    /// The bytes the access spans when every element lies after the one before, and what it adds to its base, Xn or
    /// each element of Zn, to find the first: the offset of the forms of a scalar or an immediate, zero for a vector.
    pub fn to_access_layout<LevelSpecific>(
        self,
        word: u32,
        processor: &Processor<LevelSpecific>,
    ) -> AccessLayout {
        let memory_bytes = u64::from(self.memory_bytes);
        let elements = u64::from(self.elements());
        let offset = match self.addressing {
            Addressing::ScalarPlusScalar => {
                read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                    .wrapping_mul(memory_bytes)
            }
            Addressing::ScalarPlusImmediate => {
                (IMMEDIATE4.read_signed(word) as u64).wrapping_mul(elements * memory_bytes)
            }
            Addressing::ScalarPlusVector { .. } => 0,
            Addressing::VectorPlusImmediate => u64::from(IMMEDIATE5.read(word)) * memory_bytes,
            Addressing::VectorPlusScalar => {
                read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
            }
        };
        AccessLayout {
            bytes: elements * memory_bytes,
            offset,
        }
    }

    /// The address of the element.
    fn to_address<LevelSpecific>(
        self,
        processor: &Processor<LevelSpecific>,
        word: u32,
        element: u32,
    ) -> u64 {
        let memory_bytes = u64::from(self.memory_bytes);
        let base = read_register(processor, RN.read(word), RegisterThirtyOne::StackPointer);
        let index = u64::from(element);
        match self.addressing {
            Addressing::ScalarPlusScalar => {
                let offset =
                    read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister);
                base.wrapping_add(offset.wrapping_add(index).wrapping_mul(memory_bytes))
            }
            Addressing::ScalarPlusImmediate => {
                let vectors = if crate::planted::is(3514) {
                    IMMEDIATE4.read_signed(word) as u64
                } else {
                    (IMMEDIATE4.read_signed(word) as u64).wrapping_mul(u64::from(self.elements()))
                };
                base.wrapping_add(vectors.wrapping_add(index).wrapping_mul(memory_bytes))
            }
            Addressing::ScalarPlusVector {
                offset_bits,
                extension,
                scaled,
            } => {
                let offset_bits = if crate::planted::is(3511) {
                    self.element_bits
                } else {
                    offset_bits
                };
                let raw = read_element(
                    processor.vector_registers[ZM.read(word) as usize],
                    element,
                    self.element_bits,
                ) as u64;
                let low = raw & (u64::MAX >> (u64::BITS - offset_bits));
                let signed = extension.is_some_and(|field| field.read(word) == 1)
                    && !crate::planted::is(3503);
                let offset = if signed {
                    ((low << (u64::BITS - offset_bits)) as i64 >> (u64::BITS - offset_bits)) as u64
                } else {
                    low
                };
                let shift = if scaled && !crate::planted::is(3504) {
                    self.memory_bytes.trailing_zeros()
                } else {
                    0
                };
                base.wrapping_add(offset << shift)
            }
            Addressing::VectorPlusImmediate => {
                let unit = if crate::planted::is(3505) {
                    1
                } else {
                    memory_bytes
                };
                self.to_vector_base(processor, word, element)
                    .wrapping_add(u64::from(IMMEDIATE5.read(word)) * unit)
            }
            Addressing::VectorPlusScalar => {
                let offset = if crate::planted::is(3509) {
                    0
                } else {
                    read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                };
                self.to_vector_base(processor, word, element)
                    .wrapping_add(offset)
            }
        }
    }

    /// The element of Zn, zero-extended.
    fn to_vector_base<LevelSpecific>(
        self,
        processor: &Processor<LevelSpecific>,
        word: u32,
        element: u32,
    ) -> u64 {
        read_element(
            processor.vector_registers[ZN.read(word) as usize],
            element,
            self.element_bits,
        ) as u64
    }
}

/// The access of each modeled SVE access of elements, none for every other instruction.
pub const fn to_element_access(instruction: Instruction<Infallible>) -> Option<ElementAccess> {
    use Instruction as I;
    /// The kinds the table names: a load of each faulting, a store or a prefetch.
    #[derive(Clone, Copy)]
    enum Kind {
        Load,
        FirstFault,
        NonFault,
        Store,
        Prefetch,
    }
    use Kind::{FirstFault, Load, NonFault, Prefetch, Store};
    const fn access(
        kind: Kind,
        memory_bytes: u32,
        element_bits: u32,
        signed: bool,
        addressing: Addressing,
    ) -> ElementAccess {
        let (kind, faulting) = match kind {
            Load => (AccessKind::Load, Faulting::Every),
            FirstFault => (AccessKind::Load, Faulting::AfterFirst),
            NonFault => (AccessKind::Load, Faulting::Never),
            Store => (AccessKind::Store, Faulting::Every),
            Prefetch => (AccessKind::Prefetch, Faulting::Every),
        };
        ElementAccess {
            kind,
            faulting,
            memory_bytes,
            element_bits,
            signed,
            addressing,
        }
    }
    const fn vector(offset_bits: u32, extension: Option<Field>, scaled: bool) -> Addressing {
        Addressing::ScalarPlusVector {
            offset_bits,
            extension,
            scaled,
        }
    }
    const SCALAR_PLUS_SCALAR: Addressing = Addressing::ScalarPlusScalar;
    const SCALAR_PLUS_IMMEDIATE: Addressing = Addressing::ScalarPlusImmediate;
    const VECTOR_PLUS_IMMEDIATE: Addressing = Addressing::VectorPlusImmediate;
    const VECTOR_PLUS_SCALAR: Addressing = Addressing::VectorPlusScalar;
    Some(match instruction {
        I::Ld1sbGatherExtendedOffset32 => access(Load, 1, 32, true, vector(32, LOAD_EXTEND, false)),
        I::Ld1shGatherExtendedOffset32 => access(Load, 2, 32, true, vector(32, LOAD_EXTEND, false)),
        I::Ld1wGatherExtendedOffset => access(Load, 4, 32, false, vector(32, LOAD_EXTEND, false)),
        I::Ld1bGatherExtendedOffset32 => access(Load, 1, 32, false, vector(32, LOAD_EXTEND, false)),
        I::Ld1hGatherExtendedOffset32 => access(Load, 2, 32, false, vector(32, LOAD_EXTEND, false)),
        I::Ldff1sbGatherExtendedOffset32 => {
            access(FirstFault, 1, 32, true, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1shGatherExtendedOffset32 => {
            access(FirstFault, 2, 32, true, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1wGatherExtendedOffset => {
            access(FirstFault, 4, 32, false, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1bGatherExtendedOffset32 => {
            access(FirstFault, 1, 32, false, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1hGatherExtendedOffset32 => {
            access(FirstFault, 2, 32, false, vector(32, LOAD_EXTEND, false))
        }
        I::PrfbSCALEDExtendedOffset32 => {
            access(Prefetch, 1, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfhSCALEDExtendedOffset32 => {
            access(Prefetch, 2, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfwSCALEDExtendedOffset32 => {
            access(Prefetch, 4, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfdSCALEDExtendedOffset32 => {
            access(Prefetch, 8, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1shGatherSCALEDExtendedOffset32 => {
            access(Load, 2, 32, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1hGatherSCALEDExtendedOffset32 => {
            access(Load, 2, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1shGatherSCALEDExtendedOffset32 => {
            access(FirstFault, 2, 32, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1hGatherSCALEDExtendedOffset32 => {
            access(FirstFault, 2, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1wGatherSCALEDExtendedOffset => {
            access(Load, 4, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1wGatherSCALEDExtendedOffset => {
            access(FirstFault, 4, 32, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldnt1sbZZR32 => access(Load, 1, 32, true, VECTOR_PLUS_SCALAR),
        I::Ldnt1shZZR32 => access(Load, 2, 32, true, VECTOR_PLUS_SCALAR),
        I::Ldnt1wZZR32 => access(Load, 4, 32, false, VECTOR_PLUS_SCALAR),
        I::Ldnt1bZZR32 => access(Load, 1, 32, false, VECTOR_PLUS_SCALAR),
        I::Ldnt1hZZR32 => access(Load, 2, 32, false, VECTOR_PLUS_SCALAR),
        I::PrfbPZI32 => access(Prefetch, 1, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfhPZI32 => access(Prefetch, 2, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfwPZI32 => access(Prefetch, 4, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfdPZI32 => access(Prefetch, 8, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1sbGatherImmediate32 => access(Load, 1, 32, true, VECTOR_PLUS_IMMEDIATE),
        I::Ld1shGatherImmediate32 => access(Load, 2, 32, true, VECTOR_PLUS_IMMEDIATE),
        I::Ld1wGatherImmediate => access(Load, 4, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1bGatherImmediate32 => access(Load, 1, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1hGatherImmediate32 => access(Load, 2, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1sbGatherImmediate32 => access(FirstFault, 1, 32, true, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1shGatherImmediate32 => access(FirstFault, 2, 32, true, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1wGatherImmediate => access(FirstFault, 4, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1bGatherImmediate32 => access(FirstFault, 1, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1hGatherImmediate32 => access(FirstFault, 2, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1b => access(FirstFault, 1, 8, false, SCALAR_PLUS_SCALAR),
        I::Ldff1b16 => access(FirstFault, 1, 16, false, SCALAR_PLUS_SCALAR),
        I::Ldff1b32 => access(FirstFault, 1, 32, false, SCALAR_PLUS_SCALAR),
        I::Ldff1b64 => access(FirstFault, 1, 64, false, SCALAR_PLUS_SCALAR),
        I::Ldff1sw64 => access(FirstFault, 4, 64, true, SCALAR_PLUS_SCALAR),
        I::Ldff1h => access(FirstFault, 2, 16, false, SCALAR_PLUS_SCALAR),
        I::Ldff1h32 => access(FirstFault, 2, 32, false, SCALAR_PLUS_SCALAR),
        I::Ldff1h64 => access(FirstFault, 2, 64, false, SCALAR_PLUS_SCALAR),
        I::Ldff1sh64 => access(FirstFault, 2, 64, true, SCALAR_PLUS_SCALAR),
        I::Ldff1sh32 => access(FirstFault, 2, 32, true, SCALAR_PLUS_SCALAR),
        I::Ldff1w => access(FirstFault, 4, 32, false, SCALAR_PLUS_SCALAR),
        I::Ldff1w64 => access(FirstFault, 4, 64, false, SCALAR_PLUS_SCALAR),
        I::Ldff1sb64 => access(FirstFault, 1, 64, true, SCALAR_PLUS_SCALAR),
        I::Ldff1sb32 => access(FirstFault, 1, 32, true, SCALAR_PLUS_SCALAR),
        I::Ldff1sb16 => access(FirstFault, 1, 16, true, SCALAR_PLUS_SCALAR),
        I::Ldff1d => access(FirstFault, 8, 64, false, SCALAR_PLUS_SCALAR),
        I::Ldnf1bImmediate => access(NonFault, 1, 8, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1bImmediate16 => access(NonFault, 1, 16, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1bImmediate32 => access(NonFault, 1, 32, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1bImmediate64 => access(NonFault, 1, 64, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1sw => access(NonFault, 4, 64, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1hImmediate => access(NonFault, 2, 16, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1hImmediate32 => access(NonFault, 2, 32, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1hImmediate64 => access(NonFault, 2, 64, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1shImmediate64 => access(NonFault, 2, 64, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1shImmediate32 => access(NonFault, 2, 32, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1wImmediate => access(NonFault, 4, 32, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1wImmediate64 => access(NonFault, 4, 64, false, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1sbImmediate64 => access(NonFault, 1, 64, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1sbImmediate32 => access(NonFault, 1, 32, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1sbImmediate16 => access(NonFault, 1, 16, true, SCALAR_PLUS_IMMEDIATE),
        I::Ldnf1d => access(NonFault, 8, 64, false, SCALAR_PLUS_IMMEDIATE),
        I::Ld1sbGatherExtendedOffset64 => access(Load, 1, 64, true, vector(32, LOAD_EXTEND, false)),
        I::Ld1shGatherExtendedOffset64 => access(Load, 2, 64, true, vector(32, LOAD_EXTEND, false)),
        I::Ld1swGatherExtendedOffset64 => access(Load, 4, 64, true, vector(32, LOAD_EXTEND, false)),
        I::Ld1dGatherExtendedOffset => access(Load, 8, 64, false, vector(32, LOAD_EXTEND, false)),
        I::Ld1bGatherExtendedOffset64 => access(Load, 1, 64, false, vector(32, LOAD_EXTEND, false)),
        I::Ld1hGatherExtendedOffset64 => access(Load, 2, 64, false, vector(32, LOAD_EXTEND, false)),
        I::Ld1wGatherExtendedOffset64 => access(Load, 4, 64, false, vector(32, LOAD_EXTEND, false)),
        I::Ldff1sbGatherExtendedOffset64 => {
            access(FirstFault, 1, 64, true, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1shGatherExtendedOffset64 => {
            access(FirstFault, 2, 64, true, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1swGatherExtendedOffset64 => {
            access(FirstFault, 4, 64, true, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1dGatherExtendedOffset => {
            access(FirstFault, 8, 64, false, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1bGatherExtendedOffset64 => {
            access(FirstFault, 1, 64, false, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1hGatherExtendedOffset64 => {
            access(FirstFault, 2, 64, false, vector(32, LOAD_EXTEND, false))
        }
        I::Ldff1wGatherExtendedOffset64 => {
            access(FirstFault, 4, 64, false, vector(32, LOAD_EXTEND, false))
        }
        I::PrfbSCALEDExtendedOffset64 => {
            access(Prefetch, 1, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfhSCALEDExtendedOffset64 => {
            access(Prefetch, 2, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfwSCALEDExtendedOffset64 => {
            access(Prefetch, 4, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::PrfdSCALEDExtendedOffset64 => {
            access(Prefetch, 8, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1shGatherSCALEDExtendedOffset64 => {
            access(Load, 2, 64, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1swGatherSCALEDExtendedOffset64 => {
            access(Load, 4, 64, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1dGatherSCALEDExtendedOffset => {
            access(Load, 8, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1hGatherSCALEDExtendedOffset64 => {
            access(Load, 2, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ld1wGatherSCALEDExtendedOffset64 => {
            access(Load, 4, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1shGatherSCALEDExtendedOffset64 => {
            access(FirstFault, 2, 64, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1swGatherSCALEDExtendedOffset64 => {
            access(FirstFault, 4, 64, true, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1dGatherSCALEDExtendedOffset => {
            access(FirstFault, 8, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1hGatherSCALEDExtendedOffset64 => {
            access(FirstFault, 2, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldff1wGatherSCALEDExtendedOffset64 => {
            access(FirstFault, 4, 64, false, vector(32, LOAD_EXTEND, true))
        }
        I::Ldnt1sbZZR64 => access(Load, 1, 64, true, VECTOR_PLUS_SCALAR),
        I::Ldnt1shZZR64 => access(Load, 2, 64, true, VECTOR_PLUS_SCALAR),
        I::Ldnt1sw => access(Load, 4, 64, true, VECTOR_PLUS_SCALAR),
        I::Ldnt1dZZR64 => access(Load, 8, 64, false, VECTOR_PLUS_SCALAR),
        I::Ldnt1bZZR64 => access(Load, 1, 64, false, VECTOR_PLUS_SCALAR),
        I::Ldnt1hZZR64 => access(Load, 2, 64, false, VECTOR_PLUS_SCALAR),
        I::Ldnt1wZZR64 => access(Load, 4, 64, false, VECTOR_PLUS_SCALAR),
        I::PrfbPZI64 => access(Prefetch, 1, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfhPZI64 => access(Prefetch, 2, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfwPZI64 => access(Prefetch, 4, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::PrfdPZI64 => access(Prefetch, 8, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1sbGatherImmediate64 => access(Load, 1, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ld1shGatherImmediate64 => access(Load, 2, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ld1swGatherImmediate64 => access(Load, 4, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ld1dGatherImmediate => access(Load, 8, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1bGatherImmediate64 => access(Load, 1, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1hGatherImmediate64 => access(Load, 2, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1wGatherImmediate64 => access(Load, 4, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1sbGatherImmediate64 => access(FirstFault, 1, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1shGatherImmediate64 => access(FirstFault, 2, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1swGatherImmediate64 => access(FirstFault, 4, 64, true, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1dGatherImmediate => access(FirstFault, 8, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1bGatherImmediate64 => access(FirstFault, 1, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1hGatherImmediate64 => access(FirstFault, 2, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ldff1wGatherImmediate64 => access(FirstFault, 4, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::Ld1sbGather64 => access(Load, 1, 64, true, vector(64, None, false)),
        I::Ld1shGather64 => access(Load, 2, 64, true, vector(64, None, false)),
        I::Ld1swGather64 => access(Load, 4, 64, true, vector(64, None, false)),
        I::Ld1dGather => access(Load, 8, 64, false, vector(64, None, false)),
        I::Ld1bGather64 => access(Load, 1, 64, false, vector(64, None, false)),
        I::Ld1hGather64 => access(Load, 2, 64, false, vector(64, None, false)),
        I::Ld1wGather64 => access(Load, 4, 64, false, vector(64, None, false)),
        I::Ldff1sbGather64 => access(FirstFault, 1, 64, true, vector(64, None, false)),
        I::Ldff1shGather64 => access(FirstFault, 2, 64, true, vector(64, None, false)),
        I::Ldff1swGather64 => access(FirstFault, 4, 64, true, vector(64, None, false)),
        I::Ldff1dGather => access(FirstFault, 8, 64, false, vector(64, None, false)),
        I::Ldff1bGather64 => access(FirstFault, 1, 64, false, vector(64, None, false)),
        I::Ldff1hGather64 => access(FirstFault, 2, 64, false, vector(64, None, false)),
        I::Ldff1wGather64 => access(FirstFault, 4, 64, false, vector(64, None, false)),
        I::PrfbSCALED64 => access(Prefetch, 1, 64, false, vector(64, None, true)),
        I::PrfhSCALED64 => access(Prefetch, 2, 64, false, vector(64, None, true)),
        I::PrfwSCALED64 => access(Prefetch, 4, 64, false, vector(64, None, true)),
        I::PrfdSCALED64 => access(Prefetch, 8, 64, false, vector(64, None, true)),
        I::Ld1shGatherSCALED64 => access(Load, 2, 64, true, vector(64, None, true)),
        I::Ld1swGatherSCALED64 => access(Load, 4, 64, true, vector(64, None, true)),
        I::Ld1dGatherSCALED => access(Load, 8, 64, false, vector(64, None, true)),
        I::Ld1hGatherSCALED64 => access(Load, 2, 64, false, vector(64, None, true)),
        I::Ld1wGatherSCALED64 => access(Load, 4, 64, false, vector(64, None, true)),
        I::Ldff1shGatherSCALED64 => access(FirstFault, 2, 64, true, vector(64, None, true)),
        I::Ldff1swGatherSCALED64 => access(FirstFault, 4, 64, true, vector(64, None, true)),
        I::Ldff1dGatherSCALED => access(FirstFault, 8, 64, false, vector(64, None, true)),
        I::Ldff1hGatherSCALED64 => access(FirstFault, 2, 64, false, vector(64, None, true)),
        I::Ldff1wGatherSCALED64 => access(FirstFault, 4, 64, false, vector(64, None, true)),
        I::Stnt1bZZR64 => access(Store, 1, 64, false, VECTOR_PLUS_SCALAR),
        I::Stnt1hZZR64 => access(Store, 2, 64, false, VECTOR_PLUS_SCALAR),
        I::Stnt1wZZR64 => access(Store, 4, 64, false, VECTOR_PLUS_SCALAR),
        I::Stnt1dZZR64 => access(Store, 8, 64, false, VECTOR_PLUS_SCALAR),
        I::Stnt1bZZR => access(Store, 1, 32, false, VECTOR_PLUS_SCALAR),
        I::Stnt1hZZR => access(Store, 2, 32, false, VECTOR_PLUS_SCALAR),
        I::Stnt1wZZR => access(Store, 4, 32, false, VECTOR_PLUS_SCALAR),
        I::St1bZPBzDX32Unscaled => access(Store, 1, 64, false, vector(32, STORE_EXTEND, false)),
        I::St1hZPBzDX32Unscaled => access(Store, 2, 64, false, vector(32, STORE_EXTEND, false)),
        I::St1wZPBzDX32Unscaled => access(Store, 4, 64, false, vector(32, STORE_EXTEND, false)),
        I::St1dZPBzDX32Unscaled => access(Store, 8, 64, false, vector(32, STORE_EXTEND, false)),
        I::St1bZPBzSX32Unscaled => access(Store, 1, 32, false, vector(32, STORE_EXTEND, false)),
        I::St1hZPBzSX32Unscaled => access(Store, 2, 32, false, vector(32, STORE_EXTEND, false)),
        I::St1wZPBzSX32Unscaled => access(Store, 4, 32, false, vector(32, STORE_EXTEND, false)),
        I::St1hZPBzDX32Scaled => access(Store, 2, 64, false, vector(32, STORE_EXTEND, true)),
        I::St1wZPBzDX32Scaled => access(Store, 4, 64, false, vector(32, STORE_EXTEND, true)),
        I::St1dZPBzDX32Scaled => access(Store, 8, 64, false, vector(32, STORE_EXTEND, true)),
        I::St1hZPBzSX32Scaled => access(Store, 2, 32, false, vector(32, STORE_EXTEND, true)),
        I::St1wZPBzSX32Scaled => access(Store, 4, 32, false, vector(32, STORE_EXTEND, true)),
        I::St1bScatter64 => access(Store, 1, 64, false, vector(64, None, false)),
        I::St1hScatter64 => access(Store, 2, 64, false, vector(64, None, false)),
        I::St1wScatter64 => access(Store, 4, 64, false, vector(64, None, false)),
        I::St1dScatter => access(Store, 8, 64, false, vector(64, None, false)),
        I::St1hScatterSCALED64 => access(Store, 2, 64, false, vector(64, None, true)),
        I::St1wScatterSCALED64 => access(Store, 4, 64, false, vector(64, None, true)),
        I::St1dScatterSCALED => access(Store, 8, 64, false, vector(64, None, true)),
        I::St1bScatterImmediate64 => access(Store, 1, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::St1hScatterImmediate64 => access(Store, 2, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::St1wScatterImmediate64 => access(Store, 4, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::St1dScatterImmediate => access(Store, 8, 64, false, VECTOR_PLUS_IMMEDIATE),
        I::St1bScatterImmediate => access(Store, 1, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::St1hScatterImmediate => access(Store, 2, 32, false, VECTOR_PLUS_IMMEDIATE),
        I::St1wScatterImmediate => access(Store, 4, 32, false, VECTOR_PLUS_IMMEDIATE),
        _ => return None,
    })
}

/// Each combination of the choices the access has: a prefetch none; SP's alignment only where SP can be the base; a
/// fault address's tag only where the access can abort on an address with a tag; what a store writes below a fault;
/// and for a first-fault or non-fault load, the value of an element after one whose FFR bit is clear.
pub(crate) fn list_element_access_behaviors(
    instruction: Instruction<Infallible>,
    access: ElementAccess,
) -> Vec<PreciseInstruction<Infallible>> {
    let mut behaviors = Vec::new();
    if access.kind == AccessKind::Prefetch {
        behaviors.push(PreciseInstruction::ElementAccess {
            instruction,
            choices: ElementAccessChoices {
                misaligned_stack_pointer: MisalignedStackPointer::Faults,
                across_fault: None,
                fault_address_tag: FaultAddressTag::KeepsTopByte,
                unknown_element: None,
            },
        });
        return behaviors;
    }
    let stack_pointer: &[MisalignedStackPointer] = if access.addressing.has_scalar_base() {
        &[
            MisalignedStackPointer::Faults,
            MisalignedStackPointer::Accesses,
        ]
    } else {
        &[MisalignedStackPointer::Faults]
    };
    // A 32-bit vector base has no top byte, and a non-fault load never aborts.
    let tagged = access.faulting != Faulting::Never
        && !(access.has_vector_base() && access.element_bits < u64::BITS);
    let tags: &[FaultAddressTag] = if tagged {
        &[
            FaultAddressTag::KeepsTopByte,
            FaultAddressTag::ClearsTopNibble,
        ]
    } else {
        &[FaultAddressTag::KeepsTopByte]
    };
    // An element of one byte never lies across a fault, so that its bytes below are the elements before it.
    let across: &[Option<ScalableStoreAcrossFault>] =
        if access.kind == AccessKind::Store && access.memory_bytes == 1 {
            &[
                Some(ScalableStoreAcrossFault::WritesNothing),
                Some(ScalableStoreAcrossFault::WritesBytesBelowFault),
            ]
        } else if access.kind == AccessKind::Store {
            &[
                Some(ScalableStoreAcrossFault::WritesNothing),
                Some(ScalableStoreAcrossFault::WritesBytesBelowFault),
                Some(ScalableStoreAcrossFault::WritesElementsBeforeFault),
            ]
        } else {
            &[None]
        };
    let unknown: &[Option<UnknownElement>] = if access.suppresses_faults() {
        &[Some(UnknownElement::Zero), Some(UnknownElement::Merge)]
    } else {
        &[None]
    };
    for &misaligned_stack_pointer in stack_pointer {
        for &across_fault in across {
            for &fault_address_tag in tags {
                for &unknown_element in unknown {
                    behaviors.push(PreciseInstruction::ElementAccess {
                        instruction,
                        choices: ElementAccessChoices {
                            misaligned_stack_pointer,
                            across_fault,
                            fault_address_tag,
                            unknown_element,
                        },
                    });
                }
            }
        }
    }
    behaviors
}

/// Applies the access encoded in the word; changes no register when it returns an exception.
pub fn simulate_element_access<LevelSpecific, Architecture>(
    access: ElementAccess,
    choices: ElementAccessChoices,
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    if access.kind == AccessKind::Prefetch {
        return Ok(());
    }
    let number = RN.read(word);
    if access.addressing.has_scalar_base()
        && !is_general_purpose(number)
        && !processor
            .stack_pointer
            .is_multiple_of(STACK_POINTER_ALIGNMENT)
        && choices.misaligned_stack_pointer == MisalignedStackPointer::Faults
        && !crate::planted::is(3515)
    {
        return Err(Exception::StackPointerAlignmentFault);
    }
    let esize = access.element_bits;
    let elements = access.elements();
    let memory_bytes = u64::from(access.memory_bytes);
    let predicate = read_predicate(processor, word, PG);
    let active = |element: u32| is_active(predicate, element, esize);
    // Each active element in the pseudocode's order, with its address.
    let order: Vec<u32> = if crate::planted::is(3506) {
        (0..elements).rev().collect()
    } else {
        (0..elements).collect()
    };
    let moves: Vec<(u32, u64)> = order
        .into_iter()
        .filter(|element| active(*element))
        .map(|element| (element, access.to_address(processor, word, element)))
        .collect();
    let direction = match access.kind {
        AccessKind::Store => AccessDirection::Write,
        AccessKind::Load | AccessKind::Prefetch => AccessDirection::Read,
    };
    let find = |at: u64| find_fault(memory, to_untagged_address(at), memory_bytes, direction);
    let abort = |at: u64, location: u64, status| Exception::DataAbort {
        status,
        access: direction,
        address: to_fault_address(at, location, choices.fault_address_tag),
    };
    if access.kind == AccessKind::Store {
        let source = processor.vector_registers[ZD.read(word) as usize];
        let stored = |element: u32| -> Vec<u8> {
            let bytes = if crate::planted::is(3512) {
                esize / u8::BITS
            } else {
                access.memory_bytes
            };
            read_element(source, element, esize).to_le_bytes()[..bytes as usize].to_vec()
        };
        let fault = moves.iter().enumerate().find_map(|(place, (element, at))| {
            find(*at).map(|(location, status)| (place, *element, *at, location, status))
        });
        let written = fault.map_or(moves.len(), |(place, ..)| place);
        let writes_bytes =
            choices.across_fault == Some(ScalableStoreAcrossFault::WritesBytesBelowFault);
        let writes_below = fault.is_none()
            || writes_bytes
            || choices.across_fault == Some(ScalableStoreAcrossFault::WritesElementsBeforeFault);
        if writes_below {
            for &(element, at) in &moves[..written] {
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: to_untagged_address(at),
                    bytes: Cow::Owned(stored(element)),
                });
            }
        }
        if let Some((_, element, at, location, status)) = fault {
            let below = location.wrapping_sub(to_untagged_address(at)) as usize;
            if writes_bytes && below > 0 {
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: to_untagged_address(at),
                    bytes: Cow::Owned(stored(element)[..below].to_vec()),
                });
            }
            return Err(abort(at, location, status));
        }
        return Ok(());
    }
    let extend = |bytes: &[u8]| -> u128 {
        let mut buffer = [0_u8; size_of::<u64>()];
        buffer[..bytes.len()].copy_from_slice(bytes);
        let value = u64::from_le_bytes(buffer);
        let bits = bytes.len() as u32 * u8::BITS;
        let value = if access.signed && !crate::planted::is(3507) {
            (((value << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)) as u64
        } else {
            value
        };
        u128::from(value) & (u128::MAX >> (u128::BITS - esize))
    };
    let read = |at: u64| -> u128 {
        let mut bytes = [0_u8; size_of::<u64>()];
        let bytes = &mut bytes[..memory_bytes as usize];
        read_bytes_into(memory, to_untagged_address(at), bytes);
        extend(bytes)
    };
    let target = ZD.read(word) as usize;
    let original = processor.vector_registers[target];
    if !access.suppresses_faults() {
        if let Some((at, location, status)) = moves
            .iter()
            .find_map(|(_, at)| find(*at).map(|(location, status)| (*at, location, status)))
        {
            return Err(abort(at, location, status));
        }
        let mut result = if crate::planted::is(3513) {
            original
        } else {
            0
        };
        for &(element, at) in &moves {
            let mask = (u128::MAX >> (u128::BITS - esize)) << (element * esize);
            result = (result & !mask) | (read(at) << (element * esize));
        }
        processor.vector_registers[target] = result;
        return Ok(());
    }
    // A first-fault or non-fault load, element by element; an access it may suppress reads what it would.
    let first = moves.first().map(|(element, _)| *element);
    let mut first_fault = processor.first_fault_register;
    let before = first_fault;
    let predicate_bits = esize / u8::BITS;
    let mut faulted = false;
    let mut unknown = false;
    let mut result = 0_u128;
    for element in 0..elements {
        let at = moves
            .iter()
            .find(|(moved, _)| *moved == element)
            .map(|(_, at)| *at);
        let performs_first = (access.faulting == Faulting::AfterFirst && first == Some(element))
            != crate::planted::is(3501)
            || (crate::planted::is(3508) && first == Some(element))
            || crate::planted::is(3502);
        let (data, fault) = match at {
            Some(at) => match find(at) {
                Some((location, status)) if performs_first => {
                    return Err(abort(at, location, status));
                }
                Some(_) => (0, true),
                None => (read(at), false),
            },
            None => (0, false),
        };
        faulted |= fault;
        let group = (u16::MAX >> (u16::BITS - predicate_bits)) << (element * predicate_bits);
        if faulted {
            first_fault &= !group;
        }
        let held = (before & first_fault) >> (element * predicate_bits) & 1;
        unknown |= held == 0 && !crate::planted::is(3510);
        let value = match choices.unknown_element {
            Some(UnknownElement::Merge) if unknown && at.is_none() => {
                read_element(original, element, esize)
            }
            _ if crate::planted::is(3510) && at.is_none() => read_element(original, element, esize),
            _ if unknown && fault => 0,
            _ => data,
        };
        result |= value << (element * esize);
    }
    if crate::planted::is(3514) && moves.is_empty() {
        first_fault = 0;
    }
    processor.vector_registers[target] = result;
    processor.first_fault_register = first_fault;
    Ok(())
}

/// The Z register of a first-fault or non-fault load whose value, with FFR's, the state does not decide: one that
/// completes with an access it may suppress, since [MemNF](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.MemNF.3)
/// may fail for any reason, and a Neoverse V2 suppresses accesses that would not fault by what its translations held
/// before, which no state holds; none for any other instruction.
pub(crate) fn to_undecided_register<LevelSpecific>(
    instruction: Instruction<Infallible>,
    processor: &Processor<LevelSpecific>,
    word: u32,
) -> Option<usize> {
    let access = to_element_access(instruction)?;
    if !access.suppresses_faults() {
        return None;
    }
    let predicate = read_predicate(processor, word, PG);
    let mut active = (0..access.elements())
        .filter(|element| is_active(predicate, *element, access.element_bits));
    if access.faulting == Faulting::AfterFirst {
        active.next();
    }
    active
        .next()
        .map(|_| ZD.read(word) as usize % crate::VECTOR_REGISTERS)
}

/// The bugs planted in these models, each caught by its instruction's states.
pub const ELEMENT_ACCESS_PLANTED_BUGS: [PlantedBug; 15] = [
    bug(
        3501,
        Instruction::Ldff1w,
        "a first-fault load suppresses the fault of its first active element",
    ),
    bug(
        3502,
        Instruction::Ldff1wGatherExtendedOffset,
        "a first-fault load faults on a later element",
    ),
    bug(
        3503,
        Instruction::Ld1wGatherExtendedOffset,
        "a gather zero-extends offsets the word sign-extends",
    ),
    bug(
        3504,
        Instruction::Ld1hGatherSCALEDExtendedOffset32,
        "a scaled gather leaves its offsets unscaled",
    ),
    bug(
        3505,
        Instruction::Ld1dGatherImmediate,
        "a vector-plus-immediate gather leaves its immediate unscaled",
    ),
    bug(
        3506,
        Instruction::St1wZPBzSX32Unscaled,
        "a scatter stores its elements from the last",
    ),
    bug(
        3507,
        Instruction::Ld1shGatherExtendedOffset32,
        "a signed gather zero-extends what it loads",
    ),
    bug(
        3508,
        Instruction::Ldnf1wImmediate,
        "a non-fault load faults on its first active element",
    ),
    bug(
        3509,
        Instruction::Ldnt1dZZR64,
        "a vector-plus-scalar load leaves out Xm",
    ),
    bug(
        3510,
        Instruction::Ldnf1bImmediate,
        "a non-fault load keeps its inactive elements",
    ),
    bug(
        3511,
        Instruction::Ld1dGatherExtendedOffset,
        "a gather of 32-bit offsets reads every bit of its 64-bit elements",
    ),
    bug(
        3512,
        Instruction::St1hScatter64,
        "a scatter stores whole elements instead of their low bytes",
    ),
    bug(
        3513,
        Instruction::Ld1bGatherImmediate64,
        "a gather keeps its inactive elements instead of zeroing them",
    ),
    bug(
        3514,
        Instruction::Ldnf1d,
        "a non-fault load with no active element clears FFR",
    ),
    bug(
        3515,
        Instruction::St1dScatterSCALED,
        "a scatter does not check the alignment of SP as its base",
    ),
];

const fn bug(number: u32, instruction: Instruction<Infallible>, what: &'static str) -> PlantedBug {
    PlantedBug {
        number,
        instruction,
        planted_by: PlantedBy::ScalableLoadsAndStores,
        what,
    }
}
