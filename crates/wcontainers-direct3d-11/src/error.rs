//! What a front call answers when it does not succeed, in the API's own terms.

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
    /// The back end has no memory for the object (`E_OUTOFMEMORY`).
    OutOfMemory,
    /// The device is lost (`DXGI_ERROR_DEVICE_REMOVED`).
    DeviceRemoved,
    /// A call DXGI refuses in the object's present state (`DXGI_ERROR_INVALID_CALL`).
    InvalidCall(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;

/// What this front's warnings on standard error start with.
pub(crate) const WARNING_PREFIX: &str = "wcontainers direct3d 11";

/// Returns `NotImplemented` after warning once on standard error.
pub fn not_implemented<T>(what: &'static str) -> Result<T> {
    eprintln!("{WARNING_PREFIX}: not implemented: {what}");
    Err(Error::NotImplemented(what))
}
