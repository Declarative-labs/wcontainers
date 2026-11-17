//! Block-compressed (BC) textures on a device without `texture-compression-bc` (plan 4.12,
//! `BlockDecoded`): the GPU texture holds the decoded texels, sampled and filtered as the blocks
//! would be, and the back end keeps the blocks on the host, updated by every write and copy, to
//! answer read backs and copies in blocks.

use wcontainers_gpu::{
    CopyBufferTexture, Extent, Format, ReadSource, SlowPath, TextureDesc, TextureDimension,
    TextureRegion, WriteMode,
};

use crate::{Readback, WebGpu};

/// The format a BC format is stored in when decoded: 8-bit RGBA (sRGB kept) for BC1 to BC3 and
/// BC7, 16-bit floats for BC4 and BC5 (their interpolated values) and BC6H (what it decodes to).
pub(crate) fn decoded(format: Format) -> Option<Format> {
    use Format::*;
    Some(match format {
        Bc1RgbaUnorm | Bc2RgbaUnorm | Bc3RgbaUnorm | Bc7RgbaUnorm => Rgba8Unorm,
        Bc1RgbaUnormSrgb | Bc2RgbaUnormSrgb | Bc3RgbaUnormSrgb | Bc7RgbaUnormSrgb => Rgba8UnormSrgb,
        Bc4RUnorm | Bc4RSnorm => R16Float,
        Bc5RgUnorm | Bc5RgSnorm => Rg16Float,
        Bc6hRgbUfloat | Bc6hRgbFloat => Rgba16Float,
        _ => return None,
    })
}

/// The blocks of a new texture, all zero: one array of block rows per level and layer, or per level
/// of a 3D texture with its slices in order.
pub(crate) fn zeroed(desc: &TextureDesc) -> Vec<Vec<u8>> {
    let bytes = desc.format.block().2 as usize;
    let layers = if desc.dimension == TextureDimension::D3 {
        1
    } else {
        desc.depth_or_layers
    };
    (0..desc.mip_levels)
        .flat_map(|mip| {
            let (w, h, slices) = level(desc, mip);
            (0..layers).map(move |_| vec![0; (w * h * slices) as usize * bytes])
        })
        .collect()
}

/// Blocks across and down one level, and its slices (1 but for 3D).
fn level(desc: &TextureDesc, mip: u32) -> (u32, u32, u32) {
    let (bw, bh, _) = desc.format.block();
    let size = |n: u32| (n >> mip).max(1);
    let slices = if desc.dimension == TextureDimension::D3 {
        size(desc.depth_or_layers)
    } else {
        1
    };
    (
        size(desc.width).div_ceil(bw),
        size(desc.height).div_ceil(bh),
        slices,
    )
}

/// A region in blocks: its level, the subresource and slice of each of its layers, its first block,
/// and its blocks across and down.
struct Area {
    mip: u32,
    first: (u32, u32),
    cols: u32,
    rows: u32,
    layers: Vec<(usize, u32)>,
}

impl Area {
    fn of<B: wcontainers_gpu::Backend>(
        desc: &TextureDesc,
        region: &TextureRegion<'_, B>,
        extent: Extent,
    ) -> Result<Area, &'static str> {
        let (bw, bh, _) = desc.format.block();
        let (o, mip) = (region.origin, region.mip);
        if !o.x.is_multiple_of(bw) || !o.y.is_multiple_of(bh) {
            return Err("a region of a block-compressed texture not on block edges");
        }
        let (w, h, slices) = level(desc, mip);
        let area = Area {
            mip,
            first: (o.x / bw, o.y / bh),
            cols: extent.width.div_ceil(bw),
            rows: extent.height.div_ceil(bh),
            layers: (0..extent.depth_or_layers)
                .map(|l| match desc.dimension {
                    TextureDimension::D3 => (mip as usize, o.z + l),
                    TextureDimension::D2 => {
                        ((mip * desc.depth_or_layers + region.layer + l) as usize, 0)
                    }
                })
                .collect(),
        };
        let past = area.first.0 + area.cols > w
            || area.first.1 + area.rows > h
            || area.layers.iter().any(|&(sub, slice)| {
                slice >= slices || sub >= (desc.mip_levels * desc.depth_or_layers) as usize
            });
        if past {
            return Err("a region past a block-compressed texture's level");
        }
        Ok(area)
    }

    /// Where row `r` of layer `l` starts in its subresource, in bytes.
    fn at(&self, desc: &TextureDesc, l: usize, r: u32) -> usize {
        let (w, h, _) = level(desc, self.mip);
        let slice = self.layers[l].1;
        ((slice * h + self.first.1 + r) * w + self.first.0) as usize
            * desc.format.block().2 as usize
    }
}

/// Stores blocks from `data` (rows `row_pitch` bytes apart, layers `rows_per_layer` rows apart).
fn put(
    desc: &TextureDesc,
    blocks: &mut [Vec<u8>],
    area: &Area,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
) -> Result<(), &'static str> {
    let row = (area.cols * desc.format.block().2) as usize;
    for (l, &(sub, _)) in area.layers.iter().enumerate() {
        for r in 0..area.rows {
            let from = (l * rows_per_layer as usize + r as usize) * row_pitch as usize;
            let src = data
                .get(from..from + row)
                .ok_or("a block-compressed copy or write with fewer bytes than its extent")?;
            let to = area.at(desc, l, r);
            blocks[sub][to..to + row].copy_from_slice(src);
        }
    }
    Ok(())
}

/// The blocks of an area, rows packed.
fn take(desc: &TextureDesc, blocks: &[Vec<u8>], area: &Area) -> Vec<u8> {
    let row = (area.cols * desc.format.block().2) as usize;
    let mut out = Vec::with_capacity(row * (area.rows as usize) * area.layers.len());
    for (l, &(sub, _)) in area.layers.iter().enumerate() {
        for r in 0..area.rows {
            let from = area.at(desc, l, r);
            out.extend_from_slice(&blocks[sub][from..from + row]);
        }
    }
    out
}

/// Decodes one block into 4 by 4 texels of `decoded(format)`, rows packed.
fn decode_block(format: Format, block: &[u8], out: &mut [u8]) {
    use Format::*;
    let signed = matches!(format, Bc4RSnorm | Bc5RgSnorm | Bc6hRgbFloat);
    let halves = |values: &[f32], out: &mut [u8]| {
        for (v, o) in values.iter().zip(out.as_chunks_mut::<2>().0) {
            *o = half::f16::from_f32(*v).to_le_bytes();
        }
    };
    match format {
        Bc1RgbaUnorm | Bc1RgbaUnormSrgb => bcdec_rs::bc1(block, out, 16),
        Bc2RgbaUnorm | Bc2RgbaUnormSrgb => bcdec_rs::bc2(block, out, 16),
        Bc3RgbaUnorm | Bc3RgbaUnormSrgb => bcdec_rs::bc3(block, out, 16),
        Bc7RgbaUnorm | Bc7RgbaUnormSrgb => bcdec_rs::bc7(block, out, 16),
        Bc4RUnorm | Bc4RSnorm => {
            let mut v = [0f32; 16];
            bcdec_rs::bc4_float(block, &mut v, 4, signed);
            halves(&v, out);
        }
        Bc5RgUnorm | Bc5RgSnorm => {
            let mut v = [0f32; 32];
            bcdec_rs::bc5_float(block, &mut v, 8, signed);
            halves(&v, out);
        }
        Bc6hRgbUfloat | Bc6hRgbFloat => {
            let mut rgb = [0u16; 48];
            bcdec_rs::bc6h_half(block, &mut rgb, 12, signed);
            let one = half::f16::ONE.to_bits();
            let texels = out.as_chunks_mut::<2>().0.as_chunks_mut::<4>().0;
            for (t, o) in rgb.as_chunks::<3>().0.iter().zip(texels) {
                *o = [t[0], t[1], t[2], one].map(u16::to_le_bytes);
            }
        }
        _ => unreachable!("only BC formats are decoded"),
    }
}

/// Decodes packed blocks (`cols` by `rows` per layer) into `width` by `height` texels per layer.
fn decode(format: Format, packed: &[u8], cols: u32, rows: u32, width: u32, height: u32) -> Vec<u8> {
    let bytes = format.block().2 as usize;
    let texel = decoded(format).map_or(0, |d| d.block().2) as usize;
    let (cols, rows, width, height) = (
        cols as usize,
        rows as usize,
        width as usize,
        height as usize,
    );
    let layers = packed.len() / (cols * rows * bytes).max(1);
    let mut out = vec![0u8; width * height * layers * texel];
    let mut tmp = [0u8; 16 * 8];
    for l in 0..layers {
        for by in 0..rows {
            for bx in 0..cols {
                let at = ((l * rows + by) * cols + bx) * bytes;
                decode_block(format, &packed[at..at + bytes], &mut tmp);
                for y in 0..4.min(height.saturating_sub(by * 4)) {
                    let n = 4.min(width.saturating_sub(bx * 4)) * texel;
                    let to = ((l * height + by * 4 + y) * width + bx * 4) * texel;
                    out[to..to + n].copy_from_slice(&tmp[y * 4 * texel..y * 4 * texel + n]);
                }
            }
        }
    }
    out
}

impl WebGpu {
    /// A write into a decoded texture: its blocks kept, the area decoded and written as texels.
    pub(crate) fn write_blocks(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        let t = &region.texture.0;
        let Some(blocks) = &t.blocks else {
            return self.fail("a block write into a texture holding no blocks");
        };
        let area = match Area::of(&t.desc, region, extent) {
            Ok(a) => a,
            Err(why) => return self.fail(why),
        };
        let packed = {
            let mut blocks = blocks.borrow_mut();
            if let Err(why) = put(&t.desc, &mut blocks, &area, data, row_pitch, rows_per_layer) {
                return self.fail(why);
            }
            take(&t.desc, &blocks, &area)
        };
        self.slow.hit(SlowPath::BlockDecoded);
        // The texels the area covers inside the level, which may end inside a block.
        let size = |n: u32| (n >> region.mip).max(1);
        let width = (area.cols * 4).min(size(t.desc.width) - region.origin.x);
        let height = (area.rows * 4).min(size(t.desc.height) - region.origin.y);
        let texels = decode(t.desc.format, &packed, area.cols, area.rows, width, height);
        let texel = t.held.block().2;
        let extent = Extent {
            width,
            height,
            depth_or_layers: extent.depth_or_layers,
        };
        self.write_texels(region, extent, &texels, width * texel, height, mode);
    }

    /// The blocks of a region of a decoded texture, rows packed.
    fn blocks_of(&mut self, region: &TextureRegion<'_, Self>, extent: Extent) -> Option<Vec<u8>> {
        let t = &region.texture.0;
        let blocks = t.blocks.as_ref()?;
        match Area::of(&t.desc, region, extent) {
            Ok(area) => Some(take(&t.desc, &blocks.borrow(), &area)),
            Err(why) => {
                self.fail(why);
                None
            }
        }
    }

    /// A read back of a decoded texture, answered from its blocks as they are now.
    pub(crate) fn read_blocks(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Option<Readback> {
        let bytes = self.blocks_of(region, extent)?;
        Some(self.host_readback(bytes))
    }

    /// Reads a region back and waits for it: the bytes of a texture the host keeps no copy of.
    fn read_now(&mut self, src: &ReadSource<'_, Self>, len: usize) -> Option<Vec<u8>> {
        self.slow.hit(SlowPath::HostCopyReadBack);
        let rb = match self.read_back_impl(src) {
            Ok(rb) => rb,
            Err(_) => {
                self.fail("a copy source could not be read back");
                return None;
            }
        };
        wcontainers_gpu::Backend::wait_read(self, &rb);
        let mut out = vec![0; len];
        self.finish_read_impl(rb, &mut out);
        Some(out)
    }

    /// A copy from or into a decoded texture, in blocks: from its kept blocks, or from an
    /// uncompressed source of the same block size read back first; into its blocks, decoded, or
    /// into an uncompressed texture as texels. `extent` is in the source's texels.
    pub(crate) fn copy_blocks(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (s, d) = (&src.texture.0, &dst.texture.0);
        let (sbw, sbh, bytes) = s.desc.format.block();
        let (cols, rows) = (extent.width.div_ceil(sbw), extent.height.div_ceil(sbh));
        let packed = if s.blocks.is_some() {
            self.blocks_of(src, extent)
        } else {
            let len = (cols * rows * bytes * extent.depth_or_layers) as usize;
            let region = TextureRegion {
                texture: src.texture,
                mip: src.mip,
                layer: src.layer,
                origin: src.origin,
                stencil: false,
            };
            self.read_now(&ReadSource::Texture { region, extent }, len)
        };
        let Some(packed) = packed else { return };
        let (dbw, dbh, _) = d.desc.format.block();
        let into = Extent {
            width: cols * dbw,
            height: rows * dbh,
            depth_or_layers: extent.depth_or_layers,
        };
        let (pitch, mode) = (cols * bytes, WriteMode::Ordered);
        if d.blocks.is_some() {
            self.write_blocks(dst, into, &packed, pitch, rows, mode);
        } else {
            self.write_texels(dst, into, &packed, pitch, rows, mode);
        }
    }

    /// A copy between a buffer and a decoded texture: into it from the buffer's bytes read back
    /// first; out of it from its kept blocks, written into the buffer in command order.
    pub(crate) fn copy_buffer_blocks(
        &mut self,
        buffer: &CopyBufferTexture<'_, Self>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        to_texture: bool,
    ) {
        let t = &region.texture.0;
        let (bw, bh, bytes) = t.desc.format.block();
        let (row, rows) = (
            u64::from(extent.width.div_ceil(bw) * bytes),
            u64::from(extent.height.div_ceil(bh)),
        );
        let (pitch, per_layer) = (
            u64::from(buffer.row_pitch),
            u64::from(buffer.rows_per_layer),
        );
        let layers = u64::from(extent.depth_or_layers);
        if rows == 0 || layers == 0 {
            return;
        }
        if to_texture {
            let len = pitch * (per_layer * (layers - 1) + rows - 1) + row;
            let source = ReadSource::Buffer {
                buffer: buffer.buffer,
                offset: buffer.offset,
                size: len,
            };
            let Some(data) = self.read_now(&source, len as usize) else {
                return;
            };
            let (pitch, per_layer) = (buffer.row_pitch, buffer.rows_per_layer);
            self.write_blocks(region, extent, &data, pitch, per_layer, WriteMode::Ordered);
            return;
        }
        let Some(packed) = self.blocks_of(region, extent) else {
            return;
        };
        for (i, chunk) in packed.chunks_exact(row as usize).enumerate() {
            let (l, r) = (i as u64 / rows, i as u64 % rows);
            let at = buffer.offset + (l * per_layer + r) * pitch;
            self.write_buffer_impl(buffer.buffer, at, chunk, WriteMode::Ordered);
        }
    }
}
