use std::borrow::Cow;
use std::path::Path;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission, User, ZERO_PROCESSOR,
};

use crate::Recording;
use crate::text::{MalformedRecording, from_text, to_file_name, to_text};

/// The folders of the committed recordings, one for each chip.
const RECORDINGS: [&str; 2] = [
    concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/neoverse-v2"),
    concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/apple-m5"),
];

const READ_WRITE: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::ReadWrite,
    instruction_fetch: InstructionFetch::Forbidden,
};

const NO_CHANGES: [MemoryDelta<'static, Arm64<User>>; 0] = [];

fn to_value_assignment(start: u64, bytes: &[u8]) -> MemoryDelta<'_, Arm64<User>> {
    MemoryDelta::ValueAssignment {
        start,
        bytes: Cow::Borrowed(bytes),
    }
}

/// The state that holds the memory and then the changes, with every register zero.
fn to_state<'memory>(
    memory: &[MemoryDelta<'memory, Arm64<User>>],
    changes: &[MemoryDelta<'memory, Arm64<User>>],
) -> ExecutionState<'memory, Arm64<User>> {
    ExecutionState {
        processor: ZERO_PROCESSOR,
        memory_mutations: [memory, changes].concat(),
    }
}

#[test]
fn every_committed_recording_reads_and_writes_back_byte_for_byte() {
    for folder in RECORDINGS {
        for instruction in Arm64::<User>::list_instructions() {
            let path = Path::new(folder).join(to_file_name(instruction));
            let Ok(committed) = std::fs::read_to_string(&path) else {
                panic!("reading {}", path.display());
            };
            let Ok((named, cases)) = from_text(&committed) else {
                panic!("{} does not read", path.display());
            };
            assert!(named == instruction);
            assert!(
                to_text(instruction, &cases) == committed,
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn text_written_by_hand_plays_back_the_state_it_names() {
    let text = "instruction StrImmediateUnsignedOffset64
processor x0=0x1122334455667788 x1=0x2000 pc=0x1000 exception=none
before | permission 0x1000..0x2000 read-only+execute; value 0x1000 200000f9; permission 0x2000..0x3000 read-write
after pc=0x1004 | value 0x2000 8877665544332211
";
    let Ok((instruction, cases)) = from_text(text) else {
        panic!("the text does not read");
    };
    let [(before, after)] = &cases[..] else {
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
    let (before, after) = (before.clone(), after.clone());
    let mut recording = Recording {
        instructions: vec![(instruction, cases)],
        next: (0, 0),
        played: None,
    };
    assert!(recording.execute_single_instruction(before) == after);
}

#[test]
fn malformed_line_is_named() {
    let text = "instruction Brk\nprocessor x0=0x0\nbefore x0=zero |\n";
    assert_eq!(
        from_text(text).err(),
        Some(MalformedRecording::UnreadableLine(3))
    );
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
    assert!(to_state(&memory, &whole) == to_state(&memory, &changed_only));
    assert!(to_state(&memory, &whole) != to_state(&memory, &other));
    assert!(to_state(&memory, &whole) != to_state(&memory, &NO_CHANGES));
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
    assert!(to_state(&memory, &whole_rewritten) == to_state(&memory, &NO_CHANGES));
    assert!(to_state(&memory, &whole_rewritten) != to_state(&memory, &upper_unmapped));
}

#[test]
#[should_panic(expected = "the recording holds no state equal to the one given")]
fn recording_plays_only_its_states() {
    let Ok(mut recording) = Recording::read(Path::new(RECORDINGS[0])) else {
        panic!("reading {}", RECORDINGS[0]);
    };
    let Some((_, before, _)) = recording.cases().next() else {
        panic!("the recording holds a case");
    };
    let mut unrecorded = before.clone();
    unrecorded.processor.stack_pointer ^= 1;
    recording.execute_single_instruction(unrecorded);
}
