//! The driver's `pipe_screen`: Gallium's view of the device, its caps from the interface's.

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_int, c_uint};
use std::sync::{Mutex, MutexGuard, PoisonError};

use wcontainers_gpu::{
    Backend, Caps, DeviceStatus, Features, Format, FormatUse, Object, SlotKind, SlowPath,
    TEXEL_BUFFER_TEXTURE_WIDTH,
};
use wcontainers_shaders::stage::STREAM_BUFFERS;

use crate::device::HostWindows;
use crate::draw::ShaderId;
use crate::format;
use crate::gallium::*;
use crate::present::Window;

/// The driver's name, as `GALLIUM_DRIVER` chooses it and OpenGL's renderer string starts.
pub const DRIVER_NAME: &core::ffi::CStr = c"wcontainers";

/// One screen: Gallium's struct first, so that a `pipe_screen` pointer is one to it.
#[repr(C)]
pub struct Screen<B: Backend> {
    base: pipe_screen,
    backend: Mutex<B>,
    /// Objects whose Gallium owner went, given back to the back end when it is next used (Mesa
    /// destroys resources from inside the driver's own calls).
    retired: Mutex<Vec<Object<B>>>,
    /// The live shader states of each content, which contexts keep what was made of while any is.
    shaders: Mutex<HashMap<ShaderId, usize>>,
    caps: Caps,
    name: CString,
    pub windows: Mutex<HashMap<usize, Window<B>>>,
    /// The driver's own maps, under Mesa's transfer helper (`pipe_screen::transfer_helper`), which
    /// maps a multisampled texture through a single-sampled one and blits between them.
    transfers: Box<u_transfer_vtbl>,
}

impl<B: Backend> Screen<B> {
    /// # Safety
    /// `screen` is one this driver made.
    pub unsafe fn of<'a>(screen: *mut pipe_screen) -> &'a Screen<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*screen.cast::<Screen<B>>() }
    }

    /// The back end, for one call of the driver's. Once its device is lost the call is refused
    /// here, so that no call of any context records work for a lost device.
    pub fn backend(&self) -> MutexGuard<'_, B> {
        let mut backend = whole(&self.backend);
        if let DeviceStatus::Lost(reason) = backend.status() {
            drop(backend);
            crate::state::refuse(format_args!("the device was lost: {reason:?}"));
        }
        let retired = std::mem::take(&mut *whole(&self.retired));
        for object in retired {
            backend.destroy(object);
        }
        warn_slow_paths(&mut *backend);
        backend
    }

    /// Gives an object back to the back end.
    pub fn retire(&self, object: Object<B>) {
        whole(&self.retired).push(object);
    }

    pub fn caps(&self) -> &Caps {
        &self.caps
    }

    /// Notes a shader state made of content `id`.
    pub fn shader_made(&self, id: ShaderId) {
        *whole(&self.shaders).entry(id).or_default() += 1;
    }

    /// Notes a shader state of content `id` deleted; whether one of equal content is still live.
    pub fn shader_deleted(&self, id: &ShaderId) -> bool {
        let mut shaders = whole(&self.shaders);
        let Some(live) = shaders.get_mut(id) else {
            return false;
        };
        *live -= 1;
        if *live == 0 {
            shaders.remove(id);
            return false;
        }
        true
    }

    /// Whether a shader state of content `id` is live.
    pub fn shader_live(&self, id: &ShaderId) -> bool {
        whole(&self.shaders).contains_key(id)
    }
}

/// One of the screen's locks. A guest call refused while holding it (`state::guest_call`) leaves
/// it poisoned; what it guards is whole between the back end's calls, where refusals are made.
pub fn whole<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Warns once of each slow path the first time it is taken, on the process's error output.
fn warn_slow_paths<B: Backend>(backend: &mut B) {
    for path in backend.slow_paths().take_warnings() {
        eprintln!(
            "{}: slow path: {}",
            DRIVER_NAME.to_string_lossy(),
            path.reason()
        );
    }
}

/// Every slow path the screen's device took, with how many times, for the end of its life.
fn slow_path_counts<B: Backend>(backend: &mut B) -> Vec<(&'static str, u64)> {
    let paths = backend.slow_paths();
    SlowPath::ALL
        .iter()
        .map(|&path| (path.reason(), paths.count(path)))
        .filter(|&(_, count)| count > 0)
        .collect()
}

unsafe extern "C" fn destroy<B: Backend>(screen: *mut pipe_screen) {
    crate::state::guest_call(std::ptr::null_mut(), (), move || {
        // SAFETY: Mesa destroys the screen it was given once.
        let screen = unsafe { Box::from_raw(screen.cast::<Screen<B>>()) };
        {
            let mut backend = screen.backend();
            for (reason, count) in slow_path_counts(&mut *backend) {
                eprintln!(
                    "{}: slow path taken {count} times: {reason}",
                    DRIVER_NAME.to_string_lossy()
                );
            }
        }
        // SAFETY: the helper was made with the screen and outlives every context of it.
        unsafe { u_transfer_helper_destroy(screen.base.transfer_helper) };
        drop(screen);
    })
}

unsafe extern "C" fn name(_: *mut pipe_screen) -> *const c_char {
    DRIVER_NAME.as_ptr()
}

unsafe extern "C" fn device_name<B: Backend>(screen: *mut pipe_screen) -> *const c_char {
    // SAFETY: Mesa passes this driver's screen; the name lives as long as it.
    unsafe { Screen::<B>::of(screen) }.name.as_ptr()
}

unsafe extern "C" fn is_format_supported<B: Backend>(
    screen: *mut pipe_screen,
    format: pipe_format,
    target: pipe_texture_target,
    samples: c_uint,
    storage_samples: c_uint,
    bindings: c_uint,
) -> bool {
    crate::state::guest_call(std::ptr::null_mut(), false, move || {
        // SAFETY: Mesa passes this driver's screen.
        let screen = unsafe { Screen::<B>::of(screen) };
        format::supported(
            &screen.caps.formats,
            format,
            target,
            samples,
            storage_samples,
            bindings,
        )
    })
}

/// Levels of a full chain on a side of `size` texels.
fn levels(size: u32) -> u32 {
    size.ilog2() + 1
}

/// The last level any texture the screen reports can have: a level of detail past it selects
/// that level, as one at it does.
pub fn deepest_level(ours: &Caps) -> u32 {
    let limits = &ours.limits;
    levels(
        limits
            .max_texture_2d
            .max(limits.max_texture_cube)
            .max(limits.max_texture_3d),
    ) - 1
}

/// The largest level-of-detail bias: a sampler's is added in the program, and a sample's bias is
/// clamped to it by WGSL (<https://www.w3.org/TR/WGSL/#texturesamplebias>) and Direct3D
/// (`D3D11_MIP_LOD_BIAS_MAX`).
const MAX_LOD_BIAS: f32 = 15.99;

/// The GLSL versions st/mesa's GLSL compiler takes for this driver (the core profile's, the
/// compatibility profile's) and OpenGL ES's: the versions whose programs the NIR route converts
/// and whose state the context draws with. OpenGL ES's is 3.20's, which turns on
/// `EXT_gpu_shader5`.
const GLSL_VERSION: c_uint = 330;
const GLSL_COMPATIBILITY_VERSION: c_uint = 130;
const ESSL_VERSION: c_uint = 320;

/// OpenGL 3.1's GLSL version, the core profile's where the passed values are fewer than OpenGL
/// 3.2 requires.
const GLSL_3_1_VERSION: c_uint = 140;

/// The least components OpenGL 3.2 lets a fragment program take in and a geometry program pass on
/// (<https://registry.khronos.org/OpenGL-Refpages/gl4/html/glGet.xhtml#:~:text=GL_MAX_FRAGMENT_INPUT_COMPONENTS>).
const GL_3_2_PASSED_COMPONENTS: c_uint = 128;

/// The core profile's GLSL version: 3.30's where the passed values meet OpenGL 3.2's minimums,
/// otherwise OpenGL 3.1's, so that no version is reported whose minimums are not met.
pub(crate) fn glsl_version(limits: &wcontainers_gpu::Limits) -> c_uint {
    if passed_values(limits) * naga::VectorSize::Quad as c_uint >= GL_3_2_PASSED_COMPONENTS {
        GLSL_VERSION
    } else {
        GLSL_3_1_VERSION
    }
}

/// A geometry program's limits, the least `OES_geometry_shader` allows, which are Mesa's own
/// (`main/config.h`): the stage helper sizes its records by the program's own declarations.
const GEOMETRY_EMITTED_VERTICES: c_uint = MAX_GEOMETRY_OUTPUT_VERTICES;
const GEOMETRY_OUTPUT_COMPONENTS: c_uint = MAX_GEOMETRY_TOTAL_OUTPUT_COMPONENTS;
const GEOMETRY_INVOCATIONS: c_uint = MAX_GEOMETRY_SHADER_INVOCATIONS;

/// The values a tessellation control program writes once a patch, the least OpenGL 4.0 and
/// `OES_tessellation_shader` allow (120 components): the stage helper sizes its patch records by
/// the program's own declarations.
const PATCH_VARYINGS: c_uint = 30;

/// The vertex inputs a pass-through vertex program of the stage helper takes beside the values a
/// program passes on: the position, and the point coordinate of points drawn as squares
/// (`PointsAsQuads`).
const PASS_THROUGH_INPUTS: c_uint = 2;

/// The values a vertex, geometry or evaluation program may pass on: what the host passes between
/// stages, and what the stage helper's pass-through vertex program, which draws a geometry or
/// evaluation program's output and points as squares, takes as vertex inputs beside its own.
fn passed_values(limits: &wcontainers_gpu::Limits) -> c_uint {
    limits
        .inter_stage_slots
        .min(limits.vertex_attributes.saturating_sub(PASS_THROUGH_INPUTS))
}

/// Whether points drawn as squares can pass `gl_PointCoord` at location `at`: between stages, and
/// to the pass-through vertex program beside its position.
pub(crate) fn point_coordinate_passes(limits: &wcontainers_gpu::Limits, at: u32) -> bool {
    at < limits.inter_stage_slots && at + PASS_THROUGH_INPUTS <= limits.vertex_attributes
}

/// Stream output's limits: the stage helper's (every vertex output written to its records, into
/// its four buffers), on back ends with compute work; none without.
fn stream_output(ours: &Caps) -> (c_uint, c_uint) {
    if ours.has(Features::COMPUTE) {
        (
            STREAM_BUFFERS as c_uint,
            ours.limits.inter_stage_slots * naga::VectorSize::Quad as c_uint,
        )
    } else {
        (0, 0)
    }
}

/// The largest point the screen reports: the back end's, or where it draws points of one size,
/// the largest target's side, as each point is then drawn as a square (`PointsAsQuads`), which
/// covers any target whole at that size.
pub(crate) fn largest_point_size(caps: &Caps) -> f32 {
    if caps.has(Features::POINT_SIZE) {
        caps.limits.max_point_size
    } else {
        caps.limits.max_texture_2d as f32
    }
}

/// The uniform blocks a stage reports, every one of them a Gallium slot and a host buffer: one of
/// the host's buffers is left for a converted program's helper constants, and one of Gallium's
/// slots, the first past these, for the blocks the driver binds itself (a geometry program's
/// squares, `draw::geometry_squares_for`).
pub(crate) fn uniform_blocks(caps: &Caps) -> u32 {
    (caps.limits.uniform_buffers_per_stage - 1).min(PIPE_MAX_CONSTANT_BUFFERS - 1)
}

/// Gallium's screen caps: its defaults, with the interface's limits over them and what the
/// context does and does not do.
fn fill_caps(caps: &mut pipe_caps, ours: &Caps) {
    let limits = &ours.limits;
    caps.max_texture_2d_size = limits.max_texture_2d;
    caps.max_texture_cube_levels = levels(limits.max_texture_cube);
    caps.max_texture_3d_levels = levels(limits.max_texture_3d);
    caps.max_texture_array_layers = limits.max_texture_layers;
    caps.max_render_targets = limits.color_targets;
    caps.max_vertex_buffers = limits.vertex_streams;
    caps.max_vertex_attrib_stride = limits.max_vertex_stride;
    caps.max_point_size = largest_point_size(ours);
    caps.max_texture_lod_bias = MAX_LOD_BIAS;
    caps.max_line_width = limits.max_line_width;
    caps.max_line_width_aa = limits.max_line_width;
    // Without wide lines the back end draws one width, to which OpenGL clamps every width.
    if !ours.has(Features::WIDE_LINES) {
        caps.min_line_width = limits.max_line_width;
        caps.min_line_width_aa = limits.max_line_width;
    }
    caps.max_texture_anisotropy = f32::from(limits.max_anisotropy);
    // Samplers take their anisotropy (`state::create_sampler_state`); the back end applies it to
    // fully linear filtering only, as OpenGL lets an implementation.
    caps.anisotropic_filter = limits.max_anisotropy > 1;
    caps.constant_buffer_offset_alignment = limits.min_uniform_offset_alignment;
    caps.max_constant_buffer_size = limits.max_uniform_buffer_size;
    caps.max_varyings = passed_values(limits);
    caps.glsl_feature_level = glsl_version(limits);
    caps.glsl_feature_level_compatibility = GLSL_COMPATIBILITY_VERSION;
    caps.essl_feature_level = ESSL_VERSION;
    let filters = |format| ours.formats.get(format).contains(FormatUse::FILTER);
    caps.texture_half_float_linear = filters(Format::Rgba16Float);
    caps.texture_float_linear = filters(Format::Rgba32Float);
    caps.texture_shadow_map = true;
    // Strips restart at the largest index on the host; lists, where OpenGL drops the primitive
    // a restart cuts, and strips drawn without restart are rewritten (`RestartRewrite`).
    caps.primitive_restart_fixed_index = true;
    // Restart at any other index is rewritten too.
    caps.primitive_restart = true;
    // Views' swizzles are applied in the programs that read them (`SwizzleInShader`).
    caps.texture_swizzle = true;
    caps.depth_clip_disable = ours.has(Features::DEPTH_CLAMP);
    caps.polygon_offset_clamp = ours.has(Features::DEPTH_BIAS_CLAMP);
    caps.max_dual_source_render_targets = c_uint::from(ours.has(Features::DUAL_SOURCE_BLEND));
    let timestamps = ours.has(Features::TIMESTAMPS);
    caps.query_timestamp = timestamps;
    caps.query_time_elapsed = timestamps;
    // Texel buffers, read through 2D textures the views are copied into
    // (`TexelBufferAsTexture`), rows of `TEXEL_BUFFER_TEXTURE_WIDTH` texels.
    caps.texture_buffer_objects = true;
    caps.max_texel_buffer_elements =
        TEXEL_BUFFER_TEXTURE_WIDTH.saturating_mul(limits.max_texture_2d);
    (
        caps.max_stream_output_buffers,
        caps.max_stream_output_interleaved_components,
    ) = stream_output(ours);
    // Geometry programs, run by the stage helper as compute work.
    if ours.has(Features::COMPUTE) {
        caps.max_geometry_output_vertices = GEOMETRY_EMITTED_VERTICES;
        caps.max_geometry_total_output_components = GEOMETRY_OUTPUT_COMPONENTS;
        caps.max_gs_invocations = GEOMETRY_INVOCATIONS;
        caps.max_shader_patch_varyings = PATCH_VARYINGS;
        // An evaluation program reads the levels as patch inputs, which the stage helper fills.
        caps.glsl_tess_levels_as_inputs = true;
    }
    // Compute work, storage buffers and images (in compute programs), indirect draws (their
    // arguments read back), multisampled textures, gathers, stencil texturing and framebuffers
    // without attachments: OpenGL ES 3.1's.
    caps.compute = ours.has(Features::COMPUTE);
    caps.shader_buffer_offset_alignment = limits.min_storage_offset_alignment;
    caps.max_shader_buffer_size =
        c_uint::try_from(limits.max_storage_buffer_size).unwrap_or(c_uint::MAX);
    caps.draw_indirect = true;
    // Stores to images declared without a format, in the bound image's format: a program is
    // converted for the formats of the images bound where it declares none (`image_formats`).
    caps.image_store_formatted = true;
    caps.texture_multisample =
        ours.has(Features::MULTISAMPLE_READ) && ours.has(Features::SAMPLE_MASK);
    // Programs run per sample: st/mesa makes the per-sample variant of a fragment program itself,
    // and the NIR route takes inputs at an offset or a sample and the sample positions from the
    // values the host interpolates (`interpolate_elsewhere`).
    caps.sample_shading = ours.has(Features::SAMPLE_SHADING);
    // The first target's texels read in fragment programs, from a copy of it each draw bound at
    // `FETCHED_UNIT` (multisampled where the target is), as st/mesa makes advanced blending:
    // `KHR_blend_equation_advanced`, not its coherent form.
    caps.fbfetch = c_uint::from(ours.has(Features::MULTISAMPLE_READ));
    caps.max_texture_gather_components = naga::VectorSize::Quad as c_uint;
    caps.framebuffer_no_attachment = true;
    caps.npot_textures = true;
    caps.blend_equation_separate = true;
    let independent = ours.has(Features::INDEPENDENT_BLEND);
    caps.indep_blend_enable = independent;
    caps.indep_blend_func = independent;
    // Attachments of different sizes are fitted by the pass tracker.
    caps.mixed_framebuffer_sizes = true;
    // The host's cube maps are always seamless, as OpenGL ES's are; the interface's views include
    // cube arrays.
    caps.seamless_cube_map = true;
    caps.cube_map_array = true;
    // The interface copies between formats of one texel or block size, a block-compressed one and
    // a plain one among them (`copy_texture`); st/mesa's copies between other formats of one size
    // are blits the driver makes as such copies (`blit::bit_copy`).
    caps.copy_between_compressed_and_plain_formats = true;
    // Every back end bounds a program's buffer accesses: one past a binding's end reads zero or
    // a value inside it and writes nothing outside it.
    caps.robust_buffer_access_behavior = true;
    // A context one of whose calls was refused reports itself reset by its own fault
    // (`state::guest_call`).
    caps.device_reset_status_query = true;
    caps.fragment_shader_texture_lod = true;
    // The base instance is always zero (no `start_instance`), so the instance index is OpenGL's.
    caps.vs_instanceid = true;
    // Occlusion predicates (every back end answers whether any sample passed); an exact count is
    // refused by name where the back end lacks it.
    caps.occlusion_query = true;
    // Draws and clears under a render condition wait for its answer (`PredicationWait`).
    caps.conditional_render = true;
    // Steps other than one a back end lacks are fetched in the vertex program.
    caps.vertex_element_instance_divisor = true;
    // Maps are copies the driver writes back, never the device's own memory.
    caps.buffer_map_persistent_coherent = false;
    caps.allow_mapped_buffers_during_execution = false;
    caps.texture_transfer_modes = 0;
    // The host's fragment coordinates, and depth clip spaces of both kinds (the vertex program
    // is rewritten for OpenGL's).
    caps.fs_coord_origin_upper_left = true;
    caps.fs_coord_pixel_center_half_integer = true;
    // A fragment program's face is the host's built-in (a system value), not an input.
    caps.fs_face_is_integer_sysval = true;
    caps.clip_halfz = true;
    // Uniforms packed in words into buffer 0, as the NIR route's options take them.
    caps.packed_uniforms = true;
    // Textures and samplers stay variables the program names, which the NIR route binds by
    // (Mesa's own programs, made from TGSI, otherwise name only a unit).
    caps.nir_samplers_as_deref = true;
    // OpenGL's fixed-function steps the host lacks, made by st/mesa in the programs.
    caps.flatshade = false;
    caps.alpha_test = false;
    caps.two_sided_color = false;
    caps.point_size_fixed = 0;
    caps.clip_planes = 0;
    caps.texrect = false;
    caps.gl_clamp = false;
}

/// The stages the driver takes programs for; the geometry and tessellation stages where the back
/// end runs compute work (`STAGED`).
const STAGES: &[mesa_shader_stage] = &[
    MESA_SHADER_VERTEX,
    MESA_SHADER_FRAGMENT,
    MESA_SHADER_COMPUTE,
    MESA_SHADER_GEOMETRY,
    MESA_SHADER_TESS_CTRL,
    MESA_SHADER_TESS_EVAL,
];

/// The stages the stage helper runs as compute work.
const STAGED: &[mesa_shader_stage] = &[
    MESA_SHADER_GEOMETRY,
    MESA_SHADER_TESS_CTRL,
    MESA_SHADER_TESS_EVAL,
];

/// The compute grid's limits, the interface's.
fn fill_compute_caps(caps: &mut pipe_compute_caps, ours: &Caps) {
    let limits = &ours.limits;
    caps.address_bits = u32::BITS;
    caps.grid_dimension = limits.compute_size.len() as c_uint;
    caps.max_grid_size = limits.compute_size.map(|_| limits.max_dispatch);
    caps.max_block_size = limits.compute_size;
    caps.max_threads_per_block = limits.compute_invocations;
    caps.max_local_size = limits.compute_workgroup_storage;
}

/// What a program of one stage may hold. The interface sets no bound on a program's size.
fn fill_shader_caps(caps: &mut pipe_shader_caps, stage: mesa_shader_stage, ours: &Caps) {
    let limits = &ours.limits;
    let unbounded = c_int::MAX as c_uint;
    caps.max_instructions = unbounded;
    caps.max_alu_instructions = unbounded;
    caps.max_tex_instructions = unbounded;
    caps.max_tex_indirections = unbounded;
    caps.max_control_flow_depth = unbounded;
    caps.max_temps = unbounded;
    (caps.max_inputs, caps.max_outputs) = match stage {
        MESA_SHADER_VERTEX => (limits.vertex_attributes, passed_values(limits)),
        MESA_SHADER_FRAGMENT => (passed_values(limits), limits.color_targets),
        MESA_SHADER_GEOMETRY | MESA_SHADER_TESS_CTRL | MESA_SHADER_TESS_EVAL => {
            (passed_values(limits), passed_values(limits))
        }
        _ => (0, 0),
    };
    // Storage buffers and images in compute programs, within their shared binding range; storage
    // buffers in fragment programs too, which WebGPU reads and writes there (OpenGL's
    // `ARB_shader_storage_buffer_object` asks for them), and images where the back end writes
    // storage there.
    let buffers = limits
        .storage_buffers_per_stage
        .min(PIPE_MAX_SHADER_BUFFERS);
    let images = limits
        .storage_textures_per_stage
        .min(PIPE_MAX_SHADER_IMAGES)
        .min(SlotKind::Storage.slots() - buffers);
    // Geometry and control programs run as compute work in the stage helper (`GeometryAsCompute`,
    // `TessellationAsCompute`), which reads and writes storage as compute programs do; a vertex
    // or evaluation program using storage too, before a pass-through geometry program
    // (`VertexStorageAsCompute`), where the back end runs compute work.
    let staged = ours.has(Features::COMPUTE);
    if matches!(
        stage,
        MESA_SHADER_COMPUTE | MESA_SHADER_GEOMETRY | MESA_SHADER_TESS_CTRL | MESA_SHADER_TESS_EVAL
    ) || (stage == MESA_SHADER_VERTEX && staged)
    {
        caps.max_shader_buffers = buffers;
        caps.max_shader_images = images;
    }
    if stage == MESA_SHADER_FRAGMENT {
        caps.max_shader_buffers = buffers;
        if ours.has(Features::FRAGMENT_STORAGE_WRITE) {
            caps.max_shader_images = images;
        }
    }
    caps.max_const_buffer0_size = limits.max_uniform_buffer_size;
    caps.max_const_buffers = uniform_blocks(ours);
    // Each OpenGL sampler is split into a texture and a sampler at the same binding, each within
    // its kind's range of the stage's bindings.
    let samplers = limits
        .samplers_per_stage
        .min(limits.textures_per_stage)
        .min(SlotKind::Sampler.slots())
        .min(SlotKind::Texture.slots())
        .min(PIPE_MAX_SAMPLERS);
    caps.max_texture_samplers = samplers;
    caps.max_sampler_views = samplers;
    caps.supported_irs = 1 << PIPE_SHADER_IR_NIR as c_uint;
    caps.integers = true;
    caps.cont_supported = true;
    caps.indirect_temp_addr = true;
    caps.indirect_const_addr = true;
}

/// The screen over `backend`, named `name`.
pub fn screen<B: HostWindows>(backend: B, gpu: CString) -> *mut pipe_screen {
    let caps = backend.caps().clone();
    let mut screen = Box::new(Screen {
        base: pipe_screen::default(),
        backend: Mutex::new(backend),
        retired: Mutex::new(Vec::new()),
        shaders: Mutex::new(HashMap::new()),
        caps,
        name: gpu,
        windows: Mutex::new(HashMap::new()),
        transfers: Box::new(u_transfer_vtbl {
            resource_create: Some(crate::resource::resource_create::<B>),
            resource_destroy: Some(crate::resource::resource_destroy::<B>),
            transfer_map: Some(crate::resource::texture_map::<B>),
            transfer_flush_region: Some(crate::resource::transfer_flush_region::<B>),
            transfer_unmap: Some(crate::resource::texture_unmap::<B>),
            ..Default::default()
        }),
    });
    let ours = screen.caps.clone();
    let transfers: *const u_transfer_vtbl = &*screen.transfers;
    let base = &mut screen.base;
    // SAFETY: the table lives in its own box as long as the screen, whose `destroy` frees the
    // helper first.
    base.transfer_helper =
        unsafe { u_transfer_helper_create(transfers, U_TRANSFER_HELPER_MSAA_MAP) };
    base.destroy = Some(destroy::<B>);
    base.get_name = Some(name);
    base.get_vendor = Some(name);
    base.get_device_vendor = Some(device_name::<B>);
    base.is_format_supported = Some(is_format_supported::<B>);
    base.context_create = Some(crate::context::context_create::<B>);
    base.resource_create = Some(crate::resource::resource_create::<B>);
    base.resource_destroy = Some(crate::resource::resource_destroy::<B>);
    base.fence_reference = Some(crate::context::fence_reference::<B>);
    base.fence_finish = Some(crate::context::fence_finish::<B>);
    base.flush_frontbuffer = Some(crate::present::flush_frontbuffer::<B>);
    // SAFETY: the screen is complete enough for Mesa's defaults, which only write its caps.
    unsafe { u_init_pipe_screen_caps(base, 1) };
    fill_caps(&mut base.caps, &ours);
    // st/mesa hands shaders over in the NIR route's shape.
    base.nir_options
        .fill(wcontainers_nir::mesa::compiler_options().cast());
    let staged = ours.has(Features::COMPUTE);
    for &stage in STAGES.iter().filter(|&&s| staged || !STAGED.contains(&s)) {
        fill_shader_caps(&mut base.shader_caps[stage as usize], stage, &ours);
    }
    if ours.has(Features::COMPUTE) {
        fill_compute_caps(&mut base.compute_caps, &ours);
    }
    Box::into_raw(screen).cast()
}

/// The screen on the GPU the back end finds, or null without one (Mesa then tries its next
/// driver).
#[unsafe(no_mangle)]
pub extern "C" fn wcontainers_screen_create() -> *mut pipe_screen {
    crate::state::guest_call(std::ptr::null_mut(), std::ptr::null_mut(), move || {
        match crate::device::webgpu() {
            Some((backend, name)) => screen(backend, name),
            None => std::ptr::null_mut(),
        }
    })
}

#[cfg(test)]
mod tests {
    use wcontainers_gpu::Backend;
    use wcontainers_gpu_test::Recorder;

    use super::*;

    /// The values Direct3D 12 passes between stages through wgpu, beside the position
    /// (`D3D12_VS_OUTPUT_REGISTER_COUNT` less one), and the vertex inputs it takes (less the
    /// vertex and instance numbers).
    const DIRECT3D_12_INTER_STAGE_SLOTS: u32 = 31;
    const DIRECT3D_12_VERTEX_ATTRIBUTES: u32 = 30;

    /// The vertex inputs that let the pass-through take OpenGL 3.2's 32 values beside its own.
    const VERTEX_ATTRIBUTES_FOR_32_VALUES: u32 = 34;
    const INTER_STAGE_SLOTS_FOR_32_VALUES: u32 = 32;

    fn caps_with(inter_stage_slots: u32, vertex_attributes: u32, clamp: bool) -> Caps {
        Recorder::with_caps(|caps| {
            caps.limits.inter_stage_slots = inter_stage_slots;
            caps.limits.vertex_attributes = vertex_attributes;
            caps.features.set(Features::DEPTH_BIAS_CLAMP, clamp);
        })
        .caps()
        .clone()
    }

    fn filled(ours: &Caps) -> pipe_caps {
        let mut caps = pipe_caps::default();
        fill_caps(&mut caps, ours);
        caps
    }

    #[test]
    fn the_core_profile_is_opengl_3_1_where_fewer_values_pass_than_3_2_requires() {
        let ours = caps_with(
            DIRECT3D_12_INTER_STAGE_SLOTS,
            DIRECT3D_12_VERTEX_ATTRIBUTES,
            true,
        );
        let caps = filled(&ours);
        assert_eq!(
            caps.max_varyings,
            DIRECT3D_12_VERTEX_ATTRIBUTES - PASS_THROUGH_INPUTS
        );
        assert!(caps.max_varyings * (naga::VectorSize::Quad as c_uint) < GL_3_2_PASSED_COMPONENTS);
        assert_eq!(caps.glsl_feature_level, GLSL_3_1_VERSION);
        assert_eq!(caps.essl_feature_level, ESSL_VERSION);
    }

    #[test]
    fn the_core_profile_is_opengl_3_3_where_32_values_pass() {
        let ours = caps_with(
            INTER_STAGE_SLOTS_FOR_32_VALUES,
            VERTEX_ATTRIBUTES_FOR_32_VALUES,
            true,
        );
        let caps = filled(&ours);
        assert_eq!(
            caps.max_varyings * naga::VectorSize::Quad as c_uint,
            GL_3_2_PASSED_COMPONENTS
        );
        assert_eq!(caps.glsl_feature_level, GLSL_VERSION);
    }

    #[test]
    fn anisotropic_filtering_is_reported_where_samplers_take_more_than_one() {
        let mut ours = caps_with(
            DIRECT3D_12_INTER_STAGE_SLOTS,
            DIRECT3D_12_VERTEX_ATTRIBUTES,
            true,
        );
        let most = ours.limits.max_anisotropy;
        assert!(filled(&ours).anisotropic_filter == (most > 1));
        ours.limits.max_anisotropy = 1;
        assert!(!filled(&ours).anisotropic_filter);
    }

    #[test]
    fn polygon_offset_clamp_follows_the_back_ends_depth_bias_clamp() {
        for clamp in [false, true] {
            let ours = caps_with(
                DIRECT3D_12_INTER_STAGE_SLOTS,
                DIRECT3D_12_VERTEX_ATTRIBUTES,
                clamp,
            );
            assert_eq!(filled(&ours).polygon_offset_clamp, clamp);
        }
    }

    #[test]
    fn the_point_coordinate_passes_only_where_both_the_stages_and_the_pass_through_take_it() {
        let ours = caps_with(
            DIRECT3D_12_INTER_STAGE_SLOTS,
            DIRECT3D_12_VERTEX_ATTRIBUTES,
            true,
        );
        let last = passed_values(&ours.limits);
        assert!(point_coordinate_passes(&ours.limits, last - 1));
        assert!(!point_coordinate_passes(&ours.limits, last + 1));
    }

    #[test]
    fn each_slow_path_taken_is_warned_once_and_counted_every_time() {
        const FANS: u64 = 2;
        let mut backend = Recorder::with_caps(|_| {});
        assert!(slow_path_counts(&mut backend).is_empty());
        for _ in 0..FANS {
            backend.slow_paths().hit(SlowPath::FanConverted);
        }
        backend.slow_paths().hit(SlowPath::PointsAsQuads);
        assert_eq!(
            backend.slow_paths().take_warnings(),
            [SlowPath::FanConverted, SlowPath::PointsAsQuads]
        );
        assert!(backend.slow_paths().take_warnings().is_empty());
        assert_eq!(
            slow_path_counts(&mut backend),
            [
                (SlowPath::FanConverted.reason(), FANS),
                (SlowPath::PointsAsQuads.reason(), 1)
            ]
        );
    }
}
