//! The back end the library draws through: WebGPU on `wgpu`'s Direct3D 12, or, built with the
//! `webgl` feature, WebGL 2 on ANGLE's OpenGL ES 3.0 (a desktop test route; ANGLE is never shipped).

#[cfg(feature = "webgl")]
pub use webgl::*;
#[cfg(not(feature = "webgl"))]
pub use webgpu::*;

#[cfg(not(feature = "webgl"))]
mod webgpu {
    use std::num::NonZeroIsize;

    use windows::Win32::Foundation::{E_INVALIDARG, HWND};
    use windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{GWLP_HINSTANCE, GetWindowLongPtrW};
    use windows::core::{Error, Result};

    pub type Chosen = wcontainers_gpu_webgpu::WebGpu;

    /// The environment variable naming WebGPU features taken as missing.
    pub const FEATURES_OFF: &str = "WEBGPU_OFF";

    /// The `wgpu` instance on Direct3D 12, made once: back ends and the program's windows' surfaces
    /// come from it.
    fn instance() -> &'static wgpu::Instance {
        static INSTANCE: std::sync::OnceLock<wgpu::Instance> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::DX12;
            wgpu::Instance::new(desc)
        })
    }

    /// Makes the back end on the most capable GPU `wgpu` finds through Direct3D 12, or none if there
    /// is none. Direct3D 12 is what browsers' WebGPU draws through on Windows, and it rasterises
    /// aliased lines by Direct3D's rule, which `wgpu`'s Vulkan path does not. The WebGPU features
    /// [`FEATURES_OFF`] lists by their kebab-case names (separated by spaces) are taken as missing, as
    /// browsers without them lack them.
    pub fn make() -> Option<Chosen> {
        let adapter =
            pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            }))
            .ok()?;
        let mut descriptor = Chosen::device_descriptor(&adapter);
        for name in std::env::var(FEATURES_OFF)
            .unwrap_or_default()
            .split_whitespace()
        {
            match name.parse::<wgpu::Features>() {
                Ok(f) => descriptor.required_features.remove(f),
                Err(_) => eprintln!("wcontainers direct3d 11: no WebGPU feature {name}"),
            }
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).ok()?;
        Some(Chosen::new(&adapter, device, queue))
    }

    /// Gives `backend` the program's window `hwnd` as host window `id`.
    pub fn add_window(backend: &mut Chosen, id: u32, hwnd: HWND) -> Result<()> {
        // SAFETY: reads a field of the program's window.
        let module = unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) };
        // SAFETY: asks for the program's module, which lives as long as the process.
        let program = unsafe { GetModuleHandleW(None) }.map_or(0, |m| m.0 as isize);
        let mut handle = wgpu::rwh::Win32WindowHandle::new(
            NonZeroIsize::new(hwnd.0 as isize).ok_or(E_INVALIDARG)?,
        );
        handle.hinstance = NonZeroIsize::new(module).or(NonZeroIsize::new(program));
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(handle),
        };
        // SAFETY: the program keeps its window while a swap chain uses it, as DXGI requires.
        let surface = unsafe { instance().create_surface_unsafe(target) }.map_err(|e| {
            eprintln!("wcontainers direct3d 11: no surface for the window: {e}");
            Error::from(DXGI_ERROR_INVALID_CALL)
        })?;
        backend.add_window(id, surface);
        Ok(())
    }

    /// The directory of libraries the back end loads that call Direct3D themselves: none.
    pub fn own_directory() -> Option<&'static str> {
        None
    }
}

#[cfg(feature = "webgl")]
mod webgl {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use windows::Win32::Foundation::HWND;
    use windows::core::Result;

    pub type Chosen = wcontainers_gpu_webgl::WebGl;

    /// The environment variable naming WebGL extensions taken as missing.
    pub const FEATURES_OFF: &str = "WEBGL_OFF";

    /// The directory of ANGLE's libEGL.dll and libGLESv2.dll: `WEBGL_ANGLE_DIR`, as the WebGL 2 back
    /// end's tests name it, or where the desktop's build puts them.
    fn angle_dir() -> &'static PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| {
            std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
                || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
                PathBuf::from,
            )
        })
    }

    /// Makes the back end on ANGLE's OpenGL ES 3.0 context, or none if ANGLE cannot make one. The
    /// WebGL extensions [`FEATURES_OFF`] lists (separated by spaces) are taken as missing, as browsers
    /// without them lack them.
    pub fn make() -> Option<Chosen> {
        let off = std::env::var(FEATURES_OFF).unwrap_or_default();
        let off: Vec<&str> = off.split_whitespace().collect();
        Chosen::on_angle(angle_dir(), &off)
            .map_err(|e| eprintln!("wcontainers direct3d 11: no WebGL 2 back end: {e}"))
            .ok()
    }

    /// Gives `backend` the program's window `hwnd` as host window `id`.
    pub fn add_window(backend: &mut Chosen, id: u32, hwnd: HWND) -> Result<()> {
        // SAFETY: the program keeps its window while a swap chain uses it, as DXGI requires.
        unsafe { backend.add_window(id, hwnd.0) };
        Ok(())
    }

    /// The directory of libraries the back end loads that call Direct3D themselves: ANGLE's, which
    /// loads d3d11.dll and dxgi.dll by name, so in a process holding ours it reaches this library.
    /// Lower case, ending in a backslash.
    pub fn own_directory() -> Option<&'static str> {
        static DIR: OnceLock<String> = OnceLock::new();
        let dir = DIR.get_or_init(|| {
            let dir = angle_dir().to_string_lossy().to_lowercase();
            dir.trim_end_matches('\\').to_string() + "\\"
        });
        Some(dir)
    }
}
