use std::borrow::Cow;

use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_simulation::Simulation;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, Instruction, InstructionFetch, PagePermission,
};

use crate::{MalformedRecording, from_text, leave_same_memory, record, to_text};

const READ_WRITE: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::ReadWrite,
    instruction_fetch: InstructionFetch::Forbidden,
};

const NO_CHANGES: [MemoryDelta<'static, Arm64>; 0] = [];

fn to_value_assignment(start: u64, bytes: &[u8]) -> MemoryDelta<'_, Arm64> {
    MemoryDelta::ValueAssignment {
        start,
        bytes: Cow::Borrowed(bytes),
    }
}

#[test]
fn text_gives_back_every_recorded_state() {
    for instruction in Arm64::list_instructions() {
        let recording = record(
            &mut Simulation::new(AppleM5),
            &CriticalValueProduct,
            instruction,
        );
        let text = to_text(&recording);
        let Ok(read) = from_text(&text) else {
            panic!("the text of {instruction:?} does not read back");
        };
        assert!(read.instruction() == instruction);
        assert!(read.cases() == recording.cases(), "{instruction:?}");
    }
}

#[test]
fn text_written_by_hand_reads_as_the_states_it_names() {
    let text = "instruction StrImmediateUnsignedOffset64
processor x0=0x1122334455667788 x1=0x2000 pc=0x1000 exception=none
before | permission 0x1000..0x2000 read-only+execute; value 0x1000 200000f9; permission 0x2000..0x3000 read-write
after pc=0x1004 | value 0x2000 8877665544332211
";
    let Ok(recording) = from_text(text) else {
        panic!("the text does not read");
    };
    let [(before, after)] = recording.cases() else {
        panic!("one case is written");
    };
    assert_eq!(before.processor.general_purpose_registers[1], 0x2000);
    assert_eq!(after.processor.program_counter, 0x1004);
    assert!(
        after.memory_mutations[before.memory_mutations.len()..]
            == [to_value_assignment(
                0x2000,
                &0x1122_3344_5566_7788_u64.to_le_bytes()
            )]
    );
    let Ok(simulated) = recording.simulate(before.clone()) else {
        panic!("the recorded state is not found");
    };
    assert!(simulated == *after);
}

#[test]
fn malformed_line_is_named() {
    let text = "instruction Brk\nprocessor x0=0x0\nbefore x0=zero |\n";
    assert_eq!(from_text(text).err(), Some(MalformedRecording::Line(3)));
    assert_eq!(
        from_text("instruction Brk\nprocessor pc=0x0\nbefore |\n").err(),
        Some(MalformedRecording::UnfinishedCase)
    );
}

#[test]
fn byte_rewritten_with_its_value_is_no_change() {
    let memory = [
        MemoryDelta::PermissionAssignment {
            addresses: 0x2000..0x3000,
            permission: READ_WRITE,
        },
        to_value_assignment(0x2000, &[1, 2, 3, 4]),
    ];
    let whole = [to_value_assignment(0x2000, &[1, 9, 3, 4])];
    let changed_only = [to_value_assignment(0x2001, &[9])];
    let other = [to_value_assignment(0x2001, &[8])];
    assert!(leave_same_memory(&memory, &whole, &changed_only));
    assert!(!leave_same_memory(&memory, &whole, &other));
    assert!(!leave_same_memory(&memory, &whole, &NO_CHANGES));
}

#[test]
fn permission_changed_inside_a_range_is_seen() {
    let memory = [MemoryDelta::PermissionAssignment {
        addresses: 0x2000..0x4000,
        permission: READ_WRITE,
    }];
    let upper_unmapped = [MemoryDelta::PermissionAssignment {
        addresses: 0x3000..0x4000,
        permission: PagePermission::Unmapped,
    }];
    let whole_rewritten = [MemoryDelta::PermissionAssignment {
        addresses: 0x2000..0x4000,
        permission: READ_WRITE,
    }];
    assert!(leave_same_memory(&memory, &whole_rewritten, &NO_CHANGES));
    assert!(!leave_same_memory(
        &memory,
        &whole_rewritten,
        &upper_unmapped
    ));
}

#[test]
fn recording_answers_only_its_states() {
    let recording = record(
        &mut Simulation::new(AppleM5),
        &CriticalValueProduct,
        Instruction::Brk,
    );
    let mut unrecorded = recording.cases()[0].0.clone();
    unrecorded.processor.stack_pointer ^= 1;
    assert!(recording.simulate(unrecorded).is_err());
}
