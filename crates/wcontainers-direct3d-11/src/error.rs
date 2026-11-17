//! What a front call answers when it does not succeed, in the API's own terms.

use wcontainers_gpu::GpuError;

/// Why a call failed. The shell turns each into the HRESULT the API documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The description or arguments break a rule of the API (`E_INVALIDARG`).
    InvalidArg(&'static str),
    /// A valid request this front does not serve yet (`E_NOTIMPL`), warned when returned.
    NotImplemented(&'static str),
    /// None of the feature levels asked for is offered (`DXGI_ERROR_UNSUPPORTED`).
    Unsupported,
    /// The resource cannot be mapped now without waiting (`DXGI_ERROR_WAS_STILL_DRAWING`).
    StillDrawing,
    /// The device is lost (`DXGI_ERROR_DEVICE_REMOVED`).
    DeviceRemoved,
}

impl From<GpuError> for Error {
    fn from(e: GpuError) -> Self {
        match e {
            GpuError::Absent(_) => Error::NotImplemented("a feature the back end lacks"),
            _ => Error::DeviceRemoved,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// Returns `NotImplemented` after warning once on standard error.
pub fn not_implemented<T>(what: &'static str) -> Result<T> {
    eprintln!("wcontainers direct3d 11: not implemented: {what}");
    Err(Error::NotImplemented(what))
}
