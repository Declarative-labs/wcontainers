//! Programs written in WGSL, read into the common form.

use wcontainers_gpu::{Backend, GpuError, Program, ShaderModule, ShaderStage};

/// A render program from one WGSL module with a vertex and a fragment entry point. Each shader's
/// content key is BLAKE3 of the source and its entry point, as for the interface's built-ins.
pub fn program_from_wgsl<B: Backend>(
    backend: &mut B,
    source: &str,
    vertex: &str,
    fragment: &str,
) -> Result<B::Program, GpuError> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|_| GpuError::Invalid("a WGSL program does not parse"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|_| GpuError::Invalid("a WGSL program does not validate"))?;
    let mut shader = |entry: &str, stage| {
        let key = *blake3::Hasher::new()
            .update(source.as_bytes())
            .update(entry.as_bytes())
            .finalize()
            .as_bytes();
        backend.create_shader(&ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: entry.into(),
            stage,
            key,
        })
    };
    let vs = shader(vertex, ShaderStage::Vertex)?;
    let fs = shader(fragment, ShaderStage::Fragment)?;
    let program = backend.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&fs),
    });
    backend.destroy(wcontainers_gpu::Object::Shader(vs));
    backend.destroy(wcontainers_gpu::Object::Shader(fs));
    program
}
