//! Guests through `execute` at the edges of the space, of alignment and of copies of pages a guest wrote.

use std::borrow::Cow;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{Access, Cpu, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_unicorn::space::UnicornSpace;
use wcontainers_executor_unicorn::standin::{ALIGNMENT_FAULT_STATUS, StandIn};
use wcontainers_processor_architecture_arm64::syndrome::{FaultStatus, encode_syndrome};
use wcontainers_processor_architecture::MemoryDelta;
use wcontainers_processor_architecture_arm64::{
    DataAccess, Exception, INSTRUCTION_BYTES, InstructionFetch, PagePermission, Processor, ZERO_PROCESSOR,
};
use wcontainers_volatile_bytes::VolatileBytes;

const BRK_0: u32 = 0xd420_0000;
const STR_X1_X0: u32 = 0xf900_0001;
const LDR_X1_X0: u32 = 0xf940_0001;
const LDXR_X1_X0: u32 = 0xc85f_7c01;
const LDXR_X1_SP: u32 = 0xc85f_7fe1;
const BR_X0: u32 = 0xd61f_0000;
const SMC_0: u32 = 0xd400_0003;
const HVC_0: u32 = 0xd400_0002;

const DATA: u64 = LOWEST_ADDRESS + PAGE_SIZE;
const READ_WRITE: Permissions = Permissions::READ.union(Permissions::WRITE);
const READ_EXECUTE: Permissions = Permissions::READ.union(Permissions::EXECUTE);
const VALUE: u64 = 0x1122_3344_5566_7788;

/// Answers every trap terminate and records the faults.
#[derive(Default)]
struct Terminating {
    faults: Vec<(u64, Access)>,
}

impl TrapHandler for Terminating {
    fn handle_system_call(&mut self, _request: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.faults.push((address, access));
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Terminate
    }
}

/// Writes the instructions at the start of the code page and lets it be read and executed.
fn place(space: &UnicornSpace, instructions: &[u32]) {
    let bytes: Vec<u8> = instructions.iter().flat_map(|word| word.to_le_bytes()).collect();
    let end = LOWEST_ADDRESS + u64::try_from(bytes.len()).unwrap();
    space.borrow(LOWEST_ADDRESS..end).unwrap().write(&bytes);
    space.set_permissions(LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE, READ_EXECUTE).unwrap();
}

/// Returns the processor at the code with the registers given.
fn start(registers: &[(usize, u64)]) -> Processor {
    let mut processor = Processor {
        program_counter: LOWEST_ADDRESS,
        ..ZERO_PROCESSOR
    };
    for (register, value) in registers {
        processor.general_purpose_registers[*register] = *value;
    }
    processor
}

/// Returns the state the execution stopped at, which must be ready at its first poll.
fn run(executor: &mut StandIn, space: &UnicornSpace, processor: Processor, handler: &mut Terminating) -> Processor {
    let (execution, _interrupter) = executor.execute(processor, space, handler);
    match pin!(execution).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(state) => state,
        Poll::Pending => panic!("the stand-in's execution blocks, so it is ready at its first poll"),
    }
}

fn read_word(space: &UnicornSpace, address: u64) -> u64 {
    let mut bytes = [u8::MIN; size_of::<u64>()];
    space.borrow(address..address + size_of::<u64>() as u64).unwrap().read(&mut bytes);
    u64::from_le_bytes(bytes)
}

/// Stores the value at the address with a guest's `str x1, [x0]`, which must complete.
fn guest_store(executor: &mut StandIn, space: &UnicornSpace, address: u64, value: u64) {
    place(space, &[STR_X1_X0, BRK_0]);
    let mut handler = Terminating::default();
    let stopped = run(executor, space, start(&[(0, address), (1, value)]), &mut handler);
    assert_eq!(handler.faults, []);
    assert_eq!(stopped.program_counter, LOWEST_ADDRESS + INSTRUCTION_BYTES, "{stopped:?}");
}

#[test]
fn a_load_past_the_end_is_a_translation_fault() {
    let mut executor = StandIn::new();
    let end = executor.address_space().end;
    let space = executor.create_guest_memory().unwrap();
    place(&space, &[LDR_X1_X0, BRK_0]);
    let mut handler = Terminating::default();
    let stopped = run(&mut executor, &space, start(&[(0, end + PAGE_SIZE)]), &mut handler);
    assert_eq!(handler.faults, [(end + PAGE_SIZE, Access::Read)]);
    assert_eq!(stopped.program_counter, LOWEST_ADDRESS);
    assert_eq!(
        stopped.exception,
        Some(Exception {
            syndrome: encode_syndrome(
                ESR_EL1::EC::Value::DataAbortLowerEL,
                FaultStatus::TranslationFaultLevel3 as u64
            ),
            fault_address: Some(end + PAGE_SIZE),
        })
    );
}

#[test]
fn a_jump_to_the_end_is_a_translation_fault() {
    let mut executor = StandIn::new();
    let end = executor.address_space().end;
    let space = executor.create_guest_memory().unwrap();
    place(&space, &[BR_X0]);
    let mut handler = Terminating::default();
    let stopped = run(&mut executor, &space, start(&[(0, end)]), &mut handler);
    assert_eq!(handler.faults, [(end, Access::Execute)]);
    assert_eq!(stopped.program_counter, end);
    assert_eq!(
        stopped.exception,
        Some(Exception {
            syndrome: encode_syndrome(
                ESR_EL1::EC::Value::InstrAbortLowerEL,
                FaultStatus::TranslationFaultLevel3 as u64
            ),
            fault_address: Some(end),
        })
    );
}

#[test]
fn an_unaligned_exclusive_load_is_an_alignment_fault() {
    let mut executor = StandIn::new();
    let space = executor.create_guest_memory().unwrap();
    space.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    place(&space, &[LDXR_X1_X0, BRK_0]);
    let mut handler = Terminating::default();
    let before = start(&[(0, DATA + 1), (1, VALUE)]);
    let stopped = run(&mut executor, &space, before.clone(), &mut handler);
    assert_eq!(handler.faults, []);
    assert_eq!(stopped.program_counter, LOWEST_ADDRESS);
    assert_eq!(stopped.general_purpose_registers, before.general_purpose_registers);
    assert_eq!(
        stopped.exception,
        Some(Exception {
            syndrome: encode_syndrome(ESR_EL1::EC::Value::DataAbortLowerEL, ALIGNMENT_FAULT_STATUS),
            fault_address: Some(DATA + 1),
        })
    );
}

#[test]
fn an_unaligned_exclusive_load_through_the_stack_pointer_is_its_alignment_fault() {
    let mut executor = StandIn::new();
    let space = executor.create_guest_memory().unwrap();
    space.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    place(&space, &[LDXR_X1_SP, BRK_0]);
    let mut handler = Terminating::default();
    let before = Processor {
        stack_pointer: DATA + 1,
        ..start(&[])
    };
    let stopped = run(&mut executor, &space, before, &mut handler);
    assert_eq!(stopped.program_counter, LOWEST_ADDRESS);
    assert_eq!(
        stopped.exception,
        Some(Exception {
            syndrome: encode_syndrome(ESR_EL1::EC::Value::SPAlignmentFault, u64::MIN),
            fault_address: None,
        })
    );
}

#[test]
fn an_unaligned_exclusive_load_alone_is_an_alignment_fault() {
    let mut executor = StandIn::new();
    let mut processor = start(&[(0, DATA + 1)]);
    let code = LDXR_X1_X0.to_le_bytes();
    let memory = [
        MemoryDelta::ValueAssignment {
            start: LOWEST_ADDRESS,
            bytes: Cow::Borrowed(&code),
        },
        MemoryDelta::PermissionAssignment {
            addresses: LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
            permission: PagePermission::Mapped {
                data_access: DataAccess::ReadOnly,
                instruction_fetch: InstructionFetch::Allowed,
            },
        },
        MemoryDelta::PermissionAssignment {
            addresses: DATA..DATA + PAGE_SIZE,
            permission: PagePermission::Mapped {
                data_access: DataAccess::ReadWrite,
                instruction_fetch: InstructionFetch::Forbidden,
            },
        },
    ];
    executor.execute_single_instruction_in_place(&mut processor, &memory, |_| {
        panic!("a fault changes no memory")
    });
    assert_eq!(processor.program_counter, LOWEST_ADDRESS);
    assert_eq!(
        processor.exception,
        Some(Exception {
            syndrome: encode_syndrome(ESR_EL1::EC::Value::DataAbortLowerEL, ALIGNMENT_FAULT_STATUS),
            fault_address: Some(DATA + 1),
        })
    );
}

#[test]
fn a_copy_keeps_a_store_to_zeroed_pages_put_under_writable_addresses() {
    let mut executor = StandIn::new();
    let written = executor.create_guest_memory().unwrap();
    let copy = executor.create_guest_memory().unwrap();
    written.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    written.unmap_from(DATA..DATA + PAGE_SIZE).unwrap();
    guest_store(&mut executor, &written, DATA, VALUE);
    copy.copy_from(DATA..DATA + PAGE_SIZE, &written, DATA).unwrap();
    assert_eq!(read_word(&copy, DATA), VALUE);
}

#[test]
fn a_copy_keeps_a_store_made_through_another_space_s_mapping() {
    let mut executor = StandIn::new();
    let owner = executor.create_guest_memory().unwrap();
    let mapper = executor.create_guest_memory().unwrap();
    let copy = executor.create_guest_memory().unwrap();
    mapper.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    mapper.map_from(DATA..DATA + PAGE_SIZE, &owner, DATA).unwrap();
    guest_store(&mut executor, &mapper, DATA, VALUE);
    assert_eq!(read_word(&owner, DATA), VALUE);
    copy.copy_from(DATA..DATA + PAGE_SIZE, &owner, DATA).unwrap();
    assert_eq!(read_word(&copy, DATA), VALUE);
}

#[test]
fn a_copy_of_a_copy_keeps_a_store_the_first_copy_s_guest_made() {
    let mut executor = StandIn::new();
    let original = executor.create_guest_memory().unwrap();
    let first = executor.create_guest_memory().unwrap();
    let second = executor.create_guest_memory().unwrap();
    original.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    original.unmap_from(DATA..DATA + PAGE_SIZE).unwrap();
    first.copy_from(DATA..DATA + PAGE_SIZE, &original, DATA).unwrap();
    guest_store(&mut executor, &first, DATA, VALUE);
    second.copy_from(DATA..DATA + PAGE_SIZE, &first, DATA).unwrap();
    assert_eq!(read_word(&second, DATA), VALUE);
    assert_eq!(read_word(&original, DATA), u64::MIN);
}

#[test]
fn calls_to_higher_levels_are_undefined_at_el0() {
    for call in [SMC_0, HVC_0] {
        let mut executor = StandIn::new();
        let space = executor.create_guest_memory().unwrap();
        place(&space, &[call, BRK_0]);
        let mut handler = Terminating::default();
        let stopped = run(&mut executor, &space, start(&[]), &mut handler);
        assert_eq!(stopped.program_counter, LOWEST_ADDRESS, "{call:#x}");
        assert_eq!(
            stopped.exception,
            Some(Exception {
                syndrome: encode_syndrome(ESR_EL1::EC::Value::Unknown, u64::MIN),
                fault_address: None,
            }),
            "{call:#x}"
        );
    }
}
