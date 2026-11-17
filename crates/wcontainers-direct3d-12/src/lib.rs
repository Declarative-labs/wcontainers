//! The Direct3D 12 front: Direct3D 12's rules, answered on the graphics interface. Plain Rust
//! methods named after the API's; the Windows library and the container module are thin shells
//! around them (plan section 10).

pub mod address;
pub mod device;
pub mod features;
pub mod list;
pub mod resource;
pub mod root;
pub mod state;

pub use device::{Device, FenceId, GraphicsPipelineDesc, PipelineId, ResourceId, RootId};

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
