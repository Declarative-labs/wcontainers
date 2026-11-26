//! STATE COUNTS: for each class of modeled instructions (those with a behavior besides that of a chip without their
//! feature), the states of the full product of every operand's values (register numbers with the distinct ones the
//! generator adds) and the states the generator makes, counted without a chip; then one hash of every generated state
//! of every modeled instruction in the order a run takes them, and how many hold the exclusive monitor, which the hash
//! leaves out.
//!
//! `cargo run --release -p wcontainers-precise-instruction-set-measured --example state_counts`

use std::collections::BTreeMap;
use std::convert::Infallible;

use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_instruction_input_domain_arm64::{Arm64InputDomain, OperandValue};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, ExclusiveMonitor, Instruction, PreciseInstruction, User,
};

/// [FNV-1a](https://datatracker.ietf.org/doc/html/draft-eastlake-fnv-21#section-6.1.2), 64 bits: its offset basis and prime.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

struct Hash(u64);

impl Hash {
    fn add(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
        }
    }

    /// Adds every field of the state, so that two states hash alike only where they hold the same.
    fn add_state(&mut self, state: &ExecutionState<'_, Arm64<User>>) {
        let processor = &state.processor;
        for register in processor.general_purpose_registers {
            self.add(&register.to_le_bytes());
        }
        for value in [
            processor.stack_pointer,
            processor.program_counter,
            u64::from(processor.condition_flags.bits()),
            processor.floating_point_control,
            processor.floating_point_status,
            processor.software_thread_id,
        ] {
            self.add(&value.to_le_bytes());
        }
        for register in processor.vector_registers {
            self.add(&register.to_le_bytes());
        }
        // Only where one holds a value, so that a state that holds none hashes as before the state held them.
        if processor
            .predicate_registers
            .iter()
            .any(|predicate| *predicate != 0)
            || processor.first_fault_register != 0
        {
            for register in processor.predicate_registers {
                self.add(&register.to_le_bytes());
            }
            self.add(&processor.first_fault_register.to_le_bytes());
        }
        self.add(format!("{:?}", processor.exception).as_bytes());
        for mutation in &state.memory_mutations {
            match mutation {
                MemoryDelta::ValueAssignment { start, bytes } => {
                    self.add(b"value");
                    self.add(&start.to_le_bytes());
                    self.add(&(bytes.len() as u64).to_le_bytes());
                    self.add(bytes);
                }
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    self.add(b"permission");
                    self.add(&addresses.start.to_le_bytes());
                    self.add(&addresses.end.to_le_bytes());
                    self.add(format!("{permission:?}").as_bytes());
                }
            }
        }
    }
}

fn main() {
    let domain = Arm64InputDomain;
    let generator = CriticalValueCut::default();
    let modeled: Vec<Instruction<Infallible>> = Arm64::<User>::list_instructions()
        .filter(|instruction| {
            Arm64::<User>::list_known_behaviors(*instruction)
                .any(|precise| !matches!(precise, PreciseInstruction::Unimplemented(_)))
        })
        .collect();
    let mut by_class: BTreeMap<String, (u128, u64, usize)> = BTreeMap::new();
    let mut hash = Hash(FNV_OFFSET);
    let mut states = 0_u64;
    let mut monitor_held = 0_u64;
    for &instruction in &modeled {
        let product: u128 = domain
            .list_operand_types(instruction)
            .map(|operand_type| {
                let mut values: Vec<OperandValue> =
                    domain.generate_critical_values(operand_type).collect();
                for number in domain.generate_distinct_register_numbers(operand_type) {
                    if !values.contains(&number) {
                        values.push(number);
                    }
                }
                values.len() as u128
            })
            .product();
        hash.add(format!("{instruction:?}").as_bytes());
        let mut cut = 0_u64;
        for (_, state) in generator.generate_states_with_operands(instruction) {
            hash.add_state(&state);
            monitor_held += u64::from(state.processor.exclusive_monitor != ExclusiveMonitor::Open);
            cut += 1;
        }
        states += cut;
        let class = instruction
            .to_record()
            .map_or("None".to_owned(), |record| format!("{:?}", record.class));
        println!("{instruction:?}\t{class}\t{product}\t{cut}");
        let entry = by_class.entry(class).or_default();
        entry.0 += product;
        entry.1 += cut;
        entry.2 += 1;
    }
    for (class, (product, cut, count)) in by_class {
        println!("class {class}: {count} modeled, product {product}, cut {cut}");
    }
    println!(
        "modeled {} instructions, {states} states, hash {:016x}",
        modeled.len(),
        hash.0
    );
    println!("states that hold the exclusive monitor: {monitor_held}");
}
