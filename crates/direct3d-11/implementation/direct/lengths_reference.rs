//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! A reference for the length rules of lengths.rs, sharing none of its code: every expected length here is worked out
//! afresh from Microsoft's documentation of the layouts, and the tests compare each rule of lengths.rs with it over
//! every class of format (plain, packed pairs, block-compressed, depth, planar and the other refused ones), every
//! dimension (buffer, 1D, 2D, 3D, arrays, mip chains) and boxes (none, whole, inside, off the block grid, empty, past
//! the edge), at packed, padded, zero and outsized pitches.
//!
//! The documentation each part rests on:
//! - [D3D11_SUBRESOURCE_DATA](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_subresource_data):
//!   the element at (x, y, d) of initial data is at `d * SysMemSlicePitch + y * SysMemPitch + x * BytesPerPixel`; the
//!   pitch is used only for 2D and 3D, the slice pitch only for 3D; a mip level of a 3D texture has half the slices
//!   of the one before, at least one, rounded down.
//! - [ID3D11DeviceContext::UpdateSubresource](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-updatesubresource):
//!   SrcRowPitch is the size of one row of the source and SrcDepthPitch of one depth slice; the box is in bytes for a
//!   buffer and in texels for a texture; an empty box is a no-op; the worked example (a 2 x 3 x 4 volume of 16-byte
//!   elements: row pitch 32, depth pitch 96).
//! - [D3D11_BOX](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_box): left, top and front are
//!   included, right, bottom and back are not.
//! - [D3D11_MAPPED_SUBRESOURCE](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_mapped_subresource):
//!   pData is row 0 of slice 0, RowPitch moves from row to row and DepthPitch from slice to slice, and they are for
//!   nothing but iterating rows and slices; a buffer's pitches are at least its size; a 2D texture's depth pitch is
//!   not used.
//! - [DXGI_FORMAT](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format): each format's
//!   number and bits per element; R8G8_B8G8 and G8R8_G8B8 hold a pair of texels in 32 bits; the planar and video
//!   formats.
//! - [Texture block compression](https://learn.microsoft.com/windows/win32/direct3d11/texture-block-compression-in-direct3d-11):
//!   4 x 4 texel blocks, 8 bytes for BC1 and BC4, 16 for BC2, BC3, BC5, BC6H and BC7; a mip level smaller than a
//!   block still takes a whole block.
//! - [Subresources](https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-subresources)
//!   and [D3D11CalcSubresource](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11calcsubresource):
//!   index = mip + array slice * mip levels.
//! - [Textures](https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-textures-intro)
//!   and [D3D11_TEXTURE2D_DESC](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texture2d_desc):
//!   each mip level halves each side, rounding down, to one texel; MipLevels 0 is the full chain.
//! - [Resource limits](https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-limits):
//!   no resource is larger than `min(max(128, 0.25 * dedicated video memory), 2048)` MiB, so never larger than 2048 MiB
//!   (d3d11.h's `D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_C_TERM`, 0x800).
//!
//! The glue's own choices the reference also states: a format the glue does not cover (unknown, 1-bit, planar, video,
//! palette and sampler feedback formats) has no length, so its bytes are refused; a box that does not start or end on
//! a block's edge reads every block it touches; the serving glue lends whole rows and slices at the pitches it tells
//! the C side.
//!
//! It is used by the rules' tests (lengths_tests.rs) and by the exactly sized C tests (tests/exact), each using a part.

#![allow(dead_code, reason = "each of its two users uses a part")]

use wcontainers_direct3d_11 as direct3d_11;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

/// What the documentation says one element of a format is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Element {
    /// One texel of this many bytes.
    Texel(u64),
    /// Two texels side by side in four bytes.
    Pair,
    /// A 4 x 4 block of texels in this many bytes.
    Block(u64),
    /// Not covered by the glue: its bytes are refused.
    Refused,
}

/// Each DXGI_FORMAT the glue covers, by its number, as the DXGI_FORMAT page lists it, with its element; every other
/// number (UNKNOWN 0; R1_UNORM 66, one bit a texel; the video formats AYUV, Y410, Y416, 100 to 102; the planar NV12,
/// P010, P016, 420_OPAQUE, 103 to 106; YUY2, Y210, Y216, 107 to 109; planar NV11, 110; AI44, IA44, P8, A8P8, 111 to
/// 114; planar P208, V208, V408, 130 to 132; the sampler feedback formats, 189 and 190; and any number the page does
/// not list) is refused.
pub(crate) const FORMATS: &[(i32, Element)] = &[
    // R32G32B32A32_TYPELESS, _FLOAT, _UINT, _SINT: 128 bits.
    (1, Element::Texel(16)),
    (2, Element::Texel(16)),
    (3, Element::Texel(16)),
    (4, Element::Texel(16)),
    // R32G32B32_TYPELESS, _FLOAT, _UINT, _SINT: 96 bits.
    (5, Element::Texel(12)),
    (6, Element::Texel(12)),
    (7, Element::Texel(12)),
    (8, Element::Texel(12)),
    // R16G16B16A16_TYPELESS, _FLOAT, _UNORM, _UINT, _SNORM, _SINT: 64 bits.
    (9, Element::Texel(8)),
    (10, Element::Texel(8)),
    (11, Element::Texel(8)),
    (12, Element::Texel(8)),
    (13, Element::Texel(8)),
    (14, Element::Texel(8)),
    // R32G32_TYPELESS, _FLOAT, _UINT, _SINT: 64 bits.
    (15, Element::Texel(8)),
    (16, Element::Texel(8)),
    (17, Element::Texel(8)),
    (18, Element::Texel(8)),
    // R32G8X24_TYPELESS, D32_FLOAT_S8X24_UINT (depth), R32_FLOAT_X8X24_TYPELESS, X32_TYPELESS_G8X24_UINT: 64 bits.
    (19, Element::Texel(8)),
    (20, Element::Texel(8)),
    (21, Element::Texel(8)),
    (22, Element::Texel(8)),
    // R10G10B10A2_TYPELESS, _UNORM, _UINT; R11G11B10_FLOAT: 32 bits.
    (23, Element::Texel(4)),
    (24, Element::Texel(4)),
    (25, Element::Texel(4)),
    (26, Element::Texel(4)),
    // R8G8B8A8_TYPELESS, _UNORM, _UNORM_SRGB, _UINT, _SNORM, _SINT: 32 bits.
    (27, Element::Texel(4)),
    (28, Element::Texel(4)),
    (29, Element::Texel(4)),
    (30, Element::Texel(4)),
    (31, Element::Texel(4)),
    (32, Element::Texel(4)),
    // R16G16_TYPELESS, _FLOAT, _UNORM, _UINT, _SNORM, _SINT: 32 bits.
    (33, Element::Texel(4)),
    (34, Element::Texel(4)),
    (35, Element::Texel(4)),
    (36, Element::Texel(4)),
    (37, Element::Texel(4)),
    (38, Element::Texel(4)),
    // R32_TYPELESS, D32_FLOAT (depth), R32_FLOAT, R32_UINT, R32_SINT: 32 bits.
    (39, Element::Texel(4)),
    (40, Element::Texel(4)),
    (41, Element::Texel(4)),
    (42, Element::Texel(4)),
    (43, Element::Texel(4)),
    // R24G8_TYPELESS, D24_UNORM_S8_UINT (depth), R24_UNORM_X8_TYPELESS, X24_TYPELESS_G8_UINT: 32 bits.
    (44, Element::Texel(4)),
    (45, Element::Texel(4)),
    (46, Element::Texel(4)),
    (47, Element::Texel(4)),
    // R8G8_TYPELESS, _UNORM, _UINT, _SNORM, _SINT: 16 bits.
    (48, Element::Texel(2)),
    (49, Element::Texel(2)),
    (50, Element::Texel(2)),
    (51, Element::Texel(2)),
    (52, Element::Texel(2)),
    // R16_TYPELESS, R16_FLOAT, D16_UNORM (depth), R16_UNORM, _UINT, _SNORM, _SINT: 16 bits.
    (53, Element::Texel(2)),
    (54, Element::Texel(2)),
    (55, Element::Texel(2)),
    (56, Element::Texel(2)),
    (57, Element::Texel(2)),
    (58, Element::Texel(2)),
    (59, Element::Texel(2)),
    // R8_TYPELESS, _UNORM, _UINT, _SNORM, _SINT; A8_UNORM: 8 bits.
    (60, Element::Texel(1)),
    (61, Element::Texel(1)),
    (62, Element::Texel(1)),
    (63, Element::Texel(1)),
    (64, Element::Texel(1)),
    (65, Element::Texel(1)),
    // R9G9B9E5_SHAREDEXP: 32 bits.
    (67, Element::Texel(4)),
    // R8G8_B8G8_UNORM, G8R8_G8B8_UNORM: a pair of texels in 32 bits.
    (68, Element::Pair),
    (69, Element::Pair),
    // BC1_TYPELESS, _UNORM, _UNORM_SRGB: 8 bytes a block.
    (70, Element::Block(8)),
    (71, Element::Block(8)),
    (72, Element::Block(8)),
    // BC2_TYPELESS, _UNORM, _UNORM_SRGB; BC3 the same: 16 bytes a block.
    (73, Element::Block(16)),
    (74, Element::Block(16)),
    (75, Element::Block(16)),
    (76, Element::Block(16)),
    (77, Element::Block(16)),
    (78, Element::Block(16)),
    // BC4_TYPELESS, _UNORM, _SNORM: 8 bytes a block.
    (79, Element::Block(8)),
    (80, Element::Block(8)),
    (81, Element::Block(8)),
    // BC5_TYPELESS, _UNORM, _SNORM: 16 bytes a block.
    (82, Element::Block(16)),
    (83, Element::Block(16)),
    (84, Element::Block(16)),
    // B5G6R5_UNORM, B5G5R5A1_UNORM: 16 bits.
    (85, Element::Texel(2)),
    (86, Element::Texel(2)),
    // B8G8R8A8_UNORM, B8G8R8X8_UNORM, R10G10B10_XR_BIAS_A2_UNORM, B8G8R8A8_TYPELESS, _UNORM_SRGB, B8G8R8X8_TYPELESS, _UNORM_SRGB: 32 bits.
    (87, Element::Texel(4)),
    (88, Element::Texel(4)),
    (89, Element::Texel(4)),
    (90, Element::Texel(4)),
    (91, Element::Texel(4)),
    (92, Element::Texel(4)),
    (93, Element::Texel(4)),
    // BC6H_TYPELESS, _UF16, _SF16; BC7_TYPELESS, _UNORM, _UNORM_SRGB: 16 bytes a block.
    (94, Element::Block(16)),
    (95, Element::Block(16)),
    (96, Element::Block(16)),
    (97, Element::Block(16)),
    (98, Element::Block(16)),
    (99, Element::Block(16)),
    // B4G4R4A4_UNORM, A4B4G4R4_UNORM: 16 bits.
    (115, Element::Texel(2)),
    (191, Element::Texel(2)),
];

/// A format's element, from `FORMATS`.
pub(crate) fn element(format: i32) -> Element {
    FORMATS
        .iter()
        .find(|(number, _)| *number == format)
        .map_or(Element::Refused, |(_, element)| *element)
}

/// The largest resource any device allows, in bytes: 2048 MiB, the ceiling of the documented limit
/// `min(max(128, 0.25 * dedicated video memory in MiB), 2048)` MiB.
pub(crate) const MOST_BYTES: u64 = 2048 * 1024 * 1024;

/// A side at a mip level: halved per level, rounded down, at least one.
pub(crate) fn side_at(side: u32, mip: u32) -> u32 {
    let mut side = side;
    for _ in 0..mip {
        side /= 2;
    }
    side.max(1)
}

/// The levels of a full chain for the longest side: halving until one texel, counting the level of one texel.
pub(crate) fn full_chain(longest: u32) -> u32 {
    let mut levels = 1;
    let mut side = longest.max(1);
    while side > 1 {
        side /= 2;
        levels += 1;
    }
    levels
}

/// A subresource's mip level and array slice from its index (D3D11CalcSubresource read backwards); none past the
/// resource.
pub(crate) fn mip_and_slice(index: u32, levels: u32, array_size: u32) -> Option<(u32, u32)> {
    let (mip, slice) = (index % levels, index / levels);
    (slice < array_size).then_some((mip, slice))
}

/// The texels a box covers, as `[start, end)` on each axis.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub(crate) x: (u32, u32),
    pub(crate) y: (u32, u32),
    pub(crate) z: (u32, u32),
}

/// What a box over a subresource of these sides says: past the sides (refused), empty (no bytes), or a span.
pub(crate) enum Covered {
    Past,
    Empty,
    Span(Span),
}

pub(crate) fn covered(
    sides: (u32, u32, u32),
    region: Option<&direct3d_11::structures::Box>,
) -> Covered {
    let Some(region) = region else {
        return Covered::Span(Span {
            x: (0, sides.0),
            y: (0, sides.1),
            z: (0, sides.2),
        });
    };
    if region.right > sides.0 || region.bottom > sides.1 || region.back > sides.2 {
        return Covered::Past;
    }
    if region.right <= region.left || region.bottom <= region.top || region.back <= region.front {
        return Covered::Empty;
    }
    Covered::Span(Span {
        x: (region.left, region.right),
        y: (region.top, region.bottom),
        z: (region.front, region.back),
    })
}

/// The bytes of one row of a span on the x axis, and how many rows of elements the span on the y axis takes.
pub(crate) fn row_and_rows(element: Element, x: (u32, u32), y: (u32, u32)) -> Option<(u64, u64)> {
    let first_column = |side: u32| u64::from(x.0 / side);
    let past_column = |side: u32| u64::from(x.1).div_ceil(u64::from(side));
    let first_row = |side: u32| u64::from(y.0 / side);
    let past_row = |side: u32| u64::from(y.1).div_ceil(u64::from(side));
    match element {
        Element::Texel(bytes) => Some((u64::from(x.1 - x.0) * bytes, u64::from(y.1 - y.0))),
        Element::Pair => Some(((past_column(2) - first_column(2)) * 4, u64::from(y.1 - y.0))),
        Element::Block(bytes) => Some((
            (past_column(4) - first_column(4)) * bytes,
            past_row(4) - first_row(4),
        )),
        Element::Refused => None,
    }
}

/// The bytes from the first element read to the end of the last, by the documented addressing: the last element of a
/// span starts at `(slices - 1) * slice pitch + (rows - 1) * row pitch + its row's offset`.
pub(crate) fn touched(row: u64, rows: u64, slices: u64, row_pitch: u32, slice_pitch: u32) -> u64 {
    let last_slice_start = u64::from(slice_pitch) * (slices - 1);
    let last_row_start = last_slice_start + u64::from(row_pitch) * (rows - 1);
    last_row_start + row
}

/// A length within the largest resource.
pub(crate) fn within(length: u64) -> Option<usize> {
    (length <= MOST_BYTES)
        .then(|| usize::try_from(length).ok())
        .flatten()
}

/// The bytes the serving glue lends for a mapped subresource it views `viewed` bytes of: raised to every row whole (and
/// every slice but the last whole) and to every slice whole, at the pitches.
pub(crate) fn lend(
    viewed: u64,
    rows: u64,
    slices: u64,
    row_pitch: u32,
    depth_pitch: u32,
) -> Option<usize> {
    let whole_rows = u64::from(depth_pitch) * (slices - 1) + u64::from(row_pitch) * rows;
    let whole_slices = u64::from(depth_pitch) * slices;
    within(viewed.max(whole_rows).max(whole_slices))
}

/// The resource of a test case, by dimension.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Resource {
    Buffer(u32),
    Texture1D {
        width: u32,
        mips: u32,
        array: u32,
        format: i32,
    },
    Texture2D {
        width: u32,
        height: u32,
        mips: u32,
        array: u32,
        format: i32,
    },
    Texture3D {
        width: u32,
        height: u32,
        depth: u32,
        mips: u32,
        format: i32,
    },
}

impl Resource {
    /// How many subresources it has.
    pub(crate) fn subresources(self) -> u32 {
        match self {
            Resource::Buffer(_) => 1,
            Resource::Texture1D {
                width, mips, array, ..
            } => levels(mips, width) * array,
            Resource::Texture2D {
                width,
                height,
                mips,
                array,
                ..
            } => levels(mips, width.max(height)) * array,
            Resource::Texture3D {
                width,
                height,
                depth,
                mips,
                ..
            } => levels(mips, width.max(height).max(depth)),
        }
    }

    /// A subresource's sides in texels (a buffer's in bytes) and its format's element; none past the resource.
    pub(crate) fn subresource(self, index: u32) -> Option<((u32, u32, u32), Element)> {
        match self {
            Resource::Buffer(width) => (index == 0).then_some(((width, 1, 1), Element::Texel(1))),
            Resource::Texture1D {
                width,
                mips,
                array,
                format,
            } => {
                let (mip, _) = mip_and_slice(index, levels(mips, width), array)?;
                Some(((side_at(width, mip), 1, 1), element(format)))
            }
            Resource::Texture2D {
                width,
                height,
                mips,
                array,
                format,
            } => {
                let (mip, _) = mip_and_slice(index, levels(mips, width.max(height)), array)?;
                Some((
                    (side_at(width, mip), side_at(height, mip), 1),
                    element(format),
                ))
            }
            Resource::Texture3D {
                width,
                height,
                depth,
                mips,
                format,
            } => {
                let (mip, _) = mip_and_slice(index, levels(mips, width.max(height).max(depth)), 1)?;
                Some((
                    (
                        side_at(width, mip),
                        side_at(height, mip),
                        side_at(depth, mip),
                    ),
                    element(format),
                ))
            }
        }
    }

    /// Whether the documentation uses a pitch for this dimension's rows (2D and 3D) and slices (3D).
    pub(crate) fn pitches_used(self) -> (bool, bool) {
        match self {
            Resource::Buffer(_) | Resource::Texture1D { .. } => (false, false),
            Resource::Texture2D { .. } => (true, false),
            Resource::Texture3D { .. } => (true, true),
        }
    }
}

/// A texture's mip levels: its own count, or the full chain for 0.
pub(crate) fn levels(mips: u32, longest: u32) -> u32 {
    if mips == 0 { full_chain(longest) } else { mips }
}

/// The reference for initial bytes (D3D11_SUBRESOURCE_DATA).
pub(crate) fn reference_initial(
    resource: Resource,
    index: u32,
    pitch: u32,
    slice_pitch: u32,
) -> Option<usize> {
    let (sides, element) = resource.subresource(index)?;
    let (row, rows) = row_and_rows(element, (0, sides.0), (0, sides.1))?;
    let (rows_used, slices_used) = resource.pitches_used();
    let rows = if rows_used { rows } else { 1 };
    let slices = if slices_used { u64::from(sides.2) } else { 1 };
    within(touched(row, rows, slices, pitch, slice_pitch))
}

/// The reference for an upload (UpdateSubresource with D3D11_BOX).
pub(crate) fn reference_upload(
    resource: Resource,
    index: u32,
    region: Option<&direct3d_11::structures::Box>,
    row_pitch: u32,
    depth_pitch: u32,
) -> Option<usize> {
    let (sides, element) = resource.subresource(index)?;
    let span = match covered(sides, region) {
        Covered::Past => return None,
        Covered::Empty => return Some(0),
        Covered::Span(span) => span,
    };
    let (row, rows) = row_and_rows(element, span.x, span.y)?;
    within(touched(
        row,
        rows,
        u64::from(span.z.1 - span.z.0),
        row_pitch,
        depth_pitch,
    ))
}

/// The reference for the bytes of the system's mapped memory a program may view (D3D11_MAPPED_SUBRESOURCE): a buffer's
/// size; a 1D texture's one row; a 2D texture's rows at the row pitch; a 3D texture's slices at the depth pitch too.
pub(crate) fn reference_mapped(
    resource: Resource,
    index: u32,
    row_pitch: u32,
    depth_pitch: u32,
) -> Option<usize> {
    let (sides, element) = resource.subresource(index)?;
    let (row, rows) = row_and_rows(element, (0, sides.0), (0, sides.1))?;
    let (rows_used, slices_used) = resource.pitches_used();
    let rows = if rows_used { rows } else { 1 };
    let slices = if slices_used { u64::from(sides.2) } else { 1 };
    within(touched(row, rows, slices, row_pitch, depth_pitch))
}

/// The reference for what the serving glue lends: the view, with every row and slice whole at the pitches it tells
/// the C side; a buffer's and a 1D texture's raised to both pitches.
pub(crate) fn reference_lent(
    resource: Resource,
    index: u32,
    row_pitch: u32,
    depth_pitch: u32,
) -> Option<usize> {
    let viewed = u64::try_from(reference_mapped(resource, index, row_pitch, depth_pitch)?).ok()?;
    let (sides, element) = resource.subresource(index)?;
    let (_, rows) = row_and_rows(element, (0, sides.0), (0, sides.1))?;
    match resource.pitches_used() {
        (false, _) => within(viewed.max(u64::from(row_pitch)).max(u64::from(depth_pitch))),
        (true, false) => lend(viewed, rows, 1, row_pitch, depth_pitch),
        (true, true) => lend(viewed, rows, u64::from(sides.2), row_pitch, depth_pitch),
    }
}

/// D3D11_USAGE_STAGING, a usage every rule accepts.
pub(crate) const STAGING: i32 = 3;

pub(crate) fn buffer_desc(width: u32) -> direct3d_11::structures::BufferDesc {
    direct3d_11::structures::BufferDesc {
        byte_width: width,
        usage: direct3d_11::enumerations::Usage(STAGING),
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
        structure_byte_stride: 0,
    }
}

pub(crate) fn texture_1d_desc(
    width: u32,
    mips: u32,
    array: u32,
    format: i32,
) -> direct3d_11::structures::Texture1DDesc {
    direct3d_11::structures::Texture1DDesc {
        width,
        mip_levels: mips,
        array_size: array,
        format: directx_graphics_infrastructure::enumerations::Format(format),
        usage: direct3d_11::enumerations::Usage(STAGING),
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
    }
}

pub(crate) fn texture_2d_desc(
    width: u32,
    height: u32,
    mips: u32,
    array: u32,
    format: i32,
) -> direct3d_11::structures::Texture2DDesc {
    direct3d_11::structures::Texture2DDesc {
        width,
        height,
        mip_levels: mips,
        array_size: array,
        format: directx_graphics_infrastructure::enumerations::Format(format),
        sample_desc: directx_graphics_infrastructure::structures::SampleDesc {
            count: 1,
            quality: 0,
        },
        usage: direct3d_11::enumerations::Usage(STAGING),
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
    }
}

pub(crate) fn texture_3d_desc(
    width: u32,
    height: u32,
    depth: u32,
    mips: u32,
    format: i32,
) -> direct3d_11::structures::Texture3DDesc {
    direct3d_11::structures::Texture3DDesc {
        width,
        height,
        depth,
        mip_levels: mips,
        format: directx_graphics_infrastructure::enumerations::Format(format),
        usage: direct3d_11::enumerations::Usage(STAGING),
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
    }
}

pub(crate) fn region(
    left: u32,
    top: u32,
    front: u32,
    right: u32,
    bottom: u32,
    back: u32,
) -> direct3d_11::structures::Box {
    direct3d_11::structures::Box {
        left,
        top,
        front,
        right,
        bottom,
        back,
    }
}

/// The pitches a case is tried at, for a subresource whose packed row is `row` bytes over `rows` rows: packed, padded to
/// 256 bytes, padded by an odd amount, zero, a depth pitch far past the rows, and pitches past the largest resource.
pub(crate) fn pitches(row: u64, rows: u64) -> Vec<(u32, u32)> {
    let row = u32::try_from(row).unwrap_or(u32::MAX);
    let rows = u32::try_from(rows).unwrap_or(u32::MAX);
    let padded = row.next_multiple_of(256);
    vec![
        (row, row.saturating_mul(rows)),
        (padded, padded.saturating_mul(rows)),
        (
            row.saturating_add(13),
            row.saturating_add(13)
                .saturating_mul(rows)
                .saturating_add(7),
        ),
        (0, 0),
        (row, 0),
        (padded, u32::MAX / 2),
        (u32::MAX, u32::MAX),
        (1 << 27, 1 << 27),
    ]
}

/// The boxes a case is tried with, over a subresource of these sides (in texels, a buffer's in bytes).
pub(crate) fn boxes(
    (width, height, depth): (u32, u32, u32),
) -> Vec<Option<direct3d_11::structures::Box>> {
    let mut boxes = vec![
        None,
        Some(region(0, 0, 0, width, height, depth)),
        // Inside, off the block grid on every edge where the sides allow.
        Some(region(
            width / 3,
            height / 3,
            depth / 3,
            width - width / 5,
            height - height / 5,
            depth - depth / 5,
        )),
        Some(region(
            1.min(width - 1),
            1.min(height - 1),
            0,
            width,
            height,
            depth,
        )),
        Some(region(0, 0, 0, 1, 1, 1)),
        // Empty on each axis.
        Some(region(width / 2, 0, 0, width / 2, height, depth)),
        Some(region(0, height, 0, width, height, depth)),
        Some(region(0, 0, depth, width, height, depth)),
        Some(region(width, 0, 0, 0, height, depth)),
        // Past each edge.
        Some(region(0, 0, 0, width + 1, height, depth)),
        Some(region(0, 0, 0, width, height + 1, depth)),
        Some(region(0, 0, 0, width, height, depth + 1)),
    ];
    if width >= 8 && height >= 8 {
        // Aligned to 4 x 4 blocks, and a box of one texel in the middle of a block.
        boxes.push(Some(region(4, 4, 0, 8, 8, depth)));
        boxes.push(Some(region(5, 6, 0, 6, 7, depth)));
        boxes.push(Some(region(2, 2, 0, 6, 6, depth)));
    }
    boxes
}

/// Every number a format may have, and some past the list.
pub(crate) fn every_format() -> impl Iterator<Item = i32> {
    (-1..=200).chain([255, 1000, i32::MAX, i32::MIN])
}
