//! Prints the significands RCPSS and RSQRTSS give in the chip source's guest, as the Rust of `reciprocal_recording.rs`:
//! RCPSS of 1 plus each value of the top 11 significand bits, and RSQRTSS of 2 and of 1 plus each value of the top 10.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example record_reciprocal`

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    use std::borrow::Cow;

    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
    use wcontainers_processor_architecture_x86_64::{
        DataAccess, InstructionFetch, PagePermission, Processor, ZERO_PROCESSOR,
    };

    const CODE: u64 = 0x1000;
    const PAGE: u64 = 0x1000;
    // RCPSS XMM0, XMM1 and RSQRTSS XMM0, XMM1.
    const RECIPROCAL: [u8; 4] = [0xF3, 0x0F, 0x53, 0xC1];
    const ROOT: [u8; 4] = [0xF3, 0x0F, 0x52, 0xC1];
    let mut machine = match SingleStepMachine::new() {
        Ok(machine) => machine,
        Err(error) => panic!("no single step machine: {error:?}"),
    };
    let mut run = |code: [u8; 4], source: u32| -> u16 {
        let mut processor = Processor { instruction_pointer: CODE, ..ZERO_PROCESSOR };
        processor.vector_registers[1] = u128::from(source);
        let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
        let state = ExecutionState {
            processor,
            memory_mutations: vec![
                MemoryDelta::PermissionAssignment { addresses: CODE..CODE + PAGE, permission: executable },
                MemoryDelta::ValueAssignment { start: CODE, bytes: Cow::Owned(code.to_vec()) },
            ],
        };
        let after = match machine.execute_single_instruction(state) {
            Ok(after) => after,
            Err(refusal) => panic!("the chip refuses {code:x?}: {refusal:?}"),
        };
        let result = after.processor.vector_registers[0] as u32;
        assert!(result >> 23 == 126 && result & 0x7FF == 0, "{source:#x} gives {result:#x}, outside the table's shape");
        ((result & 0x7F_FFFF) >> 11) as u16
    };
    let reciprocal: Vec<u16> = (0..2048).map(|index| run(RECIPROCAL, 127 << 23 | index << 12)).collect();
    let root: Vec<u16> = (0..2048).map(|index| run(ROOT, (if index >> 10 == 1 { 127 } else { 128 }) << 23 | (index & 1023) << 13)).collect();
    let render = |table: &[u16]| {
        table
            .chunks(16)
            .map(|row| format!("    {}", row.iter().map(|entry| format!("{entry:#05X},")).collect::<Vec<_>>().join(" ")).replace("0X", "0x"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    println!("//! Written by the `record_reciprocal` example of wcontainers-behavior-source-kvm-single-step-x86-64: the significand");
    println!("//! of RCPSS's result for each value of the source's top 11 significand bits, and of RSQRTSS's for each parity of the");
    println!("//! source's exponent (even first) and each value of its top 10 significand bits, as the chip source's guest computes");
    println!("//! them on an Intel Xeon Platinum 8488C.");
    println!();
    println!("pub static RECIPROCAL: [u16; 2048] = [\n{}\n];", render(&reciprocal));
    println!();
    println!("pub static RECIPROCAL_ROOT: [u16; 2048] = [\n{}\n];", render(&root));
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    println!("the x86-64 chip source needs a Linux x86-64 host with KVM");
}
