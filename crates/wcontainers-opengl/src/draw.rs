//! Draws: each stage's program converted by the NIR route for the state it is drawn with, its
//! resources bound by the route's reflection, and the draw handed to the pass tracker's pass.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::c_uint;

use wcontainers_gpu::{
    AddedBinding, Backend, BindingEntry, BindingResource, BlendDesc, BlendFactor, BufferDesc,
    BufferRange, BufferUsage, CullMode, DrawCall, DrawRange, FanOrLoop, Features, FormatKind,
    FormatUse, HELPER_SET, HelperSlot, HelperValue, IndexBinding, IndexFormat, IndexSource,
    Interpolation, MAX_BINDING_SETS, MapIntent, Object, PassTracker, Program, ProvokingVertex,
    RasterDesc, ReadSource, ReflectedBinding, Reflection, RingKind, STORAGE_WORD, ScissorRect,
    ShaderModule, ShaderStage, SlotKind, SlowPath, SourceStage, StageExtras, StageSet, StepMode,
    Topology, VariantKey, VertexBinding, VertexFormat, Viewport, WriteMode, cut_scissors,
    helper_binding,
};
use wcontainers_nir::mesa::{NearestSampler, PositionTransform, TextureSwizzle};
use wcontainers_shaders::Converted;
use wcontainers_shaders::stage::{
    FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages,
};

use crate::context::Context;
use crate::gallium::*;
use crate::packed::Packed;
use crate::resource::Resource;
use crate::screen::Screen;
use crate::state::{self, Shader, Stage, refuse, refused};
use crate::view::{IDENTITY, Images, View};
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
    /// The bindings of the textures the program reads as 2D arrays (an integer cube map it
    /// samples is one of its faces).
    layered: Vec<u32>,
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
    /// The units whose border colour the program applies, a bit each (`BorderInShader`).
    border: u16,
    /// The blend constant's alpha given as the blend's second source (fragment programs).
    second_source: Option<u32>,
    /// The inputs of packed formats the interface lacks, by location, which the program unpacks
    /// from one word each (vertex programs).
    packed: Vec<(u32, Packed)>,
}

impl Variant {
    /// The variant of a program that only samples what `bound` binds (a compute program), and
    /// whether it rearranges what it samples.
    pub(crate) fn sampling<B: Backend>(bound: &state::StageState<B>) -> (Variant, bool) {
        let variant = Variant {
            swizzles: swizzles(bound),
            nearest: nearest_samplers(bound),
            border: border_in_program(bound),
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
    /// Rasterizer blocks of a bound state with a sample mask of fewer than all samples.
    masked: HashMap<RasterDesc, B::Raster>,
    pub(crate) compute: HashMap<(ShaderId, Variant), B::Program>,
    /// The stage helpers of vertex programs and the geometry programs after them (none: zero),
    /// by both programs and variants.
    staged: HashMap<(ShaderId, Variant, ShaderId, Variant), Staged<B>>,
}

/// A vertex program and the geometry program after it run by the stage helper, and the vertex
/// layout a geometry program's output is drawn with.
struct Staged<B: Backend> {
    stages: Stages<B>,
    layout: Option<B::VertexLayout>,
    /// Indices made on the host, and the buffer's bytes, grown as draws need.
    indices: Option<(B::Buffer, u64)>,
}

impl<B: Backend> Staged<B> {
    fn destroy(self, backend: &mut B) {
        self.stages.destroy(backend);
        if let Some(layout) = self.layout {
            backend.destroy(Object::VertexLayout(layout));
        }
        if let Some((buffer, _)) = self.indices {
            backend.destroy(Object::Buffer(buffer));
        }
    }

    /// Writes `bytes` at the start of the host's index buffer, grown to hold them; earlier work
    /// still reads the bytes it was recorded with.
    fn write_indices(&mut self, backend: &mut B, bytes: &[u8]) -> Result<(), String> {
        let size = (bytes.len() as u64).next_multiple_of(STORAGE_WORD);
        if self.indices.as_ref().is_none_or(|&(_, held)| held < size) {
            if let Some((old, _)) = self.indices.take() {
                backend.destroy(Object::Buffer(old));
            }
            let buffer = backend
                .create_buffer(&BufferDesc {
                    size,
                    usage: BufferUsage::INDEX.union(BufferUsage::STORAGE),
                    intent: MapIntent::Often,
                })
                .map_err(|e| format!("{e:?}"))?;
            self.indices = Some((buffer, size));
        }
        let (buffer, _) = self.indices.as_ref().ok_or("the helper's indices")?;
        let mut words = bytes.to_vec();
        words.resize(size as usize, 0);
        backend.write_buffer(buffer, 0, &words, WriteMode::Discard);
        Ok(())
    }
}

impl<B: Backend> Default for Programs<B> {
    fn default() -> Self {
        Programs {
            built: HashMap::new(),
            programs: HashMap::new(),
            blends: HashMap::new(),
            masked: HashMap::new(),
            compute: HashMap::new(),
            staged: HashMap::new(),
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
        let staged: Vec<_> = self
            .staged
            .keys()
            .filter(|k| k.0 == id || k.2 == id)
            .cloned()
            .collect();
        for key in staged {
            if let Some(staged) = self.staged.remove(&key) {
                staged.destroy(&mut screen.backend());
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
        for (_, r) in self.masked.drain() {
            screen.retire(Object::Raster(r));
        }
        for (_, staged) in self.staged.drain() {
            staged.destroy(&mut screen.backend());
        }
    }
}

/// The bytes of each uniform block a converted program declares in its stage's `set`, by binding.
fn uniform_sizes(module: &naga::Module, set: u32) -> HashMap<u32, u64> {
    let mut layouter = naga::proc::Layouter::default();
    if layouter.update(module.to_ctx()).is_err() {
        return HashMap::new();
    }
    module
        .global_variables
        .iter()
        .filter(|(_, g)| g.space == naga::AddressSpace::Uniform)
        .filter_map(|(_, g)| {
            let binding = g.binding.as_ref().filter(|b| b.group == set)?;
            Some((binding.binding, u64::from(layouter[g.ty].size)))
        })
        .collect()
}

/// How the fragment program interpolates each input, as the interface names it, where not as
/// perspective at the pixel's centre.
fn output_interpolation(inputs: &[Interpolated]) -> Vec<(u32, Interpolation)> {
    use naga::Interpolation as I;
    use naga::Sampling as S;
    inputs
        .iter()
        .filter_map(|&(location, interpolation, sampling)| {
            let linear = interpolation == Some(I::Linear);
            let named = match (interpolation, sampling) {
                (Some(I::Flat), _) => Interpolation::Flat,
                (_, Some(S::Centroid)) if linear => Interpolation::LinearCentroid,
                (_, Some(S::Sample)) if linear => Interpolation::LinearSample,
                _ if linear => Interpolation::Linear,
                (_, Some(S::Centroid)) => Interpolation::PerspectiveCentroid,
                (_, Some(S::Sample)) => Interpolation::PerspectiveSample,
                _ => return None,
            };
            Some((location, named))
        })
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

/// The units of `units` (a bit each) as the slots of `kind` the program binds them at, a bit each,
/// as the variant names them.
fn slots_of(bindings: &[ReflectedBinding], kind: SlotKind, units: u16) -> Result<u16, String> {
    let mut slots = 0;
    for b in bindings
        .iter()
        .filter(|b| b.kind == kind && b.added.is_none())
    {
        for i in 0..b.count {
            let unit = b.register + i;
            if unit >= u16::BITS || units & (1 << unit) == 0 {
                continue;
            }
            let slot = b.slot() + i;
            if slot >= u16::BITS {
                return refused(format_args!("a border colour at a slot past the variant's"));
            }
            slots |= 1 << slot;
        }
    }
    Ok(slots)
}

/// The unit bound at a slot of `kind` the program binds.
fn unit_of(bindings: &[ReflectedBinding], kind: SlotKind, slot: u32) -> Option<u32> {
    bindings
        .iter()
        .filter(|b| b.kind == kind && b.added.is_none())
        .find(|b| (b.slot()..b.slot() + b.count).contains(&slot))
        .map(|b| b.register + slot - b.slot())
}

/// A program's outputs and inputs fitted to the stages and targets it meets.
fn fit_stages(module: &mut naga::Module, variant: &Variant) -> Result<(), String> {
    if !variant.interpolation.is_empty() {
        meet_inputs(module, &variant.interpolation);
    }
    if !variant.targets.is_empty() {
        outputs::fit(module, &variant.targets);
    }
    if let Some(alpha) = variant.second_source {
        outputs::second_source(module, alpha)?;
    }
    Ok(())
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
    let set = wcontainers_nir::bind::stage_set(stage.shader_stage());
    let bindings = nir.to_common_form(stage).map_err(|e| e.to_string())?;
    if shader.stage == Stage::Geometry {
        return convert_geometry(backend, &mut nir, bindings, variant, &key);
    }
    let rewrites = VariantKey {
        pulled: variant.pulled.clone(),
        border_in_shader: slots_of(&bindings, SlotKind::Sampler, variant.border)?,
        ..VariantKey::default()
    };
    let mut routed = wcontainers_nir::direct::route(&mut nir).map_err(|e| e.to_string())?;
    fit_stages(&mut routed.module, variant)?;
    crate::packed::unpack_inputs(&mut routed.module, &variant.packed)?;
    let (module, bindings, helpers, helper_size) =
        if rewrites == VariantKey::default() && routed.helpers.is_empty() {
            let module = wcontainers_nir::direct::plain(routed).map_err(|e| e.to_string())?;
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
    let uniform_sizes = uniform_sizes(&module.module, set);
    let layered = layered_textures(&module.module);
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
        layered,
        converted,
    })
}

/// A geometry program through route B in the stage helper's forms: its compute form, which each
/// draw's stage helper is made from, and the pass-through vertex program the draw of its output
/// runs, made here. Its outputs are written as the fragment program interpolates them.
fn convert_geometry<B: Backend>(
    backend: &mut B,
    nir: &mut wcontainers_nir::mesa::Nir,
    bindings: Vec<ReflectedBinding>,
    variant: &Variant,
    key: &[u8; wcontainers_shaders::cache::KEY_BYTES],
) -> Result<Built<B>, String> {
    let rewrites = VariantKey {
        border_in_shader: slots_of(&bindings, SlotKind::Sampler, variant.border)?,
        output_interpolation: output_interpolation(&variant.interpolation),
        ..VariantKey::default()
    };
    let routed = wcontainers_nir::direct::route(nir).map_err(|e| e.to_string())?;
    let mut converted = wcontainers_nir::direct::geometry(routed, bindings, &rewrites, key)
        .map_err(|e| e.to_string())?;
    stream::name_outputs(&mut converted.reflection.outputs);
    let [compute, passthrough] = converted.modules.as_slice() else {
        return refused(format_args!(
            "a geometry program's forms other than compute work and its pass-through"
        ));
    };
    let shader = backend
        .create_shader(passthrough)
        .map_err(|e| format!("{e:?}"))?;
    let set = wcontainers_nir::bind::stage_set(ShaderStage::Compute);
    Ok(Built {
        shader,
        bindings: converted.reflection.bindings.clone(),
        uniform_sizes: uniform_sizes(&compute.module, set),
        flat_inputs: false,
        inputs: Vec::new(),
        helpers: converted.reflection.helpers.clone(),
        helper_size: converted.reflection.helper_size,
        pulled: Vec::new(),
        layered: layered_textures(&compute.module),
        converted: Some(converted),
    })
}

/// The bindings of a program's sampled 2D array textures.
fn layered_textures(module: &naga::Module) -> Vec<u32> {
    module
        .global_variables
        .iter()
        .filter(|(_, g)| {
            matches!(
                module.types[g.ty].inner,
                naga::TypeInner::Image {
                    dim: naga::ImageDimension::D2,
                    arrayed: true,
                    class: naga::ImageClass::Sampled { .. },
                }
            )
        })
        .filter_map(|(_, g)| Some(g.binding.as_ref()?.binding))
        .collect()
}

/// The interface's topology of a Gallium primitive; none for a primitive with adjacency, which
/// only a geometry program takes, or one the screen does not report.
fn topology(mode: mesa_prim) -> Option<Topology> {
    Some(match mode {
        MESA_PRIM_POINTS => Topology::PointList,
        MESA_PRIM_LINES => Topology::LineList,
        MESA_PRIM_LINE_STRIP => Topology::LineStrip,
        MESA_PRIM_LINE_LOOP => Topology::LineLoop,
        MESA_PRIM_TRIANGLES => Topology::TriangleList,
        MESA_PRIM_TRIANGLE_STRIP => Topology::TriangleStrip,
        MESA_PRIM_TRIANGLE_FAN => Topology::TriangleFan,
        _ => return None,
    })
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
    // A block blending with the program's own second source leaves none to give the alpha.
    let own_second = desc
        .targets
        .iter()
        .filter_map(|t| t.blend)
        .flat_map(|(c, a)| [c.src, c.dst, a.src, a.dst])
        .any(|f| {
            matches!(
                f,
                BlendFactor::Src1
                    | BlendFactor::OneMinusSrc1
                    | BlendFactor::Src1Alpha
                    | BlendFactor::OneMinusSrc1Alpha
            )
        });
    (needed && one_target && !own_second && caps.has(Features::DUAL_SOURCE_BLEND)).then_some(alpha)
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

/// The helper constants of `built` for a draw from `first_instance`, of the stage `bound`.
fn helper_block<B: Backend>(
    built: &Built<B>,
    bound: &state::StageState<B>,
    pulled: &[Pulled<'_, B>],
    storage_align: u64,
    first_instance: u32,
) -> Result<Vec<u8>, String> {
    let mut block = vec![0; built.helper_size as usize];
    let sampler = |slot: u32| -> Result<&state::Sampler<B>, String> {
        let unit =
            unit_of(&built.bindings, SlotKind::Sampler, slot).ok_or("a sampler slot unbound")?;
        let sampler = bound
            .samplers
            .get(unit as usize)
            .filter(|s| !s.is_null())
            .ok_or("a border colour without its sampler")?;
        // SAFETY: a bound sampler lives until Mesa unbinds and deletes it.
        Ok(unsafe { &**sampler })
    };
    for slot in &built.helpers {
        let words: Vec<u32> = match slot.value {
            HelperValue::PulledStream { input } => {
                let p = pulled
                    .iter()
                    .find(|p| p.location == input)
                    .ok_or("a fetched input without its stream")?;
                let offset = (p.stream_offset % storage_align) as u32 + p.attribute_offset;
                let bias = p.divisor.saturating_sub(1).wrapping_mul(first_instance);
                vec![offset, p.stride, p.divisor, bias]
            }
            HelperValue::BorderColor { sampler: slot } => sampler(slot)?.border_color.to_vec(),
            HelperValue::SamplerState { sampler: slot } => {
                let d = sampler(slot)?
                    .border_in_program
                    .ok_or("a sampler's state for a sampler the host addresses")?;
                [d.min, d.mag, d.mip]
                    .map(|f| f as u32)
                    .into_iter()
                    .chain(d.address.map(|a| a as u32))
                    .chain([u32::from(d.anisotropy)])
                    .chain(d.lod_range)
                    .collect()
            }
            HelperValue::TextureComponents { texture: slot } => {
                let unit = unit_of(&built.bindings, SlotKind::Texture, slot)
                    .ok_or("a texture slot unbound")?;
                let view = bound
                    .views
                    .get(unit as usize)
                    .filter(|v| !v.is_null())
                    .ok_or("a border colour without its texture")?;
                // SAFETY: the slot holds a reference on a live view of this driver.
                vec![unsafe { View::<B>::of(*view) }.components]
            }
            HelperValue::TexelBufferView { set, binding } => {
                let unit = built
                    .bindings
                    .iter()
                    .find(|b| b.set == set && b.binding == binding)
                    .map(|b| b.register)
                    .ok_or("a texel buffer's slot unbound")?;
                let view = bound
                    .views
                    .get(unit as usize)
                    .filter(|v| !v.is_null())
                    .ok_or("a texel buffer the program reads is not bound")?;
                // SAFETY: the slot holds a reference on a live view of this driver.
                let texels = unsafe { View::<B>::of(*view) }
                    .texels
                    .as_ref()
                    .ok_or("a texture bound where the program reads a texel buffer")?;
                // The view starts the texture it is copied into.
                vec![0, texels.length]
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
/// program's visible to `vertex_visible`. What images are bound through goes to `made`, to
/// finish after the work that binds them (outside a pass: an image may be copied at once).
pub(crate) fn binding_sets<B: Backend>(
    backend: &mut B,
    stages: &state::Stages<B>,
    built: &[(Stage, &Built<B>)],
    pulled: &[Pulled<'_, B>],
    first_instance: u32,
    vertex_visible: StageSet,
    made: &mut Images<B>,
) -> Result<[Option<B::BindingSet>; MAX_BINDING_SETS], String> {
    let mut sets = [const { None }; MAX_BINDING_SETS];
    let limits = &backend.caps().limits;
    let align = limits.min_uniform_offset_alignment;
    let storage_align = u64::from(limits.min_storage_offset_alignment);
    let largest_storage = limits.max_storage_buffer_size;
    let visibility = |stage| match stage {
        Stage::Vertex => vertex_visible,
        Stage::Fragment => StageSet::FRAGMENT,
        Stage::Compute | Stage::Geometry => StageSet::COMPUTE,
    };
    // Every stage's helper constants, in the one helper set.
    let mut blocks = Vec::new();
    for &(stage, b) in built.iter().filter(|(_, b)| b.helper_size != 0) {
        let bytes = helper_block(b, stages.get(stage), pulled, storage_align, first_instance)?;
        blocks.push((stage, bytes));
    }
    if !blocks.is_empty() {
        let parts: Vec<&[u8]> = blocks.iter().map(|(_, bytes)| bytes.as_slice()).collect();
        let mut offsets = vec![0; parts.len()];
        let ring = backend
            .write_buffer_ring(RingKind::Data, &parts, align, &mut offsets)
            .map_err(|e| format!("{e:?}"))?;
        let entries: Vec<_> = blocks
            .iter()
            .zip(offsets)
            .map(|((stage, bytes), offset)| BindingEntry {
                binding: helper_binding(stage.nir().shader_stage()),
                stages: visibility(*stage),
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset,
                    size: bytes.len() as u64,
                }),
            })
            .collect();
        sets[HELPER_SET as usize] = Some(
            backend
                .create_binding_set(&entries)
                .map_err(|e| format!("{e:?}"))?,
        );
    }
    for &(stage, built) in built {
        let bound = stages.get(stage);
        let visible = visibility(stage);
        let images = image_views(backend, bound, built, made)?;
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
                    view.copy_texels(backend)?;
                    // An integer cube map the program samples is loaded from its faces.
                    match view.faces() {
                        Some(faces) if built.layered.contains(&b.binding) => {
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
                    let &(_, index, writable) = images
                        .iter()
                        .find(|(unit, ..)| *unit == b.register)
                        .ok_or("an image the program reads is not bound")?;
                    BindingResource::StorageTexture {
                        view: &made.views[index],
                        writable,
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
        if !entries.is_empty() {
            let set = wcontainers_nir::bind::stage_set(stage.nir().shader_stage()) as usize;
            sets[set] = Some(
                backend
                    .create_binding_set(&entries)
                    .map_err(|e| format!("{e:?}"))?,
            );
        }
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

/// A view in `made` of each image the program binds, by its unit, with whether the program may
/// write it.
fn image_views<B: Backend>(
    backend: &mut B,
    bound: &state::StageState<B>,
    built: &Built<B>,
    made: &mut Images<B>,
) -> Result<Vec<(u32, usize, bool)>, String> {
    let mut views = Vec::new();
    for b in built.bindings.iter().filter(|b| {
        b.added.is_none() && b.kind == SlotKind::Storage && b.register < PIPE_MAX_SHADER_IMAGES
    }) {
        let image = bound
            .images
            .get(b.register as usize)
            .filter(|image| !image.resource.is_null())
            .ok_or("an image the program reads is not bound")?;
        let index = crate::view::image_view(backend, image, made)?;
        let writable = u32::from(image.shader_access) & PIPE_IMAGE_ACCESS_WRITE != 0;
        views.push((b.register, index, writable));
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
    if !query::condition_holds(context) {
        return;
    }
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

/// One indirect draw's arguments as OpenGL lays them out in a buffer (`DrawArraysIndirectCommand`
/// and `DrawElementsIndirectCommand`), for their sizes.
#[repr(C)]
struct DrawArraysArguments {
    count: u32,
    instances: u32,
    first: u32,
    first_instance: u32,
}

#[repr(C)]
struct DrawElementsArguments {
    count: u32,
    instances: u32,
    first: u32,
    base_vertex: i32,
    first_instance: u32,
}

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
    let size = if info.index_size > 0 {
        size_of::<DrawElementsArguments>()
    } else {
        size_of::<DrawArraysArguments>()
    };
    let stride = (indirect.stride as usize).max(size);
    let draws = indirect.draw_count as usize;
    let span = (draws.saturating_sub(1) * stride + size) as u64;
    // SAFETY: an indirect draw names a live buffer of this driver.
    let resource = unsafe { Resource::<B>::of(indirect.buffer) };
    let buffer = resource.buffer().ok_or("a draw's arguments in a texture")?;
    let offset = u64::from(indirect.offset);
    let mut bytes = vec![0; span.next_multiple_of(STORAGE_WORD) as usize];
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
                    size: bytes.len() as u64,
                },
                &mut bytes,
            )
            .map_err(|e| format!("a draw's arguments: {e:?}"))?;
    }
    for d in 0..draws {
        let words: Vec<u32> = bytes[d * stride..d * stride + size]
            .as_chunks::<{ size_of::<u32>() }>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        let arguments = match *words.as_slice() {
            [count, instances, first, base_vertex, first_instance] => DrawElementsArguments {
                count,
                instances,
                first,
                base_vertex: base_vertex as i32,
                first_instance,
            },
            [count, instances, first, first_instance] => DrawElementsArguments {
                count,
                instances,
                first,
                base_vertex: 0,
                first_instance,
            },
            _ => return refused(format_args!("indirect arguments of another size")),
        };
        let direct = pipe_draw_info {
            instance_count: arguments.instances,
            start_instance: arguments.first_instance,
            ..*info
        };
        let range = pipe_draw_start_count_bias {
            start: arguments.first,
            count: arguments.count,
            index_bias: arguments.base_vertex,
        };
        if direct.instance_count > 0 {
            draw(context, &direct, std::slice::from_ref(&range))?;
        }
    }
    Ok(())
}

/// One draw: the API's vertices drawn by the vertex program, or, behind a geometry program, what
/// the stage helper made of them drawn by its pass-through program, with the fragment program.
fn draw<B: Backend>(
    context: &mut Context<B>,
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
) -> Result<(), String> {
    let gs = context.stages.get(Stage::Geometry).shader;
    // Behind a geometry program each range runs the stage helper and draws what it made.
    if !gs.is_null() && draws.len() > 1 {
        for d in draws {
            draw(context, info, std::slice::from_ref(d))?;
        }
        return Ok(());
    }
    // A geometry program emitting no vertex makes nothing to draw, stream or count.
    // SAFETY: a bound shader lives until Mesa unbinds and deletes it.
    if !gs.is_null() && unsafe { &*gs }.nir.geometry_vertices_out() == 0 {
        return Ok(());
    }
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
    let api_topology = topology(mesa_prim::from(info.mode));
    if gs.is_null() && api_topology.is_none() {
        return refused(format_args!(
            "a primitive with adjacency without a geometry program, or one the screen does not \
             report"
        ));
    }
    if raster.cull_both && gs.is_null() && api_topology.is_some_and(triangles) {
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
        border: border_in_program(context.stages.get(Stage::Fragment)),
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
    // The fragment program first: the last stage's outputs are interpolated as its inputs.
    // SAFETY: a bound shader lives until Mesa unbinds and deletes it.
    let inputs = programs
        .built_for(backend, unsafe { &*fs }, &fragment)?
        .inputs
        .clone();
    let (pulled, paths) = pulled_inputs(elements, &bound.vertex_buffers);
    let vertex = Variant {
        transform,
        swizzles: swizzles(context.stages.get(Stage::Vertex)),
        nearest: nearest_samplers(context.stages.get(Stage::Vertex)),
        border: border_in_program(context.stages.get(Stage::Vertex)),
        interpolation: inputs.clone(),
        pulled: pulled
            .iter()
            .filter_map(|&l| Some((l, elements.format(l)?)))
            .collect(),
        streamed: false,
        targets: Vec::new(),
        second_source: None,
        packed: elements.packed.clone(),
    };
    // The geometry program takes the position's transform, the vertex program's streamed as
    // written.
    let geometry = Variant {
        transform,
        swizzles: swizzles(context.stages.get(Stage::Geometry)),
        nearest: nearest_samplers(context.stages.get(Stage::Geometry)),
        border: border_in_program(context.stages.get(Stage::Geometry)),
        interpolation: inputs,
        ..Variant::default()
    };
    let (first, first_variant) = if gs.is_null() {
        (vs, vertex.clone())
    } else {
        (gs, geometry.clone())
    };
    // SAFETY: as above.
    programs.built_for(backend, unsafe { &*first }, &first_variant)?;
    if !first_variant.swizzles.is_empty() || !fragment.swizzles.is_empty() {
        backend.slow_paths().hit(SlowPath::SwizzleInShader);
    }
    if first_variant.border != 0 || fragment.border != 0 {
        backend.slow_paths().hit(SlowPath::BorderInShader);
    }
    if !vertex.packed.is_empty() {
        backend.slow_paths().hit(SlowPath::VertexFormatConverted);
    }
    let made = if gs.is_null() {
        for path in paths {
            backend.slow_paths().hit(path);
        }
        if context.streams.active() {
            for d in draws {
                // SAFETY: as above.
                let vertex_stage = (unsafe { &*vs }, &vertex);
                run_stages(context, backend, vertex_stage, None, elements, info, d)?;
            }
        }
        None
    } else {
        let Some(d) = draws.first() else {
            return Ok(());
        };
        // SAFETY: as above.
        let (vertex_stage, geometry_stage) = unsafe { ((&*vs, &vertex), (&*gs, &geometry)) };
        let made = run_stages(
            context,
            backend,
            vertex_stage,
            Some(geometry_stage),
            elements,
            info,
            d,
        )?;
        match made {
            Some(made) => Some(made),
            None => return Ok(()),
        }
    };
    if !context.streams.active() {
        query::count_drawn::<B>(&context.counting, info, draws, !gs.is_null())?;
    }
    let topology = match &made {
        Some(m) => m.out.topology,
        None => api_topology.ok_or("a primitive the screen does not report")?,
    };
    if raster.discard || (raster.cull_both && triangles(topology)) {
        return Ok(());
    }
    let bound = &context.bound;
    let programs = &mut context.programs;
    let built = [
        &programs.built[&(first as ShaderId, first_variant.clone())],
        &programs.built[&(fs as ShaderId, fragment.clone())],
    ];
    // Flat values from the vertex OpenGL names where the host takes the other: indices rewritten.
    let flat_from_other = raster.provoking != raster.host_provoking
        && built[1].flat_inputs
        && topology != Topology::PointList;
    if flat_from_other && made.is_some() {
        return refused(format_args!(
            "flat values from the last vertex of a geometry program's primitives (not built)"
        ));
    }
    let provoking = flat_from_other.then_some(Provoking {
        api: raster.provoking,
        host: raster.host_provoking,
    });
    let fragment_second_source = fragment.second_source.is_some();
    let program_key = (first as ShaderId, first_variant, fs as ShaderId, fragment);
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
    // Behind a geometry program the vertex stage ran already: the draw binds the fragment stage's.
    let both = [(Stage::Vertex, built[0]), (Stage::Fragment, built[1])];
    let (fetched, drawn) = match &made {
        Some(_) => (Vec::new(), &both[1..]),
        None => (
            fetched_streams(elements, &context.bound.vertex_buffers, &pulled)?,
            &both[..],
        ),
    };
    let mut images = Images::default();
    let sets = binding_sets(
        backend,
        &context.stages,
        drawn,
        &fetched,
        info.start_instance,
        StageSet::VERTEX,
        &mut images,
    )?;
    let pulled_mask = pulled.iter().fold(0, |mask, l| mask | 1 << l);
    let layout = match &made {
        Some(m) => m.layout.clone(),
        None => elements.block(backend, pulled_mask)?,
    };
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
    match &made {
        Some(m) => streams.push(Some(VertexBinding {
            buffer: &m.out.buffer,
            offset: 0,
        })),
        None => {
            for i in 0..elements.streams as usize {
                let read =
                    elements.desc.attributes.iter().flatten().any(|a| {
                        usize::from(a.stream) == i && pulled_mask & (1 << a.location) == 0
                    });
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
        }
    }
    let fan = matches!(topology, Topology::TriangleFan | Topology::LineLoop);
    let mode = mesa_prim::from(info.mode);
    let restart = match &made {
        Some(m) => m.out.index_offset.is_some(),
        None => info.primitive_restart(),
    };
    // Restart at an index other than the largest, where the host restarts.
    let other_restart = restart
        && info.index_size > 0
        && info.restart_index != index_format(info.index_size).restart_value();
    // Indices in a buffer the host cannot draw as they are (a fan or loop, flat values from the
    // other vertex, restart in a list or at another index, 8-bit indices the back end lacks): read
    // back now (`ReadBackWait`), then converted as the program's own.
    let rewritten_on_host = made.is_none()
        && (fan
            || provoking.is_some()
            || indices::restarts_otherwise(mode, restart)
            || other_restart
            || needs_widening(backend, index_format(info.index_size)));
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
    let samples = context.framebuffer.samples.clamp(1, u32::BITS);
    let all = u32::MAX >> (u32::BITS - samples);
    let mask = Some(bound.sample_mask & all).filter(|&m| samples > 1 && m != all);
    let blocks = masked_blocks(&mut context.programs, backend, raster, mask)?;
    let bound = &context.bound;
    let passes = face_passes(
        backend,
        depth_stencil,
        &blocks,
        raster.cull,
        triangles(topology),
        bound.stencil_ref,
    )?;
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
        match &made {
            Some(m) => draw_made(backend, &pass, &m.out),
            None => draw_ranges(
                backend,
                &pass,
                info,
                draws,
                read.as_ref(),
                raster.host_provoking,
                provoking,
            )?,
        }
    }
    for set in sets.into_iter().flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    images.finish(backend);
    Ok(())
}

/// Whether a topology draws triangles, which have faces.
fn triangles(topology: Topology) -> bool {
    matches!(
        topology,
        Topology::TriangleList | Topology::TriangleStrip | Topology::TriangleFan
    )
}

/// What the geometry stage made of one range of a draw: the stage helper's output, and the vertex
/// layout it is drawn with.
struct Made<B: Backend> {
    out: FinalDraw<B>,
    layout: B::VertexLayout,
}

/// The draw of what the stage helper made, as `pass` binds it otherwise: its vertices from the
/// helper's buffer as stream 0, through its indices (32-bit, strips restarting) where it gives them.
fn draw_made<B: Backend>(backend: &mut B, pass: &DrawCall<'_, B>, out: &FinalDraw<B>) {
    let index = out.index_offset.map(|offset| IndexBinding {
        buffer: &out.buffer,
        offset,
        format: IndexFormat::U32,
    });
    let range = match index {
        Some(_) => DrawRange::Indexed {
            first: 0,
            count: out.count,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        },
        None => DrawRange::Vertices {
            first: 0,
            count: out.count,
            first_instance: 0,
            instances: 1,
        },
    };
    backend.draw(&DrawCall {
        index,
        range,
        ..*pass
    });
}

/// The stage helper's topology of a Gallium primitive, adjacency included; none for a fan or loop.
fn stage_topology(mode: mesa_prim) -> Option<InputTopology> {
    Some(match mode {
        MESA_PRIM_LINES_ADJACENCY => InputTopology::LineListAdjacency,
        MESA_PRIM_LINE_STRIP_ADJACENCY => InputTopology::LineStripAdjacency,
        MESA_PRIM_TRIANGLES_ADJACENCY => InputTopology::TriangleListAdjacency,
        MESA_PRIM_TRIANGLE_STRIP_ADJACENCY => InputTopology::TriangleStripAdjacency,
        _ => return topology(mode).and_then(input_topology),
    })
}

/// The indices of one range of a draw behind a geometry program, as the stage helper takes them.
enum HelperIndices {
    /// The draw's own: none, or its index buffer.
    Own,
    /// Lists made on the host from the draw's vertices or indices.
    Rewritten(indices::Rewritten),
    /// Not one whole primitive.
    Nothing,
}

/// Lists made on the host where the stage helper cannot take a draw's own indices: a fan or loop,
/// indices in the program's memory or of 8 bits, restart in a list or at an index other than the
/// largest, an indexed strip without restart (the helper restarts strips at the largest index,
/// as Direct3D does), or a triangle strip whose odd triangles OpenGL orders otherwise. The
/// vertices keep OpenGL's order for its provoking vertex `provoking` (Mesa's index translator
/// with it as both conventions).
fn helper_indices<B: Backend>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    info: &pipe_draw_info,
    d: &pipe_draw_start_count_bias,
    provoking: ProvokingVertex,
) -> Result<HelperIndices, String> {
    let mode = mesa_prim::from(info.mode);
    let fan = stage_topology(mode).is_none();
    let strip = matches!(
        mode,
        MESA_PRIM_LINE_STRIP
            | MESA_PRIM_TRIANGLE_STRIP
            | MESA_PRIM_LINE_STRIP_ADJACENCY
            | MESA_PRIM_TRIANGLE_STRIP_ADJACENCY
    );
    let indexed = info.index_size > 0;
    let format = index_format(info.index_size);
    let restart = info.primitive_restart();
    let restarts_as_helper =
        restart == strip && (!restart || info.restart_index == format.restart_value());
    // The helper reads 16- and 32-bit indices.
    let widened = indexed && format == IndexFormat::U8;
    // The helper gives an odd triangle of a strip its first vertex first, as Direct3D does;
    // OpenGL with the last vertex provoking gives it the strip's order of its first two.
    let reordered = provoking == ProvokingVertex::Last
        && matches!(
            mode,
            MESA_PRIM_TRIANGLE_STRIP | MESA_PRIM_TRIANGLE_STRIP_ADJACENCY
        );
    let own = !indexed || (!info.has_user_indices() && !widened && restarts_as_helper);
    if !fan && !reordered && own {
        return Ok(HelperIndices::Own);
    }
    let paths = [
        (fan, SlowPath::FanConverted),
        (reordered, SlowPath::ProvokingRewrite),
        (indexed && !restarts_as_helper, SlowPath::RestartRewrite),
        (widened, SlowPath::IndexWidened),
    ];
    for (_, path) in paths.into_iter().filter(|&(taken, _)| taken) {
        backend.slow_paths().hit(path);
    }
    let rewritten = if !indexed {
        indices::generated(mode, d.start, d.count, true, provoking, provoking)?
    } else if info.has_user_indices() {
        let size = usize::from(info.index_size);
        let length = d.count as usize * size;
        // SAFETY: st/mesa's indices in memory hold the draw's range.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                info.index.user.cast::<u8>().add(d.start as usize * size),
                length,
            )
        };
        indices::translated(info, bytes, restart, true, provoking, provoking)?
    } else {
        let (bytes, _) = read_indices(tracker, backend, info, std::slice::from_ref(d))?;
        indices::translated(info, &bytes, restart, true, provoking, provoking)?
    };
    Ok(rewritten.map_or(HelperIndices::Nothing, HelperIndices::Rewritten))
}

/// One range of a draw run by the stage helper outside the pass: the vertex program, then the
/// geometry program where one is given (`GeometryAsCompute`), the last of them streamed to the
/// bound targets where stream output is active (`StreamOutAsCompute`). What a geometry program
/// made, to draw; `None` without one, or where it made no primitive.
fn run_stages<B: Backend>(
    context: &mut Context<B>,
    backend: &mut B,
    (vs, vertex): (&Shader, &Variant),
    geometry: Option<(&Shader, &Variant)>,
    elements: &state::Elements<B>,
    info: &pipe_draw_info,
    d: &pipe_draw_start_count_bias,
) -> Result<Option<Made<B>>, String> {
    let declared = geometry
        .map_or(&vs.streamed, |(gs, _)| &gs.streamed)
        .clone()?;
    let streaming = context.streams.active() && declared.is_some();
    if geometry.is_none() && !streaming {
        return Ok(None);
    }
    let mode = mesa_prim::from(info.mode);
    // A draw of no whole primitive records nothing, its statistics included.
    if indices::trimmed(mode, d.count) == 0 {
        return Ok(None);
    }
    // SAFETY: a bound rasterizer state lives until Mesa unbinds and deletes it.
    let provoking = unsafe { &*context.bound.raster }.provoking;
    let host_indices = helper_indices(&mut context.tracker, backend, info, d, provoking)?;
    let (topology, range_count) = match &host_indices {
        HelperIndices::Nothing => return Ok(None),
        HelperIndices::Own => (stage_topology(mode).ok_or("a fan or loop")?, d.count),
        HelperIndices::Rewritten(r) => (
            stage_topology(r.primitive).ok_or("rewritten indices of a fan or loop")?,
            r.count(),
        ),
    };
    let (streamed, locations) = streamed_variant(vertex, elements);
    let programs = &mut context.programs;
    programs.built_for(backend, vs, &streamed)?;
    let vertex_key = (std::ptr::from_ref(vs) as ShaderId, streamed);
    let geometry_key = geometry.map(|(gs, v)| (std::ptr::from_ref(gs) as ShaderId, v.clone()));
    let built_vertex = &programs.built[&vertex_key];
    let built_geometry = match &geometry_key {
        Some(key) => Some(&programs.built[key]),
        None => None,
    };
    if built_geometry.is_some_and(|g| g.helper_size != 0) {
        return refused(format_args!(
            "helper constants in a geometry program (not built)"
        ));
    }
    let fetched = fetched_streams(elements, &context.bound.vertex_buffers, &locations)?;
    let mut images = Images::default();
    let vertex_sets = binding_sets(
        backend,
        &context.stages,
        &[(Stage::Vertex, built_vertex)],
        &fetched,
        info.start_instance,
        StageSet::VERTEX.union(StageSet::COMPUTE),
        &mut images,
    )?;
    let geometry_sets = match built_geometry {
        Some(g) => binding_sets(
            backend,
            &context.stages,
            &[(Stage::Geometry, g)],
            &[],
            info.start_instance,
            StageSet::COMPUTE,
            &mut images,
        )?,
        None => [const { None }; MAX_BINDING_SETS],
    };
    let (geometry_id, geometry_variant) = geometry_key.unwrap_or_default();
    let key = (
        vertex_key.0,
        vertex_key.1.clone(),
        geometry_id,
        geometry_variant,
    );
    if !programs.staged.contains_key(&key) {
        let vertex_forms = built_vertex
            .converted
            .as_ref()
            .ok_or("a streamed program without its module")?;
        let geometry_forms = match built_geometry {
            Some(g) => Some(
                g.converted
                    .as_ref()
                    .ok_or("a geometry program without its forms")?,
            ),
            None => None,
        };
        let stages = Stages::new(backend, vertex_forms, geometry_forms, declared.as_ref())
            .map_err(|e| format!("the stage helper: {e:?}"))?;
        if stages.selects() {
            stages.destroy(backend);
            return refused(format_args!(
                "a geometry program writing a layer or viewport (not built)"
            ));
        }
        let layout = match stages.vertex_layout() {
            Some(desc) => Some(
                backend
                    .create_vertex_layout(&desc)
                    .map_err(|e| format!("{e:?}"))?,
            ),
            None => None,
        };
        programs.staged.insert(
            key.clone(),
            Staged {
                stages,
                layout,
                indices: None,
            },
        );
    }
    let staged = programs.staged.get_mut(&key).ok_or("the stage helper")?;
    context
        .tracker
        .close(backend)
        .map_err(|e| format!("{e:?}"))?;
    // Rewritten indices go to a buffer of the helper's, which it reads as storage.
    if let HelperIndices::Rewritten(r) = &host_indices {
        staged.write_indices(backend, r.indices())?;
    }
    let index = match &host_indices {
        HelperIndices::Rewritten(r) => Some(StageIndex {
            buffer: &staged.indices.as_ref().ok_or("the helper's indices")?.0,
            size: (r.indices().len() as u64).next_multiple_of(STORAGE_WORD),
            offset: 0,
            format: r.format,
        }),
        _ if info.index_size == 0 => None,
        _ => {
            // SAFETY: an indexed draw without indices in memory names a live buffer.
            let resource = unsafe { Resource::<B>::of(info.index.resource) };
            Some(StageIndex {
                buffer: resource.buffer().ok_or("indices in a texture")?,
                size: u64::from(resource.base.width0).next_multiple_of(STORAGE_WORD),
                offset: 0,
                format: index_format(info.index_size),
            })
        }
    };
    let range = match (&host_indices, index.is_some()) {
        // Indices made from a draw without any name the vertices themselves.
        (HelperIndices::Rewritten(_), _) => DrawRange::Indexed {
            first: 0,
            count: range_count,
            base_vertex: if info.index_size > 0 { d.index_bias } else { 0 },
            first_instance: info.start_instance,
            instances: info.instance_count,
        },
        (_, true) => DrawRange::Indexed {
            first: d.start,
            count: range_count,
            base_vertex: d.index_bias,
            first_instance: info.start_instance,
            instances: info.instance_count,
        },
        (_, false) => DrawRange::Vertices {
            first: d.start,
            count: range_count,
            first_instance: info.start_instance,
            instances: info.instance_count,
        },
    };
    let stream_output = if streaming {
        Some(context.streams.targets()?)
    } else {
        None
    };
    let ran = staged.stages.run(
        backend,
        &StageDraw {
            vertex_sets: std::array::from_fn(|set| vertex_sets[set].as_ref()),
            geometry_sets: std::array::from_fn(|set| geometry_sets[set].as_ref()),
            index,
            topology,
            range,
            stream_output,
        },
    );
    for set in vertex_sets.into_iter().chain(geometry_sets).flatten() {
        backend.destroy(Object::BindingSet(set));
    }
    images.finish(backend);
    let out = ran.map_err(|e: StageError| format!("the stage helper: {e:?}"))?;
    if streaming {
        let statistics = context
            .streams
            .filled
            .as_ref()
            .ok_or("stream output's block")?;
        query::count_emitted(&context.counting, backend, statistics)?;
    }
    let made = match (out, &staged.layout) {
        (Some(out), Some(layout)) => Some(Made {
            out,
            layout: layout.clone(),
        }),
        _ => None,
    };
    Ok(made)
}

/// The variant of a vertex program the stage helper runs, and the locations of its inputs: every
/// input fetched in it, its position streamed as written (before the host's depth range and
/// flips, which a stage after it or the stream's reader takes).
fn streamed_variant<B: Backend>(
    vertex: &Variant,
    elements: &state::Elements<B>,
) -> (Variant, Vec<u32>) {
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
        transform: PositionTransform {
            fixed_point_size: vertex.transform.fixed_point_size,
            ..PositionTransform::default()
        },
        ..vertex.clone()
    };
    (variant, locations)
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

/// One depth, stencil and rasterizer block and stencil reference a draw is made with.
type FacePass<'a, B> = (
    &'a <B as Backend>::DepthStencil,
    &'a <B as Backend>::Raster,
    [u8; 2],
);

/// The rasterizer blocks a draw takes: the one drawing both faces, and one a face for a draw split
/// by face.
struct RasterBlocks<R> {
    block: R,
    faces: Option<[R; 2]>,
}

/// The bound rasterizer's blocks, or with `mask` (fewer than all of the targets' samples) blocks of
/// it with that sample mask.
fn masked_blocks<B: Backend>(
    programs: &mut Programs<B>,
    backend: &mut B,
    raster: &state::Raster<B>,
    mask: Option<u32>,
) -> Result<RasterBlocks<B::Raster>, String> {
    let Some(mask) = mask else {
        return Ok(RasterBlocks {
            block: raster.block.clone(),
            faces: raster.faces.clone(),
        });
    };
    if !backend.caps().has(Features::SAMPLE_MASK) {
        return refused(format_args!("a sample mask on a back end without one"));
    }
    let mut made = |desc: RasterDesc| -> Result<B::Raster, String> {
        if let Some(block) = programs.masked.get(&desc) {
            return Ok(block.clone());
        }
        let block = backend
            .create_raster(&desc)
            .map_err(|e| format!("a rasterizer state: {e:?}"))?;
        programs.masked.insert(desc, block.clone());
        Ok(block)
    };
    let desc = RasterDesc {
        sample_mask: mask,
        ..raster.desc
    };
    let faces = if raster.faces.is_some() {
        let [back, front] =
            [CullMode::Back, CullMode::Front].map(|cull| RasterDesc { cull, ..desc });
        Some([made(back)?, made(front)?])
    } else {
        None
    };
    Ok(RasterBlocks {
        block: made(desc)?,
        faces,
    })
}

/// The blocks a draw is made with: its own, or, where the stencil test's faces differ in masks or
/// reference and the back end has one of each per draw, each face alone with its own
/// (`StencilFacesSplit`). Points and lines are front faces, as OpenGL draws them.
fn face_passes<'a, B: Backend>(
    backend: &mut B,
    depth_stencil: &'a state::DepthStencil<B>,
    raster: &'a RasterBlocks<B::Raster>,
    cull: CullMode,
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
    Ok(match (triangles, cull) {
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
        topology: rewritten.topology()?,
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
            if let Some(r) = indices::generated(mode, d.start, d.count, false, p.api, p.host)? {
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
            // The host restarts only at the largest value.
            let other_restart = restart && info.restart_index != format.restart_value();
            let cut = indices::restarts_otherwise(mode, restart) || other_restart;
            let lists_only = (!restart || other_restart)
                && strip
                && !restart_switch
                && holds_largest(indices, format);
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

/// The units of a stage whose sampler's border colour the program applies, a bit each: those
/// whose texture is not integer (an integer texture's nearest load applies it, `NearestSampler`).
fn border_in_program<B: Backend>(stage: &state::StageState<B>) -> u16 {
    let mut mask = 0;
    for (unit, (&v, &s)) in stage.views.iter().zip(&stage.samplers).enumerate() {
        if v.is_null() || s.is_null() {
            continue;
        }
        // SAFETY: as in `nearest_samplers`.
        let (view, sampler) = unsafe { (View::<B>::of(v), &*s) };
        if sampler.border_in_program.is_none() || view.integer {
            continue;
        }
        if unit >= u16::BITS as usize {
            refuse(format_args!(
                "a border colour at a texture unit past the variant's"
            ));
        }
        mask |= 1 << unit;
    }
    mask
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
