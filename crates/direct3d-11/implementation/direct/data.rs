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
use wcontainers_direct3d_11 as direct3d_11;
use wcontainers_direct3d_common as direct3d_common;
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

/// `D3D11_RENDER_TARGET_BLEND_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout {
    blend_enable: i32,
    src_blend: i32,
    dest_blend: i32,
    blend_op: i32,
    src_blend_alpha: i32,
    dest_blend_alpha: i32,
    blend_op_alpha: i32,
    render_target_write_mask: u8,
}

/// Reads `D3D11_RENDER_TARGET_BLEND_DESC` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetBlendDesc {
    direct3d_11::structures::RenderTargetBlendDesc {
        blend_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                blend_enable
            ),
        ) != 0,
        src_blend: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                src_blend
            ),
        )),
        dest_blend: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                dest_blend
            ),
        )),
        blend_op: direct3d_11::enumerations::BlendOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                blend_op
            ),
        )),
        src_blend_alpha: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                src_blend_alpha
            ),
        )),
        dest_blend_alpha: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                dest_blend_alpha
            ),
        )),
        blend_op_alpha: direct3d_11::enumerations::BlendOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                blend_op_alpha
            ),
        )),
        render_target_write_mask: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
                render_target_write_mask
            ),
        ),
    }
}

/// Writes `D3D11_RENDER_TARGET_BLEND_DESC` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc(
    value: &direct3d_11::structures::RenderTargetBlendDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            blend_enable
        ),
        i32::from(value.blend_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            src_blend
        ),
        value.src_blend.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            dest_blend
        ),
        value.dest_blend.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            blend_op
        ),
        value.blend_op.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            src_blend_alpha
        ),
        value.src_blend_alpha.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            dest_blend_alpha
        ),
        value.dest_blend_alpha.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            blend_op_alpha
        ),
        value.blend_op_alpha.0,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout,
            render_target_write_mask
        ),
        value.render_target_write_mask,
    );
}

/// `D3D11_BLEND_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout {
    alpha_to_coverage_enable: i32,
    independent_blend_enable: i32,
    render_target: [WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout; 8],
}

/// Reads `D3D11_BLEND_DESC` from the 264 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc(
    bytes: &[u8],
) -> direct3d_11::structures::BlendDesc {
    direct3d_11::structures::BlendDesc {
        alpha_to_coverage_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                alpha_to_coverage_enable
            ),
        ) != 0,
        independent_blend_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                independent_blend_enable
            ),
        ) != 0,
        render_target: core::array::from_fn(|index| {
            decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                    render_target
                ) + index
                    * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>()
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                        render_target
                    ) + index
                        * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>(
                        )
                        + size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>(
                        )],
            )
        }),
    }
}

/// Writes `D3D11_BLEND_DESC` as the 264 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc(
    value: &direct3d_11::structures::BlendDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
            alpha_to_coverage_enable
        ),
        i32::from(value.alpha_to_coverage_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
            independent_blend_enable
        ),
        i32::from(value.independent_blend_enable),
    );
    for (index, item) in value.render_target.iter().enumerate() {
        encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc(
            item,
            &mut bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                render_target
            ) + index
                * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>()
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BlendDescLayout,
                    render_target
                ) + index
                    * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>()
                    + size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDescLayout>()],
        );
    }
}

/// `D3D11_RENDER_TARGET_BLEND_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout {
    blend_enable: i32,
    logic_op_enable: i32,
    src_blend: i32,
    dest_blend: i32,
    blend_op: i32,
    src_blend_alpha: i32,
    dest_blend_alpha: i32,
    blend_op_alpha: i32,
    logic_op: i32,
    render_target_write_mask: u8,
}

/// Reads `D3D11_RENDER_TARGET_BLEND_DESC1` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetBlendDesc1 {
    direct3d_11::structures::RenderTargetBlendDesc1 {
        blend_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                blend_enable
            ),
        ) != 0,
        logic_op_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                logic_op_enable
            ),
        ) != 0,
        src_blend: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                src_blend
            ),
        )),
        dest_blend: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                dest_blend
            ),
        )),
        blend_op: direct3d_11::enumerations::BlendOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                blend_op
            ),
        )),
        src_blend_alpha: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                src_blend_alpha
            ),
        )),
        dest_blend_alpha: direct3d_11::enumerations::Blend(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                dest_blend_alpha
            ),
        )),
        blend_op_alpha: direct3d_11::enumerations::BlendOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                blend_op_alpha
            ),
        )),
        logic_op: direct3d_11::enumerations::LogicOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                logic_op
            ),
        )),
        render_target_write_mask: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
                render_target_write_mask
            ),
        ),
    }
}

/// Writes `D3D11_RENDER_TARGET_BLEND_DESC1` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc1(
    value: &direct3d_11::structures::RenderTargetBlendDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            blend_enable
        ),
        i32::from(value.blend_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            logic_op_enable
        ),
        i32::from(value.logic_op_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            src_blend
        ),
        value.src_blend.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            dest_blend
        ),
        value.dest_blend.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            blend_op
        ),
        value.blend_op.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            src_blend_alpha
        ),
        value.src_blend_alpha.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            dest_blend_alpha
        ),
        value.dest_blend_alpha.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            blend_op_alpha
        ),
        value.blend_op_alpha.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            logic_op
        ),
        value.logic_op.0,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout,
            render_target_write_mask
        ),
        value.render_target_write_mask,
    );
}

/// `D3D11_BLEND_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout {
    alpha_to_coverage_enable: i32,
    independent_blend_enable: i32,
    render_target: [WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout; 8],
}

/// Reads `D3D11_BLEND_DESC1` from the 328 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::BlendDesc1 {
    direct3d_11::structures::BlendDesc1 {
        alpha_to_coverage_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                alpha_to_coverage_enable
            ),
        ) != 0,
        independent_blend_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                independent_blend_enable
            ),
        ) != 0,
        render_target: core::array::from_fn(|index| {
            decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc1(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                    render_target
                ) + index
                    * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>()
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                        render_target
                    ) + index
                        * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>(
                        )
                        + size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>(
                        )],
            )
        }),
    }
}

/// Writes `D3D11_BLEND_DESC1` as the 328 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc1(
    value: &direct3d_11::structures::BlendDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
            alpha_to_coverage_enable
        ),
        i32::from(value.alpha_to_coverage_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
            independent_blend_enable
        ),
        i32::from(value.independent_blend_enable),
    );
    for (index, item) in value.render_target.iter().enumerate() {
        encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_blend_desc1(
            item,
            &mut bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                render_target
            ) + index
                * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>()
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BlendDesc1Layout,
                    render_target
                ) + index
                    * size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>()
                    + size_of::<WindowsWin32GraphicsDirect3d11D3d11RenderTargetBlendDesc1Layout>()],
        );
    }
}

/// `D3D11_BUFFER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout {
    byte_width: u32,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
    structure_byte_stride: u32,
}

/// Reads `D3D11_BUFFER_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_desc(
    bytes: &[u8],
) -> direct3d_11::structures::BufferDesc {
    direct3d_11::structures::BufferDesc {
        byte_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
                byte_width
            ),
        ),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout, usage),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
                misc_flags
            ),
        ),
        structure_byte_stride: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
                structure_byte_stride
            ),
        ),
    }
}

/// Writes `D3D11_BUFFER_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_desc(
    value: &direct3d_11::structures::BufferDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
            byte_width
        ),
        value.byte_width,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout, usage),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
            misc_flags
        ),
        value.misc_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferDescLayout,
            structure_byte_stride
        ),
        value.structure_byte_stride,
    );
}

/// `D3D11_CLASS_INSTANCE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout {
    instance_id: u32,
    instance_index: u32,
    type_id: u32,
    constant_buffer: u32,
    base_constant_buffer_offset: u32,
    base_texture: u32,
    base_sampler: u32,
    created: i32,
}

/// Reads `D3D11_CLASS_INSTANCE_DESC` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_class_instance_desc(
    bytes: &[u8],
) -> direct3d_11::structures::ClassInstanceDesc {
    direct3d_11::structures::ClassInstanceDesc {
        instance_id: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                instance_id
            ),
        ),
        instance_index: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                instance_index
            ),
        ),
        type_id: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                type_id
            ),
        ),
        constant_buffer: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                constant_buffer
            ),
        ),
        base_constant_buffer_offset: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                base_constant_buffer_offset
            ),
        ),
        base_texture: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                base_texture
            ),
        ),
        base_sampler: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                base_sampler
            ),
        ),
        created: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
                created
            ),
        ) != 0,
    }
}

/// Writes `D3D11_CLASS_INSTANCE_DESC` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_class_instance_desc(
    value: &direct3d_11::structures::ClassInstanceDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            instance_id
        ),
        value.instance_id,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            instance_index
        ),
        value.instance_index,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            type_id
        ),
        value.type_id,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            constant_buffer
        ),
        value.constant_buffer,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            base_constant_buffer_offset
        ),
        value.base_constant_buffer_offset,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            base_texture
        ),
        value.base_texture,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            base_sampler
        ),
        value.base_sampler,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ClassInstanceDescLayout,
            created
        ),
        i32::from(value.created),
    );
}

/// `D3D11_COUNTER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11CounterDescLayout {
    counter: i32,
    misc_flags: u32,
}

/// Reads `D3D11_COUNTER_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_counter_desc(
    bytes: &[u8],
) -> direct3d_11::structures::CounterDesc {
    direct3d_11::structures::CounterDesc {
        counter: direct3d_11::enumerations::CounterKind(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11CounterDescLayout,
                counter
            ),
        )),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11CounterDescLayout,
                misc_flags
            ),
        ),
    }
}

/// Writes `D3D11_COUNTER_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_counter_desc(
    value: &direct3d_11::structures::CounterDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11CounterDescLayout,
            counter
        ),
        value.counter.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11CounterDescLayout,
            misc_flags
        ),
        value.misc_flags,
    );
}

/// `D3D11_DEPTH_STENCILOP_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout {
    stencil_fail_op: i32,
    stencil_depth_fail_op: i32,
    stencil_pass_op: i32,
    stencil_func: i32,
}

/// Reads `D3D11_DEPTH_STENCILOP_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
    bytes: &[u8],
) -> direct3d_11::structures::DepthStencilopDesc {
    direct3d_11::structures::DepthStencilopDesc {
        stencil_fail_op: direct3d_11::enumerations::StencilOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
                stencil_fail_op
            ),
        )),
        stencil_depth_fail_op: direct3d_11::enumerations::StencilOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
                stencil_depth_fail_op
            ),
        )),
        stencil_pass_op: direct3d_11::enumerations::StencilOp(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
                stencil_pass_op
            ),
        )),
        stencil_func: direct3d_11::enumerations::ComparisonFunc(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
                stencil_func
            ),
        )),
    }
}

/// Writes `D3D11_DEPTH_STENCILOP_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
    value: &direct3d_11::structures::DepthStencilopDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
            stencil_fail_op
        ),
        value.stencil_fail_op.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
            stencil_depth_fail_op
        ),
        value.stencil_depth_fail_op.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
            stencil_pass_op
        ),
        value.stencil_pass_op.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
            stencil_func
        ),
        value.stencil_func.0,
    );
}

/// `D3D11_DEPTH_STENCIL_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout {
    depth_enable: i32,
    depth_write_mask: i32,
    depth_func: i32,
    stencil_enable: i32,
    stencil_read_mask: u8,
    stencil_write_mask: u8,
    front_face: WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
    back_face: WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout,
}

/// Reads `D3D11_DEPTH_STENCIL_DESC` from the 52 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_desc(
    bytes: &[u8],
) -> direct3d_11::structures::DepthStencilDesc {
    direct3d_11::structures::DepthStencilDesc {
        depth_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                depth_enable
            ),
        ) != 0,
        depth_write_mask: direct3d_11::enumerations::DepthWriteMask(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                depth_write_mask
            ),
        )),
        depth_func: direct3d_11::enumerations::ComparisonFunc(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                depth_func
            ),
        )),
        stencil_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                stencil_enable
            ),
        ) != 0,
        stencil_read_mask: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                stencil_read_mask
            ),
        ),
        stencil_write_mask: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                stencil_write_mask
            ),
        ),
        front_face: decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                front_face
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                    front_face
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout>()],
        ),
        back_face: decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                back_face
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                    back_face
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout>()],
        ),
    }
}

/// Writes `D3D11_DEPTH_STENCIL_DESC` as the 52 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_desc(
    value: &direct3d_11::structures::DepthStencilDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            depth_enable
        ),
        i32::from(value.depth_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            depth_write_mask
        ),
        value.depth_write_mask.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            depth_func
        ),
        value.depth_func.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            stencil_enable
        ),
        i32::from(value.stencil_enable),
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            stencil_read_mask
        ),
        value.stencil_read_mask,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            stencil_write_mask
        ),
        value.stencil_write_mask,
    );
    encode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
        &value.front_face,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            front_face
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                front_face
            ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout>()],
    );
    encode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencilop_desc(
        &value.back_face,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
            back_face
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilDescLayout,
                back_face
            ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DepthStencilopDescLayout>()],
    );
}

/// `D3D11_TEX1D_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX1D_DSV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DDsv {
    direct3d_11::structures::Tex1DDsv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX1D_DSV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_dsv(
    value: &direct3d_11::structures::Tex1DDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX1D_ARRAY_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX1D_ARRAY_DSV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DArrayDsv {
    direct3d_11::structures::Tex1DArrayDsv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX1D_ARRAY_DSV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_dsv(
    value: &direct3d_11::structures::Tex1DArrayDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2D_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX2D_DSV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DDsv {
    direct3d_11::structures::Tex2DDsv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX2D_DSV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_dsv(
    value: &direct3d_11::structures::Tex2DDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_DSV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayDsv {
    direct3d_11::structures::Tex2DArrayDsv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_DSV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_dsv(
    value: &direct3d_11::structures::Tex2DArrayDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2DMS_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout {
    unused_field_nothing_to_define: u32,
}

/// Reads `D3D11_TEX2DMS_DSV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsDsv {
    direct3d_11::structures::Tex2DmsDsv {
        unused_field_nothing_to_define: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout,
                unused_field_nothing_to_define
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_DSV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_dsv(
    value: &direct3d_11::structures::Tex2DmsDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout,
            unused_field_nothing_to_define
        ),
        value.unused_field_nothing_to_define,
    );
}

/// `D3D11_TEX2DMS_ARRAY_DSV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout {
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2DMS_ARRAY_DSV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_dsv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsArrayDsv {
    direct3d_11::structures::Tex2DmsArrayDsv {
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_ARRAY_DSV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_dsv(
    value: &direct3d_11::structures::Tex2DmsArrayDsv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout {
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout,
    texture_2dms: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout,
}

/// Reads `_Anonymous_e__Union` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_depth_stencil_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::DepthStencilViewDescAnonymous {
    direct3d_11::structures::DepthStencilViewDescAnonymous {
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_dsv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_depth_stencil_view_desc_anonymous(
    value: &direct3d_11::structures::DepthStencilViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_dsv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dDsvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_dsv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayDsvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_dsv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dDsvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_dsv(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayDsvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_dsv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsDsvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_dsv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayDsvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_DEPTH_STENCIL_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout {
    format: i32,
    view_dimension: i32,
    flags: u32,
    anonymous: WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout,
}

/// Reads `D3D11_DEPTH_STENCIL_VIEW_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::DepthStencilViewDesc {
    direct3d_11::structures::DepthStencilViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_11::enumerations::DsvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                view_dimension
            ),
        )),
        flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                flags
            ),
        ),
        anonymous: decode_windows_win32_graphics_direct_3d11_depth_stencil_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D11_DEPTH_STENCIL_VIEW_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_view_desc(
    value: &direct3d_11::structures::DepthStencilViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
            flags
        ),
        value.flags,
    );
    encode_windows_win32_graphics_direct_3d11_depth_stencil_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DepthStencilViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11DepthStencilViewDescAnonymousLayout>()],
    );
}

/// `D3D11_TEXTURE1D_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout {
    width: u32,
    mip_levels: u32,
    array_size: u32,
    format: i32,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
}

/// Reads `D3D11_TEXTURE1D_DESC` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texture_1d_desc(
    bytes: &[u8],
) -> direct3d_11::structures::Texture1DDesc {
    direct3d_11::structures::Texture1DDesc {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                width
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                mip_levels
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                array_size
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                format
            ),
        )),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                usage
            ),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
                misc_flags
            ),
        ),
    }
}

/// Writes `D3D11_TEXTURE1D_DESC` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texture_1d_desc(
    value: &direct3d_11::structures::Texture1DDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            array_size
        ),
        value.array_size,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            usage
        ),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture1dDescLayout,
            misc_flags
        ),
        value.misc_flags,
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

/// `D3D11_TEXTURE2D_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: i32,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
}

/// Reads `D3D11_TEXTURE2D_DESC` from the 44 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc(
    bytes: &[u8],
) -> direct3d_11::structures::Texture2DDesc {
    direct3d_11::structures::Texture2DDesc {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                height
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                mip_levels
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                array_size
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                format
            ),
        )),
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                sample_desc
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                    sample_desc
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                usage
            ),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                misc_flags
            ),
        ),
    }
}

/// Writes `D3D11_TEXTURE2D_DESC` as the 44 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc(
    value: &direct3d_11::structures::Texture2DDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            array_size
        ),
        value.array_size,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            format
        ),
        value.format.0,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            sample_desc
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
                sample_desc
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            usage
        ),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDescLayout,
            misc_flags
        ),
        value.misc_flags,
    );
}

/// `D3D11_TEXTURE3D_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout {
    width: u32,
    height: u32,
    depth: u32,
    mip_levels: u32,
    format: i32,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
}

/// Reads `D3D11_TEXTURE3D_DESC` from the 36 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc(
    bytes: &[u8],
) -> direct3d_11::structures::Texture3DDesc {
    direct3d_11::structures::Texture3DDesc {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                height
            ),
        ),
        depth: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                depth
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                mip_levels
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                format
            ),
        )),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                usage
            ),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
                misc_flags
            ),
        ),
    }
}

/// Writes `D3D11_TEXTURE3D_DESC` as the 36 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc(
    value: &direct3d_11::structures::Texture3DDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            depth
        ),
        value.depth,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            usage
        ),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDescLayout,
            misc_flags
        ),
        value.misc_flags,
    );
}

/// `_Anonymous1_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout {
    first_element: u32,
    element_offset: u32,
}

/// Reads `_Anonymous1_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous1(
    bytes: &[u8],
) -> direct3d_11::structures::BufferSrvAnonymous1 {
    direct3d_11::structures::BufferSrvAnonymous1 {
        first_element: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout,
                first_element
            ),
        ),
        element_offset: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout,
                element_offset
            ),
        ),
    }
}

/// Writes `_Anonymous1_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous1(
    value: &direct3d_11::structures::BufferSrvAnonymous1,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout,
                    first_element
                ),
                value.first_element,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout,
                    element_offset
                ),
                value.element_offset,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()]
        .copy_from_slice(&chosen.1);
}

/// `_Anonymous2_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout {
    num_elements: u32,
    element_width: u32,
}

/// Reads `_Anonymous2_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous2(
    bytes: &[u8],
) -> direct3d_11::structures::BufferSrvAnonymous2 {
    direct3d_11::structures::BufferSrvAnonymous2 {
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout,
                num_elements
            ),
        ),
        element_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout,
                element_width
            ),
        ),
    }
}

/// Writes `_Anonymous2_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous2(
    value: &direct3d_11::structures::BufferSrvAnonymous2,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout,
                    num_elements
                ),
                value.num_elements,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout,
                    element_width
                ),
                value.element_width,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_BUFFER_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout {
    anonymous1: WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout,
    anonymous2: WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout,
}

/// Reads `D3D11_BUFFER_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
    bytes: &[u8],
) -> direct3d_11::structures::BufferSrv {
    direct3d_11::structures::BufferSrv {
        anonymous1: decode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                anonymous1
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                    anonymous1
                ) + size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()],
        ),
        anonymous2: decode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous2(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                anonymous2
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                    anonymous2
                ) + size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()],
        ),
    }
}

/// Writes `D3D11_BUFFER_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
    value: &direct3d_11::structures::BufferSrv,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous1(
        &value.anonymous1,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
            anonymous1
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                anonymous1
            ) + size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous1Layout>()],
    );
    encode_windows_win32_graphics_direct_3d11_buffer_srv_anonymous2(
        &value.anonymous2,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
            anonymous2
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
                anonymous2
            ) + size_of::<WindowsWin32GraphicsDirect3d11BufferSrvAnonymous2Layout>()],
    );
}

/// `D3D11_TEX1D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
}

/// Reads `D3D11_TEX1D_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DSrv {
    direct3d_11::structures::Tex1DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
                mip_levels
            ),
        ),
    }
}

/// Writes `D3D11_TEX1D_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
    value: &direct3d_11::structures::Tex1DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
}

/// `D3D11_TEX1D_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX1D_ARRAY_SRV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DArraySrv {
    direct3d_11::structures::Tex1DArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
                mip_levels
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX1D_ARRAY_SRV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
    value: &direct3d_11::structures::Tex1DArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
}

/// Reads `D3D11_TEX2D_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DSrv {
    direct3d_11::structures::Tex2DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout,
                mip_levels
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv(
    value: &direct3d_11::structures::Tex2DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
}

/// `D3D11_TEX2D_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_SRV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArraySrv {
    direct3d_11::structures::Tex2DArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
                mip_levels
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_SRV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv(
    value: &direct3d_11::structures::Tex2DArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2DMS_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout {
    unused_field_nothing_to_define: u32,
}

/// Reads `D3D11_TEX2DMS_SRV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsSrv {
    direct3d_11::structures::Tex2DmsSrv {
        unused_field_nothing_to_define: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout,
                unused_field_nothing_to_define
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_SRV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
    value: &direct3d_11::structures::Tex2DmsSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout,
            unused_field_nothing_to_define
        ),
        value.unused_field_nothing_to_define,
    );
}

/// `D3D11_TEX2DMS_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout {
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2DMS_ARRAY_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsArraySrv {
    direct3d_11::structures::Tex2DmsArraySrv {
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_ARRAY_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
    value: &direct3d_11::structures::Tex2DmsArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX3D_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
}

/// Reads `D3D11_TEX3D_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex3DSrv {
    direct3d_11::structures::Tex3DSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
                mip_levels
            ),
        ),
    }
}

/// Writes `D3D11_TEX3D_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
    value: &direct3d_11::structures::Tex3DSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
}

/// `D3D11_TEXCUBE_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
}

/// Reads `D3D11_TEXCUBE_SRV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
    bytes: &[u8],
) -> direct3d_11::structures::TexcubeSrv {
    direct3d_11::structures::TexcubeSrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
                mip_levels
            ),
        ),
    }
}

/// Writes `D3D11_TEXCUBE_SRV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
    value: &direct3d_11::structures::TexcubeSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
}

/// `D3D11_TEXCUBE_ARRAY_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_2d_array_face: u32,
    num_cubes: u32,
}

/// Reads `D3D11_TEXCUBE_ARRAY_SRV` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
    bytes: &[u8],
) -> direct3d_11::structures::TexcubeArraySrv {
    direct3d_11::structures::TexcubeArraySrv {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
                mip_levels
            ),
        ),
        first_2d_array_face: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
                first_2d_array_face
            ),
        ),
        num_cubes: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
                num_cubes
            ),
        ),
    }
}

/// Writes `D3D11_TEXCUBE_ARRAY_SRV` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
    value: &direct3d_11::structures::TexcubeArraySrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
            first_2d_array_face
        ),
        value.first_2d_array_face,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
            num_cubes
        ),
        value.num_cubes,
    );
}

/// `D3D11_BUFFEREX_SRV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout {
    first_element: u32,
    num_elements: u32,
    flags: u32,
}

/// Reads `D3D11_BUFFEREX_SRV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
    bytes: &[u8],
) -> direct3d_11::structures::BufferexSrv {
    direct3d_11::structures::BufferexSrv {
        first_element: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
                first_element
            ),
        ),
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
                num_elements
            ),
        ),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout, flags),
        ),
    }
}

/// Writes `D3D11_BUFFEREX_SRV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
    value: &direct3d_11::structures::BufferexSrv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
            first_element
        ),
        value.first_element,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
            num_elements
        ),
        value.num_elements,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout, flags),
        value.flags,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout,
    texture_2dms: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
    texture_cube: WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
    texture_cube_array: WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
    buffer_ex: WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_shader_resource_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderResourceViewDescAnonymous {
    direct3d_11::structures::ShaderResourceViewDescAnonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>()],
        ),
        texture_cube: decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_cube
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_cube
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>()],
        ),
        texture_cube_array: decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                texture_cube_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_cube_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>()],
        ),
        buffer_ex: decode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                buffer_ex
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    buffer_ex
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_shader_resource_view_desc_anonymous(
    value: &direct3d_11::structures::ShaderResourceViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
                &value.texture_cube,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_cube
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_cube
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
                &value.texture_cube_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    texture_cube_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        texture_cube_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
                &value.buffer_ex,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                    buffer_ex
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
                        buffer_ex
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_SHADER_RESOURCE_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout,
}

/// Reads `D3D11_SHADER_RESOURCE_VIEW_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderResourceViewDesc {
    direct3d_11::structures::ShaderResourceViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_common::enumerations::SrvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_shader_resource_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D11_SHADER_RESOURCE_VIEW_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc(
    value: &direct3d_11::structures::ShaderResourceViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_shader_resource_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDescAnonymousLayout>()],
    );
}

/// `D3D11_BUFFER_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout {
    first_element: u32,
    num_elements: u32,
    flags: u32,
}

/// Reads `D3D11_BUFFER_UAV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
    bytes: &[u8],
) -> direct3d_11::structures::BufferUav {
    direct3d_11::structures::BufferUav {
        first_element: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
                first_element
            ),
        ),
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
                num_elements
            ),
        ),
        flags: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout, flags),
        ),
    }
}

/// Writes `D3D11_BUFFER_UAV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
    value: &direct3d_11::structures::BufferUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
            first_element
        ),
        value.first_element,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
            num_elements
        ),
        value.num_elements,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout, flags),
        value.flags,
    );
}

/// `D3D11_TEX1D_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX1D_UAV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DUav {
    direct3d_11::structures::Tex1DUav {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX1D_UAV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
    value: &direct3d_11::structures::Tex1DUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX1D_ARRAY_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX1D_ARRAY_UAV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DArrayUav {
    direct3d_11::structures::Tex1DArrayUav {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX1D_ARRAY_UAV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
    value: &direct3d_11::structures::Tex1DArrayUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2D_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX2D_UAV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DUav {
    direct3d_11::structures::Tex2DUav {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX2D_UAV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav(
    value: &direct3d_11::structures::Tex2DUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_UAV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayUav {
    direct3d_11::structures::Tex2DArrayUav {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_UAV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav(
    value: &direct3d_11::structures::Tex2DArrayUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX3D_UAV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout {
    mip_slice: u32,
    first_w_slice: u32,
    w_size: u32,
}

/// Reads `D3D11_TEX3D_UAV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
    bytes: &[u8],
) -> direct3d_11::structures::Tex3DUav {
    direct3d_11::structures::Tex3DUav {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout, mip_slice),
        ),
        first_w_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout,
                first_w_slice
            ),
        ),
        w_size: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout, w_size),
        ),
    }
}

/// Writes `D3D11_TEX3D_UAV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
    value: &direct3d_11::structures::Tex3DUav,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout, mip_slice),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout,
            first_w_slice
        ),
        value.first_w_slice,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout, w_size),
        value.w_size,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout,
}

/// Reads `_Anonymous_e__Union` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_unordered_access_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::UnorderedAccessViewDescAnonymous {
    direct3d_11::structures::UnorderedAccessViewDescAnonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_unordered_access_view_desc_anonymous(
    value: &direct3d_11::structures::UnorderedAccessViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_UNORDERED_ACCESS_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
}

/// Reads `D3D11_UNORDERED_ACCESS_VIEW_DESC` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::UnorderedAccessViewDesc {
    direct3d_11::structures::UnorderedAccessViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_11::enumerations::UavDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_unordered_access_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
                    anonymous
                ) + size_of::<
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout,
                >()],
        ),
    }
}

/// Writes `D3D11_UNORDERED_ACCESS_VIEW_DESC` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc(
    value: &direct3d_11::structures::UnorderedAccessViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_unordered_access_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDescAnonymousLayout>(
            )],
    );
}

/// `_Anonymous1_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout {
    first_element: u32,
    element_offset: u32,
}

/// Reads `_Anonymous1_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous1(
    bytes: &[u8],
) -> direct3d_11::structures::BufferRtvAnonymous1 {
    direct3d_11::structures::BufferRtvAnonymous1 {
        first_element: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout,
                first_element
            ),
        ),
        element_offset: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout,
                element_offset
            ),
        ),
    }
}

/// Writes `_Anonymous1_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous1(
    value: &direct3d_11::structures::BufferRtvAnonymous1,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout,
                    first_element
                ),
                value.first_element,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout,
                    element_offset
                ),
                value.element_offset,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()]
        .copy_from_slice(&chosen.1);
}

/// `_Anonymous2_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout {
    num_elements: u32,
    element_width: u32,
}

/// Reads `_Anonymous2_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous2(
    bytes: &[u8],
) -> direct3d_11::structures::BufferRtvAnonymous2 {
    direct3d_11::structures::BufferRtvAnonymous2 {
        num_elements: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout,
                num_elements
            ),
        ),
        element_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout,
                element_width
            ),
        ),
    }
}

/// Writes `_Anonymous2_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous2(
    value: &direct3d_11::structures::BufferRtvAnonymous2,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout,
                    num_elements
                ),
                value.num_elements,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()];
        {
            let bytes = &mut scratch[..];
            put_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout,
                    element_width
                ),
                value.element_width,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()]
            && size_of::<u32>() > chosen.0
        {
            chosen = (size_of::<u32>(), scratch);
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_BUFFER_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout {
    anonymous1: WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout,
    anonymous2: WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout,
}

/// Reads `D3D11_BUFFER_RTV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::BufferRtv {
    direct3d_11::structures::BufferRtv {
        anonymous1: decode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                anonymous1
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                    anonymous1
                ) + size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()],
        ),
        anonymous2: decode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous2(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                anonymous2
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                    anonymous2
                ) + size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()],
        ),
    }
}

/// Writes `D3D11_BUFFER_RTV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
    value: &direct3d_11::structures::BufferRtv,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous1(
        &value.anonymous1,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
            anonymous1
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                anonymous1
            ) + size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous1Layout>()],
    );
    encode_windows_win32_graphics_direct_3d11_buffer_rtv_anonymous2(
        &value.anonymous2,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
            anonymous2
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
                anonymous2
            ) + size_of::<WindowsWin32GraphicsDirect3d11BufferRtvAnonymous2Layout>()],
    );
}

/// `D3D11_TEX1D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX1D_RTV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DRtv {
    direct3d_11::structures::Tex1DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX1D_RTV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
    value: &direct3d_11::structures::Tex1DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX1D_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX1D_ARRAY_RTV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex1DArrayRtv {
    direct3d_11::structures::Tex1DArrayRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX1D_ARRAY_RTV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
    value: &direct3d_11::structures::Tex1DArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX2D_RTV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DRtv {
    direct3d_11::structures::Tex2DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout, mip_slice),
        ),
    }
}

/// Writes `D3D11_TEX2D_RTV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv(
    value: &direct3d_11::structures::Tex2DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout, mip_slice),
        value.mip_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_RTV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayRtv {
    direct3d_11::structures::Tex2DArrayRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_RTV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv(
    value: &direct3d_11::structures::Tex2DArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX2DMS_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout {
    unused_field_nothing_to_define: u32,
}

/// Reads `D3D11_TEX2DMS_RTV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsRtv {
    direct3d_11::structures::Tex2DmsRtv {
        unused_field_nothing_to_define: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout,
                unused_field_nothing_to_define
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_RTV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
    value: &direct3d_11::structures::Tex2DmsRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout,
            unused_field_nothing_to_define
        ),
        value.unused_field_nothing_to_define,
    );
}

/// `D3D11_TEX2DMS_ARRAY_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout {
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2DMS_ARRAY_RTV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DmsArrayRtv {
    direct3d_11::structures::Tex2DmsArrayRtv {
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2DMS_ARRAY_RTV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
    value: &direct3d_11::structures::Tex2DmsArrayRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `D3D11_TEX3D_RTV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout {
    mip_slice: u32,
    first_w_slice: u32,
    w_size: u32,
}

/// Reads `D3D11_TEX3D_RTV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex3DRtv {
    direct3d_11::structures::Tex3DRtv {
        mip_slice: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout, mip_slice),
        ),
        first_w_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout,
                first_w_slice
            ),
        ),
        w_size: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout, w_size),
        ),
    }
}

/// Writes `D3D11_TEX3D_RTV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
    value: &direct3d_11::structures::Tex3DRtv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout, mip_slice),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout,
            first_w_slice
        ),
        value.first_w_slice,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout, w_size),
        value.w_size,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout,
    texture_2dms: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout,
}

/// Reads `_Anonymous_e__Union` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_render_target_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetViewDescAnonymous {
    direct3d_11::structures::RenderTargetViewDescAnonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_render_target_view_desc_anonymous(
    value: &direct3d_11::structures::RenderTargetViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_RENDER_TARGET_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout,
}

/// Reads `D3D11_RENDER_TARGET_VIEW_DESC` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetViewDesc {
    direct3d_11::structures::RenderTargetViewDesc {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
                format
            ),
        )),
        view_dimension: direct3d_11::enumerations::RtvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_render_target_view_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D11_RENDER_TARGET_VIEW_DESC` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc(
    value: &direct3d_11::structures::RenderTargetViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_render_target_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDescAnonymousLayout>()],
    );
}

/// `D3D11_INPUT_ELEMENT_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout {
    semantic_name: usize,
    semantic_index: u32,
    format: i32,
    input_slot: u32,
    aligned_byte_offset: u32,
    input_slot_class: i32,
    instance_data_step_rate: u32,
}

/// The offsets of `D3D11_INPUT_ELEMENT_DESC`'s texts, in the order of its fields.
pub(crate) const TEXTS_WINDOWS_WIN32_GRAPHICS_DIRECT_3D11_D_3D11_INPUT_ELEMENT_DESC: [usize; 1] =
    [offset_of!(
        WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
        semantic_name
    )];

/// Reads `D3D11_INPUT_ELEMENT_DESC` from the 32 bytes C lays it out in, its texts the ones given, in the order of its fields.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_input_element_desc_with(
    bytes: &[u8],
    texts: Vec<std::ffi::CString>,
) -> direct3d_11::structures::InputElementDesc {
    let mut texts = texts.into_iter();
    direct3d_11::structures::InputElementDesc {
        semantic_name: texts.next().unwrap_or_default(),
        semantic_index: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                semantic_index
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                format
            ),
        )),
        input_slot: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                input_slot
            ),
        ),
        aligned_byte_offset: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                aligned_byte_offset
            ),
        ),
        input_slot_class: direct3d_11::enumerations::InputClassification(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                input_slot_class
            ),
        )),
        instance_data_step_rate: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
                instance_data_step_rate
            ),
        ),
    }
}

/// Writes `D3D11_INPUT_ELEMENT_DESC` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_input_element_desc(
    value: &direct3d_11::structures::InputElementDesc,
    bytes: &mut [u8],
) {
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            semantic_name
        ),
        value.semantic_name.as_ptr().expose_provenance(),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            semantic_index
        ),
        value.semantic_index,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            format
        ),
        value.format.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            input_slot
        ),
        value.input_slot,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            aligned_byte_offset
        ),
        value.aligned_byte_offset,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            input_slot_class
        ),
        value.input_slot_class.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11InputElementDescLayout,
            instance_data_step_rate
        ),
        value.instance_data_step_rate,
    );
}

/// `D3D11_SO_DECLARATION_ENTRY` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout {
    stream: u32,
    semantic_name: usize,
    semantic_index: u32,
    start_component: u8,
    component_count: u8,
    output_slot: u8,
}

/// The offsets of `D3D11_SO_DECLARATION_ENTRY`'s texts, in the order of its fields.
pub(crate) const TEXTS_WINDOWS_WIN32_GRAPHICS_DIRECT_3D11_D_3D11_SO_DECLARATION_ENTRY: [usize; 1] =
    [offset_of!(
        WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
        semantic_name
    )];

/// Reads `D3D11_SO_DECLARATION_ENTRY` from the 24 bytes C lays it out in, its texts the ones given, in the order of its fields.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_so_declaration_entry_with(
    bytes: &[u8],
    texts: Vec<std::ffi::CString>,
) -> direct3d_11::structures::SoDeclarationEntry {
    let mut texts = texts.into_iter();
    direct3d_11::structures::SoDeclarationEntry {
        stream: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
                stream
            ),
        ),
        semantic_name: texts.next().unwrap_or_default(),
        semantic_index: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
                semantic_index
            ),
        ),
        start_component: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
                start_component
            ),
        ),
        component_count: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
                component_count
            ),
        ),
        output_slot: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
                output_slot
            ),
        ),
    }
}

/// Writes `D3D11_SO_DECLARATION_ENTRY` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_so_declaration_entry(
    value: &direct3d_11::structures::SoDeclarationEntry,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            stream
        ),
        value.stream,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            semantic_name
        ),
        value.semantic_name.as_ptr().expose_provenance(),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            semantic_index
        ),
        value.semantic_index,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            start_component
        ),
        value.start_component,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            component_count
        ),
        value.component_count,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SoDeclarationEntryLayout,
            output_slot
        ),
        value.output_slot,
    );
}

/// `D3D11_RASTERIZER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout {
    fill_mode: i32,
    cull_mode: i32,
    front_counter_clockwise: i32,
    depth_bias: i32,
    depth_bias_clamp: f32,
    slope_scaled_depth_bias: f32,
    depth_clip_enable: i32,
    scissor_enable: i32,
    multisample_enable: i32,
    antialiased_line_enable: i32,
}

/// Reads `D3D11_RASTERIZER_DESC` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc(
    bytes: &[u8],
) -> direct3d_11::structures::RasterizerDesc {
    direct3d_11::structures::RasterizerDesc {
        fill_mode: direct3d_11::enumerations::FillMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                fill_mode
            ),
        )),
        cull_mode: direct3d_11::enumerations::CullMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                cull_mode
            ),
        )),
        front_counter_clockwise: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                front_counter_clockwise
            ),
        ) != 0,
        depth_bias: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                depth_bias
            ),
        ),
        depth_bias_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                depth_bias_clamp
            ),
        ),
        slope_scaled_depth_bias: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                slope_scaled_depth_bias
            ),
        ),
        depth_clip_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                depth_clip_enable
            ),
        ) != 0,
        scissor_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                scissor_enable
            ),
        ) != 0,
        multisample_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                multisample_enable
            ),
        ) != 0,
        antialiased_line_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
                antialiased_line_enable
            ),
        ) != 0,
    }
}

/// Writes `D3D11_RASTERIZER_DESC` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc(
    value: &direct3d_11::structures::RasterizerDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            fill_mode
        ),
        value.fill_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            cull_mode
        ),
        value.cull_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            front_counter_clockwise
        ),
        i32::from(value.front_counter_clockwise),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            depth_bias
        ),
        value.depth_bias,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            depth_bias_clamp
        ),
        value.depth_bias_clamp,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            slope_scaled_depth_bias
        ),
        value.slope_scaled_depth_bias,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            depth_clip_enable
        ),
        i32::from(value.depth_clip_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            scissor_enable
        ),
        i32::from(value.scissor_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            multisample_enable
        ),
        i32::from(value.multisample_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDescLayout,
            antialiased_line_enable
        ),
        i32::from(value.antialiased_line_enable),
    );
}

/// `D3D11_SAMPLER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout {
    filter: i32,
    address_u: i32,
    address_v: i32,
    address_w: i32,
    mip_lod_bias: f32,
    max_anisotropy: u32,
    comparison_func: i32,
    border_color: [f32; 4],
    min_lod: f32,
    max_lod: f32,
}

/// Reads `D3D11_SAMPLER_DESC` from the 52 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_sampler_desc(
    bytes: &[u8],
) -> direct3d_11::structures::SamplerDesc {
    direct3d_11::structures::SamplerDesc {
        filter: direct3d_11::enumerations::Filter(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout, filter),
        )),
        address_u: direct3d_11::enumerations::TextureAddressMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                address_u
            ),
        )),
        address_v: direct3d_11::enumerations::TextureAddressMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                address_v
            ),
        )),
        address_w: direct3d_11::enumerations::TextureAddressMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                address_w
            ),
        )),
        mip_lod_bias: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                mip_lod_bias
            ),
        ),
        max_anisotropy: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                max_anisotropy
            ),
        ),
        comparison_func: direct3d_11::enumerations::ComparisonFunc(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                comparison_func
            ),
        )),
        border_color: core::array::from_fn(|index| {
            get_f32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                    border_color
                ) + index * size_of::<f32>(),
            )
        }),
        min_lod: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                min_lod
            ),
        ),
        max_lod: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                max_lod
            ),
        ),
    }
}

/// Writes `D3D11_SAMPLER_DESC` as the 52 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_sampler_desc(
    value: &direct3d_11::structures::SamplerDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout, filter),
        value.filter.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            address_u
        ),
        value.address_u.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            address_v
        ),
        value.address_v.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            address_w
        ),
        value.address_w.0,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            mip_lod_bias
        ),
        value.mip_lod_bias,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            max_anisotropy
        ),
        value.max_anisotropy,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            comparison_func
        ),
        value.comparison_func.0,
    );
    for (index, item) in value.border_color.iter().enumerate() {
        put_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
                border_color
            ) + index * size_of::<f32>(),
            *item,
        );
    }
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            min_lod
        ),
        value.min_lod,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11SamplerDescLayout,
            max_lod
        ),
        value.max_lod,
    );
}

/// `D3D11_QUERY_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11QueryDescLayout {
    query: i32,
    misc_flags: u32,
}

/// Reads `D3D11_QUERY_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_query_desc(
    bytes: &[u8],
) -> direct3d_11::structures::QueryDesc {
    direct3d_11::structures::QueryDesc {
        query: direct3d_11::enumerations::QueryKind(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11QueryDescLayout, query),
        )),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11QueryDescLayout,
                misc_flags
            ),
        ),
    }
}

/// Writes `D3D11_QUERY_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_query_desc(
    value: &direct3d_11::structures::QueryDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11QueryDescLayout, query),
        value.query.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11QueryDescLayout,
            misc_flags
        ),
        value.misc_flags,
    );
}

/// `D3D11_COUNTER_INFO` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout {
    last_device_dependent_counter: i32,
    num_simultaneous_counters: u32,
    num_detectable_parallel_units: u8,
}

/// Reads `D3D11_COUNTER_INFO` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_counter_info(
    bytes: &[u8],
) -> direct3d_11::structures::CounterInfo {
    direct3d_11::structures::CounterInfo {
        last_device_dependent_counter: direct3d_11::enumerations::CounterKind(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
                last_device_dependent_counter
            ),
        )),
        num_simultaneous_counters: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
                num_simultaneous_counters
            ),
        ),
        num_detectable_parallel_units: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
                num_detectable_parallel_units
            ),
        ),
    }
}

/// Writes `D3D11_COUNTER_INFO` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_counter_info(
    value: &direct3d_11::structures::CounterInfo,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
            last_device_dependent_counter
        ),
        value.last_device_dependent_counter.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
            num_simultaneous_counters
        ),
        value.num_simultaneous_counters,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11CounterInfoLayout,
            num_detectable_parallel_units
        ),
        value.num_detectable_parallel_units,
    );
}

/// `D3D11_RASTERIZER_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout {
    fill_mode: i32,
    cull_mode: i32,
    front_counter_clockwise: i32,
    depth_bias: i32,
    depth_bias_clamp: f32,
    slope_scaled_depth_bias: f32,
    depth_clip_enable: i32,
    scissor_enable: i32,
    multisample_enable: i32,
    antialiased_line_enable: i32,
    forced_sample_count: u32,
}

/// Reads `D3D11_RASTERIZER_DESC1` from the 44 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::RasterizerDesc1 {
    direct3d_11::structures::RasterizerDesc1 {
        fill_mode: direct3d_11::enumerations::FillMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                fill_mode
            ),
        )),
        cull_mode: direct3d_11::enumerations::CullMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                cull_mode
            ),
        )),
        front_counter_clockwise: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                front_counter_clockwise
            ),
        ) != 0,
        depth_bias: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                depth_bias
            ),
        ),
        depth_bias_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                depth_bias_clamp
            ),
        ),
        slope_scaled_depth_bias: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                slope_scaled_depth_bias
            ),
        ),
        depth_clip_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                depth_clip_enable
            ),
        ) != 0,
        scissor_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                scissor_enable
            ),
        ) != 0,
        multisample_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                multisample_enable
            ),
        ) != 0,
        antialiased_line_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                antialiased_line_enable
            ),
        ) != 0,
        forced_sample_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
                forced_sample_count
            ),
        ),
    }
}

/// Writes `D3D11_RASTERIZER_DESC1` as the 44 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc1(
    value: &direct3d_11::structures::RasterizerDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            fill_mode
        ),
        value.fill_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            cull_mode
        ),
        value.cull_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            front_counter_clockwise
        ),
        i32::from(value.front_counter_clockwise),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            depth_bias
        ),
        value.depth_bias,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            depth_bias_clamp
        ),
        value.depth_bias_clamp,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            slope_scaled_depth_bias
        ),
        value.slope_scaled_depth_bias,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            depth_clip_enable
        ),
        i32::from(value.depth_clip_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            scissor_enable
        ),
        i32::from(value.scissor_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            multisample_enable
        ),
        i32::from(value.multisample_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            antialiased_line_enable
        ),
        i32::from(value.antialiased_line_enable),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc1Layout,
            forced_sample_count
        ),
        value.forced_sample_count,
    );
}

/// `D3D11_PACKED_MIP_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11PackedMipDescLayout {
    num_standard_mips: u8,
    num_packed_mips: u8,
    num_tiles_for_packed_mips: u32,
    start_tile_index_in_overall_resource: u32,
}

/// Reads `D3D11_PACKED_MIP_DESC` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_packed_mip_desc(
    bytes: &[u8],
) -> direct3d_11::structures::PackedMipDesc {
    direct3d_11::structures::PackedMipDesc {
        num_standard_mips: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PackedMipDescLayout,
                num_standard_mips
            ),
        ),
        num_packed_mips: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PackedMipDescLayout,
                num_packed_mips
            ),
        ),
        num_tiles_for_packed_mips: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PackedMipDescLayout,
                num_tiles_for_packed_mips
            ),
        ),
        start_tile_index_in_overall_resource: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PackedMipDescLayout,
                start_tile_index_in_overall_resource
            ),
        ),
    }
}

/// `D3D11_TILE_SHAPE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TileShapeLayout {
    width_in_texels: u32,
    height_in_texels: u32,
    depth_in_texels: u32,
}

/// Reads `D3D11_TILE_SHAPE` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tile_shape(
    bytes: &[u8],
) -> direct3d_11::structures::TileShape {
    direct3d_11::structures::TileShape {
        width_in_texels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileShapeLayout,
                width_in_texels
            ),
        ),
        height_in_texels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileShapeLayout,
                height_in_texels
            ),
        ),
        depth_in_texels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileShapeLayout,
                depth_in_texels
            ),
        ),
    }
}

/// `D3D11_SUBRESOURCE_TILING` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11SubresourceTilingLayout {
    width_in_tiles: u32,
    height_in_tiles: u16,
    depth_in_tiles: u16,
    start_tile_index_in_overall_resource: u32,
}

/// `D3D11_TEXTURE2D_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: i32,
    sample_desc: WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
    texture_layout: i32,
}

/// Reads `D3D11_TEXTURE2D_DESC1` from the 48 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::Texture2DDesc1 {
    direct3d_11::structures::Texture2DDesc1 {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                height
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                mip_levels
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                array_size
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                format
            ),
        )),
        sample_desc: decode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                sample_desc
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                    sample_desc
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
        ),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                usage
            ),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                misc_flags
            ),
        ),
        texture_layout: direct3d_11::enumerations::TextureLayout(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                texture_layout
            ),
        )),
    }
}

/// Writes `D3D11_TEXTURE2D_DESC1` as the 48 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc1(
    value: &direct3d_11::structures::Texture2DDesc1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            array_size
        ),
        value.array_size,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            format
        ),
        value.format.0,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_sample_desc(
        &value.sample_desc,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            sample_desc
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
                sample_desc
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiSampleDescLayout>()],
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            usage
        ),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            misc_flags
        ),
        value.misc_flags,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture2dDesc1Layout,
            texture_layout
        ),
        value.texture_layout.0,
    );
}

/// `D3D11_TEXTURE3D_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout {
    width: u32,
    height: u32,
    depth: u32,
    mip_levels: u32,
    format: i32,
    usage: i32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
    texture_layout: i32,
}

/// Reads `D3D11_TEXTURE3D_DESC1` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::Texture3DDesc1 {
    direct3d_11::structures::Texture3DDesc1 {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                height
            ),
        ),
        depth: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                depth
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                mip_levels
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                format
            ),
        )),
        usage: direct3d_11::enumerations::Usage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                usage
            ),
        )),
        bind_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                bind_flags
            ),
        ),
        cpu_access_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                cpu_access_flags
            ),
        ),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                misc_flags
            ),
        ),
        texture_layout: direct3d_11::enumerations::TextureLayout(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
                texture_layout
            ),
        )),
    }
}

/// Writes `D3D11_TEXTURE3D_DESC1` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc1(
    value: &direct3d_11::structures::Texture3DDesc1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            height
        ),
        value.height,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            depth
        ),
        value.depth,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            usage
        ),
        value.usage.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            bind_flags
        ),
        value.bind_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            cpu_access_flags
        ),
        value.cpu_access_flags,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            misc_flags
        ),
        value.misc_flags,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Texture3dDesc1Layout,
            texture_layout
        ),
        value.texture_layout.0,
    );
}

/// `D3D11_RASTERIZER_DESC2` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout {
    fill_mode: i32,
    cull_mode: i32,
    front_counter_clockwise: i32,
    depth_bias: i32,
    depth_bias_clamp: f32,
    slope_scaled_depth_bias: f32,
    depth_clip_enable: i32,
    scissor_enable: i32,
    multisample_enable: i32,
    antialiased_line_enable: i32,
    forced_sample_count: u32,
    conservative_raster: i32,
}

/// Reads `D3D11_RASTERIZER_DESC2` from the 48 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc2(
    bytes: &[u8],
) -> direct3d_11::structures::RasterizerDesc2 {
    direct3d_11::structures::RasterizerDesc2 {
        fill_mode: direct3d_11::enumerations::FillMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                fill_mode
            ),
        )),
        cull_mode: direct3d_11::enumerations::CullMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                cull_mode
            ),
        )),
        front_counter_clockwise: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                front_counter_clockwise
            ),
        ) != 0,
        depth_bias: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                depth_bias
            ),
        ),
        depth_bias_clamp: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                depth_bias_clamp
            ),
        ),
        slope_scaled_depth_bias: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                slope_scaled_depth_bias
            ),
        ),
        depth_clip_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                depth_clip_enable
            ),
        ) != 0,
        scissor_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                scissor_enable
            ),
        ) != 0,
        multisample_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                multisample_enable
            ),
        ) != 0,
        antialiased_line_enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                antialiased_line_enable
            ),
        ) != 0,
        forced_sample_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                forced_sample_count
            ),
        ),
        conservative_raster: direct3d_11::enumerations::ConservativeRasterizationMode(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
                conservative_raster
            ),
        )),
    }
}

/// Writes `D3D11_RASTERIZER_DESC2` as the 48 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc2(
    value: &direct3d_11::structures::RasterizerDesc2,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            fill_mode
        ),
        value.fill_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            cull_mode
        ),
        value.cull_mode.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            front_counter_clockwise
        ),
        i32::from(value.front_counter_clockwise),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            depth_bias
        ),
        value.depth_bias,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            depth_bias_clamp
        ),
        value.depth_bias_clamp,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            slope_scaled_depth_bias
        ),
        value.slope_scaled_depth_bias,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            depth_clip_enable
        ),
        i32::from(value.depth_clip_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            scissor_enable
        ),
        i32::from(value.scissor_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            multisample_enable
        ),
        i32::from(value.multisample_enable),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            antialiased_line_enable
        ),
        i32::from(value.antialiased_line_enable),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            forced_sample_count
        ),
        value.forced_sample_count,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RasterizerDesc2Layout,
            conservative_raster
        ),
        value.conservative_raster.0,
    );
}

/// `D3D11_TEX2D_SRV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout {
    most_detailed_mip: u32,
    mip_levels: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_SRV1` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DSrv1 {
    direct3d_11::structures::Tex2DSrv1 {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
                mip_levels
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_SRV1` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv1(
    value: &direct3d_11::structures::Tex2DSrv1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_SRV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout {
    most_detailed_mip: u32,
    mip_levels: u32,
    first_array_slice: u32,
    array_size: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_SRV1` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArraySrv1 {
    direct3d_11::structures::Tex2DArraySrv1 {
        most_detailed_mip: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
                most_detailed_mip
            ),
        ),
        mip_levels: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
                mip_levels
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
                array_size
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_SRV1` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv1(
    value: &direct3d_11::structures::Tex2DArraySrv1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
            most_detailed_mip
        ),
        value.most_detailed_mip,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
            mip_levels
        ),
        value.mip_levels,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
            array_size
        ),
        value.array_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout,
    texture_2dms: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout,
    texture_cube: WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout,
    texture_cube_array: WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout,
    buffer_ex: WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout,
}

/// Reads `_Anonymous_e__Union` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_shader_resource_view_desc1_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderResourceViewDesc1Anonymous {
    direct3d_11::structures::ShaderResourceViewDesc1Anonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>()],
        ),
        texture_cube: decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_cube
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_cube
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>()],
        ),
        texture_cube_array: decode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                texture_cube_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_cube_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>()],
        ),
        buffer_ex: decode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                buffer_ex
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    buffer_ex
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_shader_resource_view_desc1_anonymous(
    value: &direct3d_11::structures::ShaderResourceViewDesc1Anonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_srv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_srv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_srv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_srv1(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dSrv1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_srv1(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArraySrv1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_srv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_srv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_srv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_srv(
                &value.texture_cube,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_cube
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_cube
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeSrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_texcube_array_srv(
                &value.texture_cube_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    texture_cube_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        texture_cube_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11TexcubeArraySrvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_bufferex_srv(
                &value.buffer_ex,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                    buffer_ex
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                        buffer_ex
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferexSrvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_SHADER_RESOURCE_VIEW_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
}

/// Reads `D3D11_SHADER_RESOURCE_VIEW_DESC1` from the 28 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderResourceViewDesc1 {
    direct3d_11::structures::ShaderResourceViewDesc1 {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
                format
            ),
        )),
        view_dimension: direct3d_common::enumerations::SrvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_shader_resource_view_desc1_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
                    anonymous
                ) + size_of::<
                    WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout,
                >()],
        ),
    }
}

/// Writes `D3D11_SHADER_RESOURCE_VIEW_DESC1` as the 28 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc1(
    value: &direct3d_11::structures::ShaderResourceViewDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_shader_resource_view_desc1_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderResourceViewDesc1Layout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11ShaderResourceViewDesc1AnonymousLayout>(
            )],
    );
}

/// `D3D11_TEX2D_UAV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout {
    mip_slice: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_UAV1` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DUav1 {
    direct3d_11::structures::Tex2DUav1 {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout,
                mip_slice
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_UAV1` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav1(
    value: &direct3d_11::structures::Tex2DUav1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_UAV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_UAV1` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayUav1 {
    direct3d_11::structures::Tex2DArrayUav1 {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
                array_size
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_UAV1` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav1(
    value: &direct3d_11::structures::Tex2DArrayUav1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
            array_size
        ),
        value.array_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_unordered_access_view_desc1_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::UnorderedAccessViewDesc1Anonymous {
    direct3d_11::structures::UnorderedAccessViewDesc1Anonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_unordered_access_view_desc1_anonymous(
    value: &direct3d_11::structures::UnorderedAccessViewDesc1Anonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_uav(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_uav(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_uav(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayUavLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_uav1(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dUav1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_uav1(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayUav1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_uav(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dUavLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_UNORDERED_ACCESS_VIEW_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
}

/// Reads `D3D11_UNORDERED_ACCESS_VIEW_DESC1` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::UnorderedAccessViewDesc1 {
    direct3d_11::structures::UnorderedAccessViewDesc1 {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
                format
            ),
        )),
        view_dimension: direct3d_11::enumerations::UavDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_unordered_access_view_desc1_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
                    anonymous
                ) + size_of::<
                    WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout,
                >()],
        ),
    }
}

/// Writes `D3D11_UNORDERED_ACCESS_VIEW_DESC1` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc1(
    value: &direct3d_11::structures::UnorderedAccessViewDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_unordered_access_view_desc1_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11UnorderedAccessViewDesc1Layout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11UnorderedAccessViewDesc1AnonymousLayout>(
            )],
    );
}

/// `D3D11_TEX2D_RTV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout {
    mip_slice: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_RTV1` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DRtv1 {
    direct3d_11::structures::Tex2DRtv1 {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout,
                mip_slice
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_RTV1` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv1(
    value: &direct3d_11::structures::Tex2DRtv1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_RTV1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
    plane_slice: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_RTV1` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv1(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayRtv1 {
    direct3d_11::structures::Tex2DArrayRtv1 {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
                array_size
            ),
        ),
        plane_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
                plane_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_RTV1` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv1(
    value: &direct3d_11::structures::Tex2DArrayRtv1,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
            array_size
        ),
        value.array_size,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
            plane_slice
        ),
        value.plane_slice,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout {
    buffer: WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout,
    texture_1d: WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout,
    texture_1d_array: WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout,
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout,
    texture_2dms: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout,
    texture_2dms_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout,
    texture_3d: WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_render_target_view_desc1_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetViewDesc1Anonymous {
    direct3d_11::structures::RenderTargetViewDesc1Anonymous {
        buffer: decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                buffer
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    buffer
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>()],
        ),
        texture_1d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_1d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_1d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>()],
        ),
        texture_1d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_1d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_1d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>()],
        ),
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv1(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout>()],
        ),
        texture_2dms: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_2dms
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2dms
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>()],
        ),
        texture_2dms_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_2dms_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2dms_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>()],
        ),
        texture_3d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                texture_3d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_3d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_render_target_view_desc1_anonymous(
    value: &direct3d_11::structures::RenderTargetViewDesc1Anonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_buffer_rtv(
                &value.buffer,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    buffer
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        buffer
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11BufferRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_rtv(
                &value.texture_1d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_1d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_1d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_1d_array_rtv(
                &value.texture_1d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_1d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_1d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex1dArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_rtv1(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dRtv1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_rtv1(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayRtv1Layout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_rtv(
                &value.texture_2dms,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2dms
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_2dms
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2dms_array_rtv(
                &value.texture_2dms_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_2dms_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_2dms_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dmsArrayRtvLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_3d_rtv(
                &value.texture_3d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                    texture_3d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
                        texture_3d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex3dRtvLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_RENDER_TARGET_VIEW_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout {
    format: i32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout,
}

/// Reads `D3D11_RENDER_TARGET_VIEW_DESC1` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::RenderTargetViewDesc1 {
    direct3d_11::structures::RenderTargetViewDesc1 {
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
                format
            ),
        )),
        view_dimension: direct3d_11::enumerations::RtvDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
                view_dimension
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_render_target_view_desc1_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>(
                )],
        ),
    }
}

/// Writes `D3D11_RENDER_TARGET_VIEW_DESC1` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc1(
    value: &direct3d_11::structures::RenderTargetViewDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_render_target_view_desc1_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11RenderTargetViewDesc1Layout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11RenderTargetViewDesc1AnonymousLayout>()],
    );
}

/// `D3D11_QUERY_DESC1` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout {
    query: i32,
    misc_flags: u32,
    context_type: i32,
}

/// Reads `D3D11_QUERY_DESC1` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_query_desc1(
    bytes: &[u8],
) -> direct3d_11::structures::QueryDesc1 {
    direct3d_11::structures::QueryDesc1 {
        query: direct3d_11::enumerations::QueryKind(get_i32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout, query),
        )),
        misc_flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout,
                misc_flags
            ),
        ),
        context_type: direct3d_11::enumerations::ContextType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout,
                context_type
            ),
        )),
    }
}

/// Writes `D3D11_QUERY_DESC1` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_query_desc1(
    value: &direct3d_11::structures::QueryDesc1,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout, query),
        value.query.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout,
            misc_flags
        ),
        value.misc_flags,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11QueryDesc1Layout,
            context_type
        ),
        value.context_type.0,
    );
}

/// `D3D11_BOX` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11BoxLayout {
    left: u32,
    top: u32,
    front: u32,
    right: u32,
    bottom: u32,
    back: u32,
}

/// Reads `D3D11_BOX` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_box(
    bytes: &[u8],
) -> direct3d_11::structures::Box {
    direct3d_11::structures::Box {
        left: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, left),
        ),
        top: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, top),
        ),
        front: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, front),
        ),
        right: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, right),
        ),
        bottom: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, bottom),
        ),
        back: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, back),
        ),
    }
}

/// Writes `D3D11_BOX` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_box(
    value: &direct3d_11::structures::Box,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, left),
        value.left,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, top),
        value.top,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, front),
        value.front,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, right),
        value.right,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, bottom),
        value.bottom,
    );
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11BoxLayout, back),
        value.back,
    );
}

/// `D3D11_MAPPED_SUBRESOURCE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout {
    data: usize,
    row_pitch: u32,
    depth_pitch: u32,
}

/// Reads `D3D11_MAPPED_SUBRESOURCE` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_mapped_subresource(
    bytes: &[u8],
) -> direct3d_11::structures::MappedSubresource<usize> {
    direct3d_11::structures::MappedSubresource::<usize> {
        data: get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
                data
            ),
        ),
        row_pitch: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
                row_pitch
            ),
        ),
        depth_pitch: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
                depth_pitch
            ),
        ),
    }
}

/// Writes `D3D11_MAPPED_SUBRESOURCE` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_mapped_subresource(
    value: &direct3d_11::structures::MappedSubresource<usize>,
    bytes: &mut [u8],
) {
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
            data
        ),
        value.data,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
            row_pitch
        ),
        value.row_pitch,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11MappedSubresourceLayout,
            depth_pitch
        ),
        value.depth_pitch,
    );
}

/// `D3D11_VIEWPORT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ViewportLayout {
    top_left_x: f32,
    top_left_y: f32,
    width: f32,
    height: f32,
    min_depth: f32,
    max_depth: f32,
}

/// Reads `D3D11_VIEWPORT` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_viewport(
    bytes: &[u8],
) -> direct3d_11::structures::Viewport {
    direct3d_11::structures::Viewport {
        top_left_x: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ViewportLayout,
                top_left_x
            ),
        ),
        top_left_y: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ViewportLayout,
                top_left_y
            ),
        ),
        width: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, width),
        ),
        height: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, height),
        ),
        min_depth: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, min_depth),
        ),
        max_depth: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, max_depth),
        ),
    }
}

/// Writes `D3D11_VIEWPORT` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_viewport(
    value: &direct3d_11::structures::Viewport,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ViewportLayout,
            top_left_x
        ),
        value.top_left_x,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ViewportLayout,
            top_left_y
        ),
        value.top_left_y,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, width),
        value.width,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, height),
        value.height,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, min_depth),
        value.min_depth,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11ViewportLayout, max_depth),
        value.max_depth,
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

/// `D3D11_TILED_RESOURCE_COORDINATE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout {
    x: u32,
    y: u32,
    z: u32,
    subresource: u32,
}

/// Reads `D3D11_TILED_RESOURCE_COORDINATE` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tiled_resource_coordinate(
    bytes: &[u8],
) -> direct3d_11::structures::TiledResourceCoordinate {
    direct3d_11::structures::TiledResourceCoordinate {
        x: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
                x
            ),
        ),
        y: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
                y
            ),
        ),
        z: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
                z
            ),
        ),
        subresource: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
                subresource
            ),
        ),
    }
}

/// Writes `D3D11_TILED_RESOURCE_COORDINATE` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tiled_resource_coordinate(
    value: &direct3d_11::structures::TiledResourceCoordinate,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
            x
        ),
        value.x,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
            y
        ),
        value.y,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
            z
        ),
        value.z,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TiledResourceCoordinateLayout,
            subresource
        ),
        value.subresource,
    );
}

/// `D3D11_TILE_REGION_SIZE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout {
    num_tiles: u32,
    b_use_box: i32,
    width: u32,
    height: u16,
    depth: u16,
}

/// Reads `D3D11_TILE_REGION_SIZE` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tile_region_size(
    bytes: &[u8],
) -> direct3d_11::structures::TileRegionSize {
    direct3d_11::structures::TileRegionSize {
        num_tiles: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
                num_tiles
            ),
        ),
        b_use_box: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
                b_use_box
            ),
        ) != 0,
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
                width
            ),
        ),
        height: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
                height
            ),
        ),
        depth: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
                depth
            ),
        ),
    }
}

/// Writes `D3D11_TILE_REGION_SIZE` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tile_region_size(
    value: &direct3d_11::structures::TileRegionSize,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
            num_tiles
        ),
        value.num_tiles,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
            b_use_box
        ),
        i32::from(value.b_use_box),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
            width
        ),
        value.width,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
            height
        ),
        value.height,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TileRegionSizeLayout,
            depth
        ),
        value.depth,
    );
}

/// `D3D11_VERTEX_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout {
    invocation: u64,
}

/// Reads `D3D11_VERTEX_SHADER_TRACE_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_vertex_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VertexShaderTraceDesc {
    direct3d_11::structures::VertexShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout,
                invocation
            ),
        ),
    }
}

/// Writes `D3D11_VERTEX_SHADER_TRACE_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_vertex_shader_trace_desc(
    value: &direct3d_11::structures::VertexShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
}

/// `D3D11_HULL_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout {
    invocation: u64,
}

/// Reads `D3D11_HULL_SHADER_TRACE_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_hull_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::HullShaderTraceDesc {
    direct3d_11::structures::HullShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout,
                invocation
            ),
        ),
    }
}

/// Writes `D3D11_HULL_SHADER_TRACE_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_hull_shader_trace_desc(
    value: &direct3d_11::structures::HullShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
}

/// `D3D11_DOMAIN_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout {
    invocation: u64,
}

/// Reads `D3D11_DOMAIN_SHADER_TRACE_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_domain_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::DomainShaderTraceDesc {
    direct3d_11::structures::DomainShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout,
                invocation
            ),
        ),
    }
}

/// Writes `D3D11_DOMAIN_SHADER_TRACE_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_domain_shader_trace_desc(
    value: &direct3d_11::structures::DomainShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
}

/// `D3D11_GEOMETRY_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout {
    invocation: u64,
}

/// Reads `D3D11_GEOMETRY_SHADER_TRACE_DESC` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_geometry_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::GeometryShaderTraceDesc {
    direct3d_11::structures::GeometryShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout,
                invocation
            ),
        ),
    }
}

/// Writes `D3D11_GEOMETRY_SHADER_TRACE_DESC` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_geometry_shader_trace_desc(
    value: &direct3d_11::structures::GeometryShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
}

/// `D3D11_PIXEL_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout {
    invocation: u64,
    x: i32,
    y: i32,
    sample_mask: u64,
}

/// Reads `D3D11_PIXEL_SHADER_TRACE_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_pixel_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::PixelShaderTraceDesc {
    direct3d_11::structures::PixelShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
                invocation
            ),
        ),
        x: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
                x
            ),
        ),
        y: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
                y
            ),
        ),
        sample_mask: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
                sample_mask
            ),
        ),
    }
}

/// Writes `D3D11_PIXEL_SHADER_TRACE_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_pixel_shader_trace_desc(
    value: &direct3d_11::structures::PixelShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
            x
        ),
        value.x,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
            y
        ),
        value.y,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
            sample_mask
        ),
        value.sample_mask,
    );
}

/// `D3D11_COMPUTE_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout {
    invocation: u64,
    thread_id_in_group: [u32; 3],
    thread_group_id: [u32; 3],
}

/// Reads `D3D11_COMPUTE_SHADER_TRACE_DESC` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_compute_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::ComputeShaderTraceDesc {
    direct3d_11::structures::ComputeShaderTraceDesc {
        invocation: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
                invocation
            ),
        ),
        thread_id_in_group: core::array::from_fn(|index| {
            get_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
                    thread_id_in_group
                ) + index * size_of::<u32>(),
            )
        }),
        thread_group_id: core::array::from_fn(|index| {
            get_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
                    thread_group_id
                ) + index * size_of::<u32>(),
            )
        }),
    }
}

/// Writes `D3D11_COMPUTE_SHADER_TRACE_DESC` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_compute_shader_trace_desc(
    value: &direct3d_11::structures::ComputeShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
            invocation
        ),
        value.invocation,
    );
    for (index, item) in value.thread_id_in_group.iter().enumerate() {
        put_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
                thread_id_in_group
            ) + index * size_of::<u32>(),
            *item,
        );
    }
    for (index, item) in value.thread_group_id.iter().enumerate() {
        put_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
                thread_group_id
            ) + index * size_of::<u32>(),
            *item,
        );
    }
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout {
    vertex_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout,
    hull_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout,
    domain_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout,
    geometry_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout,
    pixel_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout,
    compute_shader_trace_desc: WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout,
}

/// Reads `_Anonymous_e__Union` from the 32 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_shader_trace_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderTraceDescAnonymous {
    direct3d_11::structures::ShaderTraceDescAnonymous {
        vertex_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_vertex_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    vertex_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        vertex_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout>(
                    )],
            ),
        hull_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_hull_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    hull_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        hull_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout>(
                    )],
            ),
        domain_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_domain_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    domain_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        domain_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout>(
                    )],
            ),
        geometry_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_geometry_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    geometry_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        geometry_shader_trace_desc
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout,
                    >()],
            ),
        pixel_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_pixel_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    pixel_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        pixel_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout>(
                    )],
            ),
        compute_shader_trace_desc:
            decode_windows_win32_graphics_direct_3d11_d_3d11_compute_shader_trace_desc(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    compute_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        compute_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout>(
                    )],
            ),
    }
}

/// Writes `_Anonymous_e__Union` as the 32 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_shader_trace_desc_anonymous(
    value: &direct3d_11::structures::ShaderTraceDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_vertex_shader_trace_desc(
                &value.vertex_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    vertex_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        vertex_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout>(
                    )],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout>()
                > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11VertexShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_hull_shader_trace_desc(
                &value.hull_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    hull_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        hull_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout>(
                    )],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11HullShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_domain_shader_trace_desc(
                &value.domain_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    domain_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        domain_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout>(
                    )],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout>()
                > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11DomainShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_geometry_shader_trace_desc(
                &value.geometry_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    geometry_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        geometry_shader_trace_desc
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout,
                    >()],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout>()
                > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11GeometryShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_pixel_shader_trace_desc(
                &value.pixel_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    pixel_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        pixel_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout>(
                    )],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11PixelShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_compute_shader_trace_desc(
                &value.compute_shader_trace_desc,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                    compute_shader_trace_desc
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
                        compute_shader_trace_desc
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout>(
                    )],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout>()
                > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11ComputeShaderTraceDescLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_SHADER_TRACE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout {
    type_: i32,
    flags: u32,
    anonymous: WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout,
}

/// Reads `D3D11_SHADER_TRACE_DESC` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_shader_trace_desc(
    bytes: &[u8],
) -> direct3d_11::structures::ShaderTraceDesc {
    direct3d_11::structures::ShaderTraceDesc {
        type_: direct3d_11::enumerations::ShaderType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
                type_
            ),
        )),
        flags: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
                flags
            ),
        ),
        anonymous: decode_windows_win32_graphics_direct_3d11_shader_trace_desc_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()],
        ),
    }
}

/// Writes `D3D11_SHADER_TRACE_DESC` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_shader_trace_desc(
    value: &direct3d_11::structures::ShaderTraceDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
            type_
        ),
        value.type_.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
            flags
        ),
        value.flags,
    );
    encode_windows_win32_graphics_direct_3d11_shader_trace_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11ShaderTraceDescAnonymousLayout>()],
    );
}

/// `D3D11_TRACE_STATS` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout {
    trace_desc: WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout,
    num_invocations_in_stamp: u8,
    target_stamp_index: u8,
    num_trace_steps: u32,
    input_mask: [u8; 32],
    output_mask: [u8; 32],
    num_temps: u16,
    max_indexable_temp_index: u16,
    indexable_temp_size: [u16; 4096],
    immediate_constant_buffer_size: u16,
    pixel_position: [u32; 8],
    pixel_coverage_mask: [u64; 4],
    pixel_discarded_mask: [u64; 4],
    pixel_coverage_mask_after_shader: [u64; 4],
    pixel_coverage_mask_after_a2_c_sample_mask: [u64; 4],
    pixel_coverage_mask_after_a2_c_sample_mask_depth: [u64; 4],
    pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil: [u64; 4],
    ps_outputs_depth: i32,
    ps_outputs_mask: i32,
    gs_input_primitive: i32,
    gs_inputs_primitive_id: i32,
    hs_output_patch_constant_mask: [u8; 32],
    ds_input_patch_constant_mask: [u8; 32],
}

/// Reads `D3D11_TRACE_STATS` from the 8616 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_trace_stats(
    bytes: &[u8],
) -> direct3d_11::structures::TraceStats {
    direct3d_11::structures::TraceStats {
        trace_desc: decode_windows_win32_graphics_direct_3d11_d_3d11_shader_trace_desc(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                trace_desc
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    trace_desc
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout>()],
        ),
        num_invocations_in_stamp: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                num_invocations_in_stamp
            ),
        ),
        target_stamp_index: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                target_stamp_index
            ),
        ),
        num_trace_steps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                num_trace_steps
            ),
        ),
        input_mask: core::array::from_fn(|index| {
            get_u8(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    input_mask
                ) + index * size_of::<u8>(),
            )
        }),
        output_mask: core::array::from_fn(|index| {
            get_u8(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    output_mask
                ) + index * size_of::<u8>(),
            )
        }),
        num_temps: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                num_temps
            ),
        ),
        max_indexable_temp_index: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                max_indexable_temp_index
            ),
        ),
        indexable_temp_size: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    indexable_temp_size
                ) + index * size_of::<u16>(),
            )
        }),
        immediate_constant_buffer_size: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                immediate_constant_buffer_size
            ),
        ),
        pixel_position: core::array::from_fn(|index| {
            get_u32(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_position
                ) + index * size_of::<u32>(),
            )
        }),
        pixel_coverage_mask: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_coverage_mask
                ) + index * size_of::<u64>(),
            )
        }),
        pixel_discarded_mask: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_discarded_mask
                ) + index * size_of::<u64>(),
            )
        }),
        pixel_coverage_mask_after_shader: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_coverage_mask_after_shader
                ) + index * size_of::<u64>(),
            )
        }),
        pixel_coverage_mask_after_a2_c_sample_mask: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_coverage_mask_after_a2_c_sample_mask
                ) + index * size_of::<u64>(),
            )
        }),
        pixel_coverage_mask_after_a2_c_sample_mask_depth: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_coverage_mask_after_a2_c_sample_mask_depth
                ) + index * size_of::<u64>(),
            )
        }),
        pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil: core::array::from_fn(|index| {
            get_u64(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil
                ) + index * size_of::<u64>(),
            )
        }),
        ps_outputs_depth: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                ps_outputs_depth
            ),
        ) != 0,
        ps_outputs_mask: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                ps_outputs_mask
            ),
        ) != 0,
        gs_input_primitive: direct3d_11::enumerations::TraceGsInputPrimitive(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                gs_input_primitive
            ),
        )),
        gs_inputs_primitive_id: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                gs_inputs_primitive_id
            ),
        ) != 0,
        hs_output_patch_constant_mask: core::array::from_fn(|index| {
            get_u8(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    hs_output_patch_constant_mask
                ) + index * size_of::<u8>(),
            )
        }),
        ds_input_patch_constant_mask: core::array::from_fn(|index| {
            get_u8(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                    ds_input_patch_constant_mask
                ) + index * size_of::<u8>(),
            )
        }),
    }
}

/// Writes `D3D11_TRACE_STATS` as the 8616 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_trace_stats(
    value: &direct3d_11::structures::TraceStats,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_direct_3d11_d_3d11_shader_trace_desc(
        &value.trace_desc,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            trace_desc
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                trace_desc
            ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11ShaderTraceDescLayout>()],
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            num_invocations_in_stamp
        ),
        value.num_invocations_in_stamp,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            target_stamp_index
        ),
        value.target_stamp_index,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            num_trace_steps
        ),
        value.num_trace_steps,
    );
    for (index, item) in value.input_mask.iter().enumerate() {
        put_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                input_mask
            ) + index * size_of::<u8>(),
            *item,
        );
    }
    for (index, item) in value.output_mask.iter().enumerate() {
        put_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                output_mask
            ) + index * size_of::<u8>(),
            *item,
        );
    }
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            num_temps
        ),
        value.num_temps,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            max_indexable_temp_index
        ),
        value.max_indexable_temp_index,
    );
    for (index, item) in value.indexable_temp_size.iter().enumerate() {
        put_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                indexable_temp_size
            ) + index * size_of::<u16>(),
            *item,
        );
    }
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            immediate_constant_buffer_size
        ),
        value.immediate_constant_buffer_size,
    );
    for (index, item) in value.pixel_position.iter().enumerate() {
        put_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_position
            ) + index * size_of::<u32>(),
            *item,
        );
    }
    for (index, item) in value.pixel_coverage_mask.iter().enumerate() {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_coverage_mask
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    for (index, item) in value.pixel_discarded_mask.iter().enumerate() {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_discarded_mask
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    for (index, item) in value.pixel_coverage_mask_after_shader.iter().enumerate() {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_coverage_mask_after_shader
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    for (index, item) in value
        .pixel_coverage_mask_after_a2_c_sample_mask
        .iter()
        .enumerate()
    {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_coverage_mask_after_a2_c_sample_mask
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    for (index, item) in value
        .pixel_coverage_mask_after_a2_c_sample_mask_depth
        .iter()
        .enumerate()
    {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_coverage_mask_after_a2_c_sample_mask_depth
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    for (index, item) in value
        .pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil
        .iter()
        .enumerate()
    {
        put_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil
            ) + index * size_of::<u64>(),
            *item,
        );
    }
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            ps_outputs_depth
        ),
        i32::from(value.ps_outputs_depth),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            ps_outputs_mask
        ),
        i32::from(value.ps_outputs_mask),
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            gs_input_primitive
        ),
        value.gs_input_primitive.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
            gs_inputs_primitive_id
        ),
        i32::from(value.gs_inputs_primitive_id),
    );
    for (index, item) in value.hs_output_patch_constant_mask.iter().enumerate() {
        put_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                hs_output_patch_constant_mask
            ) + index * size_of::<u8>(),
            *item,
        );
    }
    for (index, item) in value.ds_input_patch_constant_mask.iter().enumerate() {
        put_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStatsLayout,
                ds_input_patch_constant_mask
            ) + index * size_of::<u8>(),
            *item,
        );
    }
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout {
    index_1d: u16,
    index_2d: [u16; 2],
}

/// Reads `_Anonymous_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_trace_register_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::TraceRegisterAnonymous {
    direct3d_11::structures::TraceRegisterAnonymous {
        index_1d: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout,
                index_1d
            ),
        ),
        index_2d: core::array::from_fn(|index| {
            get_u16(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout,
                    index_2d
                ) + index * size_of::<u16>(),
            )
        }),
    }
}

/// Writes `_Anonymous_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_trace_register_anonymous(
    value: &direct3d_11::structures::TraceRegisterAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            put_u16(
                bytes,
                offset_of!(
                    WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout,
                    index_1d
                ),
                value.index_1d,
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()]
            && size_of::<u16>() > chosen.0
        {
            chosen = (size_of::<u16>(), scratch);
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            for (index, item) in value.index_2d.iter().enumerate() {
                put_u16(
                    bytes,
                    offset_of!(
                        WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout,
                        index_2d
                    ) + index * size_of::<u16>(),
                    *item,
                );
            }
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()]
            && size_of::<[u16; 2]>() > chosen.0
        {
            chosen = (size_of::<[u16; 2]>(), scratch);
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_TRACE_REGISTER` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout {
    reg_type: i32,
    anonymous: WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout,
    operand_index: u8,
    flags: u8,
}

/// Reads `D3D11_TRACE_REGISTER` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_trace_register(
    bytes: &[u8],
) -> direct3d_11::structures::TraceRegister {
    direct3d_11::structures::TraceRegister {
        reg_type: direct3d_11::enumerations::TraceRegisterType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                reg_type
            ),
        )),
        anonymous: decode_windows_win32_graphics_direct_3d11_trace_register_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()],
        ),
        operand_index: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                operand_index
            ),
        ),
        flags: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                flags
            ),
        ),
    }
}

/// Writes `D3D11_TRACE_REGISTER` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_trace_register(
    value: &direct3d_11::structures::TraceRegister,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
            reg_type
        ),
        value.reg_type.0,
    );
    encode_windows_win32_graphics_direct_3d11_trace_register_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11TraceRegisterAnonymousLayout>()],
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
            operand_index
        ),
        value.operand_index,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceRegisterLayout,
            flags
        ),
        value.flags,
    );
}

/// `D3D11_TRACE_VALUE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TraceValueLayout {
    bits: [u32; 4],
    valid_mask: u8,
}

/// Reads `D3D11_TRACE_VALUE` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_trace_value(
    bytes: &[u8],
) -> direct3d_11::structures::TraceValue {
    direct3d_11::structures::TraceValue {
        bits: core::array::from_fn(|index| {
            get_u32(
                bytes,
                offset_of!(WindowsWin32GraphicsDirect3d11D3d11TraceValueLayout, bits)
                    + index * size_of::<u32>(),
            )
        }),
        valid_mask: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceValueLayout,
                valid_mask
            ),
        ),
    }
}

/// Writes `D3D11_TRACE_VALUE` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_trace_value(
    value: &direct3d_11::structures::TraceValue,
    bytes: &mut [u8],
) {
    for (index, item) in value.bits.iter().enumerate() {
        put_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11TraceValueLayout, bits)
                + index * size_of::<u32>(),
            *item,
        );
    }
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceValueLayout,
            valid_mask
        ),
        value.valid_mask,
    );
}

/// `D3D11_TRACE_STEP` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout {
    id: u32,
    instruction_active: i32,
    num_registers_written: u8,
    num_registers_read: u8,
    misc_operations: u16,
    opcode_type: u32,
    current_global_cycle: u64,
}

/// Reads `D3D11_TRACE_STEP` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_trace_step(
    bytes: &[u8],
) -> direct3d_11::structures::TraceStep {
    direct3d_11::structures::TraceStep {
        id: get_u32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout, id),
        ),
        instruction_active: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                instruction_active
            ),
        ) != 0,
        num_registers_written: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                num_registers_written
            ),
        ),
        num_registers_read: get_u8(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                num_registers_read
            ),
        ),
        misc_operations: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                misc_operations
            ),
        ),
        opcode_type: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                opcode_type
            ),
        ),
        current_global_cycle: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
                current_global_cycle
            ),
        ),
    }
}

/// Writes `D3D11_TRACE_STEP` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_trace_step(
    value: &direct3d_11::structures::TraceStep,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout, id),
        value.id,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            instruction_active
        ),
        i32::from(value.instruction_active),
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            num_registers_written
        ),
        value.num_registers_written,
    );
    put_u8(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            num_registers_read
        ),
        value.num_registers_read,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            misc_operations
        ),
        value.misc_operations,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            opcode_type
        ),
        value.opcode_type,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11TraceStepLayout,
            current_global_cycle
        ),
        value.current_global_cycle,
    );
}

/// `D3D11_VIDEO_COLOR_YCbCrA` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout {
    y: f32,
    cb: f32,
    cr: f32,
    a: f32,
}

/// Reads `D3D11_VIDEO_COLOR_YCbCrA` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_color_y_cb_cr_a(
    bytes: &[u8],
) -> direct3d_11::structures::VideoColorYcbcra {
    direct3d_11::structures::VideoColorYcbcra {
        y: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout, y),
        ),
        cb: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout,
                cb
            ),
        ),
        cr: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout,
                cr
            ),
        ),
        a: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout, a),
        ),
    }
}

/// Writes `D3D11_VIDEO_COLOR_YCbCrA` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_color_y_cb_cr_a(
    value: &direct3d_11::structures::VideoColorYcbcra,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout, y),
        value.y,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout,
            cb
        ),
        value.cb,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout,
            cr
        ),
        value.cr,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout, a),
        value.a,
    );
}

/// `D3D11_VIDEO_COLOR_RGBA` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

/// Reads `D3D11_VIDEO_COLOR_RGBA` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_color_rgba(
    bytes: &[u8],
) -> direct3d_11::structures::VideoColorRgba {
    direct3d_11::structures::VideoColorRgba {
        r: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, r),
        ),
        g: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, g),
        ),
        b: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, b),
        ),
        a: get_f32(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, a),
        ),
    }
}

/// Writes `D3D11_VIDEO_COLOR_RGBA` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_color_rgba(
    value: &direct3d_11::structures::VideoColorRgba,
    bytes: &mut [u8],
) {
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, r),
        value.r,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, g),
        value.g,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, b),
        value.b,
    );
    put_f32(
        bytes,
        offset_of!(WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout, a),
        value.a,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout {
    y_cb_cr: WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout,
    rgba: WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout,
}

/// Reads `_Anonymous_e__Union` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_video_color_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::VideoColorAnonymous {
    direct3d_11::structures::VideoColorAnonymous {
        y_cb_cr: decode_windows_win32_graphics_direct_3d11_d_3d11_video_color_y_cb_cr_a(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                y_cb_cr
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                    y_cb_cr
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout>()],
        ),
        rgba: decode_windows_win32_graphics_direct_3d11_d_3d11_video_color_rgba(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                rgba
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                    rgba
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_video_color_anonymous(
    value: &direct3d_11::structures::VideoColorAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()],
    );
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_video_color_y_cb_cr_a(
                &value.y_cb_cr,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                    y_cb_cr
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                        y_cb_cr
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout>()],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorYCbCrALayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch =
            [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_video_color_rgba(
                &value.rgba,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                    rgba
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
                        rgba
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout>()],
            );
        }
        if scratch
            != [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11VideoColorRgbaLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_VIDEO_COLOR` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoColorLayout {
    anonymous: WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout,
}

/// Reads `D3D11_VIDEO_COLOR` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_color(
    bytes: &[u8],
) -> direct3d_11::structures::VideoColor {
    direct3d_11::structures::VideoColor {
        anonymous: decode_windows_win32_graphics_direct_3d11_video_color_anonymous(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoColorLayout,
                anonymous
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoColorLayout,
                    anonymous
                ) + size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()],
        ),
    }
}

/// Writes `D3D11_VIDEO_COLOR` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_color(
    value: &direct3d_11::structures::VideoColor,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_direct_3d11_video_color_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoColorLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoColorLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11VideoColorAnonymousLayout>()],
    );
}

/// `D3D11_VIDEO_PROCESSOR_COLOR_SPACE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorColorSpaceLayout {
    _bitfield: u32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_COLOR_SPACE` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_color_space(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorColorSpace {
    direct3d_11::structures::VideoProcessorColorSpace {
        _bitfield: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorColorSpaceLayout,
                _bitfield
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_COLOR_SPACE` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_color_space(
    value: &direct3d_11::structures::VideoProcessorColorSpace,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorColorSpaceLayout,
            _bitfield
        ),
        value._bitfield,
    );
}

/// `SIZE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32FoundationSizeLayout {
    cx: i32,
    cy: i32,
}

/// Reads `SIZE` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_foundation_size(
    bytes: &[u8],
) -> component_object_model::structures::Size {
    component_object_model::structures::Size {
        cx: get_i32(bytes, offset_of!(WindowsWin32FoundationSizeLayout, cx)),
        cy: get_i32(bytes, offset_of!(WindowsWin32FoundationSizeLayout, cy)),
    }
}

/// Writes `SIZE` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_foundation_size(
    value: &component_object_model::structures::Size,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationSizeLayout, cx),
        value.cx,
    );
    put_i32(
        bytes,
        offset_of!(WindowsWin32FoundationSizeLayout, cy),
        value.cy,
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

/// `D3D11_ENCRYPTED_BLOCK_INFO` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout {
    num_encrypted_bytes_at_beginning: u32,
    num_bytes_in_skip_pattern: u32,
    num_bytes_in_encrypt_pattern: u32,
}

/// Reads `D3D11_ENCRYPTED_BLOCK_INFO` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_encrypted_block_info(
    bytes: &[u8],
) -> direct3d_11::structures::EncryptedBlockInfo {
    direct3d_11::structures::EncryptedBlockInfo {
        num_encrypted_bytes_at_beginning: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
                num_encrypted_bytes_at_beginning
            ),
        ),
        num_bytes_in_skip_pattern: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
                num_bytes_in_skip_pattern
            ),
        ),
        num_bytes_in_encrypt_pattern: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
                num_bytes_in_encrypt_pattern
            ),
        ),
    }
}

/// Writes `D3D11_ENCRYPTED_BLOCK_INFO` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_encrypted_block_info(
    value: &direct3d_11::structures::EncryptedBlockInfo,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
            num_encrypted_bytes_at_beginning
        ),
        value.num_encrypted_bytes_at_beginning,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
            num_bytes_in_skip_pattern
        ),
        value.num_bytes_in_skip_pattern,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11EncryptedBlockInfoLayout,
            num_bytes_in_encrypt_pattern
        ),
        value.num_bytes_in_encrypt_pattern,
    );
}

/// `D3D11_OMAC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11OmacLayout {
    omac: [u8; 16],
}

/// Reads `D3D11_OMAC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_omac(
    bytes: &[u8],
) -> direct3d_11::structures::Omac {
    direct3d_11::structures::Omac {
        omac: core::array::from_fn(|index| {
            get_u8(
                bytes,
                offset_of!(WindowsWin32GraphicsDirect3d11D3d11OmacLayout, omac)
                    + index * size_of::<u8>(),
            )
        }),
    }
}

/// Writes `D3D11_OMAC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_omac(
    value: &direct3d_11::structures::Omac,
    bytes: &mut [u8],
) {
    for (index, item) in value.omac.iter().enumerate() {
        put_u8(
            bytes,
            offset_of!(WindowsWin32GraphicsDirect3d11D3d11OmacLayout, omac)
                + index * size_of::<u8>(),
            *item,
        );
    }
}

/// `D3D11_AUTHENTICATED_CONFIGURE_OUTPUT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout {
    omac: WindowsWin32GraphicsDirect3d11D3d11OmacLayout,
    configure_type: GuidLayout,
    h_channel: usize,
    sequence_number: u32,
    return_code: i32,
}

/// Reads `D3D11_AUTHENTICATED_CONFIGURE_OUTPUT` from the 48 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_authenticated_configure_output(
    bytes: &[u8],
) -> direct3d_11::structures::AuthenticatedConfigureOutput {
    direct3d_11::structures::AuthenticatedConfigureOutput {
        omac: decode_windows_win32_graphics_direct_3d11_d_3d11_omac(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                omac
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                    omac
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11OmacLayout>()],
        ),
        configure_type: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                configure_type
            ),
        ),
        h_channel: component_object_model::structures::Handle(get_usize(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                h_channel
            ),
        )),
        sequence_number: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                sequence_number
            ),
        ),
        return_code: component_object_model::structures::HResult(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                return_code
            ),
        )),
    }
}

/// Writes `D3D11_AUTHENTICATED_CONFIGURE_OUTPUT` as the 48 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_authenticated_configure_output(
    value: &direct3d_11::structures::AuthenticatedConfigureOutput,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_direct_3d11_d_3d11_omac(
        &value.omac,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
            omac
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
                omac
            ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11OmacLayout>()],
    );
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
            configure_type
        ),
        &value.configure_type,
    );
    put_usize(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
            h_channel
        ),
        value.h_channel.0,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
            sequence_number
        ),
        value.sequence_number,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11AuthenticatedConfigureOutputLayout,
            return_code
        ),
        value.return_code.0,
    );
}

/// `D3D11_VIDEO_SAMPLE_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout {
    width: u32,
    height: u32,
    format: i32,
    color_space: i32,
}

/// Reads `D3D11_VIDEO_SAMPLE_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_sample_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoSampleDesc {
    direct3d_11::structures::VideoSampleDesc {
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
                height
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
                format
            ),
        )),
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
                color_space
            ),
        )),
    }
}

/// Writes `D3D11_VIDEO_SAMPLE_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_sample_desc(
    value: &direct3d_11::structures::VideoSampleDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
            height
        ),
        value.height,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
            format
        ),
        value.format.0,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoSampleDescLayout,
            color_space
        ),
        value.color_space.0,
    );
}

/// `D3D11_VIDEO_PROCESSOR_STREAM_BEHAVIOR_HINT` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout {
    enable: i32,
    width: u32,
    height: u32,
    format: i32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_STREAM_BEHAVIOR_HINT` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_stream_behavior_hint(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorStreamBehaviorHint {
    direct3d_11::structures::VideoProcessorStreamBehaviorHint {
        enable: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
                enable
            ),
        ) != 0,
        width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
                width
            ),
        ),
        height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
                height
            ),
        ),
        format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
                format
            ),
        )),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_STREAM_BEHAVIOR_HINT` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_stream_behavior_hint(
    value: &direct3d_11::structures::VideoProcessorStreamBehaviorHint,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
            enable
        ),
        i32::from(value.enable),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
            width
        ),
        value.width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
            height
        ),
        value.height,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorStreamBehaviorHintLayout,
            format
        ),
        value.format.0,
    );
}

/// `D3D11_VIDEO_DECODER_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout {
    guid: GuidLayout,
    sample_width: u32,
    sample_height: u32,
    output_format: i32,
}

/// Reads `D3D11_VIDEO_DECODER_DESC` from the 28 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoDecoderDesc {
    direct3d_11::structures::VideoDecoderDesc {
        guid: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
                guid
            ),
        ),
        sample_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
                sample_width
            ),
        ),
        sample_height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
                sample_height
            ),
        ),
        output_format: directx_graphics_infrastructure::enumerations::Format(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
                output_format
            ),
        )),
    }
}

/// Writes `D3D11_VIDEO_DECODER_DESC` as the 28 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_desc(
    value: &direct3d_11::structures::VideoDecoderDesc,
    bytes: &mut [u8],
) {
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
            guid
        ),
        &value.guid,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
            sample_width
        ),
        value.sample_width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
            sample_height
        ),
        value.sample_height,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderDescLayout,
            output_format
        ),
        value.output_format.0,
    );
}

/// `D3D11_VIDEO_DECODER_CONFIG` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout {
    guid_config_bitstream_encryption: GuidLayout,
    guid_config_m_bcontrol_encryption: GuidLayout,
    guid_config_resid_diff_encryption: GuidLayout,
    config_bitstream_raw: u32,
    config_m_bcontrol_raster_order: u32,
    config_resid_diff_host: u32,
    config_spatial_resid8: u32,
    config_resid8_subtraction: u32,
    config_spatial_host8or9_clipping: u32,
    config_spatial_resid_interleaved: u32,
    config_intra_resid_unsigned: u32,
    config_resid_diff_accelerator: u32,
    config_host_inverse_scan: u32,
    config_specific_idct: u32,
    config4_grouped_coefs: u32,
    config_min_render_target_buff_count: u16,
    config_decoder_specific: u16,
}

/// Reads `D3D11_VIDEO_DECODER_CONFIG` from the 100 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_config(
    bytes: &[u8],
) -> direct3d_11::structures::VideoDecoderConfig {
    direct3d_11::structures::VideoDecoderConfig {
        guid_config_bitstream_encryption: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                guid_config_bitstream_encryption
            ),
        ),
        guid_config_m_bcontrol_encryption: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                guid_config_m_bcontrol_encryption
            ),
        ),
        guid_config_resid_diff_encryption: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                guid_config_resid_diff_encryption
            ),
        ),
        config_bitstream_raw: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_bitstream_raw
            ),
        ),
        config_m_bcontrol_raster_order: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_m_bcontrol_raster_order
            ),
        ),
        config_resid_diff_host: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_resid_diff_host
            ),
        ),
        config_spatial_resid8: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_spatial_resid8
            ),
        ),
        config_resid8_subtraction: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_resid8_subtraction
            ),
        ),
        config_spatial_host8or9_clipping: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_spatial_host8or9_clipping
            ),
        ),
        config_spatial_resid_interleaved: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_spatial_resid_interleaved
            ),
        ),
        config_intra_resid_unsigned: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_intra_resid_unsigned
            ),
        ),
        config_resid_diff_accelerator: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_resid_diff_accelerator
            ),
        ),
        config_host_inverse_scan: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_host_inverse_scan
            ),
        ),
        config_specific_idct: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_specific_idct
            ),
        ),
        config4_grouped_coefs: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config4_grouped_coefs
            ),
        ),
        config_min_render_target_buff_count: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_min_render_target_buff_count
            ),
        ),
        config_decoder_specific: get_u16(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
                config_decoder_specific
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_DECODER_CONFIG` as the 100 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_config(
    value: &direct3d_11::structures::VideoDecoderConfig,
    bytes: &mut [u8],
) {
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            guid_config_bitstream_encryption
        ),
        &value.guid_config_bitstream_encryption,
    );
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            guid_config_m_bcontrol_encryption
        ),
        &value.guid_config_m_bcontrol_encryption,
    );
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            guid_config_resid_diff_encryption
        ),
        &value.guid_config_resid_diff_encryption,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_bitstream_raw
        ),
        value.config_bitstream_raw,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_m_bcontrol_raster_order
        ),
        value.config_m_bcontrol_raster_order,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_resid_diff_host
        ),
        value.config_resid_diff_host,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_spatial_resid8
        ),
        value.config_spatial_resid8,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_resid8_subtraction
        ),
        value.config_resid8_subtraction,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_spatial_host8or9_clipping
        ),
        value.config_spatial_host8or9_clipping,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_spatial_resid_interleaved
        ),
        value.config_spatial_resid_interleaved,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_intra_resid_unsigned
        ),
        value.config_intra_resid_unsigned,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_resid_diff_accelerator
        ),
        value.config_resid_diff_accelerator,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_host_inverse_scan
        ),
        value.config_host_inverse_scan,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_specific_idct
        ),
        value.config_specific_idct,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config4_grouped_coefs
        ),
        value.config4_grouped_coefs,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_min_render_target_buff_count
        ),
        value.config_min_render_target_buff_count,
    );
    put_u16(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderConfigLayout,
            config_decoder_specific
        ),
        value.config_decoder_specific,
    );
}

/// `D3D11_TEX2D_VDOV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout {
    array_slice: u32,
}

/// Reads `D3D11_TEX2D_VDOV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vdov(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DVdov {
    direct3d_11::structures::Tex2DVdov {
        array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout,
                array_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_VDOV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vdov(
    value: &direct3d_11::structures::Tex2DVdov,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout,
            array_slice
        ),
        value.array_slice,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout {
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout,
}

/// Reads `_Anonymous_e__Union` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_video_decoder_output_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::VideoDecoderOutputViewDescAnonymous {
    direct3d_11::structures::VideoDecoderOutputViewDescAnonymous {
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vdov(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_video_decoder_output_view_desc_anonymous(
    value: &direct3d_11::structures::VideoDecoderOutputViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vdov(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout>()]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVdovLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_VIDEO_DECODER_OUTPUT_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout {
    decode_profile: GuidLayout,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
}

/// Reads `D3D11_VIDEO_DECODER_OUTPUT_VIEW_DESC` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_output_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoDecoderOutputViewDesc {
    direct3d_11::structures::VideoDecoderOutputViewDesc {
        decode_profile: get_guid(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
                decode_profile
            ),
        ),
        view_dimension: direct3d_11::enumerations::VdovDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
                view_dimension
            ),
        )),
        anonymous:
            decode_windows_win32_graphics_direct_3d11_video_decoder_output_view_desc_anonymous(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
                    anonymous
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
                        anonymous
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout,
                    >()],
            ),
    }
}

/// Writes `D3D11_VIDEO_DECODER_OUTPUT_VIEW_DESC` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_output_view_desc(
    value: &direct3d_11::structures::VideoDecoderOutputViewDesc,
    bytes: &mut [u8],
) {
    put_guid(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
            decode_profile
        ),
        &value.decode_profile,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_video_decoder_output_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoDecoderOutputViewDescLayout,
                anonymous
            ) + size_of::<WindowsWin32GraphicsDirect3d11VideoDecoderOutputViewDescAnonymousLayout>(
            )],
    );
}

/// `D3D11_TEX2D_VPIV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout {
    mip_slice: u32,
    array_slice: u32,
}

/// Reads `D3D11_TEX2D_VPIV` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpiv(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DVpiv {
    direct3d_11::structures::Tex2DVpiv {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout,
                mip_slice
            ),
        ),
        array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout,
                array_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_VPIV` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpiv(
    value: &direct3d_11::structures::Tex2DVpiv,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout,
            array_slice
        ),
        value.array_slice,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout {
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout,
}

/// Reads `_Anonymous_e__Union` from the 8 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_video_processor_input_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorInputViewDescAnonymous {
    direct3d_11::structures::VideoProcessorInputViewDescAnonymous {
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpiv(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 8 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_video_processor_input_view_desc_anonymous(
    value: &direct3d_11::structures::VideoProcessorInputViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpiv(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout>(
                )]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpivLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout {
    four_cc: u32,
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
}

/// Reads `D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_input_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorInputViewDesc {
    direct3d_11::structures::VideoProcessorInputViewDesc {
        four_cc: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
                four_cc
            ),
        ),
        view_dimension: direct3d_11::enumerations::VpivDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
                view_dimension
            ),
        )),
        anonymous:
            decode_windows_win32_graphics_direct_3d11_video_processor_input_view_desc_anonymous(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
                    anonymous
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
                        anonymous
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
                    >()],
            ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_input_view_desc(
    value: &direct3d_11::structures::VideoProcessorInputViewDesc,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
            four_cc
        ),
        value.four_cc,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_video_processor_input_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorInputViewDescLayout,
                anonymous
            ) + size_of::<
                WindowsWin32GraphicsDirect3d11VideoProcessorInputViewDescAnonymousLayout,
            >()],
    );
}

/// `D3D11_TEX2D_VPOV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout {
    mip_slice: u32,
}

/// Reads `D3D11_TEX2D_VPOV` from the 4 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpov(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DVpov {
    direct3d_11::structures::Tex2DVpov {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout,
                mip_slice
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_VPOV` as the 4 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpov(
    value: &direct3d_11::structures::Tex2DVpov,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout,
            mip_slice
        ),
        value.mip_slice,
    );
}

/// `D3D11_TEX2D_ARRAY_VPOV` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout {
    mip_slice: u32,
    first_array_slice: u32,
    array_size: u32,
}

/// Reads `D3D11_TEX2D_ARRAY_VPOV` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_vpov(
    bytes: &[u8],
) -> direct3d_11::structures::Tex2DArrayVpov {
    direct3d_11::structures::Tex2DArrayVpov {
        mip_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
                mip_slice
            ),
        ),
        first_array_slice: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
                first_array_slice
            ),
        ),
        array_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
                array_size
            ),
        ),
    }
}

/// Writes `D3D11_TEX2D_ARRAY_VPOV` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_vpov(
    value: &direct3d_11::structures::Tex2DArrayVpov,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
            mip_slice
        ),
        value.mip_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
            first_array_slice
        ),
        value.first_array_slice,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
            array_size
        ),
        value.array_size,
    );
}

/// `_Anonymous_e__Union` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout {
    texture_2d: WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout,
    texture_2d_array: WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout,
}

/// Reads `_Anonymous_e__Union` from the 12 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_video_processor_output_view_desc_anonymous(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorOutputViewDescAnonymous {
    direct3d_11::structures::VideoProcessorOutputViewDescAnonymous {
        texture_2d: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpov(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                texture_2d
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                    texture_2d
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout>()],
        ),
        texture_2d_array: decode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_vpov(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                texture_2d_array
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                    texture_2d_array
                ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout>()],
        ),
    }
}

/// Writes `_Anonymous_e__Union` as the 12 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_video_processor_output_view_desc_anonymous(
    value: &direct3d_11::structures::VideoProcessorOutputViewDescAnonymous,
    bytes: &mut [u8],
) {
    let mut chosen = (
        usize::MIN,
        [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>()],
    );
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_vpov(
                &value.texture_2d,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                    texture_2d
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                        texture_2d
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>(
                )]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dVpovLayout>(),
                scratch,
            );
        }
    }
    {
        let mut scratch = [u8::MIN;
            size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>()];
        {
            let bytes = &mut scratch[..];
            encode_windows_win32_graphics_direct_3d11_d_3d11_tex_2d_array_vpov(
                &value.texture_2d_array,
                &mut bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                    texture_2d_array
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                        texture_2d_array
                    ) + size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout>()],
            );
        }
        if scratch
            != [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>(
                )]
            && size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout>() > chosen.0
        {
            chosen = (
                size_of::<WindowsWin32GraphicsDirect3d11D3d11Tex2dArrayVpovLayout>(),
                scratch,
            );
        }
    }
    bytes[..size_of::<WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout>()]
        .copy_from_slice(&chosen.1);
}

/// `D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout {
    view_dimension: i32,
    anonymous: WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
}

/// Reads `D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_output_view_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorOutputViewDesc {
    direct3d_11::structures::VideoProcessorOutputViewDesc {
        view_dimension: direct3d_11::enumerations::VpovDimension(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
                view_dimension
            ),
        )),
        anonymous:
            decode_windows_win32_graphics_direct_3d11_video_processor_output_view_desc_anonymous(
                &bytes[offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
                    anonymous
                )
                    ..offset_of!(
                        WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
                        anonymous
                    ) + size_of::<
                        WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
                    >()],
            ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_output_view_desc(
    value: &direct3d_11::structures::VideoProcessorOutputViewDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
            view_dimension
        ),
        value.view_dimension.0,
    );
    encode_windows_win32_graphics_direct_3d11_video_processor_output_view_desc_anonymous(
        &value.anonymous,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
            anonymous
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorOutputViewDescLayout,
                anonymous
            ) + size_of::<
                WindowsWin32GraphicsDirect3d11VideoProcessorOutputViewDescAnonymousLayout,
            >()],
    );
}

/// `D3D11_VIDEO_PROCESSOR_CONTENT_DESC` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout {
    input_frame_format: i32,
    input_frame_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    input_width: u32,
    input_height: u32,
    output_frame_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    output_width: u32,
    output_height: u32,
    usage: i32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_CONTENT_DESC` from the 40 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_content_desc(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorContentDesc {
    direct3d_11::structures::VideoProcessorContentDesc {
        input_frame_format: direct3d_11::enumerations::VideoFrameFormat(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                input_frame_format
            ),
        )),
        input_frame_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                input_frame_rate
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                    input_frame_rate
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        input_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                input_width
            ),
        ),
        input_height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                input_height
            ),
        ),
        output_frame_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                output_frame_rate
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                    output_frame_rate
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        output_width: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                output_width
            ),
        ),
        output_height: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                output_height
            ),
        ),
        usage: direct3d_11::enumerations::VideoUsage(get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                usage
            ),
        )),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_CONTENT_DESC` as the 40 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_content_desc(
    value: &direct3d_11::structures::VideoProcessorContentDesc,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            input_frame_format
        ),
        value.input_frame_format.0,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.input_frame_rate,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            input_frame_rate
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                input_frame_rate
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            input_width
        ),
        value.input_width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            input_height
        ),
        value.input_height,
    );
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.output_frame_rate,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            output_frame_rate
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
                output_frame_rate
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            output_width
        ),
        value.output_width,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            output_height
        ),
        value.output_height,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorContentDescLayout,
            usage
        ),
        value.usage.0,
    );
}

/// `D3D11_VIDEO_CONTENT_PROTECTION_CAPS` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout {
    caps: u32,
    key_exchange_type_count: u32,
    block_alignment_size: u32,
    protected_memory_size: u64,
}

/// Reads `D3D11_VIDEO_CONTENT_PROTECTION_CAPS` from the 24 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_content_protection_caps(
    bytes: &[u8],
) -> direct3d_11::structures::VideoContentProtectionCaps {
    direct3d_11::structures::VideoContentProtectionCaps {
        caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
                caps
            ),
        ),
        key_exchange_type_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
                key_exchange_type_count
            ),
        ),
        block_alignment_size: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
                block_alignment_size
            ),
        ),
        protected_memory_size: get_u64(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
                protected_memory_size
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_CONTENT_PROTECTION_CAPS` as the 24 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_content_protection_caps(
    value: &direct3d_11::structures::VideoContentProtectionCaps,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
            caps
        ),
        value.caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
            key_exchange_type_count
        ),
        value.key_exchange_type_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
            block_alignment_size
        ),
        value.block_alignment_size,
    );
    put_u64(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoContentProtectionCapsLayout,
            protected_memory_size
        ),
        value.protected_memory_size,
    );
}

/// `D3D11_VIDEO_PROCESSOR_RATE_CONVERSION_CAPS` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout {
    past_frames: u32,
    future_frames: u32,
    processor_caps: u32,
    i_telecine_caps: u32,
    custom_rate_count: u32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_RATE_CONVERSION_CAPS` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_rate_conversion_caps(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorRateConversionCaps {
    direct3d_11::structures::VideoProcessorRateConversionCaps {
        past_frames: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
                past_frames
            ),
        ),
        future_frames: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
                future_frames
            ),
        ),
        processor_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
                processor_caps
            ),
        ),
        i_telecine_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
                i_telecine_caps
            ),
        ),
        custom_rate_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
                custom_rate_count
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_RATE_CONVERSION_CAPS` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_rate_conversion_caps(
    value: &direct3d_11::structures::VideoProcessorRateConversionCaps,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
            past_frames
        ),
        value.past_frames,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
            future_frames
        ),
        value.future_frames,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
            processor_caps
        ),
        value.processor_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
            i_telecine_caps
        ),
        value.i_telecine_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorRateConversionCapsLayout,
            custom_rate_count
        ),
        value.custom_rate_count,
    );
}

/// `D3D11_VIDEO_PROCESSOR_CAPS` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout {
    device_caps: u32,
    feature_caps: u32,
    filter_caps: u32,
    input_format_caps: u32,
    auto_stream_caps: u32,
    stereo_caps: u32,
    rate_conversion_caps_count: u32,
    max_input_streams: u32,
    max_stream_states: u32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_CAPS` from the 36 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_caps(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorCaps {
    direct3d_11::structures::VideoProcessorCaps {
        device_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                device_caps
            ),
        ),
        feature_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                feature_caps
            ),
        ),
        filter_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                filter_caps
            ),
        ),
        input_format_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                input_format_caps
            ),
        ),
        auto_stream_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                auto_stream_caps
            ),
        ),
        stereo_caps: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                stereo_caps
            ),
        ),
        rate_conversion_caps_count: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                rate_conversion_caps_count
            ),
        ),
        max_input_streams: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                max_input_streams
            ),
        ),
        max_stream_states: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
                max_stream_states
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_CAPS` as the 36 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_caps(
    value: &direct3d_11::structures::VideoProcessorCaps,
    bytes: &mut [u8],
) {
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            device_caps
        ),
        value.device_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            feature_caps
        ),
        value.feature_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            filter_caps
        ),
        value.filter_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            input_format_caps
        ),
        value.input_format_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            auto_stream_caps
        ),
        value.auto_stream_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            stereo_caps
        ),
        value.stereo_caps,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            rate_conversion_caps_count
        ),
        value.rate_conversion_caps_count,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            max_input_streams
        ),
        value.max_input_streams,
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCapsLayout,
            max_stream_states
        ),
        value.max_stream_states,
    );
}

/// `D3D11_VIDEO_PROCESSOR_CUSTOM_RATE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout {
    custom_rate: WindowsWin32GraphicsDxgiCommonDxgiRationalLayout,
    output_frames: u32,
    input_interlaced: i32,
    input_frames_or_fields: u32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_CUSTOM_RATE` from the 20 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_custom_rate(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorCustomRate {
    direct3d_11::structures::VideoProcessorCustomRate {
        custom_rate: decode_windows_win32_graphics_dxgi_common_dxgi_rational(
            &bytes[offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                custom_rate
            )
                ..offset_of!(
                    WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                    custom_rate
                ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
        ),
        output_frames: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                output_frames
            ),
        ),
        input_interlaced: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                input_interlaced
            ),
        ) != 0,
        input_frames_or_fields: get_u32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                input_frames_or_fields
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_CUSTOM_RATE` as the 20 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_custom_rate(
    value: &direct3d_11::structures::VideoProcessorCustomRate,
    bytes: &mut [u8],
) {
    encode_windows_win32_graphics_dxgi_common_dxgi_rational(
        &value.custom_rate,
        &mut bytes[offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
            custom_rate
        )
            ..offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
                custom_rate
            ) + size_of::<WindowsWin32GraphicsDxgiCommonDxgiRationalLayout>()],
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
            output_frames
        ),
        value.output_frames,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
            input_interlaced
        ),
        i32::from(value.input_interlaced),
    );
    put_u32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorCustomRateLayout,
            input_frames_or_fields
        ),
        value.input_frames_or_fields,
    );
}

/// `D3D11_VIDEO_PROCESSOR_FILTER_RANGE` as C lays it out: its offsets and size are this type's.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout {
    minimum: i32,
    maximum: i32,
    default: i32,
    multiplier: f32,
}

/// Reads `D3D11_VIDEO_PROCESSOR_FILTER_RANGE` from the 16 bytes C lays it out in.
pub(crate) fn decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_filter_range(
    bytes: &[u8],
) -> direct3d_11::structures::VideoProcessorFilterRange {
    direct3d_11::structures::VideoProcessorFilterRange {
        minimum: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
                minimum
            ),
        ),
        maximum: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
                maximum
            ),
        ),
        default: get_i32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
                default
            ),
        ),
        multiplier: get_f32(
            bytes,
            offset_of!(
                WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
                multiplier
            ),
        ),
    }
}

/// Writes `D3D11_VIDEO_PROCESSOR_FILTER_RANGE` as the 16 bytes C lays it out in.
pub(crate) fn encode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_filter_range(
    value: &direct3d_11::structures::VideoProcessorFilterRange,
    bytes: &mut [u8],
) {
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
            minimum
        ),
        value.minimum,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
            maximum
        ),
        value.maximum,
    );
    put_i32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
            default
        ),
        value.default,
    );
    put_f32(
        bytes,
        offset_of!(
            WindowsWin32GraphicsDirect3d11D3d11VideoProcessorFilterRangeLayout,
            multiplier
        ),
        value.multiplier,
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
