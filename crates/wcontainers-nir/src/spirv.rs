//! Candidate A's second step: SPIR-V written from NIR, read by naga into a validated module.

use crate::ConvertError;
use naga::valid::{ModuleInfo, ValidationFlags, Validator};
use wcontainers_shaders::WEBGPU_CAPABILITIES;

/// Reads SPIR-V words into a naga module and validates it with WebGPU's core capabilities.
pub fn read(words: &[u32]) -> Result<(naga::Module, ModuleInfo), ConvertError> {
    let options = naga::front::spv::Options::default();
    let module = naga::front::spv::Frontend::new(words.iter().copied(), &options)
        .parse()
        .map_err(|e| ConvertError::Reader(e.to_string()))?;
    let info = Validator::new(ValidationFlags::all(), WEBGPU_CAPABILITIES)
        .validate(&module)
        .map_err(|e| ConvertError::Invalid(e.into_inner().to_string()))?;
    Ok((module, info))
}
