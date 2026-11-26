//! Methods of the types [`crate::shader`] declares.

use crate::shader::*;
use alloc::string::String;

impl ShaderModule {
    /// The entry point `entry` of a WGSL module, for programs written in WGSL (built-ins, the present
    /// copy, tests). The content key is BLAKE3 of the source and the entry point.
    pub fn from_wgsl(
        source: &str,
        entry: &str,
        stage: ShaderStage,
    ) -> Result<Self, crate::error::GpuError> {
        let module = naga::front::wgsl::parse_str(source)
            .map_err(|_| crate::error::GpuError::Invalid("a WGSL module does not parse"))?;
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|_| crate::error::GpuError::Invalid("a WGSL module does not validate"))?;
        let key = *blake3::Hasher::new()
            .update(source.as_bytes())
            .update(entry.as_bytes())
            .finalize()
            .as_bytes();
        Ok(ShaderModule {
            module,
            info,
            entry_point: String::from(entry),
            stage,
            key,
        })
    }
}
