//! The formula of every behavior.
//!
//! Each formula decodes the instruction at the instruction pointer with iced-x86 and reads its operands from it. It
//! changes the processor only when it returns `Ok`; x86 faults are precise.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::Infallible;

use iced_x86::{Code, Decoder, DecoderError, DecoderOptions, EncodingKind, Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator};

use crate::behavior::{CpuidReading, IntegerBehavior, StoreAcrossFault};
use crate::branch::{Decision, find_branch_form};
use crate::integer::{BitChange, Condition, OperandPlace, Operation, find_integer_form, sign_extend, to_mask};
use crate::memory::{read_bytes, read_permission};
use crate::planted::{Bug, is_active};
use crate::float::{ORDER_FLAGS, find_float_form};
use crate::vector::{VectorEncoding, VectorForm, find_vector_form};
use crate::{
    Access, DataAccess, Exception, Flags, Instruction, InstructionFetch, PRIVILEGED_FORMS, PagePermission,
    PreciseInstruction, Processor, ProcessorExpectation, STACK_POINTER, VIRTUAL_ADDRESS_BITS, X86_64,
};

const MAXIMUM_INSTRUCTION_BYTES: usize = 15;
/// The byte 64-bit mode leaves undefined that XED names UDB.
const UNDEFINED_SINGLE_BYTE: u8 = 0xD6;
/// The legacy prefixes: segment overrides, operand and address size, LOCK, REPNE and REP.
const LEGACY_PREFIXES: [u8; 11] = [0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3];
/// The legacy prefixes that never change an instruction's length: the segment overrides and LOCK.
const LENGTH_NEUTRAL_PREFIXES: [u8; 7] = [0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0xF0];
/// The first bytes of the two-byte and three-byte VEX prefixes and of EVEX, which 64-bit mode always reads so.
const VECTOR_PREFIXES: [u8; 3] = [0xC5, 0xC4, 0x62];
/// The REX prefixes, 0x40 to 0x4F: the high nibble 4 and the W, R, X and B bits.
const REX_NIBBLE: u8 = 0x40;
const BITNESS: u32 = 64;
/// Legacy SSE's memory operands of 16 bytes must be aligned to 16, or the instruction raises #GP(0).
const SSE_ALIGNMENT: u64 = 16;

type Memory<'list, 'memory> = &'list [MemoryDelta<'memory, X86_64>];

#[path = "extension_simulation.rs"]
mod extension_simulation;
#[path = "fixed_simulation.rs"]
mod fixed_simulation;
#[path = "mmx_simulation.rs"]
mod mmx_simulation;
#[path = "x87_simulation.rs"]
mod x87_simulation;

/// A memory operand's first address, and whether its segment is SS, which turns #GP into #SS.
#[derive(Clone, Copy)]
struct MemoryOperand {
    address: u64,
    stack: bool,
}

impl Simulator<X86_64> for PreciseInstruction {
    type Error = Infallible;

    /// Decodes the instruction at the instruction pointer and applies this behavior to it; every register's result
    /// is one the state before decides.
    fn simulate(
        &self,
        processor: &Processor,
        memory: MemorySnapshot<'_, '_, X86_64>,
        mutation_hook: &mut impl MemorySink<X86_64>,
    ) -> Result<ProcessorExpectation, Infallible> {
        let mut processor = processor.clone();
        let before = processor.instruction_pointer;
        simulate_processor(*self, &mut processor, memory.mutations, mutation_hook);
        let mut expectation = processor.to_expectation();
        let undecided = matches!(self, PreciseInstruction::Fixed(instruction)
            if crate::fixed::find_fixed_form(*instruction).is_some_and(|form| form.outcome == crate::fixed::FixedOutcome::WritesUndecided));
        if undecided && processor.exception.is_none() {
            let decoded = fetch(memory.mutations, before).expect("a completed instruction was fetched");
            for number in crate::fixed::list_undecided_registers(&decoded) {
                expectation.general_purpose_registers[number] = crate::RegisterValueExpectation::NonDeterministic;
            }
        }
        Ok(expectation)
    }
}

/// Applies the behavior to the processor, writing memory changes through the hook.
fn simulate_processor(
    behavior: PreciseInstruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    mutation_hook: &mut impl MemorySink<X86_64>,
) {
    let decoded = match fetch(memory, processor.instruction_pointer) {
        Ok(decoded) => decoded,
        Err(exception) => {
            processor.exception = Some(exception);
            return;
        }
    };
    let mut changed = processor.clone();
    let on_memory_mutated = |mutation: MemoryDelta<'_, X86_64>| mutation_hook.on_memory_mutated(mutation);
    match apply(behavior, &mut changed, memory, &decoded, on_memory_mutated) {
        Ok(()) => {
            *processor = changed;
            // A jump leaves the instruction pointer where it goes.
            if !matches!(behavior, PreciseInstruction::Branch(_)) {
                processor.instruction_pointer = decoded.next_ip();
            }
            processor.exception = None;
        }
        Err(exception) => {
            keep_planted_writes(behavior, exception, processor, &changed);
            // #XM leaves the flags of the exceptions it found in MXCSR.
            if exception == Exception::SimdFloatingPoint
                && !(is_active(Bug::SubtractScalarSingleFaultLeavesFlags)
                    && behavior == PreciseInstruction::Float(Instruction::SubssXmmssMemss))
            {
                processor.mxcsr = changed.mxcsr;
            }
            // An MMX store that faults leaves the TOP it cleared first.
            if matches!(behavior, PreciseInstruction::Mmx { .. }) {
                processor.x87.status = changed.x87.status;
            }
            if exception.is_trap() {
                processor.instruction_pointer = decoded.next_ip();
            }
            processor.exception = Some(exception);
        }
    }
}

/// What the planted bugs that write despite a fault keep of the instruction's writes.
fn keep_planted_writes(behavior: PreciseInstruction, exception: Exception, processor: &mut Processor, changed: &Processor) {
    let exchange_add = match behavior {
        PreciseInstruction::Integer { instruction, .. } => {
            find_integer_form(instruction).is_some_and(|form| form.operation == Operation::ExchangeAdd)
        }
        _ => false,
    };
    let push = matches!(behavior, PreciseInstruction::PushGprv50 { .. });
    if is_active(Bug::FaultKeepsRegisterWrites) {
        *processor = changed.clone();
    }
    if (is_active(Bug::ExchangeAddWritesDespiteFault) && exchange_add)
        || (is_active(Bug::PushLowersStackPointerDespiteFault) && push)
    {
        processor.general_purpose_registers = changed.general_purpose_registers;
    }
    if is_active(Bug::ShiftStoreFlagsDespiteFault) {
        processor.flags = changed.flags;
    }
    let exchange = matches!(behavior, PreciseInstruction::Integer { instruction, .. }
        if find_integer_form(instruction).is_some_and(|form| form.operation == Operation::Exchange));
    if is_active(Bug::ExchangeRegisterWrittenDespiteFault) && exchange {
        processor.general_purpose_registers = changed.general_purpose_registers;
    }
    if is_active(Bug::MultiplyPackedDoubleWritesDespiteFault)
        && behavior == PreciseInstruction::Float(Instruction::MulpdXmmpdXmmpd)
        && exception == Exception::SimdFloatingPoint
    {
        processor.vector_registers = changed.vector_registers;
    }
}

fn apply(
    behavior: PreciseInstruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    match behavior {
        PreciseInstruction::Integer { instruction, behavior } => {
            execute_integer(instruction, behavior, processor, memory, decoded, on_memory_mutated)
        }
        PreciseInstruction::PushGprv50 { across_fault } => {
            expect_code(decoded, &[Code::Push_r16, Code::Push_r64]);
            push(processor, memory, decoded, across_fault, on_memory_mutated)
        }
        PreciseInstruction::Ud2 => {
            expect_code(decoded, &[Code::Ud2]);
            Err(Exception::InvalidOpcode)
        }
        PreciseInstruction::Undefined(_) if is_active(Bug::UndefinedRaisesGeneralProtection) => {
            Err(Exception::GeneralProtection { error_code: 0 })
        }
        PreciseInstruction::Undefined(_) => Err(Exception::InvalidOpcode),
        PreciseInstruction::Int3 => {
            expect_code(decoded, &[Code::Int3]);
            Err(Exception::Breakpoint)
        }
        PreciseInstruction::Cpuid(reading) => {
            expect_code(decoded, &[Code::Cpuid]);
            match reading {
                CpuidReading::Faults => Err(Exception::GeneralProtection { error_code: 0 }),
                CpuidReading::Reads(table) => {
                    let registers = &mut processor.general_purpose_registers;
                    let [eax, ebx, ecx, edx] = table.read(registers[0] as u32, registers[1] as u32);
                    // EAX, ECX, EDX and EBX are registers 0 to 3.
                    for (number, value) in [(0, eax), (3, ebx), (1, ecx), (2, edx)] {
                        registers[number] = u64::from(value);
                    }
                    Ok(())
                }
            }
        }
        PreciseInstruction::Fixed(instruction) => fixed_simulation::execute_fixed(instruction, processor, memory, decoded),
        PreciseInstruction::X87 { instruction, behavior } => {
            x87_simulation::execute_x87(instruction, behavior, processor, memory, decoded, on_memory_mutated)
        }
        PreciseInstruction::Mmx { instruction, behavior } => {
            mmx_simulation::execute_mmx(instruction, behavior, processor, memory, decoded, on_memory_mutated)
        }
        PreciseInstruction::Extension { instruction, behavior } => {
            extension_simulation::execute_extension(instruction, behavior, processor, memory, decoded, on_memory_mutated)
        }
        PreciseInstruction::Vector(instruction) => execute_vector(instruction, processor, memory, decoded),
        PreciseInstruction::Float(instruction) => execute_float(instruction, processor, memory, decoded),
        PreciseInstruction::Branch(instruction) => execute_branch(instruction, processor, decoded),
        PreciseInstruction::Privileged(instruction) => {
            let (_, codes) = PRIVILEGED_FORMS
                .iter()
                .find(|(form, _)| *form == instruction)
                .unwrap_or_else(|| panic!("{instruction:?} is no privileged form"));
            expect_code(decoded, codes);
            if is_active(Bug::PrivilegedFormsComplete) {
                return Ok(());
            }
            if is_active(Bug::ReadModelSpecificRegisterCompletes) && instruction == Instruction::Rdmsr {
                return Ok(());
            }
            if (is_active(Bug::HaltInvalidOpcode) && instruction == Instruction::Hlt)
                || (is_active(Bug::SwapgsInvalidOpcode) && instruction == Instruction::Swapgs)
                || (is_active(Bug::ReadPerformanceCounterInvalidOpcode) && instruction == Instruction::Rdpmc)
            {
                return Err(Exception::InvalidOpcode);
            }
            Err(Exception::GeneralProtection { error_code: 0 })
        }
    }
}

fn expect_code(decoded: &Decoded, codes: &[Code]) {
    assert!(
        codes.contains(&decoded.code()),
        "the next instruction is one of {codes:?}, not {:?}",
        decoded.code()
    );
}

/// Decodes the instruction at the address, or faults where the chip does: at the first byte it needs that cannot be
/// fetched (#GP(0) for a non-canonical byte, #PF for a page that refuses it), or with #GP(0) once 15 bytes are
/// fetched of an instruction longer than that, or with #UD once an undefined one, or one with a VEX, EVEX or XOP
/// prefix, is fetched as far as it can be measured. A byte that cannot be fetched is never read as a value.
fn fetch(memory: Memory<'_, '_>, address: u64) -> Result<Decoded, Exception> {
    let fetchable = (0..MAXIMUM_INSTRUCTION_BYTES as u64)
        .take_while(|offset| check_fetch(memory, address.wrapping_add(*offset)).is_ok())
        .count();
    let bytes: [u8; MAXIMUM_INSTRUCTION_BYTES] = read_bytes(memory, address);
    if is_active(Bug::MissingBytesReadAsZeros) {
        let decoded = Decoder::with_ip(BITNESS, &bytes, address, DecoderOptions::NONE).decode();
        if decoded.is_invalid() && !matches!(measure_length(&bytes), Length::Bytes(16) | Length::BeyondAvailable) {
            check_fetch(memory, address)?;
            return Err(Exception::InvalidOpcode);
        }
    }
    let available = &bytes[..fetchable];
    let needed = match measure_length(available) {
        Length::Bytes(length) => length.min(MAXIMUM_INSTRUCTION_BYTES),
        Length::BeyondAvailable => fetchable + 1,
        Length::Undefined => 1,
    };
    if needed > fetchable {
        // The byte the decoder needs next is the first that cannot be fetched, or the 16th.
        if fetchable == MAXIMUM_INSTRUCTION_BYTES {
            if is_active(Bug::SixteenBytesInvalidOpcode) {
                return Err(Exception::InvalidOpcode);
            }
            return Err(Exception::GeneralProtection { error_code: 0 });
        }
        return Err(check_fetch(memory, address.wrapping_add(fetchable as u64)).expect_err("a refused byte"));
    }
    let too_long = if is_active(Bug::FifteenBytesTooLong) { MAXIMUM_INSTRUCTION_BYTES } else { MAXIMUM_INSTRUCTION_BYTES + 1 };
    if matches!(measure_length(available), Length::Bytes(length) if length >= too_long) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    let decoded = Decoder::with_ip(BITNESS, &available[..needed], address, DecoderOptions::NONE).decode();
    // XOP is AMD's encoding: on Intel its bytes raise #UD once they are measured.
    if decoded.is_invalid() || decoded.encoding() == EncodingKind::XOP {
        return Err(Exception::InvalidOpcode);
    }
    Ok(decoded)
}

/// Faults as fetching the byte at the address does: #GP(0) where it is not canonical, #PF where its page refuses it.
fn check_fetch(memory: Memory<'_, '_>, address: u64) -> Result<(), Exception> {
    if !is_canonical(address) && !is_active(Bug::FetchCanonicalUnchecked) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    match find_fault(memory, address, 1, Access::InstructionFetch) {
        None => Ok(()),
        Some((address, page_present)) => Err(Exception::PageFault {
            access: if is_active(Bug::FetchFaultReportedAsRead) { Access::Read } else { Access::InstructionFetch },
            page_present,
            address,
        }),
    }
}

/// An instruction's length as the chip measures it before it decodes the instruction's meaning.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Length {
    /// Its length, or 16 for any length beyond 15.
    Bytes(usize),
    /// Its length needs a byte past those available.
    BeyondAvailable,
    /// Its bytes are no instruction whatever follows.
    Undefined,
}

/// Measures the instruction the bytes start with, counting its bytes here rather than trusting a decoder that stops
/// at 15. The prefix bytes are legacy prefixes and REX bytes in any order; each is counted, and iced-x86 measures
/// what is left once those that never change a length are set aside: the segment overrides and LOCK; every REX but
/// one right before the opcode, the only place a REX counts; and before a VEX or EVEX prefix every prefix, which
/// makes the instruction #UD but leaves its length. The other legacy prefixes are kept once each in the order of
/// their last use.
fn measure_length(bytes: &[u8]) -> Length {
    let rex_counted = !is_active(Bug::LengthCountsLegacyPrefixesOnly);
    let count = bytes.iter().take_while(|byte| LEGACY_PREFIXES.contains(byte) || (rex_counted && is_rex(**byte))).count();
    // Every instruction has an opcode byte after its prefixes.
    if count >= MAXIMUM_INSTRUCTION_BYTES {
        return Length::Bytes(MAXIMUM_INSTRUCTION_BYTES + 1);
    }
    if count == bytes.len() {
        return Length::BeyondAvailable;
    }
    // The chip measures 0xD6 as one byte, where iced-x86 would read a ModRM byte after it.
    if bytes[count] == UNDEFINED_SINGLE_BYTE {
        return Length::Bytes(count + 1);
    }
    let vector = VECTOR_PREFIXES.contains(&bytes[count]);
    let mut kept: Vec<u8> = Vec::new();
    for &prefix in &bytes[..count] {
        if vector || is_rex(prefix) || LENGTH_NEUTRAL_PREFIXES.contains(&prefix) {
            continue;
        }
        kept.retain(|&other| other != prefix);
        kept.push(prefix);
    }
    if let Some(&last) = bytes[..count].last().filter(|last| is_rex(**last) && !vector) {
        kept.push(last);
    }
    let set_aside = count - kept.len();
    kept.extend_from_slice(&bytes[count..]);
    let mut decoder = Decoder::new(BITNESS, &kept, DecoderOptions::NONE);
    let decoded = decoder.decode();
    if decoded.is_invalid() {
        return if decoder.last_error() == DecoderError::NoMoreBytes {
            Length::BeyondAvailable
        } else {
            Length::Undefined
        };
    }
    Length::Bytes((decoded.len() + set_aside).min(MAXIMUM_INSTRUCTION_BYTES + 1))
}

const fn is_rex(byte: u8) -> bool {
    byte & 0xF0 == REX_NIBBLE
}

/// Finds the lowest address of the access whose page refuses it, and whether that page is present. A canonical
/// address outside the program's addresses is the kernel's: present, and refused.
fn find_fault(
    memory: Memory<'_, '_>,
    start: u64,
    length: u64,
    access: Access,
) -> Option<(u64, bool)> {
    (0..length)
        .map(|offset| start.wrapping_add(offset))
        .find_map(|address| {
            if !X86_64::ADDRESSABLE.contains(&address) {
                return Some((address, !is_active(Bug::KernelHalfNotPresent)));
            }
            match read_permission(memory, address) {
                PagePermission::Unmapped => Some((address, false)),
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => {
                    let allowed = match access {
                        Access::Read => true,
                        Access::Write => data_access == DataAccess::ReadWrite,
                        Access::InstructionFetch => instruction_fetch == InstructionFetch::Allowed,
                    };
                    let present = !(is_active(Bug::ReadOnlyWriteReportedNotPresent) && access == Access::Write);
                    (!allowed).then_some((address, present))
                }
            }
        })
}

const fn is_canonical(address: u64) -> bool {
    let unused = u64::BITS - VIRTUAL_ADDRESS_BITS;
    (((address << unused) as i64) >> unused) as u64 == address
}

/// Where the memory operand lies: base plus scaled index plus displacement, or the next instruction's address plus
/// the displacement, summed in 64 bits or, under the 0x67 prefix, in 32 bits from the registers' low halves and
/// wrapped at 4 GiB; then plus the FS or GS base under those overrides. A non-canonical address raises #SS when the
/// base is RSP or RBP and no FS or GS override applies; an SS override with another base raises #GP, as the chip
/// does.
fn locate(processor: &Processor, decoded: &Decoded) -> MemoryOperand {
    locate_moved(processor, decoded, 0)
}

/// Where the memory operand lies, as [`locate`] gives it, moved by `bytes` within the address arithmetic, as a bit
/// test's register offset moves it.
fn locate_moved(processor: &Processor, decoded: &Decoded, bytes: u64) -> MemoryOperand {
    let base = decoded.memory_base();
    let mut address = compute_effective_address(processor, decoded, bytes);
    let ignored = is_active(Bug::SegmentBasesIgnored);
    match decoded.memory_segment() {
        Register::FS if ignored || is_active(Bug::FsBaseIgnored) => {}
        Register::FS if is_active(Bug::FsOverrideAddsGsBase) => address = address.wrapping_add(processor.gs_base),
        Register::FS => address = address.wrapping_add(processor.fs_base),
        Register::GS if ignored => {}
        Register::GS => address = address.wrapping_add(processor.gs_base),
        _ => {}
    }
    MemoryOperand {
        address,
        stack: (base.full_register() == Register::RSP
            || (base.full_register() == Register::RBP && !is_active(Bug::StackSegmentOnlyForRsp)))
            && (is_active(Bug::StackSegmentDespiteFsOrGs) || !matches!(decoded.segment_prefix(), Register::FS | Register::GS)),
    }
}

/// The memory operand's address before any segment base, as LEA gives it, moved by `bytes`.
fn compute_effective_address(processor: &Processor, decoded: &Decoded, bytes: u64) -> u64 {
    let base = decoded.memory_base();
    let index = decoded.memory_index();
    // iced-x86 gives an address relative to RIP or EIP already summed, wrapped to 32 bits for EIP.
    let mut address = decoded.memory_displacement64().wrapping_add(bytes);
    let eip = base == Register::EIP;
    if is_active(Bug::InstructionPointerRelativeNotWrapped) && eip {
        let raw = i64::from(decoded.memory_displacement32().wrapping_sub(decoded.next_ip() as u32) as i32);
        address = decoded.next_ip().wrapping_add(raw as u64).wrapping_add(bytes);
    }
    if is_active(Bug::DisplacementAloneZeroExtended)
        && base == Register::None
        && index == Register::None
        && decoded.memory_displ_size() == 8
    {
        address = u64::from(decoded.memory_displacement64() as u32).wrapping_add(bytes);
    }
    if is_active(Bug::DisplacementZeroExtended) && decoded.memory_displ_size() >= 4 && !matches!(base, Register::RIP | Register::EIP) {
        address &= u64::from(u32::MAX);
    }
    let whole_registers = is_active(Bug::AddressSizePrefixIgnored);
    let read_address_register = |register: Register| {
        if whole_registers {
            processor.general_purpose_registers[register.full_register().number()]
        } else {
            read_register(processor, register)
        }
    };
    let adds_eip = is_active(Bug::InstructionPointerRelativeAddsRegister) && eip;
    if !matches!(base, Register::None | Register::RIP | Register::EIP) || adds_eip {
        address = address.wrapping_add(read_address_register(base));
    }
    if index != Register::None {
        let scale = if is_active(Bug::IndexScaleIgnored) { 1 } else { u64::from(decoded.memory_index_scale()) };
        address = address.wrapping_add(read_address_register(index).wrapping_mul(scale));
    }
    // 32-bit addressing wraps at 4 GiB, also with a displacement alone (iced-x86 counts its size as 4 there, as 8 in
    // 64-bit addressing), which a bit test's offset moves.
    let narrow = base.size() == size_of::<u32>()
        || index.size() == size_of::<u32>()
        || (base == Register::None && index == Register::None && decoded.memory_displ_size() == 4);
    let unwrapped = whole_registers
        || (is_active(Bug::InstructionPointerRelativeNotWrapped) && eip)
        || (is_active(Bug::LoadEffectiveAddressNotWrapped) && decoded.mnemonic() == iced_x86::Mnemonic::Lea);
    if narrow && !unwrapped {
        address &= u64::from(u32::MAX);
    }
    address
}

/// Checks, in the chip's order, that the access's first byte is canonical, that the access is aligned where AC asks
/// for it (before the rest of it is checked: a misaligned access across the canonical edge raises #AC), that every
/// byte of it is canonical, and that its pages allow it.
fn check_access(
    flags: Flags,
    memory: Memory<'_, '_>,
    operand: MemoryOperand,
    length: u64,
    access: Access,
) -> Result<(), Exception> {
    if !is_active(Bug::FirstByteCanonicalUnchecked) {
        check_canonical(operand, 1)?;
    }
    if is_active(Bug::CanonicalBeforeAlignment) {
        check_canonical(operand, length)?;
    }
    if !(is_active(Bug::ReadWithoutAlignmentCheck) && access == Access::Read) {
        check_alignment(flags, operand, length)?;
    }
    check_canonical(operand, length)?;
    match find_fault(memory, operand.address, length, access) {
        None => Ok(()),
        Some((address, page_present)) => Err(Exception::PageFault {
            access,
            page_present: page_present || (is_active(Bug::ReadNotPresentReportedPresent) && access == Access::Read),
            address: if is_active(Bug::ReadFaultAtAccessStart) && access == Access::Read { operand.address } else { address },
        }),
    }
}

fn check_canonical(operand: MemoryOperand, length: u64) -> Result<(), Exception> {
    let last = operand.address.wrapping_add(length - 1);
    if is_canonical(operand.address) && is_canonical(last) {
        return Ok(());
    }
    Err(if operand.stack {
        Exception::StackSegmentFault { error_code: 0 }
    } else {
        Exception::GeneralProtection { error_code: 0 }
    })
}

/// #AC(0) for an access of 2, 4 or 8 bytes not aligned to its size, while AC is set (CR0.AM is, at ring 3); the chip
/// checks no 16-byte access, as MOVUPS from an odd address shows.
fn check_alignment(flags: Flags, operand: MemoryOperand, length: u64) -> Result<(), Exception> {
    let smallest = if is_active(Bug::AlignmentCheckIgnoresTwoBytes) { 2 } else { 1 };
    if flags.contains(Flags::ALIGNMENT_CHECK)
        && !is_active(Bug::AlignmentCheckNever)
        && length > smallest
        && length <= size_of::<u64>() as u64
        && !operand.address.is_multiple_of(if is_active(Bug::AlignmentCheckEightBytesAtFour) && length == 8 { 4 } else { length })
    {
        return Err(Exception::AlignmentCheck);
    }
    Ok(())
}

/// Reads up to eight bytes, little-endian.
fn read_value(memory: Memory<'_, '_>, address: u64, length: usize) -> u64 {
    let bytes: [u8; size_of::<u64>()] = read_bytes(memory, address);
    u64::from_le_bytes(bytes) & to_mask(length)
}

/// Writes the bytes at the operand, or, where a byte faults, what the behavior across a fault writes.
fn store(
    flags: Flags,
    memory: Memory<'_, '_>,
    operand: MemoryOperand,
    bytes: &[u8],
    across_fault: StoreAcrossFault,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let address = operand.address;
    if !is_active(Bug::FirstByteCanonicalUnchecked) {
        check_canonical(operand, 1)?;
    }
    check_alignment(flags, operand, bytes.len() as u64)?;
    check_canonical(operand, bytes.len() as u64)?;
    if let Some((fault_address, page_present)) =
        find_fault(memory, address, bytes.len() as u64, Access::Write)
    {
        let below_fault = &bytes[..fault_address.wrapping_sub(address) as usize];
        let writes_below = across_fault == StoreAcrossFault::WritesBytesBelowFault || is_active(Bug::StoreWritesBytesBelowFault);
        if is_active(Bug::StoreFaultWritesAllBytes) {
            on_memory_mutated(MemoryDelta::ValueAssignment {
                start: address,
                bytes: Cow::Borrowed(bytes),
            });
        } else if writes_below && !below_fault.is_empty() {
            on_memory_mutated(MemoryDelta::ValueAssignment {
                start: address,
                bytes: Cow::Borrowed(below_fault),
            });
        }
        return Err(Exception::PageFault {
            access: Access::Write,
            page_present: page_present && !is_active(Bug::StoreReadOnlyReportedNotPresent),
            address: if is_active(Bug::StoreFaultAtAccessStart) { address } else { fault_address },
        });
    }
    on_memory_mutated(MemoryDelta::ValueAssignment {
        start: address,
        bytes: Cow::Borrowed(bytes),
    });
    Ok(())
}

/// The integer skeleton: reads the destination (a register or memory) and the source (a register, memory, an
/// immediate or nothing), applies the family's operation, and writes the destination back. A destination in memory
/// that the operation writes is checked as a write before anything is read.
fn execute_integer(
    instruction: Instruction,
    behavior: IntegerBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_integer_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no integer form"));
    expect_code(decoded, form.codes);
    assert!(
        (decoded.op0_kind() == OpKind::Memory || decoded.op1_kind() == OpKind::Memory)
            == (form.place != OperandPlace::Register),
        "{instruction:?} has its operand in {:?}",
        form.place
    );
    let operation = form.operation;
    if operation == Operation::RaiseInvalidOpcode {
        return Err(Exception::InvalidOpcode);
    }
    // A form without operands (CBW, LAHF) is as wide as its code's operand size, or one byte.
    let length = match (decoded.op_count(), decoded.op0_kind()) {
        (0, _) => (decoded.code().op_code().operand_size() as usize / 8).max(1),
        (_, OpKind::Register) => decoded.op0_register().size(),
        _ => decoded.memory_size().size(),
    };
    // A bit test's register offset moves the access by its whole operands, signed, within the address arithmetic.
    let moved = match (operation, decoded.op_count() > 1 && decoded.op1_kind() == OpKind::Register) {
        (Operation::BitTest(change), true) => {
            let unsigned = read_register(processor, decoded.op1_register()) & to_mask(length);
            let offset = sign_extend(unsigned, length) as i64;
            if is_active(Bug::BitResetOffsetUnsigned) && change == BitChange::Reset {
                (unsigned >> (length * 8).trailing_zeros()).wrapping_mul(length as u64)
            } else if is_active(Bug::BitTestOffsetMovesInBytes) && change == BitChange::Keep {
                (offset >> 3) as u64
            } else {
                (offset >> (length * 8).trailing_zeros()).wrapping_mul(length as i64) as u64
            }
        }
        _ => 0,
    };
    let reads_nothing = is_active(Bug::NoOperationReadsMemory) && operation == Operation::NoOperation;
    let destination_operand = (decoded.op_count() > 0 && decoded.op0_kind() == OpKind::Memory)
        .then(|| locate_moved(processor, decoded, moved))
        .filter(|_| operation.accesses_memory() || reads_nothing);
    if let Some(operand) = destination_operand {
        let access = if operation.writes_destination() { Access::Write } else { Access::Read };
        check_access(processor.flags, memory, operand, length as u64, access)
            .or_else(|exception| match exception {
                // A write that faults part-way is the store's to report, with what it writes below the fault.
                Exception::PageFault { .. } if operation.writes_destination() => Ok(()),
                other => Err(other),
            })?;
    }
    // An operand's value and width: a register, memory, as wide as the access (MOVZX reads fewer bytes than it
    // writes), or an immediate, as wide as the destination.
    let read_operand = |processor: &Processor, index: u32| -> Result<(u64, usize), Exception> {
        match decoded.op_kind(index) {
            OpKind::Register => {
                let register = decoded.op_register(index);
                let high_as_low = is_active(Bug::AddWithCarryHighByteReadAsLow)
                    && operation == Operation::AddWithCarry
                    && matches!(register, Register::AH | Register::CH | Register::DH | Register::BH);
                let value = if high_as_low {
                    processor.general_purpose_registers[register.full_register().number()] & 0xFF
                } else {
                    read_register(processor, register)
                };
                Ok((value, register.size()))
            }
            OpKind::Memory if operation == Operation::LoadEffectiveAddress => {
                if is_active(Bug::LoadEffectiveAddressAddsSegmentBase) {
                    return Ok((locate(processor, decoded).address, length));
                }
                Ok((compute_effective_address(processor, decoded, 0), length))
            }
            OpKind::Memory => {
                let operand = locate(processor, decoded);
                let width = decoded.memory_size().size();
                let skipped = is_active(Bug::MoveIfFalseSkipsRead)
                    && matches!(operation, Operation::MoveIf(condition) if !condition.holds(processor.flags));
                if !skipped {
                    check_access(processor.flags, memory, operand, width as u64, Access::Read)?;
                }
                let short = is_active(Bug::MoveQuadwordLoadReadsFourBytes) && operation == Operation::Move && width == 8;
                Ok((read_value(memory, operand.address, if short { 4 } else { width }), width))
            }
            kind => {
                let mut immediate = decoded.immediate(index);
                let zero_extended = (is_active(Bug::SubtractImmediateZeroExtended) && operation == Operation::Subtract)
                    || (is_active(Bug::MoveImmediateZeroExtended) && operation == Operation::Move);
                if zero_extended && kind == OpKind::Immediate32to64 {
                    immediate &= u64::from(u32::MAX);
                }
                let byte = matches!(kind, OpKind::Immediate8to16 | OpKind::Immediate8to32 | OpKind::Immediate8to64);
                if (is_active(Bug::CompareImmediateByteNotSignExtended) && operation == Operation::Compare && byte)
                    || (is_active(Bug::MultiplySignedImmediateByteNotSignExtended)
                        && operation == Operation::MultiplySigned
                        && byte)
                {
                    immediate &= 0xFF;
                }
                Ok((immediate, length))
            }
        }
    };
    // IMUL with three operands multiplies its second by its third.
    let destination = match (destination_operand, decoded.op_count() > 0 && decoded.op0_kind() == OpKind::Register) {
        (Some(operand), _) => read_value(memory, operand.address, length),
        (None, true) => read_operand(processor, 0)?.0,
        (None, false) => 0,
    };
    // IMUL with three operands multiplies its second by its third; SHLD and SHRD shift by their third.
    let (destination, (source, source_length), count) = match decoded.op_count() {
        0 | 1 => (destination, (0, length), 0),
        2 => (destination, read_operand(processor, 1)?, 0),
        _ if operation == Operation::MultiplySigned => (read_operand(processor, 1)?.0, read_operand(processor, 2)?, 0),
        _ => (destination, read_operand(processor, 1)?, read_operand(processor, 2)?.0),
    };
    let registers = &processor.general_purpose_registers;
    let implicit = [registers[0], registers[2]];
    let outcome = operation.apply(
        destination,
        source,
        length,
        source_length,
        implicit,
        count,
        processor.flags,
        behavior.undefined,
    );
    if let Some(exception) = outcome.exception {
        return Err(exception);
    }
    for (register, value) in outcome.implicit.into_iter().flatten() {
        write_register(processor, register, value);
    }
    // The second operand is written first, so that XADD of a register with itself leaves the sum.
    if let Some(second) = outcome.second {
        let register = decoded.op1_register();
        if is_active(Bug::ExchangeSecondKeepsUpperHalf) && operation == Operation::Exchange && register.size() == 4 {
            write_low_half(processor, register, second);
        } else {
            write_register(processor, register, second);
        }
    }
    if let Some(result) = outcome.result {
        match destination_operand {
            Some(operand) => {
                let stored = if is_active(Bug::DecrementStoresFourBytes) && operation == Operation::Decrement {
                    length.max(4)
                } else if is_active(Bug::SetIfStoresTwoBytes) && matches!(operation, Operation::SetIf(_)) {
                    2
                } else {
                    length
                };
                let across_fault = if is_active(Bug::XorWritesBytesBelowFault) && operation == Operation::Xor {
                    StoreAcrossFault::WritesBytesBelowFault
                } else {
                    behavior.across_fault
                };
                let flags_before = processor.flags;
                if is_active(Bug::ShiftStoreFlagsDespiteFault) && operation == Operation::ShiftLeft {
                    processor.flags = outcome.flags;
                }
                store(flags_before, memory, operand, &result.to_le_bytes()[..stored], across_fault, on_memory_mutated)
                    .map_err(|exception| mutate_store_fault(exception, operation, form.place))?;
            }
            None => {
                let register = decoded.op0_register();
                let keeps_upper = register.size() == 4
                    && ((is_active(Bug::OrWriteKeepsUpperHalf) && operation == Operation::Or)
                        || (is_active(Bug::NotWriteKeepsUpperHalf) && operation == Operation::Not)
                        || (is_active(Bug::MoveWriteKeepsUpperHalf) && operation == Operation::Move)
                        || (is_active(Bug::XorWriteKeepsUpperHalf) && operation == Operation::Xor));
                let keeps_upper = keeps_upper
                    || (register.size() == 4
                        && is_active(Bug::LoadEffectiveAddressWriteKeepsUpperHalf)
                        && operation == Operation::LoadEffectiveAddress);
                let high = matches!(register, Register::AH | Register::CH | Register::DH | Register::BH);
                if is_active(Bug::SubtractHighByteWrittenLow) && operation == Operation::Subtract && high {
                    let full = &mut processor.general_purpose_registers[register.full_register().number()];
                    *full = (*full & !0xFF) | (result & 0xFF);
                } else if keeps_upper {
                    write_low_half(processor, register, result);
                } else if is_active(Bug::MoveWordWriteClearsUpperBits) && operation == Operation::Move && register.size() == 2 {
                    processor.general_purpose_registers[register.full_register().number()] = result & 0xFFFF;
                } else {
                    write_register(processor, register, result);
                }
            }
        }
    }
    processor.flags = outcome.flags;
    Ok(())
}

/// The store's fault as the planted bugs of the integer families report it.
fn mutate_store_fault(exception: Exception, operation: Operation, place: OperandPlace) -> Exception {
    let Exception::PageFault { access, page_present, address } = exception else {
        return exception;
    };
    let as_read = (is_active(Bug::MoveStoreFaultReportedAsRead) && operation == Operation::Move)
        || (is_active(Bug::LockedFaultReportedAsRead) && place == OperandPlace::LockedMemory);
    let not_present = is_active(Bug::IncrementReadOnlyReportedNotPresent) && operation == Operation::Increment;
    Exception::PageFault {
        access: if as_read { Access::Read } else { access },
        page_present: page_present && !(not_present && access == Access::Write),
        address,
    }
}

/// Writes the low 32 bits of a 32-bit register's full register, keeping the upper half, as planted bugs do.
fn write_low_half(processor: &mut Processor, register: Register, value: u64) {
    let full = &mut processor.general_purpose_registers[register.full_register().number()];
    *full = (*full & !u64::from(u32::MAX)) | (value & u64::from(u32::MAX));
}

/// PUSH: writes the register below the stack pointer, then lowers the stack pointer; PUSH RSP writes its old value.
fn push(
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    across_fault: StoreAcrossFault,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let source = decoded.op0_register();
    let length = source.size();
    let value = read_register(processor, source);
    let address = processor.general_purpose_registers[STACK_POINTER].wrapping_sub(length as u64);
    let operand = MemoryOperand {
        address,
        stack: !is_active(Bug::PushGeneralProtectionAtNonCanonical),
    };
    if is_active(Bug::PushLowersStackPointerDespiteFault) || is_active(Bug::FaultKeepsRegisterWrites) {
        processor.general_purpose_registers[STACK_POINTER] = address;
    }
    store(
        processor.flags,
        memory,
        operand,
        &value.to_le_bytes()[..length],
        across_fault,
        on_memory_mutated,
    )?;
    processor.general_purpose_registers[STACK_POINTER] = address;
    Ok(())
}

/// The legacy SSE integer skeleton: the destination's lanes and the source's, a register or 16 bytes of memory (or
/// a shift's immediate count), combined into the destination under the form's immediate if it has one; the bits of the vector register above 128 are left as they were. A misaligned
/// memory operand raises #GP(0) before the canonical check, as AC's check comes before it for other forms.
fn execute_vector(
    instruction: Instruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let form = find_vector_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no vector form"));
    let length = form.memory_bytes();
    expect_code(decoded, &[form.code]);
    if form.encoding != VectorEncoding::Legacy {
        return execute_vex_vector(form, processor, memory, decoded);
    }
    let source = match decoded.op1_kind() {
        OpKind::Register => processor.vector_registers[decoded.op1_register().number()],
        OpKind::Immediate8 => u128::from(decoded.immediate8()),
        _ => {
            let operand = locate(processor, decoded);
            let unaligned = (is_active(Bug::OrMisalignedNoFault) && instruction == Instruction::PorXmmdqMemdq)
                || (!form.requires_alignment() && !(is_active(Bug::MoveUnalignedFaults) && instruction == Instruction::MovupsXmmpsMemps))
                || (is_active(Bug::MoveAlignedDoubleMisalignedNoFault) && instruction == Instruction::MovapdXmmpdMempd);
            let length = if (is_active(Bug::DuplicateLowDoubleReadsSixteenBytes) && instruction == Instruction::MovddupXmmdqMemq)
                || (is_active(Bug::MoveLowDoubleReadsSixteenBytes) && instruction == Instruction::MovlpdXmmsdMemq)
            {
                16
            } else {
                length
            };
            if !operand.address.is_multiple_of(SSE_ALIGNMENT) && !unaligned {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            check_access(processor.flags, memory, operand, length, Access::Read)?;
            let value = u128::from_le_bytes(read_bytes(memory, operand.address)) & (u128::MAX >> (128 - 8 * length));
            if is_active(Bug::XorReadsEightBytes) && instruction == Instruction::PxorXmmdqMemdq {
                value & u128::from(u64::MAX)
            } else {
                value
            }
        }
    };
    let number = decoded.op0_register().number();
    let immediate = if decoded.op_count() > 2 { decoded.immediate8() } else { 0 };
    let result = form.operation.apply(processor.vector_registers[number], source, immediate);
    if form.operation.writes_flags() {
        processor.flags = processor.flags.difference(Flags::STATUS) | Flags::from_bits_truncate(result as u64);
    } else {
        processor.vector_registers[number] = result;
    }
    Ok(())
}

/// The VEX skeleton of a lane operation: VEX.vvvv's register and a register or memory (16 or 32 bytes, any alignment)
/// combined into the destination, each 128-bit half as the legacy form combines its lanes; VEX.128 zeroes the
/// destination's upper half.
fn execute_vex_vector(
    form: &VectorForm,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let wide = form.encoding == VectorEncoding::L256;
    let read_register = |processor: &Processor, register: Register| {
        let number = register.number();
        [processor.vector_registers[number], processor.upper_vector_halves[number]]
    };
    let first_register = if is_active(Bug::VexFirstSourceIsDestination) { decoded.op0_register() } else { decoded.op1_register() };
    let first = read_register(processor, first_register);
    let second = match decoded.op2_kind() {
        OpKind::Register => read_register(processor, decoded.op2_register()),
        _ => {
            let operand = locate(processor, decoded);
            if is_active(Bug::VexMisalignedFaults) && !operand.address.is_multiple_of(SSE_ALIGNMENT) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            let read = if is_active(Bug::VexWideReadsSixteenBytes) { 16 } else { form.memory_bytes() };
            check_access(processor.flags, memory, operand, read, Access::Read)?;
            let bytes: [u8; 2 * VECTOR_HALF_BYTES] = read_bytes(memory, operand.address);
            let (low, high) = bytes.split_at(VECTOR_HALF_BYTES);
            let half = |bytes: &[u8]| u128::from_le_bytes(bytes.try_into().expect("sixteen bytes"));
            [half(low), if wide && read > 16 { half(high) } else { 0 }]
        }
    };
    let number = decoded.op0_register().number();
    let kept = processor.upper_vector_halves[number];
    processor.vector_registers[number] = form.operation.apply(first[0], second[0], 0);
    processor.upper_vector_halves[number] = match (wide, is_active(Bug::VexWideUpperFromLowerHalf)) {
        (true, false) => form.operation.apply(first[1], second[1], 0),
        (true, true) => form.operation.apply(first[0], second[1], 0),
        (false, _) if is_active(Bug::VexNarrowKeepsUpperHalf) => kept,
        (false, _) => 0,
    };
    Ok(())
}

/// Bytes of a half of a YMM register.
const VECTOR_HALF_BYTES: usize = size_of::<u128>();

/// The legacy SSE floating-point skeleton: the destination's lanes and the source's, a register or memory (16 bytes
/// aligned to 16 for a packed form, the lowest lane's bytes for a scalar one), computed under MXCSR; the flags found
/// are added to MXCSR, and with an unmasked one #XM leaves the destination as it was.
fn execute_float(
    instruction: Instruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let form = find_float_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no floating-point form"));
    expect_code(decoded, &[form.code]);
    let source = match decoded.op1_kind() {
        OpKind::Register => processor.vector_registers[decoded.op1_register().number()],
        _ => {
            let operand = locate(processor, decoded);
            let length = match form.memory_bytes() as usize {
                16 => {
                    let unaligned =
                        is_active(Bug::DividePackedDoubleMisalignedNoFault) && instruction == Instruction::DivpdXmmpdMempd;
                    if !operand.address.is_multiple_of(SSE_ALIGNMENT) && !unaligned {
                        return Err(Exception::GeneralProtection { error_code: 0 });
                    }
                    size_of::<u128>()
                }
                _ if is_active(Bug::AddScalarSingleReadsEightBytes) && instruction == Instruction::AddssXmmssMemss => 8,
                _ if is_active(Bug::FloatScalarMisalignedFaults)
                    && instruction == Instruction::SubsdXmmsdMemsd
                    && !operand.address.is_multiple_of(SSE_ALIGNMENT) =>
                {
                    return Err(Exception::GeneralProtection { error_code: 0 });
                }
                length => length,
            };
            check_access(processor.flags, memory, operand, length as u64, Access::Read)?;
            let bytes: [u8; size_of::<u128>()] = read_bytes(memory, operand.address);
            u128::from_le_bytes(bytes) & (u128::MAX >> (u128::BITS as usize - length * 8))
        }
    };
    let number = decoded.op0_register().number();
    let immediate = if decoded.op_count() > 2 { decoded.immediate8() } else { 0 };
    match form.apply(processor.vector_registers[number], source, processor.mxcsr, immediate) {
        Ok((result, flags)) if form.operation.writes_flags() => {
            let order = Flags::from_bits_truncate(result as u64 & ORDER_FLAGS);
            processor.flags = processor.flags.difference(Flags::STATUS) | order;
            processor.mxcsr |= flags;
            Ok(())
        }
        Ok((result, flags)) => {
            processor.vector_registers[number] = result;
            processor.mxcsr |= flags;
            Ok(())
        }
        Err(flags) => {
            let written = (is_active(Bug::MultiplyPackedDoubleWritesDespiteFault) && instruction == Instruction::MulpdXmmpdXmmpd)
                .then(|| form.apply(processor.vector_registers[number], source, processor.mxcsr | crate::MXCSR_DEFAULT, 0));
            if let Some(Ok((result, _))) = written {
                processor.vector_registers[number] = result;
            }
            processor.mxcsr |= flags;
            Err(Exception::SimdFloatingPoint)
        }
    }
}

/// The near jump skeleton: whether the jump is taken, by its decision, and if so its target, which must be
/// canonical or the jump raises #GP(0) and changes nothing; LOOP writes the count less one back first. With the
/// 0x67 prefix the count register is ECX, whose write clears RCX's upper half.
fn execute_branch(instruction: Instruction, processor: &mut Processor, decoded: &Decoded) -> Result<(), Exception> {
    const COUNT: usize = 1;
    let form = find_branch_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no near jump form"));
    expect_code(decoded, form.codes);
    let narrow = matches!(
        decoded.code(),
        Code::Jecxz_rel8_64 | Code::Loop_rel8_64_ECX | Code::Loope_rel8_64_ECX | Code::Loopne_rel8_64_ECX
    );
    let mask = if narrow { u64::from(u32::MAX) } else { u64::MAX };
    let count = processor.general_purpose_registers[COUNT]
        & if is_active(Bug::JumpIfCountZeroTestsLowHalf) && matches!(form.decision, Decision::CountIsZero) { u64::from(u32::MAX) } else { mask };
    let zero = processor.flags.contains(Flags::ZERO);
    let less_one = count.wrapping_sub(1) & mask;
    let taken = match form.decision {
        Decision::Always => true,
        Decision::If(Condition::Less) if is_active(Bug::JumpIfLessIgnoresOverflow) => processor.flags.contains(Flags::SIGN),
        Decision::If(Condition::BelowOrEqual) if is_active(Bug::JumpIfBelowOrEqualIgnoresZero) => processor.flags.contains(Flags::CARRY),
        Decision::If(condition) => condition.holds(processor.flags),
        Decision::CountIsZero => count == 0,
        Decision::Loop => less_one != 0,
        Decision::LoopWhileZero => less_one != 0 && (zero || is_active(Bug::LoopWhileZeroIgnoresZero)),
        Decision::LoopWhileNotZero => less_one != 0 && (zero == is_active(Bug::LoopWhileNotZeroReversed)),
    };
    let mut target = if form.indirect { read_register(processor, decoded.op0_register()) } else { decoded.near_branch_target() };
    if is_active(Bug::JumpIndirectTruncated) && form.indirect {
        target &= u64::from(u32::MAX);
    }
    if is_active(Bug::JumpRelativeByteZeroExtended) && decoded.code() == Code::Jmp_rel8_64 {
        target = decoded.next_ip().wrapping_add(u64::from(target.wrapping_sub(decoded.next_ip()) as u8));
    }
    if taken && !is_canonical(target) && !is_active(Bug::JumpToNonCanonical) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    if matches!(form.decision, Decision::Loop | Decision::LoopWhileZero | Decision::LoopWhileNotZero) {
        let kept = if is_active(Bug::LoopCountKeepsUpperHalf) { processor.general_purpose_registers[COUNT] & !mask } else { 0 };
        processor.general_purpose_registers[COUNT] = kept | less_one;
    }
    processor.instruction_pointer = if taken { target } else { decoded.next_ip() };
    Ok(())
}

fn read_register(processor: &Processor, register: Register) -> u64 {
    let full = processor.general_purpose_registers[register.full_register().number()];
    match register {
        Register::AH | Register::CH | Register::DH | Register::BH => (full >> 8) & 0xFF,
        _ => full & to_mask(register.size()),
    }
}

/// Writes a register as x86-64 does: a 32-bit write clears the upper half, a 16-bit or 8-bit write keeps it.
fn write_register(processor: &mut Processor, register: Register, value: u64) {
    let full = &mut processor.general_purpose_registers[register.full_register().number()];
    *full = match (register, register.size()) {
        (Register::AH | Register::CH | Register::DH | Register::BH, _) => {
            (*full & !0xFF00) | ((value & 0xFF) << 8)
        }
        (_, 8) => value,
        (_, 4) if is_active(Bug::ThirtyTwoBitWriteKeepsUpperHalf) => (*full & !u64::from(u32::MAX)) | (value & u64::from(u32::MAX)),
        (_, 4) => value & u64::from(u32::MAX),
        (_, length) => (*full & !to_mask(length)) | (value & to_mask(length)),
    };
}
