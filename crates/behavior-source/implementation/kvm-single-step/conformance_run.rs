//! The behavior source suite on this host's chip, which needs ARM64 Linux with KVM.

#![cfg(all(target_arch = "aarch64", target_os = "linux"))]

use wcontainers_abi::PAGE_SIZE;
use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, ExclusiveMonitor, InstructionFetch, PagePermission, Processor, User,
    ZERO_PROCESSOR,
};

fn chip() -> KvmSingleStepMachine {
    match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

/// States the chip cannot run: a permission that starts inside a page, more mapped pages than the guest has, and an
/// exclusive monitor that holds an address.
fn to_unexecutable() -> Vec<ExecutionState<'static, Arm64<User>>> {
    let read_write = PagePermission::Mapped {
        data_access: DataAccess::ReadWrite,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    [
        PAGE_SIZE + 1..2 * PAGE_SIZE,
        0..Arm64::<User>::ADDRESSABLE.end,
    ]
    .into_iter()
    .map(|addresses| ExecutionState {
        processor: ZERO_PROCESSOR,
        memory_mutations: vec![MemoryDelta::PermissionAssignment {
            addresses,
            permission: read_write,
        }],
    })
    .chain([ExecutionState {
        processor: Processor {
            exclusive_monitor: ExclusiveMonitor::Exclusive { address: PAGE_SIZE },
            ..ZERO_PROCESSOR
        },
        memory_mutations: Vec::new(),
    }])
    .collect()
}

wcontainers_behavior_source_conformance::tests!(
    chip(),
    CriticalValueCut::default(),
    to_unexecutable()
);
