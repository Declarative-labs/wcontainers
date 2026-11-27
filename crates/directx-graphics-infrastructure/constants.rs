//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

use wcontainers_component_object_model as component_object_model;

/// The header's `DXGI_DEBUG_ALL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_ALL: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xe48ae283,
        data2: 0xda80,
        data3: 0x490b,
        data4: [0x87, 0xe6, 0x43, 0xe9, 0xa9, 0xcf, 0xda, 0x08],
    };
/// The header's `DXGI_DEBUG_APP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_APP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x06cd6e01,
        data2: 0x4219,
        data3: 0x4ebd,
        data4: [0x87, 0x09, 0x27, 0xed, 0x23, 0x36, 0x0c, 0x62],
    };
/// The header's `DXGI_DEBUG_BINARY_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_BINARY_VERSION: u32 = 1;
/// The header's `DXGI_DEBUG_DX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_DX: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x35cdd7fc,
        data2: 0x13b2,
        data3: 0x421d,
        data4: [0xa5, 0xd7, 0x7e, 0x44, 0x51, 0x28, 0x7d, 0x64],
    };
/// The header's `DXGI_DEBUG_DXGI`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_DXGI: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x25cddaa4,
        data2: 0xb1c6,
        data3: 0x47e1,
        data4: [0xac, 0x3e, 0x98, 0x87, 0x5b, 0x5a, 0x2e, 0x2a],
    };
/// The header's `DXGI_ERROR_ACCESS_DENIED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_ACCESS_DENIED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a002b_u32 as i32);
/// The header's `DXGI_ERROR_ACCESS_LOST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_ACCESS_LOST: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0026_u32 as i32);
/// The header's `DXGI_ERROR_ALREADY_EXISTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_ALREADY_EXISTS: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0036_u32 as i32);
/// The header's `DXGI_ERROR_CACHE_CORRUPT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_CACHE_CORRUPT: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0033_u32 as i32);
/// The header's `DXGI_ERROR_CACHE_FULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_CACHE_FULL: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0034_u32 as i32);
/// The header's `DXGI_ERROR_CACHE_HASH_COLLISION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_CACHE_HASH_COLLISION: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0035_u32 as i32);
/// The header's `DXGI_ERROR_CANNOT_PROTECT_CONTENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_CANNOT_PROTECT_CONTENT: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a002a_u32 as i32);
/// The header's `DXGI_ERROR_DEVICE_HUNG`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_DEVICE_HUNG: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0006_u32 as i32);
/// The header's `DXGI_ERROR_DEVICE_REMOVED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_DEVICE_REMOVED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0005_u32 as i32);
/// The header's `DXGI_ERROR_DEVICE_RESET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_DEVICE_RESET: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0007_u32 as i32);
/// The header's `DXGI_ERROR_DRIVER_INTERNAL_ERROR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_DRIVER_INTERNAL_ERROR: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0020_u32 as i32);
/// The header's `DXGI_ERROR_DYNAMIC_CODE_POLICY_VIOLATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_DYNAMIC_CODE_POLICY_VIOLATION: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0031_u32 as i32);
/// The header's `DXGI_ERROR_FRAME_STATISTICS_DISJOINT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_FRAME_STATISTICS_DISJOINT: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a000b_u32 as i32);
/// The header's `DXGI_ERROR_GRAPHICS_VIDPN_SOURCE_IN_USE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_GRAPHICS_VIDPN_SOURCE_IN_USE: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a000c_u32 as i32);
/// The header's `DXGI_ERROR_HW_PROTECTION_OUTOFMEMORY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_HW_PROTECTION_OUTOFMEMORY: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0030_u32 as i32);
/// The header's `DXGI_ERROR_INVALID_CALL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_INVALID_CALL: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0001_u32 as i32);
/// The header's `DXGI_ERROR_MODE_CHANGE_IN_PROGRESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_MODE_CHANGE_IN_PROGRESS: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0025_u32 as i32);
/// The header's `DXGI_ERROR_MORE_DATA`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_MORE_DATA: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0003_u32 as i32);
/// The header's `DXGI_ERROR_MPO_UNPINNED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_MPO_UNPINNED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0064_u32 as i32);
/// The header's `DXGI_ERROR_NAME_ALREADY_EXISTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NAME_ALREADY_EXISTS: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a002c_u32 as i32);
/// The header's `DXGI_ERROR_NONEXCLUSIVE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NONEXCLUSIVE: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0021_u32 as i32);
/// The header's `DXGI_ERROR_NON_COMPOSITED_UI`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NON_COMPOSITED_UI: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0032_u32 as i32);
/// The header's `DXGI_ERROR_NOT_CURRENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NOT_CURRENT: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a002e_u32 as i32);
/// The header's `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NOT_CURRENTLY_AVAILABLE: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0022_u32 as i32);
/// The header's `DXGI_ERROR_NOT_FOUND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_NOT_FOUND: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0002_u32 as i32);
/// The header's `DXGI_ERROR_REMOTE_CLIENT_DISCONNECTED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_REMOTE_CLIENT_DISCONNECTED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0023_u32 as i32);
/// The header's `DXGI_ERROR_REMOTE_OUTOFMEMORY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_REMOTE_OUTOFMEMORY: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0024_u32 as i32);
/// The header's `DXGI_ERROR_RESTRICT_TO_OUTPUT_STALE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_RESTRICT_TO_OUTPUT_STALE: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0029_u32 as i32);
/// The header's `DXGI_ERROR_SDK_COMPONENT_MISSING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_SDK_COMPONENT_MISSING: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a002d_u32 as i32);
/// The header's `DXGI_ERROR_SESSION_DISCONNECTED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_SESSION_DISCONNECTED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0028_u32 as i32);
/// The header's `DXGI_ERROR_UNSUPPORTED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_UNSUPPORTED: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0004_u32 as i32);
/// The header's `DXGI_ERROR_WAIT_TIMEOUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_WAIT_TIMEOUT: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a0027_u32 as i32);
/// The header's `DXGI_ERROR_WAS_STILL_DRAWING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ERROR_WAS_STILL_DRAWING: component_object_model::structures::HResult =
    component_object_model::structures::HResult(0x887a000a_u32 as i32);
/// The header's `DXGI_INFO_QUEUE_DEFAULT_MESSAGE_COUNT_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_INFO_QUEUE_DEFAULT_MESSAGE_COUNT_LIMIT: u32 = 0x400;
/// The header's `DXGI_INFO_QUEUE_MESSAGE_ID_STRING_FROM_APPLICATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_INFO_QUEUE_MESSAGE_ID_STRING_FROM_APPLICATION: u32 = 0;
/// The header's `DXGI_MAX_SWAP_CHAIN_BUFFERS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MAX_SWAP_CHAIN_BUFFERS: u32 = 16;
/// The header's `DXGI_CENTER_MULTISAMPLE_QUALITY_PATTERN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CENTER_MULTISAMPLE_QUALITY_PATTERN: u32 = 0xfffffffe;
/// The header's `DXGI_CPU_ACCESS_DYNAMIC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CPU_ACCESS_DYNAMIC: u32 = 1;
/// The header's `DXGI_CPU_ACCESS_FIELD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CPU_ACCESS_FIELD: u32 = 15;
/// The header's `DXGI_CPU_ACCESS_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CPU_ACCESS_NONE: u32 = 0;
/// The header's `DXGI_CPU_ACCESS_READ_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CPU_ACCESS_READ_WRITE: u32 = 2;
/// The header's `DXGI_CPU_ACCESS_SCRATCH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CPU_ACCESS_SCRATCH: u32 = 3;
/// The header's `DXGI_FORMAT_DEFINED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_FORMAT_DEFINED: u32 = 1;
/// The header's `DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN: u32 = 0xffffffff;
/// The header's `_FACDXGI`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const _FACDXGI: u32 = 0x87a;
/// The header's `IID_IDXGIAdapter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiadapter).
pub const IID_IDXGI_ADAPTER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x2411e7e1,
        data2: 0x12ac,
        data3: 0x4ccf,
        data4: [0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0],
    };
/// The header's `IID_IDXGIAdapter1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiadapter1).
pub const IID_IDXGI_ADAPTER1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x29038f61,
        data2: 0x3839,
        data3: 0x4626,
        data4: [0x91, 0xfd, 0x08, 0x68, 0x79, 0x01, 0x1a, 0x05],
    };
/// The header's `IID_IDXGIAdapter2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiadapter2).
pub const IID_IDXGI_ADAPTER2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x0aa1ae0a,
        data2: 0xfa0e,
        data3: 0x4b84,
        data4: [0x86, 0x44, 0xe0, 0x5f, 0xf8, 0xe5, 0xac, 0xb5],
    };
/// The header's `IID_IDXGIAdapter3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgiadapter3).
pub const IID_IDXGI_ADAPTER3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x645967a4,
        data2: 0x1392,
        data3: 0x4310,
        data4: [0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd],
    };
/// The header's `IID_IDXGIAdapter4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgiadapter4).
pub const IID_IDXGI_ADAPTER4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3c8d99d1,
        data2: 0x4fbf,
        data3: 0x4181,
        data4: [0xa8, 0x2c, 0xaf, 0x66, 0xbf, 0x7b, 0xd2, 0x4e],
    };
/// The header's `IID_IDXGIDebug`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgidebug).
pub const IID_IDXGI_DEBUG: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x119e7452,
        data2: 0xde9e,
        data3: 0x40fe,
        data4: [0x88, 0x06, 0x88, 0xf9, 0x0c, 0x12, 0xb4, 0x41],
    };
/// The header's `IID_IDXGIDebug1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgidebug1).
pub const IID_IDXGI_DEBUG1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc5a05f0c,
        data2: 0x16f2,
        data3: 0x4adf,
        data4: [0x9f, 0x4d, 0xa8, 0xc4, 0xd5, 0x8a, 0xc5, 0x50],
    };
/// The header's `IID_IDXGIDecodeSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgidecodeswapchain).
pub const IID_IDXGI_DECODE_SWAP_CHAIN: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x2633066b,
        data2: 0x4514,
        data3: 0x4c7a,
        data4: [0x8f, 0xd8, 0x12, 0xea, 0x98, 0x05, 0x9d, 0x18],
    };
/// The header's `IID_IDXGIDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevice).
pub const IID_IDXGI_DEVICE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x54ec77fa,
        data2: 0x1377,
        data3: 0x44e6,
        data4: [0x8c, 0x32, 0x88, 0xfd, 0x5f, 0x44, 0xc8, 0x4c],
    };
/// The header's `IID_IDXGIDevice1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevice1).
pub const IID_IDXGI_DEVICE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x77db970f,
        data2: 0x6276,
        data3: 0x48ba,
        data4: [0xba, 0x28, 0x07, 0x01, 0x43, 0xb4, 0x39, 0x2c],
    };
/// The header's `IID_IDXGIDevice2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgidevice2).
pub const IID_IDXGI_DEVICE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x05008617,
        data2: 0xfbfd,
        data3: 0x4051,
        data4: [0xa7, 0x90, 0x14, 0x48, 0x84, 0xb4, 0xf6, 0xa9],
    };
/// The header's `IID_IDXGIDevice3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgidevice3).
pub const IID_IDXGI_DEVICE3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6007896c,
        data2: 0x3244,
        data3: 0x4afd,
        data4: [0xbf, 0x18, 0xa6, 0xd3, 0xbe, 0xda, 0x50, 0x23],
    };
/// The header's `IID_IDXGIDevice4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgidevice4).
pub const IID_IDXGI_DEVICE4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x95b4f95f,
        data2: 0xd8da,
        data3: 0x4ca4,
        data4: [0x9e, 0xe6, 0x3b, 0x76, 0xd5, 0x96, 0x8a, 0x10],
    };
/// The header's `IID_IDXGIDeviceSubObject`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevicesubobject).
pub const IID_IDXGI_DEVICE_SUB_OBJECT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3d3e0379,
        data2: 0xf9de,
        data3: 0x4d58,
        data4: [0xbb, 0x6c, 0x18, 0xd6, 0x29, 0x92, 0xf1, 0xa6],
    };
/// The header's `IID_IDXGIDisplayControl`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgidisplaycontrol).
pub const IID_IDXGI_DISPLAY_CONTROL: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xea9dbf1a,
        data2: 0xc88e,
        data3: 0x4486,
        data4: [0x85, 0x4a, 0x98, 0xaa, 0x01, 0x38, 0xf3, 0x0c],
    };
/// The header's `IID_IDXGIFactory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgifactory).
pub const IID_IDXGI_FACTORY: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x7b7166ec,
        data2: 0x21c7,
        data3: 0x44ae,
        data4: [0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69],
    };
/// The header's `IID_IDXGIFactory1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgifactory1).
pub const IID_IDXGI_FACTORY1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x770aae78,
        data2: 0xf26f,
        data3: 0x4dba,
        data4: [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87],
    };
/// The header's `IID_IDXGIFactory2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgifactory2).
pub const IID_IDXGI_FACTORY2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x50c83a1c,
        data2: 0xe072,
        data3: 0x4c48,
        data4: [0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0],
    };
/// The header's `IID_IDXGIFactory3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgifactory3).
pub const IID_IDXGI_FACTORY3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x25483823,
        data2: 0xcd46,
        data3: 0x4c7d,
        data4: [0x86, 0xca, 0x47, 0xaa, 0x95, 0xb8, 0x37, 0xbd],
    };
/// The header's `IID_IDXGIFactory4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgifactory4).
pub const IID_IDXGI_FACTORY4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1bc6ea02,
        data2: 0xef36,
        data3: 0x464f,
        data4: [0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a],
    };
/// The header's `IID_IDXGIFactory5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgifactory5).
pub const IID_IDXGI_FACTORY5: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x7632e1f5,
        data2: 0xee65,
        data3: 0x4dca,
        data4: [0x87, 0xfd, 0x84, 0xcd, 0x75, 0xf8, 0x83, 0x8d],
    };
/// The header's `IID_IDXGIFactory6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgifactory6).
pub const IID_IDXGI_FACTORY6: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc1b6694f,
        data2: 0xff09,
        data3: 0x44a9,
        data4: [0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17],
    };
/// The header's `IID_IDXGIFactory7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgifactory7).
pub const IID_IDXGI_FACTORY7: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa4966eed,
        data2: 0x76db,
        data3: 0x44da,
        data4: [0x84, 0xc1, 0xee, 0x9a, 0x7a, 0xfb, 0x20, 0xa8],
    };
/// The header's `IID_IDXGIFactoryMedia`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgifactorymedia).
pub const IID_IDXGI_FACTORY_MEDIA: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x41e7d1f2,
        data2: 0xa591,
        data3: 0x4f7b,
        data4: [0xa2, 0xe5, 0xfa, 0x9c, 0x84, 0x3e, 0x1c, 0x12],
    };
/// The header's `IID_IDXGIInfoQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgiinfoqueue).
pub const IID_IDXGI_INFO_QUEUE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xd67441c7,
        data2: 0x672a,
        data3: 0x476f,
        data4: [0x9e, 0x82, 0xcd, 0x55, 0xb4, 0x49, 0x49, 0xce],
    };
/// The header's `IID_IDXGIKeyedMutex`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgikeyedmutex).
pub const IID_IDXGI_KEYED_MUTEX: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9d8e1289,
        data2: 0xd7b3,
        data3: 0x465f,
        data4: [0x81, 0x26, 0x25, 0x0e, 0x34, 0x9a, 0xf8, 0x5d],
    };
/// The header's `IID_IDXGIObject`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiobject).
pub const IID_IDXGI_OBJECT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xaec22fb8,
        data2: 0x76f3,
        data3: 0x4639,
        data4: [0x9b, 0xe0, 0x28, 0xeb, 0x43, 0xa6, 0x7a, 0x2e],
    };
/// The header's `IID_IDXGIOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgioutput).
pub const IID_IDXGI_OUTPUT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xae02eedb,
        data2: 0xc735,
        data3: 0x4690,
        data4: [0x8d, 0x52, 0x5a, 0x8d, 0xc2, 0x02, 0x13, 0xaa],
    };
/// The header's `IID_IDXGIOutput1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgioutput1).
pub const IID_IDXGI_OUTPUT1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x00cddea8,
        data2: 0x939b,
        data3: 0x4b83,
        data4: [0xa3, 0x40, 0xa6, 0x85, 0x22, 0x66, 0x66, 0xcc],
    };
/// The header's `IID_IDXGIOutput2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgioutput2).
pub const IID_IDXGI_OUTPUT2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x595e39d1,
        data2: 0x2724,
        data3: 0x4663,
        data4: [0x99, 0xb1, 0xda, 0x96, 0x9d, 0xe2, 0x83, 0x64],
    };
/// The header's `IID_IDXGIOutput3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgioutput3).
pub const IID_IDXGI_OUTPUT3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x8a6bb301,
        data2: 0x7e7e,
        data3: 0x41f4,
        data4: [0xa8, 0xe0, 0x5b, 0x32, 0xf7, 0xf9, 0x9b, 0x18],
    };
/// The header's `IID_IDXGIOutput4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgioutput4).
pub const IID_IDXGI_OUTPUT4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xdc7dca35,
        data2: 0x2196,
        data3: 0x414d,
        data4: [0x9f, 0x53, 0x61, 0x78, 0x84, 0x03, 0x2a, 0x60],
    };
/// The header's `IID_IDXGIOutput5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgioutput5).
pub const IID_IDXGI_OUTPUT5: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x80a07424,
        data2: 0xab52,
        data3: 0x42eb,
        data4: [0x83, 0x3c, 0x0c, 0x42, 0xfd, 0x28, 0x2d, 0x98],
    };
/// The header's `IID_IDXGIOutput6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgioutput6).
pub const IID_IDXGI_OUTPUT6: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x068346e8,
        data2: 0xaaec,
        data3: 0x4b84,
        data4: [0xad, 0xd7, 0x13, 0x7f, 0x51, 0x3f, 0x77, 0xa1],
    };
/// The header's `IID_IDXGIOutputDuplication`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgioutputduplication).
pub const IID_IDXGI_OUTPUT_DUPLICATION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x191cfac3,
        data2: 0xa341,
        data3: 0x470d,
        data4: [0xb2, 0x6e, 0xa8, 0x64, 0xf4, 0x28, 0x31, 0x9c],
    };
/// The header's `IID_IDXGIResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiresource).
pub const IID_IDXGI_RESOURCE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x035f3ab4,
        data2: 0x482e,
        data3: 0x4e50,
        data4: [0xb4, 0x1f, 0x8a, 0x7f, 0x8b, 0xd8, 0x96, 0x0b],
    };
/// The header's `IID_IDXGIResource1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiresource1).
pub const IID_IDXGI_RESOURCE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x30961379,
        data2: 0x4609,
        data3: 0x4a41,
        data4: [0x99, 0x8e, 0x54, 0xfe, 0x56, 0x7e, 0xe0, 0xc1],
    };
/// The header's `IID_IDXGISurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgisurface).
pub const IID_IDXGI_SURFACE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xcafcb56c,
        data2: 0x6ac3,
        data3: 0x4889,
        data4: [0xbf, 0x47, 0x9e, 0x23, 0xbb, 0xd2, 0x60, 0xec],
    };
/// The header's `IID_IDXGISurface1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgisurface1).
pub const IID_IDXGI_SURFACE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x4ae63092,
        data2: 0x6327,
        data3: 0x4c1b,
        data4: [0x80, 0xae, 0xbf, 0xe1, 0x2e, 0xa3, 0x2b, 0x86],
    };
/// The header's `IID_IDXGISurface2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgisurface2).
pub const IID_IDXGI_SURFACE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xaba496dd,
        data2: 0xb617,
        data3: 0x4cb8,
        data4: [0xa8, 0x66, 0xbc, 0x44, 0xd7, 0xeb, 0x1f, 0xa2],
    };
/// The header's `IID_IDXGISwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiswapchain).
pub const IID_IDXGI_SWAP_CHAIN: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x310d36a0,
        data2: 0xd2e7,
        data3: 0x4c0a,
        data4: [0xaa, 0x04, 0x6a, 0x9d, 0x23, 0xb8, 0x88, 0x6a],
    };
/// The header's `IID_IDXGISwapChain1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiswapchain1).
pub const IID_IDXGI_SWAP_CHAIN1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x790a45f7,
        data2: 0x0d42,
        data3: 0x4876,
        data4: [0x98, 0x3a, 0x0a, 0x55, 0xcf, 0xe6, 0xf4, 0xaa],
    };
/// The header's `IID_IDXGISwapChain2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgiswapchain2).
pub const IID_IDXGI_SWAP_CHAIN2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa8be2ac4,
        data2: 0x199f,
        data3: 0x4946,
        data4: [0xb3, 0x31, 0x79, 0x59, 0x9f, 0xb9, 0x8d, 0xe7],
    };
/// The header's `IID_IDXGISwapChain3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgiswapchain3).
pub const IID_IDXGI_SWAP_CHAIN3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x94d99bdb,
        data2: 0xf1f8,
        data3: 0x4ab0,
        data4: [0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1],
    };
/// The header's `IID_IDXGISwapChain4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgiswapchain4).
pub const IID_IDXGI_SWAP_CHAIN4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3d585d5a,
        data2: 0xbd4a,
        data3: 0x489e,
        data4: [0xb1, 0xf4, 0x3d, 0xbc, 0xb6, 0x45, 0x2f, 0xfb],
    };
/// The header's `IID_IDXGISwapChainMedia`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgiswapchainmedia).
pub const IID_IDXGI_SWAP_CHAIN_MEDIA: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xdd95b90b,
        data2: 0xf05f,
        data3: 0x4f6a,
        data4: [0xbd, 0x65, 0x25, 0xbf, 0xb2, 0x64, 0xbd, 0x84],
    };
/// The header's `IID_IDXGraphicsAnalysis`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const IID_IDX_GRAPHICS_ANALYSIS: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9f251514,
        data2: 0x9d4d,
        data3: 0x4902,
        data4: [0x9d, 0x60, 0x18, 0x98, 0x8a, 0xb7, 0xd4, 0xb5],
    };
/// The header's `DXGI_ADAPTER_DESC1::Description`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ADAPTER_DESC1_DESCRIPTION_LENGTH: usize = 128;
/// The header's `DXGI_ADAPTER_DESC2::Description`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ADAPTER_DESC2_DESCRIPTION_LENGTH: usize = 128;
/// The header's `DXGI_ADAPTER_DESC3::Description`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ADAPTER_DESC3_DESCRIPTION_LENGTH: usize = 128;
/// The header's `DXGI_ADAPTER_DESC::Description`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_ADAPTER_DESC_DESCRIPTION_LENGTH: usize = 128;
/// The header's `DXGI_DISPLAY_COLOR_SPACE::PrimaryCoordinates`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DISPLAY_COLOR_SPACE_PRIMARYCOORDINATES_LENGTH: usize = 16;
/// The header's `DXGI_DISPLAY_COLOR_SPACE::WhitePoints`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DISPLAY_COLOR_SPACE_WHITEPOINTS_LENGTH: usize = 32;
/// The header's `DXGI_GAMMA_CONTROL_CAPABILITIES::ControlPointPositions`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_GAMMA_CONTROL_CAPABILITIES_CONTROLPOINTPOSITIONS_LENGTH: usize = 1025;
/// The header's `DXGI_GAMMA_CONTROL::GammaCurve`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_GAMMA_CONTROL_GAMMACURVE_LENGTH: usize = 1025;
/// The header's `DXGI_HDR_METADATA_HDR10PLUS::Data`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_HDR_METADATA_HDR10PLUS_DATA_LENGTH: usize = 72;
/// The header's `DXGI_HDR_METADATA_HDR10::BluePrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_HDR_METADATA_HDR10_BLUEPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_HDR_METADATA_HDR10::GreenPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_HDR_METADATA_HDR10_GREENPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_HDR_METADATA_HDR10::RedPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_HDR_METADATA_HDR10_REDPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_HDR_METADATA_HDR10::WhitePoint`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_HDR_METADATA_HDR10_WHITEPOINT_LENGTH: usize = 2;
/// The header's `DXGI_JPEG_AC_HUFFMAN_TABLE::CodeCounts`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_JPEG_AC_HUFFMAN_TABLE_CODECOUNTS_LENGTH: usize = 16;
/// The header's `DXGI_JPEG_AC_HUFFMAN_TABLE::CodeValues`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_JPEG_AC_HUFFMAN_TABLE_CODEVALUES_LENGTH: usize = 162;
/// The header's `DXGI_JPEG_DC_HUFFMAN_TABLE::CodeCounts`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_JPEG_DC_HUFFMAN_TABLE_CODECOUNTS_LENGTH: usize = 12;
/// The header's `DXGI_JPEG_DC_HUFFMAN_TABLE::CodeValues`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_JPEG_DC_HUFFMAN_TABLE_CODEVALUES_LENGTH: usize = 12;
/// The header's `DXGI_JPEG_QUANTIZATION_TABLE::Elements`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_JPEG_QUANTIZATION_TABLE_ELEMENTS_LENGTH: usize = 64;
/// The header's `DXGI_OUTPUT_DESC1::BluePrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC1_BLUEPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_OUTPUT_DESC1::DeviceName`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC1_DEVICENAME_LENGTH: usize = 32;
/// The header's `DXGI_OUTPUT_DESC1::GreenPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC1_GREENPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_OUTPUT_DESC1::RedPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC1_REDPRIMARY_LENGTH: usize = 2;
/// The header's `DXGI_OUTPUT_DESC1::WhitePoint`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC1_WHITEPOINT_LENGTH: usize = 2;
/// The header's `DXGI_OUTPUT_DESC::DeviceName`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTPUT_DESC_DEVICENAME_LENGTH: usize = 32;
