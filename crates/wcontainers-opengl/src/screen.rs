//! The driver's `pipe_screen`: Gallium's view of the device, its caps from the interface's.

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::sync::Mutex;

use wcontainers_gpu::{Backend, Caps};

use crate::device::Device;
use crate::format;
use crate::gallium::*;

/// The driver's name, as `GALLIUM_DRIVER` chooses it and OpenGL's renderer string starts.
const DRIVER_NAME: &core::ffi::CStr = c"wcontainers";

/// One screen: Gallium's struct first, so that a `pipe_screen` pointer is one to it.
#[repr(C)]
pub struct Screen {
    base: pipe_screen,
    pub device: Mutex<Device>,
}

impl Screen {
    /// # Safety
    /// `screen` is one this driver made.
    pub unsafe fn of<'a>(screen: *mut pipe_screen) -> &'a Screen {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*screen.cast::<Screen>() }
    }
}

unsafe extern "C" fn destroy(screen: *mut pipe_screen) {
    // SAFETY: Mesa destroys the screen it was given once.
    drop(unsafe { Box::from_raw(screen.cast::<Screen>()) });
}

unsafe extern "C" fn name(_: *mut pipe_screen) -> *const c_char {
    DRIVER_NAME.as_ptr()
}

unsafe extern "C" fn device_name(screen: *mut pipe_screen) -> *const c_char {
    // SAFETY: Mesa passes this driver's screen; the name lives as long as the device.
    let screen = unsafe { Screen::of(screen) };
    screen
        .device
        .lock()
        .expect("the device lock")
        .name()
        .as_ptr()
}

unsafe extern "C" fn is_format_supported(
    screen: *mut pipe_screen,
    format: pipe_format,
    target: pipe_texture_target,
    samples: c_uint,
    storage_samples: c_uint,
    bindings: c_uint,
) -> bool {
    // SAFETY: Mesa passes this driver's screen.
    let screen = unsafe { Screen::of(screen) };
    let device = screen.device.lock().expect("the device lock");
    format::supported(
        &device.backend.caps().formats,
        format,
        target,
        samples,
        storage_samples,
        bindings,
    )
}

unsafe extern "C" fn context_create(
    _: *mut pipe_screen,
    _: *mut c_void,
    _: c_uint,
) -> *mut pipe_context {
    eprintln!(
        "{}: refused: an OpenGL context (the driver's context is not built yet)",
        DRIVER_NAME.to_string_lossy()
    );
    std::ptr::null_mut()
}

/// Levels of a full chain on a side of `size` texels.
fn levels(size: u32) -> u32 {
    size.ilog2() + 1
}

/// Gallium's screen caps: its defaults, with the interface's limits over them.
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
    caps.max_texture_anisotropy = f32::from(limits.max_anisotropy);
    caps.constant_buffer_offset_alignment = limits.min_uniform_offset_alignment;
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

/// The screen on the GPU the back end finds, or null without one (Mesa then tries its next
/// driver).
#[unsafe(no_mangle)]
pub extern "C" fn wcontainers_screen_create() -> *mut pipe_screen {
    let Some(device) = Device::new() else {
        return std::ptr::null_mut();
    };
    let mut screen = Box::new(Screen {
        base: pipe_screen::default(),
        device: Mutex::new(device),
    });
    let ours = screen
        .device
        .get_mut()
        .expect("the device lock")
        .backend
        .caps()
        .clone();
    let base = &mut screen.base;
    base.destroy = Some(destroy);
    base.get_name = Some(name);
    base.get_vendor = Some(name);
    base.get_device_vendor = Some(device_name);
    base.is_format_supported = Some(is_format_supported);
    base.context_create = Some(context_create);
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
