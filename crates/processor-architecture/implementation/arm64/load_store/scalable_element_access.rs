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
    SuppressibleLoad,
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
/// value of an element at or after one whose FFR bit is clear, as the pseudocode chooses it: what the access read
/// where it was performed (`Unpredictable_SVELDNFDATA`), else zero (`Unpredictable_SVELDNFZERO`), else what Zt held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::load_store::scalable_element_access)
)]
pub enum UnknownElement {
    Data,
    Zero,
    Merge,
}

/// The value a chip gives each kind of element at or after one whose FFR bit is clear; the pseudocode chooses for each
/// element anew, and these are the choices of a chip that chooses by the kind of element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnknownElements {
    /// An active element whose access was performed: its data, zero or what Zt held.
    pub read: UnknownElement,
    /// An active element whose access was suppressed, which has no data: zero or what Zt held; where it lies across
    /// a fault, zero may also be the bytes read below the fault with zero from it on, as `SuppressibleLoad` allows.
    pub suppressed: UnknownElement,
    /// An inactive element, whose data is zero: zero or what Zt held.
    pub inactive: UnknownElement,
}

/// The choices a chip makes for an access of elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::load_store::scalable_element_access)
)]
pub struct ElementAccessChoices {
    pub misaligned_stack_pointer: MisalignedStackPointer,
    pub across_fault: Option<ScalableStoreAcrossFault>,
    pub fault_address_tag: FaultAddressTag,
    pub unknown_elements: Option<UnknownElements>,
}

impl UnknownElements {
    /// Every choice the pseudocode allows: data, zero or merge where an access was performed, zero or merge elsewhere.
    pub fn list() -> impl Iterator<Item = Self> {
        const READ: [UnknownElement; 3] = [
            UnknownElement::Data,
            UnknownElement::Zero,
            UnknownElement::Merge,
        ];
        const OTHER: [UnknownElement; 2] = [UnknownElement::Zero, UnknownElement::Merge];
        READ.into_iter().flat_map(|read| {
            OTHER.into_iter().flat_map(move |suppressed| {
                OTHER.into_iter().map(move |inactive| Self {
                    read,
                    suppressed,
                    inactive,
                })
            })
        })
    }

    /// The value of an unknown element: `data` where it was active and its access performed.
    fn choose(self, data: u128, performed: bool, active: bool, original: u128) -> u128 {
        let choice = match (active, performed) {
            (false, _) => self.inactive,
            (true, false) if crate::planted::is(3632) => UnknownElement::Merge,
            (true, false) => self.suppressed,
            (true, true) if crate::planted::is(3633) && self.read == UnknownElement::Data => {
                UnknownElement::Zero
            }
            (true, true) => self.read,
        };
        match choice {
            UnknownElement::Data if performed => data,
            UnknownElement::Data | UnknownElement::Zero => 0,
            UnknownElement::Merge => original,
        }
    }
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
                let offset = if crate::planted::is(3605) && element > 0 {
                    0
                } else {
                    read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                };
                base.wrapping_add(offset.wrapping_add(index).wrapping_mul(memory_bytes))
            }
            Addressing::ScalarPlusImmediate => {
                let vectors = if crate::planted::is(3514) {
                    IMMEDIATE4.read_signed(word) as u64
                } else {
                    (IMMEDIATE4.read_signed(word) as u64).wrapping_mul(u64::from(self.elements()))
                };
                let index = if crate::planted::is(3601) {
                    index + 1
                } else if crate::planted::is(3604) && index == 1 {
                    0
                } else if crate::planted::is(3609) {
                    u64::from(self.elements()) - 1 - index
                } else {
                    index
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
                let address = base.wrapping_add(offset << shift);
                if crate::planted::is(3634) {
                    address & u64::from(u32::MAX)
                } else {
                    address
                }
            }
            Addressing::VectorPlusImmediate => {
                let unit = if crate::planted::is(3505) {
                    1
                } else {
                    memory_bytes
                };
                let address = self
                    .to_vector_base(processor, word, element)
                    .wrapping_add(u64::from(IMMEDIATE5.read(word)) * unit);
                let narrow = if self.element_bits == u32::BITS {
                    crate::planted::any(&[9910, 3621, 3622, 3623])
                } else {
                    crate::planted::is(3626)
                };
                if narrow {
                    address & u64::from(u32::MAX)
                } else {
                    address
                }
            }
            Addressing::VectorPlusScalar => {
                let offset = if crate::planted::is(3509) {
                    0
                } else if crate::planted::is(3612) {
                    read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                        .wrapping_mul(2)
                } else {
                    read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister)
                };
                let address = self
                    .to_vector_base(processor, word, element)
                    .wrapping_add(offset);
                if self.element_bits == u32::BITS && crate::planted::any(&[3624, 3625]) {
                    address & u64::from(u32::MAX)
                } else {
                    address
                }
            }
        }
    }

    /// The element a load reads at the address, extended to its element's bits.
    fn load_element<Architecture: ProcessorArchitecture>(
        self,
        memory: &[MemoryDelta<'_, Architecture>],
        at: u64,
    ) -> u128 {
        let mut buffer = [0_u8; size_of::<u64>()];
        let bytes = &mut buffer[..self.memory_bytes as usize];
        read_bytes_into(memory, to_untagged_address(at), bytes);
        if crate::planted::is(3602) {
            bytes.reverse();
        }
        let value = u64::from_le_bytes(buffer);
        let bits = self.memory_bytes * u8::BITS
            - u32::from(crate::planted::any(&[3618, 3627, 3628, 3629, 3630, 3631]));
        let signed = (self.signed || crate::planted::is(3603)) && !crate::planted::is(9008);
        let value = if signed && !crate::planted::is(3507) {
            (((value << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)) as u64
        } else {
            value
        };
        u128::from(value) & (u128::MAX >> (u128::BITS - self.element_bits))
    }

    /// The bytes of the element at the address below the fault's location, as read, with zero from it on.
    fn load_bytes_below<Architecture: ProcessorArchitecture>(
        self,
        memory: &[MemoryDelta<'_, Architecture>],
        at: u64,
        location: u64,
    ) -> u128 {
        let start = to_untagged_address(at);
        let below = location
            .wrapping_sub(start)
            .min(u64::from(self.memory_bytes)) as usize;
        let below = below.saturating_sub(usize::from(crate::planted::is(3635)));
        let mut buffer = [0_u8; size_of::<u64>()];
        read_bytes_into(memory, start, &mut buffer[..below]);
        u128::from(u64::from_le_bytes(buffer))
    }

    /// The element of Zn, zero-extended.
    fn to_vector_base<LevelSpecific>(
        self,
        processor: &Processor<LevelSpecific>,
        word: u32,
        element: u32,
    ) -> u64 {
        let bits = if crate::planted::is(3620) {
            u32::BITS
        } else {
            self.element_bits
        };
        let value = read_element(
            processor.vector_registers[ZN.read(word) as usize],
            element,
            bits,
        ) as u64;
        if crate::planted::any(&[9010, 3610, 3611]) && bits == u32::BITS {
            i64::from(value as u32 as i32) as u64
        } else if crate::planted::is(3613) {
            value & !(1 << (u32::BITS - 1))
        } else if crate::planted::is(3619) {
            value & !(0xFF << (u64::BITS - u8::BITS))
        } else {
            value
        }
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
                unknown_elements: None,
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
    let unknown: Vec<Option<UnknownElements>> = if access.suppresses_faults() {
        UnknownElements::list().map(Some).collect()
    } else {
        Vec::from([None])
    };
    for &misaligned_stack_pointer in stack_pointer {
        for &across_fault in across {
            for &fault_address_tag in tags {
                for &unknown_elements in &unknown {
                    behaviors.push(PreciseInstruction::ElementAccess {
                        instruction,
                        choices: ElementAccessChoices {
                            misaligned_stack_pointer,
                            across_fault,
                            fault_address_tag,
                            unknown_elements,
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
    let active = |element: u32| is_active(predicate, element, esize) || crate::planted::is(3615);
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
            let value = read_element(source, element, esize)
                >> if crate::planted::is(3617) {
                    esize / 2
                } else {
                    0
                };
            value.to_le_bytes()[..bytes as usize].to_vec()
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
    let read = |at: u64| access.load_element(memory, at);
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
        let later = first.is_some_and(|first| element > first);
        let (data, fault) = match at {
            Some(at) => match find(at) {
                Some((location, status)) if performs_first => {
                    return Err(abort(at, location, status));
                }
                Some(_) => (0, true),
                None if crate::planted::is(3606) && later => (
                    read(at) & (u128::MAX >> (u128::BITS - memory_bytes as u32 * u8::BITS)),
                    false,
                ),
                None if crate::planted::is(3607) && later => (0, false),
                None if crate::planted::is(3614) && later => {
                    (read(moves.first().map_or(at, |(_, first)| *first)), false)
                }
                None => (read(at), false),
            },
            None => (0, false),
        };
        faulted |= fault;
        let group = (u16::MAX >> (u16::BITS - predicate_bits)) << (element * predicate_bits);
        if faulted || (crate::planted::is(3608) && later) {
            first_fault &= !group;
        }
        let held = (before & first_fault) >> (element * predicate_bits) & 1;
        unknown |= held == 0 && !crate::planted::is(3510);
        let kept = read_element(original, element, esize);
        let value = match choices.unknown_elements {
            _ if crate::planted::is(3510) && at.is_none() => kept,
            Some(chosen) if unknown => chosen.choose(data, !fault, at.is_some(), kept),
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

/// The access of a first-fault or non-fault load that a chip may suppress though it would not fault, from its first
/// such active element on, with the value the choices give each element there whether its access is performed or
/// suppressed; none for any other instruction, or where no such element is active.
pub(crate) fn to_suppressible_load<LevelSpecific, Architecture>(
    instruction: Instruction<Infallible>,
    choices: ElementAccessChoices,
    processor: &Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
) -> Option<SuppressibleLoad>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let access = to_element_access(instruction)?;
    let chosen = choices.unknown_elements?;
    if !access.suppresses_faults() {
        return None;
    }
    let esize = access.element_bits;
    let predicate = read_predicate(processor, word, PG);
    let active = |element: u32| is_active(predicate, element, esize);
    let mut suppressible = (0..access.elements()).filter(|element| active(*element));
    if access.faulting == Faulting::AfterFirst {
        suppressible.next();
    }
    let first_suppressible = suppressible.next()?;
    let register = ZD.read(word) as usize % crate::VECTOR_REGISTERS;
    let original = processor.vector_registers[register];
    let (mut when_read, mut when_suppressed, mut below_fault) = (0_u128, 0_u128, 0_u128);
    for element in 0..access.elements() {
        let kept = read_element(original, element, esize);
        let is_element_active = active(element);
        let (data, below, performed) = if is_element_active {
            let at = access.to_address(processor, word, element);
            let start = to_untagged_address(at);
            let bytes = u64::from(access.memory_bytes);
            match find_fault(memory, start, bytes, AccessDirection::Read) {
                Some((location, _)) => (
                    0,
                    Some(access.load_bytes_below(memory, at, location)),
                    false,
                ),
                None => (access.load_element(memory, at), None, true),
            }
        } else {
            (0, None, false)
        };
        let suppressed = chosen.choose(0, false, is_element_active, kept);
        let below = match below {
            Some(below) if chosen.suppressed == UnknownElement::Zero => below,
            _ => suppressed,
        };
        when_read |= chosen.choose(data, performed, is_element_active, kept) << (element * esize);
        when_suppressed |= suppressed << (element * esize);
        below_fault |= below << (element * esize);
    }
    Some(SuppressibleLoad {
        register,
        element_bits: esize,
        first_suppressible,
        first_fault_before: processor.first_fault_register,
        when_read,
        when_suppressed,
        below_fault,
    })
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

/// The bugs planted for the per-lane expectation of first-fault and non-fault loads and for 32-bit vector bases with
/// bit 31 set, each caught by its instruction's states; 9008 and 9010 are the fourth check's, planted again.
pub const LANE_AND_BASE_PLANTED_BUGS: [PlantedBug; 22] = [
    lane_bug(
        9008,
        Instruction::Ldnf1shImmediate32,
        "a signed non-fault load zero-extends",
    ),
    lane_bug(
        9010,
        Instruction::Ld1wGatherImmediate,
        "a 32-bit vector base is sign-extended",
    ),
    lane_bug(
        3601,
        Instruction::Ldnf1bImmediate,
        "a non-fault load reads each element from the next one's address",
    ),
    lane_bug(
        3602,
        Instruction::Ldnf1hImmediate,
        "a non-fault halfword load swaps the bytes of each element",
    ),
    lane_bug(
        3603,
        Instruction::Ldnf1wImmediate64,
        "an unsigned non-fault load sign-extends",
    ),
    lane_bug(
        3604,
        Instruction::Ldnf1d,
        "a non-fault load reads its second element from the first's address",
    ),
    lane_bug(
        3605,
        Instruction::Ldff1b,
        "a first-fault load leaves Xm out of every element but the first",
    ),
    lane_bug(
        3606,
        Instruction::Ldff1sh32,
        "a signed first-fault load zero-extends every element but the first",
    ),
    lane_bug(
        3607,
        Instruction::Ldff1w,
        "a first-fault load zeroes every element after the first active one",
    ),
    lane_bug(
        3608,
        Instruction::Ldff1d,
        "a first-fault load clears FFR after its first active element though nothing faults",
    ),
    lane_bug(
        3609,
        Instruction::Ldnf1sbImmediate32,
        "a non-fault load reads its elements from the last",
    ),
    lane_bug(
        3610,
        Instruction::Ldff1wGatherImmediate,
        "a first-fault gather sign-extends its 32-bit vector base",
    ),
    lane_bug(
        3611,
        Instruction::St1wScatterImmediate,
        "a scatter sign-extends its 32-bit vector base",
    ),
    lane_bug(
        3612,
        Instruction::Ldnt1wZZR32,
        "a vector-plus-scalar load adds Xm twice",
    ),
    lane_bug(
        3613,
        Instruction::Ld1hGatherImmediate32,
        "a gather clears bit 31 of its 32-bit vector base",
    ),
    lane_bug(
        3614,
        Instruction::Ldff1sb16,
        "a first-fault load gives every later element the first one's data",
    ),
    lane_bug(
        3615,
        Instruction::Ldnf1hImmediate32,
        "a non-fault load takes every element as active",
    ),
    lane_bug(
        3616,
        Instruction::St1wZPBr,
        "ST1W stores doubleword elements as words",
    ),
    lane_bug(
        3617,
        Instruction::St1hScatterImmediate,
        "a halfword scatter stores the high half of each element",
    ),
    lane_bug(
        3618,
        Instruction::Ld1sbGatherImmediate32,
        "a signed byte gather extends bit 6",
    ),
    lane_bug(
        3619,
        Instruction::Ld1dGatherImmediate,
        "a 64-bit vector base loses its tag before the access",
    ),
    lane_bug(
        3620,
        Instruction::Ldnt1sbZZR64,
        "a vector-plus-scalar load reads 32-bit elements of its 64-bit base",
    ),
];

const fn lane_bug(
    number: u32,
    instruction: Instruction<Infallible>,
    what: &'static str,
) -> PlantedBug {
    PlantedBug {
        number,
        instruction,
        planted_by: PlantedBy::LanesAndBases,
        what,
    }
}

/// The bugs planted for narrow bases whose sum crosses 4 GiB, for the data a load extends, and for the choices of
/// first-fault and non-fault loads after a clear FFR bit, each caught by its instruction's states; 3618 is in
/// [`LANE_AND_BASE_PLANTED_BUGS`].
pub const GENERATOR_AND_FIRST_FAULT_PLANTED_BUGS: [PlantedBug; 16] = [
    generator_bug(
        9910,
        Instruction::Ld1wGatherImmediate,
        "a gather adds its immediate to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3621,
        Instruction::Ldff1hGatherImmediate32,
        "a first-fault gather adds its immediate to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3622,
        Instruction::St1wScatterImmediate,
        "a scatter adds its immediate to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3623,
        Instruction::Ld1sbGatherImmediate32,
        "a signed byte gather adds its immediate to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3624,
        Instruction::Ldnt1wZZR32,
        "a vector-plus-scalar load adds Xm to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3625,
        Instruction::Stnt1hZZR,
        "a vector-plus-scalar store adds Xm to a 32-bit vector base in 32 bits",
    ),
    generator_bug(
        3626,
        Instruction::Ld1dGatherImmediate,
        "a gather adds its immediate to a 64-bit vector base in 32 bits",
    ),
    generator_bug(
        3627,
        Instruction::Ldnf1sbImmediate16,
        "a signed non-fault byte load extends bit 6",
    ),
    generator_bug(
        3628,
        Instruction::Ldff1shGatherImmediate32,
        "a signed first-fault halfword gather extends bit 14",
    ),
    generator_bug(
        3629,
        Instruction::Ld1swGatherImmediate64,
        "a signed word gather extends bit 30",
    ),
    generator_bug(
        3630,
        Instruction::Ldff1sb64,
        "a signed first-fault byte load extends bit 6",
    ),
    generator_bug(
        3631,
        Instruction::Ldnt1sbZZR32,
        "a signed vector-plus-scalar byte load extends bit 6",
    ),
    generator_bug(
        3632,
        Instruction::Ldnf1wImmediate,
        "a non-fault load keeps what Zt held in a suppressed element whatever the choice",
    ),
    generator_bug(
        3633,
        Instruction::Ldff1bGather64,
        "a first-fault gather zeroes an element it read after a clear FFR bit under the data choice",
    ),
    generator_bug(
        3634,
        Instruction::Ld1wGatherExtendedOffset,
        "a gather adds its 32-bit offsets to its base in 32 bits",
    ),
    generator_bug(
        3635,
        Instruction::Ldnf1hImmediate,
        "a non-fault load leaves out the last byte it read below a fault",
    ),
];

const fn generator_bug(
    number: u32,
    instruction: Instruction<Infallible>,
    what: &'static str,
) -> PlantedBug {
    PlantedBug {
        number,
        instruction,
        planted_by: PlantedBy::GeneratorAndFirstFault,
        what,
    }
}
