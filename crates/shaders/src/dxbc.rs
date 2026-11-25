//! The call into dxbc-spirv (C++, built by `build.rs`).

use crate::ConvertError;
use crate::converters::{self, DXBC_TO_IR, Failure};
use crate::ir::{Ir, code};

/// DXBC to dxbc-spirv's legalised IR.
pub fn to_ir(dxbc: &[u8]) -> Result<Ir, ConvertError> {
    let [ir, declared] = converters::call(&DXBC_TO_IR, dxbc, converters::NO_SAMPLES).map_err(
        |(Failure::Refused(reason) | Failure::Ended(reason))| ConvertError::Rejected(reason),
    )?;
    let declared = converters::words(&declared);
    let ir = Ir::decode(&ir);
    let mut ir = ir.ok_or_else(|| ConvertError::Malformed("the IR did not decode".into()))?;
    declared_sizes(&mut ir, &declared);
    ir.spaces = crate::container::shader_model(dxbc) >= Some(DIRECT3D_12_NUMBERING);
    Ok(ir)
}

/// The first shader model whose resources Direct3D 12 numbers by register space.
const DIRECT3D_12_NUMBERING: (u32, u32) = (5, 1);

/// The largest constant buffer dxbc-spirv declares for one it cannot size.
const MAX_CONSTANTS: u32 = 4096;

/// dxbc-spirv declares every dynamically indexed constant buffer with 4096 vectors; WebGPU then requires
/// a 64 KiB binding. The bytecode's own declared size, which dxbc-spirv's parser reads as (space,
/// register, size) triples in `declared`, is what a program binds at least, so it is used.
fn declared_sizes(ir: &mut Ir, declared: &[u32]) {
    for op in ir.ops.iter_mut().filter(|op| op.code == code::DCL_CBV) {
        let at = (op.lits.first().copied(), op.lits.get(1).copied());
        let size = declared
            .as_chunks::<{ crate::dxil::TRIPLE }>()
            .0
            .iter()
            .find(|d| (Some(u64::from(d[0])), Some(u64::from(d[1]))) == at);
        if let (Some(&[_, _, size]), [dim]) = (size, &mut op.ty.dims[..])
            && *dim == MAX_CONSTANTS
            && size > 0
        {
            *dim = size.min(MAX_CONSTANTS);
        }
    }
}
