//! The device the screen stands for: the WebGPU back end on the host's GPU, and the host's windows.

use std::ffi::{CString, c_void};

use wcontainers_gpu::Backend;
use wcontainers_gpu_webgpu::WebGpu;

/// What the driver needs of a back end beside the interface: the host's windows, which OpenGL
/// names by their device contexts.
pub trait HostWindows: Backend {
    /// Registers the window a device context draws into as host window `id`; false when the
    /// back end cannot present to it.
    fn add_window_of(&mut self, id: u32, device_context: *mut c_void) -> bool;
}

/// The `wgpu` back ends a native build draws through: Direct3D 12 on Windows, as browsers'
/// WebGPU does there, and Vulkan elsewhere.
#[cfg(windows)]
const HOST_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(not(windows))]
const HOST_BACKENDS: wgpu::Backends = wgpu::Backends::VULKAN;

/// The `wgpu` instance the device and the windows' surfaces come from.
static INSTANCE: std::sync::OnceLock<wgpu::Instance> = std::sync::OnceLock::new();

/// The back end on the most capable GPU `wgpu` finds, with the GPU's name; or none.
pub fn webgpu() -> Option<(WebGpu, CString)> {
    #[cfg(windows)]
    stay_loaded()?;
    let instance = INSTANCE.get_or_init(|| {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = HOST_BACKENDS;
        desc.flags |= WebGpu::INSTANCE_FLAGS;
        wgpu::Instance::new(desc)
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let descriptor = WebGpu::device_descriptor(&adapter);
    let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).ok()?;
    let backend = WebGpu::new(&adapter, device, queue);
    let name = backend.caps().adapter.name.bytes().filter(|&b| b != 0);
    let name = CString::new(name.collect::<Vec<u8>>()).ok()?;
    Some((backend, name))
}

impl HostWindows for WebGpu {
    #[cfg(windows)]
    fn add_window_of(&mut self, id: u32, device_context: *mut c_void) -> bool {
        use std::num::NonZeroIsize;
        use windows_sys::Win32::Graphics::Gdi::WindowFromDC;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GWLP_HINSTANCE, GetWindowLongPtrW};
        let Some(instance) = INSTANCE.get() else {
            return false;
        };
        // SAFETY: asks which window a device context the program passed draws into.
        let window = unsafe { WindowFromDC(device_context) };
        let Some(handle) = NonZeroIsize::new(window as isize) else {
            return false;
        };
        let mut handle = wgpu::rwh::Win32WindowHandle::new(handle);
        // SAFETY: reads a field of the program's window.
        handle.hinstance = NonZeroIsize::new(unsafe { GetWindowLongPtrW(window, GWLP_HINSTANCE) });
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(handle),
        };
        // SAFETY: OpenGL's program keeps its window while it draws into it.
        let Ok(surface) = (unsafe { instance.create_surface_unsafe(target) }) else {
            return false;
        };
        self.add_window(id, surface);
        true
    }

    #[cfg(not(windows))]
    fn add_window_of(&mut self, _: u32, _: *mut c_void) -> bool {
        false
    }
}

/// Keeps the module holding the driver loaded until the process ends. Mesa's WGL target destroys
/// its screen when the module is unloaded, inside `DllMain` where Windows holds the loader lock,
/// and releasing the `wgpu` device there never returns (seen under glcts, which unloads
/// `opengl32.dll` before it exits); a module that stays loaded is only detached at the process's
/// end, where Mesa releases nothing.
#[cfg(windows)]
fn stay_loaded() -> Option<()> {
    libloading::os::windows::Library::open_already_loaded(DRIVER_MODULE)
        .and_then(|module| module.pin())
        .ok()
}

/// The module Mesa's WGL target links the driver into (`libgallium_wgl`, `name_prefix : ''`, in
/// `src/gallium/targets/wgl/meson.build`).
#[cfg(windows)]
const DRIVER_MODULE: &str = "libgallium_wgl.dll";
