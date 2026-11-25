//! TODO: say in one sentence how this implementation of wcontainers-shader-converters works.

use wcontainers_shader_converters::ShaderConverters;

pub struct LinkedShaderConverters;

impl ShaderConverters for LinkedShaderConverters {
    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").to_owned()
    }
}
