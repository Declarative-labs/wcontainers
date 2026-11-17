//! What Direct3D 10, 11 and 12 share, built once on the graphics interface: the adapter DXGI
//! enumerates (answered from the back end's [`Caps`]), the outputs the host shows pictures on, swap
//! chains with their back buffers, presenting, frame pacing, and what each DXGI format can be used
//! for. Fronts hold a [`SwapChain`] per DXGI swap chain and call it with their own back end.

mod adapter;
mod format;
mod output;
mod support;
mod swapchain;
mod wgsl;

pub use adapter::{ADAPTER_LUID, AdapterDesc};
pub use format::{
    BACK_BUFFER_FORMATS, dxgi_format, dxgi_vertex_format, present_format, present_formats,
};
pub use output::{DisplayMode, OutputDesc};
pub use support::{FormatSupport, format_support, plane_count, support1, support2, takes_samples};
pub use swapchain::{
    DEFAULT_LATENCY, MAX_BUFFERS, Model, SwapChain, SwapChainDesc, SwapChainError,
};
pub use wgsl::program_from_wgsl;

use wcontainers_gpu::Caps;

/// The adapters a factory enumerates: the one the back end draws on.
pub fn adapters(caps: &Caps) -> [AdapterDesc; 1] {
    [AdapterDesc::from_caps(caps)]
}
