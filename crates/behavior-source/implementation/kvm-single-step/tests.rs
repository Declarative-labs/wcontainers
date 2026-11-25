//! Every way a single step can end names its result: the decoder over every syndrome, and words outside the ten
//! instructions on the live chip.

use std::borrow::Cow;

use aarch64_cpu::registers::ESR_EL1;
use aarch64_cpu::registers::ESR_EL1::EC::Value as Class;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, InstructionFetch, PagePermission, User,
    ZERO_PROCESSOR,
};

use crate::KvmSingleStepMachine;
use crate::guest_layout::HANDLER_HALF;
use crate::syndrome::{to_exception, to_syndrome};

/// Class-specific bits that each name something in some class: none, a level 0 write fault, the most a BRK comment
/// holds, and all of them.
const SPECIFIC_BITS: [u64; 4] = [u64::MIN, 0x44, u16::MAX as u64, (1 << 25) - 1];

#[test]
fn every_syndrome_decodes_to_an_exception_that_gives_it_back() {
    let classes = u64::MIN..=ESR_EL1::EC.mask;
    for class in classes {
        for specific in SPECIFIC_BITS {
            for length in [ESR_EL1::IL::CLEAR.value, ESR_EL1::IL::SET.value] {
                let syndrome = ESR_EL1::EC.val(class).value | length | specific;
                match to_exception(syndrome, u64::MAX) {
                    None => assert_eq!(class, Class::SoftwareStepLowerEL as u64),
                    Some(exception) => assert_eq!(to_syndrome(exception), syndrome),
                }
            }
        }
    }
}

/// [SVC #0](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call-).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// [HVC #0](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/HVC--Hypervisor-call-), undefined at EL0.
const HYPERVISOR_CALL: u32 = 0xD400_0002;
/// [WFI](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/WFI--Wait-for-interrupt-), which SCTLR_EL1.nTWI clear traps.
const WAIT_FOR_INTERRUPT: u32 = 0xD503_207F;
/// [MSR DAIFSet, #2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MSR--immediate---Move-immediate-value-to-special-register-), which SCTLR_EL1.UMA clear traps.
const MASK_INTERRUPTS: u32 = 0xD503_42DF;
/// [LDR X0, \[X1\]](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--).
const LOAD: u32 = 0xF940_0020;
/// [DC CIVAC, X1](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/DC--Data-cache-operation--an-alias-of-SYS-), whose fault sets CM.
const CLEAN_AND_INVALIDATE: u32 = 0xD50B_7E21;
/// [LDADD X0, X0, \[X1\]](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDADD--LDADDA--LDADDAL--LDADDL--Atomic-add-on-word-or-doubleword-in-memory-), which faults where it leaves its 16 aligned bytes.
const ATOMIC_ADD: u32 = 0xF820_0020;
/// [BRK #0](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction-).
const BREAKPOINT: u32 = 0xD420_0000;

const CODE: u64 = PAGE_SIZE;
const DATA: u64 = 2 * PAGE_SIZE;

/// The state about to run the word at [`CODE`] with X1 at the address, a writable page at [`DATA`] and the flags.
fn to_state(
    word: u32,
    address: u64,
    flags: ConditionFlags,
) -> ExecutionState<'static, Arm64<User>> {
    let mut processor = ZERO_PROCESSOR;
    processor.program_counter = CODE;
    processor.general_purpose_registers[1] = address;
    processor.condition_flags = flags;
    ExecutionState {
        processor,
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: CODE..CODE + PAGE_SIZE,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadOnly,
                    instruction_fetch: InstructionFetch::Allowed,
                },
            },
            MemoryDelta::ValueAssignment {
                start: CODE,
                bytes: Cow::Owned(word.to_le_bytes().to_vec()),
            },
            MemoryDelta::PermissionAssignment {
                addresses: DATA..DATA + PAGE_SIZE,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadWrite,
                    instruction_fetch: InstructionFetch::Forbidden,
                },
            },
        ],
    }
}

/// Words a state may hold that none of the ten instructions is, each ending in an exception the decoder names
/// whole or keeps undecoded; the chip runs every one and goes on to the next.
#[test]
#[ignore = "needs KVM"]
fn words_outside_the_ten_instructions_end_in_a_named_result() {
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let none = ConditionFlags::empty();
    let every_bit = ConditionFlags::from_bits_retain(u32::MAX);
    let class = |exception| match exception {
        Some(Exception::Undecoded { syndrome, .. }) => {
            Some(ESR_EL1::EC.read_as_enum::<Class>(syndrome))
        }
        _ => None,
    };
    let cases = [
        (
            to_state(SUPERVISOR_CALL, DATA, none),
            Some(Some(Class::SVC64)),
        ),
        (to_state(HYPERVISOR_CALL, DATA, none), None),
        (
            to_state(WAIT_FOR_INTERRUPT, DATA, none),
            Some(Some(Class::TrappedWFIorWFE)),
        ),
        (
            to_state(MASK_INTERRUPTS, DATA, none),
            Some(Some(Class::TrappedMsrMrs)),
        ),
        // The handler's own memory, through the linear map at the start of its half.
        (
            to_state(LOAD, HANDLER_HALF, none),
            Some(Some(Class::DataAbortLowerEL)),
        ),
        (
            to_state(CLEAN_AND_INVALIDATE, DATA + PAGE_SIZE, none),
            Some(Some(Class::DataAbortLowerEL)),
        ),
        (
            to_state(ATOMIC_ADD, DATA + 12, none),
            Some(Some(Class::DataAbortLowerEL)),
        ),
        // Bits beyond NZCV would put EL0 in another state; the chip keeps only NZCV.
        (to_state(BREAKPOINT, DATA, every_bit), None),
    ];
    for (before, expected) in cases {
        let word = before
            .memory_mutations
            .iter()
            .find_map(|delta| match delta {
                MemoryDelta::ValueAssignment { bytes, .. } => Some(bytes.clone()),
                MemoryDelta::PermissionAssignment { .. } => None,
            });
        let after = chip.execute_single_instruction(before);
        println!(
            "{word:x?}: {:x?}, flags {:?}",
            after.processor.exception, after.processor.condition_flags
        );
        assert!(after.processor.exception.is_some());
        assert_eq!(class(after.processor.exception), expected, "{word:x?}");
    }
    let after = chip.execute_single_instruction(to_state(BREAKPOINT, DATA, every_bit));
    assert_eq!(after.processor.condition_flags, ConditionFlags::all());
    assert_eq!(
        after.processor.exception,
        Some(Exception::Breakpoint { comment: 0 })
    );
}
