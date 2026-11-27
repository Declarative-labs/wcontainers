//! NIR from the pinned Mesa, owned, with the steps the routes take on it, through Mesa's compiler as
//! the `wcontainers-mesa-compiler` trait gives it (our bridge, `bridge/bridge.c`, behind it).

use crate::ConvertError;
use crate::bind::{self, KINDS};
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

/// One stage of a linked GLSL program.
pub struct GlslStage<M: MesaCompiler> {
    pub stage: Stage,
    pub nir: Nir<M>,
    pub names: Vec<GlslName>,
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

/// The bridge's answer for a streamed value the route places neither at a location nor as the
/// position.
const STREAMED_ELSEWHERE: u32 = u32::MAX;
/// `wc_nir_named_layer_location`'s answer for a program naming no layer.
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

/// The primitive a pass-through geometry program takes (`Nir::storage_pass_through`), in the
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

/// One shader's NIR, owned by Mesa's compiler `M` and given back when dropped.
pub struct Nir<M: MesaCompiler> {
    compiler: M,
    shader: M::Shader,
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

impl<M: MesaCompiler + Clone> Nir<M> {
    /// A shader the compiler made, owned with it.
    fn owned(&self, shader: M::Shader) -> Nir<M> {
        Nir {
            compiler: self.compiler.clone(),
            shader,
        }
    }

    /// Route B's flat form of the shader (`wc_nir_flatten`), for a tessellation program whose
    /// input patches have `points_in` control points; `None` when the bridge cannot flatten it.
    pub(crate) fn flatten(&mut self, points_in: u32) -> Option<wcontainers_mesa_compiler::Flat> {
        self.compiler.flatten(&mut self.shader, points_in)
    }

    /// A copy of the shader, owned apart from it; None when Mesa runs out of memory.
    pub fn try_clone(&self) -> Option<Nir<M>> {
        let copy = self.compiler.cloned(&self.shader)?;
        Some(self.owned(copy))
    }

    /// A vertex program's inputs numbered as st/mesa hands its vertex elements over: each
    /// attribute at its rank among those read. Returns the attribute slots read before.
    pub fn number_vertex_elements(&mut self) -> u64 {
        self.compiler.number_vertex_elements(&mut self.shader)
    }

    /// The most vertices a geometry program emits an invocation; zero for another stage.
    pub fn geometry_vertices_out(&self) -> u32 {
        self.compiler.geometry_vertices_out(&self.shader)
    }

    /// For a geometry program writing `gl_Layer`, the location after its generic outputs, where
    /// its pass-through can pass the layer it names on; `None` for one that writes none.
    pub fn named_layer_location(&self) -> Option<u32> {
        let at = self.compiler.named_layer_location(&self.shader);
        (at != NO_NAMED_LAYER).then_some(at)
    }

    /// The control points of a tessellation control program's output patch; zero for another
    /// stage.
    pub fn control_points_out(&self) -> u32 {
        self.compiler.control_points_out(&self.shader)
    }

    /// An evaluation program in point mode, or a geometry program whose output is points.
    pub fn makes_points(&self) -> bool {
        self.compiler.makes_points(&self.shader)
    }

    /// gl_FragColor written to each of the first `buffers` draw buffers. Nothing for a program
    /// that does not write it, or of another stage.
    pub fn fragcolor_to_buffers(&mut self, buffers: u32) {
        self.compiler
            .fragcolor_to_buffers(&mut self.shader, buffers);
    }

    /// Arrays of textures and samplers read only at constant indices as one variable an
    /// element, at the element's binding.
    pub fn split_resource_arrays(&mut self) {
        self.compiler.split_resource_arrays(&mut self.shader);
    }

    /// Every 1D texture the program samples as a 2D texture of one row, for a driver making its 1D
    /// textures 2D.
    pub fn textures_1d_as_2d(&mut self) {
        self.compiler.textures_1d_as_2d(&mut self.shader);
    }

    /// The program's name and label dropped (st/mesa names each by the API program's number), so
    /// that programs of equal content have one [`key`](Nir::key).
    pub fn drop_name(&mut self) {
        self.compiler.drop_name(&mut self.shader);
    }

    /// Every uniform block access st/mesa hands over (`load_ubo`, the default uniforms as block
    /// 0) as derefs of one array of words a block, each sized by its block's declared type.
    pub fn uniform_blocks_as_words(&mut self) {
        self.compiler.uniform_blocks_as_words(&mut self.shader);
    }

    /// Every result read from the textures at the units listed rearranged by their swizzle.
    pub fn swizzle_textures(&mut self, swizzles: &[TextureSwizzle]) {
        let swizzles: Vec<wcontainers_mesa_compiler::TextureSwizzle> = swizzles
            .iter()
            .map(|s| wcontainers_mesa_compiler::TextureSwizzle {
                unit: s.unit,
                swizzle: s.swizzle.map(|c| c as u8),
            })
            .collect();
        self.compiler.swizzle_textures(&mut self.shader, &swizzles);
    }

    /// Every sampling of an integer texture at the units of `samplers` made a load of its
    /// nearest texel, as OpenGL samples them; a sampled cube map or cube map array becomes a 2D
    /// array of its faces, six a cube, which the driver binds such a view of. False where the
    /// program also gathers such a cube: its gathers then read their own face's edge texels past
    /// the edge, not the next face's, which is not built.
    #[must_use]
    pub fn nearest_integer_textures(&mut self, samplers: &[NearestSampler]) -> bool {
        let samplers: Vec<wcontainers_mesa_compiler::NearestSampler> = samplers
            .iter()
            .map(|s| wcontainers_mesa_compiler::NearestSampler {
                unit: s.unit,
                wrap: s.wrap.map(|w| w as u8),
                mipmapped: s.mipmapped,
                min_lod: f32::from_bits(s.min_lod),
                max_lod: f32::from_bits(s.max_lod),
                border: s.border,
            })
            .collect();
        self.compiler
            .nearest_integer_textures(&mut self.shader, &samplers)
    }

    /// The texture units the program samples integers at (not loads, gathers or queries), a bit
    /// each: an integer texture there is sampled by [`Self::nearest_integer_textures`].
    pub fn integer_sampled_units(&mut self) -> u64 {
        self.compiler.integer_sampled_units(&mut self.shader)
    }

    /// A streamed point size (a built-in the common form keeps and the stage helper does not
    /// stream) also written at a location no output takes, which the stream then names.
    pub fn stream_point_size_at_a_location(&mut self) {
        self.compiler
            .stream_point_size_at_a_location(&mut self.shader);
    }

    /// Where the generic locations a fragment program reads, or another stage's program writes,
    /// end.
    pub fn generic_end(&self) -> u32 {
        self.compiler.generic_end(&self.shader)
    }

    /// A vertex program's point size also written at the generic location `at`; false where it
    /// writes none.
    pub fn point_size_also_at(&mut self, at: u32) -> bool {
        self.compiler.point_size_also_at(&mut self.shader, at)
    }

    /// Every point size the program writes is a constant no larger than `size`; false where it
    /// writes none. Read after [`Self::point_size_also_at`], which lowers its outputs.
    pub fn point_size_at_most(&self, size: f32) -> bool {
        self.compiler.point_size_at_most(&self.shader, size)
    }

    /// A geometry program drawing each point of this vertex program as a square of two triangles
    /// (`SlowPath::PointsAsQuads`), each corner with every output and `gl_PointCoord` at the
    /// generic location `at`. Its size is the point size the program also writes at `at`
    /// ([`Self::point_size_also_at`]), clamped to 1..`largest`, or else the first word of uniform
    /// block 0, whose next three are half a pixel's clip-space length over w in x and in y (signed
    /// as the viewport's scale) and the direction of `gl_PointCoord`'s t (1 where it grows with the
    /// rows, -1 where it falls). `None` for a program writing clip distances or outputs at offsets
    /// known only when run.
    pub fn point_square(&self, at: u32, largest: f32) -> Option<Nir<M>> {
        let square = self.compiler.point_square(&self.shader, at, largest)?;
        Some(self.owned(square))
    }

    /// This geometry program, whose output is points and whose uniform blocks are words
    /// ([`Self::uniform_blocks_as_words`]), rewritten to draw each as a square of two triangles as
    /// [`Self::point_square`] draws them (its new block in words too): its size the point size it writes, clamped
    /// to 1..`largest`, or else the first word of uniform block `block`, whose words are
    /// [`Self::point_square`]'s block 0's; `gl_PointCoord` at the generic location `at`. `None`
    /// for a program of another output, writing clip distances, outputs at offsets known only
    /// when run or at `at`, or to more than one stream.
    pub fn geometry_squares(&self, at: u32, largest: f32, block: u32) -> Option<Nir<M>> {
        let squares = self
            .compiler
            .geometry_squares(&self.shader, at, largest, block)?;
        Some(self.owned(squares))
    }

    /// For a vertex or evaluation program using storage buffers or images, which runs as compute
    /// work before a geometry program passing each of its primitives' own vertices on unchanged,
    /// every output as written: those geometry programs, by the primitive taken (`Assembled`),
    /// with this program's point size also written at the generic location after its outputs,
    /// where the geometry program reads it and does not pass it on. An evaluation program has
    /// one, for the primitive it makes. `None` for a program of another stage or using no
    /// storage; an entry is `None` for one writing outputs at offsets known only when run, or a
    /// point size the geometry program cannot read.
    pub fn storage_pass_through(&mut self) -> Option<[Option<Nir<M>>; Assembled::ALL.len()]> {
        let made = self.compiler.storage_pass_through(&mut self.shader)?;
        Some(made.map(|geometry| geometry.map(|shader| self.owned(shader))))
    }

    /// The outputs the program streams (its transform feedback, none without it) and each of the
    /// first `N` buffers' bytes a vertex; `None` when one is a value the route places neither at a
    /// location nor as the position.
    pub fn stream_outputs<const N: usize>(&self) -> Option<(Vec<StreamedOutput>, [u32; N])> {
        let mut strides = [0; N];
        let count = self
            .compiler
            .stream_outputs(&self.shader, &mut [], &mut strides);
        if count == STREAMED_ELSEWHERE {
            return None;
        }
        let mut outputs: Vec<wcontainers_mesa_compiler::StreamOutput> = (0..count)
            .map(|_| wcontainers_mesa_compiler::StreamOutput {
                buffer: 0,
                offset: 0,
                location: 0,
                position: false,
                first_component: 0,
                components: 0,
            })
            .collect();
        let written = self
            .compiler
            .stream_outputs(&self.shader, &mut outputs, &mut strides);
        (written != STREAMED_ELSEWHERE).then(|| {
            let outputs = outputs
                .into_iter()
                .map(|o| StreamedOutput {
                    buffer: o.buffer,
                    offset: o.offset,
                    location: o.location,
                    position: o.position,
                    first_component: o.first_component,
                    components: o.components,
                })
                .collect();
            (outputs, strides)
        })
    }

    /// Whether a fragment program reads where its samples are (`gl_SamplePosition`,
    /// `interpolateAtSample`, `interpolateAtOffset` of a value read per sample), which depends on
    /// the targets' sample count.
    pub fn reads_sample_positions(&mut self) -> bool {
        self.compiler.reads_sample_positions(&mut self.shader)
    }

    /// A fragment program's reads of inputs at an offset or at a sample, and of the sample
    /// position, made from the values the common form interpolates and their slopes, for targets
    /// of `samples` samples. False where that count's positions are needed and not known (one,
    /// and the standard patterns of two, four, eight and sixteen, are).
    pub fn interpolate_elsewhere(&mut self, samples: u32) -> bool {
        self.compiler
            .interpolate_elsewhere(&mut self.shader, samples)
    }

    /// A fragment program's sample masks as OpenGL defines them: `gl_SampleMaskIn` of a program
    /// run per sample holds only that sample's bit (a host may give every covered sample's), and
    /// with `single_sampled` targets its writes of `gl_SampleMask` are dropped (OpenGL applies
    /// the mask only as a multisample operation; a host applies its first bit).
    pub fn fit_sample_masks(&mut self, single_sampled: bool) {
        self.compiler
            .fit_sample_masks(&mut self.shader, single_sampled);
    }

    /// Whether a fragment program writes `gl_SampleMask`, which depends on the targets' sample
    /// count.
    pub fn writes_sample_mask(&mut self) -> bool {
        self.compiler.writes_sample_mask(&mut self.shader)
    }

    /// Whether a fragment program reads its first target's texels (framebuffer fetch, which
    /// Mesa's lowering of `KHR_blend_equation_advanced` writes).
    pub fn fetches_target(&mut self) -> bool {
        self.compiler.fetches_target(&mut self.shader)
    }

    /// The image units the program declares images at without a format, a bit each: their stores
    /// take the bound image's format, which the variant converting it names.
    pub fn formatless_images(&mut self) -> u64 {
        self.compiler.formatless_images(&mut self.shader)
    }

    /// A fragment program's reads of its first target's texels made loads of the texture at
    /// `unit`, a copy of the target the driver binds there, multisampled where the target is.
    pub fn fetch_from_texture(&mut self, unit: u32, multisampled: bool) {
        self.compiler
            .fetch_from_texture(&mut self.shader, unit, multisampled);
    }

    /// A vertex, geometry or tessellation evaluation program's position changed by `transform`.
    /// Nothing for another stage.
    pub fn transform_position(&mut self, transform: PositionTransform) {
        let transform = wcontainers_mesa_compiler::PositionTransform {
            flip_y: transform.flip_y,
            half_z: transform.half_z,
            reverse_z: transform.reverse_z,
            fixed_point_size: transform.fixed_point_size,
        };
        self.compiler
            .transform_position(&mut self.shader, transform);
    }

    #[cfg(feature = "standalone")]
    /// The NIR of one entry point of a Vulkan SPIR-V module.
    pub fn from_spirv(
        words: &[u32],
        stage: Stage,
        entry_point: &str,
    ) -> Result<Nir<M>, ConvertError> {
        let compiler = M::new();
        let entry = CString::new(entry_point).map_err(|_| ConvertError::Mesa(SPIRV_TO_NIR))?;
        let shader = compiler
            .spirv_to_nir(words, stage.bridge(), &entry)
            .ok_or(ConvertError::Mesa(SPIRV_TO_NIR))?;
        Ok(Nir { compiler, shader })
    }

    #[cfg(feature = "standalone")]
    /// The stages of one GLSL program as the OpenGL driver receives them: compiled and linked by
    /// Mesa's GLSL compiler, then st/mesa's steps for the driver's capabilities (`bridge/glsl.cpp`),
    /// each buffer an array of words; with each stage's names. One entry a source, in
    /// `Stage::ALL`'s order.
    pub fn from_glsl(sources: &[(Stage, &str)]) -> Result<Vec<GlslStage<M>>, ConvertError> {
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
                        nir: Nir {
                            compiler: compiler.clone(),
                            shader,
                        },
                        names: names.into_iter().map(glsl_name).collect::<Result<_, _>>()?,
                    })
                },
            )
            .collect()
    }

    /// Gives the program of `stage` the common form's meaning (sampled array layers rounded to
    /// integers) and its binding rule ([`crate::bind`]), splitting combined image samplers, and gives
    /// each resource's reflection.
    pub fn to_common_form(&mut self, stage: Stage) -> Result<Vec<ReflectedBinding>, ConvertError> {
        let set = bind::stage_set(stage.shader_stage());
        let made = self
            .compiler
            .to_common_form(&mut self.shader, set, &slot_ranges())
            .map_err(|code| match code {
                constants::BIND_BEYOND_LIMIT => ConvertError::Bindings(bind::BEYOND_LIMIT),
                constants::BIND_UNBOUNDED => ConvertError::Bindings(bind::UNBOUNDED),
                constants::BIND_OUT_OF_MEMORY => ConvertError::Bindings(bind::OUT_OF_MEMORY),
                other => ConvertError::Unknown(BIND_RESULT, other),
            })?;
        made.iter()
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

    /// Vulkan's `alphaToOne` and OpenGL's `GL_SAMPLE_ALPHA_TO_ONE` made in a fragment program (plan
    /// 15.4, `AlphaToOneInShader`): Mesa's alpha to coverage demotes the samples alpha does not
    /// cover, then every colour output's alpha is one. Nothing for another stage.
    pub fn alpha_to_one(&mut self) {
        self.compiler.alpha_to_one(&mut self.shader);
    }

    /// The cache's key of what a route converts from this NIR (plan 15.3): BLAKE3 of NIR's serialised
    /// bytes and the converter's version ([`CONVERTER_VERSION`]). Taken before the route changes the
    /// shader.
    pub fn key(&self) -> Result<[u8; blake3::OUT_LEN], ConvertError> {
        let bytes = self.serialized()?;
        Ok(*blake3::Hasher::new()
            .update(&bytes)
            .update(CONVERTER_VERSION.as_bytes())
            .finalize()
            .as_bytes())
    }

    /// NIR's serialised bytes, the cache's key with the converter's version (plan 15.3).
    pub fn serialized(&self) -> Result<Vec<u8>, ConvertError> {
        self.compiler
            .serialize(&self.shader)
            .ok_or(ConvertError::Mesa(NIR_SERIALIZE))
    }

    #[cfg(feature = "standalone")]
    /// Candidate A's first step: SPIR-V written by Zink's compiler (the shader is prepared for it
    /// in place, so it is consumed).
    pub fn into_spirv(mut self) -> Result<Vec<u32>, ConvertError> {
        self.compiler
            .to_spirv(&mut self.shader)
            .ok_or(ConvertError::Mesa(NIR_TO_SPIRV))
    }
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
pub const SPIRV_TO_NIR: &str = "spirv_to_nir";
pub const NIR_SERIALIZE: &str = "nir_serialize";
pub const NIR_TO_SPIRV: &str = "nir_to_spirv";

/// The enumerations named in `ConvertError::Unknown`.
pub const BIND_RESULT: &str = "enum wc_bind_result";
pub const SLOT_KIND: &str = "enum wc_slot_kind";
pub const GLSL_NAME_KIND: &str = "enum wc_glsl_name_kind";
pub const GLSL_INTERPOLATION: &str = "enum wc_glsl_interpolation";
