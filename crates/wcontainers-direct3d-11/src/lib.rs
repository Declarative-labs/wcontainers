//! Our Direct3D 11 front (also serving Direct3D 10): Direct3D's device and immediate context as plain
//! Rust over the `wcontainers-gpu` interface, generic over the back end. The Windows library and the
//! container module (crate `wcontainers-directx`) wrap it in COM objects.

mod ahead;
pub mod cache;
mod context;
mod counting;
pub mod desc;
mod device;
mod error;
pub mod format;
pub mod object;
mod present;
pub mod query;
pub mod record;
pub mod runtime;
pub mod shader;
pub mod stream;

pub use context::{
    COLOR_COMPONENTS, COMPUTE, Color, DOMAIN, GEOMETRY, HULL, Mapped, PIXEL, RECT_EDGES, Rect,
    STAGES, UAV_SLOTS, VERTEX,
};
pub use device::{CUBE_FACES, Device, FormatSupport, Removal, level_texture_limits, top_level};
pub use error::{Error, Result, WARNING_PREFIX};
pub use present::Chain;
pub use wcontainers_directx_graphics_infrastructure::{Model, SwapChainDesc};
