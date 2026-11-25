use std::borrow::Cow;
use std::path::Path;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, DataAccess, Exception, FaultStatus, Instruction, InstructionFetch,
    PagePermission, User, ZERO_PROCESSOR,
};

use crate::text::{MalformedRecording, from_text, to_file_name, to_text};
use crate::{NotRecorded, Recording};

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
    assert!(matches!(recording.execute_single_instruction(before), Ok(played) if played == after));
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

/// A state the recording holds no case of is an error that writes neither the processor nor the hook, and the states
/// after it play as on a fresh recording.
#[test]
fn recording_plays_only_its_states() {
    let read = || match Recording::read(Path::new(RECORDINGS[0])) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {}: {error}", RECORDINGS[0]),
    };
    let (mut recording, mut fresh) = (read(), read());
    let cases: Vec<_> = fresh
        .cases()
        .take(3)
        .map(|(_, before, _)| before.clone())
        .collect();
    let mut unrecorded = cases[1].clone();
    unrecorded.processor.stack_pointer ^= 1;
    let mut processor = unrecorded.processor.clone();
    let mut mutations: Vec<MemoryDelta<'static, Arm64<User>>> = Vec::new();
    let played = recording.execute_single_instruction(cases[0].clone());
    let refused = recording.execute_single_instruction_in_place(
        &mut processor,
        &unrecorded.memory_mutations,
        &mut mutations,
    );
    assert!(matches!(refused, Err(NotRecorded)));
    assert!(processor == unrecorded.processor && mutations.is_empty());
    assert!(
        matches!((played, fresh.execute_single_instruction(cases[0].clone())), (Ok(first), Ok(second)) if first == second)
    );
    for before in &cases[1..] {
        let after = recording.execute_single_instruction(before.clone());
        let expected = fresh.execute_single_instruction(before.clone());
        assert!(matches!((after, expected), (Ok(after), Ok(expected)) if after == expected));
    }
}

#[test]
fn a_level_0_fault_and_an_undecoded_exception_read_back_as_written() {
    let instruction = Instruction::StrImmediateUnsignedOffset64;
    let cases: Vec<_> = [
        Exception::DataAbort {
            status: FaultStatus::TranslationFaultLevel0,
            access: AccessDirection::Write,
            address: Arm64::<User>::ADDRESSABLE.end,
        },
        Exception::Undecoded {
            syndrome: 0x5600_0000,
            fault_address: u64::MAX,
        },
    ]
    .into_iter()
    .map(|exception| {
        let before = to_state(&NO_CHANGES, &NO_CHANGES);
        let mut after = before.clone();
        after.processor.exception = Some(exception);
        (before, after)
    })
    .collect();
    let text = to_text(instruction, &cases);
    let Ok((named, read)) = from_text(&text) else {
        panic!("the text does not read: {text}");
    };
    assert!(named == instruction);
    let exceptions = |cases: &[(_, ExecutionState<'_, Arm64<User>>)]| {
        cases
            .iter()
            .map(|(_, after)| after.processor.exception)
            .collect::<Vec<_>>()
    };
    assert_eq!(exceptions(&read), exceptions(&cases));
}
