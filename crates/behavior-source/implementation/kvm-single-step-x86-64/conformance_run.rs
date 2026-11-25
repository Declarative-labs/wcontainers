//! The behavior source suite on this host's chip, which needs x86-64 Linux with KVM.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::borrow::Cow;

use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

fn chip() -> SingleStepMachine {
    match SingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

/// The address the code of each state below starts at, on its own executable page.
const CODE: u64 = 0x1000;

/// A state whose code page holds the bytes, with the changes after it.
fn make(code: &[u8], after: Vec<MemoryDelta<'static, X86_64>>) -> ExecutionState<'static, X86_64> {
    let mut memory_mutations = vec![
        MemoryDelta::PermissionAssignment {
            addresses: CODE..2 * CODE,
            permission: PagePermission::Mapped {
                data_access: DataAccess::ReadOnly,
                instruction_fetch: InstructionFetch::Allowed,
            },
        },
        MemoryDelta::ValueAssignment {
            start: CODE,
            bytes: Cow::Owned(code.to_vec()),
        },
    ];
    memory_mutations.extend(after);
    ExecutionState {
        processor: Processor {
            instruction_pointer: CODE,
            ..ZERO_PROCESSOR
        },
        memory_mutations,
    }
}

/// States the chip refuses: a permission that starts inside a page, bytes past the program's addresses, SYSCALL,
/// which it cannot step, and FLD1, which it runs and which changes the x87 state, which is not modeled.
fn to_unexecutable() -> Vec<ExecutionState<'static, X86_64>> {
    const ADD: [u8; 3] = [0x48, 0x01, 0xD8];
    vec![
        make(
            &ADD,
            vec![MemoryDelta::PermissionAssignment {
                addresses: 0x2001..0x3000,
                permission: PagePermission::Unmapped,
            }],
        ),
        make(
            &ADD,
            vec![MemoryDelta::ValueAssignment {
                start: 0x8000_0000_0000,
                bytes: Cow::Owned(vec![0xCC; 8]),
            }],
        ),
        make(&[0x0F, 0x05], Vec::new()),
        make(&[0xD9, 0xE8], Vec::new()),
    ]
}

wcontainers_behavior_source_conformance::tests!(
    chip(),
    CriticalValueCut::new(X86_64InputDomain),
    to_unexecutable()
);
