//! NIR from the pinned Mesa, owned, with the steps the routes take on it (through `bridge/bridge.c`).

use crate::ConvertError;
use crate::bind::{self, KINDS};
#[cfg(feature = "standalone")]
use std::ffi::{CStr, CString, c_char};
#[cfg(feature = "standalone")]
use std::sync::Once;
use wcontainers_gpu::{ReflectedBinding, ShaderStage};

/// The program stages the routes take (`enum wc_stage`).
#[repr(C)]
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
}

#[repr(C)]
struct SlotRange {
    first: u32,
    slots: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Binding {
    kind: u32,
    set: u32,
    binding: u32,
    count: u32,
    new_binding: u32,
    stride: u32,
}

#[repr(C)]
struct Bindings {
    data: *mut Binding,
    count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum BindResult {
    Ok,
    BeyondLimit,
    Unbounded,
    OutOfMemory,
}

#[repr(C)]
pub(crate) struct NirShader {
    _opaque: [u8; 0],
}

#[repr(C)]
struct Bytes {
    data: *mut u8,
    size: usize,
}

/// What a name of a GLSL program, or a state value, is in one stage (`enum wc_glsl_name_kind`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
pub struct GlslStage {
    pub stage: Stage,
    pub nir: Nir,
    pub names: Vec<GlslName>,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct RawGlslName {
    kind: GlslNameKind,
    name: *mut c_char,
    ty: *mut c_char,
    interpolation: Interpolation,
    at: u32,
    count: u32,
    element: u32,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct RawGlslNames {
    data: *mut RawGlslName,
    count: usize,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct GlslProgram {
    stages: [*mut NirShader; Stage::ALL.len()],
    names: [RawGlslNames; Stage::ALL.len()],
    log: *mut c_char,
}

#[cfg(feature = "standalone")]
/// A string the bridge owns, copied.
fn owned_string(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the bridge's strings are NUL-terminated until the program is freed.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct Words {
    data: *mut u32,
    count: usize,
}

unsafe extern "C" {
    fn wc_compiler_options() -> *const std::ffi::c_void;
    fn wc_nir_to_common_form(
        nir: *mut NirShader,
        set: u32,
        ranges: *const SlotRange,
        out: *mut Bindings,
    ) -> BindResult;
    fn wc_bindings_free(bindings: *mut Bindings);
    fn wc_nir_alpha_to_one(nir: *mut NirShader);
    fn wc_nir_serialize(nir: *const NirShader, out: *mut Bytes) -> bool;
    fn wc_nir_free(nir: *mut NirShader);
    fn wc_bytes_free(bytes: *mut Bytes);
    fn wc_nir_clone(nir: *const NirShader) -> *mut NirShader;
    fn wc_nir_number_vertex_elements(nir: *mut NirShader) -> u64;
    fn wc_nir_geometry_vertices_out(nir: *const NirShader) -> u32;
    fn wc_nir_control_points_out(nir: *const NirShader) -> u32;
    fn wc_nir_makes_points(nir: *const NirShader) -> bool;
    fn wc_nir_named_layer_location(nir: *const NirShader) -> u32;
    fn wc_nir_fragcolor_to_buffers(nir: *mut NirShader, buffers: u32);
    fn wc_nir_uniform_blocks_as_words(nir: *mut NirShader);
    fn wc_nir_split_resource_arrays(nir: *mut NirShader);
    fn wc_nir_textures_1d_as_2d(nir: *mut NirShader);
    fn wc_nir_transform_position(nir: *mut NirShader, transform: PositionTransform);
    fn wc_nir_reads_sample_positions(nir: *mut NirShader) -> bool;
    fn wc_nir_interpolate_elsewhere(nir: *mut NirShader, samples: u32) -> bool;
    fn wc_nir_fit_sample_masks(nir: *mut NirShader, single_sampled: bool);
    fn wc_nir_writes_sample_mask(nir: *mut NirShader) -> bool;
    fn wc_nir_fetches_target(nir: *mut NirShader) -> bool;
    fn wc_nir_formatless_images(nir: *mut NirShader) -> u64;
    fn wc_nir_fetch_from_texture(nir: *mut NirShader, unit: u32, multisampled: bool);
    fn wc_nir_swizzle_textures(nir: *mut NirShader, swizzles: *const TextureSwizzle, count: usize);
    fn wc_nir_nearest_integer_textures(
        nir: *mut NirShader,
        samplers: *const NearestSampler,
        count: usize,
    );
    fn wc_nir_integer_sampled_units(nir: *mut NirShader) -> u64;
    fn wc_nir_stream_point_size_at_a_location(nir: *mut NirShader);
    fn wc_nir_generic_end(nir: *const NirShader) -> u32;
    fn wc_nir_point_size_also_at(nir: *mut NirShader, at: u32) -> bool;
    fn wc_nir_point_size_at_most(nir: *const NirShader, size: f32) -> bool;
    fn wc_nir_point_square(vs: *const NirShader, at: u32, largest: f32) -> *mut NirShader;
    fn wc_nir_geometry_squares(
        gs: *const NirShader,
        at: u32,
        largest: f32,
        block: u32,
    ) -> *mut NirShader;
    fn wc_nir_stream_outputs(
        nir: *const NirShader,
        outputs: *mut StreamedOutput,
        capacity: u32,
        strides: *mut u32,
        buffers: u32,
    ) -> u32;
}

/// One output a vertex program streams (`struct wc_stream_output`): its buffer, its byte offset in
/// each vertex there, where the route places it (a generic location, or the position), and its
/// components.
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
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

/// How a vertex program's position (and point size) is changed to meet the host's clip space,
/// viewport and points (`struct wc_position_transform`).
#[repr(C)]
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

#[cfg(feature = "standalone")]
unsafe extern "C" {
    fn wc_init();
    fn wc_spirv_to_nir(
        words: *const u32,
        count: usize,
        stage: Stage,
        entry_point: *const c_char,
    ) -> *mut NirShader;
    fn wc_glsl_to_nir(sources: *const *const c_char, out: *mut GlslProgram) -> bool;
    fn wc_glsl_program_free(program: *mut GlslProgram);
    fn wc_nir_to_spirv(nir: *mut NirShader, out: *mut Words) -> bool;
    fn wc_words_free(words: *mut Words);
}

/// The `nir_shader_compiler_options` every route takes, a static of the process: a Mesa driver
/// reports them so that the NIR it is handed has the routes' shape.
pub fn compiler_options() -> *const std::ffi::c_void {
    // SAFETY: the bridge returns a pointer to a static.
    unsafe { wc_compiler_options() }
}

/// One shader's NIR, freed on drop.
pub struct Nir(*mut NirShader);

#[cfg(feature = "standalone")]
/// Mesa's type tables, held for the life of the process (Mesa counts their references).
fn initialised() {
    static INIT: Once = Once::new();
    // SAFETY: wc_init only takes a reference on Mesa's process-wide type tables.
    INIT.call_once(|| unsafe { wc_init() });
}

impl Nir {
    pub(crate) fn raw(&mut self) -> *mut NirShader {
        self.0
    }

    /// Takes the NIR a Mesa driver was handed (a Gallium shader state's `ir.nir`), freed on drop.
    ///
    /// # Safety
    /// `nir` is a live `nir_shader` the caller owns and does not use again.
    pub unsafe fn from_raw(nir: *mut std::ffi::c_void) -> Nir {
        Nir(nir.cast())
    }

    /// A copy of the shader, owned apart from it; None when Mesa runs out of memory.
    pub fn try_clone(&self) -> Option<Nir> {
        // SAFETY: self.0 is a live shader; the clone is a new allocation.
        let copy = unsafe { wc_nir_clone(self.0) };
        (!copy.is_null()).then_some(Nir(copy))
    }

    /// A vertex program's inputs numbered as st/mesa hands its vertex elements over: each
    /// attribute at its rank among those read. Returns the attribute slots read before.
    pub fn number_vertex_elements(&mut self) -> u64 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_number_vertex_elements(self.0) }
    }

    /// The most vertices a geometry program emits an invocation; zero for another stage.
    pub fn geometry_vertices_out(&self) -> u32 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_geometry_vertices_out(self.0) }
    }

    /// For a geometry program writing `gl_Layer`, the location after its generic outputs, where
    /// its pass-through can pass the layer it names on; `None` for one that writes none.
    pub fn named_layer_location(&self) -> Option<u32> {
        // SAFETY: self.0 is a live shader.
        let at = unsafe { wc_nir_named_layer_location(self.0) };
        (at != NO_NAMED_LAYER).then_some(at)
    }

    /// The control points of a tessellation control program's output patch; zero for another
    /// stage.
    pub fn control_points_out(&self) -> u32 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_control_points_out(self.0) }
    }

    /// An evaluation program in point mode, or a geometry program whose output is points.
    pub fn makes_points(&self) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_makes_points(self.0) }
    }

    /// gl_FragColor written to each of the first `buffers` draw buffers. Nothing for a program
    /// that does not write it, or of another stage.
    pub fn fragcolor_to_buffers(&mut self, buffers: u32) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_fragcolor_to_buffers(self.0, buffers) }
    }

    /// Arrays of textures and samplers read only at constant indices as one variable an
    /// element, at the element's binding.
    pub fn split_resource_arrays(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_split_resource_arrays(self.0) }
    }

    /// Every 1D texture the program samples as a 2D texture of one row, for a driver making its 1D
    /// textures 2D.
    pub fn textures_1d_as_2d(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_textures_1d_as_2d(self.0) }
    }

    /// Every uniform block access st/mesa hands over (`load_ubo`, the default uniforms as block
    /// 0) as derefs of one array of words a block, each sized by its block's declared type.
    pub fn uniform_blocks_as_words(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_uniform_blocks_as_words(self.0) }
    }

    /// Every result read from the textures at the units listed rearranged by their swizzle.
    pub fn swizzle_textures(&mut self, swizzles: &[TextureSwizzle]) {
        // SAFETY: self.0 is a live shader; the bridge reads the list during the call.
        unsafe { wc_nir_swizzle_textures(self.0, swizzles.as_ptr(), swizzles.len()) }
    }

    /// Every sampling of an integer texture at the units of `samplers` made a load of its
    /// nearest texel, as OpenGL samples them; a sampled cube map or cube map array becomes a 2D
    /// array of its faces, six a cube, which the driver binds such a view of.
    pub fn nearest_integer_textures(&mut self, samplers: &[NearestSampler]) {
        // SAFETY: self.0 is a live shader; the bridge reads the list during the call.
        unsafe { wc_nir_nearest_integer_textures(self.0, samplers.as_ptr(), samplers.len()) }
    }

    /// The texture units the program samples integers at (not loads, gathers or queries), a bit
    /// each: an integer texture there is sampled by [`Self::nearest_integer_textures`].
    pub fn integer_sampled_units(&mut self) -> u64 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_integer_sampled_units(self.0) }
    }

    /// A streamed point size (a built-in the common form keeps and the stage helper does not
    /// stream) also written at a location no output takes, which the stream then names.
    pub fn stream_point_size_at_a_location(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_stream_point_size_at_a_location(self.0) }
    }

    /// Where the generic locations a fragment program reads, or another stage's program writes,
    /// end.
    pub fn generic_end(&self) -> u32 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_generic_end(self.0) }
    }

    /// A vertex program's point size also written at the generic location `at`; false where it
    /// writes none.
    pub fn point_size_also_at(&mut self, at: u32) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_point_size_also_at(self.0, at) }
    }

    /// Every point size the program writes is a constant no larger than `size`; false where it
    /// writes none. Read after [`Self::point_size_also_at`], which lowers its outputs.
    pub fn point_size_at_most(&self, size: f32) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_point_size_at_most(self.0, size) }
    }

    /// A geometry program drawing each point of this vertex program as a square of two triangles
    /// (`SlowPath::PointsAsQuads`), each corner with every output and `gl_PointCoord` at the
    /// generic location `at`. Its size is the point size the program also writes at `at`
    /// ([`Self::point_size_also_at`]), clamped to 1..`largest`, or else the first word of uniform
    /// block 0, whose next three are half a pixel's clip-space length over w in x and in y (signed
    /// as the viewport's scale) and the direction of `gl_PointCoord`'s t (1 where it grows with the
    /// rows, -1 where it falls). `None` for a program writing clip distances or outputs at offsets
    /// known only when run.
    pub fn point_square(&self, at: u32, largest: f32) -> Option<Nir> {
        // SAFETY: self.0 is a live shader; the result is a new shader or null.
        let square = unsafe { wc_nir_point_square(self.0, at, largest) };
        (!square.is_null()).then_some(Nir(square))
    }

    /// This geometry program, whose output is points and whose uniform blocks are words
    /// ([`Self::uniform_blocks_as_words`]), rewritten to draw each as a square of two triangles as
    /// [`Self::point_square`] draws them (its new block in words too): its size the point size it writes, clamped
    /// to 1..`largest`, or else the first word of uniform block `block`, whose words are
    /// [`Self::point_square`]'s block 0's; `gl_PointCoord` at the generic location `at`. `None`
    /// for a program of another output, writing clip distances, outputs at offsets known only
    /// when run or at `at`, or to more than one stream.
    pub fn geometry_squares(&self, at: u32, largest: f32, block: u32) -> Option<Nir> {
        // SAFETY: self.0 is a live shader; the result is a new shader or null.
        let squares = unsafe { wc_nir_geometry_squares(self.0, at, largest, block) };
        (!squares.is_null()).then_some(Nir(squares))
    }

    /// The outputs the program streams (its transform feedback, none without it) and each of the
    /// first `N` buffers' bytes a vertex; `None` when one is a value the route places neither at a
    /// location nor as the position.
    pub fn stream_outputs<const N: usize>(&self) -> Option<(Vec<StreamedOutput>, [u32; N])> {
        let mut strides = [0; N];
        let buffers = N as u32;
        // SAFETY: self.0 is a live shader; the bridge writes at most the capacities given.
        let count = unsafe {
            wc_nir_stream_outputs(
                self.0,
                std::ptr::null_mut(),
                0,
                strides.as_mut_ptr(),
                buffers,
            )
        };
        let mut outputs = vec![StreamedOutput::default(); count as usize];
        // SAFETY: as above.
        let written = unsafe {
            wc_nir_stream_outputs(
                self.0,
                outputs.as_mut_ptr(),
                count,
                strides.as_mut_ptr(),
                buffers,
            )
        };
        (count != STREAMED_ELSEWHERE && written != STREAMED_ELSEWHERE).then_some((outputs, strides))
    }

    /// Whether a fragment program reads where its samples are (`gl_SamplePosition`,
    /// `interpolateAtSample`, `interpolateAtOffset` of a value read per sample), which depends on
    /// the targets' sample count.
    pub fn reads_sample_positions(&mut self) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_reads_sample_positions(self.0) }
    }

    /// A fragment program's reads of inputs at an offset or at a sample, and of the sample
    /// position, made from the values the common form interpolates and their slopes, for targets
    /// of `samples` samples. False where that count's positions are needed and not known (one,
    /// and the standard patterns of two, four, eight and sixteen, are).
    pub fn interpolate_elsewhere(&mut self, samples: u32) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_interpolate_elsewhere(self.0, samples) }
    }

    /// A fragment program's sample masks as OpenGL defines them: `gl_SampleMaskIn` of a program
    /// run per sample holds only that sample's bit (a host may give every covered sample's), and
    /// with `single_sampled` targets its writes of `gl_SampleMask` are dropped (OpenGL applies
    /// the mask only as a multisample operation; a host applies its first bit).
    pub fn fit_sample_masks(&mut self, single_sampled: bool) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_fit_sample_masks(self.0, single_sampled) }
    }

    /// Whether a fragment program writes `gl_SampleMask`, which depends on the targets' sample
    /// count.
    pub fn writes_sample_mask(&mut self) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_writes_sample_mask(self.0) }
    }

    /// Whether a fragment program reads its first target's texels (framebuffer fetch, which
    /// Mesa's lowering of `KHR_blend_equation_advanced` writes).
    pub fn fetches_target(&mut self) -> bool {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_fetches_target(self.0) }
    }

    /// The image units the program declares images at without a format, a bit each: their stores
    /// take the bound image's format, which the variant converting it names.
    pub fn formatless_images(&mut self) -> u64 {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_formatless_images(self.0) }
    }

    /// A fragment program's reads of its first target's texels made loads of the texture at
    /// `unit`, a copy of the target the driver binds there, multisampled where the target is.
    pub fn fetch_from_texture(&mut self, unit: u32, multisampled: bool) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_fetch_from_texture(self.0, unit, multisampled) }
    }

    /// A vertex, geometry or tessellation evaluation program's position changed by `transform`.
    /// Nothing for another stage.
    pub fn transform_position(&mut self, transform: PositionTransform) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_transform_position(self.0, transform) }
    }

    #[cfg(feature = "standalone")]
    /// The NIR of one entry point of a Vulkan SPIR-V module.
    pub fn from_spirv(words: &[u32], stage: Stage, entry_point: &str) -> Result<Nir, ConvertError> {
        initialised();
        let entry = CString::new(entry_point).map_err(|_| ConvertError::Mesa(SPIRV_TO_NIR))?;
        // SAFETY: the words and the name outlive the call; Mesa copies what it keeps.
        let nir = unsafe { wc_spirv_to_nir(words.as_ptr(), words.len(), stage, entry.as_ptr()) };
        if nir.is_null() {
            return Err(ConvertError::Mesa(SPIRV_TO_NIR));
        }
        Ok(Nir(nir))
    }

    #[cfg(feature = "standalone")]
    /// The stages of one GLSL program as the OpenGL driver receives them: compiled and linked by
    /// Mesa's GLSL compiler, then st/mesa's steps for the driver's capabilities (`bridge/glsl.cpp`),
    /// each buffer an array of words; with each stage's names. One entry a source, in
    /// `Stage::ALL`'s order.
    pub fn from_glsl(sources: &[(Stage, &str)]) -> Result<Vec<GlslStage>, ConvertError> {
        initialised();
        let owned = sources
            .iter()
            .map(|(stage, text)| Ok((*stage, CString::new(*text)?)))
            .collect::<Result<Vec<_>, std::ffi::NulError>>()
            .map_err(|e| ConvertError::Glsl(e.to_string()))?;
        let pointers: Vec<*const c_char> = Stage::ALL
            .iter()
            .map(|stage| {
                owned
                    .iter()
                    .find(|(s, _)| s == stage)
                    .map_or(std::ptr::null(), |(_, text)| text.as_ptr())
            })
            .collect();
        let mut program = GlslProgram {
            stages: [std::ptr::null_mut(); Stage::ALL.len()],
            names: std::array::from_fn(|_| RawGlslNames {
                data: std::ptr::null_mut(),
                count: 0,
            }),
            log: std::ptr::null_mut(),
        };
        // SAFETY: one source pointer a stage (or null), each alive for the call; the bridge fills
        // `program` and Mesa copies what it keeps.
        let linked = unsafe { wc_glsl_to_nir(pointers.as_ptr(), &mut program) };
        let log = owned_string(program.log);
        let stages = Stage::ALL
            .iter()
            .zip(program.stages.iter_mut().zip(&program.names))
            .filter(|(_, (nir, _))| !nir.is_null())
            .map(|(stage, (nir, names))| {
                let names = if names.data.is_null() {
                    &[][..]
                } else {
                    // SAFETY: the bridge's array of `count` names, alive until the program is freed.
                    unsafe { std::slice::from_raw_parts(names.data, names.count) }
                };
                GlslStage {
                    stage: *stage,
                    nir: Nir(std::mem::replace(nir, std::ptr::null_mut())),
                    names: names
                        .iter()
                        .map(|n| GlslName {
                            kind: n.kind,
                            name: owned_string(n.name),
                            ty: (!n.ty.is_null()).then(|| owned_string(n.ty)),
                            interpolation: n.interpolation,
                            at: n.at,
                            count: n.count,
                            element: n.element,
                        })
                        .collect(),
                }
            })
            .collect();
        // SAFETY: program came from the bridge; the shaders taken above are no longer in it.
        unsafe { wc_glsl_program_free(&mut program) };
        if linked {
            Ok(stages)
        } else {
            Err(ConvertError::Glsl(log))
        }
    }

    /// Gives the program of `stage` the common form's meaning (sampled array layers rounded to
    /// integers) and its binding rule ([`crate::bind`]), splitting combined image samplers, and gives
    /// each resource's reflection.
    pub fn to_common_form(&mut self, stage: Stage) -> Result<Vec<ReflectedBinding>, ConvertError> {
        let ranges = KINDS.map(|k| SlotRange {
            first: k.first_binding(),
            slots: k.slots(),
        });
        let set = bind::stage_set(stage.shader_stage());
        let mut out = Bindings {
            data: std::ptr::null_mut(),
            count: 0,
        };
        // SAFETY: self.0 is a live shader, ranges holds one range a kind, and the bridge fills `out`.
        let result = unsafe { wc_nir_to_common_form(self.0, set, ranges.as_ptr(), &mut out) };
        let made: Vec<ReflectedBinding> = if out.data.is_null() {
            Vec::new()
        } else {
            // SAFETY: data holds count entries until wc_bindings_free.
            unsafe { std::slice::from_raw_parts(out.data, out.count) }
                .iter()
                .map(|b| ReflectedBinding {
                    kind: KINDS[b.kind as usize],
                    space: b.set,
                    register: b.binding,
                    count: b.count,
                    set,
                    binding: b.new_binding,
                    added: None,
                    stride: b.stride,
                })
                .collect()
        };
        // SAFETY: out came from the bridge and is freed once.
        unsafe { wc_bindings_free(&mut out) };
        match result {
            BindResult::Ok => Ok(made),
            BindResult::BeyondLimit => Err(ConvertError::Bindings(bind::BEYOND_LIMIT)),
            BindResult::Unbounded => Err(ConvertError::Bindings(bind::UNBOUNDED)),
            BindResult::OutOfMemory => Err(ConvertError::Bindings(bind::OUT_OF_MEMORY)),
        }
    }

    /// Vulkan's `alphaToOne` and OpenGL's `GL_SAMPLE_ALPHA_TO_ONE` made in a fragment program (plan
    /// 15.4, `AlphaToOneInShader`): Mesa's alpha to coverage demotes the samples alpha does not
    /// cover, then every colour output's alpha is one. Nothing for another stage.
    pub fn alpha_to_one(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_alpha_to_one(self.0) };
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
        let mut bytes = Bytes {
            data: std::ptr::null_mut(),
            size: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `bytes` with memory it allocated.
        if !unsafe { wc_nir_serialize(self.0, &mut bytes) } {
            return Err(ConvertError::Mesa(NIR_SERIALIZE));
        }
        // SAFETY: data holds size bytes until wc_bytes_free.
        let copy = unsafe { std::slice::from_raw_parts(bytes.data, bytes.size) }.to_vec();
        // SAFETY: bytes came from the bridge and is freed once.
        unsafe { wc_bytes_free(&mut bytes) };
        Ok(copy)
    }

    #[cfg(feature = "standalone")]
    /// Candidate A's first step: SPIR-V written by Zink's compiler (the shader is prepared for it
    /// in place, so it is consumed).
    pub fn into_spirv(self) -> Result<Vec<u32>, ConvertError> {
        let mut words = Words {
            data: std::ptr::null_mut(),
            count: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `words` with memory it allocated.
        if !unsafe { wc_nir_to_spirv(self.0, &mut words) } {
            return Err(ConvertError::Mesa(NIR_TO_SPIRV));
        }
        // SAFETY: data holds count words until wc_words_free.
        let copy = unsafe { std::slice::from_raw_parts(words.data, words.count) }.to_vec();
        // SAFETY: words came from the bridge and is freed once.
        unsafe { wc_words_free(&mut words) };
        Ok(copy)
    }
}

impl Drop for Nir {
    fn drop(&mut self) {
        // SAFETY: self.0 came from the bridge and is freed once.
        unsafe { wc_nir_free(self.0) };
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
