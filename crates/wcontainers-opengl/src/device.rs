//! The device the screen stands for: the WebGPU back end on the host's GPU, and the entry points
//! the Gallium bridge (`bridge/screen.h`) calls.

use std::ffi::{CString, c_char};

use wcontainers_gpu::Backend;
use wcontainers_gpu_webgpu::WebGpu;

/// What the device can do, in the terms Gallium's caps take (`struct wc_gl_limits`).
#[repr(C)]
pub struct Limits {
    pub max_texture_2d: u32,
    pub max_texture_cube: u32,
    pub max_texture_3d: u32,
    pub max_texture_layers: u32,
    pub color_targets: u32,
    pub vertex_streams: u32,
    pub max_vertex_stride: u32,
    pub max_point_size: f32,
}

/// The device behind one Gallium screen.
pub struct Device {
    pub backend: WebGpu,
    name: CString,
}

/// The `wgpu` back ends a native build draws through: Direct3D 12 on Windows, as browsers'
/// WebGPU does there, and Vulkan elsewhere.
#[cfg(windows)]
const HOST_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(not(windows))]
const HOST_BACKENDS: wgpu::Backends = wgpu::Backends::VULKAN;

impl Device {
    /// The back end on the most capable GPU `wgpu` finds, or none.
    pub fn new() -> Option<Device> {
        #[cfg(windows)]
        stay_loaded()?;
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = HOST_BACKENDS;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .ok()?;
        let descriptor = WebGpu::device_descriptor(&adapter);
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).ok()?;
        let backend = WebGpu::new(&adapter, device, queue);
        let name = CString::new(backend.caps().adapter.name.replace('\0', "")).ok()?;
        Some(Device { backend, name })
    }

    pub fn limits(&self) -> Limits {
        let limits = &self.backend.caps().limits;
        Limits {
            max_texture_2d: limits.max_texture_2d,
            max_texture_cube: limits.max_texture_cube,
            max_texture_3d: limits.max_texture_3d,
            max_texture_layers: limits.max_texture_layers,
            color_targets: limits.color_targets,
            vertex_streams: limits.vertex_streams,
            max_vertex_stride: limits.max_vertex_stride,
            max_point_size: limits.max_point_size,
        }
    }
}

/// Keeps the module holding the driver loaded until the process ends. Mesa's WGL target destroys
/// its screen when the module is unloaded, inside `DllMain` where Windows holds the loader lock,
/// and releasing the `wgpu` device there never returns (seen under glcts, which unloads
/// `opengl32.dll` before it exits); a module that stays loaded is only detached at the process's
/// end, where Mesa releases nothing.
#[cfg(windows)]
fn stay_loaded() -> Option<()> {
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
    };
    let mut module = std::ptr::null_mut();
    let address = stay_loaded as *const u16;
    // SAFETY: the address is in this module and `module` is writable.
    let pinned = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            address,
            &mut module,
        )
    };
    (pinned != 0).then_some(())
}

/// # Safety
/// `limits` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wc_gl_device_create(limits: *mut Limits) -> *mut Device {
    let Some(device) = Device::new() else {
        return std::ptr::null_mut();
    };
    // SAFETY: the caller's.
    unsafe { limits.write(device.limits()) };
    Box::into_raw(Box::new(device))
}

/// # Safety
/// `device` came from `wc_gl_device_create` and is not used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wc_gl_device_destroy(device: *mut Device) {
    // SAFETY: the caller's.
    drop(unsafe { Box::from_raw(device) });
}

/// # Safety
/// `device` came from `wc_gl_device_create` and is alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wc_gl_device_name(device: *const Device) -> *const c_char {
    // SAFETY: the caller's.
    unsafe { &*device }.name.as_ptr()
}
