//! The input domain suite, with its product check made on the states the cut generator gives, since the product of
//! every listed instruction is about 114 billion states, which no test run walks.

use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_instruction_input_domain_arm64::{Arm64InputDomain, OperandValue};
use wcontainers_instruction_input_domain_conformance as suite;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, User};

/// The largest product of one instruction whose states the product check walks.
const WALKED_PRODUCT: u128 = 1 << 16;

#[test]
fn lists_a_critical_value_for_every_operand_type() {
    suite::lists_a_critical_value_for_every_operand_type((
        Arm64InputDomain,
        Arm64::<User>::default(),
    ));
}

#[test]
fn builds_a_critical_state_from_critical_values() {
    suite::builds_a_critical_state_from_critical_values((
        Arm64InputDomain,
        Arm64::<User>::default(),
    ));
}

#[test]
#[ignore = "walks about 114 billion states"]
fn generates_one_critical_state_per_combination() {
    suite::generates_one_critical_state_per_combination((
        Arm64InputDomain,
        Arm64::<User>::default(),
    ));
}

/// Checks that each state the cut gives is the state built from one combination of the critical values of the
/// instruction's operands, a register number also one of the distinct numbers the cut lets each register operand name,
/// that it gives no more states than the product of those values, and that the product of the critical values gives
/// one state for each combination where it is at most [`WALKED_PRODUCT`] states.
#[test]
fn generates_one_critical_state_per_combination_where_the_cut_or_a_small_product_walks() {
    let domain = Arm64InputDomain;
    let cut = CriticalValueCut::new(domain);
    for instruction in Arm64::<User>::list_instructions() {
        let critical: Vec<Vec<OperandValue>> = domain
            .list_operand_types(instruction)
            .map(|operand_type| domain.generate_critical_values(operand_type).collect())
            .collect();
        let values: Vec<Vec<OperandValue>> = domain
            .list_operand_types(instruction)
            .zip(&critical)
            .map(|(operand_type, critical)| {
                let distinct = domain
                    .generate_distinct_register_numbers(operand_type)
                    .filter(|number| !critical.contains(number));
                critical.iter().copied().chain(distinct).collect()
            })
            .collect();
        let mut found = vec![0; values.len()];
        let mut states = 0_u128;
        for (operands, state) in cut.generate_states_with_operands(instruction) {
            states += 1;
            assert_eq!(operands.len(), values.len(), "{instruction:?}");
            for ((operand, values), found) in operands.iter().zip(&values).zip(&mut found) {
                *found = find_from(values, *found, operand)
                    .unwrap_or_else(|| panic!("{instruction:?}: {operand:?} is no critical value"));
            }
            assert!(
                domain.build_state(instruction, &operands) == state,
                "{instruction:?}: the cut's state is not the one its operands build"
            );
        }
        assert!(
            to_product(&values).is_none_or(|product| states <= product),
            "{instruction:?}"
        );
        if let Some(product) = to_product(&critical).filter(|product| *product <= WALKED_PRODUCT) {
            let generated = domain.generate_critical_states(instruction).count() as u128;
            assert_eq!(generated, product, "{instruction:?}");
        }
    }
}

/// Returns how many combinations the values of the operands make, or none where it overflows.
fn to_product(values: &[Vec<OperandValue>]) -> Option<u128> {
    values.iter().try_fold(1_u128, |product, values| {
        product.checked_mul(values.len() as u128)
    })
}

/// Returns where the value lies in the values, looking from the place given first, since the cut's consecutive states
/// mostly share their operands.
fn find_from(values: &[OperandValue], from: usize, value: &OperandValue) -> Option<usize> {
    (from..values.len())
        .chain(0..from)
        .find(|&place| values[place] == *value)
}
