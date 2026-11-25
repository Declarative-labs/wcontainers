//! The Direct3D 12 front: Direct3D 12's rules, answered on the graphics interface. Plain Rust
//! methods named after the API's; the Windows library and the container module are thin shells
//! around them (plan section 10).

// The package waits on its Windows library and tests to inherit the workspace's ban; the front
// itself is held to it now.
#![forbid(unsafe_code)]

pub mod address;
pub mod cache;
pub mod device;
pub mod dxbc;
pub mod dxil;
pub mod features;
pub mod list;
pub mod resource;
pub mod root;
pub mod state;

pub use device::{
    Device, FenceId, GraphicsPipelineDesc, HeapId, MemoryId, PipelineId, QueryHeapId, QueueId,
    ResourceId,
};

/// Why a call failed, as the shells turn it into an `HRESULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The call breaks Direct3D 12's rules (`E_INVALIDARG`).
    Invalid(&'static str),
    /// The front does not do this yet (`E_NOTIMPL`); never answered as success.
    NotImplemented(&'static str),
    /// `E_OUTOFMEMORY`.
    OutOfMemory,
    /// `DXGI_ERROR_DEVICE_REMOVED`; `Device::removed` says why.
    DeviceRemoved,
}
