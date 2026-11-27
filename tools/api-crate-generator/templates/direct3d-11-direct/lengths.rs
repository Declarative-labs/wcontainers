//! The lengths the metadata does not give, hand-written beside the generator (each an exception line of src/api.rs):
//! how many bytes the C side may touch in a resource's subresource, from its description, by the one table of
//! formats (dxgiformat.h). Both glues use the same rules: the serving glue copies that many initial bytes and lends a
//! copy of ours that long; the calling glue refuses initial bytes shorter than that and views that many of the
//! system's mapped bytes. Every length is computed with checked arithmetic and is at most the largest resource.

#![allow(dead_code, reason = "a slice of the API holds some of the shapes")]

/// One mebibyte, as a shift: d3d11.h states the largest resource in mebibytes.
const MEBIBYTE_SHIFT: u32 = 20;

/// The largest resource, in bytes: d3d11.h's `D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_C_TERM` mebibytes.
fn most_bytes() -> usize {
    usize::try_from(direct3d_11::constants::D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_C_TERM).unwrap_or(usize::MAX) << MEBIBYTE_SHIFT
}

/// A block of texels: one texel for most formats, 4 by 4 for the compressed ones.
#[derive(Clone, Copy)]
struct Block {
    width: u32,
    height: u32,
    bytes: u32,
}

/// The side of a compressed format's block, in texels.
const COMPRESSED_SIDE: u32 = 4;

const fn texel(bytes: u32) -> Option<Block> {
    Some(Block { width: 1, height: 1, bytes })
}

const fn compressed(bytes: u32) -> Option<Block> {
    Some(Block { width: COMPRESSED_SIDE, height: COMPRESSED_SIDE, bytes })
}

/// Returns a format's block, or none for one this table does not cover (unknown, 1-bit, planar and video formats),
/// whose initial or mapped bytes the glue refuses.
///
/// <https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format>
const fn block(format: directx_graphics_infrastructure::enumerations::Format) -> Option<Block> {
    match format.0 {
        // R32G32B32A32
        1..=4 => texel(16),
        // R32G32B32
        5..=8 => texel(12),
        // R16G16B16A16, R32G32, R32G8X24 and its views
        9..=22 => texel(8),
        // R10G10B10A2, R11G11B10, R8G8B8A8, R16G16, R32, R24G8 and its views
        23..=47 => texel(4),
        // R8G8, R16
        48..=59 => texel(2),
        // R8, A8
        60..=65 => texel(1),
        // R9G9B9E5_SHAREDEXP
        67 => texel(4),
        // R8G8_B8G8 and G8R8_G8B8: two texels in four bytes
        68 | 69 => Some(Block { width: 2, height: 1, bytes: 4 }),
        // BC1, BC4
        70..=72 | 79..=81 => compressed(8),
        // BC2, BC3, BC5
        73..=78 | 82..=84 => compressed(16),
        // B5G6R5, B5G5R5A1
        85 | 86 => texel(2),
        // B8G8R8A8, B8G8R8X8, R10G10B10_XR_BIAS_A2
        87..=93 => texel(4),
        // BC6H, BC7
        94..=99 => compressed(16),
        // B4G4R4A4, A4B4G4R4
        115 | 191 => texel(2),
        _ => None,
    }
}

/// Returns the bytes of `slices` slices of `rows` rows of blocks: every row but the last at `row_pitch`, every slice
/// but the last at `slice_pitch`, then one row's packed bytes; none past the largest resource.
fn extent(row_bytes: u64, rows: u64, row_pitch: u32, slices: u64, slice_pitch: u32) -> Option<usize> {
    let last_slice = u64::from(row_pitch).checked_mul(rows.checked_sub(1)?)?.checked_add(row_bytes)?;
    let all = u64::from(slice_pitch).checked_mul(slices.checked_sub(1)?)?.checked_add(last_slice)?;
    usize::try_from(all).ok().filter(|length| *length <= most_bytes())
}

/// Returns a mip's side: the full side halved per level, at least one texel.
fn mip_side(side: u32, mip: u32) -> u32 {
    side.checked_shr(mip).unwrap_or(0).max(1)
}

/// Returns a texture's mip levels: its own count, or the full chain for 0; none past the 32 a side of `u32` holds.
fn mips(mip_levels: u32, longest_side: u32) -> Option<u32> {
    let levels = if mip_levels == 0 { u32::BITS - longest_side.max(1).leading_zeros() } else { mip_levels };
    (levels < u32::BITS).then_some(levels)
}

/// Returns the bytes of a subresource of `width` by `height` by `depth` texels: rows of blocks at `row_pitch`, slices
/// at `slice_pitch`; none for a format the table does not cover.
fn subresource_extent(
    format: directx_graphics_infrastructure::enumerations::Format,
    (width, height, depth): (u32, u32, u32),
    row_pitch: u32,
    slice_pitch: u32,
) -> Option<usize> {
    let block = block(format)?;
    let row_bytes = u64::from(width.div_ceil(block.width)) * u64::from(block.bytes);
    let rows = u64::from(height.div_ceil(block.height)).max(1);
    extent(row_bytes, rows, row_pitch, u64::from(depth).max(1), slice_pitch)
}

/// Returns the texels an upload's box covers in a subresource of `full` texels: the box's sides, or the whole
/// subresource for none; none for a box past the subresource's sides; zero sides for an empty box, which uploads nothing.
///
/// <https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_box>
fn region_of(full: (u32, u32, u32), region: Option<&direct3d_11::structures::Box>) -> Option<(u32, u32, u32)> {
    let Some(region) = region else { return Some(full) };
    if region.right > full.0 || region.bottom > full.1 || region.back > full.2 {
        return None;
    }
    if region.left >= region.right || region.top >= region.bottom || region.front >= region.back {
        return Some((0, 0, 0));
    }
    Some((region.right - region.left, region.bottom - region.top, region.back - region.front))
}

/// Returns the bytes an upload of `region` texels reads at the pitches: zero for an empty box.
fn upload_extent(format: directx_graphics_infrastructure::enumerations::Format, region: (u32, u32, u32), row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    if region.0 == 0 {
        return Some(0);
    }
    subresource_extent(format, region, row_pitch, depth_pitch)
}

/// d3d11.h: a texture's side, slices, samples and the most bits of each flag word it declares.
const MOST_SIDE_1D: u32 = 16384;
const MOST_SIDE_2D: u32 = 16384;
const MOST_SIDE_3D: u32 = 2048;
const MOST_SLICES: u32 = 2048;
const MOST_SAMPLES: u32 = 32;
/// DXGI_SWAP_CHAIN_DESC's BufferCount, at most.
const MOST_BACK_BUFFERS: u32 = 16;
/// D3D11_USAGE: DEFAULT, IMMUTABLE, DYNAMIC, STAGING.
const USAGES: core::ops::RangeInclusive<i32> = 0..=3;

/// Returns the total bytes of every subresource, packed, if within the largest resource.
fn total(count: u32, size: impl Fn(u32) -> Option<usize>) -> bool {
    let mut sum = 0_usize;
    for index in 0..count {
        let Some(bytes) = size(index).and_then(|bytes| sum.checked_add(bytes)) else { return false };
        sum = bytes;
    }
    sum <= most_bytes()
}

/// Returns whether a swap chain's description is within the ranges d3d11.h and dxgi.h state: 1 to 16 buffers of 0
/// (the window's) to 16384 texels a side, of a format the table covers or unknown, no more bytes than the largest
/// resource.
pub fn checked_swap_chain(desc: &directx_graphics_infrastructure::structures::SwapChainDesc) -> bool {
    let mode = &desc.buffer_desc;
    let sides = mode.width <= MOST_SIDE_2D && mode.height <= MOST_SIDE_2D;
    let format = mode.format.0 == 0 || block(mode.format).is_some();
    let bytes = block(mode.format).is_none_or(|block| {
        let row = u64::from(mode.width.max(1).div_ceil(block.width)) * u64::from(block.bytes);
        let all = row * u64::from(mode.height.max(1).div_ceil(block.height)) * u64::from(desc.buffer_count);
        usize::try_from(all).is_ok_and(|all| all <= most_bytes())
    });
    (1..=MOST_BACK_BUFFERS).contains(&desc.buffer_count) && sides && format && bytes && (1..=MOST_SAMPLES).contains(&desc.sample_desc.count)
}

/// D3D11_RESOURCE_DIMENSION of each kind of resource.
const DIMENSION_BUFFER: i32 = 1;
const DIMENSION_TEXTURE1D: i32 = 2;
const DIMENSION_TEXTURE2D: i32 = 3;
const DIMENSION_TEXTURE3D: i32 = 4;

/// Returns the size of a query kind's data, as d3d11.h's D3D11_QUERY_DATA_* structures lay it out; 0 for a kind the
/// header does not declare.
///
/// <https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query>
pub fn data_size_of_query(desc: &direct3d_11::structures::QueryDesc) -> u32 {
    match desc.query.0 {
        // EVENT, OCCLUSION_PREDICATE, SO_OVERFLOW_PREDICATE and its streams: a BOOL.
        0 | 5 | 7 | 12..=15 => 4,
        // OCCLUSION, TIMESTAMP: a UINT64.
        1 | 2 => 8,
        // TIMESTAMP_DISJOINT: a UINT64 and a BOOL, padded.
        3 => 16,
        // PIPELINE_STATISTICS: eleven UINT64.
        4 => 88,
        // SO_STATISTICS and its streams: two UINT64.
        6 | 8..=11 => 16,
        _ => 0,
    }
}

// shape: buffer
/// Returns a buffer's resource dimension.
pub fn type_of_buffer(_desc: &direct3d_11::structures::BufferDesc) -> direct3d_11::enumerations::ResourceDimension {
    direct3d_11::enumerations::ResourceDimension(DIMENSION_BUFFER)
}

/// Returns whether a buffer's description is within d3d11.h's ranges: 1 byte to the largest resource, a declared usage.
pub fn checked_buffer(desc: &direct3d_11::structures::BufferDesc) -> bool {
    usize::try_from(desc.byte_width).is_ok_and(|width| (1..=most_bytes()).contains(&width)) && USAGES.contains(&desc.usage.0)
}

/// Returns the length of a buffer's initial bytes: its byte width, for its one subresource.
pub fn initial_length_buffer(desc: &direct3d_11::structures::BufferDesc, index: usize, _pitch: u32, _slice_pitch: u32) -> Option<usize> {
    let length = usize::try_from(desc.byte_width).ok()?;
    (index == 0 && length <= most_bytes()).then_some(length)
}

/// Returns whether the serving glue lends `length` bytes for a buffer.
pub fn within_served_bound_buffer(desc: &direct3d_11::structures::BufferDesc, _subresource: u32, length: usize) -> bool {
    within_served_bound(usize::try_from(desc.byte_width).ok(), length)
}

/// Returns the length of a buffer's mapped bytes: its byte width, for its one subresource, and at least the pitches
/// the implementation gave, which the C side is told and fills.
pub fn mapped_length_buffer(desc: &direct3d_11::structures::BufferDesc, subresource: u32, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    at_least_pitches(initial_length_buffer(desc, usize::try_from(subresource).ok()?, 0, 0)?, row_pitch, depth_pitch)
}

/// Returns the length of the bytes an upload into a buffer reads: its box's bytes (a buffer's box is in bytes, one row
/// and one slice), or its byte width for none.
pub fn upload_length_buffer(desc: &direct3d_11::structures::BufferDesc, subresource: u32, region: Option<&direct3d_11::structures::Box>, _row_pitch: u32, _depth_pitch: u32) -> Option<usize> {
    if subresource != 0 {
        return None;
    }
    let (bytes, _, _) = region_of((desc.byte_width, 1, 1), region)?;
    usize::try_from(bytes).ok().filter(|length| *length <= most_bytes())
}
// end of shape

// shape: texture_1d
/// Returns the texture's resource dimension.
pub fn type_of_texture_1d(_desc: &direct3d_11::structures::Texture1DDesc) -> direct3d_11::enumerations::ResourceDimension {
    direct3d_11::enumerations::ResourceDimension(DIMENSION_TEXTURE1D)
}

/// Returns whether the description is within d3d11.h's ranges: its sides and slices, mips within the full chain, a
/// declared usage, a format the table covers, and no more bytes than the largest resource.
pub fn checked_texture_1d(desc: &direct3d_11::structures::Texture1DDesc) -> bool {
    let full = u32::BITS - (desc.width).max(1).leading_zeros();
    let fits = (1..=MOST_SIDE_1D).contains(&desc.width) && (1..=MOST_SLICES).contains(&desc.array_size) && desc.mip_levels <= full && USAGES.contains(&desc.usage.0) && block(desc.format).is_some();
    fits && subresources_texture_1d(desc).and_then(|count| u32::try_from(count).ok()).is_some_and(|count| {
        total(count, |index| {
            let size = size_texture_1d(desc, usize::try_from(index).ok()?)?;
            let block = block(desc.format)?;
            let row = usize::try_from(size.0.div_ceil(block.width)).ok()?.checked_mul(usize::try_from(block.bytes).ok()?)?;
            row.checked_mul(usize::try_from(size.1.div_ceil(block.height)).ok()?)?.checked_mul(usize::try_from(size.2).ok()?)
        })
    })
}

/// Returns the description as the documentation says it reads back: mip levels 0 is the full chain.
pub fn resolved_texture_1d(desc: &direct3d_11::structures::Texture1DDesc) -> direct3d_11::structures::Texture1DDesc {
    let mip_levels = mips(desc.mip_levels, desc.width).unwrap_or(desc.mip_levels);
    direct3d_11::structures::Texture1DDesc { mip_levels, ..*desc }
}

/// Returns how many subresources a 1D texture has: its mips times its array slices.
pub fn subresources_texture_1d(desc: &direct3d_11::structures::Texture1DDesc) -> Option<usize> {
    usize::try_from(mips(desc.mip_levels, desc.width)?.checked_mul(desc.array_size)?).ok()
}

/// Returns a 1D texture subresource's width, in subresource order (mips of the first slice first).
fn size_texture_1d(desc: &direct3d_11::structures::Texture1DDesc, index: usize) -> Option<(u32, u32, u32)> {
    let levels = mips(desc.mip_levels, desc.width)?;
    let index = u32::try_from(index).ok()?;
    (index / levels < desc.array_size).then(|| (mip_side(desc.width, index % levels), 1, 1))
}

/// Returns whether the serving glue lends `length` bytes for a subresource of the texture.
pub fn within_served_bound_texture_1d(desc: &direct3d_11::structures::Texture1DDesc, subresource: u32, length: usize) -> bool {
    let padded = usize::try_from(subresource).ok().and_then(|index| padded_extent(desc.format, size_texture_1d(desc, index)?));
    padded.is_some_and(|padded| length <= padded.max(LEAST_SERVED_BOUND))
}

/// Returns the length of a 1D texture subresource's initial bytes: one row.
pub fn initial_length_texture_1d(desc: &direct3d_11::structures::Texture1DDesc, index: usize, pitch: u32, _slice_pitch: u32) -> Option<usize> {
    subresource_extent(desc.format, size_texture_1d(desc, index)?, pitch, 0)
}

/// Returns the length of a 1D texture subresource's mapped bytes: one row, and at least the pitches the system gave.
pub fn mapped_length_texture_1d(desc: &direct3d_11::structures::Texture1DDesc, subresource: u32, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let rows = subresource_extent(desc.format, size_texture_1d(desc, usize::try_from(subresource).ok()?)?, row_pitch, 0)?;
    at_least_pitches(rows, row_pitch, depth_pitch)
}

/// Returns the length of the bytes an upload into a subresource of the texture reads: the box's rows of blocks (the whole
/// subresource's for none) at the pitches.
pub fn upload_length_texture_1d(desc: &direct3d_11::structures::Texture1DDesc, subresource: u32, region: Option<&direct3d_11::structures::Box>, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let full = size_texture_1d(desc, usize::try_from(subresource).ok()?)?;
    upload_extent(desc.format, region_of(full, region)?, row_pitch, depth_pitch)
}
// end of shape

// shape: texture_2d
/// Returns the texture's resource dimension.
pub fn type_of_texture_2d(_desc: &direct3d_11::structures::Texture2DDesc) -> direct3d_11::enumerations::ResourceDimension {
    direct3d_11::enumerations::ResourceDimension(DIMENSION_TEXTURE2D)
}

/// Returns the surface a 2D texture is, as DXGI describes it.
pub fn surface_of_texture_2d(desc: &direct3d_11::structures::Texture2DDesc) -> directx_graphics_infrastructure::structures::SurfaceDesc {
    directx_graphics_infrastructure::structures::SurfaceDesc { width: desc.width, height: desc.height, format: desc.format, sample_desc: desc.sample_desc }
}

/// Returns whether the description is within d3d11.h's ranges: its sides and slices, mips within the full chain, a
/// declared usage, a format the table covers, and no more bytes than the largest resource.
pub fn checked_texture_2d(desc: &direct3d_11::structures::Texture2DDesc) -> bool {
    let full = u32::BITS - (desc.width.max(desc.height)).max(1).leading_zeros();
    let fits = (1..=MOST_SIDE_2D).contains(&desc.width) && (1..=MOST_SIDE_2D).contains(&desc.height) && (1..=MOST_SLICES).contains(&desc.array_size) && (1..=MOST_SAMPLES).contains(&desc.sample_desc.count) && desc.mip_levels <= full && USAGES.contains(&desc.usage.0) && block(desc.format).is_some();
    fits && subresources_texture_2d(desc).and_then(|count| u32::try_from(count).ok()).is_some_and(|count| {
        total(count, |index| {
            let size = size_texture_2d(desc, usize::try_from(index).ok()?)?;
            let block = block(desc.format)?;
            let row = usize::try_from(size.0.div_ceil(block.width)).ok()?.checked_mul(usize::try_from(block.bytes).ok()?)?;
            row.checked_mul(usize::try_from(size.1.div_ceil(block.height)).ok()?)?.checked_mul(usize::try_from(size.2).ok()?)
        })
    })
}

/// Returns the description as the documentation says it reads back: mip levels 0 is the full chain.
pub fn resolved_texture_2d(desc: &direct3d_11::structures::Texture2DDesc) -> direct3d_11::structures::Texture2DDesc {
    let mip_levels = mips(desc.mip_levels, desc.width.max(desc.height)).unwrap_or(desc.mip_levels);
    direct3d_11::structures::Texture2DDesc { mip_levels, ..*desc }
}

/// Returns how many subresources a 2D texture has: its mips times its array slices.
pub fn subresources_texture_2d(desc: &direct3d_11::structures::Texture2DDesc) -> Option<usize> {
    usize::try_from(mips(desc.mip_levels, desc.width.max(desc.height))?.checked_mul(desc.array_size)?).ok()
}

/// Returns a 2D texture subresource's width and height, in subresource order (mips of the first slice first).
fn size_texture_2d(desc: &direct3d_11::structures::Texture2DDesc, index: usize) -> Option<(u32, u32, u32)> {
    let levels = mips(desc.mip_levels, desc.width.max(desc.height))?;
    let index = u32::try_from(index).ok()?;
    let mip = index % levels;
    (index / levels < desc.array_size).then(|| (mip_side(desc.width, mip), mip_side(desc.height, mip), 1))
}

/// Returns whether the serving glue lends `length` bytes for a subresource of the texture.
pub fn within_served_bound_texture_2d(desc: &direct3d_11::structures::Texture2DDesc, subresource: u32, length: usize) -> bool {
    let padded = usize::try_from(subresource).ok().and_then(|index| padded_extent(desc.format, size_texture_2d(desc, index)?));
    padded.is_some_and(|padded| length <= padded.max(LEAST_SERVED_BOUND))
}

/// Returns the length of a 2D texture subresource's initial bytes: its rows of blocks at the pitch.
pub fn initial_length_texture_2d(desc: &direct3d_11::structures::Texture2DDesc, index: usize, pitch: u32, _slice_pitch: u32) -> Option<usize> {
    subresource_extent(desc.format, size_texture_2d(desc, index)?, pitch, 0)
}

/// Returns the length of a 2D texture subresource's mapped bytes: its rows at the pitch, and at least the pitches the
/// system gave, which programs fill whole.
pub fn mapped_length_texture_2d(desc: &direct3d_11::structures::Texture2DDesc, subresource: u32, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let rows = subresource_extent(desc.format, size_texture_2d(desc, usize::try_from(subresource).ok()?)?, row_pitch, 0)?;
    at_least_pitches(rows, row_pitch, depth_pitch)
}

/// Returns the length of the bytes an upload into a subresource of the texture reads: the box's rows of blocks (the whole
/// subresource's for none) at the pitches.
pub fn upload_length_texture_2d(desc: &direct3d_11::structures::Texture2DDesc, subresource: u32, region: Option<&direct3d_11::structures::Box>, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let full = size_texture_2d(desc, usize::try_from(subresource).ok()?)?;
    upload_extent(desc.format, region_of(full, region)?, row_pitch, depth_pitch)
}
// end of shape

// shape: texture_3d
/// Returns the texture's resource dimension.
pub fn type_of_texture_3d(_desc: &direct3d_11::structures::Texture3DDesc) -> direct3d_11::enumerations::ResourceDimension {
    direct3d_11::enumerations::ResourceDimension(DIMENSION_TEXTURE3D)
}

/// Returns whether the description is within d3d11.h's ranges: its sides and slices, mips within the full chain, a
/// declared usage, a format the table covers, and no more bytes than the largest resource.
pub fn checked_texture_3d(desc: &direct3d_11::structures::Texture3DDesc) -> bool {
    let full = u32::BITS - (desc.width.max(desc.height).max(desc.depth)).max(1).leading_zeros();
    let fits = (1..=MOST_SIDE_3D).contains(&desc.width) && (1..=MOST_SIDE_3D).contains(&desc.height) && (1..=MOST_SIDE_3D).contains(&desc.depth) && desc.mip_levels <= full && USAGES.contains(&desc.usage.0) && block(desc.format).is_some();
    fits && subresources_texture_3d(desc).and_then(|count| u32::try_from(count).ok()).is_some_and(|count| {
        total(count, |index| {
            let size = size_texture_3d(desc, usize::try_from(index).ok()?)?;
            let block = block(desc.format)?;
            let row = usize::try_from(size.0.div_ceil(block.width)).ok()?.checked_mul(usize::try_from(block.bytes).ok()?)?;
            row.checked_mul(usize::try_from(size.1.div_ceil(block.height)).ok()?)?.checked_mul(usize::try_from(size.2).ok()?)
        })
    })
}

/// Returns the description as the documentation says it reads back: mip levels 0 is the full chain.
pub fn resolved_texture_3d(desc: &direct3d_11::structures::Texture3DDesc) -> direct3d_11::structures::Texture3DDesc {
    let mip_levels = mips(desc.mip_levels, desc.width.max(desc.height).max(desc.depth)).unwrap_or(desc.mip_levels);
    direct3d_11::structures::Texture3DDesc { mip_levels, ..*desc }
}

/// Returns how many subresources a 3D texture has: its mips.
pub fn subresources_texture_3d(desc: &direct3d_11::structures::Texture3DDesc) -> Option<usize> {
    usize::try_from(mips(desc.mip_levels, desc.width.max(desc.height).max(desc.depth))?).ok()
}

/// Returns a 3D texture subresource's width, height and depth.
fn size_texture_3d(desc: &direct3d_11::structures::Texture3DDesc, index: usize) -> Option<(u32, u32, u32)> {
    let levels = mips(desc.mip_levels, desc.width.max(desc.height).max(desc.depth))?;
    let mip = u32::try_from(index).ok().filter(|mip| *mip < levels)?;
    Some((mip_side(desc.width, mip), mip_side(desc.height, mip), mip_side(desc.depth, mip)))
}

/// Returns whether the serving glue lends `length` bytes for a subresource of the texture.
pub fn within_served_bound_texture_3d(desc: &direct3d_11::structures::Texture3DDesc, subresource: u32, length: usize) -> bool {
    let padded = usize::try_from(subresource).ok().and_then(|index| padded_extent(desc.format, size_texture_3d(desc, index)?));
    padded.is_some_and(|padded| length <= padded.max(LEAST_SERVED_BOUND))
}

/// Returns the length of a 3D texture subresource's initial bytes: its slices at the slice pitch, rows at the pitch.
pub fn initial_length_texture_3d(desc: &direct3d_11::structures::Texture3DDesc, index: usize, pitch: u32, slice_pitch: u32) -> Option<usize> {
    subresource_extent(desc.format, size_texture_3d(desc, index)?, pitch, slice_pitch)
}

/// Returns the length of a 3D texture subresource's mapped bytes: its slices and rows at the pitches the system gave.
pub fn mapped_length_texture_3d(desc: &direct3d_11::structures::Texture3DDesc, subresource: u32, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let slices = subresource_extent(desc.format, size_texture_3d(desc, usize::try_from(subresource).ok()?)?, row_pitch, depth_pitch)?;
    at_least_pitches(slices, row_pitch, depth_pitch)
}

/// Returns the length of the bytes an upload into a subresource of the texture reads: the box's slices and rows of
/// blocks (the whole subresource's for none) at the pitches.
pub fn upload_length_texture_3d(desc: &direct3d_11::structures::Texture3DDesc, subresource: u32, region: Option<&direct3d_11::structures::Box>, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let full = size_texture_3d(desc, usize::try_from(subresource).ok()?)?;
    upload_extent(desc.format, region_of(full, region)?, row_pitch, depth_pitch)
}
// end of shape

/// The least memory the serving glue lends for any subresource: what a small resource's padded rows may need.
const LEAST_SERVED_BOUND: usize = 1 << 16;

/// The widest row alignment the serving glue accepts from an implementation: a page.
const ROW_ALIGNMENT_BOUND: u64 = 4096;

/// Returns whether memory of `length` bytes for a subresource whose rows hold `packed` bytes is within what the serving
/// glue lends: twice the packed bytes, or 64 KiB for a small one. An implementation's pitches past that are refused,
/// so that no implementation makes the glue lend more than its resources hold.
fn within_served_bound(packed: Option<usize>, length: usize) -> bool {
    packed.is_some_and(|packed| length <= packed.saturating_mul(2).max(LEAST_SERVED_BOUND))
}

/// Returns the bytes of a subresource whose rows start a page apart at most: the most the serving glue lends for it.
fn padded_extent(format: directx_graphics_infrastructure::enumerations::Format, (width, height, depth): (u32, u32, u32)) -> Option<usize> {
    let block = block(format)?;
    let row = (u64::from(width.div_ceil(block.width)) * u64::from(block.bytes)).next_multiple_of(ROW_ALIGNMENT_BOUND);
    let all = row.checked_mul(u64::from(height.div_ceil(block.height).max(1)))?.checked_mul(u64::from(depth.max(1)))?;
    usize::try_from(all).ok().filter(|all| *all <= most_bytes())
}

/// Returns `length` raised to the pitches the system gave, which programs fill whole, if within the largest resource.
fn at_least_pitches(length: usize, row_pitch: u32, depth_pitch: u32) -> Option<usize> {
    let pitches = usize::try_from(row_pitch.max(depth_pitch)).ok()?;
    let length = length.max(pitches);
    (length <= most_bytes()).then_some(length)
}
