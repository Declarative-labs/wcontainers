//! Draws: each stage's program converted by the NIR route for the state it is drawn with, its
//! resources bound by the route's reflection, and the draw handed to the pass tracker's pass.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::c_uint;

use wcontainers_gpu::{
    AddedBinding, Backend, BindingEntry, BindingResource, BlendDesc, BlendFactor, BufferRange,
    CullMode, DrawCall, DrawRange, FanOrLoop, Features, FormatKind, FormatUse, HELPER_SET,
    HelperSlot, HelperValue, IndexBinding, IndexFormat, IndexSource, MAX_BINDING_SETS, Object,
    PassTracker, Program, ProvokingVertex, ReadSource, ReflectedBinding, Reflection, RingKind,
    STORAGE_WORD, ScissorRect, ShaderModule, ShaderStage, SlotKind, SlowPath, SourceStage,
    StageExtras, StageSet, StepMode, Topology, VariantKey, VertexBinding, VertexFormat, Viewport,
    cut_scissors, helper_binding,
};
use wcontainers_nir::mesa::{NearestSampler, PositionTransform, TextureSwizzle};
use wcontainers_shaders::Converted;
use wcontainers_shaders::stage::{InputTopology, StageDraw, Stages};

use crate::context::Context;
use crate::gallium::*;
use crate::resource::Resource;
use crate::screen::Screen;
use crate::state::{self, Shader, Stage, refuse, refused};
use crate::view::{IDENTITY, View};
use crate::{indices, outputs, query, stream};

/// One stage's program converted for one variant, and what it binds.
pub(crate) struct Built<B: Backend> {
    pub(crate) shader: B::Shader,
    bindings: Vec<ReflectedBinding>,
    /// The bytes each uniform block declares, by its binding in the converted program.
    uniform_sizes: HashMap<u32, u64>,
    /// The program reads values interpolated flat, taken from the provoking vertex.
    flat_inputs: bool,
    /// How a fragment program interpolates each input, by location.
    inputs: Vec<Interpolated>,
    /// The helper constants the program reads, and their block's bytes.
    helpers: Vec<HelperSlot>,
    helper_size: u32,
    /// The storage binding of each stream the program fetches itself, by its input's location.
    pulled: Vec<(u32, u32)>,
    /// A vertex program converted for the stage helper: its module, with its outputs.
    converted: Option<Converted>,
}

/// A shader state as a map key: the address Mesa holds it at.
pub(crate) type ShaderId = usize;

/// What a program is converted for beside its NIR: the position's transform (vertex programs) and
/// the swizzles of the views bound where it samples.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Variant {
    transform: PositionTransform,
    swizzles: Vec<TextureSwizzle>,
    /// How the fragment program it meets interpolates each input (vertex programs).
    interpolation: Vec<Interpolated>,
    /// The inputs the program fetches from their streams itself, by location, and their formats.
    pulled: Vec<(u32, VertexFormat)>,
    /// A vertex program the stage helper runs, every input fetched in it.
    streamed: bool,
    /// The kinds of the framebuffer's targets (fragment programs).
    targets: outputs::Targets,
    /// The samplers of the integer textures bound where it samples.
    nearest: Vec<NearestSampler>,
    /// The blend constant's alpha given as the blend's second source (fragment programs).
    second_source: Option<u32>,
}

impl Variant {
    /// The variant of a program that only samples what `bound` binds (a compute program), and
    /// whether it rearranges what it samples.
    pub(crate) fn sampling<B: Backend>(bound: &state::StageState<B>) -> (Variant, bool) {
        let variant = Variant {
            swizzles: swizzles(bound),
            nearest: nearest_samplers(bound),
            ..Variant::default()
        };
        let swizzled = !variant.swizzles.is_empty();
        (variant, swizzled)
    }
}

/// A value passed between stages at one location, and how it is interpolated.
type Interpolated = (u32, Option<naga::Interpolation>, Option<naga::Sampling>);

impl<B: Backend> Programs<B> {
    /// `shader` converted for `variant`, converted now if it was not before.
    pub(crate) fn built_for(
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
    pub(crate) compute: HashMap<(ShaderId, Variant), B::Program>,
    /// The stage helpers of vertex programs streamed, by program and variant.
    streamed: HashMap<(ShaderId, Variant), Stages<B>>,
}

impl<B: Backend> Default for Programs<B> {
    fn default() -> Self {
        Programs {
            built: HashMap::new(),
            programs: HashMap::new(),
            blends: HashMap::new(),
            compute: HashMap::new(),
            streamed: HashMap::new(),
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
        let compute: Vec<_> = self.compute.keys().filter(|k| k.0 == id).cloned().collect();
        for key in compute {
            if let Some(p) = self.compute.remove(&key) {
                screen.retire(Object::Program(p));
            }
        }
        let streamed: Vec<_> = self
            .streamed
            .keys()
            .filter(|k| k.0 == id)
            .cloned()
            .collect();
        for key in streamed {
            if let Some(stages) = self.streamed.remove(&key) {
                stages.destroy(&mut screen.backend());
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
        for (_, p) in self.compute.drain() {
            screen.retire(Object::Program(p));
        }
        for (_, b) in self.blends.drain() {
            screen.retire(Object::Blend(b));
        }
        for (_, stages) in self.streamed.drain() {
            stages.destroy(&mut screen.backend());
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
    if !variant.nearest.is_empty() {
        nir.nearest_integer_textures(&variant.nearest);
    }
    if !variant.swizzles.is_empty() {
        nir.swizzle_textures(&variant.swizzles);
    }
    let key = nir.key().map_err(|e| e.to_string())?;
    let stage = shader.stage.nir();
    let bindings = nir.to_common_form(stage).map_err(|e| e.to_string())?;
    let (module, bindings, helpers, helper_size) = if variant.pulled.is_empty() {
        let mut module = wcontainers_nir::direct::module(&mut nir).map_err(|e| e.to_string())?;
        if !variant.interpolation.is_empty() {
            meet_inputs(&mut module, &variant.interpolation);
        }
        if !variant.targets.is_empty() {
            outputs::fit(&mut module, &variant.targets);
        }
        if let Some(alpha) = variant.second_source {
            outputs::second_source(&mut module, alpha)?;
        }
        let info = wcontainers_nir::spirv::validate(&module).map_err(|e| e.to_string())?;
        let entry_point = module
            .entry_points
            .first()
            .map(|e| e.name.clone())
            .ok_or("no entry point")?;
        (
            ShaderModule {
                module,
                info,
                entry_point,
                stage: stage.shader_stage(),
                key,
            },
            bindings,
            Vec::new(),
            0,
        )
    } else {
        let mut routed = wcontainers_nir::direct::route(&mut nir).map_err(|e| e.to_string())?;
        if !variant.interpolation.is_empty() {
            meet_inputs(&mut routed.module, &variant.interpolation);
        }
        let rewrites = VariantKey {
            pulled: variant.pulled.clone(),
            ..VariantKey::default()
        };
        let (module, reflection) =
            wcontainers_nir::direct::rewrite(routed, bindings, &rewrites, &key)
                .map_err(|e| e.to_string())?;
        (
            module,
            reflection.bindings,
            reflection.helpers,
            reflection.helper_size,
        )
    };
    let converted = variant.streamed.then(|| Converted {
        modules: vec![module.clone()],
        reflection: Reflection {
            stage: SourceStage::Vertex,
            inputs: Vec::new(),
            outputs: stream::outputs(&module.module),
            bindings: bindings.clone(),
            helpers: helpers.clone(),
            helper_size,
            sampled_pairs: Vec::new(),
            extras: StageExtras::None,
            slow_paths: Vec::new(),
        },
    });
    let uniform_sizes = uniform_sizes(&module.module);
    let flat_inputs = flat_inputs(&module.module);
    let inputs = input_interpolation(&module.module);
    let pulled = bindings
        .iter()
        .filter_map(|b| match b.added {
            Some(AddedBinding::PulledStream(location)) => Some((b.binding, location)),
            _ => None,
        })
        .collect();
    let shader = backend
        .create_shader(&module)
        .map_err(|e| format!("{e:?}"))?;
    Ok(Built {
        shader,
        bindings,
        uniform_sizes,
        flat_inputs,
        inputs,
        helpers,
        helper_size,
        pulled,
        converted,
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
        _ => refuse(format_args!("a primitive the screen does not report")),
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
    // OpenGL clamps its depth range to the host's; only Mesa's blitter maps -1..1 to itself, for
    // depths it gives at zero or above, which the zero-to-one reading draws alike.
    let host_range = 0.0..=1.0;
    let minus_one = minus_one && host_range.contains(&(tz - sz)) && host_range.contains(&(tz + sz));
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

/// The colour factors of a blend block, source and destination, and its alpha factors.
type Factors<'a> = (Vec<&'a mut BlendFactor>, Vec<&'a mut BlendFactor>);

fn factors(desc: &mut BlendDesc) -> Factors<'_> {
    let mut colour = Vec::new();
    let mut alpha = Vec::new();
    for (c, a) in desc.targets.iter_mut().filter_map(|t| t.blend.as_mut()) {
        colour.extend([&mut c.src, &mut c.dst]);
        alpha.extend([&mut a.src, &mut a.dst]);
    }
    (colour, alpha)
}

/// The constant factor reading what `factor` reads where the constant's colour is its alpha.
fn constant_of_alpha(factor: BlendFactor) -> BlendFactor {
    match factor {
        BlendFactor::ConstantAlpha => BlendFactor::Constant,
        BlendFactor::OneMinusConstantAlpha => BlendFactor::OneMinusConstant,
        other => other,
    }
}

/// The second source's factor reading what `factor` reads where the second source holds the
/// constant's alpha in every channel.
fn second_source_of_alpha(factor: BlendFactor) -> BlendFactor {
    match factor {
        BlendFactor::ConstantAlpha => BlendFactor::Src1Alpha,
        BlendFactor::OneMinusConstantAlpha => BlendFactor::OneMinusSrc1Alpha,
        other => other,
    }
}

/// The blend constant's alpha the fragment program gives as the blend's second source: where the
/// block's colour factors read the constant's colour and its alpha, for a constant whose colour is
/// not its alpha, on a back end with no constant alpha factors but a second source, drawing into
/// one target (`BlendInShader`).
fn second_source_alpha<B: Backend>(
    backend: &B,
    desc: &BlendDesc,
    framebuffer: &pipe_framebuffer_state,
    constant: [u32; state::COLOR_CHANNELS],
) -> Option<u32> {
    let caps = backend.caps();
    let [.., alpha] = constant;
    let colour: Vec<BlendFactor> = desc
        .targets
        .iter()
        .filter_map(|t| t.blend)
        .flat_map(|(c, _)| [c.src, c.dst])
        .collect();
    let reads = |wanted: [BlendFactor; 2]| colour.iter().any(|f| wanted.contains(f));
    let needed = !caps.has(Features::BLEND_CONSTANT_ALPHA)
        && reads([
            BlendFactor::ConstantAlpha,
            BlendFactor::OneMinusConstantAlpha,
        ])
        && reads([BlendFactor::Constant, BlendFactor::OneMinusConstant])
        && constant != [alpha; state::COLOR_CHANNELS];
    let one_target = framebuffer.nr_cbufs == 1;
    (needed && one_target && caps.has(Features::DUAL_SOURCE_BLEND)).then_some(alpha)
}

/// The blend block of `desc` for the framebuffer's formats (no blending on targets that cannot
/// blend: integer targets, as OpenGL defines), and whether the draw's blend constant is its alpha
/// in every channel. On a back end without constant alpha factors, an alpha factor reading the
/// constant's alpha is the constant factor (exact); a colour factor reading it is too where the
/// constant is given its alpha in every channel, which a block whose colour factors also read the
/// constant's colour allows only for a constant whose colour is its alpha.
fn blend_for<B: Backend>(
    programs: &mut Programs<B>,
    backend: &mut B,
    desc: &BlendDesc,
    framebuffer: &pipe_framebuffer_state,
    constant: [u32; state::COLOR_CHANNELS],
    second_source: bool,
) -> (B::Blend, bool) {
    let mut desc = *desc;
    let caps = backend.caps().clone();
    let mut alpha_constant = false;
    if !caps.has(Features::BLEND_CONSTANT_ALPHA) {
        let (colour, alpha) = factors(&mut desc);
        for f in alpha {
            *f = constant_of_alpha(*f);
        }
        let reads = |wanted: [BlendFactor; 2]| colour.iter().any(|f| wanted.contains(f));
        let reads_alpha = reads([
            BlendFactor::ConstantAlpha,
            BlendFactor::OneMinusConstantAlpha,
        ]);
        let reads_colour = reads([BlendFactor::Constant, BlendFactor::OneMinusConstant]);
        if reads_alpha && second_source {
            for f in colour {
                *f = second_source_of_alpha(*f);
            }
        } else if reads_alpha {
            let [.., a] = constant;
            if reads_colour && constant != [a; state::COLOR_CHANNELS] {
                refuse(format_args!(
                    "the blend constant's colour and its alpha as colour factors of one block, for a constant whose colour is not its alpha, where no second source can give the alpha (more than one target, or none on the back end)"
                ));
            }
            for f in colour {
                *f = constant_of_alpha(*f);
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
        Err(error) => refuse(format!("a blend state: {error:?}")),
    }
}

/// Where a uniform block's bytes come from: the next copy written to the ring, or a range of a
/// buffer.
enum Uniform<'a, B: Backend> {
    Copied,
    Range(&'a B::Buffer, u64, u64),
}

/// A stream the vertex program fetches an input from itself.
pub(crate) struct Pulled<'a, B: Backend> {
    location: u32,
    buffer: &'a B::Buffer,
    /// The bytes of the buffer that hold data.
    buffer_bytes: u64,
    /// The stream's offset in the buffer, and the input's in each element.
    stream_offset: u64,
    attribute_offset: u32,
    stride: u32,
    /// How many instances read one element; zero for a stream read per vertex.
    divisor: u32,
}

/// The helper constants of `built` for a draw from `first_instance`.
fn helper_block<B: Backend>(
    built: &Built<B>,
    pulled: &[Pulled<'_, B>],
    storage_align: u64,
    first_instance: u32,
) -> Result<Vec<u8>, String> {
    let mut block = vec![0; built.helper_size as usize];
    for slot in &built.helpers {
        let words = match slot.value {
            HelperValue::PulledStream { input } => {
                let p = pulled
                    .iter()
                    .find(|p| p.location == input)
                    .ok_or("a fetched input without its stream")?;
                let offset = (p.stream_offset % storage_align) as u32 + p.attribute_offset;
                let bias = p.divisor.saturating_sub(1).wrapping_mul(first_instance);
                [offset, p.stride, p.divisor, bias]
            }
            other => return Err(format!("the helper constant {other:?}")),
        };
        let at = slot.offset as usize;
        for (i, word) in words.iter().enumerate() {
            let start = at + i * size_of::<u32>();
            block[start..start + size_of::<u32>()].copy_from_slice(&word.to_le_bytes());
        }
    }
    Ok(block)
}

/// Binds every resource the programs read, one set a stage, as the route's reflection numbers
/// them, with the streams the vertex program fetches itself and its helper constants; the vertex
/// program's visible to `vertex_visible`. The views made for images go to `made`, to destroy
/// after the work that binds them.
pub(crate) fn binding_sets<B: Backend>(
    backend: &mut B,
    stages: &state::Stages<B>,
    built: &[(Stage, &Built<B>)],
    pulled: &[Pulled<'_, B>],
    first_instance: u32,
    vertex_visible: StageSet,
    made: &mut Vec<B::TextureView>,
) -> Result<[Option<B::BindingSet>; MAX_BINDING_SETS], String> {
    let mut sets = [const { None }; MAX_BINDING_SETS];
    let limits = &backend.caps().limits;
    let align = limits.min_uniform_offset_alignment;
    let storage_align = u64::from(limits.min_storage_offset_alignment);
    let largest_storage = limits.max_storage_buffer_size;
    let mut helpers = None;
    for &(stage, b) in built {
        match (stage, b.helper_size) {
            (_, 0) => {}
            (Stage::Vertex, _) => {
                helpers = Some(helper_block(b, pulled, storage_align, first_instance)?);
            }
            (Stage::Fragment | Stage::Compute, _) => {
                return refused(format_args!("helper constants in a {stage:?} program"));
            }
        }
    }
    for &(stage, built) in built {
        let bound = stages.get(stage);
        let visible = match stage {
            Stage::Vertex => vertex_visible,
            Stage::Fragment => StageSet::FRAGMENT,
            Stage::Compute => StageSet::COMPUTE,
        };
        let images = image_views(backend, bound, built)?;
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
                return refused(format_args!(
                    "a uniform block bound smaller than the program declares: binding {}, \
                     {declared} bytes declared, {} bound at {} of a buffer of {}",
                    b.binding, slot.size, slot.offset, resource.base.width0
                ));
            }
            uniforms.push((
                b.binding,
                Uniform::Range(buffer, u64::from(slot.offset), size),
            ));
        }
        let mut parts: Vec<&[u8]> = copies.iter().map(Vec::as_slice).collect();
        let helpers = helpers.as_deref().filter(|_| stage == Stage::Vertex);
        parts.extend(helpers);
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
        let mut helper_entry = None;
        if let Some(bytes) = helpers {
            let ring = ring.as_ref().ok_or("the ring")?;
            helper_entry = Some(BindingEntry {
                binding: helper_binding(ShaderStage::Vertex),
                stages: visible,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: ring,
                    offset: offsets[copies.len()],
                    size: bytes.len() as u64,
                }),
            });
        }
        for &(binding, location) in &built.pulled {
            let p = pulled
                .iter()
                .find(|p| p.location == location)
                .ok_or("a fetched input without its stream")?;
            let start = p.stream_offset - p.stream_offset % storage_align;
            if start >= p.buffer_bytes {
                return refused(format_args!(
                    "a fetched vertex stream past its buffer's end"
                ));
            }
            let size = (p.buffer_bytes - start).next_multiple_of(STORAGE_WORD);
            entries.push(BindingEntry {
                binding,
                stages: visible,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: p.buffer,
                        offset: start,
                        size: size.min(largest_storage),
                    },
                    writable: false,
                },
            });
        }
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
        let nearest: Vec<u32> = nearest_samplers(bound).iter().map(|n| n.unit).collect();
        for b in built.bindings.iter().filter(|b| b.added.is_none()) {
            let resource = match b.kind {
                SlotKind::Uniform => continue,
                SlotKind::Texture => {
                    let view = bound.views[b.register as usize];
                    if view.is_null() {
                        return refused(format_args!("a texture the program reads is not bound"));
                    }
                    // SAFETY: the slot holds a reference on a live view of this driver.
                    let view = unsafe { View::<B>::of(view) };
                    // An integer cube map the program samples is loaded from its faces.
                    match view.faces() {
                        Some(faces) if nearest.contains(&b.register) => {
                            BindingResource::Texture(faces?)
                        }
                        _ => BindingResource::Texture(view.view()?),
                    }
                }
                SlotKind::Sampler => {
                    let sampler = bound.samplers[b.register as usize];
                    if sampler.is_null() {
                        return refused(format_args!("a sampler the program reads is not bound"));
                    }
                    // SAFETY: a bound sampler lives until Mesa unbinds and deletes it.
                    BindingResource::Sampler(unsafe { &(*sampler).sampler })
                }
                SlotKind::Storage if b.register >= PIPE_MAX_SHADER_IMAGES => {
                    storage_buffer(bound, b.register - PIPE_MAX_SHADER_IMAGES, largest_storage)?
                }
                SlotKind::Storage => {
                    let (_, view, writable) = images
                        .iter()
                        .find(|(unit, ..)| *unit == b.register)
                        .ok_or("an image the program reads is not bound")?;
                    BindingResource::StorageTexture {
                        view,
                        writable: *writable,
                    }
                }
                SlotKind::Counter => {
                    return refused(format_args!("a storage counter in an OpenGL program"));
                }
            };
            entries.push(BindingEntry {
                binding: b.binding,
                stages: visible,
                resource,
            });
        }
        if let Some(entry) = helper_entry {
            sets[HELPER_SET as usize] = Some(
                backend
                    .create_binding_set(std::slice::from_ref(&entry))
                    .map_err(|e| format!("{e:?}"))?,
            );
        }
        if !entries.is_empty() {
            let set = wcontainers_nir::bind::stage_set(stage.nir().shader_stage()) as usize;
            sets[set] = Some(
                backend
                    .create_binding_set(&entries)
                    .map_err(|e| format!("{e:?}"))?,
            );
        }
        drop(entries);
        made.extend(images.into_iter().map(|(_, view, _)| view));
    }
    Ok(sets)
}

/// The storage buffer bound at `index` of a stage, as the interface binds it: its range rounded
/// to whole words, within the buffer and the back end's largest binding.
fn storage_buffer<'a, B: Backend>(
    bound: &'a state::StageState<B>,
    index: u32,
    largest: u64,
) -> Result<BindingResource<'a, B>, String> {
    let slot = bound
        .buffers
        .get(index as usize)
        .filter(|slot| !slot.resource.is_null())
        .ok_or("a storage buffer the program reads is not bound")?;
    // SAFETY: the slot holds a reference on a live resource.
    let resource = unsafe { Resource::<B>::of(slot.resource) };
    let buffer = resource
        .buffer()
        .ok_or("a storage buffer bound to a texture")?;
    let bytes = u64::from(resource.base.width0).next_multiple_of(STORAGE_WORD);
    let offset = u64::from(slot.offset);
    let size = u64::from(slot.size).next_multiple_of(STORAGE_WORD);
    Ok(BindingResource::StorageBuffer {
        range: BufferRange {
            buffer,
            offset,
            size: size.min(bytes.saturating_sub(offset)).min(largest),
        },
        writable: slot.writable,
    })
}

/// A view of each image the program binds, by its unit, with whether the program may write it.
fn image_views<B: Backend>(
    backend: &mut B,
    bound: &state::StageState<B>,
    built: &Built<B>,
) -> Result<Vec<(u32, B::TextureView, bool)>, String> {
    let mut views = Vec::new();
    for b in built.bindings.iter().filter(|b| {
        b.added.is_none() && b.kind == SlotKind::Storage && b.register < PIPE_MAX_SHADER_IMAGES
    }) {
        let image = bound
            .images
            .get(b.register as usize)
            .filter(|image| !image.resource.is_null())
            .ok_or("an image the program reads is not bound")?;
        let view = crate::view::image_view(backend, image)?;
        let writable = u32::from(image.shader_access) & PIPE_IMAGE_ACCESS_WRITE != 0;
        views.push((b.register, view, writable));
    }
    Ok(views)
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
    // SAFETY: Mesa passes this driver's context and the draw.
    let (context, info) = unsafe { (Context::<B>::of(pipe), &*info) };
    let result = if indirect.is_null() {
        // SAFETY: a direct draw comes with `count` ranges.
        draw(context, info, unsafe {
            std::slice::from_raw_parts(draws, count as usize)
        })
    } else {
        // SAFETY: an indirect draw's arguments.
        draw_indirect(context, info, unsafe { &*indirect })
    };
    if let Err(reason) = result {
        refuse(&reason);
    }
}

/// The words of one indirect draw's arguments: count, instances, first, the base vertex of an
/// indexed draw, and the first instance.
const INDIRECT_WORDS: usize = 4;
const INDEXED_INDIRECT_WORDS: usize = 5;

/// An indirect draw, its arguments read back now (`IndirectReadBack`) and drawn as a direct draw,
/// so that every rewrite a direct draw takes applies to it.
fn draw_indirect<B: Backend>(
    context: &mut Context<B>,
    info: &pipe_draw_info,
    indirect: &pipe_draw_indirect_info,
) -> Result<(), String> {
    if !indirect.count_from_stream_output.is_null() || !indirect.indirect_draw_count.is_null() {
        return refused(format_args!(
            "a draw counted by stream output or by a buffer (the screen reports neither)"
        ));
    }
    let words = if info.index_size > 0 {
        INDEXED_INDIRECT_WORDS
    } else {
        INDIRECT_WORDS
    };
    let word = size_of::<u32>();
    let stride = (indirect.stride as usize).max(words * word);
    let draws = indirect.draw_count as usize;
    let span = (draws.saturating_sub(1) * stride + words * word) as u64;
    // SAFETY: an indirect draw names a live buffer of this driver.
    let resource = unsafe { Resource::<B>::of(indirect.buffer) };
    let buffer = resource.buffer().ok_or("a draw's arguments in a texture")?;
    let offset = u64::from(indirect.offset);
    let mut args = vec![0; span.next_multiple_of(STORAGE_WORD) as usize];
    let screen = context.screen();
    {
        let mut backend = screen.backend();
        backend.slow_paths().hit(SlowPath::IndirectReadBack);
        context
            .tracker
            .close(&mut backend)
            .map_err(|e| format!("{e:?}"))?;
        backend
            .read_back_now(
                &ReadSource::Buffer {
                    buffer,
                    offset,
                    size: args.len() as u64,
                },
                &mut args,
            )
            .map_err(|e| format!("a draw's arguments: {e:?}"))?;
    }
    for d in 0..draws {
        let at = |i: usize| {
            let start = d * stride + i * word;
            u32::from_le_bytes(args[start..start + word].try_into().expect("a word"))
        };
        let (start, index_bias, first_instance) = if info.index_size > 0 {
            (at(2), at(3) as i32, at(4))
        } else {
            (at(2), 0, at(3))
        };
        let direct = pipe_draw_info {
            instance_count: at(1),
            start_instance: first_instance,
            ..*info
        };
        let range = pipe_draw_start_count_bias {
            start,
            count: at(0),
            index_bias,
        };
        if direct.instance_count > 0 {
            draw(context, &direct, std::slice::from_ref(&range))?;
        }
    }
    Ok(())
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
        return refused(format_args!(
            "a draw without rasterizer, depth, blend or vertex state"
        ));
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
    let [vs, fs] = [Stage::Vertex, Stage::Fragment].map(|s| context.stages.get(s).shader);
    if vs.is_null() || fs.is_null() {
        return refused(format_args!("a draw without a vertex or fragment program"));
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
        targets: target_kinds(&context.framebuffer.state),
        nearest: nearest_samplers(context.stages.get(Stage::Fragment)),
        second_source: second_source_alpha(
            backend,
            &blend.desc,
            &context.framebuffer.state,
            bound.blend_color,
        ),
        ..Variant::default()
    };
    if fragment.second_source.is_some() {
        backend.slow_paths().hit(SlowPath::BlendInShader);
    }
    let programs = &mut context.programs;
    // The fragment program first: the vertex program's outputs are interpolated as its inputs.
    // SAFETY: a bound shader lives until Mesa unbinds and deletes it.
    let inputs = programs
        .built_for(backend, unsafe { &*fs }, &fragment)?
        .inputs
        .clone();
    let (pulled, paths) = pulled_inputs(elements, &bound.vertex_buffers);
    for path in paths {
        backend.slow_paths().hit(path);
    }
    let vertex = Variant {
        transform,
        swizzles: swizzles(context.stages.get(Stage::Vertex)),
        nearest: nearest_samplers(context.stages.get(Stage::Vertex)),
        interpolation: inputs,
        pulled: pulled
            .iter()
            .filter_map(|&l| Some((l, elements.format(l)?)))
            .collect(),
        streamed: false,
        targets: Vec::new(),
        second_source: None,
    };
    // SAFETY: as above.
    programs.built_for(backend, unsafe { &*vs }, &vertex)?;
    if !vertex.swizzles.is_empty() || !fragment.swizzles.is_empty() {
        backend.slow_paths().hit(SlowPath::SwizzleInShader);
    }
    if context.streams.active() {
        // SAFETY: as above.
        stream_out(
            context,
            backend,
            unsafe { &*vs },
            &vertex,
            elements,
            info,
            draws,
        )?;
    }
    if raster.discard {
        return Ok(());
    }
    let bound = &context.bound;
    let programs = &mut context.programs;
    let built = [
        &programs.built[&(vs as ShaderId, vertex.clone())],
        &programs.built[&(fs as ShaderId, fragment.clone())],
    ];
    // Flat values from the vertex OpenGL names where the host takes the other: indices rewritten.
    let provoking = (raster.provoking != raster.host_provoking
        && built[1].flat_inputs
        && topology != Topology::PointList)
        .then_some(Provoking {
            api: raster.provoking,
            host: raster.host_provoking,
        });
    let fragment_second_source = fragment.second_source.is_some();
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
    let fetched = fetched_streams(elements, &context.bound.vertex_buffers, &pulled)?;
    let mut images = Vec::new();
    let sets = binding_sets(
        backend,
        &context.stages,
        &[(Stage::Vertex, built[0]), (Stage::Fragment, built[1])],
        &fetched,
        info.start_instance,
        StageSet::VERTEX,
        &mut images,
    )?;
    let pulled_mask = pulled.iter().fold(0, |mask, l| mask | 1 << l);
    let layout = elements.block(backend, pulled_mask)?;
    let program = programs.programs[&program_key].clone();
    let (blend, alpha_constant) = blend_for(
        programs,
        backend,
        &blend.desc,
        &context.framebuffer.state,
        bound.blend_color,
        fragment_second_source,
    );
    let bound = &context.bound;
    let [.., alpha] = bound.blend_color;
    let blend_constant = if alpha_constant {
        [alpha; state::COLOR_CHANNELS]
    } else {
        bound.blend_color
    };
    let mut streams = Vec::new();
    for i in 0..elements.streams as usize {
        let read = elements
            .desc
            .attributes
            .iter()
            .flatten()
            .any(|a| usize::from(a.stream) == i && pulled_mask & (1 << a.location) == 0);
        let stream = bound
            .vertex_buffers
            .get(i)
            .filter(|s| read && !s.resource.is_null());
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
    let mode = mesa_prim::from(info.mode);
    let restart = info.primitive_restart();
    // Indices in a buffer the host cannot draw as they are (a fan or loop, flat values from the
    // other vertex, restart in a list, 8-bit indices the back end lacks): read back now
    // (`ReadBackWait`), then converted as the program's own.
    let rewritten_on_host = fan
        || provoking.is_some()
        || indices::restarts_otherwise(mode, restart)
        || needs_widening(backend, index_format(info.index_size));
    let read = if rewritten_on_host && info.index_size > 0 && !info.has_user_indices() {
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
    if restart && index_size > 0 && info.restart_index != format.restart_value() {
        return refused(format_args!(
            "primitive restart at an index that is not the largest"
        ));
    }
    let passes = face_passes(backend, depth_stencil, raster, triangles, bound.stencil_ref)?;
    let base = DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil.block,
        raster: &raster.block,
        vertex_layout: &layout,
        bindings: std::array::from_fn(|set| sets[set].as_ref()),
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
            provoking,
        )?;
    }
    for set in sets.into_iter().flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    for view in images {
        backend.destroy(Object::TextureView(view));
    }
    Ok(())
}

/// The streams of the inputs at `pulled`, which the vertex program fetches itself.
fn fetched_streams<'a, B: Backend + 'a>(
    elements: &state::Elements<B>,
    streams: &'a [state::VertexBuffer],
    pulled: &[u32],
) -> Result<Vec<Pulled<'a, B>>, String> {
    let mut fetched = Vec::new();
    for &location in pulled {
        let stream = elements
            .stream(location)
            .ok_or("a fetched input without a stream")?;
        let attribute = elements
            .desc
            .attributes
            .iter()
            .flatten()
            .find(|a| a.location == location)
            .ok_or("a fetched input without its element")?;
        let layout = elements.desc.streams[stream].ok_or("an element of no stream")?;
        let bound_stream = streams
            .get(stream)
            .filter(|s| !s.resource.is_null())
            .ok_or("a fetched vertex stream that is not bound")?;
        // SAFETY: a bound stream holds a reference on a live buffer.
        let resource = unsafe { Resource::<B>::of(bound_stream.resource) };
        let component = elements
            .component_bytes
            .iter()
            .find(|&&(l, _)| l == location)
            .map_or(1, |&(_, bytes)| bytes);
        let starts = [bound_stream.offset, attribute.offset, layout.stride];
        if starts.iter().any(|s| !s.is_multiple_of(component)) {
            return refused(format_args!(
                "a fetched vertex component across words (the shaders crate's fetch reads aligned components)"
            ));
        }
        fetched.push(Pulled {
            location,
            buffer: resource.buffer().ok_or("a vertex stream in a texture")?,
            buffer_bytes: u64::from(resource.base.width0),
            stream_offset: u64::from(bound_stream.offset),
            attribute_offset: attribute.offset,
            stride: layout.stride,
            divisor: match layout.step {
                StepMode::Vertex => 0,
                StepMode::Instance(n) => n,
            },
        });
    }
    Ok(fetched)
}

/// The stage helper's topology of a list or strip.
fn input_topology(topology: Topology) -> Option<InputTopology> {
    Some(match topology {
        Topology::PointList => InputTopology::PointList,
        Topology::LineList => InputTopology::LineList,
        Topology::LineStrip => InputTopology::LineStrip,
        Topology::TriangleList => InputTopology::TriangleList,
        Topology::TriangleStrip => InputTopology::TriangleStrip,
        _ => return None,
    })
}

/// The vertex program's outputs streamed to the bound targets for each range of a draw, by the
/// stage helper (`StreamOutAsCompute`), before the draw itself.
fn stream_out<B: Backend>(
    context: &mut Context<B>,
    backend: &mut B,
    vs: &Shader,
    vertex: &Variant,
    elements: &state::Elements<B>,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
) -> Result<(), String> {
    let Some(declared) = vs.streamed.clone()? else {
        return Ok(());
    };
    if info.index_size > 0 {
        return refused(format_args!(
            "stream output of an indexed draw (OpenGL ES 3.0 has none)"
        ));
    }
    let topology = input_topology(topology(info.mode)).ok_or("stream output of a fan or loop")?;
    let mut locations: Vec<u32> = elements
        .desc
        .attributes
        .iter()
        .flatten()
        .map(|a| a.location)
        .collect();
    locations.sort_unstable();
    let variant = Variant {
        pulled: locations
            .iter()
            .filter_map(|&l| Some((l, elements.format(l)?)))
            .collect(),
        interpolation: Vec::new(),
        streamed: true,
        // The program's own position is streamed, before the host's depth range and flips.
        transform: PositionTransform {
            fixed_point_size: vertex.transform.fixed_point_size,
            ..PositionTransform::default()
        },
        ..vertex.clone()
    };
    let programs = &mut context.programs;
    let key = (std::ptr::from_ref(vs) as ShaderId, variant.clone());
    let made = programs.streamed.contains_key(&key);
    let built = programs.built_for(backend, vs, &variant)?;
    let fetched = fetched_streams(elements, &context.bound.vertex_buffers, &locations)?;
    let mut images = Vec::new();
    let sets = binding_sets(
        backend,
        &context.stages,
        &[(Stage::Vertex, built)],
        &fetched,
        info.start_instance,
        StageSet::VERTEX.union(StageSet::COMPUTE),
        &mut images,
    )?;
    if !made {
        let converted = built
            .converted
            .clone()
            .ok_or("a streamed program without its module")?;
        let stages = Stages::new(backend, &converted, None, Some(&declared))
            .map_err(|e| format!("stream output: {e:?}"))?;
        programs.streamed.insert(key.clone(), stages);
    }
    let stages = programs.streamed.get_mut(&key).ok_or("the stage helper")?;
    context
        .tracker
        .close(backend)
        .map_err(|e| format!("{e:?}"))?;
    let fragment_set = wcontainers_nir::bind::stage_set(ShaderStage::Fragment) as usize;
    let vertex_sets = std::array::from_fn(|set| sets[set].as_ref().filter(|_| set != fragment_set));
    // A draw of no whole primitive records nothing, its statistics included.
    let whole =
        |d: &&pipe_draw_start_count_bias| indices::trimmed(info.mode as mesa_prim, d.count) > 0;
    for d in draws.iter().filter(whole) {
        let draw = StageDraw {
            vertex_sets,
            geometry_sets: [None; MAX_BINDING_SETS],
            index: None,
            topology,
            range: DrawRange::Vertices {
                first: d.start,
                count: d.count,
                first_instance: info.start_instance,
                instances: info.instance_count,
            },
            stream_output: Some(context.streams.targets()?),
        };
        stages
            .run(backend, &draw)
            .map_err(|e| format!("stream output: {e:?}"))?;
        let statistics = context
            .streams
            .filled
            .as_ref()
            .ok_or("stream output's block")?;
        query::count_emitted(context.counting, backend, statistics)?;
    }
    for set in sets.into_iter().flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    for view in images {
        backend.destroy(Object::TextureView(view));
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
        // Without the stencil test the references read nothing; the back end takes one.
        let reference = if depth_stencil.stencil {
            reference
        } else {
            [reference[0]; 2]
        };
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

/// OpenGL's provoking vertex and the host's, where a draw's flat values need them swapped.
#[derive(Clone, Copy)]
struct Provoking {
    api: ProvokingVertex,
    host: ProvokingVertex,
}

/// Whether `indices` hold the largest value of their format, at which the host's strips restart.
fn holds_largest(indices: &[u8], format: IndexFormat) -> bool {
    let largest = format.restart_value();
    match format {
        IndexFormat::U8 => indices.iter().any(|&i| u32::from(i) == largest),
        IndexFormat::U16 => indices
            .as_chunks::<{ size_of::<u16>() }>()
            .0
            .iter()
            .any(|i| u32::from(u16::from_le_bytes(*i)) == largest),
        IndexFormat::U32 => indices
            .as_chunks::<{ size_of::<u32>() }>()
            .0
            .iter()
            .any(|i| u32::from_le_bytes(*i) == largest),
    }
}

/// Draws `rewritten`'s indices, from the ring, with `base`'s blocks.
fn draw_rewritten<B: Backend>(
    backend: &mut B,
    base: &DrawCall<'_, B>,
    rewritten: &indices::Rewritten,
    base_vertex: i32,
    info: &pipe_draw_info,
) -> Result<(), String> {
    let mut offset = [0];
    let ring = backend
        .write_buffer_ring(
            RingKind::Indices,
            &[rewritten.indices()],
            rewritten.format.size(),
            &mut offset,
        )
        .map_err(|e| format!("{e:?}"))?;
    backend.draw(&DrawCall {
        topology: rewritten.topology,
        restart: false,
        index: Some(IndexBinding {
            buffer: &ring,
            offset: offset[0],
            format: rewritten.format,
        }),
        range: DrawRange::Indexed {
            first: 0,
            count: rewritten.count(),
            base_vertex,
            first_instance: info.start_instance,
            instances: info.instance_count,
        },
        ..*base
    });
    Ok(())
}

/// Each range of one draw call made with `base`'s blocks; `read` holds indices read back from
/// the index buffer, and the first one's position in it. Where `provoking` is given, every
/// primitive's vertices are reordered for the host's provoking vertex.
fn draw_ranges<B: Backend>(
    backend: &mut B,
    base: &DrawCall<'_, B>,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
    read: Option<&(Vec<u8>, u32)>,
    host_provoking: ProvokingVertex,
    provoking: Option<Provoking>,
) -> Result<(), String> {
    let topology = base.topology;
    let fan = matches!(topology, Topology::TriangleFan | Topology::LineLoop);
    let restart = base.restart;
    let index_size = info.index_size;
    let user_indices = info.has_user_indices();
    let format = index_format(index_size);
    let mode = mesa_prim::from(info.mode);
    let strip = matches!(topology, Topology::LineStrip | Topology::TriangleStrip);
    let restart_switch = backend.caps().has(Features::RESTART_SWITCH);
    for d in draws {
        if d.count == 0 {
            continue;
        }
        if let (0, Some(p)) = (index_size, provoking) {
            backend.slow_paths().hit(SlowPath::ProvokingRewrite);
            if let Some(r) = indices::generated(mode, d.start, d.count, p.api, p.host)? {
                draw_rewritten(backend, base, &r, 0, info)?;
            }
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
            // Restart cut where the host would not, or made where OpenGL does not: lists never
            // restart there, and its strips always do at the largest value.
            // A rewritten draw is cut at every restart value: its primitives may become lists.
            let cut = indices::restarts_otherwise(mode, restart);
            let lists_only = !restart && strip && !restart_switch && holds_largest(indices, format);
            if provoking.is_some() || cut || lists_only {
                let paths = [
                    (provoking.is_some(), SlowPath::ProvokingRewrite),
                    (cut || lists_only, SlowPath::RestartRewrite),
                    (needs_widening(backend, format), SlowPath::IndexWidened),
                ];
                for (_, path) in paths.into_iter().filter(|&(taken, _)| taken) {
                    backend.slow_paths().hit(path);
                }
                let Provoking { api, host } = provoking.unwrap_or(Provoking {
                    api: host_provoking,
                    host: host_provoking,
                });
                let rewritten = indices::translated(info, indices, restart, lists_only, api, host)?;
                if let Some(r) = rewritten {
                    draw_rewritten(backend, base, &r, d.index_bias, info)?;
                }
                continue;
            }
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
            return refused(format_args!(
                "indices of one byte in a buffer on a back end without them"
            ));
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

/// The inputs the vertex program fetches from their streams itself, by location, and why: those
/// the elements state named, and those of streams bound at offsets the back end cannot read from.
fn pulled_inputs<B: Backend>(
    elements: &state::Elements<B>,
    streams: &[state::VertexBuffer],
) -> (Vec<u32>, Vec<SlowPath>) {
    let mut pulled = Vec::new();
    let mut paths = Vec::new();
    let note = |path: SlowPath, paths: &mut Vec<SlowPath>| {
        if !paths.contains(&path) {
            paths.push(path);
        }
    };
    for &(location, path) in &elements.pulled {
        pulled.push(location);
        note(path, &mut paths);
    }
    for a in elements.desc.attributes.iter().flatten() {
        let unaligned = streams
            .get(usize::from(a.stream))
            .is_some_and(|s| !s.offset.is_multiple_of(state::VERTEX_ALIGNMENT));
        if unaligned && !pulled.contains(&a.location) {
            pulled.push(a.location);
            note(SlowPath::VertexPulledInShader, &mut paths);
        }
    }
    pulled.sort_unstable();
    (pulled, paths)
}

/// The samplers bound with a stage's integer textures, by unit.
fn nearest_samplers<B: Backend>(stage: &state::StageState<B>) -> Vec<NearestSampler> {
    stage
        .views
        .iter()
        .zip(&stage.samplers)
        .enumerate()
        .filter(|(_, (v, s))| !v.is_null() && !s.is_null())
        .filter_map(|(unit, (&v, &s))| {
            // SAFETY: the slots hold a reference on a live view of this driver and a bound
            // sampler, which lives until Mesa unbinds and deletes it.
            let (view, sampler) = unsafe { (View::<B>::of(v), &*s) };
            let nearest = sampler.nearest.filter(|_| view.integer)?;
            Some(NearestSampler {
                unit: unit as u32,
                ..nearest
            })
        })
        .collect()
}

/// The kind of each bound colour target, by location.
fn target_kinds(framebuffer: &pipe_framebuffer_state) -> outputs::Targets {
    framebuffer
        .cbufs
        .iter()
        .take(usize::from(framebuffer.nr_cbufs))
        .enumerate()
        .filter_map(|(i, s)| {
            let kind = match crate::format::format(s.format())?.kind() {
                FormatKind::Uint => naga::ScalarKind::Uint,
                FormatKind::Sint => naga::ScalarKind::Sint,
                _ => naga::ScalarKind::Float,
            };
            Some((i as u32, kind))
        })
        .collect()
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
