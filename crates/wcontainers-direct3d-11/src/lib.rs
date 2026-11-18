//! Our Direct3D 11 front (also serving Direct3D 10): Direct3D's device and immediate context as plain
//! Rust over the `wcontainers-gpu` interface, generic over the back end. The Windows library and the
//! container module (crate `wcontainers-directx`) wrap it in COM objects.

mod ahead;
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
pub mod shader;
pub mod stream;

pub use context::{COMPUTE, GEOMETRY, Mapped, PIXEL, UAV_SLOTS, VERTEX};
pub use device::{Device, Removal, top_level};
pub use error::{Error, Result};
pub use present::Chain;
pub use wcontainers_directx_graphics_infrastructure::{Model, SwapChainDesc};
