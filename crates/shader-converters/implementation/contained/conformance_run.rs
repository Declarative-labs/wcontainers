use wcontainers_shader_converters::StandaloneShaderConverters;
use wcontainers_shader_converters_contained::ContainedShaderConverters;

wcontainers_shader_converters_conformance::tests!(ContainedShaderConverters::new(()));
