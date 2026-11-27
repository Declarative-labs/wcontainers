//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `GUID::Data4` length, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/guiddef/ns-guiddef-guid).
pub const GUID_DATA4_LENGTH: usize = 8;

/// The header's `E_FAIL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_FAIL: crate::structures::HResult = crate::structures::HResult(0x80004005_u32 as i32);
/// The header's `E_INVALIDARG`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_INVALIDARG: crate::structures::HResult =
    crate::structures::HResult(0x80070057_u32 as i32);
/// The header's `E_NOINTERFACE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_NOINTERFACE: crate::structures::HResult =
    crate::structures::HResult(0x80004002_u32 as i32);
/// The header's `E_NOTIMPL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_NOTIMPL: crate::structures::HResult = crate::structures::HResult(0x80004001_u32 as i32);
/// The header's `E_OUTOFMEMORY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_OUTOFMEMORY: crate::structures::HResult =
    crate::structures::HResult(0x8007000e_u32 as i32);
/// The header's `E_POINTER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const E_POINTER: crate::structures::HResult = crate::structures::HResult(0x80004003_u32 as i32);
/// The header's `S_FALSE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const S_FALSE: crate::structures::HResult = crate::structures::HResult(0x1_u32 as i32);
/// The header's `S_OK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const S_OK: crate::structures::HResult = crate::structures::HResult(0x0_u32 as i32);
