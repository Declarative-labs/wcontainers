//! The tests every implementation of wcontainers-instruction-input-domain runs, written against its traits alone.
//!
//! `Eq` on the architecture is what the derives of `ExecutionState` require of it to compare two states.

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// Every operand type an instruction lists has at least one critical value, or the instruction would get no state.
pub fn lists_a_critical_value_for_every_operand_type<T, Architecture>(value: (T, Architecture))
where
    T: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture + Eq,
{
    let (domain, _) = value;
    for instruction in Architecture::list_instructions() {
        for operand_type in domain.list_operand_types(instruction) {
            assert!(
                domain
                    .generate_critical_values(operand_type)
                    .next()
                    .is_some(),
                "an operand type has no critical value"
            );
        }
    }
}

/// An instruction gets one critical state for each combination of the critical values of its operands.
pub fn generates_one_critical_state_per_combination<T, Architecture>(value: (T, Architecture))
where
    T: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture + Eq,
{
    let (domain, _) = value;
    for instruction in Architecture::list_instructions() {
        let combinations: usize = domain
            .list_operand_types(instruction)
            .map(|operand_type| domain.generate_critical_values(operand_type).count())
            .product();
        assert_eq!(
            domain.generate_critical_states(instruction).count(),
            combinations
        );
    }
}

/// The same operands build the same state, and the first critical value of each operand builds a critical state.
pub fn builds_a_critical_state_from_critical_values<T, Architecture>(value: (T, Architecture))
where
    T: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture + Eq,
{
    let (domain, _) = value;
    for instruction in Architecture::list_instructions() {
        let first_values: Option<Vec<T::OperandValue>> = domain
            .list_operand_types(instruction)
            .map(|operand_type| domain.generate_critical_values(operand_type).next())
            .collect();
        let Some(operands) = first_values else {
            panic!("an operand type has no critical value");
        };
        let state = domain.build_state(instruction, &operands);
        assert!(
            domain.build_state(instruction, &operands) == state,
            "the same operands built two different states"
        );
        assert!(
            domain
                .generate_critical_states(instruction)
                .any(|critical| critical == state),
            "the state of the first critical values is not a critical state"
        );
    }
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn lists_a_critical_value_for_every_operand_type() {
            $crate::lists_a_critical_value_for_every_operand_type($value);
        }

        #[test]
        fn generates_one_critical_state_per_combination() {
            $crate::generates_one_critical_state_per_combination($value);
        }

        #[test]
        fn builds_a_critical_state_from_critical_values() {
            $crate::builds_a_critical_state_from_critical_values($value);
        }
    };
}
