//! SPIR-V words as instructions, and the validation of a module with what the common form allows it.

use crate::ConvertError;
use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};

/// Words before a SPIR-V module's first instruction: magic, version, generator, bound, schema.
pub const HEADER_WORDS: usize = 5;
/// An instruction's first word holds its word count in the high half and its opcode in the low half.
pub const WORD_COUNT_SHIFT: u32 = u16::BITS;
/// Between the errors of a chain.
const CHAIN_SEPARATOR: &str = ": ";

/// A module's instructions after its header, each as its opcode and all its words, ending at the
/// first one whose length does not fit.
pub fn instructions(words: &[u32]) -> impl Iterator<Item = (u32, &[u32])> {
    let mut rest = words.get(HEADER_WORDS..).unwrap_or_default();
    std::iter::from_fn(move || {
        let first = *rest.first()?;
        let count = (first >> WORD_COUNT_SHIFT) as usize;
        let inst = rest.get(..count).filter(|_| count > 0)?;
        rest = &rest[count..];
        Some((u32::from(first as u16), inst))
    })
}

/// What the common form allows the module: the shaders crate's capabilities, and immediates where the
/// module reads them (the back ends size them from its `Immediate` globals).
fn capabilities(module: &naga::Module) -> Capabilities {
    let immediates = module
        .global_variables
        .iter()
        .any(|(_, g)| g.space == naga::AddressSpace::Immediate);
    let mut capabilities = wcontainers_shaders::capabilities(module);
    capabilities.set(Capabilities::IMMEDIATES, immediates);
    capabilities
}

/// Validates a module with what the common form allows it.
pub fn validate(module: &naga::Module) -> Result<ModuleInfo, ConvertError> {
    Validator::new(ValidationFlags::all(), capabilities(module))
        .validate(module)
        .map_err(|e| ConvertError::Invalid(chain(e.as_inner())))
}

/// An error with every error beneath it, outermost first.
pub fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut next = error.source();
    while let Some(e) = next {
        text.push_str(CHAIN_SEPARATOR);
        text.push_str(&e.to_string());
        next = e.source();
    }
    text
}
