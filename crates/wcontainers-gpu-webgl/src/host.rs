//! Textures kept on the host as well (`HostCopyKept`): formats GLES 3.0 cannot read back (RGB9E5,
//! the three-channel 32-bit ones and the block-compressed ones), whose read backs come from the bytes written. Block-compressed
//! textures without their WebGL extension are stored decoded (`BlockDecoded`).

use std::cell::RefCell;
use wcontainers_gpu::{Format, TextureDesc, TextureDimension};

/// Whether textures of `format` keep a host copy: GPU writes never reach them (they are not
/// renderable), and GL cannot read them back.
pub fn kept(format: Format) -> bool {
    use Format as F;
    matches!(
        format,
        F::Rgb9e5Float | F::Rgb32Float | F::Rgb32Uint | F::Rgb32Sint
    ) || format.block().0 > 1
}

/// Every level of a texture, each its slices (layers, or depth slices in 3D) of tightly packed rows
/// of blocks.
#[derive(Debug)]
pub struct HostCopy {
    bytes: RefCell<Vec<u8>>,
    /// Where each level starts.
    levels: Vec<usize>,
}

/// One level's shape: bytes per row of blocks, rows of blocks, slices.
fn shape(desc: &TextureDesc, mip: u32) -> (usize, usize, usize) {
    let (bw, bh, bytes) = desc.format.block();
    let w = (desc.width >> mip).max(1).div_ceil(bw);
    let h = (desc.height >> mip).max(1).div_ceil(bh);
    let slices = match desc.dimension {
        TextureDimension::D3 => (desc.depth_or_layers >> mip).max(1),
        _ => desc.depth_or_layers,
    };
    ((w * bytes) as usize, h as usize, slices as usize)
}

/// A region of one level: first block column and row, first slice, and the blocks and slices it
/// covers, cut to the level.
struct Blocks {
    x: usize,
    y: usize,
    z: usize,
    row: usize,
    rows: usize,
    slices: usize,
}

impl HostCopy {
    pub fn new(desc: &TextureDesc) -> HostCopy {
        let mut levels = Vec::with_capacity(desc.mip_levels as usize);
        let mut size = 0;
        for mip in 0..desc.mip_levels {
            levels.push(size);
            let (row, rows, slices) = shape(desc, mip);
            size += row * rows * slices;
        }
        HostCopy {
            bytes: RefCell::new(vec![0; size]),
            levels,
        }
    }

    fn blocks(desc: &TextureDesc, mip: u32, origin: [u32; 3], extent: [u32; 3]) -> Blocks {
        let (bw, bh, bytes) = desc.format.block();
        let (row, rows, slices) = shape(desc, mip);
        let (x, y) = ((origin[0] / bw) as usize, (origin[1] / bh) as usize);
        let z = origin[2] as usize;
        Blocks {
            x,
            y,
            z,
            row: ((extent[0].div_ceil(bw) * bytes) as usize)
                .min(row.saturating_sub(x * bytes as usize)),
            rows: (extent[1].div_ceil(bh) as usize).min(rows.saturating_sub(y)),
            slices: (extent[2] as usize).min(slices.saturating_sub(z)),
        }
    }

    /// Writes a region from `data`, `row_pitch` bytes between rows of blocks and `rows_per_layer`
    /// rows between slices. `origin` and `extent` are in texels, with slices in the third place.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        desc: &TextureDesc,
        mip: u32,
        origin: [u32; 3],
        extent: [u32; 3],
        data: &[u8],
        row_pitch: usize,
        rows_per_layer: usize,
    ) {
        let b = Self::blocks(desc, mip, origin, extent);
        let (row, rows, _) = shape(desc, mip);
        let bytes_per_block = desc.format.block().2 as usize;
        let mut host = self.bytes.borrow_mut();
        for s in 0..b.slices {
            for r in 0..b.rows {
                let from = s * row_pitch * rows_per_layer + r * row_pitch;
                let to = self.levels[mip as usize]
                    + (b.z + s) * row * rows
                    + (b.y + r) * row
                    + b.x * bytes_per_block;
                host[to..to + b.row].copy_from_slice(&data[from..from + b.row]);
            }
        }
    }

    /// Takes every byte of `other`, a host copy of the same shape.
    pub fn copy_from(&self, other: &HostCopy) {
        self.bytes
            .borrow_mut()
            .copy_from_slice(&other.bytes.borrow());
    }

    /// Reads a region into `dst` as tightly packed rows of blocks; returns the bytes written.
    pub fn read(
        &self,
        desc: &TextureDesc,
        mip: u32,
        origin: [u32; 3],
        extent: [u32; 3],
        dst: &mut [u8],
    ) -> usize {
        let b = Self::blocks(desc, mip, origin, extent);
        let (row, rows, _) = shape(desc, mip);
        let bytes_per_block = desc.format.block().2 as usize;
        let host = self.bytes.borrow();
        let mut at = 0;
        for s in 0..b.slices {
            for r in 0..b.rows {
                let from = self.levels[mip as usize]
                    + (b.z + s) * row * rows
                    + (b.y + r) * row
                    + b.x * bytes_per_block;
                dst[at..at + b.row].copy_from_slice(&host[from..from + b.row]);
                at += b.row;
            }
        }
        at
    }
}

/// A block-compressed format's GL format with the WebGL extension that holds it, and the format it is
/// stored in decoded without that extension.
pub fn compressed(format: Format) -> Option<(u32, &'static str, Format)> {
    use Format as F;
    const S3TC: &str = "WEBGL_compressed_texture_s3tc";
    const S3TC_SRGB: &str = "WEBGL_compressed_texture_s3tc_srgb";
    const RGTC: &str = "EXT_texture_compression_rgtc";
    const BPTC: &str = "EXT_texture_compression_bptc";
    Some(match format {
        F::Bc1RgbaUnorm => (0x83F1, S3TC, F::Rgba8Unorm),
        F::Bc2RgbaUnorm => (0x83F2, S3TC, F::Rgba8Unorm),
        F::Bc3RgbaUnorm => (0x83F3, S3TC, F::Rgba8Unorm),
        F::Bc1RgbaUnormSrgb => (0x8C4D, S3TC_SRGB, F::Rgba8UnormSrgb),
        F::Bc2RgbaUnormSrgb => (0x8C4E, S3TC_SRGB, F::Rgba8UnormSrgb),
        F::Bc3RgbaUnormSrgb => (0x8C4F, S3TC_SRGB, F::Rgba8UnormSrgb),
        F::Bc4RUnorm => (0x8DBB, RGTC, F::R8Unorm),
        F::Bc4RSnorm => (0x8DBC, RGTC, F::R8Snorm),
        F::Bc5RgUnorm => (0x8DBD, RGTC, F::Rg8Unorm),
        F::Bc5RgSnorm => (0x8DBE, RGTC, F::Rg8Snorm),
        F::Bc7RgbaUnorm => (0x8E8C, BPTC, F::Rgba8Unorm),
        F::Bc7RgbaUnormSrgb => (0x8E8D, BPTC, F::Rgba8UnormSrgb),
        F::Bc6hRgbFloat => (0x8E8E, BPTC, F::Rgba16Float),
        F::Bc6hRgbUfloat => (0x8E8F, BPTC, F::Rgba16Float),
        _ => return None,
    })
}

/// Decodes tightly packed blocks of `format`, `blocks_wide` to a row, into texels of the format
/// [`compressed`] names, tightly packed rows four texels high per row of blocks.
pub fn decode(format: Format, blocks: &[u8], blocks_wide: usize) -> Vec<u8> {
    use Format as F;
    let (_, _, block_bytes) = format.block();
    let block_bytes = block_bytes as usize;
    let Some((_, _, decoded)) = compressed(format) else {
        return Vec::new();
    };
    let texel = decoded.block().2 as usize;
    let pitch = blocks_wide * 4 * texel;
    let rows = blocks.len() / block_bytes / blocks_wide.max(1);
    let mut out = vec![0u8; rows * 4 * pitch];
    let mut halves = [0u16; 4 * 4 * 3];
    for (i, block) in blocks.chunks_exact(block_bytes).enumerate() {
        let (bx, by) = (i % blocks_wide, i / blocks_wide);
        let at = by * 4 * pitch + bx * 4 * texel;
        let dst = &mut out[at..];
        match format {
            F::Bc1RgbaUnorm | F::Bc1RgbaUnormSrgb => bcdec_rs::bc1(block, dst, pitch),
            F::Bc2RgbaUnorm | F::Bc2RgbaUnormSrgb => bcdec_rs::bc2(block, dst, pitch),
            F::Bc3RgbaUnorm | F::Bc3RgbaUnormSrgb => bcdec_rs::bc3(block, dst, pitch),
            F::Bc4RUnorm | F::Bc4RSnorm => bcdec_rs::bc4(block, dst, pitch, format == F::Bc4RSnorm),
            F::Bc5RgUnorm | F::Bc5RgSnorm => {
                bcdec_rs::bc5(block, dst, pitch, format == F::Bc5RgSnorm)
            }
            F::Bc7RgbaUnorm | F::Bc7RgbaUnormSrgb => bcdec_rs::bc7(block, dst, pitch),
            _ => {
                // BC6H decodes to RGB halves; GL takes them as RGBA16F with alpha one.
                bcdec_rs::bc6h_half(block, &mut halves, 12, format == F::Bc6hRgbFloat);
                for y in 0..4 {
                    for x in 0..4 {
                        let rgb = &halves[y * 12 + x * 3..y * 12 + x * 3 + 3];
                        let px = &mut dst[y * pitch + x * 8..y * pitch + x * 8 + 8];
                        for (c, h) in rgb.iter().chain(&[0x3c00]).enumerate() {
                            px[c * 2..c * 2 + 2].copy_from_slice(&h.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
    out
}
