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
use wcontainers_direct3d_12 as direct3d_12;
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

/// `D3D12_COMMAND_QUEUE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout {
    type_: i32,
    priority: i32,
    flags: i32,
    node_mask: u32,
}

/// Reads `D3D12_COMMAND_QUEUE_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(
    bytes: &[u8],
) -> direct3d_12::structures::CommandQueueDesc {
    direct3d_12::structures::CommandQueueDesc {
        type_: direct3d_12::enumerations::CommandListType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
                type_
            ),
        )),
        priority: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
                priority
            ),
        ),
        flags: direct3d_12::enumerations::CommandQueueFlags(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
                flags
            ),
        )),
        node_mask: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
                node_mask
            ),
        ),
    }
}

/// Writes `D3D12_COMMAND_QUEUE_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(
    value: &direct3d_12::structures::CommandQueueDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
            type_
        ),
        value.type_.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
            priority
        ),
        value.priority,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
            flags
        ),
        value.flags.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout,
            node_mask
        ),
        value.node_mask,
    );
}

/// `D3D12_DESCRIPTOR_HEAP_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout {
    type_: i32,
    num_descriptors: u32,
    flags: i32,
    node_mask: u32,
}

/// Reads `D3D12_DESCRIPTOR_HEAP_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
    bytes: &[u8],
) -> direct3d_12::structures::DescriptorHeapDesc {
    direct3d_12::structures::DescriptorHeapDesc {
        type_: direct3d_12::enumerations::DescriptorHeapType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
                type_
            ),
        )),
        num_descriptors: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
                num_descriptors
            ),
        ),
        flags: direct3d_12::enumerations::DescriptorHeapFlags(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
                flags
            ),
        )),
        node_mask: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
                node_mask
            ),
        ),
    }
}

/// Writes `D3D12_DESCRIPTOR_HEAP_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
    value: &direct3d_12::structures::DescriptorHeapDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
            type_
        ),
        value.type_.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
            num_descriptors
        ),
        value.num_descriptors,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
            flags
        ),
        value.flags.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout,
            node_mask
        ),
        value.node_mask,
    );
}

/// `D3D12_BUFFER_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout {
    first_element: u64,
    num_elements: u32,
    structure_byte_stride: u32,
    flags: i32,
}

/// Reads `D3D12_BUFFER_SRV` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_buffer_srv(
    bytes: &[u8],
) -> direct3d_12::structures::BufferSrv {
    direct3d_12::structures::BufferSrv {
        first_element: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
                first_element
            ),
        ),
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
                num_elements
            ),
        ),
        structure_byte_stride: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
                structure_byte_stride
            ),
        ),
        flags: direct3d_12::enumerations::BufferSrvFlags(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout, flags),
        )),
    }
}

/// Writes `D3D12_BUFFER_SRV` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_buffer_srv(
    value: &direct3d_12::structures::BufferSrv,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
            first_element
        ),
        value.first_element,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
            num_elements
        ),
        value.num_elements,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
            structure_byte_stride
        ),
        value.structure_byte_stride,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout, flags),
        value.flags.0,
    );
}

/// `D3D12_TEX1D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEX1D_SRV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex1DSrv {
    direct3d_12::structures::Tex1DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
                mip_levels
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEX1D_SRV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_srv(
    value: &direct3d_12::structures::Tex1DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEX1D_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_array_slice: u32,
    array_size: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEX1D_ARRAY_SRV` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex1DArraySrv {
    direct3d_12::structures::Tex1DArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
                mip_levels
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
                array_size
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEX1D_ARRAY_SRV` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_srv(
    value: &direct3d_12::structures::Tex1DArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEX2D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    plane_slice: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEX2D_SRV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DSrv {
    direct3d_12::structures::Tex2DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
                mip_levels
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
                plane_slice
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEX2D_SRV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_srv(
    value: &direct3d_12::structures::Tex2DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
            plane_slice
        ),
        value.plane_slice,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEX2D_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_array_slice: u32,
    array_size: u32,
    plane_slice: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEX2D_ARRAY_SRV` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DArraySrv {
    direct3d_12::structures::Tex2DArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                mip_levels
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                array_size
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                plane_slice
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEX2D_ARRAY_SRV` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_srv(
    value: &direct3d_12::structures::Tex2DArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            plane_slice
        ),
        value.plane_slice,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEX2DMS_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout {
    unused_field_nothing_to_define: u32,
}

/// Reads `D3D12_TEX2DMS_SRV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DmsSrv {
    direct3d_12::structures::Tex2DmsSrv {
        unused_field_nothing_to_define: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout,
                unused_field_nothing_to_define
            ),
        ),
    }
}

/// Writes `D3D12_TEX2DMS_SRV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_srv(
    value: &direct3d_12::structures::Tex2DmsSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout,
            unused_field_nothing_to_define
        ),
        value.unused_field_nothing_to_define,
    );
}

/// `D3D12_TEX2DMS_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout {
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D12_TEX2DMS_ARRAY_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DmsArraySrv {
    direct3d_12::structures::Tex2DmsArraySrv {
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D12_TEX2DMS_ARRAY_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_srv(
    value: &direct3d_12::structures::Tex2DmsArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D12_TEX3D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEX3D_SRV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_srv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex3DSrv {
    direct3d_12::structures::Tex3DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
                mip_levels
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEX3D_SRV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_srv(
    value: &direct3d_12::structures::Tex3DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEXCUBE_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEXCUBE_SRV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_texcube_srv(
    bytes: &[u8],
) -> direct3d_12::structures::TexcubeSrv {
    direct3d_12::structures::TexcubeSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
                mip_levels
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEXCUBE_SRV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_texcube_srv(
    value: &direct3d_12::structures::TexcubeSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_TEXCUBE_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_2d_array_face: u32,
    num_cubes: u32,
    resource_min_lod_clamp: f32,
}

/// Reads `D3D12_TEXCUBE_ARRAY_SRV` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_texcube_array_srv(
    bytes: &[u8],
) -> direct3d_12::structures::TexcubeArraySrv {
    direct3d_12::structures::TexcubeArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
                mip_levels
            ),
        ),
        first_2d_array_face: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
                first_2d_array_face
            ),
        ),
        num_cubes: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
                num_cubes
            ),
        ),
        resource_min_lod_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
                resource_min_lod_clamp
            ),
        ),
    }
}

/// Writes `D3D12_TEXCUBE_ARRAY_SRV` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_texcube_array_srv(
    value: &direct3d_12::structures::TexcubeArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
            first_2d_array_face
        ),
        value.first_2d_array_face,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
            num_cubes
        ),
        value.num_cubes,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
            resource_min_lod_clamp
        ),
        value.resource_min_lod_clamp,
    );
}

/// `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout {
    location: u64,
}

/// Reads `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_raytracing_acceleration_structure_srv(
    bytes: &[u8],
) -> direct3d_12::structures::RaytracingAccelerationStructureSrv {
    direct3d_12::structures::RaytracingAccelerationStructureSrv {
        location: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
                location
            ),
        ),
    }
}

/// Writes `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_raytracing_acceleration_structure_srv(
    value: &direct3d_12::structures::RaytracingAccelerationStructureSrv,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
            location
        ),
        value.location,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout,
    texture_2dms: WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout,
    texture_cube: WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout,
    texture_cube_array: WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout,
    raytracing_acceleration_structure:
        WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
}

/// Reads `_Anonymous_e__Union` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_shader_resource_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_12::structures::ShaderResourceViewDescAnonymous {
    direct3d_12::structures::ShaderResourceViewDescAnonymous {
        buffer: decode_windows_win32_graphics_direct_3d12_d_3d12_buffer_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout>()],
        ),
        texture_cube: decode_windows_win32_graphics_direct_3d12_d_3d12_texcube_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_cube
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_cube
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout>()],
        ),
        texture_cube_array: decode_windows_win32_graphics_direct_3d12_d_3d12_texcube_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                texture_cube_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_cube_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout>()],
        ),
        raytracing_acceleration_structure:
            decode_windows_win32_graphics_direct_3d12_d_3d12_raytracing_acceleration_structure_srv(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    raytracing_acceleration_structure
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        raytracing_acceleration_structure
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
                    >()],
            ),
    }
}

/// Writes `_Anonymous_e__Union` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_shader_resource_view_desc_anonymous(
    value: &direct3d_12::structures::ShaderResourceViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_buffer_srv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_srv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_srv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_srv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_srv(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_srv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_srv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_srv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_texcube_srv(
                &value.texture_cube,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_cube
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_cube
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_texcube_array_srv(
                &value.texture_cube_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    texture_cube_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        texture_cube_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12TexcubeArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_raytracing_acceleration_structure_srv(
                &value.raytracing_acceleration_structure,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                    raytracing_acceleration_structure
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
                        raytracing_acceleration_structure
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
                    >()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout>(
            ) > chosen.0
        {
            chosen = (
                size_of::<
                    WindowsWin32GraphicsDirect3d12D3d12RaytracingAccelerationStructureSrvLayout,
                >(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D12_SHADER_RESOURCE_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout {
    format: i32,
    view_dimension: i32,
    shader4_component_mapping: u32,
    anonymous: WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout,
}

/// Reads `D3D12_SHADER_RESOURCE_VIEW_DESC` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_shader_resource_view_desc(
    bytes: &[u8],
) -> direct3d_12::structures::ShaderResourceViewDesc {
    direct3d_12::structures::ShaderResourceViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_12::enumerations::SrvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                view_dimension
            ),
        )),
        shader4_component_mapping: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                shader4_component_mapping
            ),
        ),
        anonymous: decode_windows_win32_graphics_direct_3d12_shader_resource_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D12_SHADER_RESOURCE_VIEW_DESC` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_shader_resource_view_desc(
    value: &direct3d_12::structures::ShaderResourceViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
            shader4_component_mapping
        ),
        value.shader4_component_mapping,
    );
    encode_windows_win32_graphics_direct_3d12_shader_resource_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ShaderResourceViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d12ShaderResourceViewDescAnonymousLayout>()],
    );
}

/// `D3D12_BUFFER_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout {
    first_element: u64,
    num_elements: u32,
}

/// Reads `D3D12_BUFFER_RTV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_buffer_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::BufferRtv {
    direct3d_12::structures::BufferRtv {
        first_element: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout,
                first_element
            ),
        ),
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout,
                num_elements
            ),
        ),
    }
}

/// Writes `D3D12_BUFFER_RTV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_buffer_rtv(
    value: &direct3d_12::structures::BufferRtv,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout,
            first_element
        ),
        value.first_element,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout,
            num_elements
        ),
        value.num_elements,
    );
}

/// `D3D12_TEX1D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout {
    mip_slice: u32,
}

/// Reads `D3D12_TEX1D_RTV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex1DRtv {
    direct3d_12::structures::Tex1DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout, mip_slice),
        ),
    }
}

/// Writes `D3D12_TEX1D_RTV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_rtv(
    value: &direct3d_12::structures::Tex1DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D12_TEX1D_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D12_TEX1D_ARRAY_RTV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex1DArrayRtv {
    direct3d_12::structures::Tex1DArrayRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D12_TEX1D_ARRAY_RTV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_rtv(
    value: &direct3d_12::structures::Tex1DArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D12_TEX2D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout {
    mip_slice: u32,
    plane_slice: u32,
}

/// Reads `D3D12_TEX2D_RTV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DRtv {
    direct3d_12::structures::Tex2DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout, mip_slice),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D12_TEX2D_RTV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_rtv(
    value: &direct3d_12::structures::Tex2DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout, mip_slice),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `D3D12_TEX2D_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
    plane_slice: u32,
}

/// Reads `D3D12_TEX2D_ARRAY_RTV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DArrayRtv {
    direct3d_12::structures::Tex2DArrayRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
                array_size
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D12_TEX2D_ARRAY_RTV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_rtv(
    value: &direct3d_12::structures::Tex2DArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `D3D12_TEX2DMS_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout {
    unused_field_nothing_to_define: u32,
}

/// Reads `D3D12_TEX2DMS_RTV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DmsRtv {
    direct3d_12::structures::Tex2DmsRtv {
        unused_field_nothing_to_define: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout,
                unused_field_nothing_to_define
            ),
        ),
    }
}

/// Writes `D3D12_TEX2DMS_RTV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_rtv(
    value: &direct3d_12::structures::Tex2DmsRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout,
            unused_field_nothing_to_define
        ),
        value.unused_field_nothing_to_define,
    );
}

/// `D3D12_TEX2DMS_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout {
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D12_TEX2DMS_ARRAY_RTV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex2DmsArrayRtv {
    direct3d_12::structures::Tex2DmsArrayRtv {
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D12_TEX2DMS_ARRAY_RTV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_rtv(
    value: &direct3d_12::structures::Tex2DmsArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D12_TEX3D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout {
    mip_slice: u32,
    first_w_slice: u32,
    w_size: u32,
}

/// Reads `D3D12_TEX3D_RTV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_rtv(
    bytes: &[u8],
) -> direct3d_12::structures::Tex3DRtv {
    direct3d_12::structures::Tex3DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout, mip_slice),
        ),
        first_w_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout,
                first_w_slice
            ),
        ),
        w_size: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout, w_size),
        ),
    }
}

/// Writes `D3D12_TEX3D_RTV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_rtv(
    value: &direct3d_12::structures::Tex3DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout, mip_slice),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout,
            first_w_slice
        ),
        value.first_w_slice,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout, w_size),
        value.w_size,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout,
    texture_2dms: WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_render_target_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_12::structures::RenderTargetViewDescAnonymous {
    direct3d_12::structures::RenderTargetViewDescAnonymous {
        buffer: decode_windows_win32_graphics_direct_3d12_d_3d12_buffer_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_render_target_view_desc_anonymous(
    value: &direct3d_12::structures::RenderTargetViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_buffer_rtv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12BufferRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_rtv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_1d_array_rtv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex1dArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_rtv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2d_array_rtv(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_rtv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_2dms_array_rtv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex2dmsArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_tex_3d_rtv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12Tex3dRtvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D12_RENDER_TARGET_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout,
}

/// Reads `D3D12_RENDER_TARGET_VIEW_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_render_target_view_desc(
    bytes: &[u8],
) -> direct3d_12::structures::RenderTargetViewDesc {
    direct3d_12::structures::RenderTargetViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_12::enumerations::RtvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d12_render_target_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D12_RENDER_TARGET_VIEW_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_render_target_view_desc(
    value: &direct3d_12::structures::RenderTargetViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d12_render_target_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12RenderTargetViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d12RenderTargetViewDescAnonymousLayout>()],
    );
}

/// `D3D12_HEAP_PROPERTIES` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout {
    type_: i32,
    cpu_page_property: i32,
    memory_pool_preference: i32,
    creation_node_mask: u32,
    visible_node_mask: u32,
}

/// Reads `D3D12_HEAP_PROPERTIES` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
    bytes: &[u8],
) -> direct3d_12::structures::HeapProperties {
    direct3d_12::structures::HeapProperties {
        type_: direct3d_12::enumerations::HeapType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
                type_
            ),
        )),
        cpu_page_property: direct3d_12::enumerations::CpuPageProperty(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
                cpu_page_property
            ),
        )),
        memory_pool_preference: direct3d_12::enumerations::MemoryPool(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
                memory_pool_preference
            ),
        )),
        creation_node_mask: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
                creation_node_mask
            ),
        ),
        visible_node_mask: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
                visible_node_mask
            ),
        ),
    }
}

/// Writes `D3D12_HEAP_PROPERTIES` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
    value: &direct3d_12::structures::HeapProperties,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
            type_
        ),
        value.type_.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
            cpu_page_property
        ),
        value.cpu_page_property.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
            memory_pool_preference
        ),
        value.memory_pool_preference.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
            creation_node_mask
        ),
        value.creation_node_mask,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout,
            visible_node_mask
        ),
        value.visible_node_mask,
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

/// `D3D12_RESOURCE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout {
    dimension: i32,
    alignment: u64,
    width: u64,
    height: u32,
    depth_or_array_size: u16,
    mip_levels: u16,
    format: i32,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
    layout: i32,
    flags: i32,
}

/// Reads `D3D12_RESOURCE_DESC` from the 56 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
    bytes: &[u8],
) -> direct3d_12::structures::ResourceDesc {
    direct3d_12::structures::ResourceDesc {
        dimension: direct3d_12::enumerations::ResourceDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                dimension
            ),
        )),
        alignment: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                alignment
            ),
        ),
        width: get_u64(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout, width),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                height
            ),
        ),
        depth_or_array_size: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                depth_or_array_size
            ),
        ),
        mip_levels: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                mip_levels
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                format
            ),
        )),
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                sample_desc
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                    sample_desc
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
        layout: direct3d_12::enumerations::TextureLayout(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                layout
            ),
        )),
        flags: direct3d_12::enumerations::ResourceFlags(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout, flags),
        )),
    }
}

/// Writes `D3D12_RESOURCE_DESC` as the 56 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
    value: &direct3d_12::structures::ResourceDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            dimension
        ),
        value.dimension.0,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            alignment
        ),
        value.alignment,
    );
    put_u64(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout, width),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            height
        ),
        value.height,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            depth_or_array_size
        ),
        value.depth_or_array_size,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            format
        ),
        value.format.0,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            sample_desc
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
                sample_desc
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout,
            layout
        ),
        value.layout.0,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout, flags),
        value.flags.0,
    );
}

/// `D3D12_DEPTH_STENCIL_VALUE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout {
    depth: f32,
    stencil: u8,
}

/// Reads `D3D12_DEPTH_STENCIL_VALUE` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_depth_stencil_value(
    bytes: &[u8],
) -> direct3d_12::structures::DepthStencilValue {
    direct3d_12::structures::DepthStencilValue {
        depth: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout,
                depth
            ),
        ),
        stencil: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout,
                stencil
            ),
        ),
    }
}

/// Writes `D3D12_DEPTH_STENCIL_VALUE` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_depth_stencil_value(
    value: &direct3d_12::structures::DepthStencilValue,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout,
            depth
        ),
        value.depth,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout,
            stencil
        ),
        value.stencil,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout {
    color: [f32; 4],
    depth_stencil: WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_clear_value_anonymous(
    bytes: &[u8],
) -> direct3d_12::structures::ClearValueAnonymous {
    direct3d_12::structures::ClearValueAnonymous {
        color: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                    color
                ) + index * size_of::<f32>(),
            )
        }),
        depth_stencil: decode_windows_win32_graphics_direct_3d12_d_3d12_depth_stencil_value(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                depth_stencil
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                    depth_stencil
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_clear_value_anonymous(
    value: &direct3d_12::structures::ClearValueAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            for (index, item) in value.color.iter().enumerate() {
                put_f32(
                    bytes,
                    offset_of!(
                        WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                        color
                    ) + index * size_of::<f32>(),
                    *item,
                );
            }
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()]
            && size_of::<[f32; 4]>() > chosen.0
        {
            chosen = (size_of::<[f32; 4]>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d12_d_3d12_depth_stencil_value(
                &value.depth_stencil,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                    depth_stencil
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
                        depth_stencil
                    ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout>()],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d12D3d12DepthStencilValueLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D12_CLEAR_VALUE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout {
    format: i32,
    anonymous: WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout,
}

/// Reads `D3D12_CLEAR_VALUE` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_clear_value(
    bytes: &[u8],
) -> direct3d_12::structures::ClearValue {
    direct3d_12::structures::ClearValue {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout, format),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d12_clear_value_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()],
        ),
    }
}

/// Writes `D3D12_CLEAR_VALUE` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_clear_value(
    value: &direct3d_12::structures::ClearValue,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout, format),
        value.format.0,
    );
    encode_windows_win32_graphics_direct_3d12_clear_value_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d12ClearValueAnonymousLayout>()],
    );
}

/// `D3D12_SUBRESOURCE_FOOTPRINT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout {
    format: i32,
    width: u32,
    height: u32,
    depth: u32,
    row_pitch: u32,
}

/// Reads `D3D12_SUBRESOURCE_FOOTPRINT` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_subresource_footprint(
    bytes: &[u8],
) -> direct3d_12::structures::SubresourceFootprint {
    direct3d_12::structures::SubresourceFootprint {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
                format
            ),
        )),
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
                height
            ),
        ),
        depth: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
                depth
            ),
        ),
        row_pitch: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
                row_pitch
            ),
        ),
    }
}

/// Writes `D3D12_SUBRESOURCE_FOOTPRINT` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_subresource_footprint(
    value: &direct3d_12::structures::SubresourceFootprint,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
            format
        ),
        value.format.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
            depth
        ),
        value.depth,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
            row_pitch
        ),
        value.row_pitch,
    );
}

/// `D3D12_PLACED_SUBRESOURCE_FOOTPRINT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout {
    offset: u64,
    footprint: WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout,
}

/// Reads `D3D12_PLACED_SUBRESOURCE_FOOTPRINT` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(
    bytes: &[u8],
) -> direct3d_12::structures::PlacedSubresourceFootprint {
    direct3d_12::structures::PlacedSubresourceFootprint {
        offset: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
                offset
            ),
        ),
        footprint: decode_windows_win32_graphics_direct_3d12_d_3d12_subresource_footprint(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
                footprint
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
                    footprint
                ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout>()],
        ),
    }
}

/// Writes `D3D12_PLACED_SUBRESOURCE_FOOTPRINT` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(
    value: &direct3d_12::structures::PlacedSubresourceFootprint,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
            offset
        ),
        value.offset,
    );
    encode_windows_win32_graphics_direct_3d12_d_3d12_subresource_footprint(
        &value.footprint,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
            footprint
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12PlacedSubresourceFootprintLayout,
                footprint
            ) + size_of::<WindowsWin32GraphicsDirect3d12D3d12SubresourceFootprintLayout>()],
    );
}

/// `D3D12_QUERY_HEAP_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout {
    type_: i32,
    count: u32,
    node_mask: u32,
}

/// Reads `D3D12_QUERY_HEAP_DESC` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(
    bytes: &[u8],
) -> direct3d_12::structures::QueryHeapDesc {
    direct3d_12::structures::QueryHeapDesc {
        type_: direct3d_12::enumerations::QueryHeapType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
                type_
            ),
        )),
        count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
                count
            ),
        ),
        node_mask: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
                node_mask
            ),
        ),
    }
}

/// Writes `D3D12_QUERY_HEAP_DESC` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(
    value: &direct3d_12::structures::QueryHeapDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
            type_
        ),
        value.type_.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
            count
        ),
        value.count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout,
            node_mask
        ),
        value.node_mask,
    );
}

/// `D3D12_INDEX_BUFFER_VIEW` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout {
    buffer_location: u64,
    size_in_bytes: u32,
    format: i32,
}

/// Reads `D3D12_INDEX_BUFFER_VIEW` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_index_buffer_view(
    bytes: &[u8],
) -> direct3d_12::structures::IndexBufferView {
    direct3d_12::structures::IndexBufferView {
        buffer_location: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
                buffer_location
            ),
        ),
        size_in_bytes: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
                size_in_bytes
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
                format
            ),
        )),
    }
}

/// Writes `D3D12_INDEX_BUFFER_VIEW` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_index_buffer_view(
    value: &direct3d_12::structures::IndexBufferView,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
            buffer_location
        ),
        value.buffer_location,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
            size_in_bytes
        ),
        value.size_in_bytes,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout,
            format
        ),
        value.format.0,
    );
}

/// `D3D12_VERTEX_BUFFER_VIEW` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout {
    buffer_location: u64,
    size_in_bytes: u32,
    stride_in_bytes: u32,
}

/// Reads `D3D12_VERTEX_BUFFER_VIEW` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_vertex_buffer_view(
    bytes: &[u8],
) -> direct3d_12::structures::VertexBufferView {
    direct3d_12::structures::VertexBufferView {
        buffer_location: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
                buffer_location
            ),
        ),
        size_in_bytes: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
                size_in_bytes
            ),
        ),
        stride_in_bytes: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
                stride_in_bytes
            ),
        ),
    }
}

/// Writes `D3D12_VERTEX_BUFFER_VIEW` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_vertex_buffer_view(
    value: &direct3d_12::structures::VertexBufferView,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
            buffer_location
        ),
        value.buffer_location,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
            size_in_bytes
        ),
        value.size_in_bytes,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12VertexBufferViewLayout,
            stride_in_bytes
        ),
        value.stride_in_bytes,
    );
}

/// `D3D12_STREAM_OUTPUT_BUFFER_VIEW` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout {
    buffer_location: u64,
    size_in_bytes: u64,
    buffer_filled_size_location: u64,
}

/// Reads `D3D12_STREAM_OUTPUT_BUFFER_VIEW` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_stream_output_buffer_view(
    bytes: &[u8],
) -> direct3d_12::structures::StreamOutputBufferView {
    direct3d_12::structures::StreamOutputBufferView {
        buffer_location: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
                buffer_location
            ),
        ),
        size_in_bytes: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
                size_in_bytes
            ),
        ),
        buffer_filled_size_location: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
                buffer_filled_size_location
            ),
        ),
    }
}

/// Writes `D3D12_STREAM_OUTPUT_BUFFER_VIEW` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_stream_output_buffer_view(
    value: &direct3d_12::structures::StreamOutputBufferView,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
            buffer_location
        ),
        value.buffer_location,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
            size_in_bytes
        ),
        value.size_in_bytes,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12StreamOutputBufferViewLayout,
            buffer_filled_size_location
        ),
        value.buffer_filled_size_location,
    );
}

/// `D3D12_CPU_DESCRIPTOR_HANDLE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout {
    ptr: usize,
}

/// Reads `D3D12_CPU_DESCRIPTOR_HANDLE` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
    bytes: &[u8],
) -> direct3d_12::structures::CpuDescriptorHandle {
    direct3d_12::structures::CpuDescriptorHandle {
        ptr: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout,
                ptr
            ),
        ),
    }
}

/// Writes `D3D12_CPU_DESCRIPTOR_HANDLE` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
    value: &direct3d_12::structures::CpuDescriptorHandle,
    bytes: &mut [u8],
) {
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout,
            ptr
        ),
        value.ptr,
    );
}

/// `D3D12_RANGE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12RangeLayout {
    begin: usize,
    end: usize,
}

/// Reads `D3D12_RANGE` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_range(
    bytes: &[u8],
) -> direct3d_12::structures::Range {
    direct3d_12::structures::Range {
        begin: get_usize(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12RangeLayout, begin),
        ),
        end: get_usize(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d12D3d12RangeLayout, end),
        ),
    }
}

/// Writes `D3D12_RANGE` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_range(
    value: &direct3d_12::structures::Range,
    bytes: &mut [u8],
) {
    put_usize(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12RangeLayout, begin),
        value.begin,
    );
    put_usize(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d12D3d12RangeLayout, end),
        value.end,
    );
}

/// `D3D12_GPU_DESCRIPTOR_HANDLE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d12D3d12GpuDescriptorHandleLayout {
    ptr: u64,
}

/// Reads `D3D12_GPU_DESCRIPTOR_HANDLE` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d12_d_3d12_gpu_descriptor_handle(
    bytes: &[u8],
) -> direct3d_12::structures::GpuDescriptorHandle {
    direct3d_12::structures::GpuDescriptorHandle {
        ptr: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d12D3d12GpuDescriptorHandleLayout,
                ptr
            ),
        ),
    }
}

/// Writes `D3D12_GPU_DESCRIPTOR_HANDLE` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d12_d_3d12_gpu_descriptor_handle(
    value: &direct3d_12::structures::GpuDescriptorHandle,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d12D3d12GpuDescriptorHandleLayout,
            ptr
        ),
        value.ptr,
    );
}
