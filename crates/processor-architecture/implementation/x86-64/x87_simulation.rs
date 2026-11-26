//! The skeleton of the x87 forms: #MF while an exception is pending for a waiting form, the memory operand read or
//! written, the stack operation with its faults, the undefined condition codes, and FIP, FOP and FDP.

use iced_x86::{Instruction as Decoded, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{
    LEGACY_PREFIXES, Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, is_rex, locate,
    store, write_register,
};
use crate::memory::read_bytes;
use crate::mmx::is_pending;
use crate::planted::{Bug, is_active};
use crate::x87::{
    BUSY, C0, C1, C2, C3, Class, CodeRule, DENORMAL, EXCEPTION_FLAGS, INDEFINITE, INVALID, Ordering, PointerUpdate,
    Rounding, STACK_FAULT, SUMMARY, Source as X87Source, X87Behavior, X87Operation, classify, compare_values,
    convert_integer, examine, find_x87_form, initialize, round_to_integer, widen, MisalignedArea,
};
use crate::{Access, Exception, Flags, Instruction, Processor, X87, X86_64, x87_arithmetic, x87_state};

#[path = "x87_transcendental_simulation.rs"]
mod transcendental;

/// FSW's PE.
const PRECISION: u16 = 1 << 5;
/// FXSAVE's and FXRSTOR's area lies on 16 bytes.
const AREA_ALIGNMENT: u64 = 16;
/// The bits of MXCSR this chip implements, which FXSAVE stores as MXCSR_MASK.
const MXCSR_MASK: u32 = 0xFFFF;
/// What the 32-bit environment holds above each of its 16-bit fields.
const ENVIRONMENT_FILLER: u16 = 0xFFFF;
/// FXSAVE's area.
const EXTENDED_AREA_BYTES: u64 = 512;
/// FBSTP stores integers below 10 to the 18th.
const DECIMAL_LIMIT: u64 = 1_000_000_000_000_000_000;

/// The bits of FCW FLDCW keeps: the masks, precision, rounding and infinity control; bit 6 always reads as set.
const CONTROL_KEPT: u16 = 0x1F3F;
const CONTROL_SET: u16 = 0x0040;
/// The alignment AC checks of an 80-bit operand.
const EXTENDED_ALIGNMENT: u64 = 8;
const EXTENDED_BYTES: u64 = 10;
const QUADWORD_BYTES: usize = 8;
const SIGN: u128 = 1 << 79;
/// The bits of the opcode FOP keeps: the low three of the first byte, then the ModRM byte.
const OPCODE_LOW_BITS: u8 = 0b111;

pub(super) fn execute_x87(
    instruction: Instruction,
    behavior: X87Behavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_x87_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no x87 form"));
    expect_code(decoded, &[form.code]);
    if form.waits() && is_pending(&processor.x87) && !is_active(Bug::X87IgnoresPendingException) {
        return Err(Exception::X87FloatingPoint);
    }
    let before = processor.x87;
    let mut x87 = processor.x87;
    let operand = form.has_memory_operand().then(|| locate(processor, decoded));
    let length = form.memory_bytes();
    let load = |processor: &Processor| -> Result<u128, Exception> {
        let operand = operand.expect("a memory operand");
        check_extended_alignment(processor, operand, length)?;
        check_access(processor.flags, memory, operand, length, Access::Read)?;
        let bytes: [u8; 16] = read_bytes(memory, operand.address);
        Ok(u128::from_le_bytes(bytes) & (u128::MAX >> (128 - 8 * length)))
    };
    let index = (0..decoded.op_count())
        .map(|operand| decoded.op_register(operand))
        .find(|register| register.is_st() && *register != Register::ST0 || decoded.op_count() == 1 && register.is_st())
        .map_or(form.to_implied_index(), |register| register.number());
    let mut stored: Option<u128> = None;
    // An image wider than sixteen bytes that the form stores whole, but the byte ranges it skips.
    let mut image: Option<alloc::vec::Vec<u8>> = None;
    let mut skipped: &[core::ops::Range<usize>] = &[];
    let read_image = |processor: &Processor| -> Result<alloc::vec::Vec<u8>, Exception> {
        let operand = operand.expect("a memory operand");
        check_area_last_byte(memory, operand, length, Access::Read)?;
        check_access(processor.flags, memory, operand, length, Access::Read).map_err(|exception| to_last_byte(exception, operand, length))?;
        Ok((0..length).map(|offset| read_bytes::<X86_64, 1>(memory, operand.address.wrapping_add(offset))[0]).collect())
    };
    let check_area_alignment = |processor: &Processor| -> Result<(), Exception> {
        let address = operand.expect("a memory operand").address;
        check_canonical(operand.expect("a memory operand"), 1)?;
        if address.is_multiple_of(AREA_ALIGNMENT) || is_active(Bug::X87AreaMisalignedNoFault) {
            Ok(())
        } else if behavior.misaligned_area == MisalignedArea::AlignmentCheck && processor.flags.contains(Flags::ALIGNMENT_CHECK) {
            Err(Exception::AlignmentCheck)
        } else {
            Err(Exception::GeneralProtection { error_code: 0 })
        }
    };
    match form.operation {
        X87Operation::NoOperation | X87Operation::Wait | X87Operation::LegacyNoOperation => {}
        X87Operation::ClearExceptions => x87.status &= !(EXCEPTION_FLAGS | STACK_FAULT | SUMMARY | BUSY),
        X87Operation::Initialize => initialize(&mut x87),
        X87Operation::StoreStatus | X87Operation::StoreControl => {
            let value = if form.operation == X87Operation::StoreStatus { x87.status } else { x87.control };
            match operand {
                Some(_) => stored = Some(u128::from(value)),
                None => write_register(processor, Register::AX, u64::from(value)),
            }
        }
        X87Operation::LoadControl => {
            let kept = if is_active(Bug::LoadControlKeepsEveryBit) { u16::MAX } else { CONTROL_KEPT };
            x87.control = load(processor)? as u16 & kept | CONTROL_SET;
            // ES and B follow the flags the new masks leave unmasked.
            let pending = x87.is_unmasked(x87.status);
            x87.set_code(SUMMARY | BUSY, pending);
        }
        X87Operation::IncrementTop | X87Operation::DecrementTop => {
            let step = if form.operation == X87Operation::IncrementTop { 1 } else { 7 };
            x87.set_top(x87.top() + step);
            x87.set_code(C1, false);
        }
        X87Operation::Free { pop } => {
            x87.free(index);
            if pop {
                x87.pop();
            }
        }
        X87Operation::Exchange => {
            let (first, second) = (x87.is_empty(0), x87.is_empty(index));
            if (first || second) && !underflow(&mut x87) {
                // Unmasked, nothing is exchanged.
            } else {
                let value = |empty: bool, index: usize| if empty { INDEFINITE } else { x87.read(index) };
                let (top, other) = (value(first, 0), value(second, index));
                x87.write(0, other);
                x87.write(index, top);
                x87.set_code(C1, false);
            }
        }
        X87Operation::LoadRegister => {
            // An empty source is an underflow before a full stack is an overflow.
            if x87.is_empty(index) && !is_active(Bug::X87OverflowBeforeUnderflow) {
                if underflow(&mut x87) {
                    x87.set_top(x87.top() + 7);
                    x87.write(0, INDEFINITE);
                }
            } else {
                let value = (!x87.is_empty(index)).then(|| x87.read(index));
                push(&mut x87, value);
            }
        }
        X87Operation::LoadConstant(value) => push(&mut x87, Some(value)),
        X87Operation::LoadExtended => {
            let value = load(processor)?;
            push(&mut x87, Some(value));
        }
        X87Operation::LoadInteger => {
            let raw = load(processor)? as u64;
            let unused = 64 - 8 * length as u32;
            let value = ((raw << unused) as i64) >> unused;
            push(&mut x87, Some(convert_integer(value)));
        }
        X87Operation::StoreRegister { pop } => {
            if !x87.is_empty(0) || underflow(&mut x87) {
                let value = if x87.is_empty(0) { INDEFINITE } else { x87.read(0) };
                x87.write(index, value);
                if pop {
                    x87.pop();
                }
                x87.set_code(C1, false);
            }
        }
        X87Operation::StoreRegisterUnchecked => {
            if !x87.is_empty(0) {
                x87.write(index, x87.read(0));
            }
            x87.pop();
            x87.set_code(C1, false);
        }
        X87Operation::StoreExtended => {
            if !x87.is_empty(0) || underflow(&mut x87) {
                stored = Some(if x87.is_empty(0) { INDEFINITE } else { x87.read(0) });
                x87.pop();
                x87.set_code(C1, false);
            }
        }
        X87Operation::ChangeSign | X87Operation::Absolute => {
            if x87.is_empty(0) {
                // Masked, the underflow leaves the indefinite QNaN as it is.
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else {
                let value = x87.read(0);
                let changed = if form.operation == X87Operation::ChangeSign { value ^ SIGN } else { value & !SIGN };
                x87.write(0, changed);
                x87.set_code(C1, false);
            }
        }
        X87Operation::MoveIf(condition) => {
            if x87.is_empty(0) || x87.is_empty(index) {
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else if condition.holds(processor.flags) {
                x87.write(0, x87.read(index));
            }
        }
        X87Operation::LoadFloat => {
            let raw = load(processor)? as u64;
            let (value, raised) = if length == 4 { widen(raw, 23, 8) } else { widen(raw, 52, 11) };
            if !x87.is_empty(7) {
                push(&mut x87, Some(value));
            } else {
                x87.raise(raised);
                // An unmasked IE stops the load before it pushes; an unmasked DE does not.
                if !x87.is_unmasked(raised & INVALID) || is_active(Bug::LoadFloatPushesDespiteUnmasked) {
                    push(&mut x87, Some(value));
                }
            }
        }
        X87Operation::Compare { ordering, source, pops, to_flags } => {
            let (other, raised) = match source {
                X87Source::Register => (x87.read(index), 0),
                X87Source::Zero => (0, 0),
                X87Source::Single => widen(load(processor)? as u64, 23, 8),
                X87Source::Double => widen(load(processor)? as u64, 52, 11),
                X87Source::Integer => {
                    let raw = load(processor)? as u64;
                    let unused = 64 - 8 * length as u32;
                    (convert_integer(((raw << unused) as i64) >> unused), 0)
                }
            };
            let empty = x87.is_empty(0) || source == X87Source::Register && x87.is_empty(index);
            let top = x87.read(0);
            // A fault, masked or not, leaves the codes unordered; only a masked one lets the pops go on.
            let (order, completes) = if empty {
                (None, underflow(&mut x87))
            } else {
                compare(&mut x87, top, other, raised, ordering)
            };
            {
                let (c3, c2, c0) = match order {
                    None => (true, true, true),
                    Some(core::cmp::Ordering::Greater) => (false, false, false),
                    Some(core::cmp::Ordering::Less) => (false, false, true),
                    Some(core::cmp::Ordering::Equal) => (true, false, false),
                };
                if to_flags {
                    let mut flags = processor.flags.difference(Flags::STATUS);
                    flags.set(Flags::ZERO, c3);
                    flags.set(Flags::PARITY, c2);
                    flags.set(Flags::CARRY, c0);
                    processor.flags = flags;
                } else {
                    x87.set_code(C3, c3);
                    x87.set_code(C2, c2);
                    x87.set_code(C0, c0);
                }
                // FCOMI and FUCOMI leave C1 as it was but for a stack fault, which clears it.
                if !to_flags || empty {
                    x87.set_code(C1, false);
                }
                for _ in 0..pops {
                    if completes {
                        x87.pop();
                    }
                }
            }
        }
        X87Operation::StoreInteger { pop } => {
            let bits = 8 * length as u32;
            let indefinite = 1u128 << (bits - 1);
            let top = x87.read(0);
            let value = if x87.is_empty(0) {
                underflow(&mut x87).then_some(indefinite)
            } else {
                round_for_store(&mut x87, top, bits).map(|integer| integer.unwrap_or(indefinite))
            };
            if let Some(value) = value {
                stored = Some(value);
                if pop {
                    x87.pop();
                }
            }
        }
        X87Operation::Examine => {
            let codes = examine(x87.read(0), x87.is_empty(0));
            x87.status = (x87.status & !(C0 | C2 | C3)) | codes;
            x87.set_code(C1, x87.read(0) & SIGN != 0);
        }
        X87Operation::Arithmetic { operation, source, reverse, pop } => {
            let to_indexed = decoded.op_count() == 2 && decoded.op0_register() != Register::ST0;
            let (destination, source_index) = if to_indexed { (index, 0) } else { (0, index) };
            let (other, denormal_source) = match source {
                X87Source::Register | X87Source::Zero => (x87.read(source_index), false),
                X87Source::Single | X87Source::Double => {
                    let raw = load(processor)? as u64;
                    let (value, raised) = if source == X87Source::Single { widen(raw, 23, 8) } else { widen(raw, 52, 11) };
                    // A signaling NaN in memory stays signaling, so that the arithmetic raises IE and picks NaNs by it.
                    let value = if raised & INVALID != 0 { value & !(1 << 62) } else { value };
                    (value, raised & DENORMAL != 0)
                }
                X87Source::Integer => {
                    let raw = load(processor)? as u64;
                    let unused = 64 - 8 * length as u32;
                    (convert_integer(((raw << unused) as i64) >> unused), false)
                }
            };
            let empty = x87.is_empty(destination) || source == X87Source::Register && x87.is_empty(source_index);
            if empty {
                if underflow(&mut x87) {
                    x87.write(destination, INDEFINITE);
                    if pop {
                        x87.pop();
                    }
                }
            } else {
                let mine = x87.read(destination);
                let reverse = reverse != is_active(Bug::X87ReverseIgnored);
                let (first, second) = if reverse { (other, mine) } else { (mine, other) };
                let outcome = x87_arithmetic::compute_with_source(operation, first, second, x87.control, denormal_source);
                x87.raise(outcome.flags);
                if let Some(value) = outcome.value {
                    x87.write(destination, value);
                    x87.set_code(C1, outcome.up);
                    if pop {
                        x87.pop();
                    }
                }
            }
        }
        X87Operation::StoreFloat { pop } => {
            let (fraction_bits, exponent_bits) = if length == 4 { (23, 8) } else { (52, 11) };
            let indefinite = if length == 4 { 0xFFC0_0000 } else { 0xFFF8_0000_0000_0000 };
            if x87.is_empty(0) {
                if underflow(&mut x87) {
                    stored = Some(indefinite);
                }
            } else {
                let (bits, flags, up) = x87_arithmetic::store_float(x87.read(0), fraction_bits, exponent_bits, x87.control);
                x87.raise(flags);
                if let Some(bits) = bits {
                    stored = Some(u128::from(bits));
                    x87.set_code(C1, up);
                }
            }
            if stored.is_some() && pop {
                x87.pop();
            }
        }
        X87Operation::LoadRoundedConstant { truncated, above_half } => {
            let up = match Rounding::from_control(x87.control) {
                Rounding::Nearest => above_half,
                Rounding::Up => true,
                Rounding::Down | Rounding::TowardZero => false,
            };
            let up = up && !is_active(Bug::X87ConstantsTruncated);
            push(&mut x87, Some(truncated + u128::from(up)));
        }
        X87Operation::Scale => {
            if x87.is_empty(0) || x87.is_empty(1) {
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else {
                let outcome = x87_arithmetic::scale(x87.read(0), x87.read(1), x87.control);
                x87.raise(outcome.flags);
                if let Some(result) = outcome.value {
                    x87.write(0, result);
                    x87.set_code(C1, outcome.up);
                }
            }
        }
        X87Operation::Extract => {
            // An empty ST(0) is an underflow whatever ST(7) holds; else a full ST(7) is an overflow, which masked leaves
            // the indefinite QNaN in both.
            if x87.is_empty(0) || !x87.is_empty(7) {
                let overflow = !x87.is_empty(0);
                if fault(&mut x87, overflow) {
                    x87.write(0, INDEFINITE);
                    x87.set_top(x87.top() + 7);
                    x87.write(0, INDEFINITE);
                }
            } else {
                let (result, flags) = x87_arithmetic::extract(x87.read(0), x87.control);
                x87.raise(flags);
                if let Some((significand, exponent)) = result {
                    x87.write(0, exponent);
                    push(&mut x87, Some(significand));
                }
            }
        }
        X87Operation::PartialRemainder { nearest } => {
            if x87.is_empty(0) || x87.is_empty(1) {
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else {
                let (result, flags, codes) = x87_arithmetic::remainder(x87.read(0), x87.read(1), x87.control, nearest);
                x87.raise(flags);
                if let Some(value) = result {
                    x87.write(0, value);
                    x87.status = (x87.status & !(C0 | C1 | C2 | C3)) | codes;
                }
            }
        }
        X87Operation::LoadDecimal => {
            let (negative, magnitude) = x87_state::read_decimal(load(processor)?);
            let value = convert_integer(magnitude as i64) | if negative { SIGN } else { 0 };
            push(&mut x87, Some(value));
        }
        X87Operation::StoreDecimal => {
            let value = x87.read(0);
            let rounded = match classify(value) {
                _ if x87.is_empty(0) => None,
                Class::QuietNaN | Class::SignalingNaN | Class::Infinity | Class::Unsupported => Some(None),
                _ => Some(round_to_integer(value, Rounding::from_control(x87.control), 64).filter(|(integer, _, _)| integer.unsigned_abs() < DECIMAL_LIMIT)),
            };
            let decimal = match rounded {
                None => underflow(&mut x87).then_some(x87_state::DECIMAL_INDEFINITE),
                Some(None) => {
                    x87.raise(INVALID);
                    x87.set_code(C1, false);
                    (!x87.is_unmasked(INVALID)).then_some(x87_state::DECIMAL_INDEFINITE)
                }
                Some(Some((integer, inexact, up))) => {
                    if inexact {
                        x87.raise(PRECISION);
                    }
                    x87.set_code(C1, inexact && up);
                    Some(x87_state::write_decimal(value & SIGN != 0, integer.unsigned_abs()))
                }
            };
            if let Some(decimal) = decimal {
                stored = Some(decimal);
                x87.pop();
            }
        }
        X87Operation::StoreEnvironment | X87Operation::SaveState => {
            check_environment_alignment(processor, operand.expect("a memory operand"), length)?;
            let environment = if length == 14 || length == 94 { 14 } else { 28 };
            let mut bytes = x87_state::write_environment(&x87, environment, ENVIRONMENT_FILLER);
            if form.operation == X87Operation::SaveState {
                bytes.extend(x87_state::write_registers(&x87));
                initialize(&mut x87);
            } else {
                x87.control |= EXCEPTION_FLAGS;
                x87_state::set_summary(&mut x87);
            }
            // The 16-bit format's selector words hold zeros, as this chip deprecates FCS and FDS.
            if environment == 14 && is_active(Bug::X87EnvironmentSkipsSelectors) {
                skipped = &SIXTEEN_BIT_SELECTORS;
            }
            image = Some(bytes);
        }
        X87Operation::LoadEnvironment | X87Operation::RestoreState => {
            check_environment_alignment(processor, operand.expect("a memory operand"), length)?;
            let bytes = read_image(processor)?;
            let environment = if length == 14 || length == 94 { 14 } else { 28 };
            let _ = &mut skipped;
            x87 = x87_state::read_environment(&x87, &bytes, environment);
            if form.operation == X87Operation::RestoreState {
                x87_state::read_registers(&mut x87, &bytes[environment..]);
            }
        }
        X87Operation::SaveExtended { wide } => {
            check_area_alignment(processor)?;
            image = Some(x87_state::write_extended(&x87, processor.mxcsr, MXCSR_MASK, &processor.vector_registers, wide));
        }
        X87Operation::RestoreExtended { wide } => {
            check_area_alignment(processor)?;
            let bytes = read_image(processor)?;
            let (loaded, mxcsr, vectors) = x87_state::read_extended(&bytes, wide);
            if mxcsr & !MXCSR_MASK != 0 && !is_active(Bug::X87RestoreTakesReservedMxcsr) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            x87 = loaded;
            x87_state::set_summary(&mut x87);
            processor.mxcsr = mxcsr;
            processor.vector_registers = vectors;
        }
        X87Operation::Transcendental(function) => {
            transcendental::execute_transcendental(&mut x87, function);
        }
        X87Operation::SquareRoot | X87Operation::RoundToIntegral => {
            if x87.is_empty(0) {
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else {
                let value = x87.read(0);
                let outcome = if form.operation == X87Operation::SquareRoot {
                    x87_arithmetic::square_root(value, x87.control)
                } else {
                    x87_arithmetic::round_integral(value, x87.control)
                };
                x87.raise(outcome.flags);
                if let Some(result) = outcome.value {
                    x87.write(0, result);
                    x87.set_code(C1, outcome.up);
                }
            }
        }
    }
    let mut on_memory_mutated = on_memory_mutated;
    if let (Some(bytes), Some(operand)) = (image, operand) {
        // The whole access faults or completes as one; the bytes it skips are then not written.
        check_area_last_byte(memory, operand, length, Access::Write)?;
        check_access(processor.flags, memory, operand, length, Access::Write).map_err(|exception| to_last_byte(exception, operand, length))?;
        {
            let mut start = 0;
            for range in skipped.iter().chain([&(bytes.len()..bytes.len())]) {
                if range.start > start {
                    on_memory_mutated(MemoryDelta::ValueAssignment {
                        start: operand.address.wrapping_add(start as u64),
                        bytes: alloc::borrow::Cow::Owned(bytes[start..range.start].to_vec()),
                    });
                }
                start = range.end;
            }
        }
    }
    if let (Some(value), Some(operand)) = (stored, operand) {
        check_extended_alignment(processor, operand, length)?;
        let bytes = value.to_le_bytes();
        // The chip stores 80 bits as eight bytes, then two, so a fault in the last two leaves the first eight written.
        let split = if length == EXTENDED_BYTES && !is_active(Bug::ExtendedStoreWhole) { QUADWORD_BYTES } else { length as usize };
        store(processor.flags, memory, operand, &bytes[..split], behavior.across_fault, &mut on_memory_mutated)?;
        if split < length as usize {
            let rest = MemoryOperand { address: operand.address.wrapping_add(split as u64), ..operand };
            store(processor.flags, memory, rest, &bytes[split..length as usize], behavior.across_fault, &mut on_memory_mutated)?;
        }
    }
    let undefined = form.list_undefined_codes();
    for (codes, rule) in [(undefined & C1, behavior.codes.c1), (undefined & !C1, behavior.codes.others)] {
        let kept = match rule {
            CodeRule::Unchanged => before.status & codes,
            CodeRule::Cleared => 0,
        };
        x87.status = (x87.status & !codes) | kept;
    }
    if !form.is_control() && !is_active(Bug::X87InstructionPointerKept) {
        let unmasked = x87.is_unmasked(x87.status & !before.status);
        x87.instruction_pointer = processor.instruction_pointer;
        if behavior.opcode == PointerUpdate::Always || unmasked {
            x87.opcode = read_opcode(memory, processor.instruction_pointer);
        }
        if let Some(operand) = operand.filter(|_| behavior.data == PointerUpdate::Always || unmasked) {
            x87.data_pointer = operand.address;
        }
    }
    processor.x87 = x87;
    Ok(())
}

/// What a transcendental form's behavior leaves undecided on the state before: none for any other form.
pub(super) fn find_undecided_x87(instruction: Instruction, before: &X87) -> Option<transcendental::Undecided> {
    let X87Operation::Transcendental(function) = find_x87_form(instruction)?.operation else {
        return None;
    };
    Some(transcendental::execute_transcendental(&mut before.clone(), function))
}

/// The selectors of FIP and FDP in the 16-bit environment.
const SIXTEEN_BIT_SELECTORS: [core::ops::Range<usize>; 2] = [8..10, 12..14];

/// #AC for an environment or a save area not aligned to 2 in the 16-bit format or to 4 in the 32-bit one while AC is
/// set, after a first byte that is not canonical.
fn check_environment_alignment(processor: &Processor, operand: MemoryOperand, length: u64) -> Result<(), Exception> {
    check_canonical(operand, 1)?;
    let alignment = if length == 14 || length == 94 { 2 } else { 4 };
    if processor.flags.contains(Flags::ALIGNMENT_CHECK) && !operand.address.is_multiple_of(alignment) && !is_active(Bug::X87EnvironmentAlignmentUnchecked) {
        return Err(Exception::AlignmentCheck);
    }
    Ok(())
}

/// A page fault of an environment, a save area or FXSAVE's area beyond the page of its first byte, reported at the
/// access's last byte as the chip reports it.
fn to_last_byte(exception: Exception, operand: MemoryOperand, length: u64) -> Exception {
    const PAGE_MASK: u64 = !0xFFF;
    match exception {
        Exception::PageFault { access, page_present, address }
            if address & PAGE_MASK != operand.address & PAGE_MASK && !is_active(Bug::X87AreaFaultsAtFirstByte) =>
        {
            Exception::PageFault { access, page_present, address: operand.address.wrapping_add(length - 1) }
        }
        other => other,
    }
}

/// The page fault an access of FXSAVE's or FXRSTOR's 512 bytes raises at its last byte, which the chip checks before
/// the first.
fn check_area_last_byte(memory: Memory<'_, '_>, operand: MemoryOperand, length: u64, access: Access) -> Result<(), Exception> {
    if is_active(Bug::X87AreaFaultsAtFirstByte) || length != EXTENDED_AREA_BYTES {
        return Ok(());
    }
    let last = operand.address.wrapping_add(length - 1);
    check_canonical(MemoryOperand { address: last, ..operand }, 1)?;
    match super::find_fault(memory, last, 1, access) {
        Some((address, page_present)) => Err(Exception::PageFault { access, page_present, address }),
        None => Ok(()),
    }
}

/// #AC for an 80-bit operand not aligned to 8 while AC is set, which the integer check leaves to wider accesses;
/// after a first byte that is not canonical, as for every access.
fn check_extended_alignment(processor: &Processor, operand: MemoryOperand, length: u64) -> Result<(), Exception> {
    if length != EXTENDED_BYTES || is_active(Bug::ExtendedAlignmentUnchecked) {
        return Ok(());
    }
    check_canonical(operand, 1)?;
    check_alignment(processor.flags, operand, EXTENDED_ALIGNMENT)
}

/// Pushes the value, or with `None` reports the underflow of reading an empty register: a stack fault either way
/// where ST(7) is in use, which masked pushes the indefinite QNaN.
fn push(x87: &mut X87, value: Option<u128>) {
    let overflow = !x87.is_empty(7) && !is_active(Bug::X87PushIgnoresOverflow);
    if overflow {
        if fault(x87, true) {
            x87.set_top(x87.top() + 7);
            x87.write(0, INDEFINITE);
        }
        return;
    }
    let Some(value) = value.or_else(|| underflow(x87).then_some(INDEFINITE)) else {
        return;
    };
    x87.set_top(x87.top() + 7);
    x87.write(0, value);
    x87.set_code(C1, false);
}

/// Reports a stack underflow, and whether IE is masked, so that the form goes on with the indefinite QNaN.
fn underflow(x87: &mut X87) -> bool {
    fault(x87, false)
}

fn fault(x87: &mut X87, overflow: bool) -> bool {
    x87.raise(INVALID | STACK_FAULT);
    x87.set_code(C1, overflow);
    !x87.is_unmasked(INVALID)
}

/// Compares ST(0) with the source, raising what the operands raise: IE for an unsupported operand, an SNaN, a NaN
/// in an ordered comparison or an SNaN the source's widening found; else DE for a denormal where neither is a NaN.
/// Gives the order, `None` for unordered, and whether no exception it raised is unmasked.
fn compare(x87: &mut X87, first: u128, second: u128, raised: u16, ordering: Ordering) -> (Option<core::cmp::Ordering>, bool) {
    let classes = [classify(first), classify(second)];
    let nan = classes.iter().any(|class| matches!(class, Class::QuietNaN | Class::SignalingNaN));
    let invalid = raised & INVALID != 0
        || classes.iter().any(|class| match class {
            Class::Unsupported | Class::SignalingNaN => true,
            Class::QuietNaN => ordering == Ordering::Ordered && !is_active(Bug::OrderedComparisonQuietNaNValid),
            _ => false,
        });
    let found = if invalid {
        INVALID
    } else if !nan && (raised & DENORMAL != 0 || classes.contains(&Class::Denormal)) {
        DENORMAL
    } else {
        0
    };
    x87.raise(found);
    ((!nan && !invalid).then(|| compare_values(first, second)), !x87.is_unmasked(found))
}

/// ST(0) rounded for FIST: `None` where an unmasked exception stops the store, else the integer, `None` within for
/// the integer indefinite of a masked IE. An inexact result raises PE, with C1 telling whether the magnitude was
/// rounded up; a denormal raises no DE.
#[expect(clippy::option_option, reason = "the outer option is whether anything is stored")]
fn round_for_store(x87: &mut X87, value: u128, bits: u32) -> Option<Option<u128>> {
    let class = classify(value);
    if matches!(class, Class::QuietNaN | Class::SignalingNaN | Class::Infinity | Class::Unsupported) {
        x87.raise(INVALID);
        x87.set_code(C1, false);
        return (!x87.is_unmasked(INVALID)).then_some(None);
    }
    let Some((integer, inexact, up)) = round_to_integer(value, Rounding::from_control(x87.control), bits) else {
        x87.raise(INVALID);
        x87.set_code(C1, false);
        return (!x87.is_unmasked(INVALID)).then_some(None);
    };
    if inexact {
        x87.raise(PRECISION);
        x87.set_code(C1, up);
    } else {
        x87.set_code(C1, false);
    }
    Some(Some(integer as u64 as u128 & (u128::MAX >> (128 - bits))))
}

/// FOP's value for the form at the address: the low three bits of the opcode byte after the prefixes, then the next.
fn read_opcode(memory: Memory<'_, '_>, address: u64) -> u16 {
    let bytes: [u8; 15] = read_bytes(memory, address);
    let start = bytes.iter().take_while(|byte| LEGACY_PREFIXES.contains(byte) || is_rex(**byte)).count();
    u16::from(bytes[start] & OPCODE_LOW_BITS) << 8 | u16::from(bytes.get(start + 1).copied().unwrap_or(0))
}
