//! The names of Windows' Direct3D and DXGI runtime, which the Windows library stands in for.

/// The system's libraries whose entry points the Windows library serves, by file name (lower case,
/// as Windows reports a loaded module's path).
pub const DXGI_LIBRARY: &str = "dxgi.dll";
pub const D3D11_LIBRARY: &str = "d3d11.dll";
pub const D3D10_LIBRARY: &str = "d3d10.dll";
pub const D3D10_1_LIBRARY: &str = "d3d10_1.dll";
pub const D3D10CORE_LIBRARY: &str = "d3d10core.dll";

/// The system's entry points the Windows library serves and calls on for callers that are not the
/// program.
pub mod entry {
    use std::ffi::CStr;

    pub const CREATE_DXGI_FACTORY: &CStr = c"CreateDXGIFactory";
    pub const CREATE_DXGI_FACTORY1: &CStr = c"CreateDXGIFactory1";
    pub const CREATE_DXGI_FACTORY2: &CStr = c"CreateDXGIFactory2";
    pub const D3D11_CREATE_DEVICE: &CStr = c"D3D11CreateDevice";
    pub const D3D11_CREATE_DEVICE_AND_SWAP_CHAIN: &CStr = c"D3D11CreateDeviceAndSwapChain";
    pub const D3D10_CREATE_DEVICE: &CStr = c"D3D10CreateDevice";
    pub const D3D10_CREATE_DEVICE1: &CStr = c"D3D10CreateDevice1";
    pub const D3D10_CORE_CREATE_DEVICE: &CStr = c"D3D10CoreCreateDevice";
    pub const D3D10_CORE_GET_VERSION: &CStr = c"D3D10CoreGetVersion";
}
