//! Errors and device loss.

use crate::caps::Features;

/// Why a call failed. Fronts turn each into their API's own error (`E_OUTOFMEMORY`,
/// `DXGI_ERROR_DEVICE_REMOVED`, `GL_OUT_OF_MEMORY`, `VK_ERROR_DEVICE_LOST` and so on).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuError {
    /// The back end does not have these features; the front should have read [`crate::Caps`] first.
    Absent(Features),
    /// The GPU or the host ran out of memory for this object.
    OutOfMemory,
    /// The device is gone; every later call fails the same way until a new back end is made.
    DeviceLost,
    /// The call broke a rule of this interface. Always a bug in the front, reported with its rule.
    Invalid(&'static str),
}

/// The device's state, polled by fronts when they present and when they wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceStatus {
    Ok,
    /// Lost, with the reason the host gave (driver reset, the page hidden too long, an error).
    Lost(LossReason),
}

/// Why a device was lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LossReason {
    /// The GPU driver or the browser reset it.
    Reset,
    /// The host took it away (the browser destroyed it, or a native device was removed).
    Removed,
    /// A command was invalid on the back end's own API. A bug, here or in a front.
    Error,
}
