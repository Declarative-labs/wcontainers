//! Our Direct3D 11 front (also serving Direct3D 10): Direct3D's device and immediate context as plain
//! Rust over the `wcontainers-gpu` interface, generic over the back end. The Windows library and the
//! container module (crate `wcontainers-directx`) wrap it in COM objects.

mod context;
pub mod desc;
mod device;
mod error;
pub mod format;
pub mod object;
pub mod record;
pub mod standin;

pub use context::{Mapped, PIXEL, VERTEX};
pub use device::{Device, top_level};
pub use error::{Error, Result};
