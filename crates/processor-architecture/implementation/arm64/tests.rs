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

/// No choice for any case the manual leaves open.
const NO_CHOICES: LoadStoreChoices = LoadStoreChoices {
    misaligned_stack_pointer: None,
    across_fault: None,
    load_into_base: None,
    store_of_base: None,
    load_pair_into_one_register: None,
    failed_store_exclusive: None,
    store_exclusive_overlap: None,
    misaligned_exclusive: None,
    fault_address_tag: None,
};

const fn to_load_store(
    instruction: Instruction<Infallible>,
    choices: LoadStoreChoices,
) -> PreciseInstruction<Infallible> {
    match LoadStoreInstruction::new(instruction) {
        Some(instruction) => PreciseInstruction::LoadStore {
            instruction,
            choices,
        },
        None => panic!("a load or store"),
    }
}

const LOAD: PreciseInstruction<Infallible> = to_load_store(
    Instruction::LdrImmediateUnsignedOffset64,
    LoadStoreChoices {
        misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
        ..NO_CHOICES
    },
);
const STORE: PreciseInstruction<Infallible> = to_load_store(
    Instruction::StrImmediateUnsignedOffset64,
    LoadStoreChoices {
        misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
        across_fault: Some(StoreAcrossFault::WritesNothing),
        ..NO_CHOICES
    },
);

const fn to_store_pair(across_fault: StoreAcrossFault) -> PreciseInstruction<Infallible> {
    to_load_store(
        Instruction::StpSignedOffset64,
        LoadStoreChoices {
            misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
            across_fault: Some(across_fault),
            ..NO_CHOICES
        },
    )
}

const fn to_post_index_load(
    into_base: LoadIntoBaseWithWriteback,
) -> PreciseInstruction<Infallible> {
    to_load_store(
        Instruction::LdrImmediatePostIndex64,
        LoadStoreChoices {
            misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
            load_into_base: Some(into_base),
            ..NO_CHOICES
        },
    )
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

fn to_documented(instruction: Instruction<Infallible>) -> PreciseInstruction<Infallible> {
    match DocumentedInstruction::new(instruction) {
        Some(documented) => PreciseInstruction::Documented(documented),
        None => panic!("{instruction:?} has one documented behavior"),
    }
}

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
    let Ok(mut after) = precise.simulate_state(before);
    let made = after.memory_mutations.split_off(given);
    (after.processor.processor, made)
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
    for (instruction, _) in operands {
        assert!(
            Arm64::<User>::list_instructions().any(|listed| listed == instruction),
            "{instruction:?} is listed"
        );
    }
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
    let (after, written) = run(to_documented(Instruction::AddShiftedRegister64), before);
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
    let (after, _) = run(to_documented(Instruction::AddShiftedRegister64), before);
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
    let (after, _) = run(to_documented(Instruction::AddShiftedRegister64), before);
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
    let (after, _) = run(to_documented(Instruction::AddShiftedRegister64), before);
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
    let (after, _) = run(to_documented(Instruction::AddsShiftedRegister64), before);
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
    let (after, _) = run(to_documented(Instruction::AddsShiftedRegister64), before);
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
        let (after, _) = run(to_documented(Instruction::Csel64), before);
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

/// A tag in the top byte of a lower-half address, which TBI0 leaves out of translation.
const TAG: u64 = 0xA5 << 56;

#[test]
fn ldr_and_str_leave_a_tag_out_and_keep_it_in_the_fault_address_as_chosen() {
    let load = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1), (IMM12, 2)],
    );
    let mut before = to_state(load, READ_ONLY, READ_ONLY);
    before.processor.general_purpose_registers[1] = DATA | TAG;
    let (after, _) = run(LOAD, before);
    assert_eq!(after.general_purpose_registers[0], to_loaded(DATA + 16));
    let store = encode(
        Instruction::StrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1)],
    );
    let mut before = to_state(store, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = DATA | TAG;
    before.processor.general_purpose_registers[0] = 7;
    let (_, written) = run(STORE, before);
    assert!(
        written
            == [MemoryDelta::ValueAssignment {
                start: DATA,
                bytes: Cow::Owned(7_u64.to_le_bytes().to_vec())
            }]
    );
    for (tag, kept) in [
        (FaultAddressTag::KeepsTopByte, TAG),
        (FaultAddressTag::ClearsTopNibble, 0x05 << 56),
    ] {
        let choices = LoadStoreChoices {
            fault_address_tag: Some(tag),
            ..NO_CHOICES
        };
        let mut before = to_state(load, PagePermission::Unmapped, READ_ONLY);
        before.processor.general_purpose_registers[1] = DATA | TAG;
        let (after, _) = run(
            to_load_store(Instruction::LdrImmediateUnsignedOffset64, choices),
            before,
        );
        assert_raised(
            &after,
            to_data_abort(
                FaultStatus::TranslationFaultLevel3,
                AccessDirection::Read,
                (DATA + 16) | kept,
            ),
        );
    }
}

#[test]
fn an_exclusive_load_inside_16_bytes_faults_as_its_behavior_says() {
    let word = encode(Instruction::Ldxr64, &[(RT, 0), (RN, 1)]);
    for (choice, faults) in [
        (MisalignedExclusive::AlignmentFault, true),
        (MisalignedExclusive::WithinSixteenBytes, false),
    ] {
        let choices = LoadStoreChoices {
            misaligned_exclusive: Some(choice),
            ..NO_CHOICES
        };
        let mut before = to_state(word, READ_ONLY, READ_ONLY);
        before.processor.general_purpose_registers[1] = DATA + 4;
        let (after, _) = run(to_load_store(Instruction::Ldxr64, choices), before);
        let fault = to_data_abort(FaultStatus::AlignmentFault, AccessDirection::Read, DATA + 4);
        assert_eq!(after.exception, faults.then_some(fault));
        // Across 16 bytes every behavior faults.
        let mut before = to_state(word, READ_ONLY, READ_ONLY);
        before.processor.general_purpose_registers[1] = DATA + 12;
        let (after, _) = run(to_load_store(Instruction::Ldxr64, choices), before);
        assert_eq!(
            after.exception,
            Some(to_data_abort(
                FaultStatus::AlignmentFault,
                AccessDirection::Read,
                DATA + 12
            ))
        );
    }
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
        to_load_store(
            Instruction::LdrImmediateUnsignedOffset64,
            LoadStoreChoices {
                misaligned_stack_pointer: Some(MisalignedStackPointer::Accesses),
                ..NO_CHOICES
            },
        ),
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
        to_load_store(
            Instruction::StrImmediateUnsignedOffset64,
            LoadStoreChoices {
                misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
                across_fault: Some(StoreAcrossFault::WritesBytesBelowFault),
                ..NO_CHOICES
            },
        ),
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

/// Runs a documented instruction with the given fields and registers, from the given flags.
fn run_documented(
    instruction: Instruction<Infallible>,
    fields: &[(Field, u32)],
    registers: &[(usize, u64)],
    flags: ConditionFlags,
) -> Processor<()> {
    let mut before = to_state(encode(instruction, fields), READ_WRITE, READ_WRITE);
    for &(number, value) in registers {
        before.processor.general_purpose_registers[number] = value;
    }
    before.processor.condition_flags = flags;
    run(to_documented(instruction), before).0
}

/// The fields of the integer instructions below, at their places in the manual's encodings.
const EXTEND_TYPE: Field = Field::new(13, 3);
const EXTEND_AMOUNT: Field = Field::new(10, 3);
const IMMEDIATE_SHIFT_BIT: Field = Field::new(22, 1);
const IMMR: Field = Field::new(16, 6);
const IMMS: Field = Field::new(10, 6);
const HW: Field = Field::new(21, 2);
const IMM16_WIDE: Field = Field::new(5, 16);
const NZCV: Field = Field::new(0, 4);
const RA: Field = Field::new(10, 5);

#[test]
fn every_modeled_instruction_has_its_operands_in_fields_beside_its_fixed_bits() {
    for instruction in Arm64::<User>::list_instructions().filter(|instruction| {
        Arm64::<User>::list_known_behaviors(*instruction)
            .next()
            .is_some()
    }) {
        let Some(record) = instruction.to_record() else {
            continue;
        };
        if DocumentedInstruction::new(instruction).is_none()
            && LoadStoreInstruction::new(instruction).is_none()
        {
            continue;
        }
        let mut covered = record.fixed_bits.mask | record.should_be.mask;
        for operand in record.operands {
            let fields: Vec<Field> = match *operand {
                operand::Operand::GeneralPurposeRegister { number, .. }
                | operand::Operand::GeneralPurposeRegisterPair { number, .. }
                | operand::Operand::FloatingPointRegister { number, .. }
                | operand::Operand::VectorRegister { number, .. }
                | operand::Operand::Ignored(number) => vec![number],
                operand::Operand::SplitImmediate(fields) => fields.to_vec(),
                operand::Operand::ShiftedRegister {
                    number,
                    shift_type,
                    amount,
                    ..
                } => vec![number, shift_type, amount],
                operand::Operand::ExtendedRegister {
                    number,
                    extend_type,
                    amount,
                    ..
                } => vec![number, extend_type, amount],
                operand::Operand::ArithmeticImmediate { value, shift } => vec![value, shift],
                operand::Operand::BitmaskImmediate { field, .. }
                | operand::Operand::WideImmediateShift { field, .. }
                | operand::Operand::UnsignedImmediate(field)
                | operand::Operand::SignedImmediate(field)
                | operand::Operand::Condition(field) => vec![field],
                operand::Operand::ProgramCounterOffset { fields, .. } => fields.to_vec(),
                operand::Operand::AddressBase { number, .. } => vec![number],
                operand::Operand::AddressOffset { field, .. } => vec![field],
                operand::Operand::RegisterOffset {
                    number,
                    signed,
                    doubleword,
                    shift,
                } => [Some(number), signed, doubleword, Some(shift)]
                    .into_iter()
                    .flatten()
                    .collect(),
                operand::Operand::Unmodeled => panic!("{instruction:?} has an unmodeled operand"),
            };
            for field in fields {
                assert_eq!(covered & field.to_mask(), 0, "{instruction:?}");
                covered |= field.to_mask();
            }
        }
        assert_eq!(covered, u32::MAX, "{instruction:?}");
    }
}

#[test]
fn add_extends_and_shifts_its_register_and_may_write_the_stack_pointer() {
    // add x0, x1, w2, sxtw #2
    let after = run_documented(
        Instruction::AddExtendedRegister64,
        &[
            (RD, 0),
            (RN, 1),
            (RM, 2),
            (EXTEND_TYPE, 0b110),
            (EXTEND_AMOUNT, 2),
        ],
        &[(1, 100), (2, 0x1234_5678_FFFF_FFFF)],
        ConditionFlags::empty(),
    );
    assert_eq!(after.general_purpose_registers[0], 96);
    // add sp, x1, #1, lsl #12
    let after = run_documented(
        Instruction::AddImmediate64,
        &[(RD, 31), (RN, 1), (IMM12, 1), (IMMEDIATE_SHIFT_BIT, 1)],
        &[(1, 5)],
        ConditionFlags::empty(),
    );
    assert_eq!(after.stack_pointer, 5 + 4096);
    // An extend shift above four is unallocated.
    let after = run_documented(
        Instruction::AddExtendedRegister64,
        &[(RD, 0), (RN, 1), (RM, 2), (EXTEND_AMOUNT, 5)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(after.exception, Some(Exception::UndefinedInstruction));
}

#[test]
fn a_32_bit_result_clears_the_upper_half() {
    let after = run_documented(
        Instruction::AddShiftedRegister32,
        &[(RD, 0), (RN, 1), (RM, 2)],
        &[(0, u64::MAX), (1, u64::MAX), (2, 1)],
        ConditionFlags::empty(),
    );
    assert_eq!(after.general_purpose_registers[0], 0);
    // A 32-bit shift of 32 or more is unallocated.
    let after = run_documented(
        Instruction::AddShiftedRegister32,
        &[(RD, 0), (RN, 1), (RM, 2), (SHIFT_IMM6, 32)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(after.exception, Some(Exception::UndefinedInstruction));
}

#[test]
fn logical_immediates_decode_their_bitmask() {
    // and x0, x1, #0x5555555555555555: N 0, immr 0, imms 0b111100.
    let after = run_documented(
        Instruction::AndImmediate64,
        &[(RD, 0), (RN, 1), (IMMS, 0b11_1100)],
        &[(1, u64::MAX)],
        ConditionFlags::empty(),
    );
    assert_eq!(after.general_purpose_registers[0], 0x5555_5555_5555_5555);
    // imms 0b111110 with N 0 gives no element size.
    let after = run_documented(
        Instruction::AndImmediate64,
        &[(RD, 0), (RN, 1), (IMMS, 0b11_1110)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(after.exception, Some(Exception::UndefinedInstruction));
}

#[test]
fn bitfield_moves_extract_insert_and_extend() {
    let value = 0x0123_4567_89AB_CDEF;
    // ubfx x0, x1, #8, #4 is UBFM with immr 8 and imms 11.
    let ubfx = run_documented(
        Instruction::UbfmImmediate64,
        &[(RD, 0), (RN, 1), (IMMR, 8), (IMMS, 11)],
        &[(1, value)],
        ConditionFlags::empty(),
    );
    assert_eq!(ubfx.general_purpose_registers[0], 0xD);
    // lsl x0, x1, #4 is UBFM with immr 60 and imms 59.
    let lsl = run_documented(
        Instruction::UbfmImmediate64,
        &[(RD, 0), (RN, 1), (IMMR, 60), (IMMS, 59)],
        &[(1, value)],
        ConditionFlags::empty(),
    );
    assert_eq!(lsl.general_purpose_registers[0], value << 4);
    // sxtb w0, w1 is SBFM with immr 0 and imms 7.
    let sxtb = run_documented(
        Instruction::SbfmImmediate32,
        &[
            (RD, 0),
            (RN, 1),
            (Field::new(16, 5), 0),
            (Field::new(10, 5), 7),
        ],
        &[(1, 0x80)],
        ConditionFlags::empty(),
    );
    assert_eq!(sxtb.general_purpose_registers[0], 0xFFFF_FF80);
    // bfi x0, x1, #8, #4 is BFM with immr 56 and imms 3.
    let bfi = run_documented(
        Instruction::BfmImmediate64,
        &[(RD, 0), (RN, 1), (IMMR, 56), (IMMS, 3)],
        &[(0, u64::MAX), (1, 0x5)],
        ConditionFlags::empty(),
    );
    assert_eq!(bfi.general_purpose_registers[0], 0xFFFF_FFFF_FFFF_F5FF);
}

#[test]
fn extract_takes_bits_across_two_registers() {
    let after = run_documented(
        Instruction::Extr64,
        &[(RD, 0), (RN, 1), (RM, 2), (IMMS, 8)],
        &[(1, 0x11), (2, 0x2233_4455_6677_8899)],
        ConditionFlags::empty(),
    );
    assert_eq!(after.general_purpose_registers[0], 0x1122_3344_5566_7788);
}

#[test]
fn move_wide_keeps_or_replaces_and_reserves_upper_halves_of_32_bits() {
    let movk = run_documented(
        Instruction::MovkImmediate64,
        &[(RD, 0), (IMM16_WIDE, 0x1234), (HW, 1)],
        &[(0, u64::MAX)],
        ConditionFlags::empty(),
    );
    assert_eq!(movk.general_purpose_registers[0], 0xFFFF_FFFF_1234_FFFF);
    // Arm's encoding of the 32-bit MOVN fixes hw<1> to zero: a word with hw 1x is no MOVN, but unallocated.
    let fixed = Instruction::MovnImmediate32.to_fixed_bits();
    let reserved_shift = encode(Instruction::MovnImmediate32, &[(RD, 0), (HW, 2)]);
    assert_ne!(reserved_shift & fixed.mask, fixed.value);
}

#[test]
fn count_divide_and_multiply_at_their_edges() {
    let cls = run_documented(
        Instruction::Cls32,
        &[(RD, 0), (RN, 1)],
        &[(1, 0xFFFF_FFFF)],
        ConditionFlags::empty(),
    );
    assert_eq!(cls.general_purpose_registers[0], 31);
    let sdiv = run_documented(
        Instruction::Sdiv64,
        &[(RD, 0), (RN, 1), (RM, 2)],
        &[(1, SIGN_BIT), (2, u64::MAX)],
        ConditionFlags::empty(),
    );
    assert_eq!(sdiv.general_purpose_registers[0], SIGN_BIT);
    let udiv = run_documented(
        Instruction::Udiv32,
        &[(RD, 0), (RN, 1), (RM, 2)],
        &[(1, 7), (2, 0)],
        ConditionFlags::empty(),
    );
    assert_eq!(udiv.general_purpose_registers[0], 0);
    let smulh = run_documented(
        Instruction::Smulh64,
        &[(RD, 0), (RN, 1), (RM, 2), (RA, 31)],
        &[(1, SIGN_BIT), (2, 2)],
        ConditionFlags::empty(),
    );
    assert_eq!(smulh.general_purpose_registers[0], u64::MAX);
    let smsubl = run_documented(
        Instruction::Smsubl,
        &[(RD, 0), (RN, 1), (RM, 2), (RA, 3)],
        &[(1, 0xFFFF_FFFF), (2, 3), (3, 10)],
        ConditionFlags::empty(),
    );
    assert_eq!(smsubl.general_purpose_registers[0], 13);
    let rev16 = run_documented(
        Instruction::Rev16Register64,
        &[(RD, 0), (RN, 1)],
        &[(1, 0x0102_0304_0506_0708)],
        ConditionFlags::empty(),
    );
    assert_eq!(rev16.general_purpose_registers[0], 0x0201_0403_0605_0807);
}

#[test]
fn crc32_accumulates_without_inverting() {
    // CRC-32 of "a" is 0xE8B7BE43 and CRC-32C's is 0xC1D04330, each from all ones and inverted at the end.
    let crc = run_documented(
        Instruction::Crc32b,
        &[(RD, 0), (RN, 1), (RM, 2)],
        &[(1, 0xFFFF_FFFF), (2, u64::from(b'a'))],
        ConditionFlags::empty(),
    );
    assert_eq!(
        crc.general_purpose_registers[0],
        u64::from(!0xE8B7_BE43_u32)
    );
    let crc = run_documented(
        Instruction::Crc32cb,
        &[(RD, 0), (RN, 1), (RM, 2)],
        &[(1, 0xFFFF_FFFF), (2, u64::from(b'a'))],
        ConditionFlags::empty(),
    );
    assert_eq!(
        crc.general_purpose_registers[0],
        u64::from(!0xC1D0_4330_u32)
    );
}

#[test]
fn flag_instructions_compare_or_insert() {
    // ccmp x1, x2, #0b0010, eq
    let unequal = run_documented(
        Instruction::Ccmp64,
        &[(RN, 1), (RM, 2), (NZCV, 0b0010), (COND, 0)],
        &[(1, 5), (2, 5)],
        ConditionFlags::empty(),
    );
    assert_eq!(unequal.condition_flags, ConditionFlags::CARRY);
    let equal = run_documented(
        Instruction::Ccmp64,
        &[(RN, 1), (RM, 2), (NZCV, 0b0010), (COND, 0)],
        &[(1, 5), (2, 5)],
        ConditionFlags::ZERO,
    );
    assert_eq!(
        equal.condition_flags,
        ConditionFlags::ZERO | ConditionFlags::CARRY
    );
    // rmif x1, #4, #0b1000
    let rmif = run_documented(
        Instruction::Rmif,
        &[(RN, 1), (Field::new(15, 6), 4), (NZCV, 0b1000)],
        &[(1, 0x80)],
        ConditionFlags::CARRY,
    );
    assert_eq!(
        rmif.condition_flags,
        ConditionFlags::NEGATIVE | ConditionFlags::CARRY
    );
    let setf8 = run_documented(
        Instruction::Setf8,
        &[(RN, 1)],
        &[(1, 0x80)],
        ConditionFlags::CARRY,
    );
    assert_eq!(
        setf8.condition_flags,
        ConditionFlags::NEGATIVE | ConditionFlags::CARRY | ConditionFlags::OVERFLOW
    );
}

#[test]
fn branches_go_to_their_label_or_register_and_link() {
    // b . - 4 is imm26 all ones.
    let b = run_documented(
        Instruction::B,
        &[(Field::new(0, 26), 0x3FF_FFFF)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(b.program_counter, CODE - 4);
    let bl = run_documented(
        Instruction::Bl,
        &[(Field::new(0, 26), 2)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(
        (bl.program_counter, bl.general_purpose_registers[30]),
        (CODE + 8, CODE + 4)
    );
    // blr x30 branches to the link register's value before it is overwritten.
    let blr = run_documented(
        Instruction::Blr,
        &[(RN, 30)],
        &[(30, 0x8000)],
        ConditionFlags::empty(),
    );
    assert_eq!(
        (blr.program_counter, blr.general_purpose_registers[30]),
        (0x8000, CODE + 4)
    );
    // TBI0 leaves out the top byte of an address whose bit 55 is clear, and TBI1 is off.
    let tagged = run_documented(
        Instruction::Br,
        &[(RN, 1)],
        &[(1, 0xFF00_0000_0000_1000)],
        ConditionFlags::empty(),
    );
    assert_eq!(tagged.program_counter, 0x1000);
    let upper = run_documented(
        Instruction::Ret,
        &[(RN, 1)],
        &[(1, u64::MAX)],
        ConditionFlags::empty(),
    );
    assert_eq!(upper.program_counter, u64::MAX);
}

#[test]
fn conditional_branches_fall_through_when_not_taken() {
    let label = Field::new(5, 19);
    let not_taken = run_documented(
        Instruction::BCond,
        &[(label, 4), (Field::new(0, 4), 0)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(not_taken.program_counter, CODE + 4);
    let taken = run_documented(
        Instruction::BCond,
        &[(label, 4), (Field::new(0, 4), 0)],
        &[],
        ConditionFlags::ZERO,
    );
    assert_eq!(taken.program_counter, CODE + 16);
    // cbz w1 tests only the low word.
    let cbz = run_documented(
        Instruction::Cbz32,
        &[(label, 4), (RT, 1)],
        &[(1, 1 << 32)],
        ConditionFlags::empty(),
    );
    assert_eq!(cbz.program_counter, CODE + 16);
    // tbnz x1, #63 has b5 one and b40 all ones.
    let tbnz = run_documented(
        Instruction::Tbnz,
        &[
            (Field::new(5, 14), 4),
            (Field::new(19, 5), 31),
            (Field::new(31, 1), 1),
            (RT, 1),
        ],
        &[(1, SIGN_BIT)],
        ConditionFlags::empty(),
    );
    assert_eq!(tbnz.program_counter, CODE + 16);
}

#[test]
fn address_forms_add_their_label_to_the_instruction_or_its_page() {
    // adr x0, . + 5 has immlo 1 and immhi 1.
    let adr = run_documented(
        Instruction::Adr64,
        &[(RD, 0), (Field::new(29, 2), 1), (Field::new(5, 19), 1)],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(adr.general_purpose_registers[0], CODE + 5);
    // adrp x0, . - 4096 has every label bit set.
    let adrp = run_documented(
        Instruction::Adrp,
        &[
            (RD, 0),
            (Field::new(29, 2), 3),
            (Field::new(5, 19), 0x7_FFFF),
        ],
        &[],
        ConditionFlags::empty(),
    );
    assert_eq!(adrp.general_purpose_registers[0], CODE - PAGE);
}

#[test]
fn loads_extend_index_and_write_back() {
    let run_load = |instruction, fields: &[(Field, u32)], registers: &[(usize, u64)]| {
        let precise = Arm64::<User>::list_known_behaviors(instruction)
            .next()
            .expect("a load has a behavior");
        let mut before = to_state(encode(instruction, fields), READ_ONLY, READ_ONLY);
        for &(number, value) in registers {
            before.processor.general_purpose_registers[number] = value;
        }
        run(precise, before).0
    };
    // ldrsb w0, [x1, #1]! loads the byte at DATA + 1, sign-extends it to 32 bits and writes DATA + 1 back.
    let pre = run_load(
        Instruction::LdrsbImmediatePreIndex32,
        &[(RT, 0), (RN, 1), (IMM9, 1)],
        &[(1, DATA + 0x7F)],
    );
    assert_eq!(pre.general_purpose_registers[0], 0xFFFF_FF80);
    assert_eq!(pre.general_purpose_registers[1], DATA + 0x80);
    // ldrh w0, [x1, w2, sxtw #1] with w2 = -1 loads the halfword at DATA + 0x10 - 2.
    let register = run_load(
        Instruction::LdrhRegister32,
        &[
            (RT, 0),
            (RN, 1),
            (RM, 2),
            (Field::new(15, 1), 1),
            (Field::new(12, 1), 1),
        ],
        &[(1, DATA + 0x10), (2, 0xFFFF_FFFF)],
    );
    assert_eq!(register.general_purpose_registers[0], 0x0F0E);
    // ldpsw x0, x2, [x1], #8 loads two sign-extended words at the base, then adds 8 to it.
    let pair = run_load(
        Instruction::LdpswPostIndex64,
        &[(RT, 0), (RT2, 2), (RN, 1), (IMM7, 2)],
        &[(1, DATA + 0xFC)],
    );
    assert_eq!(pair.general_purpose_registers[0], 0xFFFF_FFFF_FFFE_FDFC);
    assert_eq!(pair.general_purpose_registers[2], 0x0302_0100);
    assert_eq!(pair.general_purpose_registers[1], DATA + 0xFC + 8);
}

#[test]
fn flag_conversions_follow_the_manual() {
    let flags = |instruction, before| run_documented(instruction, &[], &[], before).condition_flags;
    assert_eq!(
        flags(Instruction::Cfinv, ConditionFlags::CARRY),
        ConditionFlags::empty()
    );
    // AXFLAG of unordered (C and V set) gives Z set.
    assert_eq!(
        flags(
            Instruction::Axflag,
            ConditionFlags::CARRY | ConditionFlags::OVERFLOW
        ),
        ConditionFlags::ZERO
    );
    // XAFLAG of external less than (all clear) gives N set.
    assert_eq!(
        flags(Instruction::Xaflag, ConditionFlags::empty()),
        ConditionFlags::NEGATIVE
    );
    let udf = run_documented(Instruction::Udf, &[], &[], ConditionFlags::empty());
    assert_eq!(udf.exception, Some(Exception::UndefinedInstruction));
}

#[test]
fn an_exclusive_load_marks_its_granule_and_a_store_exclusive_there_stores() {
    let load = encode(Instruction::Ldxr64, &[(RT, 0), (RN, 1)]);
    let mut before = to_state(load, READ_WRITE, READ_WRITE);
    before.processor.general_purpose_registers[1] = DATA + 8;
    let (loaded, _) = run(to_load_store(Instruction::Ldxr64, NO_CHOICES), before);
    assert_eq!(
        loaded.exclusive_monitor,
        ExclusiveMonitor::Exclusive { address: DATA + 8 }
    );
    let choices = LoadStoreChoices {
        failed_store_exclusive: Some(FailedStoreExclusive::ChecksAccess),
        store_exclusive_overlap: Some(StoreExclusiveOverlap::Runs),
        ..NO_CHOICES
    };
    let store = encode(Instruction::Stxr64, &[(RM, 2), (RT, 3), (RN, 1)]);
    let at = |address: u64, monitor: ExclusiveMonitor| {
        let mut before = to_state(store, READ_WRITE, READ_WRITE);
        before.processor.general_purpose_registers[1] = address;
        before.processor.general_purpose_registers[3] = 0x1122_3344_5566_7788;
        before.processor.exclusive_monitor = monitor;
        run(to_load_store(Instruction::Stxr64, choices), before)
    };
    // In the marked 64 bytes the store happens and gives status 0; outside them, or with the monitor open, it fails.
    let (stored, made) = at(DATA + 0x38, loaded.exclusive_monitor);
    assert_eq!(stored.general_purpose_registers[2], 0);
    assert_eq!(stored.exclusive_monitor, ExclusiveMonitor::Open);
    assert!(
        made == [MemoryDelta::ValueAssignment {
            start: DATA + 0x38,
            bytes: Cow::Owned(0x1122_3344_5566_7788_u64.to_le_bytes().to_vec()),
        }]
    );
    for (address, monitor) in [
        (DATA + 0x40, loaded.exclusive_monitor),
        (DATA + 8, ExclusiveMonitor::Open),
    ] {
        let (failed, made) = at(address, monitor);
        assert_eq!(failed.general_purpose_registers[2], 1);
        assert_eq!(failed.exclusive_monitor, ExclusiveMonitor::Open);
        assert!(made.is_empty());
    }
    let mut before = to_state(encode(Instruction::Clrex, &[]), READ_WRITE, READ_WRITE);
    before.processor.exclusive_monitor = loaded.exclusive_monitor;
    let (cleared, _) = run(to_documented(Instruction::Clrex), before);
    assert_eq!(cleared.exclusive_monitor, ExclusiveMonitor::Open);
}

#[test]
fn a_random_number_read_leaves_its_register_and_nzcv_undecided() {
    let rndr = system_register::SystemRegisterEncoding::new(3, 3, 2, 4, 0).to_field();
    let word = encode(
        Instruction::Mrs,
        &[(RT, 5), (system_register::SYSTEM_REGISTER, rndr)],
    );
    for (choices, undecided) in [
        (system_register::NEOVERSE_V2_SYSTEM_REGISTERS, true),
        (system_register::OTHER_SYSTEM_REGISTERS, false),
    ] {
        let read = PreciseInstruction::<Infallible>::SystemRegisterMove {
            direction: AccessDirection::Read,
            choices,
        };
        let Ok(after) = read.simulate_state(to_state(word, READ_WRITE, READ_WRITE));
        let expected = &after.processor;
        assert_eq!(
            expected.general_purpose_registers[5] == RegisterValueExpectation::NonDeterministic,
            undecided
        );
        assert_eq!(
            expected.condition_flags == RegisterValueExpectation::NonDeterministic,
            undecided
        );
        assert_eq!(
            expected.processor.exception,
            (!undecided).then_some(Exception::UndefinedInstruction)
        );
        let mut observed = expected.processor.clone();
        observed.general_purpose_registers[5] = 0x1234;
        observed.condition_flags = ConditionFlags::ZERO;
        assert_eq!(
            Arm64::<User>::meets_expectations(&observed, expected),
            undecided
        );
    }
}

#[test]
fn an_optional_instruction_has_the_behavior_of_a_chip_without_its_feature() {
    let behaviors = |instruction| -> Vec<PreciseInstruction<Infallible>> {
        Arm64::<User>::list_known_behaviors(instruction).collect()
    };
    let unimplemented = |instruction| PreciseInstruction::Unimplemented(instruction);
    // Mandatory and modeled, optional and modeled, optional and unmodeled, the LS64 family, and a mandatory one no
    // behavior models yet.
    assert!(
        !behaviors(Instruction::AddShiftedRegister64)
            .contains(&unimplemented(Instruction::AddShiftedRegister64))
    );
    assert!(behaviors(Instruction::Ldadd64).contains(&unimplemented(Instruction::Ldadd64)));
    assert_eq!(
        behaviors(Instruction::Pacm),
        [unimplemented(Instruction::Pacm)]
    );
    assert_eq!(
        behaviors(Instruction::St64bv),
        [unimplemented(Instruction::St64bv)]
    );
    // A modeled hint that changes nothing already is what a chip without it does.
    assert!(!behaviors(Instruction::Bti).contains(&unimplemented(Instruction::Bti)));
    let hint = to_state(encode(Instruction::Pacm, &[]), READ_WRITE, READ_WRITE);
    let (after, _) = run(unimplemented(Instruction::Pacm), hint);
    assert_eq!(after.exception, None);
    let other = to_state(encode(Instruction::St64bv, &[]), READ_WRITE, READ_WRITE);
    let (after, _) = run(unimplemented(Instruction::St64bv), other);
    assert_eq!(after.exception, Some(Exception::UndefinedInstruction));
}
