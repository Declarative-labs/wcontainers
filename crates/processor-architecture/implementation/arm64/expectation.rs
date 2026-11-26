//! What a behavior expects the processor to hold after an instruction: the processor it leaves, compared bit by bit but
//! for the bits the state before does not decide.

use crate::scalable::VECTOR_BITS;
use crate::{ConditionFlags, GENERAL_PURPOSE_REGISTERS, Processor, VECTOR_REGISTERS};

/// What a behavior expects the processor to hold after an instruction: every value of `processor` but the bits
/// `uncompared` names, and after a load that may suppress an access, its Z register's lanes and FFR's bits only below
/// the element the observed FFR shows suppressed first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcessorExpectation<LevelSpecific> {
    /// The processor the behavior leaves, with every access it may suppress performed.
    pub processor: Processor<LevelSpecific>,
    pub uncompared: UncomparedBits,
    pub suppressible_load: Option<SuppressibleLoad>,
}

/// The bits of the general-purpose registers and NZCV that are not compared, as a read of [RNDR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/RNDR--Random-Number) leaves them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UncomparedBits {
    pub general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS],
    pub condition_flags: ConditionFlags,
}

/// A first-fault or non-fault load that completes, from whose element on the chip may suppress any active element's
/// access though it would not fault, since [MemNF](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.MemNF.3)
/// may fail for any reason; a Neoverse V2 does so by what its translations held before, which no state holds.
///
/// As [LDFF1W (scalar plus scalar)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LDFF1W--scalar-plus-scalar---Contiguous-load-first-fault-unsigned-words-to-vector--)
/// says, the first suppressed access clears FFR from its element on, and each later element's value is then the chip's
/// choice; every element before it is as if nothing were suppressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SuppressibleLoad {
    /// Zt.
    pub register: usize,
    pub element_bits: u32,
    /// The first active element the load may suppress: a non-fault load's first, a first-fault load's second.
    pub first_suppressible: u32,
    /// FFR before the load.
    pub first_fault_before: u16,
}

impl<LevelSpecific> ProcessorExpectation<LevelSpecific> {
    /// Expects every value the processor holds.
    pub fn new(processor: Processor<LevelSpecific>) -> Self {
        Self {
            processor,
            uncompared: UncomparedBits {
                general_purpose_registers: [0; GENERAL_PURPOSE_REGISTERS],
                condition_flags: ConditionFlags::empty(),
            },
            suppressible_load: None,
        }
    }
}

impl<LevelSpecific: PartialEq> ProcessorExpectation<LevelSpecific> {
    /// Whether the observed processor holds every compared bit; the exclusive monitor is never compared.
    pub fn is_met_by(&self, observed: &Processor<LevelSpecific>) -> bool {
        let expected = &self.processor;
        let (lanes, first_fault_bits, register) = match self.suppressible_load {
            Some(load) => {
                let (lanes, bits) = load.to_uncompared(observed.first_fault_register);
                (lanes, bits, load.register)
            }
            None => (0, 0, VECTOR_REGISTERS),
        };
        observed
            .general_purpose_registers
            .iter()
            .zip(&expected.general_purpose_registers)
            .zip(&self.uncompared.general_purpose_registers)
            .all(|((value, wanted), uncompared)| (value ^ wanted) & !uncompared == 0)
            && (observed.condition_flags.bits() ^ expected.condition_flags.bits())
                & !self.uncompared.condition_flags.bits()
                == 0
            && observed.stack_pointer == expected.stack_pointer
            && observed.program_counter == expected.program_counter
            && observed
                .vector_registers
                .iter()
                .zip(&expected.vector_registers)
                .enumerate()
                .all(|(number, (value, wanted))| {
                    let uncompared = if number == register { lanes } else { 0 };
                    (value ^ wanted) & !uncompared == 0
                })
            && observed.predicate_registers == expected.predicate_registers
            && (observed.first_fault_register ^ expected.first_fault_register) & !first_fault_bits
                == 0
            && observed.first_fault_register & first_fault_bits == 0
            && observed.floating_point_control == expected.floating_point_control
            && observed.floating_point_status == expected.floating_point_status
            && observed.software_thread_id == expected.software_thread_id
            && observed.exception == expected.exception
            && observed.level_specific == expected.level_specific
    }
}

impl SuppressibleLoad {
    /// The element from which the observed FFR allows the chip to have suppressed an access: the first element it
    /// clears that was set before, or every element if none, less the elements just below whose FFR bits were all
    /// clear before, which a suppression does not change, down to the first suppressible.
    pub fn find_first_suppressed(self, observed_first_fault: u16) -> u32 {
        let elements = VECTOR_BITS / self.element_bits;
        let group = |element: u32| -> u16 {
            let bits = self.element_bits / u8::BITS;
            (u16::MAX >> (u16::BITS - bits)) << (element * bits)
        };
        let before = self.first_fault_before;
        let mut first = (self.first_suppressible..elements)
            .find(|element| {
                before & group(*element) != 0 && observed_first_fault & group(*element) == 0
            })
            .unwrap_or(elements);
        while first > self.first_suppressible && before & group(first - 1) == 0 {
            first -= 1;
        }
        first
    }

    /// The lanes of Zt and the bits of FFR not compared against the expected values, from the element the observed FFR
    /// shows suppressed first on; there FFR must hold zero.
    pub fn to_uncompared(self, observed_first_fault: u16) -> (u128, u16) {
        let first = self.find_first_suppressed(observed_first_fault);
        let lanes = u128::MAX
            .checked_shl(first * self.element_bits)
            .unwrap_or(0);
        let bits = u16::MAX
            .checked_shl(first * self.element_bits / u8::BITS)
            .unwrap_or(0);
        (lanes, bits)
    }
}
