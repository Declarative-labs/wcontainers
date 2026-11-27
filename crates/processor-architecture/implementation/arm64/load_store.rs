//! The loads and stores of general-purpose registers and of SIMD and floating-point registers: one skeleton reads
//! the base, offset and registers the list gives each instruction, and one line per instruction says what it moves and
//! how many bytes of each register.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::Infallible;

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

pub mod scalable_access;
pub mod scalable_element_access;

#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{
    Assignment, Memory, apply, apply_all, memory_of, receives,
};
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{
    FailedStoreExclusive, FaultAddressTag, LoadIntoBaseWithWriteback, LoadPairIntoOneRegister,
    MisalignedExclusive, MisalignedStackPointer, StoreAcrossFault, StoreExclusiveOverlap,
    StoreOfBaseWithWriteback,
};
#[cfg(verus_keep_ghost)]
use crate::branch::{branch_address, is_outside_lower};
use crate::branch::{is_outside_lower_addresses, to_untagged_address};
use crate::encoding::{Field, concatenate};
#[cfg(verus_keep_ghost)]
use crate::encoding::{concatenation, fields_width};
#[cfg(verus_keep_ghost)]
use crate::integer::source_value;
use crate::integer::{low_ones, read_source};
use crate::memory::{read_byte_sequence, read_permission};
use crate::operand::{Indexing, Operand, RegisterThirtyOne, RegisterWidth, Signedness};
#[cfg(verus_keep_ghost)]
use crate::pseudocode::{
    accessed_address, immediate_offset, register_offset, sint, uint, written_back_address,
};
use crate::registers::{is_general_purpose, read_register, write_register};
#[cfg(verus_keep_ghost)]
use crate::registers::{register_file_written, register_read, stack_pointer_written};
use crate::{
    AccessDirection, DataAccess, Exception, ExclusiveMonitor, FaultStatus, Instruction,
    PagePermission, Processor,
};

vstd::prelude::verus! {

/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires, a check Linux turns on at every level.
const STACK_POINTER_ALIGNMENT: u64 = 16;
/// What a failed store exclusive writes to its status register.
const STORE_EXCLUSIVE_FAILED: u64 = 1;

/// The bytes [FEAT_LSE2](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-4-architecture-extension#:~:text=FEAT_LSE2) lets an unaligned ordered or atomic access lie anywhere inside.
const SINGLE_COPY_ATOMIC_BYTES: u64 = 16;

} // verus!

/// The bytes of each access size.
const BYTE: u32 = size_of::<u8>() as u32;
const HALFWORD: u32 = size_of::<u16>() as u32;
const WORD: u32 = size_of::<u32>() as u32;
const DOUBLEWORD: u32 = size_of::<u64>() as u32;
const QUADWORD: u32 = size_of::<u128>() as u32;

verus! {

/// What a load or store does with the bytes at its address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transfer {
    /// Loads each register from its bytes, extended as given to the register's width.
    Load(Signedness),
    Store,
    /// Changes nothing a program can see, and leaves out the stack pointer's alignment check.
    Prefetch,
    /// Writes its status register, which names the first register, and the others to memory if the exclusive monitor
    /// holds the address.
    StoreExclusive,
    /// Reads the bytes, writes what the operation makes of them and the first register's value, and gives the
    /// bytes it read to the second register, or to the first for a compare and swap.
    Atomic(AtomicOperation),
    /// Reads the bytes of two registers, stores the second pair where they equal the first pair, and gives the
    /// bytes it read to the first pair.
    CompareAndSwapPair,
}

/// What an atomic instruction stores from the value it reads and the value of its first register.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AtomicOperation {
    Add,
    Clear,
    ExclusiveOr,
    Set,
    SignedMaximum,
    SignedMinimum,
    UnsignedMaximum,
    UnsignedMinimum,
    Swap,
    /// Stores the second register's value where the bytes equal the first register's.
    CompareAndSwap,
}

/// Where an access must lie for its address not to raise an alignment fault, with SCTLR_EL1.A and nAA clear as
/// Linux leaves them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alignment {
    Unchecked,
    /// Inside one aligned 16 bytes, as FEAT_LSE2 has an ordered or atomic access.
    WithinSixteenBytes,
    /// Where the instruction's behavior for a misaligned exclusive access puts it.
    Exclusive,
}

/// What one load or store moves: each register it names gets or gives `bytes` bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoadStoreOperation {
    pub transfer: Transfer,
    pub bytes: u32,
    pub alignment: Alignment,
}

} // verus!

/// The operation of each modeled load or store of general-purpose registers, none for every other instruction.
pub const fn to_load_store_operation(
    instruction: Instruction<Infallible>,
) -> Option<LoadStoreOperation> {
    use AtomicOperation::{
        Add, Clear, CompareAndSwap, ExclusiveOr, Set, SignedMaximum, SignedMinimum, Swap,
        UnsignedMaximum, UnsignedMinimum,
    };
    use Instruction as I;
    use Signedness::{Signed, Unsigned};
    let (transfer, bytes, alignment) = match instruction {
        I::Casp32 | I::Caspa32 | I::Caspal32 | I::Caspl32 => (
            Transfer::CompareAndSwapPair,
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Casp64 | I::Caspa64 | I::Caspal64 | I::Caspl64 => (
            Transfer::CompareAndSwapPair,
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Cas32 | I::Casa32 | I::Casal32 | I::Casl32 => (
            Transfer::Atomic(CompareAndSwap),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Cas64 | I::Casa64 | I::Casal64 | I::Casl64 => (
            Transfer::Atomic(CompareAndSwap),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Casab | I::Casalb | I::Casb | I::Caslb => (
            Transfer::Atomic(CompareAndSwap),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Casah | I::Casalh | I::Cash | I::Caslh => (
            Transfer::Atomic(CompareAndSwap),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldadd32 | I::Ldadda32 | I::Ldaddal32 | I::Ldaddl32 => {
            (Transfer::Atomic(Add), WORD, Alignment::WithinSixteenBytes)
        }
        I::Ldadd64 | I::Ldadda64 | I::Ldaddal64 | I::Ldaddl64 => (
            Transfer::Atomic(Add),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldaddab | I::Ldaddalb | I::Ldaddb | I::Ldaddlb => {
            (Transfer::Atomic(Add), BYTE, Alignment::WithinSixteenBytes)
        }
        I::Ldaddah | I::Ldaddalh | I::Ldaddh | I::Ldaddlh => (
            Transfer::Atomic(Add),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldapr32 | I::Ldapur32 | I::Ldar32 | I::Ldlar32 => (
            Transfer::Load(Unsigned),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldapr64 | I::Ldapur64 | I::Ldar64 | I::Ldlar64 => (
            Transfer::Load(Unsigned),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldaprb | I::Ldapurb | I::Ldarb | I::Ldlarb => (
            Transfer::Load(Unsigned),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldaprh | I::Ldapurh | I::Ldarh | I::Ldlarh => (
            Transfer::Load(Unsigned),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldapursb64 | I::Ldapursb32 => {
            (Transfer::Load(Signed), BYTE, Alignment::WithinSixteenBytes)
        }
        I::Ldapursh64 | I::Ldapursh32 => (
            Transfer::Load(Signed),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldapursw => (Transfer::Load(Signed), WORD, Alignment::WithinSixteenBytes),
        I::Ldaxp32 | I::Ldaxr32 | I::Ldxp32 | I::Ldxr32 => {
            (Transfer::Load(Unsigned), WORD, Alignment::Exclusive)
        }
        I::Ldaxp64 | I::Ldaxr64 | I::Ldxp64 | I::Ldxr64 => {
            (Transfer::Load(Unsigned), DOUBLEWORD, Alignment::Exclusive)
        }
        I::Ldaxrb | I::Ldxrb => (Transfer::Load(Unsigned), BYTE, Alignment::Exclusive),
        I::Ldaxrh | I::Ldxrh => (Transfer::Load(Unsigned), HALFWORD, Alignment::Exclusive),
        I::Ldclr32 | I::Ldclra32 | I::Ldclral32 | I::Ldclrl32 => {
            (Transfer::Atomic(Clear), WORD, Alignment::WithinSixteenBytes)
        }
        I::Ldclr64 | I::Ldclra64 | I::Ldclral64 | I::Ldclrl64 => (
            Transfer::Atomic(Clear),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldclrab | I::Ldclralb | I::Ldclrb | I::Ldclrlb => {
            (Transfer::Atomic(Clear), BYTE, Alignment::WithinSixteenBytes)
        }
        I::Ldclrah | I::Ldclralh | I::Ldclrh | I::Ldclrlh => (
            Transfer::Atomic(Clear),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldeor32 | I::Ldeora32 | I::Ldeoral32 | I::Ldeorl32 => (
            Transfer::Atomic(ExclusiveOr),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldeor64 | I::Ldeora64 | I::Ldeoral64 | I::Ldeorl64 => (
            Transfer::Atomic(ExclusiveOr),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldeorab | I::Ldeoralb | I::Ldeorb | I::Ldeorlb => (
            Transfer::Atomic(ExclusiveOr),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldeorah | I::Ldeoralh | I::Ldeorh | I::Ldeorlh => (
            Transfer::Atomic(ExclusiveOr),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldset32 | I::Ldseta32 | I::Ldsetal32 | I::Ldsetl32 => {
            (Transfer::Atomic(Set), WORD, Alignment::WithinSixteenBytes)
        }
        I::Ldset64 | I::Ldseta64 | I::Ldsetal64 | I::Ldsetl64 => (
            Transfer::Atomic(Set),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsetab | I::Ldsetalb | I::Ldsetb | I::Ldsetlb => {
            (Transfer::Atomic(Set), BYTE, Alignment::WithinSixteenBytes)
        }
        I::Ldsetah | I::Ldsetalh | I::Ldseth | I::Ldsetlh => (
            Transfer::Atomic(Set),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmax32 | I::Ldsmaxa32 | I::Ldsmaxal32 | I::Ldsmaxl32 => (
            Transfer::Atomic(SignedMaximum),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmax64 | I::Ldsmaxa64 | I::Ldsmaxal64 | I::Ldsmaxl64 => (
            Transfer::Atomic(SignedMaximum),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmaxab | I::Ldsmaxalb | I::Ldsmaxb | I::Ldsmaxlb => (
            Transfer::Atomic(SignedMaximum),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmaxah | I::Ldsmaxalh | I::Ldsmaxh | I::Ldsmaxlh => (
            Transfer::Atomic(SignedMaximum),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmin32 | I::Ldsmina32 | I::Ldsminal32 | I::Ldsminl32 => (
            Transfer::Atomic(SignedMinimum),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsmin64 | I::Ldsmina64 | I::Ldsminal64 | I::Ldsminl64 => (
            Transfer::Atomic(SignedMinimum),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsminab | I::Ldsminalb | I::Ldsminb | I::Ldsminlb => (
            Transfer::Atomic(SignedMinimum),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldsminah | I::Ldsminalh | I::Ldsminh | I::Ldsminlh => (
            Transfer::Atomic(SignedMinimum),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumax32 | I::Ldumaxa32 | I::Ldumaxal32 | I::Ldumaxl32 => (
            Transfer::Atomic(UnsignedMaximum),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumax64 | I::Ldumaxa64 | I::Ldumaxal64 | I::Ldumaxl64 => (
            Transfer::Atomic(UnsignedMaximum),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumaxab | I::Ldumaxalb | I::Ldumaxb | I::Ldumaxlb => (
            Transfer::Atomic(UnsignedMaximum),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumaxah | I::Ldumaxalh | I::Ldumaxh | I::Ldumaxlh => (
            Transfer::Atomic(UnsignedMaximum),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumin32 | I::Ldumina32 | I::Lduminal32 | I::Lduminl32 => (
            Transfer::Atomic(UnsignedMinimum),
            WORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Ldumin64 | I::Ldumina64 | I::Lduminal64 | I::Lduminl64 => (
            Transfer::Atomic(UnsignedMinimum),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Lduminab | I::Lduminalb | I::Lduminb | I::Lduminlb => (
            Transfer::Atomic(UnsignedMinimum),
            BYTE,
            Alignment::WithinSixteenBytes,
        ),
        I::Lduminah | I::Lduminalh | I::Lduminh | I::Lduminlh => (
            Transfer::Atomic(UnsignedMinimum),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Stllr32 | I::Stlr32 | I::Stlur32 => {
            (Transfer::Store, WORD, Alignment::WithinSixteenBytes)
        }
        I::Stllr64 | I::Stlr64 | I::Stlur64 => {
            (Transfer::Store, DOUBLEWORD, Alignment::WithinSixteenBytes)
        }
        I::Stllrb | I::Stlrb | I::Stlurb => (Transfer::Store, BYTE, Alignment::WithinSixteenBytes),
        I::Stllrh | I::Stlrh | I::Stlurh => {
            (Transfer::Store, HALFWORD, Alignment::WithinSixteenBytes)
        }
        I::Stlxp32 | I::Stlxr32 | I::Stxp32 | I::Stxr32 => {
            (Transfer::StoreExclusive, WORD, Alignment::Exclusive)
        }
        I::Stlxp64 | I::Stlxr64 | I::Stxp64 | I::Stxr64 => {
            (Transfer::StoreExclusive, DOUBLEWORD, Alignment::Exclusive)
        }
        I::Stlxrb | I::Stxrb => (Transfer::StoreExclusive, BYTE, Alignment::Exclusive),
        I::Stlxrh | I::Stxrh => (Transfer::StoreExclusive, HALFWORD, Alignment::Exclusive),
        I::Swp32 | I::Swpa32 | I::Swpal32 | I::Swpl32 => {
            (Transfer::Atomic(Swap), WORD, Alignment::WithinSixteenBytes)
        }
        I::Swp64 | I::Swpa64 | I::Swpal64 | I::Swpl64 => (
            Transfer::Atomic(Swap),
            DOUBLEWORD,
            Alignment::WithinSixteenBytes,
        ),
        I::Swpab | I::Swpalb | I::Swpb | I::Swplb => {
            (Transfer::Atomic(Swap), BYTE, Alignment::WithinSixteenBytes)
        }
        I::Swpah | I::Swpalh | I::Swph | I::Swplh => (
            Transfer::Atomic(Swap),
            HALFWORD,
            Alignment::WithinSixteenBytes,
        ),
        _ => {
            let Some((transfer, bytes)) = to_plain_operation(instruction) else {
                return None;
            };
            (transfer, bytes, Alignment::Unchecked)
        }
    };
    Some(LoadStoreOperation {
        transfer,
        bytes,
        alignment,
    })
}

/// The transfer and access size of each load or store whose alignment is unchecked.
const fn to_plain_operation(instruction: Instruction<Infallible>) -> Option<(Transfer, u32)> {
    use Instruction as I;
    use Signedness::{Signed, Unsigned};
    Some(match instruction {
        I::LdrbImmediateUnsignedOffset32
        | I::LdrbImmediatePreIndex32
        | I::LdrbImmediatePostIndex32
        | I::LdrbExtendedRegister32
        | I::LdrbShiftedRegister32
        | I::Ldurb
        | I::Ldtrb => (Transfer::Load(Unsigned), BYTE),
        I::LdrhImmediateUnsignedOffset32
        | I::LdrhImmediatePreIndex32
        | I::LdrhImmediatePostIndex32
        | I::LdrhRegister32
        | I::Ldurh
        | I::Ldtrh => (Transfer::Load(Unsigned), HALFWORD),
        I::LdrImmediateUnsignedOffset32
        | I::LdrImmediatePreIndex32
        | I::LdrImmediatePostIndex32
        | I::LdrRegister32
        | I::LdrLiteral32
        | I::Ldur32
        | I::Ldtr32
        | I::LdpSignedOffset32
        | I::LdpPreIndex32
        | I::LdpPostIndex32
        | I::LdnpSignedOffset32 => (Transfer::Load(Unsigned), WORD),
        I::LdrImmediateUnsignedOffset64
        | I::LdrImmediatePreIndex64
        | I::LdrImmediatePostIndex64
        | I::LdrRegister64
        | I::LdrLiteral64
        | I::Ldur64
        | I::Ldtr64
        | I::LdpSignedOffset64
        | I::LdpPreIndex64
        | I::LdpPostIndex64
        | I::LdnpSignedOffset64 => (Transfer::Load(Unsigned), DOUBLEWORD),
        I::LdrsbImmediateUnsignedOffset32
        | I::LdrsbImmediateUnsignedOffset64
        | I::LdrsbImmediatePreIndex32
        | I::LdrsbImmediatePreIndex64
        | I::LdrsbImmediatePostIndex32
        | I::LdrsbImmediatePostIndex64
        | I::LdrsbExtendedRegister32
        | I::LdrsbExtendedRegister64
        | I::LdrsbShiftedRegister32
        | I::LdrsbShiftedRegister64
        | I::Ldursb32
        | I::Ldursb64
        | I::Ldtrsb32
        | I::Ldtrsb64 => (Transfer::Load(Signed), BYTE),
        I::LdrshImmediateUnsignedOffset32
        | I::LdrshImmediateUnsignedOffset64
        | I::LdrshImmediatePreIndex32
        | I::LdrshImmediatePreIndex64
        | I::LdrshImmediatePostIndex32
        | I::LdrshImmediatePostIndex64
        | I::LdrshRegister32
        | I::LdrshRegister64
        | I::Ldursh32
        | I::Ldursh64
        | I::Ldtrsh32
        | I::Ldtrsh64 => (Transfer::Load(Signed), HALFWORD),
        I::LdrswImmediateUnsignedOffset64
        | I::LdrswImmediatePreIndex64
        | I::LdrswImmediatePostIndex64
        | I::LdrswRegister64
        | I::LdrswLiteral64
        | I::Ldursw
        | I::Ldtrsw
        | I::LdpswSignedOffset64
        | I::LdpswPreIndex64
        | I::LdpswPostIndex64 => (Transfer::Load(Signed), WORD),
        I::StrbImmediateUnsignedOffset32
        | I::StrbImmediatePreIndex32
        | I::StrbImmediatePostIndex32
        | I::StrbExtendedRegister32
        | I::StrbShiftedRegister32
        | I::Sturb
        | I::Sttrb => (Transfer::Store, BYTE),
        I::StrhImmediateUnsignedOffset32
        | I::StrhImmediatePreIndex32
        | I::StrhImmediatePostIndex32
        | I::StrhRegister32
        | I::Sturh
        | I::Sttrh => (Transfer::Store, HALFWORD),
        I::StrImmediateUnsignedOffset32
        | I::StrImmediatePreIndex32
        | I::StrImmediatePostIndex32
        | I::StrRegister32
        | I::Stur32
        | I::Sttr32
        | I::StpSignedOffset32
        | I::StpPreIndex32
        | I::StpPostIndex32
        | I::StnpSignedOffset32 => (Transfer::Store, WORD),
        I::StrImmediateUnsignedOffset64
        | I::StrImmediatePreIndex64
        | I::StrImmediatePostIndex64
        | I::StrRegister64
        | I::Stur64
        | I::Sttr64
        | I::StpSignedOffset64
        | I::StpPreIndex64
        | I::StpPostIndex64
        | I::StnpSignedOffset64 => (Transfer::Store, DOUBLEWORD),
        // [LDRAA](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDRAA--LDRAB--Load-register--with-pointer-authentication-)
        // with the data keys off, so that AuthDA and AuthDB leave the base as it is.
        I::LdraaIndexed64 | I::LdraaWriteback64 | I::LdrabIndexed64 | I::LdrabWriteback64 => {
            (Transfer::Load(Unsigned), DOUBLEWORD)
        }
        I::PrfmImmediateUnsignedOffset64 | I::PrfmRegister | I::PrfmLiteral | I::Prfum => {
            (Transfer::Prefetch, DOUBLEWORD)
        }
        // [LDR (immediate, SIMD&FP)](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/LDR--immediate--SIMD-FP---Load-SIMD-FP-register--immediate-offset--)
        // and the other loads and stores of SIMD and floating-point registers, which access no register's upper bits
        // and zero them on a load.
        I::LdrSimdFpImmediateUnsignedOffset8
        | I::LdrSimdFpImmediatePreIndex8
        | I::LdrSimdFpImmediatePostIndex8
        | I::LdurSimdFp8
        | I::LdrBroWExtendedRegister8
        | I::LdrBroXShiftedRegister8 => (Transfer::Load(Unsigned), BYTE),
        I::LdrSimdFpImmediateUnsignedOffset16
        | I::LdrSimdFpImmediatePreIndex16
        | I::LdrSimdFpImmediatePostIndex16
        | I::LdurSimdFp16
        | I::LdrHroW16 => (Transfer::Load(Unsigned), HALFWORD),
        I::LdrSimdFpImmediateUnsignedOffset32
        | I::LdrSimdFpImmediatePreIndex32
        | I::LdrSimdFpImmediatePostIndex32
        | I::LdurSimdFp32
        | I::LdrSroW32
        | I::LdrSimdFpLiteral32
        | I::LdpSimdFpSignedOffset32
        | I::LdpSimdFpPreIndex32
        | I::LdpSimdFpPostIndex32
        | I::LdnpSimdFpSignedOffset32 => (Transfer::Load(Unsigned), WORD),
        I::LdrSimdFpImmediateUnsignedOffset64
        | I::LdrSimdFpImmediatePreIndex64
        | I::LdrSimdFpImmediatePostIndex64
        | I::LdurSimdFp64
        | I::LdrDroW64
        | I::LdrSimdFpLiteral64
        | I::LdpSimdFpSignedOffset64
        | I::LdpSimdFpPreIndex64
        | I::LdpSimdFpPostIndex64
        | I::LdnpSimdFpSignedOffset64 => (Transfer::Load(Unsigned), DOUBLEWORD),
        I::LdrSimdFpImmediateUnsignedOffset128
        | I::LdrSimdFpImmediatePreIndex128
        | I::LdrSimdFpImmediatePostIndex128
        | I::LdurSimdFp128
        | I::LdrSimdFpLiteral128
        | I::LdpSimdFpSignedOffset128
        | I::LdpSimdFpPreIndex128
        | I::LdpSimdFpPostIndex128
        | I::LdnpSimdFpSignedOffset128
        | I::LdrQLdstRegoff => (Transfer::Load(Unsigned), QUADWORD),
        I::StrSimdFpImmediateUnsignedOffset8
        | I::StrSimdFpImmediatePreIndex8
        | I::StrSimdFpImmediatePostIndex8
        | I::SturSimdFp8
        | I::StrBroWExtendedRegister8
        | I::StrBroXShiftedRegister8 => (Transfer::Store, BYTE),
        I::StrSimdFpImmediateUnsignedOffset16
        | I::StrSimdFpImmediatePreIndex16
        | I::StrSimdFpImmediatePostIndex16
        | I::SturSimdFp16
        | I::StrHroW16 => (Transfer::Store, HALFWORD),
        I::StrSimdFpImmediateUnsignedOffset32
        | I::StrSimdFpImmediatePreIndex32
        | I::StrSimdFpImmediatePostIndex32
        | I::SturSimdFp32
        | I::StrSroW32
        | I::StpSimdFpSignedOffset32
        | I::StpSimdFpPreIndex32
        | I::StpSimdFpPostIndex32
        | I::StnpSimdFpSignedOffset32 => (Transfer::Store, WORD),
        I::StrSimdFpImmediateUnsignedOffset64
        | I::StrSimdFpImmediatePreIndex64
        | I::StrSimdFpImmediatePostIndex64
        | I::SturSimdFp64
        | I::StrDroW64
        | I::StpSimdFpSignedOffset64
        | I::StpSimdFpPreIndex64
        | I::StpSimdFpPostIndex64
        | I::StnpSimdFpSignedOffset64 => (Transfer::Store, DOUBLEWORD),
        I::StrSimdFpImmediateUnsignedOffset128
        | I::StrSimdFpImmediatePreIndex128
        | I::StrSimdFpImmediatePostIndex128
        | I::SturSimdFp128
        | I::StpSimdFpSignedOffset128
        | I::StpSimdFpPreIndex128
        | I::StpSimdFpPostIndex128
        | I::StnpSimdFpSignedOffset128
        | I::StrQLdstRegoff => (Transfer::Store, QUADWORD),
        _ => return None,
    })
}

/// A load or store whose behavior is modeled, with a choice for each case the manual leaves open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoadStoreInstruction(Instruction<Infallible>);

verus! {

/// What a load or store does in each case the manual leaves open; none where the instruction has no such case.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub struct LoadStoreChoices {
    pub misaligned_stack_pointer: Option<MisalignedStackPointer>,
    pub across_fault: Option<StoreAcrossFault>,
    pub load_into_base: Option<LoadIntoBaseWithWriteback>,
    pub store_of_base: Option<StoreOfBaseWithWriteback>,
    pub load_pair_into_one_register: Option<LoadPairIntoOneRegister>,
    pub failed_store_exclusive: Option<FailedStoreExclusive>,
    pub store_exclusive_overlap: Option<StoreExclusiveOverlap>,
    pub misaligned_exclusive: Option<MisalignedExclusive>,
    pub fault_address_tag: Option<FaultAddressTag>,
    /// The bytes of the reservation granule the exclusive monitor marks, which CTR_EL0.ERG gives.
    pub reservation_granule_bytes: Option<u64>,
}

} // verus!

impl LoadStoreChoices {
    /// These choices for the cases the instruction has, none for the others.
    pub fn restrict_to(self, instruction: LoadStoreInstruction) -> Self {
        let open = instruction.to_open_cases();
        Self {
            misaligned_stack_pointer: self
                .misaligned_stack_pointer
                .filter(|_| open.misaligned_stack_pointer),
            across_fault: self.across_fault.filter(|_| open.across_fault),
            load_into_base: self.load_into_base.filter(|_| open.load_into_base),
            store_of_base: self.store_of_base.filter(|_| open.store_of_base),
            load_pair_into_one_register: self
                .load_pair_into_one_register
                .filter(|_| open.load_pair_into_one_register),
            failed_store_exclusive: self
                .failed_store_exclusive
                .filter(|_| open.failed_store_exclusive),
            store_exclusive_overlap: self
                .store_exclusive_overlap
                .filter(|_| open.store_exclusive_overlap),
            misaligned_exclusive: self
                .misaligned_exclusive
                .filter(|_| open.misaligned_exclusive),
            fault_address_tag: self.fault_address_tag.filter(|_| open.fault_address_tag),
            reservation_granule_bytes: self
                .reservation_granule_bytes
                .filter(|_| open.reservation_granule),
        }
    }
}

/// Which cases the manual leaves open a load or store has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpenCases {
    pub misaligned_stack_pointer: bool,
    pub across_fault: bool,
    pub load_into_base: bool,
    pub store_of_base: bool,
    pub load_pair_into_one_register: bool,
    pub failed_store_exclusive: bool,
    pub store_exclusive_overlap: bool,
    pub misaligned_exclusive: bool,
    pub fault_address_tag: bool,
    pub reservation_granule: bool,
}

verus! {

/// The most registers one load or store names: a compare and swap pair's four.
pub const MAXIMUM_REGISTERS: usize = 4;

/// The registers, base and offset an encoding of a load or store names.
struct Addressed {
    registers: [(u32, RegisterWidth); MAXIMUM_REGISTERS],
    count: usize,
    /// Whether the registers are SIMD and floating-point registers.
    vector: bool,
    /// The base register's number and indexing, none for a literal.
    base: Option<(u32, Indexing)>,
    /// What the instruction adds to its base, or for a literal to its own address.
    offset: u64,
}

} // verus!

impl LoadStoreInstruction {
    /// None for an instruction that is not a modeled load or store.
    pub const fn new(instruction: Instruction<Infallible>) -> Option<Self> {
        match (
            to_load_store_operation(instruction),
            instruction.to_record(),
        ) {
            (Some(_), Some(_)) => Some(Self(instruction)),
            _ => None,
        }
    }

    pub const fn instruction(self) -> Instruction<Infallible> {
        self.0
    }

    pub fn to_operation(self) -> LoadStoreOperation {
        to_load_store_operation(self.0).expect("`new` makes only loads and stores")
    }

    fn to_operands(self) -> &'static [Operand] {
        self.0
            .to_record()
            .expect("`new` makes only listed instructions")
            .operands
    }

    pub fn to_open_cases(self) -> OpenCases {
        let operation = self.to_operation();
        let operands = self.to_operands();
        let base = operands.iter().find_map(|operand| match *operand {
            Operand::AddressBase { indexing, .. } => Some(indexing),
            _ => None,
        });
        let registers = operands
            .iter()
            .map(|operand| match operand {
                Operand::GeneralPurposeRegister { .. } | Operand::VectorRegister { .. } => 1,
                Operand::GeneralPurposeRegisterPair { .. } => 2,
                _ => 0,
            })
            .sum::<u32>();
        let writeback = matches!(base, Some(Indexing::PreIndex | Indexing::PostIndex));
        // A SIMD and floating-point register is never the base, a general-purpose one.
        let general_purpose = operands.iter().any(|operand| {
            matches!(
                operand,
                Operand::GeneralPurposeRegister { .. } | Operand::GeneralPurposeRegisterPair { .. }
            )
        });
        let loads = matches!(operation.transfer, Transfer::Load(_));
        let stores = operation.transfer == Transfer::Store;
        let stores_exclusive = operation.transfer == Transfer::StoreExclusive;
        OpenCases {
            // Prefetches leave out CheckSPAlignment.
            misaligned_stack_pointer: base.is_some() && operation.transfer != Transfer::Prefetch,
            // An access inside 16 aligned bytes never crosses a page.
            across_fault: stores
                && operation.bytes * registers > 1
                && operation.alignment == Alignment::Unchecked,
            load_into_base: loads && writeback && general_purpose,
            store_of_base: stores && writeback && general_purpose,
            load_pair_into_one_register: loads && registers == 2,
            failed_store_exclusive: stores_exclusive,
            store_exclusive_overlap: stores_exclusive,
            // An exclusive pair of doublewords is misaligned exactly where it leaves its 16 aligned bytes.
            // A byte is aligned anywhere.
            misaligned_exclusive: operation.alignment == Alignment::Exclusive
                && (BYTE as u64 + 1..SINGLE_COPY_ATOMIC_BYTES).contains(&u64::from(
                    operation.bytes * (registers - u32::from(stores_exclusive)),
                )),
            // A prefetch never faults; a literal's address is the instruction's own, which is never tagged.
            fault_address_tag: base.is_some() && operation.transfer != Transfer::Prefetch,
            reservation_granule: stores_exclusive,
        }
    }

    /// The bytes the encoding accesses and what it adds to its base's value to find the first, given the registers.
    pub fn to_access_layout<LevelSpecific>(
        self,
        word: u32,
        processor: &Processor<LevelSpecific>,
    ) -> Option<AccessLayout> {
        let addressed =
            read_addressed(processor, word, self.to_operands(), self.to_operation()).ok()?;
        let (_, indexing) = addressed.base?;
        Some(AccessLayout {
            bytes: u64::from(self.to_operation().bytes)
                * addressed.to_data_count(self.to_operation()) as u64,
            offset: match indexing {
                Indexing::PostIndex => 0,
                Indexing::Offset | Indexing::PreIndex => addressed.offset,
            },
        })
    }

    /// Applies the instruction encoded in the word; changes no register when it returns an exception.
    pub(crate) fn simulate<LevelSpecific, Architecture>(
        self,
        choices: LoadStoreChoices,
        processor: &mut Processor<LevelSpecific>,
        memory: &[MemoryDelta<'_, Architecture>],
        word: u32,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Result<(), Exception>
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    {
        simulate_load_store(
            processor,
            memory,
            word,
            self.to_operands(),
            self.to_operation(),
            choices,
            mutation_hook,
        )
    }
}

/// The bytes a load or store accesses, and what it adds to its base's value to find the first of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AccessLayout {
    pub bytes: u64,
    pub offset: u64,
}

impl Addressed {
    /// The registers whose values the access moves: all but a store exclusive's status, and one for an atomic.
    fn to_data_count(&self, operation: LoadStoreOperation) -> usize {
        match operation.transfer {
            Transfer::StoreExclusive => self.count - 1,
            Transfer::Atomic(_) => 1,
            Transfer::CompareAndSwapPair => self.count / 2,
            Transfer::Load(_) | Transfer::Store | Transfer::Prefetch => self.count,
        }
    }
}

verus! {

/// What an immediate offset of a load or store adds to its base: the field zero- or sign-extended to 64 bits, shifted
/// left by the scale.
fn to_immediate_offset(field: Field, word: u32, signedness: Signedness, scale: u32) -> (offset: u64)
    requires
        field.fits(),
        0 < field.width,
        scale <= 4,
    ensures
        offset as int == crate::pseudocode::immediate_offset(
            word as int / (pow2(field.lowest_bit as nat) as int),
            field.width as nat,
            signedness,
            scale as nat,
        ),
{
    // VERUS: `planted::any` has no specification, so it is two `is`.
    let value = match signedness {
        Signedness::Signed if crate::planted::is(438) || crate::planted::is(54) => {
            u64::from(field.read(word))
        }
        Signedness::Unsigned => u64::from(field.read(word)),
        Signedness::Signed => {
            let signed_value = field.read_signed(word);
            proof {
                crate::lemmas::lemma_signed_pattern(signed_value);
            }
            signed_value as u64
        },
    };
    let scale = if crate::planted::is(448) {
        scale - 1
    } else {
        scale
    };
    // VERUS: no specification of `From<bool>`, so the bit is cast.
    let scale = match scale {
        1 if crate::planted::is(53) => 2,
        3 if crate::planted::is(56) => 2,
        0 if crate::planted::is(57) => 3,
        other => other,
    } + crate::planted::is(152) as u32;
    let scale = if crate::planted::is(279) || crate::planted::is(280) {
        3
    } else if crate::planted::is(281) || crate::planted::is(291) || crate::planted::is(294) {
        scale + 1
    } else {
        scale
    };
    proof {
        let field_value = word as int / (pow2(field.lowest_bit as nat) as int);
        let extended = match signedness {
            Signedness::Unsigned => uint(field_value, field.width as nat),
            Signedness::Signed => sint(field_value, field.width as nat),
        };
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        crate::lemmas::lemma_shift_left(value, scale, 64);
        assert(value << scale == (value << scale) & 0xffff_ffff_ffff_ffffu64) by (bit_vector);
        // The value is the extended field modulo 2^64, and so is its product with 2^scale.
        if signedness is Unsigned {
            vstd::arithmetic::div_mod::lemma_small_mod(value as nat, pow2(64));
        }
        assert(value as int == extended % (pow2(64) as int));
        vstd::arithmetic::div_mod::lemma_mul_mod_noop_left(extended, pow2(scale as nat) as int, pow2(64) as int);
    }
    value << scale
}

/// What a register offset of a load or store adds to its base: the register's low word or the whole register, zero- or
/// sign-extended, shifted left by the access size's scale where S is set.
fn to_register_offset(value: u64, signed: bool, doubleword: bool, scaled: bool, bytes: u32) -> (offset: u64)
    requires
        bytes == 1 || bytes == 2 || bytes == 4 || bytes == 8 || bytes == 16,
    ensures
        offset as int == crate::pseudocode::register_offset(value, signed, doubleword, scaled, bytes as nat),
{
    // VERUS: `QUADWORD` and `DOUBLEWORD` are outside `verus!` (`size_of` has no specification here), so their sizes
    // are written out; `From<bool>` and `planted::any` have no specification, so bits are cast and bugs are `is`.
    let bits = if doubleword || crate::planted::is(286) || (crate::planted::is(718) && bytes == 16) {
        u64::BITS
    } else {
        u32::BITS
    };
    let shift_bits = u64::BITS - bits;
    let extended = if (signed && !(crate::planted::is(445) || crate::planted::is(455) || crate::planted::is(37)))
        || (crate::planted::is(285) && !doubleword)
    {
        (((value << shift_bits) as i64) >> shift_bits) as u64
    } else {
        (value << shift_bits) >> shift_bits
    };
    proof {
        vstd::std_specs::bits::axiom_u32_trailing_zeros(bytes);
        let zeros = vstd::std_specs::bits::u32_trailing_zeros(bytes);
        assert(zeros == if bytes == 1 { 0u32 } else if bytes == 2 { 1u32 } else if bytes == 4 { 2u32 } else if bytes == 8 { 3u32 } else { 4u32 }) by (bit_vector)
            requires
                bytes == 1u32 || bytes == 2u32 || bytes == 4u32 || bytes == 8u32 || bytes == 16u32,
                zeros < 32u32,
                (bytes >> zeros) & 1u32 == 1u32;
    }
    let amount = if scaled {
        bytes.trailing_zeros() + (crate::planted::is(444) || crate::planted::is(454)) as u32
            - (crate::planted::is(38) && bytes == 8) as u32 - crate::planted::is(153) as u32
    } else {
        0
    };
    proof {
        let (n, a) = (64nat, amount as nat);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        assert(a <= 4 && pow2(a) <= 16);
        assert(a == if scaled { crate::pseudocode::access_scale(bytes as nat) } else { 0 }) by {
            assert(crate::pseudocode::highest_set_bit(0) == -1);
            assert(crate::pseudocode::highest_set_bit(1) == 0);
            assert(crate::pseudocode::highest_set_bit(2) == 1);
            assert(crate::pseudocode::highest_set_bit(4) == 2);
            assert(crate::pseudocode::highest_set_bit(8) == 3);
            assert(crate::pseudocode::highest_set_bit(16) == 4);
        }
        crate::lemmas::lemma_shift_left(extended, amount, n);
        assert(extended << amount == (extended << amount) & 0xffff_ffff_ffff_ffffu64) by (bit_vector);
        crate::lemmas::lemma_mul_pow2_mod(value as int, a, n);
        if bits == 64 {
            // The whole register: the extension changes nothing, and the shift drops its top `amount` bits.
            assert(((value << 0u32) as i64 >> 0u32) as u64 == value && (value << 0u32) >> 0u32 == value) by (bit_vector);
            vstd::arithmetic::div_mod::lemma_small_mod(value as nat, pow2(64));
            let field = uint(value as int, (64 - a) as nat) * pow2(a);
            vstd::arithmetic::div_mod::lemma_small_mod(field as nat, pow2(64));
            vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(-1, field, pow2(64) as int);
            assert(uint(sint(field, 64), 64) == field);
        } else {
            // The low word: zero-extended, or sign-extended to SInt of it, then scaled within 64 bits.
            crate::lemmas::lemma_mask(value, 32);
            vstd::arithmetic::div_mod::lemma_mod_bound(value as int, pow2(32) as int);
            if signed {
                let signed_value = ((value << shift_bits) as i64) >> shift_bits;
                crate::lemmas::lemma_sign_extension(value, shift_bits);
                crate::lemmas::lemma_signed_pattern(signed_value);
                vstd::arithmetic::div_mod::lemma_mul_mod_noop_left(signed_value as int, pow2(a) as int, pow2(64) as int);
                crate::lemmas::lemma_sint_shift(value as int, 32, a);
            } else {
                assert((value << 32u32) >> 32u32 == value & 0xffff_ffffu64) by (bit_vector);
                assert(extended * pow2(a) < pow2(64)) by (nonlinear_arith)
                    requires
                        extended < 0x1_0000_0000,
                        pow2(a) <= 16,
                        pow2(64) == 0x1_0000_0000_0000_0000;
                vstd::arithmetic::div_mod::lemma_small_mod((extended * pow2(a)) as nat, pow2(64));
            }
        }
    }
    extended << amount
}

/// The address a load or store accesses: its base plus its offset, or for post-indexing its base alone.
fn to_accessed_address(base: u64, offset: u64, indexing: Indexing) -> (address: u64)
    ensures
        address as int == crate::pseudocode::accessed_address(base, offset, indexing),
{
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        let sum = base + offset;
        if sum >= pow2(64) {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, pow2(64) as int, 1, sum - pow2(64));
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(sum as nat, pow2(64));
        }
    }
    match indexing {
        Indexing::PostIndex if crate::planted::is(278) => base.wrapping_add(offset),
        Indexing::PostIndex => base,
        Indexing::PreIndex if crate::planted::is(150) => base,
        Indexing::Offset | Indexing::PreIndex => base.wrapping_add(offset),
    }
}

/// What a load or store with writeback writes to its base: the base plus the offset, which is the accessed address
/// unless it is post-indexed.
fn to_written_back_address(base: u64, offset: u64, address: u64, indexing: Indexing) -> (written_back: u64)
    requires
        address as int == crate::pseudocode::accessed_address(base, offset, indexing),
    ensures
        written_back as int == crate::pseudocode::written_back_address(base, offset),
{
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        let sum = base + offset;
        if sum >= pow2(64) {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, pow2(64) as int, 1, sum - pow2(64));
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(sum as nat, pow2(64));
        }
    }
    match indexing {
        Indexing::PostIndex => base.wrapping_add(offset),
        Indexing::Offset | Indexing::PreIndex => address,
    }
}

} // verus!

verus! {

/// What a load or store reads from its operands in the word, as its decode does: the registers it moves, with their
/// widths, whether they are SIMD and floating-point registers, its base register and indexing, and what it adds to its
/// base, the last offset operand's; none where an operand is UNDEFINED, gives no register or address, or names more
/// registers than a load or store moves.
pub open spec fn operands_addressed<L>(processor: Processor<L>, word: u32, operands: Seq<Operand>, bytes: u32)
    -> Option<(Seq<(u32, RegisterWidth)>, bool, Option<(u32, Indexing)>, int)>
    decreases operands.len(),
{
    if operands.len() == 0 {
        Some((Seq::empty(), false, None, 0))
    } else {
        match operands_addressed(processor, word, operands.drop_last(), bytes) {
            None => None,
            Some((registers, vector, base, offset)) => {
                let operand = operands.last();
                if !operand.fits() {
                    None
                } else {
                    match operand {
                        Operand::GeneralPurposeRegister { number, width, .. } => if registers.len() < MAXIMUM_REGISTERS {
                            Some((registers.push((number.value(word) as u32, width)), vector, base, offset))
                        } else {
                            None
                        },
                        Operand::VectorRegister { number, .. } => if registers.len() < MAXIMUM_REGISTERS {
                            Some((registers.push((number.value(word) as u32, RegisterWidth::Bits64)), true, base, offset))
                        } else {
                            None
                        },
                        Operand::GeneralPurposeRegisterPair { number, width } =>
                            if registers.len() + 2 <= MAXIMUM_REGISTERS && number.value(word) + 1 < 0x1_0000_0000 {
                                Some((
                                    registers.push((number.value(word) as u32, width)).push(((number.value(word) + 1) as u32, width)),
                                    vector,
                                    base,
                                    offset,
                                ))
                            } else {
                                None
                            },
                        Operand::AddressBase { number, indexing } =>
                            Some((registers, vector, Some((number.value(word) as u32, indexing)), offset)),
                        Operand::AddressOffset { field, signedness, scale } => Some((
                            registers,
                            vector,
                            base,
                            immediate_offset(word as int / (pow2(field.lowest_bit as nat) as int), field.width as nat, signedness, scale as nat),
                        )),
                        Operand::SplitAddressOffset { fields, scale } => Some((
                            registers,
                            vector,
                            base,
                            uint(sint(concatenation(word, fields@) as int, fields_width(fields@)) * pow2(scale as nat), 64),
                        )),
                        Operand::RegisterOffset { number, signed, doubleword, shift } =>
                            if bytes == 1 || bytes == 2 || bytes == 4 || bytes == 8 || bytes == 16 {
                                Some((
                                    registers,
                                    vector,
                                    base,
                                    register_offset(
                                        register_read(processor.general_purpose_registers@, processor.stack_pointer, number.value(word), RegisterThirtyOne::ZeroRegister),
                                        match signed {
                                            Some(field) => field.value(word) != 0,
                                            None => false,
                                        },
                                        // Where option<0> is absent the option is LSL, which takes the register whole.
                                        match doubleword {
                                            Some(field) => field.value(word) != 0,
                                            None => true,
                                        },
                                        shift.value(word) != 0,
                                        bytes as nat,
                                    ),
                                ))
                            } else {
                                None
                            },
                        Operand::ProgramCounterOffset { .. } => match source_value(processor, word, operand) {
                            Some(value) => Some((registers, vector, base, value)),
                            None => None,
                        },
                        Operand::UnsignedImmediate(_) => Some((registers, vector, base, offset)),
                        _ => None,
                    }
                }
            },
        }
    }
}

/// Where the operands up to `count` are UNDEFINED, so are all of them.
proof fn lemma_operands_addressed_prefix<L>(processor: Processor<L>, word: u32, operands: Seq<Operand>, bytes: u32, count: int)
    requires
        0 <= count <= operands.len(),
        operands_addressed(processor, word, operands.take(count), bytes) is None,
    ensures
        operands_addressed(processor, word, operands, bytes) is None,
    decreases operands.len() - count,
{
    if count < operands.len() {
        assert(operands.take(count + 1).drop_last() =~= operands.take(count));
        lemma_operands_addressed_prefix(processor, word, operands, bytes, count + 1);
    } else {
        assert(operands.take(count) =~= operands);
    }
}

/// Whether the field, where the encoding has it, is set, or else the value fixed for it.
fn is_set(field: Option<Field>, word: u32, fixed: bool) -> (set: bool)
    requires
        field matches Some(field) ==> field.fits(),
    ensures
        set == match field {
            Some(field) => field.value(word) != 0,
            None => fixed,
        },
{
    match field {
        Some(field) => field.read(word) != 0,
        None => fixed,
    }
}

/// Reads the registers, base and offset of the encoding, or returns the exception of an unallocated one.
fn read_addressed<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: LoadStoreOperation,
) -> (addressed: Result<Addressed, Exception>)
    ensures
        match operands_addressed(*processor, word, operands@, operation.bytes) {
            Some((registers, vector, base, offset)) => addressed matches Ok(addressed)
                && addressed.count == registers.len() && addressed.count <= MAXIMUM_REGISTERS
                && (forall|index: int| 0 <= index < registers.len() ==> addressed.registers[index] == #[trigger] registers[index])
                && addressed.vector == vector && addressed.base == base && addressed.offset as int == offset,
            None => addressed == Err::<Addressed, Exception>(Exception::UndefinedInstruction),
        },
{
    let mut addressed = Addressed {
        registers: [(0, RegisterWidth::Bits64), (0, RegisterWidth::Bits64), (0, RegisterWidth::Bits64), (0, RegisterWidth::Bits64)],
        count: 0,
        vector: false,
        base: None,
        offset: 0,
    };
    proof {
        vstd::arithmetic::power2::lemma2_to64();
    }
    // VERUS: no specification of iterating a slice, so the operands are read by index.
    let mut index: usize = 0;
    while index < operands.len()
        invariant
            index <= operands.len(),
            operands_addressed(*processor, word, operands@.take(index as int), operation.bytes) matches Some((registers, vector, base, offset))
                && addressed.count == registers.len() && addressed.count <= MAXIMUM_REGISTERS
                && (forall|at: int| 0 <= at < registers.len() ==> addressed.registers[at] == #[trigger] registers[at])
                && addressed.vector == vector && addressed.base == base && addressed.offset as int == offset,
        decreases operands.len() - index,
    {
        let operand = operands[index];
        proof {
            assert(operands@.take(index + 1).drop_last() =~= operands@.take(index as int));
            assert(operands@.take(index + 1).last() == operand);
        }
        // VERUS: the operand lists are a table Verus does not read, so an operand whose fields do not fit, that gives
        // no register or address, or one register more than a load or store moves, is refused as UNDEFINED, where it
        // would have read past its field or the registers, or panicked.
        if !operand.is_well_formed() {
            proof {
                lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
            }
            return Err(Exception::UndefinedInstruction);
        }
        match operand {
            Operand::GeneralPurposeRegister { number, width, .. } => {
                if addressed.count >= MAXIMUM_REGISTERS {
                    proof {
                        lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                    }
                    return Err(Exception::UndefinedInstruction);
                }
                let read = number.read(word);
                proof {
                    number.lemma_value_below(word, 32);
                }
                addressed.registers[addressed.count] = (read, width);
                addressed.count += 1;
            },
            Operand::VectorRegister { number, .. } => {
                if addressed.count >= MAXIMUM_REGISTERS {
                    proof {
                        lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                    }
                    return Err(Exception::UndefinedInstruction);
                }
                let read = number.read(word);
                proof {
                    number.lemma_value_below(word, 32);
                }
                addressed.registers[addressed.count] = (read, RegisterWidth::Bits64);
                addressed.count += 1;
                addressed.vector = true;
            },
            Operand::GeneralPurposeRegisterPair { number, width } => {
                let first = number.read(word);
                if addressed.count + 2 > MAXIMUM_REGISTERS || first == u32::MAX {
                    proof {
                        lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                    }
                    return Err(Exception::UndefinedInstruction);
                }
                proof {
                    number.lemma_value_below(word, 32);
                }
                addressed.registers[addressed.count] = (first, width);
                addressed.registers[addressed.count + 1] = (first + 1, width);
                addressed.count += 2;
            },
            Operand::AddressBase { number, indexing } => {
                let read = number.read(word);
                proof {
                    number.lemma_value_below(word, 32);
                }
                addressed.base = Some((read, indexing));
            },
            Operand::AddressOffset { field, signedness, scale } => {
                addressed.offset = to_immediate_offset(field, word, signedness, scale);
            },
            Operand::SplitAddressOffset { fields, scale } => {
                // VERUS: no specification of `Iterator::sum` or `fold`, so the fields are joined by a loop.
                let (bits, width) = concatenate(word, fields);
                let shift = u64::BITS - width;
                let scale = if crate::planted::is(711) {
                    scale - 1
                } else {
                    scale
                };
                let extended = (bits << shift) as i64 >> shift;
                proof {
                    crate::lemmas::lemma_sign_extension(bits, shift);
                    crate::lemmas::lemma_signed_shift_left(extended, scale);
                    assert(((extended as u64) << scale) == ((extended << scale) as u64)) by (bit_vector)
                        requires
                            scale < 64;
                }
                addressed.offset = (extended as u64) << scale;
            },
            Operand::RegisterOffset { number, signed, doubleword, shift } => {
                if !(operation.bytes == 1 || operation.bytes == 2 || operation.bytes == 4 || operation.bytes == 8
                    || operation.bytes == 16)
                {
                    proof {
                        lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                    }
                    return Err(Exception::UndefinedInstruction);
                }
                let value = read_register(
                    processor,
                    number.read(word),
                    if crate::planted::is(55) {
                        RegisterThirtyOne::StackPointer
                    } else {
                        RegisterThirtyOne::ZeroRegister
                    },
                );
                // Where option<0> is absent the option is LSL, which takes the register whole.
                addressed.offset = to_register_offset(
                    value,
                    is_set(signed, word, false),
                    is_set(doubleword, word, true),
                    shift.read(word) != 0,
                    operation.bytes,
                );
            },
            Operand::ProgramCounterOffset { .. } => {
                addressed.offset = match read_source(processor, word, &operand) {
                    Ok(offset) => offset,
                    Err(exception) => {
                        proof {
                            lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                        }
                        return Err(exception);
                    },
                };
            },
            Operand::UnsignedImmediate(_) => {},
            // VERUS: an operand that gives no register or address is refused as UNDEFINED, where it panicked.
            _ => {
                proof {
                    lemma_operands_addressed_prefix(*processor, word, operands@, operation.bytes, index + 1);
                }
                return Err(Exception::UndefinedInstruction);
            },
        }
        index += 1;
    }
    proof {
        assert(operands@.take(index as int) =~= operands@);
    }
    Ok(addressed)
}

} // verus!

verus! {

/// [AllInAlignedQuantity](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.AllInAlignedQuantity.3)
/// of `size` bytes at the address: `Align(address + size - 1, alignment) == Align(address, alignment)`, the first and
/// last byte in one aligned block.
pub open spec fn all_in_aligned_quantity(address: u64, size: nat, alignment: nat) -> bool {
    (address + size - 1) / (alignment as int) == address as int / (alignment as int)
}

/// Whether an access of `size` bytes at the address raises no alignment fault, as [AArch64.UnalignedAccessFaults](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#AArch64.UnalignedAccessFaults.3)
/// gives it with FEAT_LSE2 and SCTLR_EL1.A and nAA clear, as Linux leaves them: an exclusive, atomic or ordered access
/// faults unless it lies in 16 aligned bytes, any other access never; where the chip's behavior for a misaligned
/// exclusive is the fault of a chip before FEAT_LSE2, an exclusive access faults unless it is aligned to its size.
pub open spec fn aligned_access(alignment: Alignment, misaligned_exclusive: Option<MisalignedExclusive>, address: u64, size: nat) -> bool {
    match (alignment, misaligned_exclusive) {
        (Alignment::Unchecked, _) => true,
        (Alignment::Exclusive, Some(MisalignedExclusive::AlignmentFault)) => address as int % (size as int) == 0,
        _ => all_in_aligned_quantity(address, size, 16),
    }
}

/// Whether an access of `total` bytes at the address lies where it raises no alignment fault.
fn is_aligned(
    alignment: Alignment,
    misaligned_exclusive: Option<MisalignedExclusive>,
    address: u64,
    total: u64,
) -> (aligned: bool)
    requires
        1 <= total <= 32,
    ensures
        aligned == aligned_access(alignment, misaligned_exclusive, address, total as nat),
{
    let granule = if crate::planted::is(265) {
        SINGLE_COPY_ATOMIC_BYTES / 2
    } else {
        SINGLE_COPY_ATOMIC_BYTES
    };
    let within_sixteen_bytes = address % granule + total <= granule;
    proof {
        let (a, t) = (address as int, total as int);
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(a, 16);
        assert(((a + t - 1) / 16 == a / 16) == (a % 16 + t <= 16)) by (nonlinear_arith)
            requires
                a == 16 * (a / 16) + a % 16,
                0 <= a % 16 < 16,
                1 <= t <= 32;
    }
    // VERUS: `planted::any` has no specification, so it is two `is`.
    crate::planted::is(276) || match (alignment, misaligned_exclusive) {
        _ if crate::planted::is(436) => true,
        (Alignment::Unchecked, _) if crate::planted::is(443) || crate::planted::is(456) => within_sixteen_bytes,
        (Alignment::WithinSixteenBytes, _) if crate::planted::is(437) => address % 8 + total <= 8,
        (Alignment::Unchecked, _) => true,
        (Alignment::WithinSixteenBytes, _) if crate::planted::is(41) => address.is_multiple_of(total),
        (Alignment::Exclusive, Some(MisalignedExclusive::WithinSixteenBytes)) if crate::planted::is(42) => true,
        (Alignment::Exclusive, None) if crate::planted::is(42) => true,
        (Alignment::WithinSixteenBytes, _) if crate::planted::is(158) => address.is_multiple_of(total),
        (Alignment::Exclusive, _) if crate::planted::is(166) => address.is_multiple_of(total),
        (Alignment::WithinSixteenBytes, _) => within_sixteen_bytes,
        (Alignment::Exclusive, Some(MisalignedExclusive::WithinSixteenBytes)) | (Alignment::Exclusive, None) => {
            within_sixteen_bytes
        },
        (Alignment::Exclusive, Some(MisalignedExclusive::AlignmentFault)) => address.is_multiple_of(total),
    }
}

} // verus!

verus! {

/// What a load or store does in a case the manual leaves open.
enum Constrained {
    /// Runs as in every other case, with the writeback or second register's value left out where given.
    Runs {
        writeback: bool,
        pair_order_reversed: bool,
    },
    Undefined,
    NoOperation,
}

/// X0 to X30, SP, V0 to V31 and the exclusive monitor: all a load or store changes in a processor.
#[cfg(verus_keep_ghost)]
pub type RegisterView = (Seq<u64>, u64, Seq<u128>, ExclusiveMonitor);

pub open spec fn register_view<LevelSpecific>(processor: Processor<LevelSpecific>) -> RegisterView {
    (
        processor.general_purpose_registers@,
        processor.stack_pointer,
        processor.vector_registers@,
        processor.exclusive_monitor,
    )
}

/// The processor changed nowhere but in what a load or store changes.
pub open spec fn only_registers_changed<LevelSpecific>(before: Processor<LevelSpecific>, after: Processor<LevelSpecific>) -> bool {
    after == Processor {
        general_purpose_registers: after.general_purpose_registers,
        stack_pointer: after.stack_pointer,
        vector_registers: after.vector_registers,
        exclusive_monitor: after.exclusive_monitor,
        ..before
    }
}

/// `X[number]`, with 31 the zero register or SP as given.
pub open spec fn read_x(view: RegisterView, number: u32, thirty_one: RegisterThirtyOne) -> u64 {
    register_read(view.0, view.1, number as nat, thirty_one)
}

/// The registers after `X[number] = value`, with 31 nothing or SP as given.
pub open spec fn write_x(view: RegisterView, number: u32, value: u64, thirty_one: RegisterThirtyOne) -> RegisterView {
    (
        register_file_written(view.0, number as nat, value),
        stack_pointer_written(view.1, number as nat, value, thirty_one),
        view.2,
        view.3,
    )
}

/// The registers after `V[number] = value`.
pub open spec fn write_v(view: RegisterView, number: u32, value: u128) -> RegisterView {
    (view.0, view.1, view.2.update(number as int, value), view.3)
}

pub open spec fn with_monitor(view: RegisterView, monitor: ExclusiveMonitor) -> RegisterView {
    (view.0, view.1, view.2, monitor)
}

/// `value<8 * index + 7:8 * index>`.
pub open spec fn byte_of(value: int, index: nat) -> u8 {
    ((value / (pow2(8 * index) as int)) % 256) as u8
}

/// The bytes `Mem[address, length] = value` writes from the address up, as a little-endian access orders them.
pub open spec fn little_endian_bytes(value: int, length: nat) -> Seq<u8> {
    Seq::new(length, |index: int| byte_of(value, index as nat))
}

/// The value `Mem[address, length]` reads from its bytes, the first the lowest: the sum of each byte times 2^(8 index).
pub open spec fn little_endian(bytes: Seq<u8>) -> int
    decreases bytes.len(),
{
    if bytes.len() == 0 {
        0
    } else {
        bytes[0] + 256 * little_endian(bytes.drop_first())
    }
}

/// The `length` bytes the memory holds from the location.
pub open spec fn memory_bytes(memory: Memory<PagePermission>, location: u64, length: nat) -> Seq<u8> {
    Seq::new(length, |offset: int| memory(location.wrapping_add(offset as u64)).0)
}

/// Whether an access of `bytes` bytes is one an encoding gives: 1, 2, 4 or 8 bytes, or 16 where `quadword`.
pub open spec fn is_access_size(bytes: u32, quadword: bool) -> bool {
    bytes == 1 || bytes == 2 || bytes == 4 || bytes == 8 || (quadword && bytes == 16)
}

/// What every load and store's decode gives, which Verus cannot read from the operand tables: register numbers of five
/// bits; for a load or store one or two registers of 1 to 8 bytes, or 16 for a SIMD and floating-point register,
/// sign-extended from at most 8; none for a prefetch; a status register and one or two general-purpose registers for
/// a store exclusive; two for an atomic and four for a compare and swap pair.
#[verifier::opaque]
pub open spec fn well_shaped(registers: Seq<(u32, RegisterWidth)>, vector: bool, operation: LoadStoreOperation) -> bool {
    &&& forall|index: int| 0 <= index < registers.len() ==> #[trigger] registers[index].0 < 32
    &&& match operation.transfer {
        Transfer::Load(signedness) => registers.len() <= 2 && is_access_size(operation.bytes, vector) && (
        signedness == Signedness::Signed ==> operation.bytes <= 8),
        Transfer::Store => registers.len() <= 2 && is_access_size(operation.bytes, vector),
        Transfer::Prefetch => registers.len() == 0,
        Transfer::StoreExclusive => 2 <= registers.len() <= 3 && !vector && is_access_size(operation.bytes, false),
        Transfer::Atomic(_) => registers.len() == 2 && !vector && is_access_size(operation.bytes, false),
        Transfer::CompareAndSwapPair => registers.len() == 4 && !vector && (operation.bytes == 4 || operation.bytes == 8),
    }
}

/// How many registers' bytes the access moves: all but a store exclusive's status, an atomic's first register's, and
/// one pair of a compare and swap pair.
pub open spec fn data_count(transfer: Transfer, count: nat) -> nat {
    match transfer {
        Transfer::StoreExclusive => (count - 1) as nat,
        Transfer::Atomic(_) => 1,
        Transfer::CompareAndSwapPair => count / 2,
        _ => count,
    }
}

/// Whether one of the registers from `from` up to `to` is register `number`.
pub open spec fn names_register(registers: Seq<(u32, RegisterWidth)>, from: int, to: int, number: u32) -> bool {
    exists|index: int| from <= index < to && #[trigger] registers[index].0 == number
}

/// What the behavior's choices make of the cases the manual leaves CONSTRAINED UNPREDICTABLE: Unpredictable_WBOVERLAPLD
/// and Unpredictable_WBOVERLAPST, a writeback to a general-purpose base the access also names; a store exclusive's
/// status register that is also a data register or its base (Unpredictable_DATAOVERLAP, Unpredictable_BASEOVERLAP);
/// and Unpredictable_LDPOVERLAP, a load pair into one register.
#[verifier::opaque]
spec fn constrained(
    registers: Seq<(u32, RegisterWidth)>,
    vector: bool,
    base: Option<(u32, Indexing)>,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
) -> Constrained {
    let writeback = base matches Some((_, indexing)) && indexing != Indexing::Offset;
    let base_overlap = writeback && !vector && (base matches Some((number, _)) && number < 31 && names_register(
        registers,
        0,
        registers.len() as int,
        number,
    ));
    let is_load = operation.transfer matches Transfer::Load(_);
    let first = if !base_overlap {
        None
    } else if is_load {
        match choices.load_into_base {
            Some(LoadIntoBaseWithWriteback::Undefined) => Some(Constrained::Undefined),
            Some(LoadIntoBaseWithWriteback::NoOperation) => Some(Constrained::NoOperation),
            _ => None,
        }
    } else if operation.transfer == Transfer::Store {
        match choices.store_of_base {
            Some(StoreOfBaseWithWriteback::Undefined) => Some(Constrained::Undefined),
            Some(StoreOfBaseWithWriteback::NoOperation) => Some(Constrained::NoOperation),
            _ => None,
        }
    } else {
        None
    };
    let writeback = writeback && !(base_overlap && is_load && choices.load_into_base == Some(
        LoadIntoBaseWithWriteback::WritebackSuppressed,
    ));
    let status_overlap = operation.transfer == Transfer::StoreExclusive && registers.len() >= 1 && (names_register(
        registers,
        1,
        registers.len() as int,
        registers[0].0,
    ) || (base matches Some((number, _)) && number < 31 && number == registers[0].0));
    let pair_overlap = is_load && registers.len() == 2 && registers[0].0 == registers[1].0;
    match first {
        Some(constrained) => constrained,
        None => if status_overlap && choices.store_exclusive_overlap == Some(StoreExclusiveOverlap::Undefined) {
            Constrained::Undefined
        } else if status_overlap && choices.store_exclusive_overlap == Some(StoreExclusiveOverlap::NoOperation) {
            Constrained::NoOperation
        } else if pair_overlap && choices.load_pair_into_one_register == Some(LoadPairIntoOneRegister::Undefined) {
            Constrained::Undefined
        } else if pair_overlap && choices.load_pair_into_one_register == Some(LoadPairIntoOneRegister::NoOperation) {
            Constrained::NoOperation
        } else {
            Constrained::Runs {
                writeback,
                pair_order_reversed: pair_overlap && choices.load_pair_into_one_register == Some(
                    LoadPairIntoOneRegister::KeepsFirstValue,
                ),
            }
        },
    }
}

/// The value a store takes from a register: `V[number]`, or `X[number]` with 31 the zero register.
pub open spec fn stored_value(view: RegisterView, number: u32, vector: bool) -> int {
    if vector {
        view.2[number as int] as int
    } else {
        read_x(view, number, RegisterThirtyOne::ZeroRegister) as int
    }
}

/// The bytes a store writes from the registers, each register's low `bytes` bytes in turn.
pub open spec fn stored_bytes(view: RegisterView, registers: Seq<(u32, RegisterWidth)>, vector: bool, bytes: nat) -> Seq<u8>
    decreases registers.len(),
{
    if registers.len() == 0 {
        Seq::empty()
    } else {
        stored_bytes(view, registers.drop_last(), vector, bytes) + little_endian_bytes(
            stored_value(view, registers.last().0, vector),
            bytes,
        )
    }
}

/// The data abort of a fault at `fault_location` of an access to the address.
pub open spec fn data_abort(fault: (u64, FaultStatus), access: AccessDirection, address: u64, tag: FaultAddressTag) -> Exception {
    Exception::DataAbort { status: fault.1, access, address: fault_address(address, fault.0, tag) }
}

/// What a store does where a byte faults: the behavior's choice, or nothing where it has none.
pub open spec fn store_across_fault(choices: LoadStoreChoices) -> StoreAcrossFault {
    match choices.across_fault {
        Some(across_fault) => across_fault,
        None => StoreAcrossFault::WritesNothing,
    }
}

/// What a store writes, as [Mem\[\]](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.Mem.write.3)
/// writes the bytes from the lowest up: all of them, or the fault of the first its page refuses and, as the behavior
/// across a fault has it, nothing or the bytes below it.
#[verifier::opaque]
pub open spec fn store_outcome(
    memory: Memory<PagePermission>,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    data: Seq<u8>,
    across_fault: StoreAcrossFault,
) -> (Result<(), Exception>, Seq<Assignment<PagePermission>>) {
    match first_fault(memory, location, data.len(), AccessDirection::Write) {
        None => (Ok(()), seq![Assignment::ValueAssignment { start: location, bytes: data }]),
        Some(fault) => {
            let below_fault = data.take(fault.0.wrapping_sub(location) as int);
            (
                Err(data_abort(fault, AccessDirection::Write, address, tag)),
                if across_fault == StoreAcrossFault::WritesBytesBelowFault && below_fault.len() > 0 {
                    seq![Assignment::ValueAssignment { start: location, bytes: below_fault }]
                } else {
                    Seq::empty()
                },
            )
        },
    }
}

/// What a store exclusive does: where the monitor marks the location's reservation granule, the store, with its
/// status 0, and the monitor cleared; elsewhere the status 1, the monitor cleared, and no store, with the access's
/// fault where the behavior checks a failed store's access.
#[verifier::opaque]
pub open spec fn store_exclusive_outcome(
    view: RegisterView,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    bytes: nat,
    choices: LoadStoreChoices,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    total: nat,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    let (status, width) = registers[0];
    let holds = match (view.3, choices.reservation_granule_bytes) {
        (ExclusiveMonitor::Exclusive { address: marked }, Some(granule)) => granule != 0 && marked / granule == location
            / granule,
        _ => false,
    };
    if holds {
        let data = stored_bytes(view, registers.drop_first(), false, bytes);
        match first_fault(memory, location, data.len(), AccessDirection::Write) {
            None => (
                Ok(()),
                with_monitor(write_x(view, status, 0, RegisterThirtyOne::ZeroRegister), ExclusiveMonitor::Open),
                seq![Assignment::ValueAssignment { start: location, bytes: data }],
            ),
            Some(fault) => (Err(data_abort(fault, AccessDirection::Write, address, tag)), view, Seq::empty()),
        }
    } else {
        match first_fault(memory, location, total, AccessDirection::Write) {
            Some(fault) => if choices.failed_store_exclusive == Some(FailedStoreExclusive::ChecksAccess) {
                (Err(data_abort(fault, AccessDirection::Write, address, tag)), view, Seq::empty())
            } else {
                (
                    Ok(()),
                    write_x(
                        with_monitor(view, ExclusiveMonitor::Open),
                        status,
                        uint(1, width.bits()) as u64,
                        RegisterThirtyOne::ZeroRegister,
                    ),
                    Seq::empty(),
                )
            },
            None => (
                Ok(()),
                write_x(
                    with_monitor(view, ExclusiveMonitor::Open),
                    status,
                    uint(1, width.bits()) as u64,
                    RegisterThirtyOne::ZeroRegister,
                ),
                Seq::empty(),
            ),
        }
    }
}

/// What an atomic stores from the value it read and its operands, as [MemAtomic](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.MemAtomic.4)
/// computes it on `bits` bits: ADD, BIC, EOR, ORR, SMAX, SMIN, UMAX, UMIN and SWP, and for CAS the swapped value
/// where the value read equals the compared one, else nothing.
pub open spec fn atomic_value(atomic: AtomicOperation, old: u64, operand: u64, swapped: u64, bits: nat) -> Option<u64> {
    match atomic {
        AtomicOperation::Add => Some(uint(old + operand, bits) as u64),
        AtomicOperation::Clear => Some(old & !operand),
        AtomicOperation::ExclusiveOr => Some(old ^ operand),
        AtomicOperation::Set => Some(old | operand),
        AtomicOperation::SignedMaximum => Some(
            if sint(old as int, bits) >= sint(operand as int, bits) {
                old
            } else {
                operand
            },
        ),
        AtomicOperation::SignedMinimum => Some(
            if sint(old as int, bits) <= sint(operand as int, bits) {
                old
            } else {
                operand
            },
        ),
        AtomicOperation::UnsignedMaximum => Some(
            if old >= operand {
                old
            } else {
                operand
            },
        ),
        AtomicOperation::UnsignedMinimum => Some(
            if old <= operand {
                old
            } else {
                operand
            },
        ),
        AtomicOperation::Swap => Some(operand),
        AtomicOperation::CompareAndSwap => if old == operand {
            Some(swapped)
        } else {
            None
        },
    }
}

/// What an atomic does: the fault of a read and then of a write of its bytes; else it reads them, stores what the
/// operation makes of them and its first register's low bits, and gives what it read to its second register, or to
/// its first for a compare and swap.
#[verifier::opaque]
pub open spec fn atomic_outcome(
    view: RegisterView,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    atomic: AtomicOperation,
    bytes: nat,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    match first_fault(memory, location, bytes, AccessDirection::Read) {
        Some(fault) => (Err(data_abort(fault, AccessDirection::Read, address, tag)), view, Seq::empty()),
        None => match first_fault(memory, location, bytes, AccessDirection::Write) {
            Some(fault) => (Err(data_abort(fault, AccessDirection::Write, address, tag)), view, Seq::empty()),
            None => {
                let bits = 8 * bytes;
                let old = little_endian(memory_bytes(memory, location, bytes)) as u64;
                let (first, first_width) = registers[0];
                let (second, second_width) = registers[1];
                let operand = uint(read_x(view, first, RegisterThirtyOne::ZeroRegister) as int, bits) as u64;
                let swapped = uint(read_x(view, second, RegisterThirtyOne::ZeroRegister) as int, bits) as u64;
                let (target, width) = if atomic == AtomicOperation::CompareAndSwap {
                    (first, first_width)
                } else {
                    (second, second_width)
                };
                (
                    Ok(()),
                    write_x(view, target, uint(old as int, width.bits()) as u64, RegisterThirtyOne::ZeroRegister),
                    match atomic_value(atomic, old, operand, swapped, bits) {
                        Some(new) => seq![
                            Assignment::ValueAssignment { start: location, bytes: little_endian_bytes(new as int, bytes) },
                        ],
                        None => Seq::empty(),
                    },
                )
            },
        },
    }
}

/// The value of the register pair from `first` as a compare and swap pair compares or stores it: the first's low
/// `bits` bits below the second's.
pub open spec fn pair_value(view: RegisterView, first: u32, bits: nat) -> int {
    uint(read_x(view, first, RegisterThirtyOne::ZeroRegister) as int, bits) + uint(
        read_x(view, (first + 1) as u32, RegisterThirtyOne::ZeroRegister) as int,
        bits,
    ) * pow2(bits)
}

/// What a compare and swap pair does: the fault of a read and then of a write of its bytes; else it reads them,
/// stores the second pair where they equal the first pair, and gives what it read to the first pair.
#[verifier::opaque]
pub open spec fn pair_outcome(
    view: RegisterView,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    bytes: nat,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    let total = 2 * bytes;
    match first_fault(memory, location, total, AccessDirection::Read) {
        Some(fault) => (Err(data_abort(fault, AccessDirection::Read, address, tag)), view, Seq::empty()),
        None => match first_fault(memory, location, total, AccessDirection::Write) {
            Some(fault) => (Err(data_abort(fault, AccessDirection::Write, address, tag)), view, Seq::empty()),
            None => {
                let bits = 8 * bytes;
                let old = little_endian(memory_bytes(memory, location, total));
                let (compared, width) = registers[0];
                let swapped = registers[2].0;
                let after = write_x(
                    write_x(
                        view,
                        compared,
                        uint(uint(old, bits), width.bits()) as u64,
                        RegisterThirtyOne::ZeroRegister,
                    ),
                    (compared + 1) as u32,
                    uint(uint(old / (pow2(bits) as int), bits), width.bits()) as u64,
                    RegisterThirtyOne::ZeroRegister,
                );
                (
                    Ok(()),
                    after,
                    if old == pair_value(view, compared, bits) {
                        seq![
                            Assignment::ValueAssignment {
                                start: location,
                                bytes: little_endian_bytes(pair_value(view, swapped, bits), total),
                            },
                        ]
                    } else {
                        Seq::empty()
                    },
                )
            },
        },
    }
}

/// The value a load gives register `index` of its data: its bytes, sign-extended where the load is signed.
pub open spec fn loaded_value(data: Seq<u8>, index: nat, bytes: nat, signedness: Signedness) -> int {
    let value = little_endian(data.subrange((index * bytes) as int, (index * bytes + bytes) as int));
    match signedness {
        Signedness::Signed => sint(value, 8 * bytes),
        Signedness::Unsigned => value,
    }
}

/// The registers after a load writes a value of 128 bits to one: `V[number] = value`, or `X[number]` its low bits of
/// the register's width, with 31 the zero register.
pub open spec fn write_loaded(view: RegisterView, register: (u32, RegisterWidth), vector: bool, value: u128) -> RegisterView {
    if vector {
        write_v(view, register.0, value)
    } else {
        write_x(view, register.0, uint(value as int, register.1.bits()) as u64, RegisterThirtyOne::ZeroRegister)
    }
}

/// What a load does: the fault of the first byte its page refuses; else each register gets its bytes, extended,
/// the second first where the behavior for a load pair into one register keeps the first value, and a load exclusive
/// marks the location in the exclusive monitor.
#[verifier::opaque]
pub open spec fn load_outcome(
    view: RegisterView,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    vector: bool,
    operation: LoadStoreOperation,
    signedness: Signedness,
    pair_order_reversed: bool,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    total: nat,
) -> (Result<(), Exception>, RegisterView) {
    match first_fault(memory, location, total, AccessDirection::Read) {
        Some(fault) => (Err(data_abort(fault, AccessDirection::Read, address, tag)), view),
        None => {
            let data = memory_bytes(memory, location, total);
            let bytes = operation.bytes as nat;
            let first = uint(loaded_value(data, 0, bytes, signedness), 128) as u128;
            let second = uint(loaded_value(data, 1, bytes, signedness), 128) as u128;
            let after = if registers.len() == 2 && pair_order_reversed {
                write_loaded(write_loaded(view, registers[1], vector, second), registers[0], vector, first)
            } else if registers.len() == 2 {
                write_loaded(write_loaded(view, registers[0], vector, first), registers[1], vector, second)
            } else if registers.len() == 1 {
                write_loaded(view, registers[0], vector, first)
            } else {
                view
            };
            (
                Ok(()),
                if operation.alignment == Alignment::Exclusive {
                    with_monitor(after, ExclusiveMonitor::Exclusive { address: location })
                } else {
                    after
                },
            )
        },
    }
}

/// What the access does once its address passed its checks, before any writeback.
#[verifier::opaque]
pub open spec fn transfer_outcome(
    view: RegisterView,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    vector: bool,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    pair_order_reversed: bool,
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    total: nat,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    let bytes = operation.bytes as nat;
    match operation.transfer {
        Transfer::Prefetch => (Ok(()), view, Seq::empty()),
        Transfer::StoreExclusive => store_exclusive_outcome(
            view,
            memory,
            registers,
            bytes,
            choices,
            address,
            location,
            tag,
            total,
        ),
        Transfer::Atomic(atomic) => atomic_outcome(view, memory, registers, atomic, bytes, address, location, tag),
        Transfer::CompareAndSwapPair => pair_outcome(view, memory, registers, bytes, address, location, tag),
        Transfer::Store => {
            let (outcome, changes) = store_outcome(
                memory,
                address,
                location,
                tag,
                stored_bytes(view, registers, vector, bytes),
                store_across_fault(choices),
            );
            (outcome, view, changes)
        },
        Transfer::Load(signedness) => {
            let (outcome, after) = load_outcome(
                view,
                memory,
                registers,
                vector,
                operation,
                signedness,
                pair_order_reversed,
                address,
                location,
                tag,
                total,
            );
            (outcome, after, Seq::empty())
        },
    }
}

/// What a load or store does from its decoded operands, as its Arm pseudocode does with the behavior's choices:
/// the base, or for a literal the program counter, and the address; [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0)
/// where SP is the base and the behavior faults; the alignment fault; the access to the location, the address with
/// its top byte left out; and the writeback of a pre- or post-indexed base once the access completed.
#[verifier::opaque]
pub open spec fn accessed_outcome(
    view: RegisterView,
    program_counter: u64,
    memory: Memory<PagePermission>,
    registers: Seq<(u32, RegisterWidth)>,
    vector: bool,
    base: Option<(u32, Indexing)>,
    offset: u64,
    writeback: bool,
    pair_order_reversed: bool,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    let (base_value, address) = match base {
        None => (0u64, accessed_address(program_counter, offset, Indexing::Offset) as u64),
        Some((number, indexing)) => {
            let base_value = read_x(view, number, RegisterThirtyOne::StackPointer);
            (base_value, accessed_address(base_value, offset, indexing) as u64)
        },
    };
    let misaligned_stack_pointer = base matches Some((number, _)) && number >= 31 && base_value % 16 != 0;
    if misaligned_stack_pointer && choices.misaligned_stack_pointer == Some(MisalignedStackPointer::Faults) {
        (Err(Exception::StackPointerAlignmentFault), view, Seq::empty())
    } else {
        let total = (operation.bytes as nat * data_count(operation.transfer, registers.len())) as nat;
        let access = match operation.transfer {
            Transfer::Store | Transfer::StoreExclusive => AccessDirection::Write,
            _ => AccessDirection::Read,
        };
        let tag = match choices.fault_address_tag {
            Some(tag) => tag,
            None => FaultAddressTag::KeepsTopByte,
        };
        if total > 0 && !aligned_access(operation.alignment, choices.misaligned_exclusive, address, total) {
            (
                Err(Exception::DataAbort {
                    status: FaultStatus::AlignmentFault,
                    access,
                    address: fault_address(address, address, tag),
                }),
                view,
                Seq::empty(),
            )
        } else {
            let (outcome, after, changes) = transfer_outcome(
                view,
                memory,
                registers,
                vector,
                operation,
                choices,
                pair_order_reversed,
                address,
                branch_address(address),
                tag,
                total,
            );
            match base {
                Some((number, _)) => if writeback && outcome is Ok {
                    (
                        outcome,
                        write_x(
                            after,
                            number,
                            written_back_address(base_value, offset) as u64,
                            RegisterThirtyOne::StackPointer,
                        ),
                        changes,
                    )
                } else {
                    (outcome, after, changes)
                },
                None => (outcome, after, changes),
            }
        }
    }
}

/// What a load or store does: UNDEFINED where its operands are; then what the behavior's choices make of a case the
/// manual leaves open; then its access.
spec fn load_store_outcome<LevelSpecific>(
    processor: Processor<LevelSpecific>,
    memory: Memory<PagePermission>,
    word: u32,
    operands: Seq<Operand>,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
) -> (Result<(), Exception>, RegisterView, Seq<Assignment<PagePermission>>) {
    let view = register_view(processor);
    match operands_addressed(processor, word, operands, operation.bytes) {
        None => (Err(Exception::UndefinedInstruction), view, Seq::empty()),
        Some((registers, vector, base, offset)) => if !well_shaped(registers, vector, operation) {
            (Err(Exception::UndefinedInstruction), view, Seq::empty())
        } else {
            match constrained(registers, vector, base, operation, choices) {
                Constrained::Undefined => (Err(Exception::UndefinedInstruction), view, Seq::empty()),
                Constrained::NoOperation => (Ok(()), view, Seq::empty()),
                Constrained::Runs { writeback, pair_order_reversed } => accessed_outcome(
                    view,
                    processor.program_counter,
                    memory,
                    registers,
                    vector,
                    base,
                    offset as u64,
                    writeback,
                    pair_order_reversed,
                    operation,
                    choices,
                ),
            }
        },
    }
}

/// The registers the access names.
spec fn named(addressed: Addressed) -> Seq<(u32, RegisterWidth)> {
    addressed.registers@.take(addressed.count as int)
}

/// How a load or store runs once the behavior's choices constrained it, and where its access goes once its address
/// passed its checks.
#[derive(Clone, Copy)]
struct Place {
    /// Whether a pre- or post-indexed base is written back.
    writeback: bool,
    /// Whether a load pair into one register keeps the first value.
    pair_order_reversed: bool,
    /// The base register's value, 0 for a literal.
    base: u64,
    address: u64,
    /// The address with its top byte left out where TBI0 applies.
    location: u64,
    tag: FaultAddressTag,
    /// The bytes the access moves.
    total: usize,
}

/// Whether one of the registers from `from` up to `to` is register `number`.
fn names_among(addressed: &Addressed, from: usize, to: usize, number: u32) -> (found: bool)
    requires
        from <= to <= addressed.count <= MAXIMUM_REGISTERS,
    ensures
        found == names_register(named(*addressed), from as int, to as int, number),
{
    let mut index = from;
    while index < to
        invariant
            from <= index <= to <= addressed.count <= MAXIMUM_REGISTERS,
            !names_register(named(*addressed), from as int, index as int, number),
        decreases to - index,
    {
        proof {
            assert(named(*addressed)[index as int] == addressed.registers[index as int]);
        }
        if addressed.registers[index].0 == number {
            proof {
                assert(named(*addressed)[index as int].0 == number);
            }
            return true;
        }
        index += 1;
    }
    false
}

/// Whether the decoded operands are ones a load or store's decode gives.
fn is_well_shaped(addressed: &Addressed, operation: LoadStoreOperation) -> (shaped: bool)
    requires
        addressed.count <= MAXIMUM_REGISTERS,
    ensures
        shaped == well_shaped(named(*addressed), addressed.vector, operation),
{
    proof {
        reveal(well_shaped);
    }
    let count = addressed.count;
    let mut index = 0;
    while index < count
        invariant
            index <= count == addressed.count <= MAXIMUM_REGISTERS,
            forall|at: int| 0 <= at < index ==> #[trigger] named(*addressed)[at].0 < 32,
        decreases count - index,
    {
        proof {
            assert(named(*addressed)[index as int] == addressed.registers[index as int]);
        }
        if addressed.registers[index].0 >= 32 {
            proof {
                reveal(well_shaped);
                assert(named(*addressed)[index as int].0 >= 32);
            }
            return false;
        }
        index += 1;
    }
    let bytes = operation.bytes;
    let size = bytes == 1 || bytes == 2 || bytes == 4 || bytes == 8;
    let vector = addressed.vector;
    match operation.transfer {
        Transfer::Load(signedness) => count <= 2 && (size || (vector && bytes == 16)) && (matches!(
            signedness,
            Signedness::Unsigned,
        ) || bytes <= 8),
        Transfer::Store => count <= 2 && (size || (vector && bytes == 16)),
        Transfer::Prefetch => count == 0,
        Transfer::StoreExclusive => (count == 2 || count == 3) && !vector && size,
        Transfer::Atomic(_) => count == 2 && !vector && size,
        Transfer::CompareAndSwapPair => count == 4 && !vector && (bytes == 4 || bytes == 8),
    }
}

fn constrain(addressed: &Addressed, operation: LoadStoreOperation, choices: LoadStoreChoices) -> (outcome: Constrained)
    requires
        addressed.count <= MAXIMUM_REGISTERS,
    ensures
        outcome == constrained(named(*addressed), addressed.vector, addressed.base, operation, choices),
{
    proof {
        reveal(constrained);
    }
    let count = addressed.count;
    let mut writeback = matches!(addressed.base, Some((_, Indexing::PreIndex)) | Some((_, Indexing::PostIndex)));
    let mut pair_order_reversed = false;
    // A SIMD and floating-point register is never the base.
    let written_back_base = match addressed.base {
        Some((base, _)) if writeback && !addressed.vector && is_general_purpose(base) => Some(base),
        _ => None,
    };
    let mut base_overlap = false;
    if let Some(base) = written_back_base {
        let limit = if crate::planted::is(168) && count > 0 {
            1
        } else {
            count
        };
        base_overlap = names_among(addressed, 0, limit, base);
    }
    let is_load = matches!(operation.transfer, Transfer::Load(_));
    if base_overlap {
        if is_load {
            match choices.load_into_base {
                Some(LoadIntoBaseWithWriteback::Undefined) => return Constrained::Undefined,
                Some(LoadIntoBaseWithWriteback::NoOperation) => return Constrained::NoOperation,
                Some(LoadIntoBaseWithWriteback::WritebackSuppressed) => writeback = false,
                Some(LoadIntoBaseWithWriteback::WritebackOverwritesLoad) | None => {},
            }
        } else if matches!(operation.transfer, Transfer::Store) {
            match choices.store_of_base {
                Some(StoreOfBaseWithWriteback::Undefined) => return Constrained::Undefined,
                Some(StoreOfBaseWithWriteback::NoOperation) => return Constrained::NoOperation,
                Some(StoreOfBaseWithWriteback::StoresOriginalValue) | None => {},
            }
        }
    }
    if matches!(operation.transfer, Transfer::StoreExclusive) && count >= 1 {
        let status = addressed.registers[0].0;
        let base_is_status = match addressed.base {
            Some((base, _)) => is_general_purpose(base) && base == status,
            None => false,
        };
        if names_among(addressed, 1, count, status) || base_is_status {
            match choices.store_exclusive_overlap {
                Some(StoreExclusiveOverlap::Undefined) => return Constrained::Undefined,
                Some(StoreExclusiveOverlap::NoOperation) => return Constrained::NoOperation,
                Some(StoreExclusiveOverlap::Runs) | None => {},
            }
        }
    }
    if is_load && count == 2 && addressed.registers[0].0 == addressed.registers[1].0 {
        match choices.load_pair_into_one_register {
            Some(LoadPairIntoOneRegister::Undefined) => return Constrained::Undefined,
            Some(LoadPairIntoOneRegister::NoOperation) => return Constrained::NoOperation,
            Some(LoadPairIntoOneRegister::KeepsFirstValue) => pair_order_reversed = true,
            Some(LoadPairIntoOneRegister::KeepsSecondValue) | None => {},
        }
    }
    Constrained::Runs { writeback, pair_order_reversed }
}

/// Appends the low `length` bytes of the value, the lowest first.
fn push_little_endian(data: &mut Vec<u8>, value: u128, length: usize)
    requires
        length <= 16,
    ensures
        final(data)@ == old(data)@ + little_endian_bytes(value as int, length as nat),
{
    let ghost bytes = little_endian_bytes(value as int, length as nat);
    let mut index: usize = 0;
    while index < length
        invariant
            index <= length <= 16,
            bytes == little_endian_bytes(value as int, length as nat),
            data@ == old(data)@ + bytes.take(index as int),
        decreases length - index,
    {
        let shift = 8 * index as u128;
        let byte = ((value >> shift) & 0xff) as u8;
        proof {
            vstd::bits::lemma_u128_shr_is_div(value, shift);
            let shifted = value >> shift;
            assert(shifted & 0xff == shifted % 256) by (bit_vector);
            assert(byte == byte_of(value as int, index as nat));
            assert(bytes.take(index + 1) =~= bytes.take(index as int).push(byte));
        }
        data.push(byte);
        index += 1;
    }
    proof {
        assert(bytes.take(length as int) =~= bytes);
    }
}

/// Appends the low `length` bytes of the value, the highest first: a planted bug's order.
fn push_big_endian(data: &mut Vec<u8>, value: u128, length: usize)
    requires
        length <= 16,
{
    let mut index: usize = length;
    while index > 0
        invariant
            index <= length <= 16,
        decreases index,
    {
        index -= 1;
        data.push(((value >> (8 * index as u128)) & 0xff) as u8);
    }
}

/// The value of `length` bytes of the data from `from`, the first the lowest.
fn little_endian_value(data: &[u8], from: usize, length: usize) -> (value: u128)
    requires
        from + length <= data@.len(),
        length <= 16,
    ensures
        value as int == little_endian(data@.subrange(from as int, from + length)),
        (value as int) < pow2(8 * length as nat),
{
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::power2::lemma_pow2_adds(64, 64);
        assert(data@.subrange(from + length, from + length) =~= Seq::<u8>::empty());
    }
    let mut value: u128 = 0;
    let mut remaining = length;
    let _data_length = data.len();
    while remaining > 0
        invariant
            remaining <= length <= 16,
            _data_length == data@.len(),
            from + length <= data@.len(),
            value as int == little_endian(data@.subrange(from + remaining, from + length)),
            (value as int) < pow2(8 * (length - remaining) as nat),
        decreases remaining,
    {
        remaining -= 1;
        let byte = data[from + remaining];
        proof {
            vstd::arithmetic::power2::lemma2_to64();
            vstd::arithmetic::power2::lemma2_to64_rest();
            vstd::arithmetic::power2::lemma_pow2_adds(64, 64);
            let bytes = data@.subrange(from + remaining, from + length);
            assert(bytes.drop_first() =~= data@.subrange(from + remaining + 1, from + length));
            assert(bytes[0] == byte);
            let below = (8 * (length - remaining - 1)) as nat;
            vstd::arithmetic::power2::lemma_pow2_adds(below, 8);
            assert(below + 8 == 8 * (length - remaining) as nat);
            if below + 8 < 128 {
                vstd::arithmetic::power2::lemma_pow2_strictly_increases(below + 8, 128);
            }
            let (v, b, p) = (value as int, byte as int, pow2(below) as int);
            assert(v * 256 + b < p * 256) by (nonlinear_arith)
                requires
                    0 <= v < p,
                    0 <= b < 256;
        }
        value = value * 256 + byte as u128;
    }
    value
}

/// The value of a load's register: its bytes from `from`, as they are or sign-extended from `sign_bits` bits.
fn extended_value(data: &[u8], from: usize, bytes: usize, signedness: Signedness, sign_bits: u32) -> (value: u128)
    requires
        from + bytes <= data@.len(),
        bytes <= 16,
        signedness == Signedness::Signed ==> bytes <= 8 && sign_bits == 8 * bytes,
    ensures
        value as int == uint(
            match signedness {
                Signedness::Signed => sint(little_endian(data@.subrange(from as int, from + bytes)), 8 * bytes as nat),
                Signedness::Unsigned => little_endian(data@.subrange(from as int, from + bytes)),
            },
            128,
        ),
{
    let loaded = little_endian_value(data, from, bytes);
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::power2::lemma_pow2_adds(64, 64);
        if 8 * bytes < 128 {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases(8 * bytes as nat, 128);
        }
        vstd::arithmetic::div_mod::lemma_small_mod(loaded as nat, pow2(128));
    }
    match signedness {
        Signedness::Unsigned => loaded,
        // A planted bug may sign-extend a quadword, which leaves it as it is.
        Signedness::Signed if sign_bits == 0 || sign_bits > 64 => loaded,
        Signedness::Signed => {
            // VERUS: the sign extension is in 64 bits, where a lemma relates it to SInt, and the 128-bit pattern of
            // the result is written as a sum; a signed load moves at most eight bytes.
            let shift = u64::BITS - sign_bits;
            let narrow = loaded as u64;
            let extended = ((narrow << shift) as i64) >> shift;
            proof {
                lemma_low_doubleword(loaded);
                if 8 * bytes < 64 {
                    vstd::arithmetic::power2::lemma_pow2_strictly_increases(8 * bytes as nat, 64);
                }
                vstd::arithmetic::div_mod::lemma_small_mod(loaded as nat, pow2(64));
                crate::lemmas::lemma_sign_extension(narrow, shift);
                crate::lemmas::lemma_signed_pattern(extended);
                let x = extended as int;
                let s = pow2(sign_bits as nat) as int;
                vstd::arithmetic::power2::lemma_pow2_pos(sign_bits as nat);
                assert(-0x8000_0000_0000_0000 <= x < 0x8000_0000_0000_0000);
                if x >= 0 {
                    vstd::arithmetic::div_mod::lemma_small_mod(x as nat, pow2(64));
                    vstd::arithmetic::div_mod::lemma_small_mod(x as nat, pow2(128));
                } else {
                    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(x, pow2(64) as int, -1, x + pow2(64));
                    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(x, pow2(128) as int, -1, x + pow2(128));
                }
            }
            (extended as u64) as u128 + if extended < 0 {
                0xffff_ffff_ffff_ffff_0000_0000_0000_0000u128
            } else {
                0
            }
        },
    }
}

/// The bytes a store writes from the registers from `from`: each register's low `bytes` bytes in turn.
fn register_bytes<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    addressed: &Addressed,
    from: usize,
    bytes: usize,
) -> (data: Vec<u8>)
    requires
        from <= addressed.count <= MAXIMUM_REGISTERS,
        bytes <= 16,
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
    ensures
        data@ == stored_bytes(register_view(*processor), named(*addressed).skip(from as int), addressed.vector, bytes as nat),
{
    let ghost registers = named(*addressed);
    let mut data: Vec<u8> = Vec::new();
    let mut index = from;
    proof {
        assert(registers.subrange(from as int, from as int) =~= Seq::<(u32, RegisterWidth)>::empty());
    }
    while index < addressed.count
        invariant
            from <= index <= addressed.count <= MAXIMUM_REGISTERS,
            bytes <= 16,
            registers == named(*addressed),
            forall|at: int| 0 <= at < addressed.count ==> #[trigger] registers[at].0 < 32,
            data@ == stored_bytes(
                register_view(*processor),
                registers.subrange(from as int, index as int),
                addressed.vector,
                bytes as nat,
            ),
        decreases addressed.count - index,
    {
        let number = if crate::planted::is(283) {
            addressed.registers[0].0
        } else {
            addressed.registers[index].0
        };
        proof {
            assert(registers[index as int].0 == addressed.registers[index as int].0);
        }
        let value = if addressed.vector {
            processor.vector_registers[number as usize]
        } else {
            read_register(processor, number, RegisterThirtyOne::ZeroRegister) as u128
        };
        let value = if crate::planted::is(290) {
            value >> 16
        } else {
            value
        };
        if crate::planted::is(156) {
            // Each register eight bytes past the last, the bytes between them zero, in a buffer of two quadwords.
            let start = 8 * (index - from);
            let mut kept: Vec<u8> = Vec::new();
            let mut at = 0;
            while at < start && at < data.len()
                decreases start - at,
            {
                kept.push(data[at]);
                at += 1;
            }
            while kept.len() < start
                decreases start - kept.len(),
            {
                kept.push(0);
            }
            if start + bytes <= 32 {
                push_little_endian(&mut kept, value, bytes);
            }
            data = kept;
        } else if crate::planted::is(299) {
            push_big_endian(&mut data, value, bytes);
        } else {
            push_little_endian(&mut data, value, bytes);
        }
        proof {
            let done = registers.subrange(from as int, index + 1);
            assert(done.drop_last() =~= registers.subrange(from as int, index as int));
            assert(done.last() == registers[index as int]);
        }
        index += 1;
    }
    proof {
        assert(registers.subrange(from as int, addressed.count as int) =~= registers.skip(from as int));
    }
    data
}

/// The low 64 bits of a 128-bit value, as `as u64` gives them, are the value modulo 2^64.
proof fn lemma_low_doubleword(x: u128)
    ensures
        (x as u64) as int == x as int % (pow2(64) as int),
{
    assert((x as u64) as u128 == x % 0x1_0000_0000_0000_0000u128) by (bit_vector);
    vstd::arithmetic::power2::lemma2_to64_rest();
}

/// The bytes a store writes from the registers are `bytes` from each.
proof fn lemma_stored_bytes_length(view: RegisterView, registers: Seq<(u32, RegisterWidth)>, vector: bool, bytes: nat)
    ensures
        stored_bytes(view, registers, vector, bytes).len() == bytes * registers.len(),
    decreases registers.len(),
{
    if registers.len() > 0 {
        lemma_stored_bytes_length(view, registers.drop_last(), vector, bytes);
        let count = registers.len() as int;
        assert(bytes * count == bytes * (count - 1) + bytes) by (nonlinear_arith);
    }
}

/// The data with zeros after it up to `length` bytes: a planted bug's.
fn zero_padded(data: Vec<u8>, length: usize) -> Vec<u8> {
    let mut data = data;
    while data.len() < length
        decreases length - data.len(),
    {
        data.push(0);
    }
    data
}

/// The value of the register pair from `first` as a compare and swap pair compares or stores it.
fn read_pair<LevelSpecific>(processor: &Processor<LevelSpecific>, first: u32, mask: u64, element_bits: u32) -> (value: u128)
    requires
        first < 32,
        element_bits == 32 || element_bits == 64,
        mask == crate::pseudocode::ones(element_bits as nat),
    ensures
        value as int == pair_value(register_view(*processor), first, element_bits as nat),
{
    let low = read_register(processor, first, RegisterThirtyOne::ZeroRegister) & mask;
    let high = read_register(processor, first + 1, RegisterThirtyOne::ZeroRegister) & mask;
    // VERUS: `high * 2^element_bits + low` in place of a shift and an or, which no lemma relates to the sum in 128 bits.
    let scale: u128 = if element_bits == 32 {
        0x1_0000_0000
    } else {
        0x1_0000_0000_0000_0000
    };
    proof {
        crate::lemmas::lemma_mask(read_x(register_view(*processor), first, RegisterThirtyOne::ZeroRegister), element_bits as nat);
        crate::lemmas::lemma_mask(read_x(register_view(*processor), (first + 1) as u32, RegisterThirtyOne::ZeroRegister), element_bits as nat);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        assert(scale as int == pow2(element_bits as nat));
        let (h, l, s) = (high as int, low as int, scale as int);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(read_x(register_view(*processor), first, RegisterThirtyOne::ZeroRegister) as int, s);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(read_x(register_view(*processor), (first + 1) as u32, RegisterThirtyOne::ZeroRegister) as int, s);
        assert(h * s + l <= 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffffint) by (nonlinear_arith)
            requires
                0 <= h < s,
                0 <= l < s,
                s <= 0x1_0000_0000_0000_0000int;
    }
    low as u128 + high as u128 * scale
}

/// Writes a value a load read to one register: the whole of it to a SIMD and floating-point register, else its low
/// bits of the register's width.
fn write_loaded_register<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    register: (u32, RegisterWidth),
    vector: bool,
    value: u128,
    bytes: u32,
)
    requires
        register.0 < 32,
        bytes <= 16,
    ensures
        register_view(*final(processor)) == write_loaded(register_view(*old(processor)), register, vector, value),
        only_registers_changed(*old(processor), *final(processor)),
{
    let (number, width) = register;
    if vector {
        processor.vector_registers[number as usize] = if crate::planted::is(451) {
            let mask = if bytes >= 16 {
                u128::MAX
            } else {
                (1u128 << (8 * bytes)) - 1
            };
            (processor.vector_registers[number as usize] & !mask) | value
        } else {
            value
        };
        return;
    }
    let mut written = value as u64 & width.to_mask();
    proof {
        lemma_low_doubleword(value);
        crate::lemmas::lemma_mask(value as u64, width.bits());
        vstd::arithmetic::power2::lemma2_to64_rest();
        crate::lemmas::lemma_wrapped_mod(value as int, width.bits());
    }
    if crate::planted::is(277) || crate::planted::is(297) {
        written = value as u64;
    }
    if crate::planted::is(293) && matches!(width, RegisterWidth::Bits32) {
        written |= read_register(processor, number, RegisterThirtyOne::ZeroRegister) & !width.to_mask();
    }
    write_register(processor, number, written, RegisterThirtyOne::ZeroRegister);
}

/// The planted bugs' writeback on a fault.
fn write_back<LevelSpecific>(processor: &mut Processor<LevelSpecific>, addressed: &Addressed, base: u64, address: u64)
    requires
        addressed.base matches Some((_, indexing)) ==> address as int == crate::pseudocode::accessed_address(
            base,
            addressed.offset,
            indexing,
        ),
{
    if let Some((number, indexing)) = addressed.base {
        let written_back = to_written_back_address(base, addressed.offset, address, indexing);
        write_register(processor, number, written_back, RegisterThirtyOne::StackPointer);
    }
}

/// A store exclusive's access, once its address passed its checks.
fn store_exclusive<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    bytes: usize,
    choices: LoadStoreChoices,
    place: Place,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        2 <= addressed.count <= 3,
        !addressed.vector,
        bytes <= 8,
        place.total == bytes * (addressed.count - 1),
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
    ensures
        ({
            let (outcome, after, changes) = store_exclusive_outcome(
                register_view(*old(processor)),
                memory_of(memory@),
                named(*addressed),
                bytes as nat,
                choices,
                place.address,
                place.location,
                place.tag,
                place.total as nat,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(store_exclusive_outcome);
    }
    let (status, width) = addressed.registers[0];
    proof {
        assert(named(*addressed)[0] == addressed.registers[0]);
        crate::lemmas::lemma_mask(STORE_EXCLUSIVE_FAILED, width.bits());
        crate::lemmas::lemma_mask(0, width.bits());
    }
    // VERUS: the two planted bugs' writes are one `if`.
    if crate::planted::is(58) || crate::planted::is(267) {
        write_register(processor, status, STORE_EXCLUSIVE_FAILED & width.to_mask(), RegisterThirtyOne::ZeroRegister);
    }
    // VERUS: no panic in verified code, so a store exclusive with no reservation granule, which no behavior gives, or
    // one of no bytes, fails as where the monitor does not mark the place.location.
    let holds = match (processor.exclusive_monitor, choices.reservation_granule_bytes) {
        (ExclusiveMonitor::Exclusive { address: marked }, Some(granule)) => granule != 0 && marked / granule == place.location
            / granule,
        _ => false,
    };
    if holds {
        let data = register_bytes(processor, addressed, 1, bytes);
        proof {
            assert(named(*addressed).skip(1) =~= named(*addressed).drop_first());
        }
        match store(
            memory,
            place.address,
            place.location,
            place.tag,
            data.as_slice(),
            StoreAcrossFault::WritesNothing,
            mutation_hook,
        ) {
            Ok(()) => {},
            Err(exception) => return Err(exception),
        }
        write_register(processor, status, 0, RegisterThirtyOne::ZeroRegister);
        processor.exclusive_monitor = ExclusiveMonitor::Open;
        return Ok(());
    }
    let fault = if matches!(choices.failed_store_exclusive, Some(FailedStoreExclusive::ChecksAccess)) {
        find_fault(memory, place.location, place.total as u64, AccessDirection::Write)
    } else {
        None
    };
    if let Some((fault_address, status)) = fault {
        proof {
            lemma_receives_none(*mutation_hook);
        }
        return Err(Exception::DataAbort {
            status,
            access: AccessDirection::Write,
            address: to_fault_address(place.address, fault_address, place.tag),
        });
    }
    processor.exclusive_monitor = ExclusiveMonitor::Open;
    // VERUS: `planted::any` has no specification, so it is two `is`.
    let failed = if crate::planted::is(442) || crate::planted::is(163) {
        0
    } else {
        STORE_EXCLUSIVE_FAILED
    };
    write_register(processor, status, failed & width.to_mask(), RegisterThirtyOne::ZeroRegister);
    proof {
        lemma_receives_none(*mutation_hook);
    }
    Ok(())
}

/// What an atomic stores from the value it read, its operand, its first register's whole value and the swapped value.
fn atomic_store_value(
    atomic: AtomicOperation,
    previous: u64,
    operand: u64,
    unmasked: u64,
    swapped: u64,
    mask: u64,
    bits: u32,
) -> (new: Option<u64>)
    requires
        1 <= bits <= 64,
        mask == crate::pseudocode::ones(bits as nat),
    ensures
        new == atomic_value(atomic, previous, operand, swapped, bits as nat),
{
    let shift = if crate::planted::is(272) {
        u64::BITS - u8::BITS
    } else {
        u64::BITS - bits
    };
    let signed_previous = ((previous << shift) as i64) >> shift;
    let signed_operand = ((operand << shift) as i64) >> shift;
    proof {
        crate::lemmas::lemma_sign_extension(previous, shift);
        crate::lemmas::lemma_sign_extension(operand, shift);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        let sum = previous.wrapping_add(operand);
        crate::lemmas::lemma_mask(sum, bits as nat);
        if previous + operand > u64::MAX {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(
                previous + operand,
                pow2(64) as int,
                1,
                previous + operand - pow2(64),
            );
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod((previous + operand) as nat, pow2(64));
        }
        crate::lemmas::lemma_wrapped_mod(previous + operand, bits as nat);
    }
    // VERUS: no specification of `bool::then`, `max` or `min`, so each is an `if`.
    match atomic {
        AtomicOperation::Add if crate::planted::is(439) => Some(previous),
        AtomicOperation::Add => Some(previous.wrapping_add(operand) & mask),
        AtomicOperation::Clear if crate::planted::is(47) => Some(previous & operand),
        AtomicOperation::Clear => Some(previous & !operand),
        AtomicOperation::ExclusiveOr if crate::planted::is(270) => Some(previous | operand),
        AtomicOperation::ExclusiveOr => Some(previous ^ operand),
        AtomicOperation::Set => Some(previous | operand),
        AtomicOperation::SignedMaximum if crate::planted::is(160) => Some(
            if previous >= operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::SignedMaximum => Some(
            if signed_previous >= signed_operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::SignedMinimum if crate::planted::is(48) => Some(
            if previous <= operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::SignedMinimum => Some(
            if signed_previous <= signed_operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::UnsignedMaximum => Some(
            if previous >= operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::UnsignedMinimum if crate::planted::is(273) => Some(
            if signed_previous <= signed_operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::UnsignedMinimum => Some(
            if previous <= operand {
                previous
            } else {
                operand
            },
        ),
        AtomicOperation::Swap if crate::planted::is(300) => Some(swapped),
        AtomicOperation::Swap => Some(operand),
        AtomicOperation::CompareAndSwap if crate::planted::is(44) => if previous & 0xFF == operand & 0xFF {
            Some(swapped)
        } else {
            None
        },
        AtomicOperation::CompareAndSwap if crate::planted::is(161) => Some(swapped),
        AtomicOperation::CompareAndSwap if crate::planted::is(162) => if previous == unmasked {
            Some(swapped)
        } else {
            None
        },
        AtomicOperation::CompareAndSwap => if previous == operand {
            Some(swapped)
        } else {
            None
        },
    }
}

/// An atomic's access, once its address passed its checks.
fn atomic_access<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    atomic: AtomicOperation,
    bytes: usize,
    place: Place,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count == 2,
        bytes <= 8,
        is_access_size(bytes as u32, false),
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
    ensures
        ({
            let (outcome, after, changes) = atomic_outcome(
                register_view(*old(processor)),
                memory_of(memory@),
                named(*addressed),
                atomic,
                bytes as nat,
                place.address,
                place.location,
                place.tag,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(atomic_outcome);
    }
    // VERUS: no specification of an array's iterator, so the read and the write are checked in turn.
    if let Some((fault_address, status)) = find_fault(memory, place.location, bytes as u64, AccessDirection::Read) {
        proof {
            lemma_receives_none(*mutation_hook);
        }
        return Err(Exception::DataAbort {
            status,
            access: AccessDirection::Read,
            address: to_fault_address(place.address, fault_address, place.tag),
        });
    }
    if let Some((fault_address, status)) = find_fault(memory, place.location, bytes as u64, AccessDirection::Write) {
        proof {
            lemma_receives_none(*mutation_hook);
        }
        return Err(Exception::DataAbort {
            status,
            access: AccessDirection::Write,
            address: to_fault_address(place.address, fault_address, place.tag),
        });
    }
    let bits = bytes as u32 * u8::BITS;
    let mask = low_ones(bits);
    let read = read_byte_sequence(memory, place.location, bytes);
    let loaded = little_endian_value(read.as_slice(), 0, bytes);
    proof {
        assert(read@.subrange(0, bytes as int) =~= memory_bytes(memory_of(memory@), place.location, bytes as nat));
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        if bits < 64 {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases(bits as nat, 64);
        }
    }
    proof {
        lemma_low_doubleword(loaded);
        vstd::arithmetic::div_mod::lemma_small_mod(loaded as nat, pow2(64));
    }
    let previous = loaded as u64;
    let (first, first_width) = addressed.registers[0];
    let (second, second_width) = addressed.registers[1];
    proof {
        assert(named(*addressed)[0] == addressed.registers[0]);
        assert(named(*addressed)[1] == addressed.registers[1]);
    }
    let unmasked = read_register(processor, first, RegisterThirtyOne::ZeroRegister);
    let operand = unmasked & if crate::planted::is(440) {
        u64::MAX
    } else {
        mask
    };
    let swapped = read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask;
    proof {
        crate::lemmas::lemma_mask(unmasked, bits as nat);
        crate::lemmas::lemma_mask(read_x(register_view(*processor), second, RegisterThirtyOne::ZeroRegister), bits as nat);
    }
    let new = atomic_store_value(atomic, previous, operand, unmasked, swapped, mask, bits);
    if let Some(new) = new {
        let mut stored: Vec<u8> = Vec::new();
        push_little_endian(&mut stored, new as u128, bytes);
        let ghost stored_bytes = stored@;
        mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment { start: place.location, bytes: Cow::Owned(stored) });
        proof {
            lemma_receives_one(
                *old(mutation_hook),
                *mutation_hook,
                Assignment::ValueAssignment { start: place.location, bytes: stored_bytes },
            );
        }
    } else {
        proof {
            lemma_receives_none(*mutation_hook);
        }
    }
    let (target, width) = match atomic {
        AtomicOperation::CompareAndSwap => (first, first_width),
        _ if crate::planted::is(300) => (first, first_width),
        _ => (second, second_width),
    };
    let returned = if crate::planted::is(275) {
        ((previous << 48) as i64 >> 48) as u64
    } else {
        match (atomic, new) {
            (AtomicOperation::Swap, Some(new)) if crate::planted::is(49) => new,
            (_, Some(new)) if crate::planted::is(159) => new,
            _ => previous,
        }
    };
    proof {
        crate::lemmas::lemma_mask(returned, width.bits());
    }
    if !(crate::planted::is(268) && matches!(atomic, AtomicOperation::CompareAndSwap) && previous != operand) {
        write_register(
            processor,
            target,
            returned & width.to_mask(),
            if crate::planted::is(167) {
                RegisterThirtyOne::StackPointer
            } else {
                RegisterThirtyOne::ZeroRegister
            },
        );
    }
    Ok(())
}

/// A compare and swap pair's access, once its address passed its checks.
fn compare_and_swap_pair<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    bytes: usize,
    place: Place,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count == 4,
        bytes == 4 || bytes == 8,
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
    ensures
        ({
            let (outcome, after, changes) = pair_outcome(
                register_view(*old(processor)),
                memory_of(memory@),
                named(*addressed),
                bytes as nat,
                place.address,
                place.location,
                place.tag,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(pair_outcome);
    }
    let total = 2 * bytes;
    // VERUS: no specification of an array's iterator, so the read and the write are checked in turn.
    if let Some((fault_address, status)) = find_fault(memory, place.location, total as u64, AccessDirection::Read) {
        proof {
            lemma_receives_none(*mutation_hook);
        }
        return Err(Exception::DataAbort {
            status,
            access: AccessDirection::Read,
            address: to_fault_address(place.address, fault_address, place.tag),
        });
    }
    if let Some((fault_address, status)) = find_fault(memory, place.location, total as u64, AccessDirection::Write) {
        proof {
            lemma_receives_none(*mutation_hook);
        }
        return Err(Exception::DataAbort {
            status,
            access: AccessDirection::Write,
            address: to_fault_address(place.address, fault_address, place.tag),
        });
    }
    let element_bits = bytes as u32 * u8::BITS;
    let mask = low_ones(element_bits);
    let read = read_byte_sequence(memory, place.location, total);
    let previous = little_endian_value(read.as_slice(), 0, total);
    let (compared, width) = addressed.registers[0];
    let swapped = addressed.registers[2].0;
    proof {
        assert(read@.subrange(0, total as int) =~= memory_bytes(memory_of(memory@), place.location, total as nat));
        assert(named(*addressed)[0] == addressed.registers[0]);
        assert(named(*addressed)[2] == addressed.registers[2]);
    }
    let compared_value = read_pair(processor, compared, mask, element_bits);
    let equal = if crate::planted::is(45) {
        previous as u64 & mask == compared_value as u64 & mask
    } else {
        previous == compared_value
    };
    if equal {
        let new = read_pair(processor, swapped, mask, element_bits);
        let new = if crate::planted::is(46) {
            let low = new & ((1_u128 << element_bits) - 1);
            (new >> element_bits) | (low << element_bits)
        } else {
            new
        };
        let mut stored: Vec<u8> = Vec::new();
        push_little_endian(&mut stored, new, total);
        let ghost stored_bytes = stored@;
        mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment { start: place.location, bytes: Cow::Owned(stored) });
        proof {
            lemma_receives_one(
                *old(mutation_hook),
                *mutation_hook,
                Assignment::ValueAssignment { start: place.location, bytes: stored_bytes },
            );
        }
    } else {
        proof {
            lemma_receives_none(*mutation_hook);
        }
    }
    let returned_to = if crate::planted::is(269) {
        swapped
    } else {
        compared
    };
    let high = previous >> element_bits;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::bits::lemma_u128_shr_is_div(previous, element_bits as u128);
        assert(previous >> element_bits == previous >> (element_bits as u128)) by (bit_vector);
        lemma_low_doubleword(previous);
        lemma_low_doubleword(high);
        crate::lemmas::lemma_mask(previous as u64, element_bits as nat);
        crate::lemmas::lemma_mask(high as u64, element_bits as nat);
        crate::lemmas::lemma_wrapped_mod(previous as int, element_bits as nat);
        crate::lemmas::lemma_wrapped_mod(high as int, element_bits as nat);
        crate::lemmas::lemma_mask(previous as u64 & mask, width.bits());
        crate::lemmas::lemma_mask(high as u64 & mask, width.bits());
    }
    write_register(processor, returned_to, previous as u64 & mask & width.to_mask(), RegisterThirtyOne::ZeroRegister);
    write_register(processor, returned_to + 1, high as u64 & mask & width.to_mask(), RegisterThirtyOne::ZeroRegister);
    Ok(())
}

/// The data with the bytes of its third register moved to its second: a planted bug's.
// VERUS: no specification of `copy_within`, so the copy is a loop.
fn copied_within(data: &[u8], bytes: usize) -> Vec<u8>
    requires
        bytes <= 16,
{
    let mut copied: Vec<u8> = Vec::new();
    let mut at = 0;
    while at < data.len()
        invariant
            bytes <= 16,
        decreases data.len() - at,
    {
        copied.push(
            if bytes <= at && at - bytes < bytes && bytes < data.len() - at {
                data[at + bytes]
            } else {
                data[at]
            },
        );
        at += 1;
    }
    copied
}

/// A load's access, once its address passed its checks.
fn load_registers<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    operation: LoadStoreOperation,
    signedness: Signedness,
    place: Place,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count <= 2,
        is_access_size(operation.bytes, addressed.vector),
        signedness == Signedness::Signed ==> operation.bytes <= 8,
        place.total == operation.bytes * addressed.count,
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
        addressed.base matches Some((_, indexing)) ==> place.address as int == crate::pseudocode::accessed_address(
            place.base,
            addressed.offset,
            indexing,
        ),
    ensures
        ({
            let (outcome, after) = load_outcome(
                register_view(*old(processor)),
                memory_of(memory@),
                named(*addressed),
                addressed.vector,
                operation,
                signedness,
                place.pair_order_reversed,
                place.address,
                place.location,
                place.tag,
                place.total as nat,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
        }),
{
    proof {
        reveal(load_outcome);
    }
    let bytes = operation.bytes as usize;
    let count = addressed.count;
    let pair_of_words = crate::planted::is(39) && count == 2 && bytes == 4;
    let fetched = if pair_of_words {
        2 * place.total
    } else {
        place.total
    };
    let data = match load(memory, place.address, place.location, place.tag, fetched) {
        Ok(data) => data,
        Err(exception) => {
            if crate::planted::is(434) && place.writeback {
                write_back(processor, addressed, place.base, place.address);
            }
            // VERUS: `planted::any` has no specification, so it is two `is`.
            if (crate::planted::is(261) || crate::planted::is(295)) && place.writeback {
                write_back(processor, addressed, place.base, place.address);
            }
            if let (true, Some((number, Indexing::PostIndex))) = (
                crate::planted::is(149) && place.writeback,
                addressed.base,
            ) {
                let written_back = place.base.wrapping_add(addressed.offset);
                write_register(processor, number, written_back, RegisterThirtyOne::StackPointer);
            }
            return Err(exception);
        },
    };
    let data = if pair_of_words {
        copied_within(data.as_slice(), bytes)
    } else {
        data
    };
    // VERUS: `planted::any` has no specification, so it is two `is`.
    let signedness = if crate::planted::is(441) || crate::planted::is(289) {
        Signedness::Signed
    } else if crate::planted::is(296) {
        Signedness::Unsigned
    } else {
        signedness
    };
    // A planted bug sign-extends a word from 16 bits.
    let sign_bits = if crate::planted::is(36) && bytes == 4 {
        u16::BITS
    } else {
        bytes as u32 * u8::BITS
    };
    let ghost view = register_view(*processor);
    let ghost loaded = memory_bytes(memory_of(memory@), place.location, place.total as nat);
    proof {
        assert(data@ =~= loaded);
        assert(loaded.len() == place.total);
    }
    let mut values: [u128; 2] = [0, 0];
    let mut index = 0;
    while index < count
        invariant
            index <= count == addressed.count <= 2,
            bytes == operation.bytes,
            is_access_size(operation.bytes, addressed.vector),
            signedness == Signedness::Signed ==> operation.bytes <= 8,
            place.total == bytes * count,
            data@ == loaded,
            loaded.len() == place.total,
            sign_bits == 8 * bytes,
            forall|at: int| 0 <= at < index ==> #[trigger] values@[at] as int == uint(
                loaded_value(loaded, at as nat, bytes as nat, signedness),
                128,
            ),
        decreases count - index,
    {
        let from = if crate::planted::is(266) {
            0
        } else {
            index
        };
        // VERUS: `planted::any` has no specification, so it is two `is`.
        let signedness = if crate::planted::is(52) || crate::planted::is(151) || (crate::planted::is(155) && index == 1) {
            Signedness::Unsigned
        } else {
            signedness
        };
        proof {
            assert(from * bytes + bytes <= place.total) by (nonlinear_arith)
                requires
                    from < count,
                    place.total == bytes * count;
        }
        values[index] = extended_value(data.as_slice(), from * bytes, bytes, signedness, sign_bits);
        index += 1;
    }
    if crate::planted::is(450) {
        values[1] = values[0];
    }
    // VERUS: no specification of `swap`, so the planted bugs' swap is written out.
    if crate::planted::is(457) || crate::planted::is(292) {
        values = [values[1], values[0]];
    }
    proof {
        if count >= 1 {
            assert(addressed.registers[0] == named(*addressed)[0]);
            assert(values@[0] as int == uint(loaded_value(loaded, 0, bytes as nat, signedness), 128));
        }
        if count == 2 {
            assert(addressed.registers[1] == named(*addressed)[1]);
            assert(values@[1] as int == uint(loaded_value(loaded, 1, bytes as nat, signedness), 128));
        }
    }
    if count == 2 && place.pair_order_reversed {
        write_loaded_register(processor, addressed.registers[1], addressed.vector, values[1], operation.bytes);
        write_loaded_register(processor, addressed.registers[0], addressed.vector, values[0], operation.bytes);
    } else {
        if count >= 1 {
            write_loaded_register(processor, addressed.registers[0], addressed.vector, values[0], operation.bytes);
        }
        if count == 2 {
            write_loaded_register(processor, addressed.registers[1], addressed.vector, values[1], operation.bytes);
        }
    }
    if matches!(operation.alignment, Alignment::Exclusive) {
        processor.exclusive_monitor = ExclusiveMonitor::Exclusive { address: place.location };
    }
    Ok(())
}

/// A store's access, once its address passed its checks.
fn store_registers<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    place: Place,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count <= 2,
        is_access_size(operation.bytes, addressed.vector),
        place.total == operation.bytes * addressed.count,
        forall|index: int| 0 <= index < addressed.count ==> #[trigger] named(*addressed)[index].0 < 32,
        addressed.base matches Some((_, indexing)) ==> place.address as int == accessed_address(
            place.base,
            addressed.offset,
            indexing,
        ),
    ensures
        ({
            let (outcome, changes) = store_outcome(
                memory_of(memory@),
                place.address,
                place.location,
                place.tag,
                stored_bytes(register_view(*old(processor)), named(*addressed), addressed.vector, operation.bytes as nat),
                store_across_fault(choices),
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == register_view(*old(processor))
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(store_outcome);
    }
    let ghost registers = named(*addressed);
    let bytes = operation.bytes as usize;
    let data = register_bytes(processor, addressed, 0, bytes);
    proof {
        assert(registers.skip(0) =~= registers);
    }
    let written = if crate::planted::is(43) && bytes == 8 {
        place.total / 2
    } else if crate::planted::is(50) && bytes == 1 {
        if 2 * place.total < 32 {
            2 * place.total
        } else {
            32
        }
    } else {
        place.total
    };
    proof {
        lemma_stored_bytes_length(register_view(*processor), registers, addressed.vector, bytes as nat);
    }
    // A planted bug writes more bytes than the registers give; the rest are zero.
    let data = if crate::planted::is(50) {
        zero_padded(data, written)
    } else {
        data
    };
    // VERUS: `planted::any` has no specification, so it is two `is`.
    if (crate::planted::is(262) || crate::planted::is(284)) && place.writeback && find_fault(
        memory,
        place.location,
        place.total as u64,
        AccessDirection::Write,
    ).is_some() {
        write_back(processor, addressed, place.base, place.address);
    }
    let across_fault = match choices.across_fault {
        Some(across_fault) => across_fault,
        None => StoreAcrossFault::WritesNothing,
    };
    proof {
        assert(written == data@.len());
        assert(data@.take(data@.len() as int) =~= data@);
    }
    let stored = store(
        memory,
        place.address,
        place.location,
        place.tag,
        slice_prefix(data.as_slice(), written),
        across_fault,
        mutation_hook,
    );
    if let Err(exception) = stored {
        if crate::planted::is(449) && place.writeback {
            write_back(processor, addressed, place.base, place.address);
        }
        return Err(exception);
    }
    Ok(())
}

/// A load or store's access once its address passed its checks, before any writeback.
fn transfer<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    place: Place,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count <= MAXIMUM_REGISTERS,
        well_shaped(named(*addressed), addressed.vector, operation),
        place.total == operation.bytes * data_count(operation.transfer, addressed.count as nat),
        addressed.base matches Some((_, indexing)) ==> place.address as int == accessed_address(
            place.base,
            addressed.offset,
            indexing,
        ),
    ensures
        ({
            let (outcome, after, changes) = transfer_outcome(
                register_view(*old(processor)),
                memory_of(memory@),
                named(*addressed),
                addressed.vector,
                operation,
                choices,
                place.pair_order_reversed,
                place.address,
                place.location,
                place.tag,
                place.total as nat,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(transfer_outcome);
        reveal(well_shaped);
        lemma_receives_none(*mutation_hook);
    }
    let ghost registers = named(*addressed);
    let bytes = operation.bytes as usize;
    let access = match operation.transfer {
        Transfer::Store | Transfer::StoreExclusive => AccessDirection::Write,
        Transfer::Load(_) | Transfer::Prefetch | Transfer::Atomic(_) | Transfer::CompareAndSwapPair => AccessDirection::Read,
    };
    match operation.transfer {
        Transfer::Prefetch => {
            let fault = if crate::planted::is(51) {
                find_fault(memory, place.location, 8, access)
            } else {
                None
            };
            if let Some((fault_address, status)) = fault {
                return Err(Exception::DataAbort {
                    status,
                    access,
                    address: to_fault_address(place.address, fault_address, place.tag),
                });
            }
            Ok(())
        },
        Transfer::StoreExclusive => store_exclusive(
            processor,
            memory,
            addressed,
            bytes,
            choices,
            place,
            mutation_hook,
        ),
        Transfer::Atomic(atomic) => atomic_access(
            processor,
            memory,
            addressed,
            atomic,
            bytes,
            place,
            mutation_hook,
        ),
        Transfer::CompareAndSwapPair => compare_and_swap_pair(
            processor,
            memory,
            addressed,
            bytes,
            place,
            mutation_hook,
        ),
        Transfer::Store => store_registers(
            processor,
            memory,
            addressed,
            operation,
            choices,
            place,
            mutation_hook,
        ),
        Transfer::Load(signedness) => load_registers(
            processor,
            memory,
            addressed,
            operation,
            signedness,
            place,
        ),
    }
}

/// Applies the load or store the word encodes, as its Arm pseudocode does with the behavior's choices for the cases
/// the manual leaves open.
fn simulate_load_store<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    operands: &[Operand],
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    ensures
        ({
            let (outcome, after, changes) = load_store_outcome(
                *old(processor),
                memory_of(memory@),
                word,
                operands@,
                operation,
                choices,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        lemma_receives_none(*mutation_hook);
    }
    let addressed = match read_addressed(processor, word, operands, operation) {
        Ok(addressed) => addressed,
        // Its only error is UNDEFINED, where an operand is.
        Err(_) => return Err(Exception::UndefinedInstruction),
    };
    let ghost registers = named(addressed);
    proof {
        let (decoded, _, _, _) = operands_addressed(*processor, word, operands@, operation.bytes).unwrap();
        assert(decoded =~= registers);
    }
    // VERUS: the operand lists are a table Verus does not read, so a list no load or store's decode gives is refused as
    // UNDEFINED, where it would have read past the registers or panicked.
    if !is_well_shaped(&addressed, operation) {
        return Err(Exception::UndefinedInstruction);
    }
    let (writeback, pair_order_reversed) = match constrain(&addressed, operation, choices) {
        Constrained::Undefined => return Err(Exception::UndefinedInstruction),
        Constrained::NoOperation => return Ok(()),
        Constrained::Runs { writeback, pair_order_reversed } => (writeback, pair_order_reversed),
    };
    simulate_access(processor, memory, &addressed, operation, choices, (writeback, pair_order_reversed), mutation_hook)
}

/// A load or store's access from its decoded operands, as the behavior's choices constrain it.
fn simulate_access<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    addressed: &Addressed,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    run: (bool, bool),
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (result: Result<(), Exception>)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    requires
        addressed.count <= MAXIMUM_REGISTERS,
        well_shaped(named(*addressed), addressed.vector, operation),
    ensures
        ({
            let (outcome, after, changes) = accessed_outcome(
                register_view(*old(processor)),
                old(processor).program_counter,
                memory_of(memory@),
                named(*addressed),
                addressed.vector,
                addressed.base,
                addressed.offset,
                run.0,
                run.1,
                operation,
                choices,
            );
            &&& result == outcome
            &&& register_view(*final(processor)) == after
            &&& only_registers_changed(*old(processor), *final(processor))
            &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
        }),
{
    proof {
        reveal(accessed_outcome);
        reveal(well_shaped);
        lemma_receives_none(*mutation_hook);
    }
    // Whether a pre- or post-indexed base is written back, and whether a load pair into one register keeps the first
    // value.
    let (writeback, pair_order_reversed) = run;
    let (base, address) = match addressed.base {
        None => (0, to_accessed_address(processor.program_counter, addressed.offset, Indexing::Offset)),
        Some((number, indexing)) => {
            let base = read_register(processor, number, RegisterThirtyOne::StackPointer);
            let checked = if crate::planted::is(60) && matches!(indexing, Indexing::PostIndex) {
                base.wrapping_add(addressed.offset)
            } else {
                base
            };
            // VERUS: no specification of `is_multiple_of`, so the remainder is compared with zero.
            let misaligned = !is_general_purpose(number) && checked % if crate::planted::is(165) {
                8
            } else {
                STACK_POINTER_ALIGNMENT
            } != 0;
            if (misaligned && matches!(choices.misaligned_stack_pointer, Some(MisalignedStackPointer::Faults))
                && !crate::planted::is(282)) || (misaligned && crate::planted::is(288)) {
                return Err(Exception::StackPointerAlignmentFault);
            }
            (base, to_accessed_address(base, addressed.offset, indexing))
        },
    };
    let bytes = operation.bytes as usize;
    let count = addressed.count;
    let moved = match operation.transfer {
        Transfer::StoreExclusive => count - 1,
        Transfer::Atomic(_) => 1,
        Transfer::CompareAndSwapPair => count / 2,
        Transfer::Load(_) | Transfer::Store | Transfer::Prefetch => count,
    };
    proof {
        assert(moved == 0 || (bytes <= 16 && moved <= 2));
        assert(bytes * moved <= 32) by (nonlinear_arith)
            requires
                moved == 0 || (bytes <= 16 && moved <= 2);
    }
    let total = bytes * moved;
    proof {
        assert(named(*addressed).len() == count);
        assert(moved as nat == data_count(operation.transfer, named(*addressed).len()));
        assert(total as nat == (operation.bytes as nat * data_count(operation.transfer, named(*addressed).len())) as nat);
    }
    let access = match operation.transfer {
        Transfer::Store | Transfer::StoreExclusive => AccessDirection::Write,
        Transfer::Load(_) | Transfer::Prefetch | Transfer::Atomic(_) | Transfer::CompareAndSwapPair => AccessDirection::Read,
    };
    // VERUS: an access of no bytes, which only a prefetch makes, has no alignment to check.
    let aligned = total == 0 || is_aligned(operation.alignment, choices.misaligned_exclusive, address, total as u64);
    let tag = match choices.fault_address_tag {
        Some(tag) => tag,
        None => FaultAddressTag::KeepsTopByte,
    };
    if !aligned {
        return Err(Exception::DataAbort {
            status: FaultStatus::AlignmentFault,
            access,
            address: to_fault_address(address, address, tag),
        });
    }
    // TBI0, which Linux sets for EL0, leaves the top byte of a lower-half address out of translation.
    let location = if crate::planted::is(301) {
        address
    } else {
        to_untagged_address(address)
    };
    let place = Place { writeback, pair_order_reversed, base, address, location, tag, total };
    let transferred = transfer(processor, memory, addressed, operation, choices, place, mutation_hook);
    // The writeback once the access completed.
    if let (true, Some((number, indexing))) = (
        transferred.is_ok() && writeback && !crate::planted::is(154),
        addressed.base,
    ) {
        let written_back = to_written_back_address(base, addressed.offset, address, indexing);
        let written_back = if crate::planted::is(34) {
            to_untagged_address(written_back)
        } else {
            written_back
        };
        // VERUS: `planted::any` has no specification, so it is two `is`.
        let written_back = if crate::planted::is(435) || crate::planted::is(453) {
            written_back & low_ones(56)
        } else {
            written_back
        };
        let written_back = if crate::planted::is(263) {
            to_untagged_address(written_back)
        } else {
            written_back
        };
        write_register(processor, number, written_back, RegisterThirtyOne::StackPointer);
    }
    // The access's own `Ok`, which the writeback leaves.
    transferred
}


} // verus!

verus! {

/// Where the four tag bits start that a fault address need not keep.
const UNKNOWN_TAG_BITS_SHIFT: u32 = u64::BITS - u8::BITS / 2;

/// The fault an access of one byte at the address takes, none where its page lets it through: a translation fault at
/// level 0 outside the lower half's addresses, at level 3 where no page is mapped, and a permission fault at level 3
/// where the page forbids the access.
pub open spec fn byte_fault(memory: Memory<PagePermission>, address: u64, access: AccessDirection) -> Option<FaultStatus> {
    if is_outside_lower(address) {
        Some(FaultStatus::TranslationFaultLevel0)
    } else {
        match memory(address).1 {
            PagePermission::Unmapped => Some(FaultStatus::TranslationFaultLevel3),
            PagePermission::Mapped { data_access, .. } => match (data_access, access) {
                (DataAccess::ReadWrite, _) | (DataAccess::ReadOnly, AccessDirection::Read) => None,
                _ => Some(FaultStatus::PermissionFaultLevel3),
            },
        }
    }
}

/// The lowest address of the `length` bytes from `start` whose page refuses the access, with its fault.
pub open spec fn first_fault(memory: Memory<PagePermission>, start: u64, length: nat, access: AccessDirection) -> Option<(u64, FaultStatus)>
    decreases length,
{
    if length == 0 {
        None
    } else {
        match first_fault(memory, start, (length - 1) as nat, access) {
            Some(found) => Some(found),
            None => {
                let address = start.wrapping_add((length - 1) as u64);
                match byte_fault(memory, address, access) {
                    Some(status) => Some((address, status)),
                    None => None,
                }
            },
        }
    }
}

/// The address FAR_EL1 gives for a fault at `fault_location` of an access to `address`: the location with its top
/// byte left out, and the access's tag bits, all of them or all but the top four, as the behavior keeps them.
pub open spec fn fault_address(address: u64, fault_location: u64, tag: FaultAddressTag) -> u64 {
    let tag_bits = address ^ branch_address(address);
    branch_address(fault_location) | match tag {
        FaultAddressTag::KeepsTopByte => tag_bits,
        FaultAddressTag::ClearsTopNibble => tag_bits & !(u64::MAX << 60u32),
    }
}

/// Writes the bytes at the address, or, where a byte faults, what the behavior across a fault writes.
pub(crate) fn store<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    bytes: &[u8],
    across_fault: StoreAcrossFault,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (stored: Result<(), Exception>)
    ensures
        match first_fault(memory_of(memory@), location, bytes@.len(), AccessDirection::Write) {
            None => stored is Ok && receives(
                *old(mutation_hook),
                *final(mutation_hook),
                seq![Assignment::ValueAssignment { start: location, bytes: bytes@ }],
            ),
            Some((fault_location, status)) => {
                let below_fault = bytes@.take(fault_location.wrapping_sub(location) as int);
                &&& stored == Err::<(), Exception>(Exception::DataAbort {
                    status,
                    access: AccessDirection::Write,
                    address: fault_address(address, fault_location, tag),
                })
                &&& receives(
                    *old(mutation_hook),
                    *final(mutation_hook),
                    if across_fault == StoreAcrossFault::WritesBytesBelowFault && below_fault.len() > 0 {
                        seq![Assignment::ValueAssignment { start: location, bytes: below_fault }]
                    } else {
                        Seq::empty()
                    },
                )
            },
        },
{
    let access = AccessDirection::Write;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
    }
    if let Some((fault_location, status)) = find_fault(memory, location, bytes.len() as u64, access) {
        proof {
            lemma_first_fault_offset(memory_of(memory@), location, bytes@.len(), access);
        }
        let below_fault = slice_prefix(bytes, fault_location.wrapping_sub(location) as usize);
        // VERUS: no `==` on `StoreAcrossFault` in executable code, so the behavior is matched.
        if (matches!(across_fault, StoreAcrossFault::WritesBytesBelowFault) || crate::planted::is(40))
            && !below_fault.is_empty()
        {
            mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                start: location,
                bytes: Cow::Borrowed(below_fault),
            });
            proof {
                lemma_receives_one(*old(mutation_hook), *mutation_hook, Assignment::ValueAssignment { start: location, bytes: below_fault@ });
            }
        } else {
            proof {
                lemma_receives_none(*mutation_hook);
            }
        }
        return Err(Exception::DataAbort {
            status,
            access,
            address: if crate::planted::is(157) {
                address
            } else {
                to_fault_address(address, fault_location, tag)
            },
        });
    }
    mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
        start: location,
        bytes: Cow::Borrowed(bytes),
    });
    proof {
        lemma_receives_one(*old(mutation_hook), *mutation_hook, Assignment::ValueAssignment { start: location, bytes: bytes@ });
    }
    Ok(())
}

/// The first `length` bytes of a slice, or all of them where it has fewer.
fn slice_prefix(bytes: &[u8], length: usize) -> (prefix: &[u8])
    ensures
        prefix@ == bytes@.take(if length < bytes@.len() { length as int } else { bytes@.len() as int }),
{
    let length = if length < bytes.len() {
        length
    } else {
        bytes.len()
    };
    // VERUS: slicing by a range is specified through `subrange`.
    vstd::slice::slice_subrange(bytes, 0, length)
}

/// A sink that received one change went from its memory to that change applied to it.
proof fn lemma_receives_one<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    before: Sink,
    after: Sink,
    change: Assignment<Architecture::PagePermission>,
)
    requires
        forall|memory: Memory<Architecture::PagePermission>|
            #[trigger] after.applied_to(memory) == apply(before.applied_to(memory), change),
    ensures
        receives(before, after, seq![change]),
{
    assert forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), seq![change]) by {
        let changes = seq![change];
        let none = changes.drop_last();
        assert(none =~= Seq::<Assignment<Architecture::PagePermission>>::empty());
        assert(apply_all(before.applied_to(memory), none) == before.applied_to(memory));
        assert(changes.last() == change);
        assert(apply_all(before.applied_to(memory), changes) == apply(apply_all(before.applied_to(memory), none), change));
    }
}

/// A sink that received nothing holds what it held.
proof fn lemma_receives_none<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(sink: Sink)
    ensures
        receives(sink, sink, Seq::empty()),
{
}

/// Reads `length` bytes at the address, or returns the fault of the first one whose page forbids reading it.
pub(crate) fn load<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    length: usize,
) -> (loaded: Result<Vec<u8>, Exception>)
    ensures
        match first_fault(memory_of(memory@), location, length as nat, AccessDirection::Read) {
            None => loaded matches Ok(bytes) && bytes@ == memory_bytes(memory_of(memory@), location, length as nat),
            Some(fault) => loaded == Err::<Vec<u8>, Exception>(data_abort(fault, AccessDirection::Read, address, tag)),
        },
{
    let access = AccessDirection::Read;
    match find_fault(memory, location, length as u64, access) {
        None => {
            let bytes = read_byte_sequence(memory, location, length);
            proof {
                assert(bytes@ =~= memory_bytes(memory_of(memory@), location, length as nat));
            }
            Ok(bytes)
        },
        Some((fault_location, status)) => Err(Exception::DataAbort {
            status,
            access,
            address: to_fault_address(address, fault_location, tag),
        }),
    }
}

/// Finds the lowest address of the access that its page refuses, and the fault's status.
pub(crate) fn find_fault<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    start: u64,
    length: u64,
    access: AccessDirection,
) -> (fault: Option<(u64, FaultStatus)>)
    ensures
        fault == first_fault(memory_of(memory@), start, length as nat, access),
{
    // VERUS: no specification of `Iterator::map` or `find_map`, so the offsets are walked by a loop.
    let mut offset: u64 = 0;
    while offset < length
        invariant
            offset <= length,
            first_fault(memory_of(memory@), start, offset as nat, access) is None,
        decreases length - offset,
    {
        let address = start.wrapping_add(offset);
        proof {
            assert(first_fault(memory_of(memory@), start, (offset + 1) as nat, access) == match byte_fault(
                memory_of(memory@),
                address,
                access,
            ) {
                Some(status) => Some((address, status)),
                None => None::<(u64, FaultStatus)>,
            });
        }
        if is_outside_lower_addresses(address) {
            proof {
                lemma_first_fault_stays(memory_of(memory@), start, (offset + 1) as nat, length as nat, access);
            }
            return Some((address, FaultStatus::TranslationFaultLevel0));
        }
        let status = match read_permission(memory, address) {
            PagePermission::Unmapped => Some(FaultStatus::TranslationFaultLevel3),
            PagePermission::Mapped { data_access, .. } => match (data_access, access) {
                (DataAccess::ReadWrite, _) | (DataAccess::ReadOnly, AccessDirection::Read) => None,
                (DataAccess::ReadOnly, AccessDirection::Write) | (DataAccess::Forbidden, _) => {
                    Some(FaultStatus::PermissionFaultLevel3)
                },
            },
        };
        if let Some(status) = status {
            proof {
                lemma_first_fault_stays(memory_of(memory@), start, (offset + 1) as nat, length as nat, access);
            }
            return Some((address, status));
        }
        offset += 1;
    }
    None
}

/// The first fault lies `wrapping_sub(start)` bytes past the start, inside the access.
proof fn lemma_first_fault_offset(memory: Memory<PagePermission>, start: u64, length: nat, access: AccessDirection)
    ensures
        first_fault(memory, start, length, access) matches Some((address, _)) ==> address.wrapping_sub(start) < length
            && address == start.wrapping_add(address.wrapping_sub(start)),
    decreases length,
{
    if length > 0 {
        lemma_first_fault_offset(memory, start, (length - 1) as nat, access);
    }
}

/// The first fault of a prefix of the bytes is the first fault of all of them.
proof fn lemma_first_fault_stays(memory: Memory<PagePermission>, start: u64, prefix: nat, length: nat, access: AccessDirection)
    requires
        prefix <= length,
        first_fault(memory, start, prefix, access) is Some,
    ensures
        first_fault(memory, start, length, access) == first_fault(memory, start, prefix, access),
    decreases length - prefix,
{
    if prefix < length {
        lemma_first_fault_stays(memory, start, prefix, (length - 1) as nat, access);
    }
}

/// The fault address FAR_EL1 gives for a fault at `fault_location` of an access to `address`, with the tag the
/// behavior keeps.
pub(crate) fn to_fault_address(address: u64, fault_location: u64, tag: FaultAddressTag) -> (fault: u64)
    ensures
        fault == fault_address(address, fault_location, tag),
{
    let location = to_untagged_address(fault_location);
    // VERUS: `planted::any` has no specification, so it is two `is`.
    let tag_bits = if crate::planted::is(264) || crate::planted::is(35) {
        0
    } else {
        address ^ to_untagged_address(address)
    };
    let kept = match tag {
        FaultAddressTag::KeepsTopByte => tag_bits,
        FaultAddressTag::ClearsTopNibble => tag_bits & !(u64::MAX << UNKNOWN_TAG_BITS_SHIFT),
    };
    location | kept
}

} // verus!
