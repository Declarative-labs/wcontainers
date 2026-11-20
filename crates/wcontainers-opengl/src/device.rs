//! The device the screen stands for: the WebGPU back end on the host's GPU.

use std::ffi::{CStr, CString};

use wcontainers_gpu::Backend;
use wcontainers_gpu_webgpu::WebGpu;

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
        let name = backend.caps().adapter.name.bytes().filter(|&b| b != 0);
        let name = CString::new(name.collect::<Vec<u8>>()).ok()?;
        Some(Device { backend, name })
    }

    /// The GPU's name, as its host gives it.
    pub fn name(&self) -> &CStr {
        &self.name
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
