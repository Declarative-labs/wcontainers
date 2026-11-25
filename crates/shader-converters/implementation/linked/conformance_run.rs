use wcontainers_shader_converters::StandaloneShaderConverters;
use wcontainers_shader_converters_linked::LinkedShaderConverters;

wcontainers_shader_converters_conformance::tests!(LinkedShaderConverters::new(()));
