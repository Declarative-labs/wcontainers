//! The rules of lengths.rs against the reference (lengths_reference.rs), which shares no code with them: every rule,
//! every class of format, every dimension, boxes and pitches, and cases worked out by hand from the pages it cites.

use crate::lengths_reference::{Resource, boxes, buffer_desc, every_format, pitches, reference_initial, reference_lent, reference_mapped, reference_upload, region, row_and_rows, texture_1d_desc, texture_2d_desc, texture_3d_desc};

/// The four rules of lengths.rs for a resource, called with its description.
struct Rules {
    upload: Option<usize>,
    initial: Option<usize>,
    mapped: Option<usize>,
    lent: Option<usize>,
}

fn rules(resource: Resource, index: u32, region: Option<&direct3d_11::structures::Box>, row_pitch: u32, depth_pitch: u32) -> Rules {
    let at = usize::try_from(index).unwrap_or(usize::MAX);
    match resource {
        Resource::Buffer(width) => {
            let desc = buffer_desc(width);
            Rules {
                upload: crate::lengths::upload_length_buffer(&desc, index, region, row_pitch, depth_pitch),
                initial: crate::lengths::initial_length_buffer(&desc, at, row_pitch, depth_pitch),
                mapped: crate::lengths::mapped_length_buffer(&desc, index, row_pitch, depth_pitch),
                lent: crate::lengths::lent_length_buffer(&desc, index, row_pitch, depth_pitch),
            }
        }
        Resource::Texture1D { width, mips, array, format } => {
            let desc = texture_1d_desc(width, mips, array, format);
            Rules {
                upload: crate::lengths::upload_length_texture_1d(&desc, index, region, row_pitch, depth_pitch),
                initial: crate::lengths::initial_length_texture_1d(&desc, at, row_pitch, depth_pitch),
                mapped: crate::lengths::mapped_length_texture_1d(&desc, index, row_pitch, depth_pitch),
                lent: crate::lengths::lent_length_texture_1d(&desc, index, row_pitch, depth_pitch),
            }
        }
        Resource::Texture2D { width, height, mips, array, format } => {
            let desc = texture_2d_desc(width, height, mips, array, format);
            Rules {
                upload: crate::lengths::upload_length_texture_2d(&desc, index, region, row_pitch, depth_pitch),
                initial: crate::lengths::initial_length_texture_2d(&desc, at, row_pitch, depth_pitch),
                mapped: crate::lengths::mapped_length_texture_2d(&desc, index, row_pitch, depth_pitch),
                lent: crate::lengths::lent_length_texture_2d(&desc, index, row_pitch, depth_pitch),
            }
        }
        Resource::Texture3D { width, height, depth, mips, format } => {
            let desc = texture_3d_desc(width, height, depth, mips, format);
            Rules {
                upload: crate::lengths::upload_length_texture_3d(&desc, index, region, row_pitch, depth_pitch),
                initial: crate::lengths::initial_length_texture_3d(&desc, at, row_pitch, depth_pitch),
                mapped: crate::lengths::mapped_length_texture_3d(&desc, index, row_pitch, depth_pitch),
                lent: crate::lengths::lent_length_texture_3d(&desc, index, row_pitch, depth_pitch),
            }
        }
    }
}

/// Compares every rule with the reference over every subresource of `resource`, every box and every pitch; returns how
/// many comparisons were made.
fn compare(resource: Resource) -> usize {
    let mut compared = 0;
    for index in 0..=resource.subresources() {
        let sides = resource.subresource(index).map_or((1, 1, 1), |(sides, _)| sides);
        let (row, rows) = resource
            .subresource(index)
            .and_then(|(sides, element)| row_and_rows(element, (0, sides.0), (0, sides.1)))
            .unwrap_or((16, 1));
        for (row_pitch, depth_pitch) in pitches(row, rows) {
            for region in boxes(sides) {
                let found = rules(resource, index, region.as_ref(), row_pitch, depth_pitch);
                let case = format!("{resource:?} subresource {index} box {region:?} pitches {row_pitch} {depth_pitch}");
                assert_eq!(found.upload, reference_upload(resource, index, region.as_ref(), row_pitch, depth_pitch), "upload: {case}");
                compared += 1;
            }
            let found = rules(resource, index, None, row_pitch, depth_pitch);
            let case = format!("{resource:?} subresource {index} pitches {row_pitch} {depth_pitch}");
            assert_eq!(found.initial, reference_initial(resource, index, row_pitch, depth_pitch), "initial: {case}");
            assert_eq!(found.mapped, reference_mapped(resource, index, row_pitch, depth_pitch), "mapped: {case}");
            assert_eq!(found.lent, reference_lent(resource, index, row_pitch, depth_pitch), "lent: {case}");
            compared += 3;
        }
    }
    compared
}

#[test]
fn upload_length_buffer_matches_the_reference() {
    let mut compared = 0;
    for width in [1, 3, 16, 255, 256, 4097, 65536, 128 * 1024 * 1024, u32::MAX] {
        compared += compare(Resource::Buffer(width));
    }
    assert!(compared > 0);
}

#[test]
fn initial_and_mapped_lengths_of_a_texture_1d_match_the_reference_for_every_format() {
    let mut compared = 0;
    for format in every_format() {
        compared += compare(Resource::Texture1D { width: 300, mips: 0, array: 3, format });
    }
    for (width, mips, array) in [(1, 1, 1), (2, 2, 1), (16384, 3, 2), (7, 0, 5)] {
        compared += compare(Resource::Texture1D { width, mips, array, format: 2 });
    }
    assert!(compared > 0);
}

#[test]
fn upload_initial_and_mapped_lengths_of_a_texture_2d_match_the_reference_for_every_format() {
    let mut compared = 0;
    for format in every_format() {
        compared += compare(Resource::Texture2D { width: 37, height: 23, mips: 0, array: 2, format });
    }
    // Plain, packed pairs, block-compressed and depth formats at other shapes: one level, odd sides, a side of one.
    for format in [2, 6, 10, 28, 40, 45, 55, 61, 68, 71, 77, 98, 20] {
        for (width, height, mips, array) in [(1, 1, 1, 1), (5, 3, 1, 1), (64, 64, 0, 1), (130, 1, 0, 4), (1, 130, 2, 2), (4096, 2, 1, 1)] {
            compared += compare(Resource::Texture2D { width, height, mips, array, format });
        }
    }
    assert!(compared > 0);
}

#[test]
fn upload_initial_and_mapped_lengths_of_a_texture_3d_match_the_reference_for_every_format() {
    let mut compared = 0;
    for format in every_format() {
        compared += compare(Resource::Texture3D { width: 19, height: 13, depth: 7, mips: 0, format });
    }
    for format in [2, 28, 61, 69, 71, 83, 95] {
        for (width, height, depth, mips) in [(1, 1, 1, 1), (8, 8, 8, 0), (2, 3, 4, 1), (33, 5, 2, 0), (4, 4, 2048, 1)] {
            compared += compare(Resource::Texture3D { width, height, depth, mips, format });
        }
    }
    assert!(compared > 0);
}

/// Cases worked out by hand from the pages, each number written out, so that the reference itself is checked.
#[test]
fn lengths_worked_out_by_hand() {
    // UpdateSubresource's example: a 2 x 3 x 4 volume of R32G32B32A32_FLOAT, row pitch 32, depth pitch 96: four
    // slices of 96 bytes (its full chain: three levels).
    let volume = texture_3d_desc(2, 3, 4, 0, 2);
    assert_eq!(crate::lengths::upload_length_texture_3d(&volume, 0, None, 32, 96), Some(384));
    assert_eq!(crate::lengths::initial_length_texture_3d(&volume, 0, 32, 96), Some(384));
    assert_eq!(crate::lengths::mapped_length_texture_3d(&volume, 0, 32, 96), Some(384));
    // The same volume at padded pitches 256 and 1024: three slices of 1024, two rows of 256, one row of 32.
    assert_eq!(crate::lengths::upload_length_texture_3d(&volume, 0, None, 256, 1024), Some(3 * 1024 + 2 * 256 + 32));
    // Served: every row whole in the last slice (3 * 256), every slice whole (4 * 1024).
    assert_eq!(crate::lengths::lent_length_texture_3d(&volume, 0, 256, 1024), Some(4 * 1024));
    // Its second mip level: 1 x 1 x 2 texels, 16 bytes a row: one slice at 1024 and one row.
    assert_eq!(crate::lengths::mapped_length_texture_3d(&volume, 1, 256, 1024), Some(1024 + 16));
    // UpdateSubresource's 2D example: the box (120, 100) to (200, 220) of a 512 x 512 R8G8B8A8 texture, row pitch 2048:
    // 119 rows at 2048 and one row of 80 texels of 4 bytes.
    let picture = texture_2d_desc(512, 512, 1, 1, 28);
    let example = region(120, 100, 0, 200, 220, 1);
    assert_eq!(crate::lengths::upload_length_texture_2d(&picture, 0, Some(&example), 2048, 0), Some(119 * 2048 + 320));
    // Its 1D example: the box 120 to 200 of a 512-texel texture: 80 texels of 4 bytes, the pitch not used.
    let line = texture_1d_desc(512, 1, 1, 28);
    let example_1d = region(120, 0, 0, 200, 1, 1);
    assert_eq!(crate::lengths::upload_length_texture_1d(&line, 0, Some(&example_1d), 9999, 9999), Some(320));
    // BC1 64 x 64 at a packed pitch: 16 rows of 16 blocks of 8 bytes, 2048 bytes.
    let compressed = texture_2d_desc(64, 64, 0, 1, 71);
    assert_eq!(crate::lengths::initial_length_texture_2d(&compressed, 0, 128, 0), Some(2048));
    // Its level of 2 x 2 texels (index 5) still takes one whole block of 8 bytes.
    assert_eq!(crate::lengths::initial_length_texture_2d(&compressed, 5, 8, 0), Some(8));
    // Its 7 levels end at index 6 (1 x 1): index 7 is past the texture.
    assert_eq!(crate::lengths::initial_length_texture_2d(&compressed, 7, 8, 0), None);
    // A box of BC1 from texel (2, 2) to (6, 6) touches 2 x 2 blocks: one row at 128 and one of 16 bytes.
    assert_eq!(crate::lengths::upload_length_texture_2d(&compressed, 0, Some(&region(2, 2, 0, 6, 6, 1)), 128, 0), Some(128 + 16));
    // BC7 of 13 x 9: 4 blocks a row of 16 bytes, 3 rows of blocks, pitch 64: 2 * 64 + 64.
    let bc7 = texture_2d_desc(13, 9, 1, 1, 98);
    assert_eq!(crate::lengths::mapped_length_texture_2d(&bc7, 0, 64, 999), Some(192));
    // R8G8_B8G8 of 5 texels: 3 pairs of 4 bytes.
    let pairs = texture_1d_desc(5, 1, 1, 68);
    assert_eq!(crate::lengths::initial_length_texture_1d(&pairs, 0, 0, 0), Some(12));
    // A box from texel 1 to 2 of it touches one pair.
    assert_eq!(crate::lengths::upload_length_texture_1d(&pairs, 0, Some(&region(1, 0, 0, 2, 1, 1)), 0, 0), Some(4));
    // D24_UNORM_S8_UINT 10 x 4 at pitch 64: 3 rows at 64 and one of 40.
    let depth = texture_2d_desc(10, 4, 1, 1, 45);
    assert_eq!(crate::lengths::initial_length_texture_2d(&depth, 0, 64, 0), Some(232));
    // Served at that pitch: 4 whole rows, 256; at a depth pitch of 4096, 4096.
    assert_eq!(crate::lengths::lent_length_texture_2d(&depth, 0, 64, 0), Some(256));
    assert_eq!(crate::lengths::lent_length_texture_2d(&depth, 0, 64, 4096), Some(4096));
    // D32_FLOAT_S8X24_UINT: 8 bytes a texel. D16_UNORM: 2.
    assert_eq!(crate::lengths::initial_length_texture_2d(&texture_2d_desc(3, 2, 1, 1, 20), 0, 24, 0), Some(48));
    assert_eq!(crate::lengths::initial_length_texture_2d(&texture_2d_desc(3, 2, 1, 1, 55), 0, 6, 0), Some(12));
    // A 2D array of 3 slices and 4 levels: index 6 is level 2 of slice 1, 16 x 4 texels of R32_FLOAT: 3 * 100 + 64.
    let array = texture_2d_desc(64, 16, 4, 3, 41);
    assert_eq!(crate::lengths::initial_length_texture_2d(&array, 6, 100, 0), Some(364));
    assert_eq!(crate::lengths::initial_length_texture_2d(&array, 12, 100, 0), None);
    // A buffer: its byte width, whatever the pitches; a box of bytes 3 to 10 reads 7; a box past it is refused.
    let buffer = buffer_desc(100);
    assert_eq!(crate::lengths::initial_length_buffer(&buffer, 0, 7, 9), Some(100));
    assert_eq!(crate::lengths::mapped_length_buffer(&buffer, 0, 4096, 4096), Some(100));
    assert_eq!(crate::lengths::lent_length_buffer(&buffer, 0, 4096, 4096), Some(4096));
    assert_eq!(crate::lengths::upload_length_buffer(&buffer, 0, Some(&region(3, 0, 0, 10, 1, 1)), 0, 0), Some(7));
    assert_eq!(crate::lengths::upload_length_buffer(&buffer, 0, Some(&region(3, 0, 0, 101, 1, 1)), 0, 0), None);
    // A 2D texture's depth pitch is not used for what a program views.
    assert_eq!(crate::lengths::mapped_length_texture_2d(&depth, 0, 64, u32::MAX), Some(232));
    // NV12, a planar format, and YUY2, a video format: refused.
    assert_eq!(crate::lengths::initial_length_texture_2d(&texture_2d_desc(64, 64, 1, 1, 103), 0, 64, 0), None);
    assert_eq!(crate::lengths::mapped_length_texture_2d(&texture_2d_desc(64, 64, 1, 1, 107), 0, 128, 0), None);
    // Up to the largest resource and past it: 16384 x 8192 texels of 16 bytes at a packed pitch are exactly 2048 MiB, one
    // row more is past it.
    assert_eq!(crate::lengths::initial_length_texture_2d(&texture_2d_desc(16384, 8192, 1, 1, 2), 0, 262_144, 0), Some(2048 * 1024 * 1024));
    assert_eq!(crate::lengths::initial_length_texture_2d(&texture_2d_desc(16384, 8193, 1, 1, 2), 0, 262_144, 0), None);
}

/// D3D11_QUERY_DESC of a kind, flags 0.
fn query(kind: i32) -> direct3d_11::structures::QueryDesc {
    direct3d_11::structures::QueryDesc { query: direct3d_11::enumerations::QueryKind(kind), misc_flags: 0 }
}

/// Each query kind's data size, as Microsoft's D3D11_QUERY page names its data and d3d11.h lays the structures out
/// (https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query): written out kind by kind, not taken
/// from lengths.rs.
#[test]
fn data_size_of_every_query_kind_is_its_documented_structure() {
    // D3D11_QUERY_EVENT: BOOL.
    assert_eq!(crate::lengths::data_size_of_query(&query(0)), 4);
    // D3D11_QUERY_OCCLUSION: UINT64.
    assert_eq!(crate::lengths::data_size_of_query(&query(1)), 8);
    // D3D11_QUERY_TIMESTAMP: UINT64.
    assert_eq!(crate::lengths::data_size_of_query(&query(2)), 8);
    // D3D11_QUERY_TIMESTAMP_DISJOINT: D3D11_QUERY_DATA_TIMESTAMP_DISJOINT, a UINT64 Frequency and a BOOL Disjoint,
    // padded to the UINT64's alignment.
    assert_eq!(crate::lengths::data_size_of_query(&query(3)), 16);
    // D3D11_QUERY_PIPELINE_STATISTICS: D3D11_QUERY_DATA_PIPELINE_STATISTICS, eleven UINT64.
    assert_eq!(crate::lengths::data_size_of_query(&query(4)), 88);
    // D3D11_QUERY_OCCLUSION_PREDICATE: BOOL.
    assert_eq!(crate::lengths::data_size_of_query(&query(5)), 4);
    // D3D11_QUERY_SO_STATISTICS: D3D11_QUERY_DATA_SO_STATISTICS, two UINT64.
    assert_eq!(crate::lengths::data_size_of_query(&query(6)), 16);
    // D3D11_QUERY_SO_OVERFLOW_PREDICATE: BOOL.
    assert_eq!(crate::lengths::data_size_of_query(&query(7)), 4);
    // D3D11_QUERY_SO_STATISTICS_STREAM0 to STREAM3: D3D11_QUERY_DATA_SO_STATISTICS.
    for kind in 8..=11 {
        assert_eq!(crate::lengths::data_size_of_query(&query(kind)), 16, "kind {kind}");
    }
    // D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM0 to STREAM3: BOOL.
    for kind in 12..=15 {
        assert_eq!(crate::lengths::data_size_of_query(&query(kind)), 4, "kind {kind}");
    }
    // A kind the header does not declare has no data.
    for kind in [-1, 16, 255, i32::MAX] {
        assert_eq!(crate::lengths::data_size_of_query(&query(kind)), 0, "kind {kind}");
    }
}

/// A buffer is D3D11_RESOURCE_DIMENSION_BUFFER (1), and its description is within d3d11.h's ranges only with a byte
/// width from 1 to the largest resource (2048 MiB) and one of the four declared usages (D3D11_USAGE_DEFAULT,
/// _IMMUTABLE, _DYNAMIC, _STAGING: 0 to 3).
#[test]
fn a_buffer_is_of_the_buffer_dimension_and_checked_by_its_documented_ranges() {
    const MIB: u32 = 1 << 20;
    let desc = |width: u32, usage: i32| direct3d_11::structures::BufferDesc { byte_width: width, usage: direct3d_11::enumerations::Usage(usage), bind_flags: 0, cpu_access_flags: 0, misc_flags: 0, structure_byte_stride: 0 };
    assert_eq!(crate::lengths::type_of_buffer(&desc(256, 0)), direct3d_11::enumerations::ResourceDimension(1));
    for (width, usage, checked) in [(0, 0, false), (1, 0, true), (256, 1, true), (256, 2, true), (256, 3, true), (256, 4, false), (256, -1, false), (2048 * MIB, 0, true), (2048 * MIB + 1, 0, false), (u32::MAX, 3, false)] {
        assert_eq!(crate::lengths::checked_buffer(&desc(width, usage)), checked, "width {width} usage {usage}");
    }
}
