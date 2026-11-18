//! Shaders written as GLSL ES 3.00 by naga, and programs linked once per pair of them.

use glow::HasContext;
use naga::back::glsl;
use std::rc::Rc;
use wcontainers_gpu::{Features, GpuError, ShaderModule, ShaderStage};

/// A (set, binding) pair: where a program reads a resource.
pub type Slot = (u32, u32);

/// A part of the immediates a program reads as a plain uniform (WebGL 2 has no immediates): its
/// GLSL name, its offset in the immediates and its type.
pub type Immediate = (String, u32, glsl::GlslUniformType);

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
    pub immediates: Vec<Immediate>,
    /// How many clip distances a vertex shader writes.
    pub clip_distances: u8,
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
    /// How it captures, for a capture program.
    pub capture: Option<Capture>,
    /// Its variants for vertex layouts GL ES cannot read as stored, by [`GlLayout::converted`],
    /// made at the first draw with each.
    ///
    /// [`GlLayout::converted`]: crate::state::GlLayout::converted
    pub variants: std::cell::RefCell<Vec<(u32, Rc<ProgramObj>)>>,
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
    /// Each part of the immediates the program reads: its uniform, offset and type.
    pub immediates: Vec<(glow::UniformLocation, u32, glsl::GlslUniformType)>,
    /// The immediates its uniforms hold.
    pub held: std::cell::RefCell<Vec<u8>>,
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

/// A module rewritten by [`split_samplers`], and each copy's slot with the slot it stands for.
type Split = (ShaderModule, Vec<(Slot, Slot)>);

/// The binding of the first copy [`split_samplers`] makes, past every binding a front gives.
const COPY_BINDINGS: u32 = 1 << 20;

/// GLSL combines each texture with one sampler, and naga's writer refuses a texture sampled through
/// two. Where a program does that, each texture read through a further sampler is read through a copy
/// of its global variable, at a binding of its own; the copies and the slots they stand for are
/// returned with the rewritten module (None where no texture has two samplers).
fn split_samplers(module: &ShaderModule) -> Result<Option<Split>, &'static str> {
    use naga::{Expression, GlobalVariable, Handle};
    use std::collections::{BTreeMap, btree_map::Entry};
    type Var = Handle<GlobalVariable>;
    let m = &module.module;
    // The sampler each texture keeps: its first by handle.
    let mut kept: BTreeMap<Var, Var> = BTreeMap::new();
    for (i, _) in m.entry_points.iter().enumerate() {
        for s in module.info.get_entry_point(i).sampling_set.iter() {
            let first = kept.entry(s.image).or_insert(s.sampler);
            *first = (*first).min(s.sampler);
        }
    }
    // Functions by handle, then entry points by index.
    let functions = m.functions.iter().map(|(h, f)| (Ok(h), f));
    let entries = m.entry_points.iter().enumerate();
    let mut moves = Vec::new();
    for (index, f) in functions.chain(entries.map(|(i, e)| (Err(i), &e.function))) {
        // The sampler each expression naming a texture is sampled through.
        let mut through: BTreeMap<Handle<Expression>, (Var, Var)> = BTreeMap::new();
        for (_, e) in f.expressions.iter() {
            let Expression::ImageSample { image, sampler, .. } = *e else {
                continue;
            };
            let (Expression::GlobalVariable(t), Expression::GlobalVariable(s)) =
                (&f.expressions[image], &f.expressions[sampler])
            else {
                continue;
            };
            match through.entry(image) {
                Entry::Vacant(v) => {
                    v.insert((*t, *s));
                }
                Entry::Occupied(o) if o.get().1 != *s => {
                    return Err("webgl: one expression of a texture is sampled through two \
                                samplers (not built)");
                }
                Entry::Occupied(_) => {}
            }
        }
        for (expression, (t, s)) in through {
            if kept.get(&t) != Some(&s) {
                moves.push((index, expression, t, s));
            }
        }
    }
    if moves.is_empty() {
        return Ok(None);
    }
    let mut out = m.clone();
    let mut copies: BTreeMap<(Var, Var), Var> = BTreeMap::new();
    let mut own = Vec::new();
    for (index, expression, t, s) in moves {
        let copy = match copies.get(&(t, s)) {
            Some(c) => *c,
            None => {
                let original = &out.global_variables[t];
                let Some(binding) = original.binding else {
                    return Err("webgl: a sampled texture without a binding");
                };
                let at = naga::ResourceBinding {
                    group: binding.group,
                    binding: COPY_BINDINGS + own.len() as u32,
                };
                own.push(((at.group, at.binding), (binding.group, binding.binding)));
                let var = GlobalVariable {
                    name: original.name.as_ref().map(|n| format!("{n}_{}", own.len())),
                    binding: Some(at),
                    ..original.clone()
                };
                let c = out.global_variables.append(var, Default::default());
                copies.insert((t, s), c);
                c
            }
        };
        let f = match index {
            Ok(h) => &mut out.functions[h],
            Err(i) => &mut out.entry_points[i].function,
        };
        f.expressions[expression] = Expression::GlobalVariable(copy);
    }
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&out)
    .map_err(|_| "webgl: a texture's copy for its second sampler does not validate")?;
    Ok(Some((
        ShaderModule {
            module: out,
            info,
            entry_point: module.entry_point.clone(),
            stage: module.stage,
            key: module.key,
        },
        own,
    )))
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
    let split = split_samplers(module).map_err(GpuError::Invalid)?;
    let (module, own) = match &split {
        Some((m, own)) => (m, &own[..]),
        None => (module, &[][..]),
    };
    let (source, reflection) = write_glsl(module, flip_y)
        .map_err(|_| GpuError::Invalid("webgl: naga could not write the shader as GLSL ES 3.00"))?;
    // A copy's slot is its texture's.
    let slot = |m: &naga::Module, var| {
        let at = slot(m, var)?;
        Some(own.iter().find(|(copy, _)| *copy == at).map_or(at, |o| o.1))
    };
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
    let immediates = reflection
        .immediates_items
        .iter()
        .map(|i| (i.access_path.clone(), i.offset, i.ty))
        .collect();
    let clip_distances = results(module)
        .find(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::ClipDistances))
        .map_or(0, |(_, n)| n);
    let cull_distances = results(module)
        .find(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::CullDistance))
        .map_or(0, |(_, n)| n);
    // WEBGL_clip_cull_distance guarantees eight of each and eight together, Direct3D's limit.
    if clip_distances + cull_distances > 8 {
        return Err(GpuError::Invalid(
            "webgl: more than eight clip and cull distances",
        ));
    }
    let raw = start_compile(gl, module.stage, &source)?;
    Ok(ShaderObj {
        id,
        stage: module.stage,
        raw,
        textures,
        blocks,
        source,
        outputs,
        immediates,
        clip_distances: clip_distances as u8,
    })
}

/// The entry point's output bindings, each with its array length (1 for what is not an array).
fn results(module: &ShaderModule) -> impl Iterator<Item = (naga::Binding, u32)> + '_ {
    let m = &module.module;
    let result = m
        .entry_points
        .iter()
        .find(|e| e.name == module.entry_point)
        .and_then(|e| e.function.result.as_ref());
    let length = |ty: naga::Handle<naga::Type>| match m.types[ty].inner {
        naga::TypeInner::Array {
            size: naga::ArraySize::Constant(n),
            ..
        } => n.get(),
        _ => 1,
    };
    result.into_iter().flat_map(move |r| {
        let single = r.binding.clone().map(|b| (b, length(r.ty)));
        let fields = match &m.types[r.ty].inner {
            naga::TypeInner::Struct { members, .. } if r.binding.is_none() => &members[..],
            _ => &[],
        };
        single.into_iter().chain(
            fields
                .iter()
                .filter_map(move |f| Some((f.binding.clone()?, length(f.ty)))),
        )
    })
}

/// The entry point's input bindings.
fn arguments(module: &ShaderModule) -> impl Iterator<Item = naga::Binding> + '_ {
    let m = &module.module;
    m.entry_points
        .iter()
        .filter(|e| e.name == module.entry_point)
        .flat_map(|e| &e.function.arguments)
        .flat_map(move |a| {
            let fields = match &m.types[a.ty].inner {
                naga::TypeInner::Struct { members, .. } if a.binding.is_none() => &members[..],
                _ => &[],
            };
            a.binding
                .clone()
                .into_iter()
                .chain(fields.iter().filter_map(|f| f.binding.clone()))
        })
}

/// The features a shader needs that a back end may lack, read from its inputs and outputs, the
/// globals its entry point uses, its types and its expressions. Storage that a fragment program only
/// reads is named by `FRAGMENT_STORAGE_WRITE`, as binding sets name it: the interface has no bit of
/// its own for it.
pub fn needs(module: &ShaderModule) -> Features {
    use naga::{
        AddressSpace, Binding, BuiltIn, Expression, ImageClass, ImageQuery, Sampling, ScalarKind,
        StorageAccess, TypeInner,
    };
    let m = &module.module;
    let vertex = module.stage == ShaderStage::Vertex;
    let storage = |access: StorageAccess| match (vertex, access.contains(StorageAccess::STORE)) {
        (true, false) => Features::VERTEX_STORAGE_READ,
        (true, true) => Features::VERTEX_STORAGE_WRITE,
        (false, _) => Features::FRAGMENT_STORAGE_WRITE,
    };
    let mut f = Features::empty();
    for b in results(module).map(|(b, _)| b).chain(arguments(module)) {
        f |= match b {
            Binding::BuiltIn(BuiltIn::ClipDistances) => Features::CLIP_DISTANCE,
            Binding::BuiltIn(BuiltIn::CullDistance) => Features::CULL_DISTANCE,
            Binding::BuiltIn(BuiltIn::PointSize) => Features::POINT_SIZE,
            Binding::BuiltIn(BuiltIn::SampleIndex | BuiltIn::SampleMask) => {
                Features::SAMPLE_SHADING
            }
            Binding::BuiltIn(BuiltIn::PrimitiveIndex) => Features::PRIMITIVE_INDEX,
            Binding::BuiltIn(
                BuiltIn::NumSubgroups
                | BuiltIn::SubgroupId
                | BuiltIn::SubgroupSize
                | BuiltIn::SubgroupInvocationId,
            ) => Features::SUBGROUPS,
            Binding::Location {
                blend_src: Some(1), ..
            } => Features::DUAL_SOURCE_BLEND,
            Binding::Location {
                sampling: Some(Sampling::Sample),
                ..
            } => Features::SAMPLE_SHADING,
            _ => Features::empty(),
        };
    }
    let entry = m
        .entry_points
        .iter()
        .position(|e| e.name == module.entry_point);
    if let Some(i) = entry {
        let used = module.info.get_entry_point(i);
        for (h, var) in m.global_variables.iter() {
            if used[h].is_empty() {
                continue;
            }
            if let AddressSpace::Storage { access } = var.space {
                f |= storage(access);
            }
            f |= match &m.types[var.ty].inner {
                TypeInner::Image { class, .. } => match *class {
                    ImageClass::Sampled { multi: true, .. } | ImageClass::Depth { multi: true } => {
                        Features::MULTISAMPLE_READ
                    }
                    ImageClass::Storage { access, .. } => storage(access),
                    _ => Features::empty(),
                },
                TypeInner::BindingArray { .. } => Features::RESOURCE_TABLES,
                _ => Features::empty(),
            };
        }
    }
    // Scalar widths, from the declared types and from each expression's type (naga keeps the
    // types of most expressions out of the module's arena).
    let width = |inner: &TypeInner| match *inner {
        TypeInner::Scalar(s)
        | TypeInner::Vector { scalar: s, .. }
        | TypeInner::Matrix { scalar: s, .. }
        | TypeInner::Atomic(s) => match (s.kind, s.width) {
            (ScalarKind::Float, 2) => Features::SHADER_F16,
            (ScalarKind::Float, 8) => Features::SHADER_F64,
            (ScalarKind::Sint | ScalarKind::Uint, 8) => Features::SHADER_I64,
            _ => Features::empty(),
        },
        _ => Features::empty(),
    };
    for (_, t) in m.types.iter() {
        f |= width(&t.inner);
    }
    let functions = m.functions.iter().map(|(h, func)| (func, &module.info[h]));
    let entry = entry.map(|i| (&m.entry_points[i].function, module.info.get_entry_point(i)));
    for (func, info) in functions.chain(entry) {
        for (h, e) in func.expressions.iter() {
            f |= width(info[h].ty.inner_with(&m.types));
            f |= match e {
                Expression::ImageQuery {
                    query: ImageQuery::NumLevels | ImageQuery::NumSamples,
                    ..
                } => Features::TEXTURE_LEVEL_QUERY,
                Expression::SubgroupBallotResult | Expression::SubgroupOperationResult { .. } => {
                    Features::SUBGROUPS
                }
                _ => Features::empty(),
            };
        }
    }
    f
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

/// Starts linking `program`'s shaders again from their sources, as a capture program whose
/// location outputs are transform feedback varyings (one that does not draw links the empty fragment
/// shader, so no fragment output meets the framebuffer), or for a vertex layout whose attributes in
/// `convert` GL ES cannot read as stored (see [`convert_inputs`]). The new program captures as
/// `program` does unless `capture` names how.
pub fn relink(
    gl: &glow::Context,
    id: u64,
    program: &ProgramObj,
    capture: Option<Capture>,
    convert: u32,
    empty_fragment: glow::Shader,
) -> Result<ProgramObj, GpuError> {
    let capture = capture.or(program.capture);
    let (vertex, fragment) = &program.stages;
    let fresh = |s: &ShaderObj, source: String| -> Result<Rc<ShaderObj>, GpuError> {
        Ok(Rc::new(ShaderObj {
            raw: start_compile(gl, s.stage, &source)?,
            textures: s.textures.clone(),
            blocks: s.blocks.clone(),
            source,
            outputs: s.outputs.clone(),
            immediates: s.immediates.clone(),
            ..*s
        }))
    };
    let source = match convert {
        0 => vertex.source.clone(),
        _ => convert_inputs(&vertex.source, convert).ok_or(GpuError::Invalid(
            "webgl: a converted vertex attribute the program does not read as naga writes it",
        ))?,
    };
    let v = fresh(vertex, source)?;
    let f = match fragment {
        Some(f) if capture.is_none_or(|c| c.rasterize) => Some(fresh(f, f.source.clone())?),
        _ => None,
    };
    let names: Vec<&str> = v.outputs.iter().map(|(n, _)| n.as_str()).collect();
    // SAFETY: plain GL calls on the current context; the shaders are alive.
    let raw = unsafe {
        let raw = gl.create_program().map_err(|_| GpuError::OutOfMemory)?;
        gl.attach_shader(raw, v.raw);
        gl.attach_shader(raw, f.as_ref().map_or(empty_fragment, |f| f.raw));
        if let Some(c) = capture {
            let mode = if c.separate {
                glow::SEPARATE_ATTRIBS
            } else {
                glow::INTERLEAVED_ATTRIBS
            };
            gl.transform_feedback_varyings(raw, &names, mode);
        }
        gl.link_program(raw);
        // Flagged only: GL deletes them with the program.
        gl.delete_shader(v.raw);
        if let Some(f) = &f {
            gl.delete_shader(f.raw);
        }
        raw
    };
    let mut p = started(id, raw, (v, f), program.pair);
    p.capture = capture;
    Ok(p)
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
        capture: None,
        variants: Default::default(),
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
        let mut immediates = Vec::new();
        for s in core::iter::once(&vertex).chain(&fragment) {
            for (name, offset, ty) in &s.immediates {
                if let Some(location) = gl.get_uniform_location(raw, name) {
                    immediates.push((location, *offset, *ty));
                }
            }
            for (name, texture, sampler) in &s.textures {
                let location = gl.get_uniform_location(raw, name);
                let size = gl
                    .get_uniform_location(raw, &format!("{name}_size"))
                    .map(|l| (l, std::cell::Cell::new([-1; 3])));
                // A texture only sized has no sampler uniform left, but still takes a unit so its
                // size uniform follows the bound view.
                if location.is_none() && size.is_none() {
                    continue;
                }
                if let Some(location) = &location {
                    gl.uniform_1_i32(Some(location), units.len() as i32);
                }
                units.push((*texture, *sampler));
                sizes.push(size);
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
            immediates,
            held: Default::default(),
        })
    }
}

/// Sets the uniforms standing for the program's immediates from `bytes`, unless they hold them
/// already. Bytes past the end of `bytes` read as zero, as GL's uniforms start. The program is in use.
pub fn set_immediates(gl: &glow::Context, linked: &Linked, bytes: &[u8]) {
    use glsl::{GlslScalarKind as Kind, GlslUniformType as Ty};
    let mut held = linked.held.borrow_mut();
    if linked.immediates.is_empty() || *held == bytes {
        return;
    }
    held.clear();
    held.extend_from_slice(bytes);
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map_or([0; 4], |w| [w[0], w[1], w[2], w[3]])
    };
    for (location, offset, ty) in &linked.immediates {
        let at = *offset as usize;
        let l = Some(location);
        // SAFETY: plain GL calls; the program is in use.
        unsafe {
            match *ty {
                Ty::Scalar(s) | Ty::Vector { scalar: s, .. } => {
                    let n = match *ty {
                        Ty::Vector { size, .. } => size as usize,
                        _ => 1,
                    };
                    let words: [[u8; 4]; 4] = core::array::from_fn(|i| word(at + 4 * i));
                    match s.kind {
                        Kind::Float => {
                            let v = words.map(f32::from_le_bytes);
                            let v = &v[..n];
                            match n {
                                1 => gl.uniform_1_f32_slice(l, v),
                                2 => gl.uniform_2_f32_slice(l, v),
                                3 => gl.uniform_3_f32_slice(l, v),
                                _ => gl.uniform_4_f32_slice(l, v),
                            }
                        }
                        Kind::Sint => {
                            let v = words.map(i32::from_le_bytes);
                            let v = &v[..n];
                            match n {
                                1 => gl.uniform_1_i32_slice(l, v),
                                2 => gl.uniform_2_i32_slice(l, v),
                                3 => gl.uniform_3_i32_slice(l, v),
                                _ => gl.uniform_4_i32_slice(l, v),
                            }
                        }
                        Kind::Uint => {
                            let v = words.map(u32::from_le_bytes);
                            let v = &v[..n];
                            match n {
                                1 => gl.uniform_1_u32_slice(l, v),
                                2 => gl.uniform_2_u32_slice(l, v),
                                3 => gl.uniform_3_u32_slice(l, v),
                                _ => gl.uniform_4_u32_slice(l, v),
                            }
                        }
                    }
                }
                Ty::Matrix { columns, rows, .. } => {
                    // Columns are vectors of `rows`, each aligned as such a vector; GL takes them packed.
                    let (c, r) = (columns as usize, rows as usize);
                    let stride = if r == 2 { 8 } else { 16 };
                    let mut v = [0f32; 16];
                    for col in 0..c {
                        for row in 0..r {
                            v[col * r + row] =
                                f32::from_le_bytes(word(at + col * stride + 4 * row));
                        }
                    }
                    let v = &v[..c * r];
                    match (c, r) {
                        (2, 2) => gl.uniform_matrix_2_f32_slice(l, false, v),
                        (3, 3) => gl.uniform_matrix_3_f32_slice(l, false, v),
                        (4, 4) => gl.uniform_matrix_4_f32_slice(l, false, v),
                        (2, 3) => gl.uniform_matrix_2x3_f32_slice(l, false, v),
                        (3, 2) => gl.uniform_matrix_3x2_f32_slice(l, false, v),
                        (2, 4) => gl.uniform_matrix_2x4_f32_slice(l, false, v),
                        (4, 2) => gl.uniform_matrix_4x2_f32_slice(l, false, v),
                        (3, 4) => gl.uniform_matrix_3x4_f32_slice(l, false, v),
                        _ => gl.uniform_matrix_4x3_f32_slice(l, false, v),
                    }
                }
            }
        }
    }
}

/// Attributes GL ES cannot read as stored are read as something it can, and turned into what the
/// program reads at the start of `main`: per location, two bits of `convert` (see
/// [`crate::state::conversion`]). Attributes the program does not read stay as they are. None when
/// the source has no `main` as naga writes it.
pub fn convert_inputs(source: &str, convert: u32) -> Option<String> {
    const SMALL_FLOAT: &str = "float _wc_small_float(uint b, uint m) {
    uint e = b >> m;
    uint f = b & ((1u << m) - 1u);
    float scale = float(1u << m);
    if (e == 0u) { return float(f) / scale * exp2(-14.0); }
    if (e == 31u) { return uintBitsToFloat(f == 0u ? 0x7f800000u : 0x7fc00000u); }
    return exp2(float(e) - 15.0) * (1.0 + float(f) / scale);
}
";
    let mut out = String::with_capacity(source.len() + 1024);
    let mut starts = String::new();
    for line in source.lines() {
        let declared = (0..16u32).find(|l| {
            convert >> (2 * l) & 3 != 0
                && line.starts_with(&format!("layout(location = {l}) in "))
                && line.ends_with(&format!(" _p2vs_location{l};"))
        });
        let Some(l) = declared else {
            out.push_str(line);
            out.push('\n');
            if line == "void main() {" {
                out.push_str(&starts);
            }
            continue;
        };
        let ty = line
            .strip_prefix(&format!("layout(location = {l}) in "))?
            .strip_suffix(&format!(" _p2vs_location{l};"))?;
        let (raw, value) = match convert >> (2 * l) & 3 {
            1 => ("vec4", format!("_wc_raw{l}.zyxw")),
            2 => (
                "uint",
                format!(
                    "uvec4(_wc_raw{l} & 1023u, (_wc_raw{l} >> 10) & 1023u, \
                     (_wc_raw{l} >> 20) & 1023u, _wc_raw{l} >> 30)"
                ),
            ),
            _ => (
                "uint",
                format!(
                    "vec4(_wc_small_float(_wc_raw{l} & 2047u, 6u), \
                     _wc_small_float((_wc_raw{l} >> 11) & 2047u, 6u), \
                     _wc_small_float(_wc_raw{l} >> 22, 5u), 1.0)"
                ),
            ),
        };
        out.push_str(&format!(
            "layout(location = {l}) in {raw} _wc_raw{l};\n{ty} _p2vs_location{l};\n"
        ));
        if convert >> (2 * l) & 3 == 3 && !out.contains("_wc_small_float(uint") {
            out.push_str(SMALL_FLOAT);
        }
        starts.push_str(&format!("    _p2vs_location{l} = {ty}({value});\n"));
    }
    out.contains("void main() {").then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vertex program's clip distances are counted, and written as `gl_ClipDistance` under
    /// WEBGL_clip_cull_distance's directive.
    #[test]
    fn clip_distances_are_written_and_counted() {
        let wgsl = "
            enable clip_distances;
            struct Out {
                @builtin(position) p: vec4<f32>,
                @builtin(clip_distances) d: array<f32, 2>,
            }
            @vertex fn vs(@location(0) p: vec4<f32>) -> Out {
                return Out(p, array<f32, 2>(p.x, p.y));
            }";
        let module = naga::front::wgsl::parse_str(wgsl).expect("parses");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("validates");
        let module = ShaderModule {
            module,
            info,
            entry_point: "vs".into(),
            stage: ShaderStage::Vertex,
            key: [0; 32],
        };
        assert_eq!(needs(&module), Features::CLIP_DISTANCE);
        let distances: Vec<_> = results(&module)
            .filter(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::ClipDistances))
            .collect();
        assert_eq!(distances.len(), 1);
        assert_eq!(distances[0].1, 2);
        let (glsl, _) = write_glsl(&module, false).expect("writes");
        assert!(
            glsl.contains("#extension GL_ANGLE_clip_cull_distance"),
            "{glsl}"
        );
        assert!(glsl.contains("gl_ClipDistance = _tmp_return.d;"), "{glsl}");
    }

    /// The attributes a layout converts are declared as GL reads them and turned into what the
    /// program reads at the start of `main`; others stay.
    #[test]
    fn converted_inputs_are_turned_into_their_values() {
        let wgsl = "
            @vertex fn vs(@location(0) p: vec4<f32>, @location(1) c: vec4<f32>,
                          @location(2) u: vec4<u32>, @location(3) f: vec3<f32>)
                -> @builtin(position) vec4<f32> {
                return p + c + vec4<f32>(u) + vec4<f32>(f, 0.0);
            }";
        let module = ShaderModule::from_wgsl(wgsl, "vs", ShaderStage::Vertex).expect("validates");
        let (glsl, _) = write_glsl(&module, false).expect("writes");
        let convert = 1 << 2 | 2 << 4 | 3 << 6;
        let out = convert_inputs(&glsl, convert).expect("converts");
        for line in [
            "layout(location = 0) in vec4 _p2vs_location0;",
            "layout(location = 1) in vec4 _wc_raw1;",
            "layout(location = 2) in uint _wc_raw2;",
            "layout(location = 3) in uint _wc_raw3;",
            "uvec4 _p2vs_location2;",
            "    _p2vs_location1 = vec4(_wc_raw1.zyxw);",
        ] {
            assert!(out.lines().any(|l| l == line), "{line} in\n{out}");
        }
        assert_eq!(out.matches("float _wc_small_float(").count(), 1);
        let main = out.find("void main() {").expect("main");
        assert!(out[main..].contains("_p2vs_location3 = vec3(vec4(_wc_small_float("));
    }
}
