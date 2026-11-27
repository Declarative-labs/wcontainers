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
