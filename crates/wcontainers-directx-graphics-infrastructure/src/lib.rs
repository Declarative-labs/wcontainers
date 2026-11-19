//! What Direct3D 10, 11 and 12 share, built once on the graphics interface: the adapter DXGI
//! enumerates (answered from the back end's [`Caps`]), the outputs the host shows pictures on, swap
//! chains with their back buffers, presenting, frame pacing, and what each DXGI format can be used
//! for. Fronts hold a [`SwapChain`] per DXGI swap chain and call it with their own back end.

mod adapter;
pub mod d3d;
mod format;
mod output;
mod support;
mod swapchain;

pub use adapter::{ADAPTER_LUID, AdapterDesc};
pub use format::{
    BACK_BUFFER_FORMATS, dxgi, dxgi_format, dxgi_vertex_format, present_format, present_formats,
    typeless_group,
};
pub use output::{COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB, DisplayMode, OutputDesc, presents_in, sdr};
pub use support::{FormatSupport, format_support, plane_count, support1, support2, takes_samples};
pub use swapchain::{
    DEFAULT_LATENCY, MAX_BUFFERS, Model, SwapChain, SwapChainDesc, SwapChainError,
};

use wcontainers_gpu::Caps;

/// The adapters a factory enumerates: the one the back end draws on.
pub fn adapters(caps: &Caps) -> [AdapterDesc; 1] {
    [AdapterDesc::from_caps(caps)]
}
