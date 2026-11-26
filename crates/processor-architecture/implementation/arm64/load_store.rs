//! The loads and stores of general-purpose registers and of SIMD and floating-point registers: one skeleton reads
//! the base, offset and registers the list gives each instruction, and one line per instruction says what it moves and
//! how many bytes of each register.

use alloc::borrow::Cow;
use core::convert::Infallible;

pub mod scalable_access;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{
    FailedStoreExclusive, FaultAddressTag, LoadIntoBaseWithWriteback, LoadPairIntoOneRegister,
    MisalignedExclusive, MisalignedStackPointer, StoreAcrossFault, StoreExclusiveOverlap,
    StoreOfBaseWithWriteback,
};
use crate::branch::{is_outside_lower_addresses, to_untagged_address};
use crate::integer::{low_ones, read_source};
use crate::memory::{read_bytes_into, read_permission};
use crate::operand::{Indexing, Operand, RegisterThirtyOne, RegisterWidth, Signedness};
use crate::registers::{is_general_purpose, read_register, write_register};
use crate::{
    AccessDirection, DataAccess, Exception, ExclusiveMonitor, FaultStatus, Instruction,
    PagePermission, Processor,
};

/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires, a check Linux turns on at every level.
const STACK_POINTER_ALIGNMENT: u64 = 16;
/// The most registers one load or store names: a compare and swap pair's four.
const MAXIMUM_REGISTERS: usize = 4;
/// The most bytes one load or store moves: two quadwords.
const MAXIMUM_BYTES: usize = 2 * size_of::<u128>();
/// The bytes [FEAT_LSE2](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-4-architecture-extension#:~:text=FEAT_LSE2) lets an unaligned ordered or atomic access lie anywhere inside.
const SINGLE_COPY_ATOMIC_BYTES: u64 = 16;
/// Where the four tag bits start that a fault address need not keep.
const UNKNOWN_TAG_BITS_SHIFT: u32 = u64::BITS - u8::BITS / 2;
/// What a failed store exclusive writes to its status register.
const STORE_EXCLUSIVE_FAILED: u64 = 1;

/// The bytes of each access size.
const BYTE: u32 = size_of::<u8>() as u32;
const HALFWORD: u32 = size_of::<u16>() as u32;
const WORD: u32 = size_of::<u32>() as u32;
const DOUBLEWORD: u32 = size_of::<u64>() as u32;
const QUADWORD: u32 = size_of::<u128>() as u32;

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

/// What a load or store does in each case the manual leaves open; none where the instruction has no such case.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

fn read_addressed<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: LoadStoreOperation,
) -> Result<Addressed, Exception> {
    let mut addressed = Addressed {
        registers: [(0, RegisterWidth::Bits64); MAXIMUM_REGISTERS],
        count: 0,
        vector: false,
        base: None,
        offset: 0,
    };
    for operand in operands {
        match *operand {
            Operand::GeneralPurposeRegister { number, width, .. } => {
                addressed.registers[addressed.count] = (number.read(word), width);
                addressed.count += 1;
            }
            Operand::VectorRegister { number, .. } => {
                addressed.registers[addressed.count] = (number.read(word), RegisterWidth::Bits64);
                addressed.count += 1;
                addressed.vector = true;
            }
            Operand::GeneralPurposeRegisterPair { number, width } => {
                let first = number.read(word);
                addressed.registers[addressed.count] = (first, width);
                addressed.registers[addressed.count + 1] = (first + 1, width);
                addressed.count += 2;
            }
            Operand::AddressBase { number, indexing } => {
                addressed.base = Some((number.read(word), indexing));
            }
            Operand::AddressOffset {
                field,
                signedness,
                scale,
            } => {
                let value = match signedness {
                    Signedness::Signed if crate::planted::any(&[438, 54]) => {
                        u64::from(field.read(word))
                    }
                    Signedness::Unsigned => u64::from(field.read(word)),
                    Signedness::Signed => field.read_signed(word) as u64,
                };
                let scale = if crate::planted::is(448) {
                    scale - 1
                } else {
                    scale
                };
                let scale = match scale {
                    1 if crate::planted::is(53) => 2,
                    3 if crate::planted::is(56) => 2,
                    0 if crate::planted::is(57) => 3,
                    other => other,
                } + u32::from(crate::planted::is(152));
                let scale = if crate::planted::is(279) || crate::planted::is(280) {
                    3
                } else if crate::planted::any(&[281, 291, 294]) {
                    scale + 1
                } else {
                    scale
                };
                addressed.offset = value << scale;
            }
            Operand::SplitAddressOffset { fields, scale } => {
                let width: u32 = fields.iter().map(|field| field.width).sum();
                let bits = fields.iter().fold(0_u64, |value, field| {
                    (value << field.width) | u64::from(field.read(word))
                });
                let shift = u64::BITS - width;
                let scale = if crate::planted::is(711) {
                    scale - 1
                } else {
                    scale
                };
                addressed.offset = (((bits << shift) as i64 >> shift) as u64) << scale;
            }
            Operand::RegisterOffset {
                number,
                signed,
                doubleword,
                shift,
            } => {
                let value = read_register(
                    processor,
                    number.read(word),
                    if crate::planted::is(55) {
                        RegisterThirtyOne::StackPointer
                    } else {
                        RegisterThirtyOne::ZeroRegister
                    },
                );
                let is_set = |field: Option<crate::encoding::Field>, fixed: bool| {
                    field.map_or(fixed, |field| field.read(word) != 0)
                };
                // Where option<0> is absent the option is LSL, which takes the register whole.
                let bits = if is_set(doubleword, true)
                    || crate::planted::is(286)
                    || (crate::planted::is(718) && operation.bytes == QUADWORD)
                {
                    u64::BITS
                } else {
                    u32::BITS
                };
                let shift_bits = u64::BITS - bits;
                let extended = if (is_set(signed, false) && !crate::planted::any(&[445, 455, 37]))
                    || (crate::planted::is(285) && !is_set(doubleword, true))
                {
                    (((value << shift_bits) as i64) >> shift_bits) as u64
                } else {
                    (value << shift_bits) >> shift_bits
                };
                let amount = if shift.read(word) != 0 {
                    operation.bytes.trailing_zeros() + u32::from(crate::planted::any(&[444, 454]))
                        - u32::from(crate::planted::is(38) && operation.bytes == DOUBLEWORD)
                        - u32::from(crate::planted::is(153))
                } else {
                    0
                };
                addressed.offset = extended << amount;
            }
            Operand::ProgramCounterOffset { .. } => {
                addressed.offset = read_source(processor, word, operand)?;
            }
            Operand::UnsignedImmediate(_) => {}
            _ => panic!("{operand:?} is no operand of a load or store"),
        }
    }
    Ok(addressed)
}

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

fn constrain(
    addressed: &Addressed,
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
) -> Constrained {
    let mut writeback = matches!(
        addressed.base,
        Some((_, Indexing::PreIndex | Indexing::PostIndex))
    );
    let mut pair_order_reversed = false;
    let registers = &addressed.registers[..addressed.count];
    // A SIMD and floating-point register is never the base.
    if let (true, Some((base, _)), false) = (writeback, addressed.base, addressed.vector)
        && is_general_purpose(base)
        && registers[..if crate::planted::is(168) {
            1
        } else {
            registers.len()
        }]
            .iter()
            .any(|(number, _)| *number == base)
    {
        match operation.transfer {
            Transfer::Load(_) => match choices.load_into_base {
                Some(LoadIntoBaseWithWriteback::Undefined) => return Constrained::Undefined,
                Some(LoadIntoBaseWithWriteback::NoOperation) => return Constrained::NoOperation,
                Some(LoadIntoBaseWithWriteback::WritebackSuppressed) => writeback = false,
                Some(LoadIntoBaseWithWriteback::WritebackOverwritesLoad) | None => {}
            },
            Transfer::Store => match choices.store_of_base {
                Some(StoreOfBaseWithWriteback::Undefined) => return Constrained::Undefined,
                Some(StoreOfBaseWithWriteback::NoOperation) => return Constrained::NoOperation,
                Some(StoreOfBaseWithWriteback::StoresOriginalValue) | None => {}
            },
            Transfer::Prefetch
            | Transfer::StoreExclusive
            | Transfer::Atomic(_)
            | Transfer::CompareAndSwapPair => {}
        }
    }
    if let (Transfer::StoreExclusive, [(status, _), data @ ..]) = (operation.transfer, registers) {
        let base = addressed
            .base
            .map(|(base, _)| base)
            .filter(|base| is_general_purpose(*base));
        if data.iter().any(|(number, _)| number == status) || base == Some(*status) {
            match choices.store_exclusive_overlap {
                Some(StoreExclusiveOverlap::Undefined) => return Constrained::Undefined,
                Some(StoreExclusiveOverlap::NoOperation) => return Constrained::NoOperation,
                Some(StoreExclusiveOverlap::Runs) | None => {}
            }
        }
    }
    if let (Transfer::Load(_), [(first, _), (second, _)]) = (operation.transfer, registers)
        && first == second
    {
        match choices.load_pair_into_one_register {
            Some(LoadPairIntoOneRegister::Undefined) => return Constrained::Undefined,
            Some(LoadPairIntoOneRegister::NoOperation) => return Constrained::NoOperation,
            Some(LoadPairIntoOneRegister::KeepsFirstValue) => pair_order_reversed = true,
            Some(LoadPairIntoOneRegister::KeepsSecondValue) | None => {}
        }
    }
    Constrained::Runs {
        writeback,
        pair_order_reversed,
    }
}

fn simulate_load_store<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    operands: &[Operand],
    operation: LoadStoreOperation,
    choices: LoadStoreChoices,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let addressed = read_addressed(processor, word, operands, operation)?;
    let (writeback, pair_order_reversed) = match constrain(&addressed, operation, choices) {
        Constrained::Undefined => return Err(Exception::UndefinedInstruction),
        Constrained::NoOperation => return Ok(()),
        Constrained::Runs {
            writeback,
            pair_order_reversed,
        } => (writeback, pair_order_reversed),
    };
    let registers = &addressed.registers[..addressed.count];
    let planted_writeback = |processor: &mut Processor<LevelSpecific>, base: u64, address: u64| {
        if writeback && let Some((number, indexing)) = addressed.base {
            let written_back = match indexing {
                Indexing::PostIndex => base.wrapping_add(addressed.offset),
                Indexing::Offset | Indexing::PreIndex => address,
            };
            write_register(
                processor,
                number,
                written_back,
                RegisterThirtyOne::StackPointer,
            );
        }
    };
    let (base, address) = match addressed.base {
        None => (0, processor.program_counter.wrapping_add(addressed.offset)),
        Some((number, indexing)) => {
            let base = read_register(processor, number, RegisterThirtyOne::StackPointer);
            let checked = if crate::planted::is(60) && indexing == Indexing::PostIndex {
                base.wrapping_add(addressed.offset)
            } else {
                base
            };
            let misaligned = !is_general_purpose(number)
                && !checked.is_multiple_of(if crate::planted::is(165) {
                    8
                } else {
                    STACK_POINTER_ALIGNMENT
                });
            if (misaligned
                && choices.misaligned_stack_pointer == Some(MisalignedStackPointer::Faults)
                && !crate::planted::is(282))
                || (misaligned && crate::planted::is(288))
            {
                return Err(Exception::StackPointerAlignmentFault);
            }
            match indexing {
                Indexing::PostIndex if crate::planted::is(278) => {
                    (base, base.wrapping_add(addressed.offset))
                }
                Indexing::PostIndex => (base, base),
                Indexing::PreIndex if crate::planted::is(150) => (base, base),
                Indexing::Offset | Indexing::PreIndex => {
                    (base, base.wrapping_add(addressed.offset))
                }
            }
        }
    };
    let bytes = operation.bytes as usize;
    let total = bytes * addressed.to_data_count(operation);
    let access = match operation.transfer {
        Transfer::Store | Transfer::StoreExclusive => AccessDirection::Write,
        Transfer::Load(_)
        | Transfer::Prefetch
        | Transfer::Atomic(_)
        | Transfer::CompareAndSwapPair => AccessDirection::Read,
    };
    let granule = if crate::planted::is(265) {
        SINGLE_COPY_ATOMIC_BYTES / 2
    } else {
        SINGLE_COPY_ATOMIC_BYTES
    };
    let within_sixteen_bytes = address % granule + total as u64 <= granule;
    let aligned = crate::planted::is(276)
        || match (operation.alignment, choices.misaligned_exclusive) {
            _ if crate::planted::is(436) => true,
            (Alignment::Unchecked, _) if crate::planted::any(&[443, 456]) => within_sixteen_bytes,
            (Alignment::WithinSixteenBytes, _) if crate::planted::is(437) => {
                address % 8 + total as u64 <= 8
            }
            (Alignment::Unchecked, _) => true,
            (Alignment::WithinSixteenBytes, _) if crate::planted::is(41) => {
                address.is_multiple_of(total as u64)
            }
            (Alignment::Exclusive, Some(MisalignedExclusive::WithinSixteenBytes) | None)
                if crate::planted::is(42) =>
            {
                true
            }
            (Alignment::WithinSixteenBytes, _) if crate::planted::is(158) => {
                address.is_multiple_of(total as u64)
            }
            (Alignment::Exclusive, _) if crate::planted::is(166) => {
                address.is_multiple_of(total as u64)
            }
            (Alignment::WithinSixteenBytes, _)
            | (Alignment::Exclusive, Some(MisalignedExclusive::WithinSixteenBytes) | None) => {
                within_sixteen_bytes
            }
            (Alignment::Exclusive, Some(MisalignedExclusive::AlignmentFault)) => {
                address.is_multiple_of(total as u64)
            }
        };
    let tag = choices
        .fault_address_tag
        .unwrap_or(FaultAddressTag::KeepsTopByte);
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
    match operation.transfer {
        Transfer::Prefetch if crate::planted::is(51) => {
            if let Some((fault_address, status)) =
                find_fault(memory, location, u64::from(DOUBLEWORD), access)
            {
                return Err(Exception::DataAbort {
                    status,
                    access,
                    address: to_fault_address(address, fault_address, tag),
                });
            }
        }
        Transfer::Prefetch => {}
        Transfer::StoreExclusive => {
            if crate::planted::is(58) {
                let (status, width) = registers[0];
                write_register(
                    processor,
                    status,
                    STORE_EXCLUSIVE_FAILED & width.to_mask(),
                    RegisterThirtyOne::ZeroRegister,
                );
            }
            if crate::planted::is(267) {
                let (status, width) = registers[0];
                write_register(
                    processor,
                    status,
                    STORE_EXCLUSIVE_FAILED & width.to_mask(),
                    RegisterThirtyOne::ZeroRegister,
                );
            }
            let (status, width) = registers[0];
            let granule = choices
                .reservation_granule_bytes
                .expect("a store exclusive has a reservation granule");
            if let ExclusiveMonitor::Exclusive { address: marked } = processor.exclusive_monitor
                && marked / granule == location / granule
            {
                let mut data = [0; MAXIMUM_BYTES];
                for (index, (number, _)) in registers[1..].iter().enumerate() {
                    let value = read_register(processor, *number, RegisterThirtyOne::ZeroRegister);
                    data[index * bytes..(index + 1) * bytes]
                        .copy_from_slice(&value.to_le_bytes()[..bytes]);
                }
                store(
                    memory,
                    address,
                    location,
                    tag,
                    &data[..total],
                    StoreAcrossFault::WritesNothing,
                    mutation_hook,
                )?;
                write_register(processor, status, 0, RegisterThirtyOne::ZeroRegister);
                processor.exclusive_monitor = ExclusiveMonitor::Open;
                return Ok(());
            }
            if choices.failed_store_exclusive == Some(FailedStoreExclusive::ChecksAccess)
                && let Some((fault_address, status)) =
                    find_fault(memory, location, total as u64, access)
            {
                return Err(Exception::DataAbort {
                    status,
                    access,
                    address: to_fault_address(address, fault_address, tag),
                });
            }
            processor.exclusive_monitor = ExclusiveMonitor::Open;
            write_register(
                processor,
                status,
                if crate::planted::any(&[442, 163]) {
                    0
                } else {
                    STORE_EXCLUSIVE_FAILED
                } & width.to_mask(),
                RegisterThirtyOne::ZeroRegister,
            );
        }
        Transfer::Atomic(atomic) => {
            for direction in [AccessDirection::Read, AccessDirection::Write] {
                if let Some((fault_address, status)) =
                    find_fault(memory, location, total as u64, direction)
                {
                    return Err(Exception::DataAbort {
                        status,
                        access: direction,
                        address: to_fault_address(address, fault_address, tag),
                    });
                }
            }
            let mask = to_value_mask(operation.bytes);
            let mut read = [0; size_of::<u64>()];
            read_bytes_into(memory, location, &mut read[..bytes]);
            let old = u64::from_le_bytes(read);
            let [(first, first_width), (second, second_width), ..] = addressed.registers;
            let unmasked = read_register(processor, first, RegisterThirtyOne::ZeroRegister);
            let operand = unmasked
                & if crate::planted::is(440) {
                    u64::MAX
                } else {
                    mask
                };
            let shift = if crate::planted::is(272) {
                u64::BITS - u8::BITS
            } else {
                u64::BITS - operation.bytes * u8::BITS
            };
            let signed = |value: u64| ((value << shift) as i64) >> shift;
            let new = match atomic {
                AtomicOperation::Add if crate::planted::is(439) => Some(old),
                AtomicOperation::Add => Some(old.wrapping_add(operand) & mask),
                AtomicOperation::Clear if crate::planted::is(47) => Some(old & operand),
                AtomicOperation::Clear => Some(old & !operand),
                AtomicOperation::ExclusiveOr if crate::planted::is(270) => Some(old | operand),
                AtomicOperation::ExclusiveOr => Some(old ^ operand),
                AtomicOperation::Set => Some(old | operand),
                AtomicOperation::SignedMaximum if crate::planted::is(160) => Some(old.max(operand)),
                AtomicOperation::SignedMaximum => Some(if signed(old) >= signed(operand) {
                    old
                } else {
                    operand
                }),
                AtomicOperation::SignedMinimum if crate::planted::is(48) => Some(old.min(operand)),
                AtomicOperation::SignedMinimum => Some(if signed(old) <= signed(operand) {
                    old
                } else {
                    operand
                }),
                AtomicOperation::UnsignedMaximum => Some(old.max(operand)),
                AtomicOperation::UnsignedMinimum if crate::planted::is(273) => {
                    Some(if signed(old) <= signed(operand) {
                        old
                    } else {
                        operand
                    })
                }
                AtomicOperation::UnsignedMinimum => Some(old.min(operand)),
                AtomicOperation::Swap if crate::planted::is(300) => {
                    Some(read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask)
                }
                AtomicOperation::Swap => Some(operand),
                AtomicOperation::CompareAndSwap if crate::planted::is(44) => {
                    (old & 0xFF == operand & 0xFF).then(|| {
                        read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask
                    })
                }
                AtomicOperation::CompareAndSwap if crate::planted::is(161) => {
                    Some(read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask)
                }
                AtomicOperation::CompareAndSwap if crate::planted::is(162) => (old == unmasked)
                    .then(|| {
                        read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask
                    }),
                AtomicOperation::CompareAndSwap => (old == operand).then(|| {
                    read_register(processor, second, RegisterThirtyOne::ZeroRegister) & mask
                }),
            };
            if let Some(new) = new {
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: location,
                    bytes: Cow::Owned(new.to_le_bytes()[..bytes].to_vec()),
                });
            }
            let (target, width) = match atomic {
                AtomicOperation::CompareAndSwap => (first, first_width),
                _ if crate::planted::is(300) => (first, first_width),
                _ => (second, second_width),
            };
            let returned = if crate::planted::is(275) {
                ((old << 48) as i64 >> 48) as u64
            } else {
                match (atomic, new) {
                    (AtomicOperation::Swap, Some(new)) if crate::planted::is(49) => new,
                    (_, Some(new)) if crate::planted::is(159) => new,
                    _ => old,
                }
            };
            if !(crate::planted::is(268)
                && atomic == AtomicOperation::CompareAndSwap
                && old != operand)
            {
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
        }
        Transfer::CompareAndSwapPair => {
            for direction in [AccessDirection::Read, AccessDirection::Write] {
                if let Some((fault_address, status)) =
                    find_fault(memory, location, total as u64, direction)
                {
                    return Err(Exception::DataAbort {
                        status,
                        access: direction,
                        address: to_fault_address(address, fault_address, tag),
                    });
                }
            }
            let mask = to_value_mask(operation.bytes);
            let element_bits = operation.bytes * u8::BITS;
            let mut read = [0; size_of::<u128>()];
            read_bytes_into(memory, location, &mut read[..total]);
            let old = u128::from_le_bytes(read);
            let [(compared, width), _, (swapped, _), _] = addressed.registers;
            let read_pair = |first: u32| {
                let low = read_register(processor, first, RegisterThirtyOne::ZeroRegister) & mask;
                let high =
                    read_register(processor, first + 1, RegisterThirtyOne::ZeroRegister) & mask;
                u128::from(low) | (u128::from(high) << element_bits)
            };
            let equal = if crate::planted::is(45) {
                old as u64 & mask == read_pair(compared) as u64 & mask
            } else {
                old == read_pair(compared)
            };
            if equal {
                let new = read_pair(swapped);
                let new = if crate::planted::is(46) {
                    let low = new & ((1_u128 << element_bits) - 1);
                    (new >> element_bits) | (low << element_bits)
                } else {
                    new
                };
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: location,
                    bytes: Cow::Owned(new.to_le_bytes()[..total].to_vec()),
                });
            }
            let returned_to = if crate::planted::is(269) {
                swapped
            } else {
                compared
            };
            for (number, value) in [
                (returned_to, old as u64),
                (returned_to + 1, (old >> element_bits) as u64),
            ] {
                write_register(
                    processor,
                    number,
                    value & mask & width.to_mask(),
                    RegisterThirtyOne::ZeroRegister,
                );
            }
        }
        Transfer::Store => {
            let mut data = [0; MAXIMUM_BYTES];
            for (index, (number, _)) in registers.iter().enumerate() {
                let number = if crate::planted::is(283) {
                    &registers[0].0
                } else {
                    number
                };
                let value = if addressed.vector {
                    processor.vector_registers[*number as usize]
                } else {
                    u128::from(read_register(
                        processor,
                        *number,
                        RegisterThirtyOne::ZeroRegister,
                    ))
                };
                let value = if crate::planted::is(290) {
                    value >> 16
                } else {
                    value
                };
                let stride = if crate::planted::is(156) {
                    size_of::<u64>()
                } else {
                    bytes
                };
                if index * stride + bytes > data.len() {
                    continue;
                }
                data[index * stride..index * stride + bytes]
                    .copy_from_slice(&value.to_le_bytes()[..bytes]);
                if crate::planted::is(299) {
                    data[index * bytes..(index + 1) * bytes].reverse();
                }
            }
            let written = match operation.bytes {
                DOUBLEWORD if crate::planted::is(43) => total / 2,
                BYTE if crate::planted::is(50) => (2 * total).min(MAXIMUM_BYTES),
                _ => total,
            };
            if crate::planted::any(&[262, 284])
                && writeback
                && find_fault(memory, location, total as u64, AccessDirection::Write).is_some()
            {
                write_back(processor, &addressed, base, address);
            }
            if let Err(exception) = store(
                memory,
                address,
                location,
                tag,
                &data[..written],
                choices
                    .across_fault
                    .unwrap_or(StoreAcrossFault::WritesNothing),
                mutation_hook,
            ) {
                if crate::planted::is(449) {
                    planted_writeback(processor, base, address);
                }
                return Err(exception);
            }
        }
        Transfer::Load(signedness) => {
            let mut data = [0; MAXIMUM_BYTES];
            let pair_of_words =
                crate::planted::is(39) && registers.len() == 2 && bytes == WORD as usize;
            let fetched = if pair_of_words { 2 * total } else { total };
            if let Err(exception) = load(memory, address, location, tag, &mut data[..fetched]) {
                if crate::planted::is(434) {
                    planted_writeback(processor, base, address);
                }
                if crate::planted::any(&[261, 295]) && writeback {
                    write_back(processor, &addressed, base, address);
                }
                if crate::planted::is(149)
                    && writeback
                    && let Some((number, Indexing::PostIndex)) = addressed.base
                {
                    let written_back = base.wrapping_add(addressed.offset);
                    write_register(
                        processor,
                        number,
                        written_back,
                        RegisterThirtyOne::StackPointer,
                    );
                }
                return Err(exception);
            }
            if pair_of_words {
                data.copy_within(2 * bytes..3 * bytes, bytes);
            }
            let signedness = if crate::planted::any(&[441, 289]) {
                Signedness::Signed
            } else if crate::planted::is(296) {
                Signedness::Unsigned
            } else {
                signedness
            };
            let mut values = [0_u128; MAXIMUM_REGISTERS];
            for (index, value) in values[..registers.len()].iter_mut().enumerate() {
                let mut bytes_of = [0; size_of::<u128>()];
                let from = if crate::planted::is(266) { 0 } else { index };
                bytes_of[..bytes].copy_from_slice(&data[from * bytes..(from + 1) * bytes]);
                let loaded = u128::from_le_bytes(bytes_of);
                let shift = u128::BITS - operation.bytes * u8::BITS;
                let shift = if crate::planted::is(36) && operation.bytes == WORD {
                    u128::BITS - u16::BITS
                } else {
                    shift
                };
                let signedness =
                    if crate::planted::any(&[52, 151]) || (crate::planted::is(155) && index == 1) {
                        Signedness::Unsigned
                    } else {
                        signedness
                    };
                *value = match signedness {
                    Signedness::Unsigned => loaded,
                    Signedness::Signed => (((loaded << shift) as i128) >> shift) as u128,
                };
            }
            if crate::planted::is(450) {
                values[1] = values[0];
            }
            if crate::planted::is(457) {
                values.swap(0, 1);
            }
            if crate::planted::is(292) {
                values.swap(0, 1);
            }
            let order: &[usize] = if pair_order_reversed {
                &[1, 0]
            } else {
                &[0, 1]
            };
            for &index in order.iter().filter(|index| **index < registers.len()) {
                let (number, width) = registers[index];
                if addressed.vector {
                    processor.vector_registers[number as usize] = if crate::planted::is(451) {
                        let mask = u128::MAX >> (u128::BITS - operation.bytes * u8::BITS);
                        (processor.vector_registers[number as usize] & !mask) | values[index]
                    } else {
                        values[index]
                    };
                    continue;
                }
                let mut value = values[index] as u64 & width.to_mask();
                if crate::planted::any(&[277, 297]) {
                    value = values[index] as u64;
                }
                if crate::planted::is(293) && width == RegisterWidth::Bits32 {
                    value |= read_register(processor, number, RegisterThirtyOne::ZeroRegister)
                        & !width.to_mask();
                }
                write_register(processor, number, value, RegisterThirtyOne::ZeroRegister);
            }
            if operation.alignment == Alignment::Exclusive {
                processor.exclusive_monitor = ExclusiveMonitor::Exclusive { address: location };
            }
        }
    }
    if writeback
        && !crate::planted::is(154)
        && let Some((number, indexing)) = addressed.base
    {
        let written_back = match indexing {
            Indexing::PostIndex => base.wrapping_add(addressed.offset),
            Indexing::Offset | Indexing::PreIndex => address,
        };
        let written_back = if crate::planted::is(34) {
            to_untagged_address(written_back)
        } else {
            written_back
        };
        let written_back = if crate::planted::any(&[435, 453]) {
            written_back & low_ones(56)
        } else {
            written_back
        };
        let written_back = if crate::planted::is(263) {
            to_untagged_address(written_back)
        } else {
            written_back
        };
        write_register(
            processor,
            number,
            written_back,
            RegisterThirtyOne::StackPointer,
        );
    }
    Ok(())
}

/// The planted bugs' writeback on a fault.
fn write_back<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    addressed: &Addressed,
    base: u64,
    address: u64,
) {
    if let Some((number, indexing)) = addressed.base {
        let written_back = match indexing {
            Indexing::PostIndex => base.wrapping_add(addressed.offset),
            Indexing::Offset | Indexing::PreIndex => address,
        };
        write_register(
            processor,
            number,
            written_back,
            RegisterThirtyOne::StackPointer,
        );
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
) -> Result<(), Exception> {
    let access = AccessDirection::Write;
    if let Some((fault_location, status)) = find_fault(memory, location, bytes.len() as u64, access)
    {
        let below_fault = &bytes[..fault_location.wrapping_sub(location) as usize];
        if (across_fault == StoreAcrossFault::WritesBytesBelowFault || crate::planted::is(40))
            && !below_fault.is_empty()
        {
            mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                start: location,
                bytes: Cow::Borrowed(below_fault),
            });
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
    Ok(())
}

pub(crate) fn load<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
    location: u64,
    tag: FaultAddressTag,
    bytes: &mut [u8],
) -> Result<(), Exception> {
    let access = AccessDirection::Read;
    match find_fault(memory, location, bytes.len() as u64, access) {
        None => {
            read_bytes_into(memory, location, bytes);
            Ok(())
        }
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
) -> Option<(u64, FaultStatus)> {
    (0..length)
        .map(|offset| start.wrapping_add(offset))
        .find_map(|address| {
            if is_outside_lower_addresses(address) {
                return Some((address, FaultStatus::TranslationFaultLevel0));
            }
            let status = match read_permission(memory, address) {
                PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3,
                PagePermission::Mapped { data_access, .. } => match (data_access, access) {
                    (DataAccess::ReadWrite, _) | (DataAccess::ReadOnly, AccessDirection::Read) => {
                        return None;
                    }
                    (DataAccess::ReadOnly, AccessDirection::Write) | (DataAccess::Forbidden, _) => {
                        FaultStatus::PermissionFaultLevel3
                    }
                },
            };
            Some((address, status))
        })
}

/// The fault address FAR_EL1 gives for a fault at `fault_location` of an access to `address`, with the tag the
/// behavior keeps.
pub(crate) fn to_fault_address(address: u64, fault_location: u64, tag: FaultAddressTag) -> u64 {
    let location = to_untagged_address(fault_location);
    let tag_bits = if crate::planted::any(&[264, 35]) {
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

/// Ones in the bits of an access of this many bytes.
const fn to_value_mask(bytes: u32) -> u64 {
    low_ones(bytes * u8::BITS)
}
