//! Draws: each stage's program converted by the NIR route for the state it is drawn with, its
//! resources bound by the route's reflection, and the draw handed to the pass tracker's pass.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::c_uint;

use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendDesc, BlendFactor, BufferRange, CullMode,
    DrawCall, DrawRange, FanOrLoop, Features, FormatKind, FormatUse, IndexBinding, IndexFormat,
    IndexSource, MAX_BINDING_SETS, Object, PassTracker, Program, ProvokingVertex, ReadSource,
    ReflectedBinding, RingKind, STORAGE_WORD, ScissorRect, ShaderModule, ShaderStage, SlotKind,
    SlowPath, StageSet, Topology, VertexBinding, Viewport, cut_scissors,
};
use wcontainers_nir::mesa::{PositionTransform, TextureSwizzle};

use crate::context::Context;
use crate::gallium::*;
use crate::resource::Resource;
use crate::screen::Screen;
use crate::state::{self, Shader, Stage, refuse};
use crate::view::{IDENTITY, View};

/// One stage's program converted for one variant, and what it binds.
struct Built<B: Backend> {
    shader: B::Shader,
    bindings: Vec<ReflectedBinding>,
    /// The bytes each uniform block declares, by its binding in the converted program.
    uniform_sizes: HashMap<u32, u64>,
    /// The program reads values interpolated flat, taken from the provoking vertex.
    flat_inputs: bool,
    /// How a fragment program interpolates each input, by location.
    inputs: Vec<Interpolated>,
}

/// A shader state as a map key: the address Mesa holds it at.
type ShaderId = usize;

/// What a program is converted for beside its NIR: the position's transform (vertex programs) and
/// the swizzles of the views bound where it samples.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Variant {
    transform: PositionTransform,
    swizzles: Vec<TextureSwizzle>,
    /// How the fragment program it meets interpolates each input (vertex programs).
    interpolation: Vec<Interpolated>,
}

/// A value passed between stages at one location, and how it is interpolated.
type Interpolated = (u32, Option<naga::Interpolation>, Option<naga::Sampling>);

impl<B: Backend> Programs<B> {
    /// `shader` converted for `variant`, converted now if it was not before.
    fn built_for(
        &mut self,
        backend: &mut B,
        shader: &Shader,
        variant: &Variant,
    ) -> Result<&Built<B>, String> {
        let key = (std::ptr::from_ref(shader) as ShaderId, variant.clone());
        Ok(match self.built.entry(key) {
            Entry::Occupied(built) => built.into_mut(),
            Entry::Vacant(slot) => slot.insert(convert(backend, shader, variant)?),
        })
    }
}

/// The programs and blocks made so far, by what they were made from.
pub struct Programs<B: Backend> {
    built: HashMap<(ShaderId, Variant), Built<B>>,
    programs: HashMap<(ShaderId, Variant, ShaderId, Variant), B::Program>,
    blends: HashMap<BlendDesc, B::Blend>,
}

impl<B: Backend> Default for Programs<B> {
    fn default() -> Self {
        Programs {
            built: HashMap::new(),
            programs: HashMap::new(),
            blends: HashMap::new(),
        }
    }
}

impl<B: Backend> Programs<B> {
    /// Drops everything made from one shader state, which Mesa is deleting.
    pub fn forget(&mut self, screen: &Screen<B>, shader: *const Shader) {
        let id = shader as ShaderId;
        let built: Vec<_> = self.built.keys().filter(|k| k.0 == id).cloned().collect();
        for key in built {
            if let Some(b) = self.built.remove(&key) {
                screen.retire(Object::Shader(b.shader));
            }
        }
        let programs: Vec<_> = self
            .programs
            .keys()
            .filter(|k| k.0 == id || k.2 == id)
            .cloned()
            .collect();
        for key in programs {
            if let Some(p) = self.programs.remove(&key) {
                screen.retire(Object::Program(p));
            }
        }
    }

    /// Gives every object back, with the context.
    pub fn release(&mut self, screen: &Screen<B>) {
        for (_, b) in self.built.drain() {
            screen.retire(Object::Shader(b.shader));
        }
        for (_, p) in self.programs.drain() {
            screen.retire(Object::Program(p));
        }
        for (_, b) in self.blends.drain() {
            screen.retire(Object::Blend(b));
        }
    }
}

/// The bytes of each uniform block a converted program declares, by binding.
fn uniform_sizes(module: &naga::Module) -> HashMap<u32, u64> {
    let mut layouter = naga::proc::Layouter::default();
    if layouter.update(module.to_ctx()).is_err() {
        return HashMap::new();
    }
    module
        .global_variables
        .iter()
        .filter(|(_, g)| g.space == naga::AddressSpace::Uniform)
        .filter_map(|(_, g)| Some((g.binding.as_ref()?.binding, u64::from(layouter[g.ty].size))))
        .collect()
}

/// Whether an entry point's inputs include a flat value.
fn flat_inputs(module: &naga::Module) -> bool {
    let flat = |binding: &Option<naga::Binding>| {
        matches!(
            binding,
            Some(naga::Binding::Location {
                interpolation: Some(naga::Interpolation::Flat),
                ..
            })
        )
    };
    module.entry_points.iter().any(|entry| {
        entry.function.arguments.iter().any(|argument| {
            flat(&argument.binding)
                || match &module.types[argument.ty].inner {
                    naga::TypeInner::Struct { members, .. } => {
                        members.iter().any(|m| flat(&m.binding))
                    }
                    _ => false,
                }
        })
    })
}

/// How an entry point interpolates each input it reads at a location.
fn input_interpolation(module: &naga::Module) -> Vec<Interpolated> {
    let mut found = Vec::new();
    let mut note = |binding: &Option<naga::Binding>| {
        if let Some(naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) = binding
        {
            found.push((*location, *interpolation, *sampling));
        }
    };
    for entry in &module.entry_points {
        for argument in &entry.function.arguments {
            note(&argument.binding);
            if let naga::TypeInner::Struct { members, .. } = &module.types[argument.ty].inner {
                members.iter().for_each(|m| note(&m.binding));
            }
        }
    }
    found.sort_unstable_by_key(|&(location, ..)| location);
    found
}

/// A vertex program's outputs interpolated as the fragment program's inputs at the same
/// locations: the host requires the two to agree, where OpenGL's fragment stage alone decides.
fn meet_inputs(module: &mut naga::Module, inputs: &[Interpolated]) {
    let meet = |binding: &mut Option<naga::Binding>| {
        if let Some(naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) = binding
            && let Some(&(_, i, s)) = inputs.iter().find(|&&(at, ..)| at == *location)
        {
            (*interpolation, *sampling) = (i, s);
        }
    };
    let types = &mut module.types;
    for entry in &mut module.entry_points {
        let Some(result) = &mut entry.function.result else {
            continue;
        };
        meet(&mut result.binding);
        let old = &types[result.ty];
        if let naga::TypeInner::Struct { members, span } = &old.inner {
            let mut members = members.clone();
            members.iter_mut().for_each(|m| meet(&mut m.binding));
            let new = naga::Type {
                name: old.name.clone(),
                inner: naga::TypeInner::Struct {
                    members,
                    span: *span,
                },
            };
            if types.get(&new).is_none() {
                types.replace(result.ty, new);
            }
        }
    }
}

/// One stage's program through the NIR route, for `variant`.
fn convert<B: Backend>(
    backend: &mut B,
    shader: &Shader,
    variant: &Variant,
) -> Result<Built<B>, String> {
    let mut nir = shader.nir.try_clone().ok_or("out of memory")?;
    if variant.transform != PositionTransform::default() {
        nir.transform_position(variant.transform);
    }
    if !variant.swizzles.is_empty() {
        nir.swizzle_textures(&variant.swizzles);
    }
    let key = nir.key().map_err(|e| e.to_string())?;
    let stage = shader.stage.nir();
    let bindings = nir.to_common_form(stage).map_err(|e| e.to_string())?;
    let mut module = wcontainers_nir::direct::module(&mut nir).map_err(|e| e.to_string())?;
    if !variant.interpolation.is_empty() {
        meet_inputs(&mut module, &variant.interpolation);
    }
    let info = wcontainers_nir::spirv::validate(&module).map_err(|e| e.to_string())?;
    let entry_point = module
        .entry_points
        .first()
        .map(|e| e.name.clone())
        .ok_or("no entry point")?;
    let uniform_sizes = uniform_sizes(&module);
    let flat_inputs = flat_inputs(&module);
    let inputs = input_interpolation(&module);
    let module = ShaderModule {
        module,
        info,
        entry_point,
        stage: stage.shader_stage(),
        key,
    };
    let shader = backend
        .create_shader(&module)
        .map_err(|e| format!("{e:?}"))?;
    Ok(Built {
        shader,
        bindings,
        uniform_sizes,
        flat_inputs,
        inputs,
    })
}

/// The interface's topology of a Gallium primitive, and whether it is a fan or loop.
fn topology(mode: u8) -> Topology {
    match mesa_prim::from(mode) {
        MESA_PRIM_POINTS => Topology::PointList,
        MESA_PRIM_LINES => Topology::LineList,
        MESA_PRIM_LINE_STRIP => Topology::LineStrip,
        MESA_PRIM_LINE_LOOP => Topology::LineLoop,
        MESA_PRIM_TRIANGLES => Topology::TriangleList,
        MESA_PRIM_TRIANGLE_STRIP => Topology::TriangleStrip,
        MESA_PRIM_TRIANGLE_FAN => Topology::TriangleFan,
        _ => refuse("a primitive the screen does not report"),
    }
}

fn index_format(size: u16) -> IndexFormat {
    match u32::from(size) {
        s if s == IndexFormat::U8.size() => IndexFormat::U8,
        s if s == IndexFormat::U16.size() => IndexFormat::U16,
        _ => IndexFormat::U32,
    }
}

/// The interface's viewport of Gallium's, and how the vertex program meets it: the NIR route
/// takes a program's clip space as Vulkan's, whose viewport maps y downwards in the rows, as a
/// Gallium viewport with a positive y scale does, so a negative scale negates y in the program
/// first; depth from OpenGL's -1..1 moves to the host's 0..1 in the program, and a range whose
/// near value is the larger is drawn reversed.
fn viewport(v: &pipe_viewport_state, minus_one: bool) -> (Viewport, PositionTransform) {
    let [sx, sy, sz] = v.scale;
    let [tx, ty, tz] = v.translate;
    let (near, far) = if minus_one {
        (tz - sz, tz + sz)
    } else {
        (tz, tz + sz)
    };
    let reverse_z = near > far;
    let (min, max) = if reverse_z { (far, near) } else { (near, far) };
    (
        Viewport {
            x: (tx - sx.abs()).to_bits(),
            y: (ty - sy.abs()).to_bits(),
            width: (2.0 * sx.abs()).to_bits(),
            height: (2.0 * sy.abs()).to_bits(),
            min_depth: min.to_bits(),
            max_depth: max.to_bits(),
        },
        PositionTransform {
            flip_y: sy < 0.0,
            half_z: minus_one,
            reverse_z,
            fixed_point_size: false,
        },
    )
}

/// Every factor of a blend block, source and destination, colour and alpha.
fn factors(desc: &mut BlendDesc) -> impl Iterator<Item = &mut BlendFactor> {
    desc.targets
        .iter_mut()
        .filter_map(|t| t.blend.as_mut())
        .flat_map(|(color, alpha)| {
            [
                &mut color.src,
                &mut color.dst,
                &mut alpha.src,
                &mut alpha.dst,
            ]
        })
}

/// The blend block of `desc` for the framebuffer's formats (no blending on targets that cannot
/// blend: integer targets, as OpenGL defines), and whether the block reads the blend constant's
/// alpha in every channel: on a back end without constant alpha factors, a block that uses no
/// constant colour takes the constant factors instead, given the alpha in all four channels.
fn blend_for<B: Backend>(
    programs: &mut Programs<B>,
    backend: &mut B,
    desc: &BlendDesc,
    framebuffer: &pipe_framebuffer_state,
) -> (B::Blend, bool) {
    let mut desc = *desc;
    let caps = backend.caps().clone();
    let mut alpha_constant = false;
    if !caps.has(Features::BLEND_CONSTANT_ALPHA) {
        let used: Vec<BlendFactor> = factors(&mut desc).map(|f| *f).collect();
        let reads_alpha = used.iter().any(|f| {
            matches!(
                f,
                BlendFactor::ConstantAlpha | BlendFactor::OneMinusConstantAlpha
            )
        });
        if reads_alpha {
            if used
                .iter()
                .any(|f| matches!(f, BlendFactor::Constant | BlendFactor::OneMinusConstant))
            {
                refuse(
                    "the blend constant's colour and its alpha in one block (blending in the program is not built)",
                );
            }
            for f in factors(&mut desc) {
                *f = match *f {
                    BlendFactor::ConstantAlpha => BlendFactor::Constant,
                    BlendFactor::OneMinusConstantAlpha => BlendFactor::OneMinusConstant,
                    other => other,
                };
            }
            alpha_constant = true;
        }
    }
    for (i, target) in desc.targets.iter_mut().enumerate() {
        let format = framebuffer
            .cbufs
            .get(i)
            .filter(|_| i < usize::from(framebuffer.nr_cbufs))
            .and_then(|s| crate::format::format(s.format()));
        let blends = format.is_some_and(|f| {
            !matches!(f.kind(), FormatKind::Uint | FormatKind::Sint)
                && caps.formats.get(f).contains(FormatUse::BLEND)
        });
        if !blends {
            target.blend = None;
        }
    }
    if desc.targets.iter().any(|t| t != &desc.targets[0]) {
        desc.same_for_all = false;
    }
    if let Some(block) = programs.blends.get(&desc) {
        return (block.clone(), alpha_constant);
    }
    match backend.create_blend(&desc) {
        Ok(block) => {
            programs.blends.insert(desc, block.clone());
            (block, alpha_constant)
        }
        Err(error) => refuse(&format!("a blend state: {error:?}")),
    }
}

/// Where a uniform block's bytes come from: the next copy written to the ring, or a range of a
/// buffer.
enum Uniform<'a, B: Backend> {
    Copied,
    Range(&'a B::Buffer, u64, u64),
}

/// Binds every resource the two programs read, one set a stage, as the route's reflection
/// numbers them.
fn binding_sets<B: Backend>(
    backend: &mut B,
    stages: &state::Stages<B>,
    built: [&Built<B>; Stage::ALL.len()],
) -> Result<[Option<B::BindingSet>; MAX_BINDING_SETS], String> {
    let mut sets = [const { None }; MAX_BINDING_SETS];
    let align = backend.caps().limits.min_uniform_offset_alignment;
    for (stage, built) in Stage::ALL.into_iter().zip(built) {
        let bound = stages.get(stage);
        let visible = match stage {
            Stage::Vertex => StageSet::VERTEX,
            Stage::Fragment => StageSet::FRAGMENT,
        };
        // Uniform blocks the program reads that the frontend handed as bytes go through the
        // ring first, each grown to the block's declared size.
        let mut copies: Vec<Vec<u8>> = Vec::new();
        let mut uniforms: Vec<(u32, Uniform<'_, B>)> = Vec::new();
        for b in built
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Uniform)
        {
            let slot = &bound.constants[b.register as usize];
            let declared = built.uniform_sizes.get(&b.binding).copied().unwrap_or(0);
            if !slot.bytes.is_empty() || slot.resource.is_null() {
                let mut bytes = slot.bytes.clone();
                bytes.resize(bytes.len().max(declared as usize), 0);
                copies.push(bytes);
                uniforms.push((b.binding, Uniform::Copied));
                continue;
            }
            // SAFETY: the slot holds a reference on a live buffer.
            let resource = unsafe { Resource::<B>::of(slot.resource) };
            let buffer = resource
                .buffer()
                .ok_or("a uniform block bound to a texture")?;
            let size = u64::from(slot.size).max(declared);
            if u64::from(slot.offset) + size > u64::from(resource.base.width0) {
                backend.slow_paths().hit(SlowPath::ConstantBufferGrown);
                return Err("a uniform block bound smaller than the program declares".into());
            }
            uniforms.push((
                b.binding,
                Uniform::Range(buffer, u64::from(slot.offset), size),
            ));
        }
        let parts: Vec<&[u8]> = copies.iter().map(Vec::as_slice).collect();
        let mut offsets = vec![0; parts.len()];
        let ring = if parts.is_empty() {
            None
        } else {
            Some(
                backend
                    .write_buffer_ring(RingKind::Data, &parts, align, &mut offsets)
                    .map_err(|e| format!("{e:?}"))?,
            )
        };
        let mut copied = offsets.iter().zip(&copies);
        let mut entries = Vec::new();
        for (binding, uniform) in &uniforms {
            let range = match uniform {
                Uniform::Copied => {
                    let (offset, bytes) = copied.next().ok_or("a uniform copy")?;
                    BufferRange {
                        buffer: ring.as_ref().ok_or("the ring")?,
                        offset: *offset,
                        size: bytes.len() as u64,
                    }
                }
                Uniform::Range(buffer, offset, size) => BufferRange {
                    buffer: *buffer,
                    offset: *offset,
                    size: *size,
                },
            };
            entries.push(BindingEntry {
                binding: *binding,
                stages: visible,
                resource: BindingResource::UniformBuffer(range),
            });
        }
        for b in &built.bindings {
            let resource = match b.kind {
                SlotKind::Uniform => continue,
                SlotKind::Texture => {
                    let view = bound.views[b.register as usize];
                    if view.is_null() {
                        return Err("a texture the program reads is not bound".into());
                    }
                    // SAFETY: the slot holds a reference on a live view of this driver.
                    let view = unsafe { View::<B>::of(view) };
                    BindingResource::Texture(view.view()?)
                }
                SlotKind::Sampler => {
                    let sampler = bound.samplers[b.register as usize];
                    if sampler.is_null() {
                        return Err("a sampler the program reads is not bound".into());
                    }
                    // SAFETY: a bound sampler lives until Mesa unbinds and deletes it.
                    BindingResource::Sampler(unsafe { &(*sampler).sampler })
                }
                _ => return Err("storage the screen does not report".into()),
            };
            entries.push(BindingEntry {
                binding: b.binding,
                stages: visible,
                resource,
            });
        }
        if entries.is_empty() {
            continue;
        }
        let set = wcontainers_nir::bind::stage_set(match stage {
            Stage::Vertex => ShaderStage::Vertex,
            Stage::Fragment => ShaderStage::Fragment,
        }) as usize;
        sets[set] = Some(
            backend
                .create_binding_set(&entries)
                .map_err(|e| format!("{e:?}"))?,
        );
    }
    Ok(sets)
}

/// `pipe_context::draw_vbo`.
pub unsafe extern "C" fn draw_vbo<B: Backend>(
    pipe: *mut pipe_context,
    info: *const pipe_draw_info,
    _: c_uint,
    indirect: *const pipe_draw_indirect_info,
    draws: *const pipe_draw_start_count_bias,
    count: c_uint,
) {
    if !indirect.is_null() {
        refuse("an indirect draw (the screen reports none)");
    }
    // SAFETY: Mesa passes this driver's context, the draw and `count` ranges.
    let (context, info, draws) = unsafe {
        (
            Context::<B>::of(pipe),
            &*info,
            std::slice::from_raw_parts(draws, count as usize),
        )
    };
    if let Err(reason) = draw(context, info, draws) {
        refuse(&reason);
    }
}

fn draw<B: Backend>(
    context: &mut Context<B>,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
) -> Result<(), String> {
    let bound = &context.bound;
    let (raster, depth_stencil, blend, elements) = (
        bound.raster,
        bound.depth_stencil,
        bound.blend,
        bound.elements,
    );
    if raster.is_null() || depth_stencil.is_null() || blend.is_null() || elements.is_null() {
        return Err("a draw without rasterizer, depth, blend or vertex state".into());
    }
    // SAFETY: bound states live until Mesa unbinds and deletes them.
    let (raster, depth_stencil, blend, elements) =
        unsafe { (&*raster, &*depth_stencil, &*blend, &*elements) };
    let topology = topology(info.mode);
    let triangles = matches!(
        topology,
        Topology::TriangleList | Topology::TriangleStrip | Topology::TriangleFan
    );
    if raster.cull_both && triangles {
        return Ok(());
    }
    let shaders = Stage::ALL.map(|s| context.stages.get(s).shader);
    let [vs, fs] = shaders;
    if vs.is_null() || fs.is_null() {
        return Err("a draw without a vertex or fragment program".into());
    }
    let (viewport, mut transform) = viewport(&bound.viewport, raster.minus_one);
    let screen = context.screen();
    let mut backend = screen.backend();
    let backend = &mut *backend;
    transform.fixed_point_size = !backend.caps().has(Features::POINT_SIZE);
    if transform.half_z {
        backend.slow_paths().hit(SlowPath::ClipDepthRewrite);
    }
    if transform.reverse_z {
        backend.slow_paths().hit(SlowPath::ViewportDepthFlip);
    }
    let fragment = Variant {
        swizzles: swizzles(context.stages.get(Stage::Fragment)),
        ..Variant::default()
    };
    let programs = &mut context.programs;
    // The fragment program first: the vertex program's outputs are interpolated as its inputs.
    // SAFETY: a bound shader lives until Mesa unbinds and deletes it.
    let inputs = programs
        .built_for(backend, unsafe { &*fs }, &fragment)?
        .inputs
        .clone();
    let vertex = Variant {
        transform,
        swizzles: swizzles(context.stages.get(Stage::Vertex)),
        interpolation: inputs,
    };
    // SAFETY: as above.
    programs.built_for(backend, unsafe { &*vs }, &vertex)?;
    if !vertex.swizzles.is_empty() || !fragment.swizzles.is_empty() {
        backend.slow_paths().hit(SlowPath::SwizzleInShader);
    }
    let built = [
        &programs.built[&(vs as ShaderId, vertex.clone())],
        &programs.built[&(fs as ShaderId, fragment.clone())],
    ];
    if raster.provoking != raster.host_provoking && built[1].flat_inputs {
        return Err(format!(
            "flat values from the {:?} vertex, which the back end lacks",
            raster.provoking
        ));
    }
    let program_key = (vs as ShaderId, vertex, fs as ShaderId, fragment);
    if let Entry::Vacant(slot) = programs.programs.entry(program_key.clone()) {
        slot.insert(
            backend
                .create_program(&Program::Render {
                    vertex: &built[0].shader,
                    fragment: Some(&built[1].shader),
                })
                .map_err(|e| format!("a program: {e:?}"))?,
        );
    }
    let sets = binding_sets(backend, &context.stages, built)?;
    let program = programs.programs[&program_key].clone();
    let (blend, alpha_constant) =
        blend_for(programs, backend, &blend.desc, &context.framebuffer.state);
    let bound = &context.bound;
    let [.., alpha] = bound.blend_color;
    let blend_constant = if alpha_constant {
        [alpha; 4]
    } else {
        bound.blend_color
    };
    let mut streams = Vec::new();
    for i in 0..elements.streams as usize {
        let stream = bound
            .vertex_buffers
            .get(i)
            .filter(|s| !s.resource.is_null());
        streams.push(stream.and_then(|s| {
            // SAFETY: a bound stream holds a reference on a live buffer.
            let resource = unsafe { Resource::<B>::of(s.resource) };
            Some(VertexBinding {
                buffer: resource.buffer()?,
                offset: u64::from(s.offset),
            })
        }));
    }
    let fan = matches!(topology, Topology::TriangleFan | Topology::LineLoop);
    // A fan or loop drawn from an index buffer: its indices read back now (`ReadBackWait`),
    // then converted as the program's own.
    let read = if fan && info.index_size > 0 && !info.has_user_indices() {
        Some(read_indices(&mut context.tracker, backend, info, draws)?)
    } else {
        None
    };
    let mut scissors = Vec::new();
    let area = context
        .tracker
        .before_draw(backend, &[])
        .map_err(|e| format!("{e:?}"))?;
    let s = &bound.scissor;
    let scissor = ScissorRect {
        x: s.minx,
        y: s.miny,
        width: s.maxx.saturating_sub(s.minx),
        height: s.maxy.saturating_sub(s.miny),
    };
    match (raster.scissor, area) {
        (true, Some(area)) => cut_scissors(&[scissor], area, &mut scissors),
        (true, None) => scissors.push(scissor),
        (false, Some(area)) => scissors.push(area),
        (false, None) => {}
    }
    let index_size = info.index_size;
    let format = index_format(index_size);
    let restart = info.primitive_restart();
    if restart && index_size > 0 && info.restart_index != format.restart_value() {
        return Err("primitive restart at an index that is not the largest".into());
    }
    let passes = face_passes(backend, depth_stencil, raster, triangles, bound.stencil_ref)?;
    let base = DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil.block,
        raster: &raster.block,
        vertex_layout: &elements.block,
        bindings: [
            sets[0].as_ref(),
            sets[1].as_ref(),
            sets[2].as_ref(),
            sets[3].as_ref(),
        ],
        vertex_buffers: &streams,
        index: None,
        topology,
        restart,
        viewports: std::slice::from_ref(&viewport),
        scissors: &scissors,
        blend_constant,
        stencil_reference: bound.stencil_ref,
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 0,
            first_instance: 0,
            instances: 0,
        },
    };
    for (depth_stencil, block, stencil_reference) in passes {
        let pass = DrawCall {
            depth_stencil,
            raster: block,
            stencil_reference,
            index: None,
            range: DrawRange::Vertices {
                first: 0,
                count: 0,
                first_instance: 0,
                instances: 0,
            },
            ..base
        };
        draw_ranges(
            backend,
            &pass,
            info,
            draws,
            read.as_ref(),
            raster.host_provoking,
            fan,
        )?;
    }
    for set in sets.into_iter().flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    Ok(())
}

/// One depth, stencil and rasterizer block and stencil reference a draw is made with.
type FacePass<'a, B> = (
    &'a <B as Backend>::DepthStencil,
    &'a <B as Backend>::Raster,
    [u8; 2],
);

/// The blocks a draw is made with: its own, or, where the stencil test's faces differ in masks or
/// reference and the back end has one of each per draw, each face alone with its own
/// (`StencilFacesSplit`). Points and lines are front faces, as OpenGL draws them.
fn face_passes<'a, B: Backend>(
    backend: &mut B,
    depth_stencil: &'a state::DepthStencil<B>,
    raster: &'a state::Raster<B>,
    triangles: bool,
    reference: [u8; 2],
) -> Result<Vec<FacePass<'a, B>>, String> {
    let split = depth_stencil.stencil
        && (depth_stencil.faces.is_some() || reference[0] != reference[1])
        && !backend.caps().has(Features::STENCIL_PER_FACE);
    if !split {
        return Ok(vec![(&depth_stencil.block, &raster.block, reference)]);
    }
    let face = |i: usize, block: &'a B::Raster| {
        let blocks = depth_stencil.faces.as_ref();
        (
            blocks.map_or(&depth_stencil.block, |f| &f[i]),
            block,
            [reference[i]; 2],
        )
    };
    const FRONT: usize = 0;
    const BACK: usize = 1;
    Ok(match (triangles, raster.cull) {
        (false, _) | (true, CullMode::Back) => vec![face(FRONT, &raster.block)],
        (true, CullMode::Front) => vec![face(BACK, &raster.block)],
        (true, CullMode::None) => {
            let [front, back] = raster
                .faces
                .as_ref()
                .ok_or("a rasterizer state without its blocks a face")?;
            backend.slow_paths().hit(SlowPath::StencilFacesSplit);
            vec![face(FRONT, front), face(BACK, back)]
        }
    })
}

/// The indices of every range of a draw from an index buffer, read back, and the first one's
/// position in the buffer.
fn read_indices<B: Backend>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
) -> Result<(Vec<u8>, u32), String> {
    let bytes = u64::from(info.index_size);
    let first = draws.iter().map(|d| d.start).min().unwrap_or(0);
    let end = draws
        .iter()
        .map(|d| u64::from(d.start) + u64::from(d.count))
        .max()
        .unwrap_or(0);
    // SAFETY: an index buffer of this driver, held by st/mesa for the draw.
    let resource = unsafe { Resource::<B>::of(info.index.resource) };
    let buffer = resource.buffer().ok_or("indices in a texture")?;
    tracker.close(backend).map_err(|e| format!("{e:?}"))?;
    // Read backs move whole words: the words around the indices, then the indices from them.
    let (start, stop) = (u64::from(first) * bytes, end * bytes);
    let from = start - start % STORAGE_WORD;
    let to = stop.next_multiple_of(STORAGE_WORD);
    let mut words = vec![0; (to - from) as usize];
    backend
        .read_back_now(
            &ReadSource::Buffer {
                buffer,
                offset: from,
                size: to - from,
            },
            &mut words,
        )
        .map_err(|e| format!("{e:?}"))?;
    words.truncate((stop - from) as usize);
    words.drain(..(start - from) as usize);
    Ok((words, first))
}

/// Each range of one draw call made with `base`'s blocks; `read` holds indices read back from
/// the index buffer, and the first one's position in it.
fn draw_ranges<B: Backend>(
    backend: &mut B,
    base: &DrawCall<'_, B>,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
    read: Option<&(Vec<u8>, u32)>,
    host_provoking: ProvokingVertex,
    fan: bool,
) -> Result<(), String> {
    let topology = base.topology;
    let restart = base.restart;
    let index_size = info.index_size;
    let user_indices = info.has_user_indices();
    let format = index_format(index_size);
    for d in draws {
        if d.count == 0 {
            continue;
        }
        if index_size == 0 {
            let vertices = DrawCall {
                index: None,
                range: DrawRange::Vertices {
                    first: d.start,
                    count: d.count,
                    first_instance: info.start_instance,
                    instances: info.instance_count,
                },
                ..*base
            };
            if fan {
                let source = IndexSource::Sequential {
                    first: d.start,
                    count: d.count,
                };
                backend
                    .draw_fan_or_loop(
                        &vertices,
                        fan_or_loop(topology),
                        source,
                        host_provoking,
                        info.start_instance,
                        info.instance_count,
                    )
                    .map_err(|e| format!("{e:?}"))?;
            } else {
                backend.draw(&vertices);
            }
            continue;
        }
        let bytes = u64::from(index_size);
        let range = || DrawRange::Indexed {
            first: 0,
            count: d.count,
            base_vertex: d.index_bias,
            first_instance: info.start_instance,
            instances: info.instance_count,
        };
        let at = |first: u32| (u64::from(d.start - first) * bytes) as usize;
        let length = (u64::from(d.count) * bytes) as usize;
        let host = match read {
            Some((indices, first)) => Some(&indices[at(*first)..at(*first) + length]),
            // SAFETY: Mesa's indices cover the draw's range.
            None if user_indices => Some(unsafe {
                std::slice::from_raw_parts(info.index.user.cast::<u8>().add(at(0)), length)
            }),
            None => None,
        };
        if let Some(indices) = host {
            if fan {
                let source = IndexSource::Guest {
                    indices,
                    format,
                    base_vertex: d.index_bias,
                };
                backend
                    .draw_fan_or_loop(
                        base,
                        fan_or_loop(topology),
                        source,
                        host_provoking,
                        info.start_instance,
                        info.instance_count,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                continue;
            }
            let widened;
            let (indices, format) = if needs_widening(backend, format) {
                backend.slow_paths().hit(SlowPath::IndexWidened);
                widened = widen(indices, restart);
                (widened.as_slice(), IndexFormat::U16)
            } else {
                (indices, format)
            };
            let mut offset = [0];
            let ring = backend
                .write_buffer_ring(RingKind::Indices, &[indices], format.size(), &mut offset)
                .map_err(|e| format!("{e:?}"))?;
            backend.draw(&DrawCall {
                index: Some(IndexBinding {
                    buffer: &ring,
                    offset: offset[0],
                    format,
                }),
                range: range(),
                ..*base
            });
            continue;
        }
        if needs_widening(backend, format) {
            return Err("8-bit indices in a buffer on a back end without them".into());
        }
        // SAFETY: an index buffer of this driver, held by st/mesa for the draw.
        let resource = unsafe { Resource::<B>::of(info.index.resource) };
        let buffer = resource.buffer().ok_or("indices in a texture")?;
        backend.draw(&DrawCall {
            index: Some(IndexBinding {
                buffer,
                offset: u64::from(d.start) * bytes,
                format,
            }),
            range: range(),
            ..*base
        });
    }
    Ok(())
}

/// The swizzles of a stage's bound views that rearrange their results.
fn swizzles<B: Backend>(stage: &state::StageState<B>) -> Vec<TextureSwizzle> {
    stage
        .views
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_null())
        .filter_map(|(unit, &v)| {
            // SAFETY: the slot holds a reference on a live view of this driver.
            let swizzle = unsafe { View::<B>::of(v) }.swizzle;
            (swizzle != IDENTITY).then_some(TextureSwizzle {
                unit: unit as u32,
                swizzle,
            })
        })
        .collect()
}

fn fan_or_loop(topology: Topology) -> FanOrLoop {
    if topology == Topology::LineLoop {
        FanOrLoop::LineLoop
    } else {
        FanOrLoop::TriangleFan
    }
}

/// Whether the back end lacks indices of this width.
fn needs_widening<B: Backend>(backend: &B, format: IndexFormat) -> bool {
    format == IndexFormat::U8 && !backend.caps().has(Features::INDEX_U8)
}

/// 8-bit indices as 16-bit ones, restart values kept as restart values.
fn widen(indices: &[u8], restart: bool) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|&i| {
            let wide = if restart && u32::from(i) == IndexFormat::U8.restart_value() {
                u16::MAX
            } else {
                u16::from(i)
            };
            wide.to_le_bytes()
        })
        .collect()
}
