//! NIR from the pinned Mesa, owned, through Mesa's compiler as the `wcontainers-mesa-compiler` trait
//! gives it: a program is a value of the type of its place in the chains the callers take (read
//! from SPIR-V, then in the common form; received from GLSL, prepared, then a draw's variant).

use crate::ConvertError;
use crate::bind::{self, KINDS};
use crate::direct::FLATTEN;
#[cfg(feature = "standalone")]
use std::ffi::{CStr, CString};
use wcontainers_gpu::{reflect::ReflectedBinding, reflect::SLOT_KINDS, shader::ShaderStage};
use wcontainers_mesa_compiler::{MesaCompiler, constants};
use zerocopy::TryFromBytes;

/// The program stages the routes take (`enum wc_stage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

impl Stage {
    /// Every stage, in the bridge's order (`WC_STAGES`).
    pub const ALL: &[Stage] = &[Stage::Vertex, Stage::Fragment, Stage::Compute];

    pub const fn shader_stage(self) -> ShaderStage {
        match self {
            Stage::Vertex => ShaderStage::Vertex,
            Stage::Fragment => ShaderStage::Fragment,
            Stage::Compute => ShaderStage::Compute,
        }
    }

    /// The stage's number in the bridge's `enum wc_stage`.
    #[cfg(feature = "standalone")]
    const fn bridge(self) -> u32 {
        match self {
            Stage::Vertex => constants::STAGE_VERTEX,
            Stage::Fragment => constants::STAGE_FRAGMENT,
            Stage::Compute => constants::STAGE_COMPUTE,
        }
    }
}

/// What a name of a GLSL program, or a state value, is in one stage (`enum wc_glsl_name_kind`).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromBytes)]
#[allow(dead_code, reason = "the bridge makes every value")]
pub enum GlslNameKind {
    /// A default-block uniform: `at` its first word in uniform buffer 0, `count` its words.
    Uniform,
    /// A subroutine uniform by its API name, as `Uniform`: each element's word holds the index of
    /// the subroutine chosen for it.
    SubroutineUniform,
    /// `STATE_FB_WPOS_Y_TRANSFORM`'s four words at `at`.
    FragCoordYTransform,
    /// `STATE_FB_PNTC_Y_TRANSFORM`'s four words at `at`.
    PointCoordYTransform,
    /// Any other state value: a matrix's rows as `matrix.<matrix>.<form>.<index>.<first
    /// row>.<last row>` (form `plain`, `inverse`, `transpose` or `invtrans`, as Mesa's state
    /// tokens), anything else as `state.` and Mesa's five tokens.
    OtherState,
    /// `at` the program binding, `count` the bindings.
    UniformBlock,
    StorageBlock,
    Sampler,
    Image,
    /// An atomic counter buffer, which the driver gives the program as a storage block: `at` its
    /// program binding, `element` its OpenGL binding point.
    AtomicCounterBuffer,
    /// `at` the location in the common form, `count` the locations.
    Input,
    Output,
}

/// A stage value's interpolation qualifier (`enum wc_glsl_interpolation`).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromBytes)]
#[allow(dead_code, reason = "the bridge makes every value")]
pub enum Interpolation {
    Unqualified,
    Smooth,
    Flat,
    NoPerspective,
    Other,
}

/// One name of a GLSL program in one stage and what the OpenGL driver's state reaches it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlslName {
    pub kind: GlslNameKind,
    pub name: String,
    /// A stage value's GLSL type.
    pub ty: Option<String>,
    pub interpolation: Interpolation,
    pub at: u32,
    pub count: u32,
    /// The first of the value's slots the name places: a vertex attribute read in part (some
    /// columns of a matrix) has a name a run of the slots read.
    pub element: u32,
}

/// One output a vertex program streams (`struct wc_stream_output`): its buffer, its byte offset in
/// each vertex there, where the route places it (a generic location, or the position), and its
/// components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamedOutput {
    pub buffer: u32,
    pub offset: u32,
    pub location: u32,
    pub position: bool,
    pub first_component: u8,
    pub components: u8,
}

/// The compiler's named layer for a program naming no layer.
const NO_NAMED_LAYER: u32 = u32::MAX;

/// What one component of a texture's result is made of (`enum wc_swizzle`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Swizzle {
    X,
    Y,
    Z,
    W,
    Zero,
    One,
}

/// The swizzle of the texture bound at one texture unit (`struct wc_texture_swizzle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureSwizzle {
    pub unit: u32,
    /// One entry per component of a texture's result, a vector's most (`WC_COMPONENTS`).
    pub swizzle: [Swizzle; naga::VectorSize::MAX],
}

/// How a texture coordinate outside the texture wraps, for a nearest texel (`enum wc_wrap`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wrap {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
    /// Outside the texture, the border colour.
    ClampToBorder,
}

/// The sampler at one texture unit whose texture holds integers (`struct wc_nearest_sampler`):
/// its wraps, whether it selects a level by the level of detail, that level's bounds (as f32
/// bits), and the border colour's integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NearestSampler {
    pub unit: u32,
    /// One a texture coordinate, the most a texture has (`WC_TEXTURE_AXES`).
    pub wrap: [Wrap; naga::VectorSize::Tri as usize],
    pub mipmapped: bool,
    pub min_lod: u32,
    pub max_lod: u32,
    pub border: [u32; naga::VectorSize::MAX],
}

/// The primitive a pass-through geometry program takes (`Prepared::pass_through`), in the
/// bridge's order (`WC_PASS_THROUGH_INPUTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assembled {
    Points,
    Lines,
    Triangles,
    LinesAdjacency,
    TrianglesAdjacency,
}

impl Assembled {
    /// Every one, in the bridge's order.
    pub const ALL: &[Assembled] = &[
        Assembled::Points,
        Assembled::Lines,
        Assembled::Triangles,
        Assembled::LinesAdjacency,
        Assembled::TrianglesAdjacency,
    ];
}

/// How a vertex program's position (and point size) is changed to meet the host's clip space,
/// viewport and points (`struct wc_position_transform`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PositionTransform {
    /// y negated before the route's own negation, for a Gallium viewport whose y scale is
    /// negative.
    pub flip_y: bool,
    /// Depth from OpenGL's -w..w to the host's 0..w.
    pub half_z: bool,
    /// Depth as w - z, for a depth range whose near value is the larger.
    pub reverse_z: bool,
    /// The point size output removed: the host draws points of one size, to which OpenGL clamps
    /// every size a program writes.
    pub fixed_point_size: bool,
}

/// Each binding kind's range of binding numbers, in the bridge's order of kinds (`enum wc_slot_kind`).
fn slot_ranges() -> [wcontainers_mesa_compiler::SlotRange; KINDS.len()] {
    KINDS.map(|k| wcontainers_mesa_compiler::SlotRange {
        first: SLOT_KINDS[k as usize].first_binding,
        slots: SLOT_KINDS[k as usize].slots,
    })
}

/// A name of a GLSL program as the compiler gave it, its two enumerations checked.
#[cfg(feature = "standalone")]
fn glsl_name(name: wcontainers_mesa_compiler::GlslName) -> Result<GlslName, ConvertError> {
    let text = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned();
    Ok(GlslName {
        kind: crate::checked(name.kind, GLSL_NAME_KIND)?,
        name: name.name.map(text).unwrap_or_default(),
        ty: name.type_.map(text),
        interpolation: crate::checked(name.interpolation, GLSL_INTERPOLATION)?,
        at: name.at,
        count: name.count,
        element: name.element,
    })
}

/// The cache's key of what a route converts from NIR whose serialised bytes are `bytes` (plan
/// 15.3): BLAKE3 of the bytes and the converter's version ([`CONVERTER_VERSION`]).
fn keyed(bytes: &[u8]) -> [u8; blake3::OUT_LEN] {
    *blake3::Hasher::new()
        .update(bytes)
        .update(CONVERTER_VERSION.as_bytes())
        .finalize()
        .as_bytes()
}

/// Each resource the common form bound in `set`, its kind checked.
fn reflected(
    bindings: &[wcontainers_mesa_compiler::Binding],
    set: u32,
) -> Result<Vec<ReflectedBinding>, ConvertError> {
    bindings
        .iter()
        .map(|b| {
            let kind = usize::try_from(b.kind)
                .ok()
                .and_then(|at| KINDS.get(at))
                .ok_or(ConvertError::Unknown(SLOT_KIND, b.kind))?;
            Ok(ReflectedBinding {
                kind: *kind,
                space: b.set,
                register: b.binding,
                count: b.count,
                set,
                binding: b.new_binding,
                added: None,
                stride: b.stride,
            })
        })
        .collect()
}

impl PositionTransform {
    /// The transform as the compiler takes it.
    fn bridge(self) -> wcontainers_mesa_compiler::PositionTransform {
        wcontainers_mesa_compiler::PositionTransform {
            flip_y: self.flip_y,
            half_z: self.half_z,
            reverse_z: self.reverse_z,
            fixed_point_size: self.fixed_point_size,
        }
    }
}

/// What a draw makes a variant of a program for: the driver's settings at a draw (its targets'
/// samples, the texture its first target is copied to, the position's transform).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawSettings {
    /// The position's transform (none where every field is false).
    pub transform: PositionTransform,
    /// The transform the cache's key is taken with: where it is not `transform`, the key is of a
    /// copy with it applied after `transform` (a geometry program's records).
    pub key_transform: PositionTransform,
    /// A program reading its first target's texels reads them from the texture at this unit,
    /// multisampled where `fetch_multisampled`.
    pub fetch_unit: u32,
    pub fetch_multisampled: bool,
    /// The targets' samples: one is single-sampled.
    pub samples: u32,
}

impl Default for DrawSettings {
    /// Single-sampled targets, no transform, no target read.
    fn default() -> DrawSettings {
        DrawSettings {
            transform: PositionTransform::default(),
            key_transform: PositionTransform::default(),
            fetch_unit: 0,
            fetch_multisampled: false,
            samples: 1,
        }
    }
}

/// The settings as the compiler takes them, the common form's bindings in the set of `stage`.
fn variant_settings(
    settings: &DrawSettings,
    stage: Stage,
) -> wcontainers_mesa_compiler::VariantSettings {
    wcontainers_mesa_compiler::VariantSettings {
        transform: settings.transform.bridge(),
        key_transform: settings.key_transform.bridge(),
        fetch_unit: settings.fetch_unit,
        fetch_multisampled: settings.fetch_multisampled,
        single_sampled: settings.samples <= 1,
        samples: settings.samples,
        set: bind::stage_set(stage.shader_stage()),
        ranges: slot_ranges(),
    }
}

/// The swizzles as the compiler takes them.
fn bridge_swizzles(swizzles: &[TextureSwizzle]) -> Vec<wcontainers_mesa_compiler::TextureSwizzle> {
    swizzles
        .iter()
        .map(|s| wcontainers_mesa_compiler::TextureSwizzle {
            unit: s.unit,
            swizzle: s.swizzle.map(|c| c as u8),
        })
        .collect()
}

/// The samplers as the compiler takes them.
fn bridge_samplers(samplers: &[NearestSampler]) -> Vec<wcontainers_mesa_compiler::NearestSampler> {
    samplers
        .iter()
        .map(|s| wcontainers_mesa_compiler::NearestSampler {
            unit: s.unit,
            wrap: s.wrap.map(|w| w as u8),
            mipmapped: s.mipmapped,
            min_lod: f32::from_bits(s.min_lod),
            max_lod: f32::from_bits(s.max_lod),
            border: s.border,
        })
        .collect()
}

/// A program read from SPIR-V for Vulkan, owned by Mesa's compiler `M` and given back when dropped:
/// the corpus's first step.
pub struct SpirvProgram<M: MesaCompiler> {
    compiler: M,
    shader: M::SpirvShader,
}

/// A program read from SPIR-V in the common form, which either candidate converts.
pub struct CommonProgram<M: MesaCompiler> {
    compiler: M,
    shader: M::SpirvCommon,
}

/// One stage of a linked GLSL program as the OpenGL driver receives it, with its names.
pub struct GlslStage<M: MesaCompiler> {
    pub stage: Stage,
    pub received: Received<M>,
    pub names: Vec<GlslName>,
}

/// A stage as the OpenGL driver receives it from st/mesa.
pub struct Received<M: MesaCompiler> {
    compiler: M,
    shader: M::Received,
}

/// A stage after the driver's steps at its creation, and what the driver reads of it then and at
/// its draws.
pub struct Prepared<M: MesaCompiler> {
    compiler: M,
    shader: M::Prepared,
    /// The cache's key of the prepared program.
    pub key: [u8; blake3::OUT_LEN],
    /// The outputs it streams (its transform feedback, none without it); `None` when one is a value
    /// the route places neither at a location nor as the position.
    pub streamed: Option<Vec<StreamedOutput>>,
    /// Each stream buffer's bytes a vertex.
    pub strides: [u32; constants::STREAM_BUFFERS as usize],
    /// Whether a fragment program reads where its samples are (`gl_SamplePosition`,
    /// `interpolateAtSample`, `interpolateAtOffset` of a value read per sample), which depends on
    /// the targets' sample count.
    pub reads_sample_positions: bool,
    /// Whether a fragment program writes `gl_SampleMask`.
    pub writes_sample_mask: bool,
    /// Whether a fragment program reads its first target's texels (framebuffer fetch).
    pub fetches_target: bool,
    /// The image units the program declares images at without a format, a bit each: their stores
    /// take the bound image's format, which the variant converting it names.
    pub formatless_images: u64,
    /// The texture units the program samples integers at, a bit each: an integer texture there is
    /// sampled through the variant's nearest samplers.
    pub integer_sampled_units: u64,
    /// For a geometry program writing `gl_Layer`, the location after its generic outputs.
    pub named_layer: Option<u32>,
    /// For a vertex or evaluation program using storage, the geometry programs passing each
    /// primitive's vertices on, by the primitive taken (`Assembled`); `None` for any other. An entry
    /// is `None` where it cannot be made.
    pub pass_through: Option<[Option<Made<M>>; Assembled::ALL.len()]>,
    /// Where the generic locations a fragment program reads, or another stage's program writes, end.
    pub generic_end: u32,
    /// The most vertices a geometry program emits an invocation; zero for another stage.
    pub geometry_vertices_out: u32,
    /// The control points of a tessellation control program's output patch; zero for another stage.
    pub control_points_out: u32,
    /// An evaluation program in point mode, or a geometry program whose output is points.
    pub makes_points: bool,
}

/// A program the driver's steps make: a pass-through geometry program, a vertex program with its
/// point size also at a location, or a geometry program drawing points as squares.
pub struct Made<M: MesaCompiler> {
    compiler: M,
    shader: M::Made,
}

/// A draw's variant of a program in the common form, its key and its bindings.
pub struct Variant<M: MesaCompiler> {
    compiler: M,
    shader: M::Variant,
    /// The cache's key of the variant.
    pub key: [u8; blake3::OUT_LEN],
    /// Each resource's reflection.
    pub bindings: Vec<ReflectedBinding>,
}

/// What drawing a program's points as squares makes.
pub enum Squares<M: MesaCompiler> {
    /// Every point the program writes is one pixel: the host draws them itself.
    OnePixel,
    /// The program writes clip distances, or outputs at offsets known only when run.
    Refused,
    /// The geometry program drawing the squares (`SlowPath::PointsAsQuads`), each corner with every
    /// output and `gl_PointCoord` at the location asked; and for a vertex program writing a point
    /// size, the copy of it also writing that size there, drawn before.
    Made {
        before: Option<Made<M>>,
        geometry: Made<M>,
    },
}

impl<M: MesaCompiler + Clone> SpirvProgram<M> {
    #[cfg(feature = "standalone")]
    /// The NIR of one entry point of a Vulkan SPIR-V module, with Vulkan's `alphaToOne` made in a
    /// fragment program where `alpha_to_one` (plan 15.4, `AlphaToOneInShader`).
    pub fn read(
        words: &[u32],
        stage: Stage,
        entry_point: &str,
        alpha_to_one: bool,
    ) -> Result<SpirvProgram<M>, ConvertError> {
        let compiler = M::new();
        let entry = CString::new(entry_point).map_err(|_| ConvertError::Mesa(SPIRV_TO_NIR))?;
        let shader = compiler
            .spirv_read(words, stage.bridge(), &entry, alpha_to_one)
            .ok_or(ConvertError::Mesa(SPIRV_TO_NIR))?;
        Ok(SpirvProgram { compiler, shader })
    }

    /// NIR's serialised bytes, taken before the common form.
    pub fn serialized(&self) -> Result<Vec<u8>, ConvertError> {
        self.compiler
            .spirv_shader_key(&self.shader)
            .ok_or(ConvertError::Mesa(NIR_SERIALIZE))
    }

    /// The cache's key of what a route converts from this program (plan 15.3).
    pub fn key(&self) -> Result<[u8; blake3::OUT_LEN], ConvertError> {
        Ok(keyed(&self.serialized()?))
    }

    /// The program of `stage` given the common form's meaning (sampled array layers rounded to
    /// integers) and its binding rule ([`crate::bind`]), combined image samplers split, with each
    /// resource's reflection.
    pub fn to_common_form(
        self,
        stage: Stage,
    ) -> Result<(CommonProgram<M>, Vec<ReflectedBinding>), ConvertError> {
        let set = bind::stage_set(stage.shader_stage());
        let SpirvProgram { compiler, shader } = self;
        let made = compiler.spirv_to_common_form(shader, set, &slot_ranges());
        match made.result {
            constants::BIND_OK => {}
            constants::BIND_BEYOND_LIMIT => return Err(ConvertError::Bindings(bind::BEYOND_LIMIT)),
            constants::BIND_UNBOUNDED => return Err(ConvertError::Bindings(bind::UNBOUNDED)),
            constants::BIND_OUT_OF_MEMORY => {
                return Err(ConvertError::Bindings(bind::OUT_OF_MEMORY));
            }
            other => return Err(ConvertError::Unknown(BIND_RESULT, other)),
        }
        let bindings = reflected(&made.bindings, set)?;
        let shader = made
            .shader
            .ok_or(ConvertError::Bindings(bind::OUT_OF_MEMORY))?;
        Ok((CommonProgram { compiler, shader }, bindings))
    }
}

impl<M: MesaCompiler + Clone> CommonProgram<M> {
    #[cfg(feature = "standalone")]
    /// Candidate A's first step: SPIR-V written by Zink's compiler.
    pub fn into_spirv(self) -> Result<Vec<u32>, ConvertError> {
        self.compiler
            .spirv_common_to_spirv(self.shader)
            .ok_or(ConvertError::Mesa(NIR_TO_SPIRV))
    }

    /// Candidate B's first step: the program out of SSA and flattened.
    pub fn flatten(self) -> Result<wcontainers_mesa_compiler::Flat, ConvertError> {
        self.compiler
            .spirv_common_flatten(self.shader)
            .ok_or(ConvertError::Mesa(FLATTEN))
    }
}

impl<M: MesaCompiler + Clone> GlslStage<M> {
    #[cfg(feature = "standalone")]
    /// The stages of one GLSL program as the OpenGL driver receives them: compiled and linked by
    /// Mesa's GLSL compiler, then st/mesa's steps for the driver's capabilities (the bridge's
    /// `glsl.cpp`); with each stage's names. One entry a source, in `Stage::ALL`'s order.
    pub fn compile(sources: &[(Stage, &str)]) -> Result<Vec<GlslStage<M>>, ConvertError> {
        let compiler = M::new();
        let owned = sources
            .iter()
            .map(|(stage, text)| Ok((*stage, CString::new(*text)?)))
            .collect::<Result<Vec<_>, std::ffi::NulError>>()
            .map_err(|e| ConvertError::Glsl(e.to_string()))?;
        let texts: [Option<&CStr>; constants::STAGES as usize] = std::array::from_fn(|at| {
            owned
                .iter()
                .find(|(s, _)| Stage::ALL.get(at) == Some(s))
                .map(|(_, text)| text.as_c_str())
        });
        let program = compiler
            .glsl_to_nir(texts)
            .map_err(|log| ConvertError::Glsl(String::from_utf8_lossy(&log).into_owned()))?;
        Stage::ALL
            .iter()
            .zip(program.stages.into_iter().zip(program.names))
            .filter_map(|(stage, (shader, names))| shader.map(|shader| (*stage, shader, names)))
            .map(
                |(stage, shader, names)| -> Result<GlslStage<M>, ConvertError> {
                    Ok(GlslStage {
                        stage,
                        received: Received {
                            compiler: compiler.clone(),
                            shader,
                        },
                        names: names.into_iter().map(glsl_name).collect::<Result<_, _>>()?,
                    })
                },
            )
            .collect()
    }
}

impl<M: MesaCompiler + Clone> Received<M> {
    /// The driver's steps at the stage's creation, in its order: the name dropped (so programs of
    /// equal content have one key), uniform blocks as words, resource arrays read at constant
    /// indices split, 1D textures as 2D; a vertex program's inputs numbered as vertex elements, a
    /// fragment program's gl_FragColor written to every draw buffer; the last stage before the
    /// rasterizer's point size streamed at a location; the facts read; a vertex or evaluation
    /// program's pass-through programs made.
    pub fn prepare(self) -> Result<Prepared<M>, ConvertError> {
        let Received { compiler, shader } = self;
        let form = compiler.received_prepare(shader);
        let shader = form.shader.ok_or(ConvertError::Mesa(NIR_SERIALIZE))?;
        let streamed = (!form.streams_elsewhere).then(|| {
            form.streamed
                .iter()
                .map(|o| StreamedOutput {
                    buffer: o.buffer,
                    offset: o.offset,
                    location: o.location,
                    position: o.position,
                    first_component: o.first_component,
                    components: o.components,
                })
                .collect()
        });
        let owned = |shader: M::Made| Made {
            compiler: compiler.clone(),
            shader,
        };
        let pass_through = form
            .passes_through
            .then(|| form.pass_through.map(|made| made.map(owned)));
        Ok(Prepared {
            key: keyed(&form.key),
            streamed,
            strides: form.strides,
            reads_sample_positions: form.reads_sample_positions,
            writes_sample_mask: form.writes_sample_mask,
            fetches_target: form.fetches_target,
            formatless_images: form.formatless_images,
            integer_sampled_units: form.integer_sampled_units,
            named_layer: (form.named_layer != NO_NAMED_LAYER).then_some(form.named_layer),
            pass_through,
            generic_end: form.generic_end,
            geometry_vertices_out: form.geometry_vertices_out,
            control_points_out: form.control_points_out,
            makes_points: form.makes_points,
            compiler,
            shader,
        })
    }
}

/// The variant the compiler made, its result checked, its bindings in the set of `stage`.
fn variant<M: MesaCompiler + Clone>(
    compiler: &M,
    form: wcontainers_mesa_compiler::VariantForm<M::Variant>,
    stage: Stage,
    samples: u32,
) -> Result<Variant<M>, ConvertError> {
    match form.result {
        constants::VARIANT_MADE => {}
        constants::VARIANT_GATHERS_INTEGER_CUBE => {
            return Err(ConvertError::Unsupported(
                "a gather of an integer cube map, whose faces' edges are not joined".into(),
            ));
        }
        constants::VARIANT_SAMPLES_UNKNOWN => {
            return Err(ConvertError::Unsupported(format!(
                "reads at the positions of {samples} samples, which are not known"
            )));
        }
        constants::VARIANT_BEYOND_LIMIT => return Err(ConvertError::Bindings(bind::BEYOND_LIMIT)),
        constants::VARIANT_UNBOUNDED => return Err(ConvertError::Bindings(bind::UNBOUNDED)),
        constants::VARIANT_OUT_OF_MEMORY => {
            return Err(ConvertError::Bindings(bind::OUT_OF_MEMORY));
        }
        other => return Err(ConvertError::Unknown(VARIANT_RESULT, other)),
    }
    let set = bind::stage_set(stage.shader_stage());
    let bindings = reflected(&form.bindings, set)?;
    let shader = form
        .shader
        .ok_or(ConvertError::Bindings(bind::OUT_OF_MEMORY))?;
    Ok(Variant {
        compiler: compiler.clone(),
        shader,
        key: keyed(&form.key),
        bindings,
    })
}

/// What drawing points as squares made, its kind checked.
fn squares<M: MesaCompiler + Clone>(
    compiler: &M,
    made: wcontainers_mesa_compiler::Squares<M::Made>,
) -> Result<Squares<M>, ConvertError> {
    let owned = |shader: M::Made| Made {
        compiler: compiler.clone(),
        shader,
    };
    match made.kind {
        constants::SQUARES_ONE_PIXEL => Ok(Squares::OnePixel),
        constants::SQUARES_REFUSED => Ok(Squares::Refused),
        constants::SQUARES_MADE => Ok(Squares::Made {
            before: made.before.map(owned),
            geometry: made
                .geometry
                .map(owned)
                .ok_or(ConvertError::Unknown(SQUARES_KIND, made.kind))?,
        }),
        other => Err(ConvertError::Unknown(SQUARES_KIND, other)),
    }
}

impl<M: MesaCompiler + Clone> Prepared<M> {
    /// The driver's steps at a draw on a copy of the program, in its order: the position
    /// transformed, integer textures' samplings made nearest loads at the units of `samplers`, the
    /// textures at the units of `swizzles` swizzled, the first target's texels read from the
    /// settings' unit, the sample masks fitted, reads at offsets and samples made from the
    /// interpolated values; the key taken; the common form, its bindings in the set of `stage`.
    /// Refused by name where the program gathers an integer cube map, or reads at the positions of a
    /// sample count whose positions are not known (one, and the standard patterns of two, four, eight
    /// and sixteen, are).
    pub fn variant(
        &self,
        stage: Stage,
        settings: &DrawSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> Result<Variant<M>, ConvertError> {
        let form = self.compiler.prepared_variant(
            &self.shader,
            variant_settings(settings, stage),
            &bridge_swizzles(swizzles),
            &bridge_samplers(samplers),
        );
        variant(&self.compiler, form, stage, settings.samples)
    }

    /// A vertex program's points drawn as squares where the host draws points of one size: a
    /// copy writing its point size also at the generic location `at`, and the geometry program
    /// drawing each point as a square of two triangles, its size that point size clamped to
    /// 1..`largest` or else the first word of uniform block 0, whose next three are half a pixel's
    /// clip-space length over w in x and in y and the direction of `gl_PointCoord`'s t.
    pub fn points_as_squares(&self, at: u32, largest: f32) -> Result<Squares<M>, ConvertError> {
        let made = self
            .compiler
            .prepared_points_as_squares(&self.shader, at, largest);
        squares(&self.compiler, made)
    }

    /// A geometry program whose output is points rewritten to draw each as a square, as
    /// [`Self::points_as_squares`] draws them, its size else read from uniform block `block`.
    pub fn geometry_points_as_squares(
        &self,
        at: u32,
        largest: f32,
        block: u32,
    ) -> Result<Squares<M>, ConvertError> {
        let made =
            self.compiler
                .prepared_geometry_points_as_squares(&self.shader, at, largest, block);
        squares(&self.compiler, made)
    }
}

impl<M: MesaCompiler + Clone> Made<M> {
    /// The cache's key of the made program.
    pub fn key(&self) -> Result<[u8; blake3::OUT_LEN], ConvertError> {
        self.compiler
            .made_key(&self.shader)
            .map(|bytes| keyed(&bytes))
            .ok_or(ConvertError::Mesa(NIR_SERIALIZE))
    }

    /// For a made geometry program writing `gl_Layer`, the location after its generic outputs;
    /// `None` for one naming no layer.
    pub fn named_layer_location(&self) -> Option<u32> {
        let at = self.compiler.made_named_layer_location(&self.shader);
        (at != NO_NAMED_LAYER).then_some(at)
    }

    /// The driver's steps at a draw on the made program, as [`Prepared::variant`] takes them.
    pub fn variant(
        &self,
        stage: Stage,
        settings: &DrawSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> Result<Variant<M>, ConvertError> {
        let form = self.compiler.made_variant(
            &self.shader,
            variant_settings(settings, stage),
            &bridge_swizzles(swizzles),
            &bridge_samplers(samplers),
        );
        variant(&self.compiler, form, stage, settings.samples)
    }
}

impl<M: MesaCompiler + Clone> Variant<M> {
    /// The variant out of SSA and flattened, the route the driver converts it by, with its key and
    /// bindings; `points_in` a tessellation program's input control points (a control program's
    /// draw's patch size, an evaluation program's control program's output).
    pub fn flatten(self, points_in: u32) -> Result<Flattened, ConvertError> {
        let Variant {
            compiler,
            shader,
            key,
            bindings,
        } = self;
        let flat = compiler
            .variant_flatten(shader, points_in)
            .ok_or(ConvertError::Mesa(FLATTEN))?;
        Ok(Flattened {
            flat,
            key,
            bindings,
        })
    }
}

/// A variant flattened, with its key and bindings.
pub struct Flattened {
    pub flat: wcontainers_mesa_compiler::Flat,
    pub key: [u8; blake3::OUT_LEN],
    pub bindings: Vec<ReflectedBinding>,
}

/// The converter's version in the cache's key: this crate's version, a hash of its sources and of
/// the naga it writes, and the Mesa it links (commit and build options, from `mesa/build`'s
/// record).
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("WCONTAINERS_NIR_SOURCES"),
    "+mesa.",
    env!("WCONTAINERS_MESA_MADE_FROM")
);

#[cfg(feature = "standalone")]
/// Lavapipe, Mesa's Vulkan driver on the processor, built with the Mesa this crate links: the
/// reference the corpus run holds the routes to (plan 15.3).
pub const LAVAPIPE: &str = env!("WCONTAINERS_LAVAPIPE");

#[cfg(feature = "standalone")]
/// Mesa's EGL from the same build, through which llvmpipe, Mesa's OpenGL driver on the processor,
/// is reached: the reference for GLSL programs (plan 15.3).
pub const EGL: &str = env!("WCONTAINERS_EGL");

/// The steps named in `ConvertError::Mesa`.
pub const SPIRV_TO_NIR: &str = "spirv_read";
pub const NIR_SERIALIZE: &str = "nir_serialize";
pub const NIR_TO_SPIRV: &str = "spirv_common_to_spirv";

/// The enumerations named in `ConvertError::Unknown`.
pub const BIND_RESULT: &str = "enum wc_bind_result";
pub const VARIANT_RESULT: &str = "enum wc_variant_result";
pub const SQUARES_KIND: &str = "enum wc_squares_kind";
pub const SLOT_KIND: &str = "enum wc_slot_kind";
pub const GLSL_NAME_KIND: &str = "enum wc_glsl_name_kind";
pub const GLSL_INTERPOLATION: &str = "enum wc_glsl_interpolation";
