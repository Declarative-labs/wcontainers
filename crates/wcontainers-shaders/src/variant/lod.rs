//! Direct3D's level-of-detail query (`CalculateLevelOfDetail`, `lod`), which WGSL and GLSL ES lack: a
//! library function written in WGSL (`lod.wgsl`) computes it from the coordinate's derivatives and the
//! sampler's state in the helper block.

use alloc::format;
use naga::{Function, Handle, Module, Span};

use super::Result;
use crate::ConvertError;

const LIBRARY: &str = include_str!("lod.wgsl");
const FUNCTION: &str = "level_of_detail";

/// The library's `level_of_detail` in `module`, imported on first use (it calls no other function, so
/// it may go before any caller made after it).
pub(crate) fn function(module: &mut Module) -> Result<Handle<Function>> {
    if let Some((h, _)) = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(FUNCTION))
    {
        return Ok(h);
    }
    let parsed = naga::front::wgsl::parse_str(LIBRARY).map_err(|e| {
        ConvertError::Malformed(format!("the level-of-detail library does not parse: {e}"))
    })?;
    let f = parsed
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(FUNCTION))
        .map(|(h, _)| h)
        .ok_or_else(|| ConvertError::Malformed(format!("the library has no {FUNCTION}")))?;
    let mut library = super::filter::Library::default();
    library.import(module, &parsed, f)?;
    let mut made = None;
    for g in library.ahead.into_inner() {
        made = Some(module.functions.append(g, Span::UNDEFINED));
    }
    made.ok_or_else(|| ConvertError::Malformed(format!("{FUNCTION} was not imported")))
}
