//! Prints the CPUID answers the chip source's guest reads, as the Rust of `cpuid_recording.rs`: for every leaf the
//! generated states ask that lies in its range, and for the highest basic leaf, at subleaves 0, 1 and 0xFFFFFFFF.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example record_cpuid`

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
    use wcontainers_instruction_input_domain::InstructionInputDomain;
    use wcontainers_instruction_input_domain_x86_64::{OperandValue, X86_64InputDomain};
    use wcontainers_processor_architecture_x86_64::Instruction;

    const SUBLEAVES: [u32; 3] = [0, 1, u32::MAX];
    let mut machine = match SingleStepMachine::new() {
        Ok(machine) => machine,
        Err(error) => panic!("no single step machine: {error:?}"),
    };
    let mut read = |leaf: u32, subleaf: u32| -> [u32; 4] {
        let operands = [
            OperandValue::Bits(u64::from(leaf)),
            OperandValue::Bits(u64::from(subleaf)),
            OperandValue::Bits(0),
        ];
        let before = X86_64InputDomain.build_state(Instruction::Cpuid, &operands);
        let after = match machine.execute_single_instruction(before) {
            Ok(after) => after,
            Err(refusal) => panic!("the chip refuses CPUID: {refusal:?}"),
        };
        assert!(after.processor.exception.is_none(), "CPUID completes");
        let registers = after.processor.general_purpose_registers;
        [registers[0], registers[3], registers[1], registers[2]].map(|value| value as u32)
    };
    let highest_basic = read(0, 0)[0];
    let highest_extended = read(0x8000_0000, 0)[0];
    let mut leaves = vec![0, 1, 7, 0xD, highest_basic, 0x4000_0000, 0x8000_0000, 0x8000_0001, 0x8000_0008];
    leaves.retain(|&leaf| leaf < 0x8000_0000 || leaf <= highest_extended);
    leaves.sort_unstable();
    leaves.dedup();
    let render = |[eax, ebx, ecx, edx]: [u32; 4]| format!("[{eax:#010x}, {ebx:#010x}, {ecx:#010x}, {edx:#010x}]");
    let mut lines = Vec::new();
    for leaf in leaves {
        let answers = SUBLEAVES.map(|subleaf| read(leaf, subleaf));
        if answers.iter().all(|answer| *answer == answers[0]) {
            lines.push(format!(
                "    CpuidLeaf {{ leaf: {leaf:#x}, subleaf: None, registers: {} }},",
                render(answers[0])
            ));
        } else {
            for (subleaf, answer) in SUBLEAVES.iter().zip(answers) {
                lines.push(format!(
                    "    CpuidLeaf {{ leaf: {leaf:#x}, subleaf: Some({subleaf:#x}), registers: {} }},",
                    render(answer)
                ));
            }
        }
    }
    println!("//! Written by the `record_cpuid` example of wcontainers-behavior-source-kvm-single-step-x86-64.");
    println!();
    println!("use crate::CpuidLeaf;");
    println!();
    println!("pub static LEAVES: [CpuidLeaf; {}] = [", lines.len());
    for line in lines {
        println!("{line}");
    }
    println!("];");
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    println!("the x86-64 chip source needs a Linux x86-64 host with KVM");
}
