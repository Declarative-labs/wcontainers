//! TODO: say in one sentence how this implementation of wcontainers-shader-converters works.

use wcontainers_shader_converters::ShaderConverters;

pub struct ContainedShaderConverters;

impl ShaderConverters for ContainedShaderConverters {
    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").to_owned()
    }
}
