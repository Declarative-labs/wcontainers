//! The bytes of an EVEX form's memory reference, which AC's check reads.

use wcontainers_instruction_input_domain_x86_64::OperandValue;
use wcontainers_processor_architecture_x86_64::evex::EvexForm;

/// One element for a broadcast, every element's otherwise.
pub fn to_reference_bytes(form: &EvexForm, operands: &[Option<OperandValue>]) -> u64 {
    let broadcast = operands.iter().flatten().any(|value| *value == OperandValue::Broadcast(true));
    (if broadcast { form.memory_element_bytes } else { form.memory_bytes() }) as u64
}
