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
    misaligned_ordered_or_atomic: None,
    fault_address_tag: None,
    reservation_granule_bytes: None,
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
    before.processor.condition_flags = Nzcv::from(ConditionFlags::CARRY);
    let (after, _) = run(to_documented(Instruction::AddShiftedRegister64), before);
    assert_eq!(after.general_purpose_registers[0], 0);
    assert_eq!(
        ConditionFlags::from(after.condition_flags),
        ConditionFlags::CARRY
    );
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
        ConditionFlags::from(after.condition_flags),
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
    before.processor.condition_flags = Nzcv::from(ConditionFlags::NEGATIVE);
    let (after, _) = run(to_documented(Instruction::AddsShiftedRegister64), before);
    assert_eq!(after.general_purpose_registers[0], 0);
    assert_eq!(
        ConditionFlags::from(after.condition_flags),
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
        before.processor.condition_flags = Nzcv::from(flags);
        let (after, _) = run(to_documented(Instruction::Csel64), before);
        assert_eq!(
            after.general_purpose_registers[0], selected,
            "condition {condition}"
        );
        assert_eq!(ConditionFlags::from(after.condition_flags), flags);
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
fn a_load_acquire_inside_16_bytes_faults_as_its_behavior_says() {
    let word = encode(Instruction::Ldar64, &[(RT, 0), (RN, 1)]);
    for (choice, faults) in [
        (MisalignedOrderedOrAtomic::AlignmentFault, true),
        (MisalignedOrderedOrAtomic::WithinSixteenBytes, false),
    ] {
        let choices = LoadStoreChoices {
            misaligned_ordered_or_atomic: Some(choice),
            ..NO_CHOICES
        };
        let mut before = to_state(word, READ_ONLY, READ_ONLY);
        before.processor.general_purpose_registers[1] = DATA + 4;
        let (after, _) = run(to_load_store(Instruction::Ldar64, choices), before);
        let fault = to_data_abort(FaultStatus::AlignmentFault, AccessDirection::Read, DATA + 4);
        assert_eq!(after.exception, faults.then_some(fault));
    }
    // A byte and a pair of doublewords leave nothing to choose.
    let open = |instruction| {
        LoadStoreInstruction::new(instruction)
            .is_some_and(|access| access.to_open_cases().misaligned_ordered_or_atomic)
    };
    assert!(open(Instruction::Ldar64) && open(Instruction::Casp32) && open(Instruction::Ldaddh));
    assert!(!open(Instruction::Ldarb) && !open(Instruction::Casp64) && !open(Instruction::Ldxr64));
}

#[test]
fn an_optional_encoding_is_undefined_without_its_feature() {
    const SIZE: Field = Field::new(22, 2);
    const IMMH: Field = Field::new(19, 4);
    let without = |instruction| {
        PreciseInstruction::WithoutOptionalEncodings(
            DocumentedInstruction::new(instruction).expect("a documented instruction"),
        )
    };
    let behaviors = |instruction| -> Vec<PreciseInstruction<Infallible>> {
        Arm64::<User>::list_known_behaviors(instruction).collect()
    };
    let pmull = behaviors(Instruction::PmullAsimddiffL);
    assert!(pmull.contains(&to_documented(Instruction::PmullAsimddiffL)));
    assert!(pmull.contains(&without(Instruction::PmullAsimddiffL)));
    assert!(
        !behaviors(Instruction::AddShiftedRegister64)
            .contains(&without(Instruction::AddShiftedRegister64))
    );
    for (instruction, optional, other) in [
        (Instruction::PmullAsimddiffL, (SIZE, 0b11), (SIZE, 0b00)),
        (
            Instruction::ScvtfVectorImmediateImmediate,
            (IMMH, 0b0011),
            (IMMH, 0b0100),
        ),
        (
            Instruction::FcvtzuScalarImmediate,
            (IMMH, 0b0010),
            (IMMH, 0b1000),
        ),
    ] {
        for (field, undefined) in [(optional, true), (other, false)] {
            let word = encode(instruction, &[field]);
            let state = || to_state(word, READ_WRITE, READ_WRITE);
            let (after, _) = run(without(instruction), state());
            assert_eq!(
                after.exception == Some(Exception::UndefinedInstruction),
                undefined,
                "{instruction:?} {word:#010x}"
            );
            let (documented, _) = run(to_documented(instruction), state());
            assert_eq!(documented.exception, None, "{instruction:?} {word:#010x}");
            if !undefined {
                assert_eq!(after, documented);
            }
        }
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
    before.processor.condition_flags = Nzcv::from(flags);
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
        // An Advanced SIMD instruction of `advanced_simd` reads its fields from the word by its encoding class.
        if (DocumentedInstruction::new(instruction).is_none()
            && LoadStoreInstruction::new(instruction).is_none())
            || crate::advanced_simd::to_advanced_simd_operation(instruction).is_some()
        {
            continue;
        }
        // An SVE instruction's operands are named by `scalable`, since the list leaves them unmodeled.
        if crate::scalable::list_scalable_operands(instruction).is_some() {
            continue;
        }
        // So are those of `sve_remaining`, by the fields its domain lists.
        if crate::sve_remaining::to_remaining_operation(instruction).is_some() {
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
                operand::Operand::SplitAddressOffset { fields, .. } => fields.to_vec(),
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
    assert_eq!(
        ConditionFlags::from(unequal.condition_flags),
        ConditionFlags::CARRY
    );
    let equal = run_documented(
        Instruction::Ccmp64,
        &[(RN, 1), (RM, 2), (NZCV, 0b0010), (COND, 0)],
        &[(1, 5), (2, 5)],
        ConditionFlags::ZERO,
    );
    assert_eq!(
        ConditionFlags::from(equal.condition_flags),
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
        ConditionFlags::from(rmif.condition_flags),
        ConditionFlags::NEGATIVE | ConditionFlags::CARRY
    );
    let setf8 = run_documented(
        Instruction::Setf8,
        &[(RN, 1)],
        &[(1, 0x80)],
        ConditionFlags::CARRY,
    );
    assert_eq!(
        ConditionFlags::from(setf8.condition_flags),
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
    let flags = |instruction, before| {
        ConditionFlags::from(run_documented(instruction, &[], &[], before).condition_flags)
    };
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
        reservation_granule_bytes: Some(64),
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
            expected.uncompared.general_purpose_registers[5] == u64::MAX,
            undecided
        );
        assert_eq!(
            expected.uncompared.condition_flags == ConditionFlags::all(),
            undecided
        );
        assert_eq!(
            expected.processor.exception,
            (!undecided).then_some(Exception::UndefinedInstruction)
        );
        let mut observed = expected.processor.clone();
        observed.general_purpose_registers[5] = 0x1234;
        observed.condition_flags = Nzcv::from(ConditionFlags::ZERO);
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
        behaviors(Instruction::Ld1b2Z),
        [unimplemented(Instruction::Ld1b2Z)]
    );
    assert_eq!(
        behaviors(Instruction::St64bv),
        [unimplemented(Instruction::St64bv)]
    );
    // A modeled hint that changes nothing already is what a chip without it does.
    assert!(!behaviors(Instruction::Bti).contains(&unimplemented(Instruction::Bti)));
    assert!(!behaviors(Instruction::Pacm).contains(&unimplemented(Instruction::Pacm)));
    let hint = to_state(encode(Instruction::Pacm, &[]), READ_WRITE, READ_WRITE);
    let (after, _) = run(unimplemented(Instruction::Pacm), hint);
    assert_eq!(after.exception, None);
    let other = to_state(encode(Instruction::St64bv, &[]), READ_WRITE, READ_WRITE);
    let (after, _) = run(unimplemented(Instruction::St64bv), other);
    assert_eq!(after.exception, Some(Exception::UndefinedInstruction));
}

/// The two laws a proof takes on trust for this architecture, `equality_is_identity` and `clone_is_identity`, on
/// processors that differ from the zero processor in one field each, on every permission and on every instruction:
/// `==` holds exactly when every field is the same, and a clone is equal to what it was cloned from.
#[test]
fn equality_and_clone_of_processors_permissions_and_instructions_are_identity() {
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
        |processor| processor.condition_flags = Nzcv::from(ConditionFlags::all()),
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
    let instructions: Vec<_> = Arm64::<User>::list_instructions().collect();
    for first in &instructions {
        for second in &instructions {
            assert_eq!(
                *first == *second,
                alloc::format!("{first:?}") == alloc::format!("{second:?}")
            );
        }
    }
}

/// The conversion between the verified code's flags and PSTATE's, which Verus cannot read, on all sixteen values, and
/// the immediate's bits in PSTATE's order.
#[test]
fn every_flag_value_converts_to_its_bit_and_back() {
    use crate::condition_flags::{
        IMMEDIATE_CARRY, IMMEDIATE_NEGATIVE, IMMEDIATE_OVERFLOW, IMMEDIATE_ZERO, Nzcv,
    };
    let position = ConditionFlags::OVERFLOW.bits().trailing_zeros();
    for (immediate, flag) in [
        (IMMEDIATE_NEGATIVE, ConditionFlags::NEGATIVE),
        (IMMEDIATE_ZERO, ConditionFlags::ZERO),
        (IMMEDIATE_CARRY, ConditionFlags::CARRY),
        (IMMEDIATE_OVERFLOW, ConditionFlags::OVERFLOW),
    ] {
        assert_eq!(u64::from(flag.bits() >> position), immediate);
    }
    for bits in 0..=ConditionFlags::all().bits() >> position {
        let immediate = u64::from(bits);
        let nzcv = Nzcv {
            negative: immediate & IMMEDIATE_NEGATIVE != 0,
            zero: immediate & IMMEDIATE_ZERO != 0,
            carry: immediate & IMMEDIATE_CARRY != 0,
            overflow: immediate & IMMEDIATE_OVERFLOW != 0,
        };
        let flags = ConditionFlags::from(nzcv);
        assert_eq!(flags.bits(), bits << position);
        assert_eq!(Nzcv::from(flags), nzcv);
        assert_eq!(ConditionFlags::from(Nzcv::from(flags)), flags);
    }
}

/// The proven integer instructions refuse, as UNDEFINED, an operand list that gives other values than their operation
/// reads, which Verus cannot rule out from tables it does not read: every listed integer instruction's list gives the
/// values its operation reads, each in the range the operation takes, so that no listed word meets that refusal.
#[test]
fn every_integer_operand_list_gives_the_values_its_operation_reads() {
    use crate::integer::{IntegerOperation as O, MAXIMUM_SOURCES, to_integer_operation};
    use crate::operand::{Operand, OperandAccess, RegisterWidth};
    let mut checked = 0;
    for instruction in LISTED_INSTRUCTIONS {
        let Some(operation) = to_integer_operation(instruction) else {
            continue;
        };
        let operands = instruction
            .to_record()
            .expect("a listed instruction")
            .operands;
        assert!(
            operands.iter().all(Operand::is_well_formed),
            "{instruction:?}"
        );
        let values: Vec<Operand> = operands
            .iter()
            .copied()
            .filter(|operand| {
                !matches!(
                    operand,
                    Operand::GeneralPurposeRegister {
                        access: OperandAccess::Written,
                        ..
                    }
                )
            })
            .collect();
        let width = operands.iter().find_map(|operand| match operand {
            Operand::GeneralPurposeRegister { width, .. } => Some(*width),
            _ => None,
        });
        let bits = width.map_or(u64::BITS, RegisterWidth::to_bits);
        // A register read at the instruction's width gives a value below 2^bits.
        let register = |operand: &Operand| matches!(operand, Operand::GeneralPurposeRegister { width: read, .. } if Some(*read) == width);
        let immediate = |operand: &Operand, most: u32| matches!(operand, Operand::UnsignedImmediate(field) | Operand::Condition(field) if field.width <= most);
        let arity = match operation {
            O::InvertCarry
            | O::ConvertFlagsToExternalFormat
            | O::ConvertFlagsFromExternalFormat => 0,
            O::ReverseBits
            | O::ReverseBytes { .. }
            | O::CountLeadingZeros
            | O::CountLeadingSignBits
            | O::SetFlags { .. } => 1,
            O::Bitfield(_)
            | O::Extract
            | O::ConditionalSelect(_)
            | O::MultiplyAdd
            | O::MultiplySubtract
            | O::MultiplyAddLong(_)
            | O::MultiplySubtractLong(_)
            | O::RotateMaskInsertFlags => 3,
            O::ConditionalCompare(_) => 4,
            _ => 2,
        };
        assert_eq!(values.len(), arity, "{instruction:?}");
        assert!(arity <= MAXIMUM_SOURCES);
        let fits = match operation {
            O::MoveWideNot | O::MoveWideZero | O::MoveWideKeep => {
                immediate(&values[0], 16) && matches!(values[1], Operand::WideImmediateShift { .. })
            }
            O::Bitfield(_) => immediate(&values[1], 6) && immediate(&values[2], 6),
            O::Extract => register(&values[0]) && register(&values[1]) && immediate(&values[2], 6),
            O::ConditionalSelect(_) => {
                matches!(values[2], Operand::Condition(field) if field.width <= 4)
            }
            O::ConditionalCompare(_) => {
                matches!(values[3], Operand::Condition(field) if field.width <= 4)
            }
            O::ReverseBytes { container_bits } => {
                [16, 32, 64].contains(&container_bits) && container_bits <= bits
            }
            O::CountLeadingZeros
            | O::CountLeadingSignBits
            | O::ShiftLeftVariable
            | O::ShiftRightLogicalVariable
            | O::ShiftRightArithmeticVariable
            | O::RotateRightVariable => register(&values[0]),
            O::Divide(_) => register(&values[0]) && register(&values[1]),
            O::Crc32 { bytes, .. } => [1, 2, 4, 8].contains(&bytes),
            O::RotateMaskInsertFlags => {
                bits == u64::BITS && immediate(&values[1], 6) && immediate(&values[2], 4)
            }
            O::SetFlags { bits } => [8, 16].contains(&bits),
            _ => true,
        };
        assert!(fits, "{instruction:?}: {values:?}");
        checked += 1;
    }
    assert_eq!(checked, 133, "integer instructions");
}

/// The proven read of a load or store's operands, and the proven load or store, refuse, as UNDEFINED, an operand list
/// they cannot take, which Verus cannot rule out from tables it does not read: every listed load or store names only
/// registers of five bits, a base and offsets, each fitting its encoding, and a register offset only with a size it
/// can scale by; a load or store one or two registers of 1 to 8 bytes, or 16 for SIMD and floating-point registers,
/// sign-extended from at most 8; a prefetch none; a store exclusive a status register and one or two general-purpose
/// registers of up to 8 bytes; an atomic two; and a compare and swap pair four of 4 or 8 bytes.
#[test]
fn every_load_store_operand_list_is_one_the_proven_read_takes() {
    use crate::load_store::{
        LoadStoreInstruction, MAXIMUM_REGISTERS, Transfer, to_load_store_operation,
    };
    use crate::operand::{Operand, Signedness};
    let mut checked = 0;
    for instruction in LISTED_INSTRUCTIONS {
        let (Some(_), Some(operation)) = (
            LoadStoreInstruction::new(instruction),
            to_load_store_operation(instruction),
        ) else {
            continue;
        };
        let operands = instruction
            .to_record()
            .expect("a listed instruction")
            .operands;
        let mut registers = 0;
        let mut vector = false;
        for operand in operands {
            assert!(operand.is_well_formed(), "{instruction:?}: {operand:?}");
            match operand {
                Operand::GeneralPurposeRegister { number, .. }
                | Operand::VectorRegister { number, .. }
                | Operand::GeneralPurposeRegisterPair { number, .. } => {
                    assert!(number.width <= 5, "{instruction:?}");
                }
                _ => {}
            }
            vector |= matches!(operand, Operand::VectorRegister { .. });
            registers += match operand {
                Operand::GeneralPurposeRegister { .. } | Operand::VectorRegister { .. } => 1,
                Operand::GeneralPurposeRegisterPair { .. } => 2,
                Operand::RegisterOffset { .. } => {
                    assert!(
                        [1, 2, 4, 8, 16].contains(&operation.bytes),
                        "{instruction:?}"
                    );
                    0
                }
                Operand::AddressBase { .. }
                | Operand::AddressOffset { .. }
                | Operand::SplitAddressOffset { .. }
                | Operand::ProgramCounterOffset { .. }
                | Operand::UnsignedImmediate(_) => 0,
                _ => panic!("{instruction:?} names {operand:?}"),
            };
        }
        assert!(registers <= MAXIMUM_REGISTERS, "{instruction:?}");
        let bytes = operation.bytes;
        let size = [1, 2, 4, 8].contains(&bytes);
        let shaped = match operation.transfer {
            Transfer::Load(signedness) => {
                registers <= 2
                    && (size || (vector && bytes == 16))
                    && (signedness == Signedness::Unsigned || bytes <= 8)
            }
            Transfer::Store => registers <= 2 && (size || (vector && bytes == 16)),
            Transfer::Prefetch => registers == 0,
            Transfer::StoreExclusive => (2..=3).contains(&registers) && !vector && size,
            Transfer::Atomic(_) => registers == 2 && !vector && size,
            Transfer::CompareAndSwapPair => registers == 4 && !vector && [4, 8].contains(&bytes),
        };
        assert!(
            shaped,
            "{instruction:?}: {registers} registers of {bytes} bytes"
        );
        checked += 1;
    }
    assert!(checked > 100, "{checked} loads and stores");
}

/// The branch family Arm's decode gives an encoding with these fixed bits, from the
/// [A64 encoding index](https://developer.arm.com/documentation/ddi0602/latest/Index-by-Encoding)'s fields; none for an
/// encoding of no family the branch table models. It shares no code with `to_branch_operation`.
fn to_decoded_branch(fixed: encoding::FixedBits) -> Option<branch::BranchOperation> {
    use branch::BranchOperation as O;
    use branch::BranchWhen::{NonZero, Zero};
    let is = |mask: u32, value: u32| fixed.mask & mask == mask && fixed.value & mask == value;
    // PC-rel. addressing: op<31>, 10000<28:24>.
    let address = |op: u32| is(1 << 31 | 0b11111 << 24, op << 31 | 0b10000 << 24);
    // Unconditional branch (immediate): op<31>, 00101<30:26>.
    let immediate = |op: u32| is(1 << 31 | 0b11111 << 26, op << 31 | 0b00101 << 26);
    // Compare and branch (immediate): 011010<30:25>, op<24>; test and branch (immediate): 011011<30:25>, op<24>.
    let compare = |op: u32| is(0b111_1111 << 24, (0b011010 << 1 | op) << 24);
    let test = |op: u32| is(0b111_1111 << 24, (0b011011 << 1 | op) << 24);
    // Unconditional branch (register): 1101011<31:25>, opc<24:21>, op2<20:16> = 11111, op3<15:10>, Rn<9:5>, op4<4:0>.
    let register = |opc: u32, op3: u32, rn: Option<u32>, op4: Option<u32>| {
        let rn_mask = if rn.is_some() { 0b11111 << 5 } else { 0 };
        let op4_mask = if op4.is_some() { 0b11111 } else { 0 };
        is(
            0b111_1111 << 25 | 0b1111 << 21 | 0b11111 << 16 | 0b11_1111 << 10 | rn_mask | op4_mask,
            0b110_1011 << 25
                | opc << 21
                | 0b11111 << 16
                | op3 << 10
                | rn.unwrap_or(0) << 5
                | op4.unwrap_or(0),
        )
    };
    // op3 000010 is the A key, 000011 the B key; op4 11111 names no modifier.
    let (a, b, none) = (0b00_0010, 0b00_0011, Some(0b11111));
    Some(if address(0) {
        O::FormAddress
    } else if address(1) {
        O::FormPageAddress
    } else if immediate(0) {
        O::Branch
    } else if immediate(1) {
        O::BranchWithLink
    // Conditional branch (immediate): 0101010<31:25>, o1<24> = 0, o0<4> = 0 (o0 = 1 is BC.cond).
    } else if is(0xFF00_0010, 0b0101_0100 << 24) {
        O::BranchConditionally
    } else if compare(0) {
        O::CompareAndBranch(Zero)
    } else if compare(1) {
        O::CompareAndBranch(NonZero)
    } else if test(0) {
        O::TestBitAndBranch(Zero)
    } else if test(1) {
        O::TestBitAndBranch(NonZero)
    } else if register(0b0000, 0, None, Some(0))
        || register(0b0000, a, None, none)
        || register(0b0000, b, None, none)
        || register(0b1000, a, None, None)
        || register(0b1000, b, None, None)
    {
        O::BranchToRegister
    } else if register(0b0001, 0, None, Some(0))
        || register(0b0001, a, None, none)
        || register(0b0001, b, None, none)
        || register(0b1001, a, None, None)
        || register(0b1001, b, None, None)
    {
        O::BranchWithLinkToRegister
    } else if register(0b0010, 0, None, Some(0))
        || register(0b0010, a, Some(0b11111), none)
        || register(0b0010, b, Some(0b11111), none)
    {
        O::Return
    } else {
        return None;
    })
}

#[test]
fn the_branch_table_gives_each_listed_instruction_the_family_arm_decodes() {
    let mut families = 0;
    for instruction in LISTED_INSTRUCTIONS {
        let record = instruction
            .to_record()
            .expect("every listed instruction has a record");
        let decoded = to_decoded_branch(record.fixed_bits);
        assert_eq!(
            branch::to_branch_operation(instruction),
            decoded,
            "{instruction:?}"
        );
        families += usize::from(decoded.is_some());
    }
    // ADR, ADRP, B, BL, B.cond, two CBZ, two CBNZ, TBZ, TBNZ, five BR, five BLR and three RET.
    assert_eq!(families, 24);
}

#[test]
fn only_b_cond_reads_the_flags_and_only_bl_and_blr_write_x30() {
    use Instruction as I;
    for instruction in LISTED_INSTRUCTIONS {
        let Some(operation) = branch::to_branch_operation(instruction) else {
            continue;
        };
        // ConditionHolds reads PSTATE.NZCV only in B.cond; BranchType_DIRCALL and BranchType_INDCALL write X30.
        assert_eq!(
            operation.reads_condition_flags(),
            instruction == I::BCond,
            "{instruction:?}"
        );
        assert_eq!(
            operation.links(),
            matches!(
                instruction,
                I::Bl | I::Blr | I::Blraaz | I::Blrabz | I::Blraa | I::Blrab
            ),
            "{instruction:?}"
        );
    }
}

/// The integer operation Arm's decode gives an encoding with these fixed bits, from the
/// [A64 encoding index](https://developer.arm.com/documentation/ddi0602/latest/Index-by-Encoding)'s fields of Data
/// Processing -- Immediate, Data Processing -- Register and the PSTATE writes; none for an encoding of no operation the
/// integer table models. It shares no code with `to_integer_operation`.
fn to_decoded_integer(fixed: encoding::FixedBits) -> Option<integer::IntegerOperation> {
    use integer::FlagSetting::{Preserves, Sets};
    use integer::IntegerOperation as O;
    use integer::{BitfieldFill, Comparison, CrcPolynomial, SelectAlternative};
    use operand::Signedness::{Signed, Unsigned};
    // The bits <high:low>, where every one of them is fixed.
    let bits = |high: u32, low: u32| {
        let mask = (u32::MAX >> (31 - high + low)) << low;
        (fixed.mask & mask == mask).then_some((fixed.value & mask) >> low)
    };
    let is = |high: u32, low: u32, value: u32| bits(high, low) == Some(value);
    let setting = |s: u32| if s == 1 { Sets } else { Preserves };
    let (sf, op, s) = (bits(31, 31), bits(30, 30), bits(29, 29));
    // Add/subtract (immediate): op<30>, S<29>, 100010<28:23>.
    if is(28, 23, 0b10_0010) {
        return Some(if op? == 0 {
            O::Add(setting(s?))
        } else {
            O::Subtract(setting(s?))
        });
    }
    // Logical (immediate): opc<30:29>, 100100<28:23>.
    if is(28, 23, 0b10_0100) {
        return Some(
            [O::And(Preserves), O::Or, O::ExclusiveOr, O::And(Sets)][bits(30, 29)? as usize],
        );
    }
    // Move wide (immediate): opc<30:29>, 100101<28:23>; opc 01 is unallocated.
    if is(28, 23, 0b10_0101) {
        return [
            Some(O::MoveWideNot),
            None,
            Some(O::MoveWideZero),
            Some(O::MoveWideKeep),
        ][bits(30, 29)? as usize];
    }
    // Bitfield: opc<30:29>, 100110<28:23>; opc 11 is unallocated.
    if is(28, 23, 0b10_0110) {
        use BitfieldFill::{DestinationBits, SignBits, Zeros};
        return [SignBits, DestinationBits, Zeros]
            .get(bits(30, 29)? as usize)
            .map(|&fill| O::Bitfield(fill));
    }
    // Extract: op21<30:29> = 00, 100111<28:23>, o0<21> = 0.
    if is(28, 23, 0b10_0111) && is(30, 29, 0) && is(21, 21, 0) {
        return Some(O::Extract);
    }
    // Logical (shifted register): opc<30:29>, 01010<28:24>, N<21>.
    if is(28, 24, 0b01010) {
        let operations = [
            [O::And(Preserves), O::AndNot(Preserves)],
            [O::Or, O::OrNot],
            [O::ExclusiveOr, O::ExclusiveOrNot],
            [O::And(Sets), O::AndNot(Sets)],
        ];
        return Some(operations[bits(30, 29)? as usize][bits(21, 21)? as usize]);
    }
    // Add/subtract (shifted register): 01011<28:24>, 0<21>; (extended register): 01011<28:24>, opt<23:22> = 00, 1<21>.
    if is(28, 24, 0b01011) && (is(21, 21, 0) || (is(21, 21, 1) && is(23, 22, 0))) {
        return Some(if op? == 0 {
            O::Add(setting(s?))
        } else {
            O::Subtract(setting(s?))
        });
    }
    // Data-processing (2 source): op0<30> = 0, S<29> = 0, 11010110<28:21>, opcode<15:10>.
    if is(28, 21, 0b1101_0110) && op == Some(0) && s == Some(0) {
        let opcode = bits(15, 10)?;
        return match opcode {
            0b00_0010 => Some(O::Divide(Unsigned)),
            0b00_0011 => Some(O::Divide(Signed)),
            0b00_1000 => Some(O::ShiftLeftVariable),
            0b00_1001 => Some(O::ShiftRightLogicalVariable),
            0b00_1010 => Some(O::ShiftRightArithmeticVariable),
            0b00_1011 => Some(O::RotateRightVariable),
            // CRC32 and CRC32C: 010<15:13>, C<12>, sz<11:10>.
            0b01_0000..=0b01_0111 => Some(O::Crc32 {
                polynomial: if opcode & 0b100 == 0 {
                    CrcPolynomial::Crc32
                } else {
                    CrcPolynomial::Crc32C
                },
                bytes: 1 << (opcode & 0b11),
            }),
            _ => None,
        };
    }
    // Data-processing (1 source): op0<30> = 1, S<29> = 0, 11010110<28:21>, opcode2<20:16> = 00000, opcode<15:10>.
    if is(28, 21, 0b1101_0110) && op == Some(1) && s == Some(0) && is(20, 16, 0) {
        return match (bits(15, 10)?, sf?) {
            (0b00_0000, _) => Some(O::ReverseBits),
            (0b00_0001, _) => Some(O::ReverseBytes { container_bits: 16 }),
            // REV32 of an X register and REV of a W register.
            (0b00_0010, _) => Some(O::ReverseBytes { container_bits: 32 }),
            (0b00_0011, 1) => Some(O::ReverseBytes { container_bits: 64 }),
            (0b00_0100, _) => Some(O::CountLeadingZeros),
            (0b00_0101, _) => Some(O::CountLeadingSignBits),
            _ => None,
        };
    }
    if is(28, 21, 0b1101_0000) {
        // Rotate right into flags: sf op S = 1 0 1, imm6, 00001<14:10>, Rn, o2<4> = 0, mask.
        if sf == Some(1) && op == Some(0) && s == Some(1) && is(14, 10, 0b00001) && is(4, 4, 0) {
            return Some(O::RotateMaskInsertFlags);
        }
        // Evaluate into flags: sf op S = 0 0 1, opcode2<20:15> = 0, sz<14>, 0010<13:10>, Rn, o3<4> = 0, mask = 1101.
        if sf == Some(0)
            && op == Some(0)
            && s == Some(1)
            && is(20, 15, 0)
            && is(13, 10, 0b0010)
            && is(4, 0, 0b0_1101)
        {
            return Some(O::SetFlags {
                bits: if bits(14, 14)? == 0 { 8 } else { 16 },
            });
        }
        // Add/subtract (with carry): op3<15:10> = 000000.
        if is(15, 10, 0) {
            return Some(if op? == 0 {
                O::AddWithCarry(setting(s?))
            } else {
                O::SubtractWithCarry(setting(s?))
            });
        }
        return None;
    }
    // Conditional compare (register and immediate): op<30>, S<29> = 1, 11010010<28:21>, o2<10> = 0, o3<4> = 0.
    if is(28, 21, 0b1101_0010) && s == Some(1) && is(10, 10, 0) && is(4, 4, 0) {
        return Some(O::ConditionalCompare(if op? == 0 {
            Comparison::Sum
        } else {
            Comparison::Difference
        }));
    }
    // Conditional select: op<30>, S<29> = 0, 11010100<28:21>, op2<11:10>.
    if is(28, 21, 0b1101_0100) && s == Some(0) {
        use SelectAlternative::{Increment, Invert, Negate, Select};
        let alternatives = [
            [Some(Select), Some(Increment), None, None],
            [Some(Invert), Some(Negate), None, None],
        ];
        return alternatives[op? as usize][bits(11, 10)? as usize].map(O::ConditionalSelect);
    }
    // Data-processing (3 source): op54<30:29> = 00, 11011<28:24>, op31<23:21>, o0<15>.
    if is(28, 24, 0b11011) && is(30, 29, 0) {
        return match (bits(23, 21)?, bits(15, 15)?) {
            (0b000, 0) => Some(O::MultiplyAdd),
            (0b000, 1) => Some(O::MultiplySubtract),
            (0b001, 0) => Some(O::MultiplyAddLong(Signed)),
            (0b001, 1) => Some(O::MultiplySubtractLong(Signed)),
            (0b010, 0) => Some(O::MultiplyHigh(Signed)),
            (0b101, 0) => Some(O::MultiplyAddLong(Unsigned)),
            (0b101, 1) => Some(O::MultiplySubtractLong(Unsigned)),
            (0b110, 0) => Some(O::MultiplyHigh(Unsigned)),
            _ => None,
        };
    }
    // PSTATE: MSR (immediate) with op1 = 000, CRn = 0100, CRm<11:8> should be 0000, Rt = 11111; op2<7:5> 000 CFINV,
    // 001 XAFLAG, 010 AXFLAG.
    let pstate = |op2: u32| is(31, 12, 0b1101_0101_0000_0000_0100) && is(7, 0, op2 << 5 | 0b11111);
    if pstate(0b000) {
        return Some(O::InvertCarry);
    }
    if pstate(0b001) {
        return Some(O::ConvertFlagsFromExternalFormat);
    }
    if pstate(0b010) {
        return Some(O::ConvertFlagsToExternalFormat);
    }
    None
}

#[test]
fn the_integer_table_gives_each_listed_instruction_the_operation_arm_decodes() {
    let mut operations = 0;
    for instruction in LISTED_INSTRUCTIONS {
        let record = instruction
            .to_record()
            .expect("every listed instruction has a record");
        let decoded = to_decoded_integer(record.fixed_bits);
        assert_eq!(
            integer::to_integer_operation(instruction),
            decoded,
            "{instruction:?} ({})",
            record.mnemonic
        );
        operations += usize::from(decoded.is_some());
    }
    assert!(operations > 100, "{operations} integer instructions");
}

#[test]
fn integer_instructions_read_the_flags_and_their_register_as_the_manual_says() {
    for instruction in LISTED_INSTRUCTIONS {
        let Some(operation) = integer::to_integer_operation(instruction) else {
            continue;
        };
        let mnemonic = instruction.to_record().expect("listed").mnemonic;
        let inputs = operation.to_implicit_inputs();
        // PSTATE.NZCV is read by the forms with carry, the conditional selects and compares, and the instructions that
        // change only some flags or convert them; X[d] by MOVK and BFM, which keep part of it.
        let reads_flags = [
            "adc", "adcs", "sbc", "sbcs", "csel", "csinc", "csinv", "csneg", "ccmn", "ccmp",
            "rmif", "setf8", "setf16", "cfinv", "axflag", "xaflag",
        ]
        .contains(&mnemonic);
        assert_eq!(inputs.condition_flags, reads_flags, "{instruction:?}");
        assert_eq!(
            inputs.destination,
            ["movk", "bfm"].contains(&mnemonic),
            "{instruction:?}"
        );
        assert!(
            !inputs.program_counter && !inputs.floating_point_control,
            "{instruction:?}"
        );
    }
}

/// The load or store Arm's decode gives an encoding of general-purpose or SIMD and floating-point registers with these
/// fixed bits, from the [A64 encoding index](https://developer.arm.com/documentation/ddi0602/latest/Index-by-Encoding)'s
/// fields of Loads and Stores and its pointer authentication loads: what it transfers, the bytes of each register, and
/// where an ordered, atomic or exclusive access must lie; none for an encoding of no load or store the table models. It
/// shares no code with `to_load_store_operation`.
fn to_decoded_load_store(fixed: encoding::FixedBits) -> Option<load_store::LoadStoreOperation> {
    use load_store::AtomicOperation::{
        Add, Clear, ExclusiveOr, Set, SignedMaximum, SignedMinimum, Swap, UnsignedMaximum,
        UnsignedMinimum,
    };
    use load_store::{Alignment, AtomicOperation, LoadStoreOperation, Transfer};
    use operand::Signedness::{Signed, Unsigned};
    let bits = |high: u32, low: u32| {
        let mask = (u32::MAX >> (31 - high + low)) << low;
        (fixed.mask & mask == mask).then_some((fixed.value & mask) >> low)
    };
    let is = |high: u32, low: u32, value: u32| bits(high, low) == Some(value);
    // A field the manual writes as (1)(1)(1)(1)(1): ones where the list fixes it, and should-be ones where it does not.
    let ones = |high: u32, low: u32| {
        bits(high, low).is_none_or(|value| value == (1 << (high - low + 1)) - 1)
    };
    let operation = |transfer, bytes, alignment| {
        Some(LoadStoreOperation {
            transfer,
            bytes,
            alignment,
        })
    };
    // size<31:30> gives the bytes of a general-purpose access.
    let size_bytes = || bits(31, 30).map(|size| 1 << size);
    // A single register with size<31:30>, V<26> and opc<23:22> (LDR, STR, LDUR, LDTR and their sign-extending forms):
    // for general-purpose registers opc 00 stores, 01 loads, 1x loads signed (LDRSB, LDRSH, LDRSW; size 11 opc 10 is
    // PRFM, and the rest unallocated); for SIMD and floating-point registers size:opc<1> gives 1, 2, 4, 8 or 16 bytes and
    // opc<0> loads.
    let single = |prefetch: bool, unprivileged: bool| {
        let (size, vector, opc) = (bits(31, 30)?, bits(26, 26)?, bits(23, 22)?);
        if vector == 1 {
            if unprivileged {
                return None;
            }
            let bytes = match (size, opc >> 1) {
                (_, 0) => 1 << size,
                (0, 1) => 16,
                _ => return None,
            };
            let transfer = if opc & 1 == 1 {
                Transfer::Load(Unsigned)
            } else {
                Transfer::Store
            };
            return operation(transfer, bytes, Alignment::Unchecked);
        }
        let transfer = match (size, opc) {
            (_, 0) => Transfer::Store,
            (_, 1) => Transfer::Load(Unsigned),
            (0b11, 0b10) if prefetch => Transfer::Prefetch,
            (0b00 | 0b01, _) | (0b10, 0b10) => Transfer::Load(Signed),
            _ => return None,
        };
        operation(transfer, 1 << size, Alignment::Unchecked)
    };
    // Load/store register (unsigned immediate): size, 111<29:27>, V<26>, 01<25:24>, opc<23:22>.
    if is(29, 27, 0b111) && is(25, 24, 0b01) {
        return single(true, false);
    }
    // Load/store register (unscaled immediate, immediate post-indexed, unprivileged, immediate pre-indexed): 111<29:27>,
    // 00<25:24>, 0<21>, op2<11:10> 00, 01, 10, 11; only the unscaled form has PRFUM.
    if is(29, 27, 0b111) && is(25, 24, 0) && is(21, 21, 0) {
        let op2 = bits(11, 10)?;
        return single(op2 == 0, op2 == 0b10);
    }
    // Load/store register (register offset): 111<29:27>, 00<25:24>, 1<21>, 10<11:10>.
    if is(29, 27, 0b111) && is(25, 24, 0) && is(21, 21, 1) && is(11, 10, 0b10) {
        // [RPRFM](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RPRFM--Range-prefetch-memory-):
        // the PRFM of size 11, V 0, opc 10 with option<14> = 1 and Rt<4:3> = 11 is a range prefetch hint.
        if is(31, 30, 0b11) && is(26, 26, 0) && is(23, 22, 0b10) && is(14, 14, 1) && is(4, 3, 0b11)
        {
            return None;
        }
        return single(true, false);
    }
    // Load register (pac): size 11, 111<29:27>, V<26> = 0, 00<25:24>, M<23>, S<22>, 1<21>, W<11>, 1<10>; with the data
    // keys off it loads the doubleword at its base plus offset.
    if is(31, 26, 0b11_1110) && is(25, 24, 0) && is(21, 21, 1) && is(10, 10, 1) {
        return operation(Transfer::Load(Unsigned), 8, Alignment::Unchecked);
    }
    // Atomic memory operations: size, 111<29:27>, V<26> = 0, 00<25:24>, A<23>, R<22>, 1<21>, Rs, o3<15>, opc<14:12>,
    // 00<11:10>; o3 = 1 with opc 000 is SWP, and with opc 100, A = 1, R = 0 and Rs = 11111 is LDAPR.
    if is(29, 26, 0b1110) && is(25, 24, 0) && is(21, 21, 1) && is(11, 10, 0) {
        let atomic = |operation: AtomicOperation| Transfer::Atomic(operation);
        let transfer = match (bits(15, 15)?, bits(14, 12)?) {
            (0, opc) => atomic(
                [
                    Add,
                    Clear,
                    ExclusiveOr,
                    Set,
                    SignedMaximum,
                    SignedMinimum,
                    UnsignedMaximum,
                    UnsignedMinimum,
                ][opc as usize],
            ),
            (1, 0b000) => atomic(Swap),
            (1, 0b100) if is(23, 22, 0b10) && ones(20, 16) => Transfer::Load(Unsigned),
            _ => return None,
        };
        return operation(transfer, size_bytes()?, Alignment::WithinSixteenBytes);
    }
    // LDAPUR, STLUR and the sign-extending LDAPURS (RCpc unscaled immediate): size, 011001<29:24>, opc<23:22>, 0<21>,
    // 00<11:10>; opc 00 stores, 01 loads, 10 sign-extends to 64 bits (bytes, halfwords, words), 11 to 32 (bytes,
    // halfwords).
    if is(29, 24, 0b01_1001) && is(21, 21, 0) && is(11, 10, 0) {
        let size = bits(31, 30)?;
        let transfer = match (size, bits(23, 22)?) {
            (_, 0) => Transfer::Store,
            (_, 1) => Transfer::Load(Unsigned),
            (0b00..=0b10, 0b10) | (0b00 | 0b01, 0b11) => Transfer::Load(Signed),
            _ => return None,
        };
        return operation(transfer, 1 << size, Alignment::WithinSixteenBytes);
    }
    // Load/store exclusive and ordered, compare and swap: size<31:30>, 001000<29:24>, o2<23>, L<22>, o1<21>, Rs, o0<15>,
    // Rt2<14:10>.
    if is(29, 24, 0b00_1000) {
        let (o2, load, o1) = (bits(23, 23)?, bits(22, 22)? == 1, bits(21, 21)?);
        return match (o2, o1) {
            // Load/store exclusive register: LDXR, LDAXR, STXR, STLXR.
            (0, 0) => {
                let transfer = if load {
                    Transfer::Load(Unsigned)
                } else {
                    Transfer::StoreExclusive
                };
                operation(transfer, size_bytes()?, Alignment::Exclusive)
            }
            // Load/store exclusive pair (1<31>, sz<30>), or compare and swap pair (0<31>, sz<30>, Rt2 = 11111).
            (0, 1) => {
                let bytes = if bits(30, 30)? == 0 { 4 } else { 8 };
                match bits(31, 31)? {
                    1 => operation(
                        if load {
                            Transfer::Load(Unsigned)
                        } else {
                            Transfer::StoreExclusive
                        },
                        bytes,
                        Alignment::Exclusive,
                    ),
                    _ if ones(14, 10) => operation(
                        Transfer::CompareAndSwapPair,
                        bytes,
                        Alignment::WithinSixteenBytes,
                    ),
                    _ => None,
                }
            }
            // Load/store ordered: LDAR, LDLAR, STLR, STLLR.
            (1, 0) => operation(
                if load {
                    Transfer::Load(Unsigned)
                } else {
                    Transfer::Store
                },
                size_bytes()?,
                Alignment::WithinSixteenBytes,
            ),
            // Compare and swap (Rt2 = 11111).
            _ if ones(14, 10) => operation(
                Transfer::Atomic(AtomicOperation::CompareAndSwap),
                size_bytes()?,
                Alignment::WithinSixteenBytes,
            ),
            _ => None,
        };
    }
    // Load register (literal): opc<31:30>, 011<29:27>, V<26>, 00<25:24>: opc 00 a word, 01 a doubleword, 10 LDRSW, 11
    // PRFM; for SIMD and floating-point registers 4, 8 or 16 bytes.
    if is(29, 27, 0b011) && is(25, 24, 0) {
        let (opc, vector) = (bits(31, 30)?, bits(26, 26)?);
        let (transfer, bytes) = match (vector, opc) {
            (0, 0b00) => (Transfer::Load(Unsigned), 4),
            (0, 0b01) => (Transfer::Load(Unsigned), 8),
            (0, 0b10) => (Transfer::Load(Signed), 4),
            (0, _) => (Transfer::Prefetch, 8),
            (_, 0b11) => return None,
            (_, opc) => (Transfer::Load(Unsigned), 4 << opc),
        };
        return operation(transfer, bytes, Alignment::Unchecked);
    }
    // Load/store pair (no-allocate, post-indexed, offset, pre-indexed): opc<31:30>, 101<29:27>, V<26>, 0<25>, L<22>;
    // for general-purpose registers opc 00 words, 10 doublewords, 01 LDPSW (STGP and the no-allocate 01 are no plain
    // pair); for SIMD and floating-point registers opc 00, 01, 10 give 4, 8 or 16 bytes.
    if is(29, 27, 0b101) && is(25, 25, 0) {
        let (opc, vector, load) = (bits(31, 30)?, bits(26, 26)?, bits(22, 22)? == 1);
        let no_allocate = is(24, 23, 0);
        let plain = if load {
            Transfer::Load(Unsigned)
        } else {
            Transfer::Store
        };
        let (transfer, bytes) = match (vector, opc) {
            (0, 0b00) => (plain, 4),
            (0, 0b10) => (plain, 8),
            (0, 0b01) if load && !no_allocate => (Transfer::Load(Signed), 4),
            (1, 0b00..=0b10) => (plain, 4 << opc),
            _ => return None,
        };
        return operation(transfer, bytes, Alignment::Unchecked);
    }
    None
}

#[test]
fn the_load_store_table_gives_each_listed_instruction_the_operation_arm_decodes() {
    let mut operations = 0;
    let mut differing = Vec::new();
    for instruction in LISTED_INSTRUCTIONS {
        let record = instruction
            .to_record()
            .expect("every listed instruction has a record");
        let decoded = to_decoded_load_store(record.fixed_bits);
        let tabled = load_store::to_load_store_operation(instruction);
        if tabled != decoded {
            differing.push(alloc::format!(
                "{instruction:?} ({}): {tabled:?}, decoded {decoded:?}",
                record.mnemonic
            ));
        }
        operations += usize::from(decoded.is_some());
    }
    assert!(differing.is_empty(), "{}", differing.join("\n"));
    assert!(operations > 300, "{operations} loads and stores");
}

#[test]
fn the_architecture_addresses_the_lower_48_bits_and_has_one_value() {
    // A 4 KiB granule without FEAT_LVA gives EL0 the virtual addresses below 2^48.
    assert_eq!(
        Arm64::<User>::ADDRESSABLE,
        0..0x0001_0000_0000_0000,
        "the lower half's 48 bits"
    );
    // The architecture holds no state, so every value of it is every other.
    let architecture = Arm64::<User>::default();
    assert!(architecture == Arm64::<User>::default());
    assert!(architecture.clone() == architecture);
}
