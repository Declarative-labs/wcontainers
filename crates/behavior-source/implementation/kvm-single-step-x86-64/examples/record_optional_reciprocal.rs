//! Prints the significands VRCP14SS, VRSQRT14SS, VRCPSH and VRSQRTSH give in the chip source's guest, as the Rust of
//! `optional_recording.rs`: VRCP14SS of 1 plus each value of the top 16 significand bits, VRSQRT14SS of 1 and 2 plus
//! each value of the top 15, VRCPSH of 1 plus each fraction, VRSQRTSH of 1 and 2 plus each fraction, and VRCPSH of
//! each source of exponent 29 and 30, whose results are tiny.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example record_optional_reciprocal`

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    use std::borrow::Cow;

    use iced_x86::{Code, Encoder, Instruction, Register};
    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
    use wcontainers_processor_architecture_x86_64::{
        DataAccess, InstructionFetch, PagePermission, Processor, ZERO_PROCESSOR,
    };

    const CODE: u64 = 0x1000;
    const PAGE: u64 = 0x1000;
    const MXCSR_DEFAULT: u32 = 0x1F80;
    let encode = |code: Code| -> Vec<u8> {
        let instruction = Instruction::with3(code, Register::XMM0, Register::XMM1, Register::XMM2)
            .unwrap_or_else(|error| panic!("{code:?}: {error}"));
        let mut encoder = Encoder::new(64);
        encoder.encode(&instruction, CODE).unwrap_or_else(|error| panic!("{code:?}: {error}"));
        encoder.take_buffer()
    };
    let mut machine = match SingleStepMachine::new() {
        Ok(machine) => machine,
        Err(error) => panic!("no single step machine: {error:?}"),
    };
    let mut run = |code: &[u8], source: u64| -> u64 {
        let mut processor = Processor { instruction_pointer: CODE, mxcsr: MXCSR_DEFAULT, ..ZERO_PROCESSOR };
        processor.vector_registers[2] = u128::from(source);
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
        assert!(after.processor.exception.is_none() && after.processor.mxcsr == MXCSR_DEFAULT, "{source:#x} raises or flags");
        after.processor.vector_registers[0] as u64
    };
    let reciprocal = encode(Code::EVEX_Vrcp14ss_xmm_k1z_xmm_xmmm32);
    let root = encode(Code::EVEX_Vrsqrt14ss_xmm_k1z_xmm_xmmm32);
    let half_reciprocal = encode(Code::EVEX_Vrcpsh_xmm_k1z_xmm_xmmm16);
    let half_root = encode(Code::EVEX_Vrsqrtsh_xmm_k1z_xmm_xmmm16);
    // The significand of a single result of exponent 126, its low 7 bits clear; 1 plus one ulp stands for 1.
    let single = |result: u64| -> u16 {
        assert!(result >> 23 & 0x1FF == 126 && result & 0x7F == 0, "{result:#x} lies outside the table's shape");
        (result >> 7 & 0xFFFF) as u16
    };
    let half = |result: u64| -> u16 {
        assert!(result >> 10 & 0x3F == 14, "{result:#x} lies outside the table's shape");
        (result & 0x3FF) as u16
    };
    let fourteen: Vec<u16> =
        (0..1u64 << 16).map(|index| single(run(&reciprocal, 127 << 23 | index << 7 | u64::from(index == 0)))).collect();
    let root_fourteen: Vec<u16> = (0..1u64 << 16)
        .map(|index| {
            let (parity, top) = (index >> 15, index & 0x7FFF);
            single(run(&root, (127 + parity) << 23 | top << 8 | u64::from(index == 0)))
        })
        .collect();
    let half_table: Vec<u16> = (0..1u64 << 10).map(|fraction| if fraction == 0 { 0 } else { half(run(&half_reciprocal, 15 << 10 | fraction)) }).collect();
    let half_root_table: Vec<u16> =
        (0..1u64 << 11).map(|index| if index == 0 { 0 } else { half(run(&half_root, (15 + (index >> 10)) << 10 | (index & 0x3FF))) }).collect();
    let tiny: Vec<u16> = (0..1u64 << 11)
        .map(|index| {
            let result = run(&half_reciprocal, (29 + (index >> 10)) << 10 | (index & 0x3FF));
            if index == 0 {
                return 0;
            }
            assert!(result >> 10 == 0, "{index:#x} gives {result:#x}, which is not tiny");
            result as u16
        })
        .collect();
    let render = |name: &str, table: &[u16]| {
        let rows: Vec<String> = table
            .chunks(16)
            .map(|row| format!("    {}", row.iter().map(|entry| format!("{entry:#06X},").replace("0X", "0x")).collect::<Vec<_>>().join(" ")))
            .collect();
        format!("pub static {name}: [u16; {}] = [\n{}\n];", table.len(), rows.join("\n"))
    };
    println!("//! Written by the `record_optional_reciprocal` example of wcontainers-behavior-source-kvm-single-step-x86-64, as");
    println!("//! the chip source's guest computes them on an Intel Xeon Platinum 8488C: VRCP14's significand of 16 bits for each");
    println!("//! value of the source's top 16 significand bits; VRSQRT14's for each parity of the source's exponent (even first)");
    println!("//! and each value of its top 15; VRCPPH's significand of 10 bits for each fraction; VRSQRTPH's for each parity and");
    println!("//! fraction; and VRCPPH's tiny result for each source of exponent 29 and 30 (the first entry unused).");
    println!();
    println!("{}", render("RECIPROCAL_FOURTEEN", &fourteen));
    println!();
    println!("{}", render("ROOT_RECIPROCAL_FOURTEEN", &root_fourteen));
    println!();
    println!("{}", render("RECIPROCAL_HALF", &half_table));
    println!();
    println!("{}", render("ROOT_RECIPROCAL_HALF", &half_root_table));
    println!();
    println!("{}", render("RECIPROCAL_HALF_TINY", &tiny));
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    println!("the x86-64 chip source needs a Linux x86-64 host with KVM");
}
