//! The entry points a SPIR-V module declares, read from its `OpEntryPoint` instructions.

/// `OpEntryPoint`'s operands before its name: the execution model and the function.
const NAME_OPERAND: usize = 3;

/// One entry point: its execution model and name.
pub struct EntryPoint {
    pub model: spirv::ExecutionModel,
    pub name: String,
}

/// The words of a SPIR-V file, or None when its length is not whole words.
pub fn words(bytes: &[u8]) -> Option<Vec<u32>> {
    let (chunks, rest) = bytes.as_chunks::<{ size_of::<u32>() }>();
    rest.is_empty()
        .then(|| chunks.iter().map(|&c| u32::from_le_bytes(c)).collect())
}

/// Every entry point of the module, in order; empty when the words are not SPIR-V.
pub fn entry_points(words: &[u32]) -> Vec<EntryPoint> {
    wcontainers_nir::spirv::instructions(words)
        .filter(|&(opcode, _)| opcode == spirv::Op::EntryPoint as u32)
        .filter_map(|(_, inst)| {
            let model = spirv::ExecutionModel::from_u32(*inst.get(1)?)?;
            let name = string(inst.get(NAME_OPERAND..)?);
            Some(EntryPoint { model, name })
        })
        .collect()
}

/// A SPIR-V literal string: bytes packed into words, little end first, ending at a zero byte.
fn string(words: &[u32]) -> String {
    let bytes: Vec<u8> = words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take_while(|&b| b != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
