//! Shaders written as GLSL ES 3.00 by naga, and programs linked once per pair of them.

use glow::HasContext;
use naga::back::glsl;
use std::rc::Rc;
use wcontainers_gpu::{GpuError, ShaderModule, ShaderStage};

/// A (set, binding) pair: where a program reads a resource.
pub type Slot = (u32, u32);

/// A compiled shader and what its program must bind.
#[derive(Debug)]
pub struct ShaderObj {
    pub id: u64,
    pub stage: ShaderStage,
    pub raw: glow::Shader,
    /// Combined samplers by GLSL name: the texture's slot and its sampler's.
    pub textures: Vec<(String, Slot, Option<Slot>)>,
    /// Uniform blocks by GLSL name.
    pub blocks: Vec<(String, Slot)>,
    /// The GLSL, for the log of a program that fails to build.
    pub source: String,
}

/// One linked program and how its resources reach GL.
#[derive(Debug)]
pub struct ProgramObj {
    pub id: u64,
    pub raw: glow::Program,
    /// Texture unit n samples the texture in slot `.0` through the sampler in slot `.1`.
    pub units: Vec<(Slot, Option<Slot>)>,
    /// Uniform block binding point n reads the uniform buffer in this slot.
    pub blocks: Vec<Slot>,
    /// naga's `naga_vs_first_instance`, added to `gl_InstanceID`, and the value it holds.
    pub first_instance: Option<glow::UniformLocation>,
    pub first_instance_value: std::cell::Cell<u32>,
    /// The (vertex, fragment) pair it was linked from, and how many handles name it.
    pub pair: (u64, u64),
    pub handles: std::cell::Cell<u32>,
}

/// Writes `module` as GLSL ES 3.00. Without EXT_clip_control the last vertex stage negates y as its
/// last step, so row 0 is the top of the picture on both origin routes.
pub fn write_glsl(
    module: &ShaderModule,
    flip_y: bool,
) -> Result<(String, glsl::ReflectionInfo), String> {
    let stage = match module.stage {
        ShaderStage::Vertex => naga::ShaderStage::Vertex,
        ShaderStage::Fragment => naga::ShaderStage::Fragment,
        ShaderStage::Compute => return Err("compute shaders are absent on WebGL 2".into()),
    };
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        // GLES leaves the size of points a vertex program does not write undefined.
        writer_flags: glsl::WriterFlags::FORCE_POINT_SIZE,
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: false,
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: module.entry_point.clone(),
        multiview: None,
    };
    let mut out = String::new();
    let mut writer = glsl::Writer::new(
        &mut out,
        &module.module,
        &module.info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .map_err(|e| e.to_string())?;
    let reflection = writer.write().map_err(|e| e.to_string())?;
    if flip_y && stage == naga::ShaderStage::Vertex {
        // naga writes each `gl_Position = ...;` on a line of its own, just before the entry returns.
        let mut flipped = String::with_capacity(out.len() + 64);
        for line in out.lines() {
            flipped.push_str(line);
            flipped.push('\n');
            let trimmed = line.trim_start();
            if trimmed.starts_with("gl_Position = ") {
                flipped.push_str(&line[..line.len() - trimmed.len()]);
                flipped.push_str("gl_Position.y = -gl_Position.y;\n");
            }
        }
        out = flipped;
    }
    Ok((out, reflection))
}

/// The slot a global variable of `module` is bound at.
fn slot(module: &naga::Module, var: naga::Handle<naga::GlobalVariable>) -> Option<Slot> {
    module.global_variables[var]
        .binding
        .as_ref()
        .map(|b| (b.group, b.binding))
}

/// Compiles a shader. The compile status is read when its program links, so compiles can overlap.
pub fn compile(
    gl: &glow::Context,
    id: u64,
    module: &ShaderModule,
    flip_y: bool,
) -> Result<ShaderObj, GpuError> {
    let (source, reflection) = write_glsl(module, flip_y)
        .map_err(|_| GpuError::Invalid("webgl: naga could not write the shader as GLSL ES 3.00"))?;
    let m = &module.module;
    let mut textures: Vec<_> = reflection
        .texture_mapping
        .iter()
        .filter_map(|(name, map)| {
            Some((
                name.clone(),
                slot(m, map.texture)?,
                map.sampler.and_then(|s| slot(m, s)),
            ))
        })
        .collect();
    textures.sort();
    let mut blocks: Vec<_> = reflection
        .uniforms
        .iter()
        .filter_map(|(var, name)| Some((name.clone(), slot(m, *var)?)))
        .collect();
    blocks.sort();
    let kind = match module.stage {
        ShaderStage::Vertex => glow::VERTEX_SHADER,
        _ => glow::FRAGMENT_SHADER,
    };
    // SAFETY: plain GL calls on the current context.
    let raw = unsafe {
        let raw = gl.create_shader(kind).map_err(|_| GpuError::OutOfMemory)?;
        gl.shader_source(raw, &source);
        gl.compile_shader(raw);
        raw
    };
    Ok(ShaderObj {
        id,
        stage: module.stage,
        raw,
        textures,
        blocks,
        source,
    })
}

/// Links a vertex and an optional fragment shader, then points each combined sampler at a unit and each
/// uniform block at a binding point. A fragment-less program gets an empty fragment shader, since
/// GLES links none without one. On failure the error carries GL's logs.
pub fn link(
    gl: &glow::Context,
    id: u64,
    vertex: &Rc<ShaderObj>,
    fragment: Option<&Rc<ShaderObj>>,
    empty_fragment: glow::Shader,
) -> Result<ProgramObj, String> {
    // SAFETY: plain GL calls on the current context; the shaders are alive.
    unsafe {
        let raw = gl.create_program()?;
        gl.attach_shader(raw, vertex.raw);
        gl.attach_shader(raw, fragment.map_or(empty_fragment, |f| f.raw));
        gl.link_program(raw);
        if !gl.get_program_link_status(raw) {
            let mut log = gl.get_program_info_log(raw);
            for s in core::iter::once(vertex).chain(fragment) {
                if !gl.get_shader_compile_status(s.raw) {
                    log.push_str(&gl.get_shader_info_log(s.raw));
                    log.push_str(&s.source);
                }
            }
            gl.delete_program(raw);
            return Err(log);
        }
        gl.use_program(Some(raw));
        let mut units = Vec::new();
        let mut blocks: Vec<Slot> = Vec::new();
        for s in core::iter::once(vertex).chain(fragment) {
            for (name, texture, sampler) in &s.textures {
                if let Some(location) = gl.get_uniform_location(raw, name) {
                    gl.uniform_1_i32(Some(&location), units.len() as i32);
                    units.push((*texture, *sampler));
                }
            }
            for (name, slot) in &s.blocks {
                if let Some(index) = gl.get_uniform_block_index(raw, name) {
                    let point = match blocks.iter().position(|b| b == slot) {
                        Some(p) => p,
                        None => {
                            blocks.push(*slot);
                            blocks.len() - 1
                        }
                    };
                    gl.uniform_block_binding(raw, index, point as u32);
                }
            }
        }
        let first_instance = gl.get_uniform_location(raw, glsl::FIRST_INSTANCE_BINDING);
        Ok(ProgramObj {
            id,
            raw,
            units,
            blocks,
            first_instance,
            first_instance_value: std::cell::Cell::new(0),
            pair: (vertex.id, fragment.map_or(0, |f| f.id)),
            handles: std::cell::Cell::new(1),
        })
    }
}
