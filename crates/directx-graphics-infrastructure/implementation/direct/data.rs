//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! Every structure this crate passes, as the bytes C lays it out in.

#![allow(
    dead_code,
    unused_imports,
    reason = "each structure's both forms are written; a method may use one"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_component_object_model_direct as component_object_model_direct;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

use core::mem::offset_of;

macro_rules! primitive_bytes {
    ($($get:ident $put:ident $type:ty;)*) => {$(
        /// Reads the value at this offset, as C lays it out (little-endian).
        pub fn $get(bytes: &[u8], at: usize) -> $type {
            let mut value = [u8::MIN; size_of::<$type>()];
            value.copy_from_slice(&bytes[at..at + size_of::<$type>()]);
            <$type>::from_le_bytes(value)
        }

        /// Writes the value at this offset, as C lays it out (little-endian).
        pub fn $put(bytes: &mut [u8], at: usize, value: $type) {
            bytes[at..at + size_of::<$type>()].copy_from_slice(&value.to_le_bytes());
        }
    )*};
}

primitive_bytes! {
    get_u8 put_u8 u8;
    get_i8 put_i8 i8;
    get_u16 put_u16 u16;
    get_i16 put_i16 i16;
    get_u32 put_u32 u32;
    get_i32 put_i32 i32;
    get_u64 put_u64 u64;
    get_i64 put_i64 i64;
    get_f32 put_f32 f32;
    get_f64 put_f64 f64;
    get_usize put_usize usize;
    get_isize put_isize isize;
}

/// Reads a GUID at this offset.
pub(crate) fn get_guid(
    bytes: &[u8],
    at: usize,
) -> wcontainers_component_object_model::structures::Guid {
    let mut value = [u8::MIN; wcontainers_component_object_model_direct::GUID_SIZE];
    value.copy_from_slice(&bytes[at..at + wcontainers_component_object_model_direct::GUID_SIZE]);
    wcontainers_component_object_model_direct::guid_from(&value)
}

/// Writes a GUID at this offset.
pub(crate) fn put_guid(
    bytes: &mut [u8],
    at: usize,
    value: &wcontainers_component_object_model::structures::Guid,
) {
    let value = wcontainers_component_object_model_direct::guid_bytes(value);
    bytes[at..at + value.len()].copy_from_slice(&value);
}

/// Reads a GUID from its 16 bytes.
pub(crate) fn decode_guid(bytes: &[u8]) -> wcontainers_component_object_model::structures::Guid {
    get_guid(bytes, usize::MIN)
}

/// Writes a GUID as its 16 bytes.
pub(crate) fn encode_guid(
    value: &wcontainers_component_object_model::structures::Guid,
    bytes: &mut [u8],
) {
    put_guid(bytes, usize::MIN, value);
}

/// A GUID as C lays it out: its 16 bytes, aligned as its first field (a 32-bit word).
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GuidLayout(
    [u32; wcontainers_component_object_model_direct::GUID_SIZE / size_of::<u32>()],
);

/// `LUID` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32FoundationLuidLayout {
    low_part: u32,
    high_part: i32,
}

/// Reads `LUID` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_foundation_luid(
    bytes: &[u8],
) -> component_object_model::structures::Luid {
    component_object_model::structures::Luid {
        low_part: get_u32(
            bytes,
            offset_of!(WindowsWin32FoundationLuidLayout, low_part),
        ),
        high_part: get_i32(
            bytes,
            offset_of!(WindowsWin32FoundationLuidLayout, high_part),
        ),
    }
}

/// Writes `LUID` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_foundation_luid(
    value: &component_object_model::structures::Luid,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32FoundationLuidLayout, low_part),
        value.low_part,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationLuidLayout, high_part),
        value.high_part,
    );
}

/// `DXGI_ADAPTER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiAdapterDescLayout {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid: WindowsWin32FoundationLuidLayout,
}

/// Reads `DXGI_ADAPTER_DESC` from the 304 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_adapter_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::AdapterDesc {
    directx_graphics_infrastructure::structures::AdapterDesc {
        description: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, description)
                    + index * size_of::<u16>(),
            )
        }),
        vendor_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, vendor_id),
        ),
        device_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, device_id),
        ),
        sub_sys_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, sub_sys_id),
        ),
        revision: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, revision),
        ),
        dedicated_video_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
                dedicated_video_memory
            ),
        ),
        dedicated_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
                dedicated_system_memory
            ),
        ),
        shared_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
                shared_system_memory
            ),
        ),
        adapter_luid: decode_windows_win32_foundation_luid(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, adapter_luid)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, adapter_luid)
                    + size_of::<WindowsWin32FoundationLuidLayout>()],
        ),
    }
}

/// Writes `DXGI_ADAPTER_DESC` as the 304 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_adapter_desc(
    value: &directx_graphics_infrastructure::structures::AdapterDesc,
    bytes: &mut [u8],
) {
    for (index, item) in value.description.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, description)
                + index * size_of::<u16>(),
            *item,
        );
    }
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, vendor_id),
        value.vendor_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, device_id),
        value.device_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, sub_sys_id),
        value.sub_sys_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, revision),
        value.revision,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
            dedicated_video_memory
        ),
        value.dedicated_video_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
            dedicated_system_memory
        ),
        value.dedicated_system_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDescLayout,
            shared_system_memory
        ),
        value.shared_system_memory,
    );
    encode_windows_win32_foundation_luid(
        &value.adapter_luid,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, adapter_luid)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDescLayout, adapter_luid)
                + size_of::<WindowsWin32FoundationLuidLayout>()],
    );
}

/// `DXGI_ADAPTER_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid: WindowsWin32FoundationLuidLayout,
    flags: u32,
}

/// Reads `DXGI_ADAPTER_DESC1` from the 312 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_adapter_desc1(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::AdapterDesc1 {
    directx_graphics_infrastructure::structures::AdapterDesc1 {
        description: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, description)
                    + index * size_of::<u16>(),
            )
        }),
        vendor_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, vendor_id),
        ),
        device_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, device_id),
        ),
        sub_sys_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, sub_sys_id),
        ),
        revision: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, revision),
        ),
        dedicated_video_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
                dedicated_video_memory
            ),
        ),
        dedicated_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
                dedicated_system_memory
            ),
        ),
        shared_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
                shared_system_memory
            ),
        ),
        adapter_luid: decode_windows_win32_foundation_luid(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, adapter_luid)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, adapter_luid)
                    + size_of::<WindowsWin32FoundationLuidLayout>()],
        ),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, flags),
        ),
    }
}

/// Writes `DXGI_ADAPTER_DESC1` as the 312 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_adapter_desc1(
    value: &directx_graphics_infrastructure::structures::AdapterDesc1,
    bytes: &mut [u8],
) {
    for (index, item) in value.description.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, description)
                + index * size_of::<u16>(),
            *item,
        );
    }
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, vendor_id),
        value.vendor_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, device_id),
        value.device_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, sub_sys_id),
        value.sub_sys_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, revision),
        value.revision,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
            dedicated_video_memory
        ),
        value.dedicated_video_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
            dedicated_system_memory
        ),
        value.dedicated_system_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout,
            shared_system_memory
        ),
        value.shared_system_memory,
    );
    encode_windows_win32_foundation_luid(
        &value.adapter_luid,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, adapter_luid)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, adapter_luid)
                + size_of::<WindowsWin32FoundationLuidLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout, flags),
        value.flags,
    );
}

/// `DXGI_ADAPTER_DESC2` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid: WindowsWin32FoundationLuidLayout,
    flags: u32,
    graphics_preemption_granularity: i32,
    compute_preemption_granularity: i32,
}

/// Reads `DXGI_ADAPTER_DESC2` from the 320 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_adapter_desc2(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::AdapterDesc2 {
    directx_graphics_infrastructure::structures::AdapterDesc2 {
        description: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, description)
                    + index * size_of::<u16>(),
            )
        }),
        vendor_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, vendor_id),
        ),
        device_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, device_id),
        ),
        sub_sys_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, sub_sys_id),
        ),
        revision: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, revision),
        ),
        dedicated_video_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
                dedicated_video_memory
            ),
        ),
        dedicated_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
                dedicated_system_memory
            ),
        ),
        shared_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
                shared_system_memory
            ),
        ),
        adapter_luid: decode_windows_win32_foundation_luid(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, adapter_luid)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, adapter_luid)
                    + size_of::<WindowsWin32FoundationLuidLayout>()],
        ),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, flags),
        ),
        graphics_preemption_granularity:
            directx_graphics_infrastructure::enumerations::GraphicsPreemptionGranularity(get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
                    graphics_preemption_granularity
                ),
            )),
        compute_preemption_granularity:
            directx_graphics_infrastructure::enumerations::ComputePreemptionGranularity(get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
                    compute_preemption_granularity
                ),
            )),
    }
}

/// Writes `DXGI_ADAPTER_DESC2` as the 320 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_adapter_desc2(
    value: &directx_graphics_infrastructure::structures::AdapterDesc2,
    bytes: &mut [u8],
) {
    for (index, item) in value.description.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, description)
                + index * size_of::<u16>(),
            *item,
        );
    }
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, vendor_id),
        value.vendor_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, device_id),
        value.device_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, sub_sys_id),
        value.sub_sys_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, revision),
        value.revision,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
            dedicated_video_memory
        ),
        value.dedicated_video_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
            dedicated_system_memory
        ),
        value.dedicated_system_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
            shared_system_memory
        ),
        value.shared_system_memory,
    );
    encode_windows_win32_foundation_luid(
        &value.adapter_luid,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, adapter_luid)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, adapter_luid)
                + size_of::<WindowsWin32FoundationLuidLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout, flags),
        value.flags,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
            graphics_preemption_granularity
        ),
        value.graphics_preemption_granularity.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout,
            compute_preemption_granularity
        ),
        value.compute_preemption_granularity.0,
    );
}

/// `DXGI_QUERY_VIDEO_MEMORY_INFO` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout {
    budget: u64,
    current_usage: u64,
    available_for_reservation: u64,
    current_reservation: u64,
}

/// Reads `DXGI_QUERY_VIDEO_MEMORY_INFO` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_query_video_memory_info(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::QueryVideoMemoryInfo {
    directx_graphics_infrastructure::structures::QueryVideoMemoryInfo {
        budget: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
                budget
            ),
        ),
        current_usage: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
                current_usage
            ),
        ),
        available_for_reservation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
                available_for_reservation
            ),
        ),
        current_reservation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
                current_reservation
            ),
        ),
    }
}

/// Writes `DXGI_QUERY_VIDEO_MEMORY_INFO` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_query_video_memory_info(
    value: &directx_graphics_infrastructure::structures::QueryVideoMemoryInfo,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
            budget
        ),
        value.budget,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
            current_usage
        ),
        value.current_usage,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
            available_for_reservation
        ),
        value.available_for_reservation,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout,
            current_reservation
        ),
        value.current_reservation,
    );
}

/// `DXGI_ADAPTER_DESC3` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid: WindowsWin32FoundationLuidLayout,
    flags: i32,
    graphics_preemption_granularity: i32,
    compute_preemption_granularity: i32,
}

/// Reads `DXGI_ADAPTER_DESC3` from the 320 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_adapter_desc3(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::AdapterDesc3 {
    directx_graphics_infrastructure::structures::AdapterDesc3 {
        description: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, description)
                    + index * size_of::<u16>(),
            )
        }),
        vendor_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, vendor_id),
        ),
        device_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, device_id),
        ),
        sub_sys_id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, sub_sys_id),
        ),
        revision: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, revision),
        ),
        dedicated_video_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
                dedicated_video_memory
            ),
        ),
        dedicated_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
                dedicated_system_memory
            ),
        ),
        shared_system_memory: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
                shared_system_memory
            ),
        ),
        adapter_luid: decode_windows_win32_foundation_luid(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, adapter_luid)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, adapter_luid)
                    + size_of::<WindowsWin32FoundationLuidLayout>()],
        ),
        flags: directx_graphics_infrastructure::enumerations::AdapterFlag3(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, flags),
        )),
        graphics_preemption_granularity:
            directx_graphics_infrastructure::enumerations::GraphicsPreemptionGranularity(get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
                    graphics_preemption_granularity
                ),
            )),
        compute_preemption_granularity:
            directx_graphics_infrastructure::enumerations::ComputePreemptionGranularity(get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
                    compute_preemption_granularity
                ),
            )),
    }
}

/// Writes `DXGI_ADAPTER_DESC3` as the 320 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_adapter_desc3(
    value: &directx_graphics_infrastructure::structures::AdapterDesc3,
    bytes: &mut [u8],
) {
    for (index, item) in value.description.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, description)
                + index * size_of::<u16>(),
            *item,
        );
    }
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, vendor_id),
        value.vendor_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, device_id),
        value.device_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, sub_sys_id),
        value.sub_sys_id,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, revision),
        value.revision,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
            dedicated_video_memory
        ),
        value.dedicated_video_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
            dedicated_system_memory
        ),
        value.dedicated_system_memory,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
            shared_system_memory
        ),
        value.shared_system_memory,
    );
    encode_windows_win32_foundation_luid(
        &value.adapter_luid,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, adapter_luid)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, adapter_luid)
                + size_of::<WindowsWin32FoundationLuidLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout, flags),
        value.flags.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
            graphics_preemption_granularity
        ),
        value.graphics_preemption_granularity.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout,
            compute_preemption_granularity
        ),
        value.compute_preemption_granularity.0,
    );
}

/// `RECT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32FoundationRectLayout {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// Reads `RECT` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_foundation_rect(
    bytes: &[u8],
) -> component_object_model::structures::Rect {
    component_object_model::structures::Rect {
        left: get_i32(bytes, offset_of!(WindowsWin32FoundationRectLayout, left)),
        top: get_i32(bytes, offset_of!(WindowsWin32FoundationRectLayout, top)),
        right: get_i32(bytes, offset_of!(WindowsWin32FoundationRectLayout, right)),
        bottom: get_i32(bytes, offset_of!(WindowsWin32FoundationRectLayout, bottom)),
    }
}

/// Writes `RECT` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_foundation_rect(
    value: &component_object_model::structures::Rect,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationRectLayout, left),
        value.left,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationRectLayout, top),
        value.top,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationRectLayout, right),
        value.right,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationRectLayout, bottom),
        value.bottom,
    );
}

/// `DXGI_SAMPLE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout {
    count: u32,
    quality: u32,
}

/// Reads `DXGI_SAMPLE_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SampleDesc {
    directx_graphics_infrastructure::structures::SampleDesc {
        count: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout, count),
        ),
        quality: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout, quality),
        ),
    }
}

/// Writes `DXGI_SAMPLE_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
    value: &directx_graphics_infrastructure::structures::SampleDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout, count),
        value.count,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout, quality),
        value.quality,
    );
}

/// `DXGI_SURFACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout {
    width: u32,
    height: u32,
    format: i32,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
}

/// Reads `DXGI_SURFACE_DESC` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_surface_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SurfaceDesc {
    directx_graphics_infrastructure::structures::SurfaceDesc {
        width: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, width),
        ),
        height: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, height),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, format),
        )),
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, sample_desc)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, sample_desc)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
    }
}

/// Writes `DXGI_SURFACE_DESC` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_surface_desc(
    value: &directx_graphics_infrastructure::structures::SurfaceDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, width),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, height),
        value.height,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, format),
        value.format.0,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, sample_desc)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout, sample_desc)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
}

/// `DXGI_SHARED_RESOURCE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiSharedResourceLayout {
    handle: usize,
}

/// Reads `DXGI_SHARED_RESOURCE` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_shared_resource(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SharedResource {
    directx_graphics_infrastructure::structures::SharedResource {
        handle: component_object_model::structures::Handle(get_usize(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSharedResourceLayout, handle),
        )),
    }
}

/// Writes `DXGI_SHARED_RESOURCE` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_shared_resource(
    value: &directx_graphics_infrastructure::structures::SharedResource,
    bytes: &mut [u8],
) {
    put_usize(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSharedResourceLayout, handle),
        value.handle.0,
    );
}

/// `DXGI_RATIONAL` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiRationalLayout {
    numerator: u32,
    denominator: u32,
}

/// Reads `DXGI_RATIONAL` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_rational(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::Rational {
    directx_graphics_infrastructure::structures::Rational {
        numerator: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRationalLayout, numerator),
        ),
        denominator: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
                denominator
            ),
        ),
    }
}

/// Writes `DXGI_RATIONAL` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_rational(
    value: &directx_graphics_infrastructure::structures::Rational,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRationalLayout, numerator),
        value.numerator,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
            denominator
        ),
        value.denominator,
    );
}

/// `DXGI_MODE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout {
    width: u32,
    height: u32,
    refresh_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    format: i32,
    scanline_ordering: i32,
    scaling: i32,
}

/// Reads `DXGI_MODE_DESC` from the 28 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::ModeDesc {
    directx_graphics_infrastructure::structures::ModeDesc {
        width: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, width),
        ),
        height: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, height),
        ),
        refresh_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
                refresh_rate
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
                    refresh_rate
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, format),
        )),
        scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
            get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
                    scanline_ordering
                ),
            ),
        ),
        scaling: directx_graphics_infrastructure::enumerations::ModeScaling(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, scaling),
        )),
    }
}

/// Writes `DXGI_MODE_DESC` as the 28 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
    value: &directx_graphics_infrastructure::structures::ModeDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, width),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, height),
        value.height,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.refresh_rate,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
            refresh_rate
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
                refresh_rate
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, format),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
            scanline_ordering
        ),
        value.scanline_ordering.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout, scaling),
        value.scaling.0,
    );
}

/// `DXGI_SWAP_CHAIN_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout {
    buffer_desc: WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
    buffer_usage: u32,
    buffer_count: u32,
    output_window: usize,
    windowed: i32,
    swap_effect: i32,
    flags: u32,
}

/// Reads `DXGI_SWAP_CHAIN_DESC` from the 72 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SwapChainDesc {
    directx_graphics_infrastructure::structures::SwapChainDesc {
        buffer_desc: decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, buffer_desc)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, buffer_desc)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()],
        ),
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, sample_desc)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, sample_desc)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
        buffer_usage: directx_graphics_infrastructure::enumerations::Usage(get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
                buffer_usage
            ),
        )),
        buffer_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
                buffer_count
            ),
        ),
        output_window: component_object_model::structures::Hwnd(get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
                output_window
            ),
        )),
        windowed: get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, windowed),
        ) != 0,
        swap_effect: directx_graphics_infrastructure::enumerations::SwapEffect(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, swap_effect),
        )),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, flags),
        ),
    }
}

/// Writes `DXGI_SWAP_CHAIN_DESC` as the 72 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(
    value: &directx_graphics_infrastructure::structures::SwapChainDesc,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
        &value.buffer_desc,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, buffer_desc)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, buffer_desc)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()],
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, sample_desc)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, sample_desc)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
            buffer_usage
        ),
        value.buffer_usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
            buffer_count
        ),
        value.buffer_count,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout,
            output_window
        ),
        value.output_window.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, windowed),
        i32::from(value.windowed),
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, swap_effect),
        value.swap_effect.0,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout, flags),
        value.flags,
    );
}

/// `DXGI_SWAP_CHAIN_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout {
    width: u32,
    height: u32,
    format: i32,
    stereo: i32,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
    buffer_usage: u32,
    buffer_count: u32,
    scaling: i32,
    swap_effect: i32,
    alpha_mode: i32,
    flags: u32,
}

/// Reads `DXGI_SWAP_CHAIN_DESC1` from the 48 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SwapChainDesc1 {
    directx_graphics_infrastructure::structures::SwapChainDesc1 {
        width: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, width),
        ),
        height: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, height),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, format),
        )),
        stereo: get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, stereo),
        ) != 0,
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                sample_desc
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                    sample_desc
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
        buffer_usage: directx_graphics_infrastructure::enumerations::Usage(get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                buffer_usage
            ),
        )),
        buffer_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                buffer_count
            ),
        ),
        scaling: directx_graphics_infrastructure::enumerations::Scaling(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, scaling),
        )),
        swap_effect: directx_graphics_infrastructure::enumerations::SwapEffect(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                swap_effect
            ),
        )),
        alpha_mode: directx_graphics_infrastructure::enumerations::AlphaMode(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, alpha_mode),
        )),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, flags),
        ),
    }
}

/// Writes `DXGI_SWAP_CHAIN_DESC1` as the 48 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(
    value: &directx_graphics_infrastructure::structures::SwapChainDesc1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, width),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, height),
        value.height,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, format),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, stereo),
        i32::from(value.stereo),
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
            sample_desc
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
                sample_desc
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
            buffer_usage
        ),
        value.buffer_usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
            buffer_count
        ),
        value.buffer_count,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, scaling),
        value.scaling.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout,
            swap_effect
        ),
        value.swap_effect.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, alpha_mode),
        value.alpha_mode.0,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout, flags),
        value.flags,
    );
}

/// `DXGI_SWAP_CHAIN_FULLSCREEN_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout {
    refresh_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    scanline_ordering: i32,
    scaling: i32,
    windowed: i32,
}

/// Reads `DXGI_SWAP_CHAIN_FULLSCREEN_DESC` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::SwapChainFullscreenDesc {
    directx_graphics_infrastructure::structures::SwapChainFullscreenDesc {
        refresh_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                refresh_rate
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                    refresh_rate
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
            get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                    scanline_ordering
                ),
            ),
        ),
        scaling: directx_graphics_infrastructure::enumerations::ModeScaling(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                scaling
            ),
        )),
        windowed: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                windowed
            ),
        ) != 0,
    }
}

/// Writes `DXGI_SWAP_CHAIN_FULLSCREEN_DESC` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(
    value: &directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.refresh_rate,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
            refresh_rate
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
                refresh_rate
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
            scanline_ordering
        ),
        value.scanline_ordering.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
            scaling
        ),
        value.scaling.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout,
            windowed
        ),
        i32::from(value.windowed),
    );
}

/// `DXGI_DECODE_SWAP_CHAIN_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiDecodeSwapChainDescLayout {
    flags: u32,
}

/// Reads `DXGI_DECODE_SWAP_CHAIN_DESC` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_decode_swap_chain_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::DecodeSwapChainDesc {
    directx_graphics_infrastructure::structures::DecodeSwapChainDesc {
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiDecodeSwapChainDescLayout, flags),
        ),
    }
}

/// Writes `DXGI_DECODE_SWAP_CHAIN_DESC` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_decode_swap_chain_desc(
    value: &directx_graphics_infrastructure::structures::DecodeSwapChainDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiDecodeSwapChainDescLayout, flags),
        value.flags,
    );
}

/// `DXGI_OUTPUT_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutputDescLayout {
    device_name: [u16; 32],
    desktop_coordinates: WindowsWin32FoundationRectLayout,
    attached_to_desktop: i32,
    rotation: i32,
    monitor: usize,
}

/// Reads `DXGI_OUTPUT_DESC` from the 96 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_output_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutputDesc {
    directx_graphics_infrastructure::structures::OutputDesc {
        device_name: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, device_name)
                    + index * size_of::<u16>(),
            )
        }),
        desktop_coordinates: decode_windows_win32_foundation_rect(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
                desktop_coordinates
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
                    desktop_coordinates
                ) + size_of::<WindowsWin32FoundationRectLayout>()],
        ),
        attached_to_desktop: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
                attached_to_desktop
            ),
        ) != 0,
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, rotation),
        )),
        monitor: component_object_model::structures::Hmonitor(get_usize(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, monitor),
        )),
    }
}

/// Writes `DXGI_OUTPUT_DESC` as the 96 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_output_desc(
    value: &directx_graphics_infrastructure::structures::OutputDesc,
    bytes: &mut [u8],
) {
    for (index, item) in value.device_name.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, device_name)
                + index * size_of::<u16>(),
            *item,
        );
    }
    encode_windows_win32_foundation_rect(
        &value.desktop_coordinates,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
            desktop_coordinates
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
                desktop_coordinates
            ) + size_of::<WindowsWin32FoundationRectLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDescLayout,
            attached_to_desktop
        ),
        i32::from(value.attached_to_desktop),
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, rotation),
        value.rotation.0,
    );
    put_usize(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDescLayout, monitor),
        value.monitor.0,
    );
}

/// `DXGI_GAMMA_CONTROL_CAPABILITIES` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout {
    scale_and_offset_supported: i32,
    max_converted_value: f32,
    min_converted_value: f32,
    num_gamma_control_points: u32,
    control_point_positions: [f32; 1025],
}

/// Reads `DXGI_GAMMA_CONTROL_CAPABILITIES` from the 4116 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control_capabilities(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::GammaControlCapabilities {
    directx_graphics_infrastructure::structures::GammaControlCapabilities {
        scale_and_offset_supported: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                scale_and_offset_supported
            ),
        ) != 0,
        max_converted_value: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                max_converted_value
            ),
        ),
        min_converted_value: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                min_converted_value
            ),
        ),
        num_gamma_control_points: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                num_gamma_control_points
            ),
        ),
        control_point_positions: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                    control_point_positions
                ) + index * size_of::<f32>(),
            )
        }),
    }
}

/// Writes `DXGI_GAMMA_CONTROL_CAPABILITIES` as the 4116 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_gamma_control_capabilities(
    value: &directx_graphics_infrastructure::structures::GammaControlCapabilities,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
            scale_and_offset_supported
        ),
        i32::from(value.scale_and_offset_supported),
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
            max_converted_value
        ),
        value.max_converted_value,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
            min_converted_value
        ),
        value.min_converted_value,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
            num_gamma_control_points
        ),
        value.num_gamma_control_points,
    );
    for (index, item) in value.control_point_positions.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout,
                control_point_positions
            ) + index * size_of::<f32>(),
            *item,
        );
    }
}

/// `DXGI_RGB` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiRgbLayout {
    red: f32,
    green: f32,
    blue: f32,
}

/// Reads `DXGI_RGB` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_rgb(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::Rgb {
    directx_graphics_infrastructure::structures::Rgb {
        red: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, red),
        ),
        green: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, green),
        ),
        blue: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, blue),
        ),
    }
}

/// Writes `DXGI_RGB` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_rgb(
    value: &directx_graphics_infrastructure::structures::Rgb,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, red),
        value.red,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, green),
        value.green,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiCommonDxgiRgbLayout, blue),
        value.blue,
    );
}

/// `DXGI_GAMMA_CONTROL` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout {
    scale: WindowsWin32GraphicsDxgiCommonDxgiRgbLayout,
    offset: WindowsWin32GraphicsDxgiCommonDxgiRgbLayout,
    gamma_curve: [WindowsWin32GraphicsDxgiCommonDxgiRgbLayout; 1025],
}

/// Reads `DXGI_GAMMA_CONTROL` from the 12324 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::GammaControl {
    directx_graphics_infrastructure::structures::GammaControl {
        scale: decode_windows_win32_graphics_dxgi_common_dxgi_rgb(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, scale)
                ..offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, scale)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
        ),
        offset: decode_windows_win32_graphics_dxgi_common_dxgi_rgb(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, offset)
                ..offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, offset)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
        ),
        gamma_curve: core::array::from_fn(|index| {
            decode_windows_win32_graphics_dxgi_common_dxgi_rgb(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout,
                    gamma_curve
                ) + index * size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()
                    ..offset_of!(
                        WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout,
                        gamma_curve
                    ) + index * size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()
                        + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
            )
        }),
    }
}

/// Writes `DXGI_GAMMA_CONTROL` as the 12324 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(
    value: &directx_graphics_infrastructure::structures::GammaControl,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_dxgi_common_dxgi_rgb(
        &value.scale,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, scale)
            ..offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, scale)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_rgb(
        &value.offset,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, offset)
            ..offset_of!(WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout, offset)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
    );
    for (index, item) in value.gamma_curve.iter().enumerate() {
        encode_windows_win32_graphics_dxgi_common_dxgi_rgb(
            item,
            &mut bytes[offset_of!(
                WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout,
                gamma_curve
            ) + index * size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()
                ..offset_of!(
                    WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout,
                    gamma_curve
                ) + index * size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRgbLayout>()],
        );
    }
}

/// `DXGI_FRAME_STATISTICS` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout {
    present_count: u32,
    present_refresh_count: u32,
    sync_refresh_count: u32,
    sync_qpc_time: i64,
    sync_gpu_time: i64,
}

/// Reads `DXGI_FRAME_STATISTICS` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_frame_statistics(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::FrameStatistics {
    directx_graphics_infrastructure::structures::FrameStatistics {
        present_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
                present_count
            ),
        ),
        present_refresh_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
                present_refresh_count
            ),
        ),
        sync_refresh_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
                sync_refresh_count
            ),
        ),
        sync_qpc_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
                sync_qpc_time
            ),
        ),
        sync_gpu_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
                sync_gpu_time
            ),
        ),
    }
}

/// Writes `DXGI_FRAME_STATISTICS` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_frame_statistics(
    value: &directx_graphics_infrastructure::structures::FrameStatistics,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
            present_count
        ),
        value.present_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
            present_refresh_count
        ),
        value.present_refresh_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
            sync_refresh_count
        ),
        value.sync_refresh_count,
    );
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
            sync_qpc_time
        ),
        value.sync_qpc_time,
    );
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout,
            sync_gpu_time
        ),
        value.sync_gpu_time,
    );
}

/// `DXGI_MODE_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiModeDesc1Layout {
    width: u32,
    height: u32,
    refresh_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    format: i32,
    scanline_ordering: i32,
    scaling: i32,
    stereo: i32,
}

/// Reads `DXGI_MODE_DESC1` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_mode_desc1(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::ModeDesc1 {
    directx_graphics_infrastructure::structures::ModeDesc1 {
        width: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, width),
        ),
        height: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, height),
        ),
        refresh_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, refresh_rate)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, refresh_rate)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, format),
        )),
        scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
            get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiModeDesc1Layout,
                    scanline_ordering
                ),
            ),
        ),
        scaling: directx_graphics_infrastructure::enumerations::ModeScaling(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, scaling),
        )),
        stereo: get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, stereo),
        ) != 0,
    }
}

/// Writes `DXGI_MODE_DESC1` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_mode_desc1(
    value: &directx_graphics_infrastructure::structures::ModeDesc1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, width),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, height),
        value.height,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.refresh_rate,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, refresh_rate)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, refresh_rate)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, format),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiModeDesc1Layout,
            scanline_ordering
        ),
        value.scanline_ordering.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, scaling),
        value.scaling.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiModeDesc1Layout, stereo),
        i32::from(value.stereo),
    );
}

/// `DXGI_OUTPUT_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout {
    device_name: [u16; 32],
    desktop_coordinates: WindowsWin32FoundationRectLayout,
    attached_to_desktop: i32,
    rotation: i32,
    monitor: usize,
    bits_per_color: u32,
    color_space: i32,
    red_primary: [f32; 2],
    green_primary: [f32; 2],
    blue_primary: [f32; 2],
    white_point: [f32; 2],
    min_luminance: f32,
    max_luminance: f32,
    max_full_frame_luminance: f32,
}

/// Reads `DXGI_OUTPUT_DESC1` from the 152 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_output_desc1(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutputDesc1 {
    directx_graphics_infrastructure::structures::OutputDesc1 {
        device_name: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, device_name)
                    + index * size_of::<u16>(),
            )
        }),
        desktop_coordinates: decode_windows_win32_foundation_rect(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                desktop_coordinates
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                    desktop_coordinates
                ) + size_of::<WindowsWin32FoundationRectLayout>()],
        ),
        attached_to_desktop: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                attached_to_desktop
            ),
        ) != 0,
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, rotation),
        )),
        monitor: component_object_model::structures::Hmonitor(get_usize(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, monitor),
        )),
        bits_per_color: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                bits_per_color
            ),
        ),
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, color_space),
        )),
        red_primary: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, red_primary)
                    + index * size_of::<f32>(),
            )
        }),
        green_primary: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, green_primary)
                    + index * size_of::<f32>(),
            )
        }),
        blue_primary: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, blue_primary)
                    + index * size_of::<f32>(),
            )
        }),
        white_point: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, white_point)
                    + index * size_of::<f32>(),
            )
        }),
        min_luminance: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, min_luminance),
        ),
        max_luminance: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, max_luminance),
        ),
        max_full_frame_luminance: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                max_full_frame_luminance
            ),
        ),
    }
}

/// Writes `DXGI_OUTPUT_DESC1` as the 152 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_output_desc1(
    value: &directx_graphics_infrastructure::structures::OutputDesc1,
    bytes: &mut [u8],
) {
    for (index, item) in value.device_name.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, device_name)
                + index * size_of::<u16>(),
            *item,
        );
    }
    encode_windows_win32_foundation_rect(
        &value.desktop_coordinates,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
            desktop_coordinates
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
                desktop_coordinates
            ) + size_of::<WindowsWin32FoundationRectLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
            attached_to_desktop
        ),
        i32::from(value.attached_to_desktop),
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, rotation),
        value.rotation.0,
    );
    put_usize(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, monitor),
        value.monitor.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
            bits_per_color
        ),
        value.bits_per_color,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, color_space),
        value.color_space.0,
    );
    for (index, item) in value.red_primary.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, red_primary)
                + index * size_of::<f32>(),
            *item,
        );
    }
    for (index, item) in value.green_primary.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, green_primary)
                + index * size_of::<f32>(),
            *item,
        );
    }
    for (index, item) in value.blue_primary.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, blue_primary)
                + index * size_of::<f32>(),
            *item,
        );
    }
    for (index, item) in value.white_point.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, white_point)
                + index * size_of::<f32>(),
            *item,
        );
    }
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, min_luminance),
        value.min_luminance,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout, max_luminance),
        value.max_luminance,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout,
            max_full_frame_luminance
        ),
        value.max_full_frame_luminance,
    );
}

/// `DXGI_OUTDUPL_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutduplDescLayout {
    mode_desc: WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout,
    rotation: i32,
    desktop_image_in_system_memory: i32,
}

/// Reads `DXGI_OUTDUPL_DESC` from the 36 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_outdupl_desc(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutduplDesc {
    directx_graphics_infrastructure::structures::OutduplDesc {
        mode_desc: decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
            &bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, mode_desc)
                ..offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, mode_desc)
                    + size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()],
        ),
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, rotation),
        )),
        desktop_image_in_system_memory: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplDescLayout,
                desktop_image_in_system_memory
            ),
        ) != 0,
    }
}

/// Writes `DXGI_OUTDUPL_DESC` as the 36 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_outdupl_desc(
    value: &directx_graphics_infrastructure::structures::OutduplDesc,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
        &value.mode_desc,
        &mut bytes[offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, mode_desc)
            ..offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, mode_desc)
                + size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiOutduplDescLayout, rotation),
        value.rotation.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplDescLayout,
            desktop_image_in_system_memory
        ),
        i32::from(value.desktop_image_in_system_memory),
    );
}

/// `POINT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32FoundationPointLayout {
    x: i32,
    y: i32,
}

/// Reads `POINT` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_foundation_point(
    bytes: &[u8],
) -> component_object_model::structures::Point {
    component_object_model::structures::Point {
        x: get_i32(bytes, offset_of!(WindowsWin32FoundationPointLayout, x)),
        y: get_i32(bytes, offset_of!(WindowsWin32FoundationPointLayout, y)),
    }
}

/// Writes `POINT` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_foundation_point(
    value: &component_object_model::structures::Point,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationPointLayout, x),
        value.x,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationPointLayout, y),
        value.y,
    );
}

/// `DXGI_OUTDUPL_POINTER_POSITION` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout {
    position: WindowsWin32FoundationPointLayout,
    visible: i32,
}

/// Reads `DXGI_OUTDUPL_POINTER_POSITION` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_position(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutduplPointerPosition {
    directx_graphics_infrastructure::structures::OutduplPointerPosition {
        position: decode_windows_win32_foundation_point(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
                position
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
                    position
                ) + size_of::<WindowsWin32FoundationPointLayout>()],
        ),
        visible: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
                visible
            ),
        ) != 0,
    }
}

/// Writes `DXGI_OUTDUPL_POINTER_POSITION` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_position(
    value: &directx_graphics_infrastructure::structures::OutduplPointerPosition,
    bytes: &mut [u8],
) {
    encode_windows_win32_foundation_point(
        &value.position,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
            position
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
                position
            ) + size_of::<WindowsWin32FoundationPointLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
            visible
        ),
        i32::from(value.visible),
    );
}

/// `DXGI_OUTDUPL_FRAME_INFO` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout {
    last_present_time: i64,
    last_mouse_update_time: i64,
    accumulated_frames: u32,
    rects_coalesced: i32,
    protected_content_masked_out: i32,
    pointer_position: WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout,
    total_metadata_buffer_size: u32,
    pointer_shape_buffer_size: u32,
}

/// Reads `DXGI_OUTDUPL_FRAME_INFO` from the 48 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_outdupl_frame_info(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutduplFrameInfo {
    directx_graphics_infrastructure::structures::OutduplFrameInfo {
        last_present_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                last_present_time
            ),
        ),
        last_mouse_update_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                last_mouse_update_time
            ),
        ),
        accumulated_frames: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                accumulated_frames
            ),
        ),
        rects_coalesced: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                rects_coalesced
            ),
        ) != 0,
        protected_content_masked_out: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                protected_content_masked_out
            ),
        ) != 0,
        pointer_position: decode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_position(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                pointer_position
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                    pointer_position
                ) + size_of::<WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout>()],
        ),
        total_metadata_buffer_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                total_metadata_buffer_size
            ),
        ),
        pointer_shape_buffer_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                pointer_shape_buffer_size
            ),
        ),
    }
}

/// Writes `DXGI_OUTDUPL_FRAME_INFO` as the 48 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_outdupl_frame_info(
    value: &directx_graphics_infrastructure::structures::OutduplFrameInfo,
    bytes: &mut [u8],
) {
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            last_present_time
        ),
        value.last_present_time,
    );
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            last_mouse_update_time
        ),
        value.last_mouse_update_time,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            accumulated_frames
        ),
        value.accumulated_frames,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            rects_coalesced
        ),
        i32::from(value.rects_coalesced),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            protected_content_masked_out
        ),
        i32::from(value.protected_content_masked_out),
    );
    encode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_position(
        &value.pointer_position,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            pointer_position
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
                pointer_position
            ) + size_of::<WindowsWin32GraphicsDxgiDxgiOutduplPointerPositionLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            total_metadata_buffer_size
        ),
        value.total_metadata_buffer_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout,
            pointer_shape_buffer_size
        ),
        value.pointer_shape_buffer_size,
    );
}

/// `DXGI_OUTDUPL_MOVE_RECT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout {
    source_point: WindowsWin32FoundationPointLayout,
    destination_rect: WindowsWin32FoundationRectLayout,
}

/// Reads `DXGI_OUTDUPL_MOVE_RECT` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_outdupl_move_rect(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutduplMoveRect {
    directx_graphics_infrastructure::structures::OutduplMoveRect {
        source_point: decode_windows_win32_foundation_point(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                source_point
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                    source_point
                ) + size_of::<WindowsWin32FoundationPointLayout>()],
        ),
        destination_rect: decode_windows_win32_foundation_rect(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                destination_rect
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                    destination_rect
                ) + size_of::<WindowsWin32FoundationRectLayout>()],
        ),
    }
}

/// Writes `DXGI_OUTDUPL_MOVE_RECT` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_outdupl_move_rect(
    value: &directx_graphics_infrastructure::structures::OutduplMoveRect,
    bytes: &mut [u8],
) {
    encode_windows_win32_foundation_point(
        &value.source_point,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
            source_point
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                source_point
            ) + size_of::<WindowsWin32FoundationPointLayout>()],
    );
    encode_windows_win32_foundation_rect(
        &value.destination_rect,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
            destination_rect
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplMoveRectLayout,
                destination_rect
            ) + size_of::<WindowsWin32FoundationRectLayout>()],
    );
}

/// `DXGI_OUTDUPL_POINTER_SHAPE_INFO` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout {
    type_: u32,
    width: u32,
    height: u32,
    pitch: u32,
    hot_spot: WindowsWin32FoundationPointLayout,
}

/// Reads `DXGI_OUTDUPL_POINTER_SHAPE_INFO` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_shape_info(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::OutduplPointerShapeInfo {
    directx_graphics_infrastructure::structures::OutduplPointerShapeInfo {
        type_: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                type_
            ),
        ),
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                height
            ),
        ),
        pitch: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                pitch
            ),
        ),
        hot_spot: decode_windows_win32_foundation_point(
            &bytes[offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                hot_spot
            )
                ..offset_of!(
                    WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                    hot_spot
                ) + size_of::<WindowsWin32FoundationPointLayout>()],
        ),
    }
}

/// Writes `DXGI_OUTDUPL_POINTER_SHAPE_INFO` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_shape_info(
    value: &directx_graphics_infrastructure::structures::OutduplPointerShapeInfo,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
            type_
        ),
        value.type_,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
            pitch
        ),
        value.pitch,
    );
    encode_windows_win32_foundation_point(
        &value.hot_spot,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
            hot_spot
        )
            ..offset_of!(
                WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout,
                hot_spot
            ) + size_of::<WindowsWin32FoundationPointLayout>()],
    );
}

/// `DXGI_RGBA` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiRgbaLayout {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

/// Reads `DXGI_RGBA` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_rgba(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::Rgba {
    directx_graphics_infrastructure::structures::Rgba {
        r: get_f32(bytes, offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, r)),
        g: get_f32(bytes, offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, g)),
        b: get_f32(bytes, offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, b)),
        a: get_f32(bytes, offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, a)),
    }
}

/// Writes `DXGI_RGBA` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_rgba(
    value: &directx_graphics_infrastructure::structures::Rgba,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, r),
        value.r,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, g),
        value.g,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, b),
        value.b,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiRgbaLayout, a),
        value.a,
    );
}

/// `DXGI_MATRIX_3X2_F` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout {
    _11: f32,
    _12: f32,
    _21: f32,
    _22: f32,
    _31: f32,
    _32: f32,
}

/// Reads `DXGI_MATRIX_3X2_F` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::Matrix3x2F {
    directx_graphics_infrastructure::structures::Matrix3x2F {
        _11: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _11),
        ),
        _12: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _12),
        ),
        _21: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _21),
        ),
        _22: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _22),
        ),
        _31: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _31),
        ),
        _32: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _32),
        ),
    }
}

/// Writes `DXGI_MATRIX_3X2_F` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(
    value: &directx_graphics_infrastructure::structures::Matrix3x2F,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _11),
        value._11,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _12),
        value._12,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _21),
        value._21,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _22),
        value._22,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _31),
        value._31,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout, _32),
        value._32,
    );
}

/// `DXGI_FRAME_STATISTICS_MEDIA` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout {
    present_count: u32,
    present_refresh_count: u32,
    sync_refresh_count: u32,
    sync_qpc_time: i64,
    sync_gpu_time: i64,
    composition_mode: i32,
    approved_present_duration: u32,
}

/// Reads `DXGI_FRAME_STATISTICS_MEDIA` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_dxgi_dxgi_frame_statistics_media(
    bytes: &[u8],
) -> directx_graphics_infrastructure::structures::FrameStatisticsMedia {
    directx_graphics_infrastructure::structures::FrameStatisticsMedia {
        present_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                present_count
            ),
        ),
        present_refresh_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                present_refresh_count
            ),
        ),
        sync_refresh_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                sync_refresh_count
            ),
        ),
        sync_qpc_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                sync_qpc_time
            ),
        ),
        sync_gpu_time: get_i64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                sync_gpu_time
            ),
        ),
        composition_mode: directx_graphics_infrastructure::enumerations::FramePresentationMode(
            get_i32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                    composition_mode
                ),
            ),
        ),
        approved_present_duration: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
                approved_present_duration
            ),
        ),
    }
}

/// Writes `DXGI_FRAME_STATISTICS_MEDIA` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_dxgi_dxgi_frame_statistics_media(
    value: &directx_graphics_infrastructure::structures::FrameStatisticsMedia,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            present_count
        ),
        value.present_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            present_refresh_count
        ),
        value.present_refresh_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            sync_refresh_count
        ),
        value.sync_refresh_count,
    );
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            sync_qpc_time
        ),
        value.sync_qpc_time,
    );
    put_i64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            sync_gpu_time
        ),
        value.sync_gpu_time,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            composition_mode
        ),
        value.composition_mode.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout,
            approved_present_duration
        ),
        value.approved_present_duration,
    );
}
