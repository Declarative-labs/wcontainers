//! Form I as a behavior source: one instruction run alone in a memory made of the deltas, the bytes it wrote reported
//! and the processor it left given back, each checked against what the Arm ARM says the instruction does.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::borrow::Cow;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor_in_process::execution::{InProcess, SingleInstructionRefusal};
use wcontainers_executor_in_process::memory::{GUEST_PAGE, InProcessPool};
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, MemorySnapshot};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, DataAccess, Exception, ExclusiveMonitor, FaultStatus, InstructionFetch,
    PagePermission, Processor, User, ZERO_PROCESSOR,
};

/// Where the instruction is, its data page, and a page it may do nothing with.
const CODE: u64 = 0x4_0000;
const DATA: u64 = 0x10_0000;
const NOTHING: u64 = 0x30_0000;
/// The instructions ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions)):
/// `str x3, [x2, #8]`, `ldrb w5, [x2, #16]`, `svc #0`, `ldr x0, [x1]` and `b .`.
const STORE_X3: u32 = 0xf900_0443;
const LOAD_BYTE_X5: u32 = 0x3940_4045;
const SYSTEM_CALL: u32 = 0xd400_0001;
const LOAD_THROUGH_X1: u32 = 0xf940_0020;
const BRANCH_TO_ITSELF: u32 = 0x1400_0000;
/// Where `str` writes past x2, and `ldrb` reads.
const STORED_AT: u64 = 8;
const BYTE_AT: u64 = 16;
/// The registers: the address of the data, what is stored, what is loaded, the faulting access's address.
const ADDRESS: usize = 2;
const STORED: usize = 3;
const LOADED: usize = 5;
const FAULTING: usize = 1;
/// A word with no zero byte, so that every one of its bytes is a change; and the byte the data holds.
const WORD: u64 = 0x1122_3344_5566_7788;
const BYTE: u8 = 0xab;
/// The size of an instruction.
const INSTRUCTION: u64 = 4;

/// The bytes the instruction wrote, as the source reported them.
#[derive(Default)]
struct Written(Vec<(u64, Vec<u8>)>);

impl MemorySink<Arm64<User>> for Written {
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Arm64<User>>) {
        match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => {
                self.0.push((start, bytes.into_owned()))
            }
            MemoryDelta::PermissionAssignment { addresses, .. } => {
                panic!("a single instruction changed the permissions of {addresses:x?}")
            }
        }
    }
}

fn read_write() -> PagePermission {
    PagePermission::Mapped {
        data_access: DataAccess::ReadWrite,
        instruction_fetch: InstructionFetch::Forbidden,
    }
}

fn read_execute() -> PagePermission {
    PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    }
}

/// The memory: the data page writable, then the instruction's page executable (in that order, so that a later
/// assignment that leaves the data page alone keeps it writable), the instruction, and one byte of data.
fn deltas(word: u32) -> Vec<MemoryDelta<'static, Arm64<User>>> {
    vec![
        MemoryDelta::PermissionAssignment {
            addresses: DATA..DATA + GUEST_PAGE,
            permission: read_write(),
        },
        MemoryDelta::PermissionAssignment {
            addresses: CODE..CODE + GUEST_PAGE,
            permission: read_execute(),
        },
        MemoryDelta::ValueAssignment {
            start: CODE,
            bytes: Cow::Owned(word.to_le_bytes().to_vec()),
        },
        MemoryDelta::ValueAssignment {
            start: DATA + BYTE_AT,
            bytes: Cow::Owned(vec![BYTE]),
        },
    ]
}

/// A processor at the instruction, x2 the data's address, x3 the word stored and x1 a page that allows nothing.
fn processor() -> Processor<()> {
    let mut processor = ZERO_PROCESSOR;
    for (r, value) in processor.general_purpose_registers.iter_mut().enumerate() {
        *value = r as u64 * 0x101;
    }
    processor.general_purpose_registers[ADDRESS] = DATA;
    processor.general_purpose_registers[STORED] = WORD;
    processor.general_purpose_registers[FAULTING] = NOTHING;
    processor.program_counter = CODE;
    processor
}

/// Runs the instruction `word` from [`processor`] and returns the processor it left and the bytes it wrote.
fn run(word: u32) -> (Processor<()>, Written) {
    let mut source = InProcess::new(InProcessPool::new().expect("a host with the guest's pages"));
    let deltas = deltas(word);
    let mut ran = processor();
    let mut written = Written::default();
    source
        .execute_single_instruction_in_place(
            &mut ran,
            MemorySnapshot { mutations: &deltas },
            &mut written,
        )
        .expect("the instruction runs");
    (ran, written)
}

#[test]
fn a_store_writes_its_bytes_and_the_pc_moves_on() {
    let (ran, written) = run(STORE_X3);
    assert_eq!(written.0, [(DATA + STORED_AT, WORD.to_le_bytes().to_vec())]);
    let wanted = Processor {
        program_counter: CODE + INSTRUCTION,
        ..processor()
    };
    assert_eq!(ran, wanted);
}

#[test]
fn a_load_reads_a_byte_the_deltas_wrote() {
    let (ran, written) = run(LOAD_BYTE_X5);
    assert!(written.0.is_empty());
    let mut wanted = Processor {
        program_counter: CODE + INSTRUCTION,
        ..processor()
    };
    wanted.general_purpose_registers[LOADED] = u64::from(BYTE);
    assert_eq!(ran, wanted);
}

#[test]
fn a_system_call_and_a_fault_end_at_their_exceptions() {
    let (ran, written) = run(SYSTEM_CALL);
    assert!(written.0.is_empty());
    assert_eq!(
        ran,
        Processor {
            program_counter: CODE + INSTRUCTION,
            exception: Some(Exception::SupervisorCall { immediate: 0 }),
            ..processor()
        }
    );
    let (ran, written) = run(LOAD_THROUGH_X1);
    assert!(written.0.is_empty());
    assert_eq!(
        ran,
        Processor {
            exception: Some(Exception::DataAbort {
                status: FaultStatus::TranslationFaultLevel3,
                access: AccessDirection::Read,
                address: NOTHING,
            }),
            ..processor()
        }
    );
}

#[test]
fn a_branch_to_itself_runs_and_stays() {
    let (ran, written) = run(BRANCH_TO_ITSELF);
    assert!(written.0.is_empty());
    assert_eq!(ran, processor());
}

#[test]
fn a_state_whose_exclusive_monitor_holds_an_address_is_refused() {
    let mut source = InProcess::new(InProcessPool::new().expect("a host with the guest's pages"));
    let held = ExclusiveMonitor::Exclusive { address: DATA };
    let mut state = Processor {
        exclusive_monitor: held,
        ..processor()
    };
    let deltas = deltas(STORE_X3);
    let mut written = Written::default();
    assert_eq!(
        source.execute_single_instruction_in_place(
            &mut state,
            MemorySnapshot { mutations: &deltas },
            &mut written,
        ),
        Err(SingleInstructionRefusal::ExclusiveMonitorHeld(held))
    );
    assert!(written.0.is_empty());
    assert_eq!(state.program_counter, CODE);
}
