//! DXBC handed to the device: its stage from the container, and its conversion through the shaders
//! crate into the common form.

use wcontainers_gpu::{SourceStage, VariantKey};
use wcontainers_shaders::{ConvertError, Converted, Source};

use crate::error::{Error, Result, not_implemented};

/// The stage a DXBC container's program is written for, read from the version token of its `SHDR` or
/// `SHEX` chunk; an error if the bytes are not such a container.
pub fn stage(bytecode: &[u8]) -> Result<SourceStage> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    let word = |at: usize| -> Result<u32> {
        bytecode
            .get(at..)
            .and_then(|b| b.get(..4))
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or(bad)
    };
    if bytecode.len() < 32 || &bytecode[..4] != b"DXBC" || word(24)? as usize > bytecode.len() {
        return Err(bad);
    }
    for i in 0..word(28)? as usize {
        let at = word(32 + 4 * i)? as usize;
        let tag = bytecode.get(at..).and_then(|b| b.get(..4)).ok_or(bad)?;
        if tag == b"SHDR" || tag == b"SHEX" {
            return match word(at + 8)? >> 16 {
                0 => Ok(SourceStage::Pixel),
                1 => Ok(SourceStage::Vertex),
                2 => Ok(SourceStage::Geometry),
                3 => Ok(SourceStage::Hull),
                4 => Ok(SourceStage::Domain),
                5 => Ok(SourceStage::Compute),
                _ => Err(bad),
            };
        }
    }
    Err(bad)
}

/// Converts a shader of `expected` stage. Bytecode the converter refuses is invalid to Direct3D too;
/// what it cannot convert yet is not implemented.
pub fn convert(bytecode: &[u8], expected: SourceStage) -> Result<Converted> {
    if stage(bytecode)? != expected {
        return Err(Error::InvalidArg(
            "the bytecode is a shader of another stage",
        ));
    }
    match wcontainers_shaders::convert(Source::Dxbc(bytecode), &VariantKey::default(), None) {
        Ok(c) if c.modules.len() != 1 => {
            not_implemented("shaders converted into several program stages")
        }
        Ok(c) if !c.reflection.helpers.is_empty() => {
            not_implemented("shaders that read the helper constants")
        }
        Ok(c) => Ok(c),
        Err(ConvertError::Rejected(why)) => {
            eprintln!("wcontainers direct3d 11: shader refused by the converter: {why}");
            Err(Error::InvalidArg("bytecode the converter refuses"))
        }
        Err(ConvertError::Unsupported(what)) => not_implemented(what),
        Err(e) => {
            eprintln!("wcontainers direct3d 11: shader conversion failed: {e}");
            not_implemented("shaders the converter does not yet turn into a valid module")
        }
    }
}
