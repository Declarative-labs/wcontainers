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
    /// The GLSL, for the log of a program that fails to build and for capture programs.
    pub source: String,
    /// A vertex shader's location outputs in location order: GLSL name and bytes.
    pub outputs: Vec<(String, u32)>,
}

/// A vertex shader and an optional fragment shader.
pub type Pair = (Rc<ShaderObj>, Option<Rc<ShaderObj>>);

/// A program and how its resources reach GL. Linking runs from creation; what the link decides is
/// read at the first draw, so links overlap other work.
#[derive(Debug)]
pub struct ProgramObj {
    pub id: u64,
    pub raw: glow::Program,
    /// The shaders it links, until the link is read.
    pub shaders: std::cell::RefCell<Option<Pair>>,
    /// The shaders it was made from, whose sources capture programs compile again.
    pub stages: Pair,
    /// Its capture programs, made at the first capture of each kind (see [`Capture::slot`]).
    pub captures: [std::cell::OnceCell<Rc<ProgramObj>>; 4],
    pub linked: std::cell::OnceCell<Linked>,
    /// The (vertex, fragment) pair it was linked from, and how many handles name it.
    pub pair: (u64, u64),
    pub handles: std::cell::Cell<u32>,
}

/// What a finished link decided.
#[derive(Debug)]
pub struct Linked {
    /// Texture unit n samples the texture in slot `.0` through the sampler in slot `.1`.
    pub units: Vec<(Slot, Option<Slot>)>,
    /// Unit n's hidden size uniform (see [`sizes_from_uniforms`]), with the value it holds.
    pub sizes: Vec<Option<(glow::UniformLocation, std::cell::Cell<[i32; 3]>)>>,
    /// Uniform block binding point n reads the uniform buffer in this slot.
    pub blocks: Vec<Slot>,
    /// naga's `naga_vs_first_instance` (added to `gl_InstanceID`) and `naga_vs_base_vertex` (added to
    /// `gl_VertexID`), each with the value it holds.
    pub first_instance: Option<(glow::UniformLocation, std::cell::Cell<u32>)>,
    pub base_vertex: Option<(glow::UniformLocation, std::cell::Cell<u32>)>,
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
    let names: Vec<&str> = reflection
        .texture_mapping
        .keys()
        .map(String::as_str)
        .collect();
    let source = sizes_from_uniforms(source, &names);
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
    let outputs = match module.stage {
        ShaderStage::Vertex => outputs(module),
        _ => Vec::new(),
    };
    let raw = start_compile(gl, module.stage, &source)?;
    Ok(ShaderObj {
        id,
        stage: module.stage,
        raw,
        textures,
        blocks,
        source,
        outputs,
    })
}

/// Turns each `textureSize(t, l)` into a shift of the hidden uniform `t_size`, the size of the bound
/// view's first level, which the back end sets with the view. ANGLE on Direct3D 11 answers
/// `textureSize` for the wrong level once a texture's base level has changed.
fn sizes_from_uniforms(source: String, names: &[&str]) -> String {
    if !source.contains("textureSize(") {
        return source;
    }
    const HELPERS: &str = "ivec2 _wc_size_2d(ivec3 s, int l) { return max(s.xy >> l, ivec2(1)); }\n\
        ivec3 _wc_size_array(ivec3 s, int l) { return ivec3(max(s.xy >> l, ivec2(1)), s.z); }\n\
        ivec3 _wc_size_3d(ivec3 s, int l) { return max(s >> l, ivec3(1)); }\n";
    let mut out = String::with_capacity(source.len() + 512);
    let mut helpers = false;
    for line in source.lines() {
        out.push_str(line);
        out.push('\n');
        let declared = names.iter().find(|n| {
            line.starts_with("uniform ")
                && line.contains("sampler")
                && line
                    .strip_suffix(';')
                    .is_some_and(|l| l.ends_with(&format!(" {n}")))
        });
        if let Some(name) = declared {
            out.push_str(&format!("uniform highp ivec3 {name}_size;\n"));
            if !helpers {
                out.push_str(HELPERS);
                helpers = true;
            }
        }
    }
    for name in names {
        let helper = match source.lines().find(|l| {
            l.starts_with("uniform ")
                && l.strip_suffix(';')
                    .is_some_and(|l| l.ends_with(&format!(" {name}")))
        }) {
            Some(l) if l.contains("sampler3D") => "_wc_size_3d",
            Some(l) if l.contains("Array") => "_wc_size_array",
            _ => "_wc_size_2d",
        };
        out = out.replace(
            &format!("textureSize({name}, "),
            &format!("{helper}({name}_size, "),
        );
    }
    out
}

/// Starts compiling GLSL of `stage`.
fn start_compile(
    gl: &glow::Context,
    stage: ShaderStage,
    source: &str,
) -> Result<glow::Shader, GpuError> {
    let kind = match stage {
        ShaderStage::Vertex => glow::VERTEX_SHADER,
        _ => glow::FRAGMENT_SHADER,
    };
    // SAFETY: plain GL calls on the current context.
    unsafe {
        let raw = gl.create_shader(kind).map_err(|_| GpuError::OutOfMemory)?;
        gl.shader_source(raw, source);
        gl.compile_shader(raw);
        Ok(raw)
    }
}

/// The entry point's location outputs in location order, named as naga's GLSL writer names vertex
/// outputs, with their size in bytes (0 for types transform feedback cannot write).
fn outputs(module: &ShaderModule) -> Vec<(String, u32)> {
    let m = &module.module;
    let Some(entry) = m.entry_points.iter().find(|e| e.name == module.entry_point) else {
        return Vec::new();
    };
    let Some(result) = &entry.function.result else {
        return Vec::new();
    };
    let size = |ty: naga::Handle<naga::Type>| match m.types[ty].inner {
        naga::TypeInner::Scalar(s) if s.width == 4 => 4,
        naga::TypeInner::Vector { size, scalar } if scalar.width == 4 => 4 * size as u32,
        _ => 0,
    };
    let mut found: Vec<(u32, u32)> = match (&result.binding, &m.types[result.ty].inner) {
        (Some(naga::Binding::Location { location, .. }), _) => vec![(*location, size(result.ty))],
        (None, naga::TypeInner::Struct { members, .. }) => members
            .iter()
            .filter_map(|member| match member.binding {
                Some(naga::Binding::Location { location, .. }) => Some((location, size(member.ty))),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    found.sort();
    found
        .into_iter()
        .map(|(location, bytes)| (format!("_vs2fs_location{location}"), bytes))
        .collect()
}

/// What a capture program writes: interleaved into one target or separate, and whether it draws.
#[derive(Clone, Copy, Debug)]
pub struct Capture {
    pub separate: bool,
    pub rasterize: bool,
}

impl Capture {
    /// Its place in [`ProgramObj::captures`].
    pub fn slot(self) -> usize {
        usize::from(self.separate) * 2 + usize::from(self.rasterize)
    }
}

/// Starts linking `program`'s shaders again from their sources with its location outputs as
/// transform feedback varyings. One that does not draw links the empty fragment shader, so no
/// fragment output meets the framebuffer.
pub fn link_capture(
    gl: &glow::Context,
    id: u64,
    program: &ProgramObj,
    capture: Capture,
    empty_fragment: glow::Shader,
) -> Result<ProgramObj, GpuError> {
    let (vertex, fragment) = &program.stages;
    let fresh = |s: &ShaderObj| -> Result<Rc<ShaderObj>, GpuError> {
        Ok(Rc::new(ShaderObj {
            raw: start_compile(gl, s.stage, &s.source)?,
            textures: s.textures.clone(),
            blocks: s.blocks.clone(),
            source: s.source.clone(),
            outputs: s.outputs.clone(),
            ..*s
        }))
    };
    let v = fresh(vertex)?;
    let f = match fragment {
        Some(f) if capture.rasterize => Some(fresh(f)?),
        _ => None,
    };
    let names: Vec<&str> = v.outputs.iter().map(|(n, _)| n.as_str()).collect();
    let mode = if capture.separate {
        glow::SEPARATE_ATTRIBS
    } else {
        glow::INTERLEAVED_ATTRIBS
    };
    // SAFETY: plain GL calls on the current context; the shaders are alive.
    let raw = unsafe {
        let raw = gl.create_program().map_err(|_| GpuError::OutOfMemory)?;
        gl.attach_shader(raw, v.raw);
        gl.attach_shader(raw, f.as_ref().map_or(empty_fragment, |f| f.raw));
        gl.transform_feedback_varyings(raw, &names, mode);
        gl.link_program(raw);
        // Flagged only: GL deletes them with the program.
        gl.delete_shader(v.raw);
        if let Some(f) = &f {
            gl.delete_shader(f.raw);
        }
        raw
    };
    Ok(started(id, raw, (v, f), program.pair))
}

/// A program whose link has started.
fn started(id: u64, raw: glow::Program, stages: Pair, pair: (u64, u64)) -> ProgramObj {
    ProgramObj {
        id,
        raw,
        shaders: std::cell::RefCell::new(Some(stages.clone())),
        stages,
        captures: Default::default(),
        linked: std::cell::OnceCell::new(),
        pair,
        handles: std::cell::Cell::new(1),
    }
}

/// Starts linking a vertex and an optional fragment shader. A fragment-less program gets an empty
/// fragment shader, since GLES links none without one.
pub fn link(
    gl: &glow::Context,
    id: u64,
    vertex: &Rc<ShaderObj>,
    fragment: Option<&Rc<ShaderObj>>,
    empty_fragment: glow::Shader,
) -> Result<ProgramObj, GpuError> {
    // SAFETY: plain GL calls on the current context; the shaders are alive.
    let raw = unsafe {
        let raw = gl.create_program().map_err(|_| GpuError::OutOfMemory)?;
        gl.attach_shader(raw, vertex.raw);
        gl.attach_shader(raw, fragment.map_or(empty_fragment, |f| f.raw));
        gl.link_program(raw);
        raw
    };
    Ok(started(
        id,
        raw,
        (vertex.clone(), fragment.cloned()),
        (vertex.id, fragment.map_or(0, |f| f.id)),
    ))
}

/// Reads a started link, waiting for it, then points each combined sampler at a unit and each uniform
/// block at a binding point. Leaves the program in use. On failure the error carries GL's logs.
pub fn finish(gl: &glow::Context, program: &ProgramObj) -> Result<Linked, String> {
    let Some((vertex, fragment)) = program.shaders.borrow_mut().take() else {
        return Err("the link already failed".into());
    };
    let raw = program.raw;
    // SAFETY: plain GL calls on the current context; the program and shaders are alive.
    unsafe {
        if !gl.get_program_link_status(raw) {
            let mut log = gl.get_program_info_log(raw);
            for s in core::iter::once(&vertex).chain(&fragment) {
                if !gl.get_shader_compile_status(s.raw) {
                    log.push_str(&gl.get_shader_info_log(s.raw));
                    log.push_str(&s.source);
                }
            }
            return Err(log);
        }
        gl.use_program(Some(raw));
        let mut units = Vec::new();
        let mut sizes = Vec::new();
        let mut blocks: Vec<Slot> = Vec::new();
        for s in core::iter::once(&vertex).chain(&fragment) {
            for (name, texture, sampler) in &s.textures {
                if let Some(location) = gl.get_uniform_location(raw, name) {
                    gl.uniform_1_i32(Some(&location), units.len() as i32);
                    units.push((*texture, *sampler));
                    sizes.push(
                        gl.get_uniform_location(raw, &format!("{name}_size"))
                            .map(|l| (l, std::cell::Cell::new([-1; 3]))),
                    );
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
        let uniform = |name| {
            gl.get_uniform_location(raw, name)
                .map(|l| (l, std::cell::Cell::new(0)))
        };
        Ok(Linked {
            units,
            sizes,
            blocks,
            first_instance: uniform(glsl::FIRST_INSTANCE_BINDING),
            base_vertex: uniform(glsl::BASE_VERTEX_BINDING),
        })
    }
}
