//! Each formula against cases worked out by hand from the manual.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};

use crate::encoding::{
    COND, Field, IMM7, IMM9, IMM12, IMM16, RD, RDVL_IMM6, RM, RN, RT, RT2, SHIFT, SHIFT_IMM6,
};
use crate::*;

/// A 4 KiB page.
const PAGE: u64 = 1 << 12;
const CODE: u64 = PAGE;
const DATA: u64 = 2 * PAGE;
const UPPER_DATA: u64 = 3 * PAGE;
const SIGN_BIT: u64 = 1 << 63;

const LOAD: PreciseInstruction<Infallible> = PreciseInstruction::LdrImmediateUnsignedOffset64 {
    misaligned_stack_pointer: MisalignedStackPointer::Faults,
};
const STORE: PreciseInstruction<Infallible> = PreciseInstruction::StrImmediateUnsignedOffset64 {
    misaligned_stack_pointer: MisalignedStackPointer::Faults,
    across_fault: StoreAcrossFault::WritesNothing,
};

const fn to_store_pair(across_fault: StoreAcrossFault) -> PreciseInstruction<Infallible> {
    PreciseInstruction::StpSignedOffset64 {
        misaligned_stack_pointer: MisalignedStackPointer::Faults,
        across_fault,
    }
}

const fn to_post_index_load(
    into_base: LoadIntoBaseWithWriteback,
) -> PreciseInstruction<Infallible> {
    PreciseInstruction::LdrImmediatePostIndex64 {
        misaligned_stack_pointer: MisalignedStackPointer::Faults,
        into_base,
    }
}

const READ_WRITE: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::ReadWrite,
    instruction_fetch: InstructionFetch::Forbidden,
};
const READ_ONLY: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::ReadOnly,
    instruction_fetch: InstructionFetch::Forbidden,
};
const FORBIDDEN: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::Forbidden,
    instruction_fetch: InstructionFetch::Forbidden,
};

fn encode(instruction: Instruction<Infallible>, fields: &[(Field, u32)]) -> u32 {
    fields
        .iter()
        .fold(instruction.to_fixed_bits().value, |word, (field, value)| {
            word | field.to_bits(*value)
        })
}

/// The state about to run the word, with every register zero and the data pages as given.
fn to_state(
    word: u32,
    lower: PagePermission,
    upper: PagePermission,
) -> ExecutionState<'static, Arm64<User>> {
    let readable_and_executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let data: Vec<u8> = (0..2 * PAGE).map(|offset| offset as u8).collect();
    ExecutionState {
        processor: Processor {
            program_counter: CODE,
            ..ZERO_PROCESSOR
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: CODE..CODE + PAGE,
                permission: readable_and_executable,
            },
            MemoryDelta::ValueAssignment {
                start: CODE,
                bytes: Cow::Owned(word.to_le_bytes().to_vec()),
            },
            MemoryDelta::ValueAssignment {
                start: DATA,
                bytes: Cow::Owned(data),
            },
            MemoryDelta::PermissionAssignment {
                addresses: DATA..UPPER_DATA,
                permission: lower,
            },
            MemoryDelta::PermissionAssignment {
                addresses: UPPER_DATA..UPPER_DATA + PAGE,
                permission: upper,
            },
        ],
    }
}

/// The value the data pages hold at an address: each byte is the low byte of its offset.
fn to_loaded(address: u64) -> u64 {
    u64::from_le_bytes(core::array::from_fn(|byte| {
        (address - DATA + byte as u64) as u8
    }))
}

/// Simulates and returns the state after and the memory mutations the instruction made.
fn run(
    precise: PreciseInstruction<Infallible>,
    before: ExecutionState<'static, Arm64<User>>,
) -> (Processor<()>, Vec<MemoryDelta<'static, Arm64<User>>>) {
    let given = before.memory_mutations.len();
    let Ok(mut after) = precise.simulate(before);
    let made = after.memory_mutations.split_off(given);
    (after.processor, made)
}

fn assert_completed(before: &Processor<()>, after: &Processor<()>) {
    assert_eq!(after.program_counter, before.program_counter + 4);
    assert_eq!(after.exception, None);
}

fn assert_raised(after: &Processor<()>, exception: Exception) {
    assert_eq!(after.program_counter, CODE);
    assert_eq!(after.exception, Some(exception));
}

const fn to_data_abort(status: FaultStatus, access: AccessDirection, address: u64) -> Exception {
    Exception::DataAbort {
        status,
        access,
        address,
    }
}

#[test]
fn every_encoding_is_its_fixed_bits_and_its_fields() {
    let operands: [(Instruction<Infallible>, &[Field]); 10] = [
        (
            Instruction::AddShiftedRegister64,
            &[RD, RN, RM, SHIFT, SHIFT_IMM6],
        ),
        (
            Instruction::AddsShiftedRegister64,
            &[RD, RN, RM, SHIFT, SHIFT_IMM6],
        ),
        (Instruction::Csel64, &[RD, RN, RM, COND]),
        (Instruction::LdrImmediateUnsignedOffset64, &[RT, RN, IMM12]),
        (Instruction::StrImmediateUnsignedOffset64, &[RT, RN, IMM12]),
        (Instruction::StpSignedOffset64, &[RT, RN, RT2, IMM7]),
        (Instruction::LdrImmediatePostIndex64, &[RT, RN, IMM9]),
        (Instruction::Brk, &[IMM16]),
        (Instruction::Rdvl, &[RD, RDVL_IMM6]),
        (Instruction::MrsCtrEl0, &[RT]),
    ];
    assert!(
        operands
            .iter()
            .map(|(instruction, _)| *instruction)
            .eq(Arm64::<User>::list_instructions())
    );
    for (instruction, fields) in operands {
        let fixed = instruction.to_fixed_bits();
        assert_eq!(fixed.value & !fixed.mask, 0, "{instruction:?}");
        let covered = fields.iter().try_fold(fixed.mask, |covered, field| {
            (covered & field.to_mask() == 0).then_some(covered | field.to_mask())
        });
        assert_eq!(covered, Some(u32::MAX), "{instruction:?}");
    }
}

#[test]
fn signed_fields_extend_their_sign() {
    assert_eq!(IMM9.read_signed(IMM9.to_bits(0x1FF)), -1);
    assert_eq!(IMM9.read_signed(IMM9.to_bits(0x100)), -256);
    assert_eq!(IMM9.read_signed(IMM9.to_bits(0xFF)), 255);
}

#[test]
fn add_shifts_its_second_operand() {
    let word = encode(
        Instruction::AddShiftedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2), (SHIFT, 0), (SHIFT_IMM6, 2)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = 5;
    before.processor.general_purpose_registers[2] = 3;
    let processor = before.processor.clone();
    let (after, written) = run(PreciseInstruction::AddShiftedRegister64, before);
    assert_completed(&processor, &after);
    assert_eq!(after.general_purpose_registers[0], 17);
    assert!(written.is_empty());
}

#[test]
fn add_shifts_arithmetically_wraps_and_keeps_the_flags() {
    let word = encode(
        Instruction::AddShiftedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2), (SHIFT, 2), (SHIFT_IMM6, 63)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = 1;
    before.processor.general_purpose_registers[2] = SIGN_BIT;
    before.processor.condition_flags = ConditionFlags::CARRY;
    let (after, _) = run(PreciseInstruction::AddShiftedRegister64, before);
    assert_eq!(after.general_purpose_registers[0], 0);
    assert_eq!(after.condition_flags, ConditionFlags::CARRY);
}

#[test]
fn add_with_the_reserved_shift_is_undefined() {
    let word = encode(
        Instruction::AddShiftedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2), (SHIFT, 3)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = 1;
    let (after, _) = run(PreciseInstruction::AddShiftedRegister64, before);
    assert_raised(&after, Exception::UndefinedInstruction);
    assert_eq!(after.general_purpose_registers[0], 0);
}

#[test]
fn add_reads_and_discards_the_zero_register() {
    let word = encode(
        Instruction::AddShiftedRegister64,
        &[(RD, 31), (RN, 31), (RM, 1)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = 7;
    before.processor.stack_pointer = 9;
    let processor = before.processor.clone();
    let (after, _) = run(PreciseInstruction::AddShiftedRegister64, before);
    assert_eq!(
        after.general_purpose_registers,
        processor.general_purpose_registers
    );
    assert_eq!(after.stack_pointer, 9);
}

#[test]
fn adds_sets_overflow_and_negative() {
    let word = encode(
        Instruction::AddsShiftedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = i64::MAX as u64;
    before.processor.general_purpose_registers[2] = 1;
    let (after, _) = run(PreciseInstruction::AddsShiftedRegister64, before);
    assert_eq!(after.general_purpose_registers[0], SIGN_BIT);
    assert_eq!(
        after.condition_flags,
        ConditionFlags::NEGATIVE | ConditionFlags::OVERFLOW
    );
}

#[test]
fn adds_sets_zero_and_carry() {
    let word = encode(
        Instruction::AddsShiftedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = u64::MAX;
    before.processor.general_purpose_registers[2] = 1;
    before.processor.condition_flags = ConditionFlags::NEGATIVE;
    let (after, _) = run(PreciseInstruction::AddsShiftedRegister64, before);
    assert_eq!(after.general_purpose_registers[0], 0);
    assert_eq!(
        after.condition_flags,
        ConditionFlags::ZERO | ConditionFlags::CARRY
    );
}

#[test]
fn csel_selects_by_its_condition() {
    // EQ is 0, NE is 1, GT is 12 and NV is 15.
    for (condition, flags, selected) in [
        (0, ConditionFlags::ZERO, 11),
        (1, ConditionFlags::ZERO, 22),
        (12, ConditionFlags::empty(), 11),
        (12, ConditionFlags::NEGATIVE, 22),
        (15, ConditionFlags::empty(), 11),
    ] {
        let word = encode(
            Instruction::Csel64,
            &[(RD, 0), (RN, 1), (RM, 2), (COND, condition)],
        );
        let mut before = to_state(word, READ_WRITE, READ_WRITE);
        before.processor.general_purpose_registers[1] = 11;
        before.processor.general_purpose_registers[2] = 22;
        before.processor.condition_flags = flags;
        let (after, _) = run(PreciseInstruction::Csel64, before);
        assert_eq!(
            after.general_purpose_registers[0], selected,
            "condition {condition}"
        );
        assert_eq!(after.condition_flags, flags);
    }
}

#[test]
fn ldr_loads_at_its_scaled_offset() {
    let word = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1), (IMM12, 2)],
    );
    let mut before = to_state(word, READ_ONLY, READ_ONLY);
    before.processor.general_purpose_registers[1] = DATA;
    let processor = before.processor.clone();
    let (after, written) = run(LOAD, before);
    assert_completed(&processor, &after);
    assert_eq!(after.general_purpose_registers[0], to_loaded(DATA + 16));
    assert!(written.is_empty());
}

#[test]
fn ldr_faults_on_an_unmapped_or_unreadable_page() {
    let word = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1), (IMM12, 1)],
    );
    for (permission, status) in [
        (
            PagePermission::Unmapped,
            FaultStatus::TranslationFaultLevel3,
        ),
        (FORBIDDEN, FaultStatus::PermissionFaultLevel3),
    ] {
        let mut before = to_state(word, permission, READ_WRITE);
        before.processor.general_purpose_registers[1] = DATA;
        before.processor.general_purpose_registers[0] = 5;
        let (after, _) = run(LOAD, before);
        assert_raised(
            &after,
            to_data_abort(status, AccessDirection::Read, DATA + 8),
        );
        assert_eq!(after.general_purpose_registers[0], 5);
    }
}

#[test]
fn ldr_from_a_misaligned_stack_pointer_faults() {
    let word = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 31)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.stack_pointer = DATA + 8;
    let (after, _) = run(LOAD, before);
    assert_raised(&after, Exception::StackPointerAlignmentFault);
}

#[test]
fn ldr_from_a_misaligned_stack_pointer_accesses_where_its_behavior_says() {
    let word = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 31)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.stack_pointer = DATA + 8;
    let (after, _) = run(
        PreciseInstruction::LdrImmediateUnsignedOffset64 {
            misaligned_stack_pointer: MisalignedStackPointer::Accesses,
        },
        before,
    );
    assert_eq!(after.exception, None);
    assert_eq!(after.general_purpose_registers[0], to_loaded(DATA + 8));
}

#[test]
fn str_across_a_fault_writes_what_its_behavior_says() {
    let word = encode(
        Instruction::StrImmediateUnsignedOffset64,
        &[(RT, 2), (RN, 1)],
    );
    let mut before = to_state(word, READ_WRITE, READ_ONLY);
    before.processor.general_purpose_registers[1] = UPPER_DATA - 4;
    before.processor.general_purpose_registers[2] = u64::MAX;
    let (after, written) = run(STORE, before.clone());
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::PermissionFaultLevel3,
            AccessDirection::Write,
            UPPER_DATA,
        ),
    );
    assert!(written.is_empty());
    let (after, written) = run(
        PreciseInstruction::StrImmediateUnsignedOffset64 {
            misaligned_stack_pointer: MisalignedStackPointer::Faults,
            across_fault: StoreAcrossFault::WritesBytesBelowFault,
        },
        before,
    );
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::PermissionFaultLevel3,
            AccessDirection::Write,
            UPPER_DATA,
        ),
    );
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: UPPER_DATA - 4,
                bytes: Cow::Owned([u8::MAX; 4].to_vec())
            }]
    );
}

#[test]
fn str_stores_and_faults_on_a_read_only_page() {
    let word = encode(
        Instruction::StrImmediateUnsignedOffset64,
        &[(RT, 2), (RN, 1), (IMM12, 1)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = DATA;
    before.processor.general_purpose_registers[2] = u64::MAX;
    let (_, written) = run(STORE, before.clone());
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: DATA + 8,
                bytes: Cow::Owned(u64::MAX.to_le_bytes().to_vec())
            }]
    );
    before
        .memory_mutations
        .push(MemoryDelta::PermissionAssignment {
            addresses: DATA..UPPER_DATA,
            permission: READ_ONLY,
        });
    let (after, written) = run(STORE, before);
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::PermissionFaultLevel3,
            AccessDirection::Write,
            DATA + 8,
        ),
    );
    assert!(written.is_empty());
}

#[test]
fn stp_across_a_fault_writes_what_its_behavior_says() {
    let word = encode(
        Instruction::StpSignedOffset64,
        &[(RT, 1), (RT2, 2), (RN, 3), (IMM7, 0x7F)],
    );
    let mut before = to_state(word, READ_WRITE, PagePermission::Unmapped);
    before.processor.general_purpose_registers[1] = 11;
    before.processor.general_purpose_registers[2] = 22;
    // An offset of -8 puts the second register at the first byte of the unmapped page.
    before.processor.general_purpose_registers[3] = UPPER_DATA;
    let (after, written) = run(
        to_store_pair(StoreAcrossFault::WritesNothing),
        before.clone(),
    );
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::TranslationFaultLevel3,
            AccessDirection::Write,
            UPPER_DATA,
        ),
    );
    assert!(written.is_empty());
    let (after, written) = run(
        to_store_pair(StoreAcrossFault::WritesBytesBelowFault),
        before,
    );
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::TranslationFaultLevel3,
            AccessDirection::Write,
            UPPER_DATA,
        ),
    );
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: UPPER_DATA - 8,
                bytes: Cow::Owned(11_u64.to_le_bytes().to_vec())
            }]
    );
}

#[test]
fn stp_stores_both_registers_in_order() {
    let word = encode(
        Instruction::StpSignedOffset64,
        &[(RT, 1), (RT2, 31), (RN, 3), (IMM7, 1)],
    );
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = 11;
    before.processor.general_purpose_registers[3] = DATA;
    let (_, written) = run(to_store_pair(StoreAcrossFault::WritesNothing), before);
    let mut expected = 11_u64.to_le_bytes().to_vec();
    expected.extend_from_slice(&0_u64.to_le_bytes());
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: DATA + 8,
                bytes: Cow::Owned(expected)
            }]
    );
}

#[test]
fn ldr_post_index_loads_then_writes_back() {
    let word = encode(
        Instruction::LdrImmediatePostIndex64,
        &[(RT, 0), (RN, 1), (IMM9, 0x1F8)],
    );
    let mut before = to_state(word, READ_ONLY, READ_ONLY);
    before.processor.general_purpose_registers[1] = DATA + 8;
    let (after, _) = run(
        to_post_index_load(LoadIntoBaseWithWriteback::Undefined),
        before,
    );
    assert_eq!(after.general_purpose_registers[0], to_loaded(DATA + 8));
    assert_eq!(after.general_purpose_registers[1], DATA);
}

#[test]
fn ldr_post_index_into_its_base_takes_each_choice() {
    let word = encode(
        Instruction::LdrImmediatePostIndex64,
        &[(RT, 1), (RN, 1), (IMM9, 8)],
    );
    let mut before = to_state(word, READ_ONLY, READ_ONLY);
    before.processor.general_purpose_registers[1] = DATA;
    let base = |choice| run(to_post_index_load(choice), before.clone()).0;
    assert_eq!(
        base(LoadIntoBaseWithWriteback::WritebackSuppressed).general_purpose_registers[1],
        to_loaded(DATA)
    );
    assert_eq!(
        base(LoadIntoBaseWithWriteback::WritebackOverwritesLoad).general_purpose_registers[1],
        DATA + 8
    );
    assert_raised(
        &base(LoadIntoBaseWithWriteback::Undefined),
        Exception::UndefinedInstruction,
    );
    let skipped = base(LoadIntoBaseWithWriteback::NoOperation);
    assert_completed(&before.processor, &skipped);
    assert_eq!(skipped.general_purpose_registers[1], DATA);
}

#[test]
fn brk_raises_a_breakpoint_with_its_comment() {
    let word = encode(Instruction::Brk, &[(IMM16, 0x1234)]);
    let (after, _) = run(
        PreciseInstruction::Brk,
        to_state(word, READ_WRITE, READ_WRITE),
    );
    assert_raised(&after, Exception::Breakpoint { comment: 0x1234 });
}

#[test]
fn rdvl_reads_a_multiple_of_the_vector_length_or_raises_its_exception() {
    let word = encode(Instruction::Rdvl, &[(RD, 4), (RDVL_IMM6, 0x3E)]);
    let reading = VectorLengthReading::Reads(VectorLength::Bits128);
    let (after, _) = run(
        PreciseInstruction::Rdvl(reading),
        to_state(word, READ_WRITE, READ_WRITE),
    );
    assert_eq!(after.general_purpose_registers[4], (-32_i64) as u64);
    let (after, _) = run(
        PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
        to_state(word, READ_WRITE, READ_WRITE),
    );
    assert_raised(&after, Exception::UndefinedInstruction);
    let (after, _) = run(
        PreciseInstruction::Rdvl(VectorLengthReading::SmeTrapped(SmeTrapCode::Disabled)),
        to_state(word, READ_WRITE, READ_WRITE),
    );
    assert_raised(&after, Exception::SmeTrap(SmeTrapCode::Disabled));
}

#[test]
fn an_instruction_on_a_page_without_instruction_fetch_faults_before_it_runs() {
    let word = encode(Instruction::Brk, &[]);
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before
        .memory_mutations
        .push(MemoryDelta::PermissionAssignment {
            addresses: CODE..CODE + PAGE,
            permission: READ_ONLY,
        });
    let (after, _) = run(PreciseInstruction::Brk, before);
    assert_raised(
        &after,
        Exception::InstructionAbort {
            status: FaultStatus::PermissionFaultLevel3,
            address: CODE,
        },
    );
}

#[test]
fn mrs_reads_the_cache_type_the_chip_fixes() {
    let word = encode(Instruction::MrsCtrEl0, &[(RT, 3)]);
    let before = to_state(word, READ_WRITE, READ_WRITE);
    let processor = before.processor.clone();
    let (after, _) = run(
        PreciseInstruction::MrsCtrEl0 {
            cache_type: 0x8444_C004,
        },
        before,
    );
    assert_completed(&processor, &after);
    assert_eq!(after.general_purpose_registers[3], 0x8444_C004);
}

/// The first address above those a program can use.
const ADDRESSABLE_END: u64 = Arm64::<User>::ADDRESSABLE.end;

/// The state about to run the word with its last addressable page writable as well.
fn to_state_with_last_page(word: u32) -> ExecutionState<'static, Arm64<User>> {
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before
        .memory_mutations
        .push(MemoryDelta::PermissionAssignment {
            addresses: ADDRESSABLE_END - PAGE..ADDRESSABLE_END,
            permission: READ_WRITE,
        });
    before
}

#[test]
fn str_across_the_last_addressable_byte_faults_at_level_0() {
    let word = encode(
        Instruction::StrImmediateUnsignedOffset64,
        &[(RT, 2), (RN, 1)],
    );
    let mut before = to_state_with_last_page(word);
    before.processor.general_purpose_registers[1] = ADDRESSABLE_END - 4;
    before.processor.general_purpose_registers[2] = u64::MAX;
    let fault = to_data_abort(
        FaultStatus::TranslationFaultLevel0,
        AccessDirection::Write,
        ADDRESSABLE_END,
    );
    let (after, written) = run(STORE, before.clone());
    assert_raised(&after, fault);
    assert!(written.is_empty());
    let (after, written) = run(
        PreciseInstruction::StrImmediateUnsignedOffset64 {
            misaligned_stack_pointer: MisalignedStackPointer::Faults,
            across_fault: StoreAcrossFault::WritesBytesBelowFault,
        },
        before,
    );
    assert_raised(&after, fault);
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: ADDRESSABLE_END - 4,
                bytes: Cow::Owned([u8::MAX; 4].to_vec())
            }]
    );
}

#[test]
fn ldr_across_the_last_addressable_byte_faults_at_level_0() {
    let word = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1)],
    );
    let mut before = to_state_with_last_page(word);
    before.processor.general_purpose_registers[1] = ADDRESSABLE_END - 4;
    let (after, _) = run(LOAD, before);
    assert_raised(
        &after,
        to_data_abort(
            FaultStatus::TranslationFaultLevel0,
            AccessDirection::Read,
            ADDRESSABLE_END,
        ),
    );
}

#[test]
fn an_instruction_above_the_addressable_addresses_faults_at_level_0_whatever_its_page_holds() {
    let word = encode(Instruction::Brk, &[]);
    let mut before = to_state(word, READ_WRITE, READ_WRITE);
    before.processor.program_counter = ADDRESSABLE_END;
    before.memory_mutations.extend([
        MemoryDelta::PermissionAssignment {
            addresses: ADDRESSABLE_END..ADDRESSABLE_END + PAGE,
            permission: PagePermission::Mapped {
                data_access: DataAccess::ReadOnly,
                instruction_fetch: InstructionFetch::Allowed,
            },
        },
        MemoryDelta::ValueAssignment {
            start: ADDRESSABLE_END,
            bytes: Cow::Owned(word.to_le_bytes().to_vec()),
        },
    ]);
    let (after, _) = run(PreciseInstruction::Brk, before);
    assert_eq!(
        after.exception,
        Some(Exception::InstructionAbort {
            status: FaultStatus::TranslationFaultLevel0,
            address: ADDRESSABLE_END,
        })
    );
    assert_eq!(after.program_counter, ADDRESSABLE_END);
}

/// The two laws a proof takes on trust for this architecture, `equality_is_identity` and `clone_is_identity`, on
/// processors that differ from the zero processor in one field each and on every permission: `==` holds exactly when
/// every field is the same, and a clone is equal to what it was cloned from.
#[test]
fn equality_and_clone_of_processors_and_permissions_are_identity() {
    let mut processors = vec![ZERO_PROCESSOR];
    for register in 0..GENERAL_PURPOSE_REGISTERS {
        let mut processor = ZERO_PROCESSOR;
        processor.general_purpose_registers[register] = u64::MAX;
        processors.push(processor);
    }
    for register in 0..VECTOR_REGISTERS {
        let mut processor = ZERO_PROCESSOR;
        processor.vector_registers[register] = u128::MAX;
        processors.push(processor);
    }
    let one_field: [fn(&mut Processor<()>); 7] = [
        |processor| processor.stack_pointer = u64::MAX,
        |processor| processor.program_counter = u64::MAX,
        |processor| processor.condition_flags = ConditionFlags::all(),
        |processor| processor.floating_point_control = u64::MAX,
        |processor| processor.floating_point_status = u64::MAX,
        |processor| processor.software_thread_id = u64::MAX,
        |processor| processor.exception = Some(Exception::UndefinedInstruction),
    ];
    for change in one_field {
        let mut processor = ZERO_PROCESSOR;
        change(&mut processor);
        processors.push(processor);
    }
    for first in &processors {
        assert!(*first == first.clone());
        for second in &processors {
            assert_eq!(
                *first == *second,
                alloc::format!("{first:?}") == alloc::format!("{second:?}")
            );
        }
    }
    let mut permissions = vec![PagePermission::Unmapped];
    for data_access in [
        DataAccess::Forbidden,
        DataAccess::ReadOnly,
        DataAccess::ReadWrite,
    ] {
        for instruction_fetch in [InstructionFetch::Forbidden, InstructionFetch::Allowed] {
            permissions.push(PagePermission::Mapped {
                data_access,
                instruction_fetch,
            });
        }
    }
    for first in &permissions {
        for second in &permissions {
            assert_eq!(
                *first == *second,
                alloc::format!("{first:?}") == alloc::format!("{second:?}")
            );
        }
    }
}
