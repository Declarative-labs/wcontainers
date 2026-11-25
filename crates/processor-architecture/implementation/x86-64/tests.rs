use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator};

use crate::{
    Access, DataAccess, Exception, Flags, Instruction, InstructionFetch, PagePermission, PreciseInstruction,
    Processor, STACK_POINTER, StoreAcrossFault, X86_64, ZERO_PROCESSOR,
};

const CODE: u64 = 0x1000;
const DATA: u64 = 0x2000;

fn to_state(code: &[u8], processor: Processor, data: PagePermission) -> ExecutionState<'static, X86_64> {
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    ExecutionState {
        processor: Processor {
            instruction_pointer: CODE,
            ..processor
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: CODE..CODE + 0x1000,
                permission: executable,
            },
            MemoryDelta::ValueAssignment {
                start: CODE,
                bytes: Cow::Owned(code.to_vec()),
            },
            MemoryDelta::PermissionAssignment {
                addresses: DATA..DATA + 0x1000,
                permission: data,
            },
        ],
    }
}

fn first_behavior(instruction: Instruction) -> PreciseInstruction {
    X86_64::list_known_behaviors(instruction).next().expect("a modeled form")
}

fn simulate(
    behavior: PreciseInstruction,
    state: ExecutionState<'static, X86_64>,
) -> (Processor, Vec<MemoryDelta<'static, X86_64>>) {
    let start = state.memory_mutations.len();
    let Ok(after) = behavior.simulate_state(state);
    (after.processor, after.memory_mutations[start..].to_vec())
}

#[test]
fn add_of_32_bits_clears_the_upper_half_and_sets_carry_and_parity() {
    // add eax, ecx
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[0] = 0xDEAD_BEEF_FFFF_FFFF;
    processor.general_purpose_registers[1] = 1;
    let (after, _) = simulate(
        first_behavior(Instruction::AddGprvGprv01),
        to_state(&[0x01, 0xC8], processor, PagePermission::Unmapped),
    );
    assert_eq!(after.general_purpose_registers[0], 0);
    assert_eq!(
        after.flags,
        Flags::CARRY | Flags::ZERO | Flags::PARITY | Flags::AUXILIARY_CARRY
    );
    assert_eq!(after.instruction_pointer, CODE + 2);
}

#[test]
fn cmovle_of_32_bits_clears_the_upper_half_when_it_does_not_move() {
    // cmovle eax, ecx, with every flag clear
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[0] = 0x1234_5678_9ABC_DEF0;
    let (after, _) = simulate(
        first_behavior(Instruction::CmovleGprvGprv),
        to_state(&[0x0F, 0x4E, 0xC1], processor, PagePermission::Unmapped),
    );
    assert_eq!(after.general_purpose_registers[0], 0x9ABC_DEF0);
}

#[test]
fn push_onto_a_read_only_page_faults_and_keeps_the_stack_pointer() {
    // push rax
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[STACK_POINTER] = DATA + 8;
    let read_only = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    let (after, written) = simulate(
        PreciseInstruction::PushGprv50 {
            across_fault: StoreAcrossFault::WritesNothing,
        },
        to_state(&[0x50], processor, read_only),
    );
    assert_eq!(after.general_purpose_registers[STACK_POINTER], DATA + 8);
    assert_eq!(after.instruction_pointer, CODE);
    assert_eq!(
        after.exception,
        Some(Exception::PageFault {
            access: Access::Write,
            page_present: true,
            address: DATA,
        })
    );
    assert!(written.is_empty());
}

#[test]
fn int3_moves_past_itself() {
    let (after, _) = simulate(
        PreciseInstruction::Int3,
        to_state(&[0xCC], ZERO_PROCESSOR, PagePermission::Unmapped),
    );
    assert_eq!(after.instruction_pointer, CODE + 1);
    assert_eq!(after.exception, Some(Exception::Breakpoint));
}

#[test]
fn an_instruction_of_16_bytes_raises_general_protection() {
    // 13 DS prefixes and add rax, rbx
    let mut code = vec![0x3E; 13];
    code.extend([0x48, 0x01, 0xD8]);
    let (after, _) = simulate(
        first_behavior(Instruction::AddGprvGprv01),
        to_state(&code, ZERO_PROCESSOR, PagePermission::Unmapped),
    );
    assert_eq!(after.exception, Some(Exception::GeneralProtection { error_code: 0 }));
    assert_eq!(after.instruction_pointer, CODE);
}

#[test]
fn a_fetch_past_the_last_canonical_address_raises_general_protection() {
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let last_page = 0x7FFF_FFFF_F000;
    let state = ExecutionState {
        processor: Processor {
            instruction_pointer: 0x7FFF_FFFF_FFFE,
            ..ZERO_PROCESSOR
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: last_page..last_page + 0x1000,
                permission: executable,
            },
            MemoryDelta::ValueAssignment {
                start: 0x7FFF_FFFF_FFFE,
                bytes: Cow::Owned(vec![0x48, 0x01]),
            },
        ],
    };
    let (after, _) = simulate(first_behavior(Instruction::AddGprvGprv01), state);
    assert_eq!(after.exception, Some(Exception::GeneralProtection { error_code: 0 }));
}

#[test]
fn a_non_canonical_address_through_fs_raises_general_protection_even_from_rbp() {
    // mov rax, fs:[rbp]
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[5] = 1 << 63;
    let (after, _) = simulate(
        first_behavior(Instruction::MovGprvMemv),
        to_state(&[0x64, 0x48, 0x8B, 0x45, 0x00], processor, PagePermission::Unmapped),
    );
    assert_eq!(after.exception, Some(Exception::GeneralProtection { error_code: 0 }));
}

#[test]
fn a_misaligned_load_with_alignment_checking_raises_alignment_check() {
    // mov rax, [rbx]
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[3] = DATA + 1;
    processor.flags = Flags::ALIGNMENT_CHECK;
    let readable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    let (after, _) = simulate(
        first_behavior(Instruction::MovGprvMemv),
        to_state(&[0x48, 0x8B, 0x03], processor, readable),
    );
    assert_eq!(after.exception, Some(Exception::AlignmentCheck));
}

#[test]
fn a_load_from_the_kernel_half_faults_on_a_present_page() {
    // mov rax, [rbx]
    let mut processor = ZERO_PROCESSOR;
    processor.general_purpose_registers[3] = 0xFFFF_FFFF_FFE0_5000;
    let (after, _) = simulate(
        first_behavior(Instruction::MovGprvMemv),
        to_state(&[0x48, 0x8B, 0x03], processor, PagePermission::Unmapped),
    );
    assert_eq!(
        after.exception,
        Some(Exception::PageFault {
            access: Access::Read,
            page_present: true,
            address: 0xFFFF_FFFF_FFE0_5000,
        })
    );
}
