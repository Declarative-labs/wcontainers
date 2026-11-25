//! Every way a single step can end names its result: the decoder over every syndrome, words outside the ten
//! instructions and addresses outside those a program can use on the live chip, and states the chip cannot run, which
//! it refuses before it writes anything.

use std::borrow::Cow;
use std::panic::{AssertUnwindSafe, catch_unwind};

use aarch64_cpu::registers::ESR_EL1;
use aarch64_cpu::registers::ESR_EL1::EC::Value as Class;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, FaultStatus, InstructionFetch,
    PagePermission, User, ZERO_PROCESSOR,
};

use crate::guest_layout::{HANDLER_HALF, linear};
use crate::syndrome::{to_exception, to_syndrome};
use crate::{HANDLER, KvmSingleStepMachine, MemoryLoading, WINDOW_BYTES};

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

/// Words a state may hold that no modeled instruction is, each ending in an exception the decoder names whole or
/// keeps undecoded with its class; the chip runs every one and goes on to the next.
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
        (to_state(SUPERVISOR_CALL, DATA, none), None),
        (to_state(HYPERVISOR_CALL, DATA, none), None),
        (to_state(WAIT_FOR_INTERRUPT, DATA, none), None),
        (to_state(MASK_INTERRUPTS, DATA, none), None),
        (
            to_state(CLEAN_AND_INVALIDATE, DATA + PAGE_SIZE, none),
            Some(Some(Class::DataAbortLowerEL)),
        ),
        (to_state(ATOMIC_ADD, DATA + 12, none), None),
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

/// [STR X0, \[X1\]](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate--).
const STORE: u32 = 0xF900_0020;

/// What X0 holds in the states that store, a value with no zero byte.
const STORED: u64 = 0x0123_4567_89AB_CDEF;

/// The loading modes a machine can be made with.
const LOADINGS: [MemoryLoading; 3] = [
    MemoryLoading::Full,
    MemoryLoading::Incremental,
    MemoryLoading::Checked,
];

fn to_machine(loading: MemoryLoading) -> KvmSingleStepMachine {
    match KvmSingleStepMachine::with_loading(loading) {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

/// The state about to store X0 at the address, with the extra deltas after its own.
fn to_store(
    address: u64,
    extra: Vec<MemoryDelta<'static, Arm64<User>>>,
) -> ExecutionState<'static, Arm64<User>> {
    let mut state = to_state(STORE, address, ConditionFlags::empty());
    state.processor.general_purpose_registers[0] = STORED;
    state.memory_mutations.extend(extra);
    state
}

fn to_permission(
    addresses: std::ops::Range<u64>,
    permission: PagePermission,
) -> MemoryDelta<'static, Arm64<User>> {
    MemoryDelta::PermissionAssignment {
        addresses,
        permission,
    }
}

const READ_WRITE: PagePermission = PagePermission::Mapped {
    data_access: DataAccess::ReadWrite,
    instruction_fetch: InstructionFetch::Forbidden,
};

/// What a run ends in: the exception, the program counter, and where each run of bytes the instruction changed starts
/// with the bytes it holds.
type Outcome = (Option<Exception>, u64, Vec<(u64, Vec<u8>)>);

/// Gives what the instruction does, without the state's own changes.
fn run_on(
    chip: &mut KvmSingleStepMachine,
    before: &ExecutionState<'static, Arm64<User>>,
) -> Outcome {
    let own = before.memory_mutations.len();
    let after = chip.execute_single_instruction(before.clone());
    let changes = after.memory_mutations[own..]
        .iter()
        .map(|mutation| match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => (*start, bytes.to_vec()),
            MemoryDelta::PermissionAssignment { .. } => panic!("the chip changed a permission"),
        })
        .collect();
    (
        after.processor.exception,
        after.processor.program_counter,
        changes,
    )
}

/// A load from the handler's half, or from above the addresses a program can use, faults at level 0, as the
/// architecture's model has it; a tagged address reaches its untagged page, as Linux sets TBI0.
#[test]
#[ignore = "needs KVM"]
fn addresses_a_program_cannot_use_fault_at_level_0() {
    let tagged = |tag: u64| tag << 56 | DATA;
    let addresses = [
        HANDLER_HALF,
        linear(HANDLER),
        Arm64::<User>::ADDRESSABLE.end + DATA,
        tagged(1) + Arm64::<User>::ADDRESSABLE.end,
    ];
    for loading in LOADINGS {
        let mut chip = to_machine(loading);
        for address in addresses {
            let (exception, program_counter, changes) =
                run_on(&mut chip, &to_state(LOAD, address, ConditionFlags::empty()));
            let fault = Exception::DataAbort {
                status: FaultStatus::TranslationFaultLevel0,
                access: AccessDirection::Read,
                address,
            };
            assert_eq!(exception, Some(fault), "{loading:?} {address:#x}");
            assert_eq!((program_counter, changes.len()), (CODE, 0));
        }
        for tag in [1, 0x80, 0xFF] {
            let (exception, program_counter, _) = run_on(
                &mut chip,
                &to_state(LOAD, tagged(tag), ConditionFlags::empty()),
            );
            assert_eq!(exception, None, "{loading:?} {:#x}", tagged(tag));
            assert_eq!(program_counter, CODE + 4);
        }
    }
}

/// Returns the text of a panic.
fn to_message(payload: &(dyn std::any::Any + Send)) -> String {
    match (
        payload.downcast_ref::<String>(),
        payload.downcast_ref::<&str>(),
    ) {
        (Some(message), _) => message.clone(),
        (None, Some(message)) => (*message).to_owned(),
        (None, None) => String::new(),
    }
}

/// A state the chip cannot run stops it before anything is written, saying which rule it breaks, and the states after
/// it run as on a fresh machine in every loading mode.
#[test]
#[ignore = "needs KVM"]
fn a_state_the_chip_cannot_run_is_refused_before_anything_is_written() {
    let refused = [
        (
            to_store(
                DATA,
                vec![to_permission(DATA + 5..DATA + PAGE_SIZE, READ_WRITE)],
            ),
            "a chip gives permissions to whole pages",
        ),
        (
            to_store(DATA, vec![to_permission(0..1 << 40, READ_WRITE)]),
            "pages of the guest",
        ),
        (
            to_store(
                DATA,
                vec![to_permission(0..Arm64::<User>::ADDRESSABLE.end, READ_WRITE)],
            ),
            "pages of the guest",
        ),
    ];
    let afterwards = [
        to_store(DATA + 8, Vec::new()),
        to_state(LOAD, DATA, ConditionFlags::empty()),
        to_store(DATA + PAGE_SIZE, Vec::new()),
    ];
    for loading in LOADINGS {
        let mut fresh = to_machine(loading);
        let expected: Vec<_> = afterwards
            .iter()
            .map(|state| run_on(&mut fresh, state))
            .collect();
        let mut chip = to_machine(loading);
        for (state, rule) in &refused {
            run_on(&mut chip, &afterwards[0]);
            let Err(payload) = catch_unwind(AssertUnwindSafe(|| run_on(&mut chip, state))) else {
                panic!("{loading:?}: the chip ran a state it cannot ({rule})");
            };
            let message = to_message(&*payload);
            assert!(
                message.starts_with("the state is refused before anything is written")
                    && message.contains(rule),
                "{loading:?}: {message}"
            );
            let observed: Vec<_> = afterwards
                .iter()
                .map(|state| run_on(&mut chip, state))
                .collect();
            assert_eq!(observed, expected, "{loading:?} after {rule}");
        }
    }
}

/// An empty permission assigns nothing wherever it lies, and an unmapped range of any size costs nothing: each state
/// runs as the same state without it.
#[test]
#[ignore = "needs KVM"]
fn empty_and_unmapped_ranges_change_nothing() {
    let top = Arm64::<User>::ADDRESSABLE.end;
    let extras = [
        vec![to_permission(5..5, READ_WRITE)],
        vec![to_permission(top + 5..top + 5, READ_WRITE)],
        vec![to_permission(
            DATA + PAGE_SIZE..DATA + PAGE_SIZE,
            READ_WRITE,
        )],
        vec![to_permission(
            DATA + PAGE_SIZE..top,
            PagePermission::Unmapped,
        )],
        vec![to_permission(
            DATA + PAGE_SIZE..u64::MAX,
            PagePermission::Unmapped,
        )],
    ];
    for loading in LOADINGS {
        let mut fresh = to_machine(loading);
        let mut chip = to_machine(loading);
        let plain = to_store(DATA + 8, Vec::new());
        let expected = run_on(&mut fresh, &plain);
        assert_eq!(expected.2.len(), 1);
        for (index, extra) in extras.iter().enumerate() {
            let state = to_store(DATA + 8, extra.clone());
            assert_eq!(
                run_on(&mut chip, &state),
                expected,
                "{loading:?} extra {index}"
            );
        }
        // Unmapped over every address before the state's own permissions.
        let mut state = to_store(DATA + 8, Vec::new());
        state
            .memory_mutations
            .insert(0, to_permission(0..u64::MAX, PagePermission::Unmapped));
        assert_eq!(run_on(&mut chip, &state), expected, "{loading:?}");
    }
}

/// A large writable range runs, and a store across the end of one compared window is one change.
#[test]
#[ignore = "needs KVM"]
fn a_large_writable_range_reports_each_change_once() {
    let large = |address| {
        to_store(
            address,
            vec![to_permission(DATA..DATA + (32 << 20), READ_WRITE)],
        )
    };
    let addresses = [
        DATA + WINDOW_BYTES - 4,
        DATA + 3 * WINDOW_BYTES - 1,
        DATA + (32 << 20) - 8,
    ];
    for loading in LOADINGS {
        let mut chip = to_machine(loading);
        for address in addresses {
            let (exception, _, changes) = run_on(&mut chip, &large(address));
            let stored = (address, STORED.to_le_bytes().to_vec());
            assert_eq!(
                (exception, changes),
                (None, vec![stored]),
                "{loading:?} {address:#x}"
            );
        }
    }
}

/// Pages of the small address space the resolving is compared in.
const MODEL_PAGES: u64 = 16;

/// The permissions the resolving is compared with, the default among them.
const MODEL_PERMISSIONS: [PagePermission; 4] = [
    PagePermission::Unmapped,
    READ_WRITE,
    PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    },
    PagePermission::Mapped {
        data_access: DataAccess::Forbidden,
        instruction_fetch: InstructionFetch::Forbidden,
    },
];

/// Returns the next number of a fixed sequence.
fn to_next(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed >> u32::BITS
}

/// Returns overlapping, empty and reversed permissions over whole pages and values of any length, one wrapping past the
/// last address, in an order the seed chooses.
fn to_changes(seed: u64) -> Vec<MemoryDelta<'static, Arm64<User>>> {
    let mut seed = seed;
    let mut changes = Vec::new();
    for _ in 0..to_next(&mut seed) % 64 {
        let start = to_next(&mut seed) % (MODEL_PAGES + 1) * PAGE_SIZE;
        let end = to_next(&mut seed) % (MODEL_PAGES + 1) * PAGE_SIZE;
        let permission = MODEL_PERMISSIONS[(to_next(&mut seed) % 4) as usize];
        changes.push(to_permission(start..end, permission));
        let start = to_next(&mut seed) % (MODEL_PAGES * PAGE_SIZE);
        let length = to_next(&mut seed) % (2 * PAGE_SIZE) + 1;
        let bytes = (0..length).map(|_| to_next(&mut seed) as u8).collect();
        changes.push(MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Owned(bytes),
        });
    }
    changes.push(MemoryDelta::ValueAssignment {
        start: u64::MAX - 2,
        bytes: Cow::Owned(vec![0xAB; 9]),
    });
    changes
}

/// What the resolved memory gives each page and byte agrees with applying every change in order, page by page and byte
/// by byte; and the resolving of a million ranges in descending order takes time in their number.
#[test]
fn resolving_agrees_with_applying_each_change_in_order() {
    let mut resolved = crate::Resolved::default();
    for seed in 0..500 {
        let changes = to_changes(seed);
        let mut permissions = [PagePermission::Unmapped; MODEL_PAGES as usize];
        let mut bytes = vec![0; (MODEL_PAGES * PAGE_SIZE) as usize];
        for change in &changes {
            match change {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    for page in addresses.clone().step_by(PAGE_SIZE as usize) {
                        permissions[(page / PAGE_SIZE) as usize] = *permission;
                    }
                }
                MemoryDelta::ValueAssignment {
                    start,
                    bytes: value,
                } => {
                    for (offset, byte) in value.iter().enumerate() {
                        let address = start.wrapping_add(offset as u64);
                        if let Some(held) = bytes.get_mut(address as usize) {
                            *held = *byte;
                        }
                    }
                }
            }
        }
        if let Err(refusal) = crate::resolve_memory(&changes, &mut resolved) {
            panic!("seed {seed}: {refusal}");
        }
        let mut resolved_permissions = [PagePermission::Unmapped; MODEL_PAGES as usize];
        for range in &resolved.mapped {
            for page in range.addresses.clone().step_by(PAGE_SIZE as usize) {
                resolved_permissions[(page / PAGE_SIZE) as usize] = PagePermission::Mapped {
                    data_access: range.data_access,
                    instruction_fetch: range.instruction_fetch,
                };
            }
        }
        assert_eq!(resolved_permissions, permissions, "seed {seed}");
        let mut page_bytes = vec![0; PAGE_SIZE as usize];
        for (page, expected) in bytes.chunks(PAGE_SIZE as usize).enumerate() {
            crate::compose_bytes(
                page as u64 * PAGE_SIZE,
                &resolved.values,
                &changes,
                &mut page_bytes,
            );
            assert_eq!(page_bytes, expected, "seed {seed} page {page}");
        }
    }
    let descending: Vec<_> = (0..1_000_000)
        .rev()
        .map(|page| to_permission(page * PAGE_SIZE..(page + 1) * PAGE_SIZE, READ_WRITE))
        .collect();
    if let Err(refusal) = crate::resolve_memory(&descending, &mut resolved) {
        panic!("{refusal}");
    }
    assert_eq!(resolved.mapped.len(), 1_000_000);
}
