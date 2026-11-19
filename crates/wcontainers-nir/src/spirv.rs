//! Candidate A's second step: SPIR-V written from NIR, read by naga into a validated module.

use crate::ConvertError;
use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};

/// Words before a SPIR-V module's first instruction: magic, version, generator, bound, schema.
pub const HEADER_WORDS: usize = 5;
/// An instruction's first word holds its word count in the high half and its opcode in the low half.
const WORD_COUNT_SHIFT: u32 = u16::BITS;
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

/// The built-ins that need the `DrawParameters` capability.
const DRAW_PARAMETERS: [spirv::BuiltIn; 3] = [
    spirv::BuiltIn::BaseVertex,
    spirv::BuiltIn::BaseInstance,
    spirv::BuiltIn::DrawIndex,
];

/// The module without what Zink writes that says nothing and that naga's reader refuses or misreads:
/// `Index 0` decorations (the first input of blending, which no decoration also says; read as
/// dual-source blending), and the `DrawParameters` capability where no built-in needs it (Zink
/// declares it for every vertex program reading its instance index).
fn cleaned(words: &[u32]) -> Vec<u32> {
    let decorates = |inst: &[u32], decoration: spirv::Decoration| matches!(inst, [_, _, d, ..] if *d == decoration as u32);
    let draw_parameters = instructions(words).any(|(opcode, inst)| {
        opcode == spirv::Op::Decorate as u32
            && decorates(inst, spirv::Decoration::BuiltIn)
            && DRAW_PARAMETERS
                .iter()
                .any(|&b| inst.last() == Some(&(b as u32)))
    });
    let mut out = words[..HEADER_WORDS.min(words.len())].to_vec();
    for (opcode, inst) in instructions(words) {
        let index_zero = opcode == spirv::Op::Decorate as u32
            && decorates(inst, spirv::Decoration::Index)
            && matches!(inst, [_, _, _, 0]);
        let unneeded = opcode == spirv::Op::Capability as u32
            && !draw_parameters
            && inst.get(1) == Some(&(spirv::Capability::DrawParameters as u32));
        if !index_zero && !unneeded {
            out.extend_from_slice(inst);
        }
    }
    out
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

/// Reads SPIR-V words into a naga module and validates it with what the common form allows it.
pub fn read(words: &[u32]) -> Result<(naga::Module, ModuleInfo), ConvertError> {
    let options = naga::front::spv::Options::default();
    let words = cleaned(words);
    let module = naga::front::spv::Frontend::new(words.iter().copied(), &options)
        .parse()
        .map_err(|e| ConvertError::Reader(chain(&e)))?;
    let info = Validator::new(ValidationFlags::all(), capabilities(&module))
        .validate(&module)
        .map_err(|e| ConvertError::Invalid(chain(e.as_inner())))?;
    Ok((module, info))
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
