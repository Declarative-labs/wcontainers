//! The driver's `pipe_screen`: Gallium's view of the device, its caps from the interface's.

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_int, c_uint};
use std::sync::{Mutex, MutexGuard};

use wcontainers_gpu::{Backend, Caps, Features, Object};

use crate::device::HostWindows;
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
    caps: Caps,
    name: CString,
    pub windows: Mutex<HashMap<usize, Window<B>>>,
}

impl<B: Backend> Screen<B> {
    /// # Safety
    /// `screen` is one this driver made.
    pub unsafe fn of<'a>(screen: *mut pipe_screen) -> &'a Screen<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*screen.cast::<Screen<B>>() }
    }

    /// The back end, for one call of the driver's.
    pub fn backend(&self) -> MutexGuard<'_, B> {
        let mut backend = self.backend.lock().expect("the device lock");
        let retired = std::mem::take(&mut *self.retired.lock().expect("the retired lock"));
        for object in retired {
            backend.destroy(object);
        }
        backend
    }

    /// Gives an object back to the back end.
    pub fn retire(&self, object: Object<B>) {
        self.retired.lock().expect("the retired lock").push(object);
    }

    pub fn caps(&self) -> &Caps {
        &self.caps
    }
}

unsafe extern "C" fn destroy<B: Backend>(screen: *mut pipe_screen) {
    // SAFETY: Mesa destroys the screen it was given once.
    drop(unsafe { Box::from_raw(screen.cast::<Screen<B>>()) });
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
}

/// Levels of a full chain on a side of `size` texels.
fn levels(size: u32) -> u32 {
    size.ilog2() + 1
}

/// The GLSL version st/mesa's GLSL compiler takes for this driver, and OpenGL ES's: the
/// versions whose programs the NIR route converts and whose state the context draws with.
const GLSL_VERSION: c_uint = 120;
const ESSL_VERSION: c_uint = 100;

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
    caps.max_point_size = limits.max_point_size;
    caps.max_line_width = limits.max_line_width;
    caps.max_line_width_aa = limits.max_line_width;
    // Without wide lines the back end draws one width, to which OpenGL clamps every width.
    if !ours.has(Features::WIDE_LINES) {
        caps.min_line_width = limits.max_line_width;
        caps.min_line_width_aa = limits.max_line_width;
    }
    caps.max_texture_anisotropy = f32::from(limits.max_anisotropy);
    caps.constant_buffer_offset_alignment = limits.min_uniform_offset_alignment;
    caps.max_varyings = limits.inter_stage_slots;
    caps.glsl_feature_level = GLSL_VERSION;
    caps.glsl_feature_level_compatibility = GLSL_VERSION;
    caps.essl_feature_level = ESSL_VERSION;
    caps.npot_textures = true;
    caps.blend_equation_separate = true;
    let independent = ours.has(Features::INDEPENDENT_BLEND);
    caps.indep_blend_enable = independent;
    caps.indep_blend_func = independent;
    // Attachments of different sizes are fitted by the pass tracker.
    caps.mixed_framebuffer_sizes = true;
    // The host's cube maps are always seamless, as OpenGL ES's are.
    caps.seamless_cube_map = true;
    caps.fragment_shader_texture_lod = true;
    // The base instance is always zero (no `start_instance`), so the instance index is OpenGL's.
    caps.vs_instanceid = true;
    // Occlusion predicates (every back end answers whether any sample passed); an exact count is
    // refused by name where the back end lacks it.
    caps.occlusion_query = true;
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

/// The stages the driver takes programs for. Compute waits on the context's dispatch.
const STAGES: &[mesa_shader_stage] = &[MESA_SHADER_VERTEX, MESA_SHADER_FRAGMENT];

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
    (caps.max_inputs, caps.max_outputs) = if stage == MESA_SHADER_VERTEX {
        (limits.vertex_attributes, limits.inter_stage_slots)
    } else {
        (limits.inter_stage_slots, limits.color_targets)
    };
    caps.max_const_buffer0_size = limits.max_uniform_buffer_size;
    // One of the stage's uniform buffers holds a converted program's helper constants.
    caps.max_const_buffers = limits.uniform_buffers_per_stage - 1;
    // Each OpenGL sampler is split into a texture and a sampler at the same binding.
    let samplers = limits
        .samplers_per_stage
        .min(limits.textures_per_stage)
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
        caps,
        name: gpu,
        windows: Mutex::new(HashMap::new()),
    });
    let ours = screen.caps.clone();
    let base = &mut screen.base;
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
    for &stage in STAGES {
        fill_shader_caps(&mut base.shader_caps[stage as usize], stage, &ours);
    }
    Box::into_raw(screen).cast()
}

/// The screen on the GPU the back end finds, or null without one (Mesa then tries its next
/// driver).
#[unsafe(no_mangle)]
pub extern "C" fn wcontainers_screen_create() -> *mut pipe_screen {
    match crate::device::webgpu() {
        Some((backend, name)) => screen(backend, name),
        None => std::ptr::null_mut(),
    }
}
