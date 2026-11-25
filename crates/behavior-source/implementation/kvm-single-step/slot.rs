//! One state in guest memory, as the handler reads and writes it; the build script reads its layout for the handler.

use std::mem::offset_of;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_processor_architecture_arm64::{
    GENERAL_PURPOSE_REGISTERS, PREDICATE_REGISTERS, VECTOR_REGISTERS,
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// Bytes of one general register.
const REGISTER_BYTES: usize = size_of::<u64>();

/// What a program at EL0 holds, in the order the handler loads and saves it.
#[repr(C)]
#[derive(Clone, Copy, IntoBytes, FromBytes, Immutable, KnownLayout)]
pub(crate) struct Registers {
    pub(crate) general_purpose: [u64; GENERAL_PURPOSE_REGISTERS],
    /// SP_EL0.
    pub(crate) stack_pointer: u64,
    /// ELR_EL1: where the instruction is, and after, where the exception that ended it returns to.
    pub(crate) program_counter: u64,
    /// SPSR_EL1: EL0's PSTATE.
    pub(crate) saved_program_status: u64,
    /// TPIDR_EL0.
    pub(crate) software_thread_id: u64,
    pub(crate) floating_point_control: u64,
    pub(crate) floating_point_status: u64,
    /// ESR_EL1 after.
    pub(crate) syndrome: u64,
    /// FAR_EL1 after.
    pub(crate) fault_address: u64,
    /// Unused: it puts `vector` where the alignment of its registers puts it.
    pub(crate) alignment: u64,
    pub(crate) vector: [u128; VECTOR_REGISTERS],
    /// P0 to P15 at a 128-bit vector length, which the handler moves only where the slot says so.
    pub(crate) predicate: [u16; PREDICATE_REGISTERS],
    /// FFR, moved with the predicates.
    pub(crate) first_fault: u16,
    /// Unused: it ends the registers at the alignment of their vector registers.
    pub(crate) predicate_alignment: [u16; 7],
}

/// One state in guest memory: its registers before and after, and where the handler goes next.
#[repr(C)]
#[derive(Clone, Copy, IntoBytes, FromBytes, Immutable, KnownLayout)]
pub(crate) struct Slot {
    /// TTBR0_EL1 while the instruction runs: the slot's top-level table and its ASID.
    pub(crate) translation_table: u64,
    /// The linear address of the next slot of the batch, zero after the last.
    pub(crate) next: u64,
    /// An [`Ending`], which the handler writes.
    pub(crate) ending: u64,
    /// Nonzero where the chip has SVE, so that the handler moves the predicates and FFR; it also puts `before` where the
    /// alignment of its vector registers puts it.
    pub(crate) scalable_vectors: u64,
    pub(crate) before: Registers,
    pub(crate) after: Registers,
}

/// What ended a slot's run.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    NotRun,
    /// An exception from EL0: the software step after the instruction, or the instruction's own.
    ExceptionFromEl0,
    /// Any other exception, which the handler does not expect: the batch stops there.
    Unexpected,
}

// The handler moves registers in pairs, and the general registers from the start of the block.
const _: () = {
    assert!(offset_of!(Registers, general_purpose) == 0);
    assert!(
        offset_of!(Registers, program_counter)
            == offset_of!(Registers, stack_pointer) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, software_thread_id)
            == offset_of!(Registers, saved_program_status) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, floating_point_status)
            == offset_of!(Registers, floating_point_control) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, fault_address) == offset_of!(Registers, syndrome) + REGISTER_BYTES
    );
    assert!(size_of::<Slot>() as u64 <= PAGE_SIZE);
};
