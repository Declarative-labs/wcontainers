//! Formats WebGPU lacks, held in an 8-bit RGBA format (plan 5.7, `FormatConverted`). Drawn into
//! with only their own channels written: 8-bit alpha as (0, 0, 0, a), and BGRX as BGRA with alpha
//! one. Sampled and filtered only: the 16-bit packed formats (`B5G6R5`, `B5G5R5A1`, `B4G4R4A4`),
//! each channel widened to 8 bits (a draw would keep 8 bits a channel, not 4, 5 or 6), and the
//! two-pixel formats (`R8G8_B8G8`, `G8R8_G8B8`), each pixel its own texel (`BlockDecoded`'s formats,
//! converted the same way). Writes are widened on the host; read backs and copies out are packed
//! again on the host, exactly, since 8 bits keep every narrower value apart.

use wcontainers_gpu::{
    CopyBufferTexture, Extent, Format, ReadSource, SlowPath, TextureRegion, WriteMode,
};

use crate::WebGpu;

/// The format a texture of `format` is held in, where the back end converts it.
pub(crate) fn held(format: Format) -> Option<Format> {
    use Format::*;
    Some(match format {
        A8Unorm | B5g6r5Unorm | Bgr5a1Unorm | Bgra4Unorm | Rgbg8Unorm | Grgb8Unorm => Rgba8Unorm,
        Bgrx8Unorm => Bgra8Unorm,
        Bgrx8UnormSrgb => Bgra8UnormSrgb,
        _ => return None,
    })
}

/// The channels a draw into a converted format writes, where it can be drawn into: the others hold
/// what the format reads as (zero colour for 8-bit alpha, alpha one for BGRX).
pub(crate) fn drawn(format: Format) -> Option<wgpu::ColorWrites> {
    match format {
        Format::A8Unorm => Some(wgpu::ColorWrites::ALPHA),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => Some(wgpu::ColorWrites::COLOR),
        _ => None,
    }
}

/// A clear colour as a converted target keeps it.
pub(crate) fn clear(format: Format, c: wgpu::Color) -> wgpu::Color {
    match format {
        Format::A8Unorm => wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            ..c
        },
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => wgpu::Color { a: 1.0, ..c },
        _ => c,
    }
}

/// Bits of blue, green, red and alpha, from the lowest up, of a 16-bit packed format.
fn fields(format: Format) -> [u32; 4] {
    match format {
        Format::B5g6r5Unorm => [5, 6, 5, 0],
        Format::Bgr5a1Unorm => [5, 5, 5, 1],
        _ => [4, 4, 4, 4],
    }
}

/// Appends the blocks `packed` of `format` widened to texels of its held format: 16-bit channels'
/// bits repeated down to 8 bits, as the RTX 3080's Direct3D 11 loads them (up to 0.72 of an 8-bit
/// step from `v / (2^n - 1)`), a missing alpha one; two texels for each two-pixel block.
pub(crate) fn widen(format: Format, packed: &[u8], out: &mut Vec<u8>) {
    match format {
        Format::A8Unorm => out.extend(packed.iter().flat_map(|&a| [0, 0, 0, a])),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            out.extend(
                packed
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|t| [t[0], t[1], t[2], 255]),
            );
        }
        Format::Rgbg8Unorm | Format::Grgb8Unorm => {
            for p in packed.as_chunks::<4>().0 {
                let [r, g0, b, g1] = match format {
                    Format::Rgbg8Unorm => *p,
                    _ => [p[1], p[0], p[3], p[2]],
                };
                out.extend([r, g0, b, 255, r, g1, b, 255]);
            }
        }
        _ => {
            let f = fields(format);
            for t in packed.as_chunks::<2>().0 {
                let v = u32::from(u16::from_le_bytes(*t));
                let (mut bgra, mut at) = ([255u8; 4], 0);
                for (c, bits) in bgra.iter_mut().zip(f).filter(|(_, bits)| *bits > 0) {
                    let x = (v >> at) & ((1 << bits) - 1);
                    let (mut wide, mut filled) = (0, 0);
                    while filled < 8 {
                        (wide, filled) = (wide << bits | x, filled + bits);
                    }
                    *c = (wide >> (filled - 8)) as u8;
                    at += bits;
                }
                out.extend([bgra[2], bgra[1], bgra[0], bgra[3]]);
            }
        }
    }
}

/// Appends texels of the held format packed into `format`: each channel the nearest value of its
/// bits, BGRX's alpha one, a two-pixel block from two texels (a lone last texel taken for both).
pub(crate) fn pack(format: Format, texels: &[u8], out: &mut Vec<u8>) {
    let texels = texels.as_chunks::<4>().0;
    match format {
        Format::A8Unorm => out.extend(texels.iter().map(|t| t[3])),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            out.extend(texels.iter().flat_map(|t| [t[0], t[1], t[2], 255]));
        }
        Format::Rgbg8Unorm | Format::Grgb8Unorm => {
            for pair in texels.chunks(2) {
                let (p0, p1) = (pair[0], pair[pair.len() - 1]);
                out.extend(match format {
                    Format::Rgbg8Unorm => [p0[0], p0[1], p0[2], p1[1]],
                    _ => [p0[1], p0[0], p1[1], p0[2]],
                });
            }
        }
        _ => {
            let f = fields(format);
            for t in texels {
                let (mut v, mut at) = (0u32, 0);
                for (c, bits) in [t[2], t[1], t[0], t[3]].into_iter().zip(f) {
                    let max = (1 << bits) - 1;
                    v |= ((u32::from(c) * max + 127) / 255) << at;
                    at += bits;
                }
                out.extend((v as u16).to_le_bytes());
            }
        }
    }
}

/// Bytes of `texels` texels of `format` packed: whole blocks.
pub(crate) fn packed_bytes(format: Format, texels: u64) -> u64 {
    let (bw, _, bytes) = format.block();
    texels.div_ceil(u64::from(bw)) * u64::from(bytes)
}

/// The same region of the same texture, borrowed again.
fn again<'a>(r: &TextureRegion<'a, WebGpu>) -> TextureRegion<'a, WebGpu> {
    TextureRegion {
        texture: r.texture,
        mip: r.mip,
        layer: r.layer,
        origin: r.origin,
        stencil: r.stencil,
    }
}

impl WebGpu {
    /// A write into a converted texture: its texels widened on the host.
    pub(crate) fn write_packed(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        let format = region.texture.0.desc.format;
        if !region.origin.x.is_multiple_of(format.block().0) {
            return self.fail("a region of a two-pixel format starting inside a pair");
        }
        let row = packed_bytes(format, u64::from(extent.width)) as usize;
        let wide = extent.width as usize * 4;
        let mut texels =
            Vec::with_capacity(wide * extent.height as usize * extent.depth_or_layers as usize);
        for l in 0..extent.depth_or_layers as usize {
            for r in 0..extent.height as usize {
                let from = (l * rows_per_layer as usize + r) * row_pitch as usize;
                let Some(src) = data.get(from..from + row) else {
                    return self
                        .fail("a write into a converted texture with fewer bytes than its extent");
                };
                let at = texels.len();
                widen(format, src, &mut texels);
                // A pair reaching past an odd level's edge gives one texel.
                texels.truncate(at + wide);
            }
        }
        self.slow.hit(SlowPath::FormatConverted);
        self.write_texels(
            region,
            extent,
            &texels,
            extent.width * 4,
            extent.height,
            mode,
        );
    }

    /// A copy between a buffer and a converted texture: into it from the buffer's bytes read back
    /// first; out of it from its texels read back and packed, written into the buffer in command
    /// order.
    pub(crate) fn copy_buffer_packed(
        &mut self,
        buffer: &CopyBufferTexture<'_, Self>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        to_texture: bool,
    ) {
        let format = region.texture.0.desc.format;
        let row = packed_bytes(format, u64::from(extent.width));
        let (pitch, per_layer) = (
            u64::from(buffer.row_pitch),
            u64::from(buffer.rows_per_layer),
        );
        let (rows, layers) = (u64::from(extent.height), u64::from(extent.depth_or_layers));
        if row == 0 || rows == 0 || layers == 0 {
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
            self.write_packed(region, extent, &data, pitch, per_layer, WriteMode::Ordered);
            return;
        }
        let source = ReadSource::Texture {
            region: again(region),
            extent,
        };
        let Some(packed) = self.read_now(&source, (row * rows * layers) as usize) else {
            return;
        };
        // Rows written one by one only where the buffer's rows lie apart.
        let tight = pitch == row && (layers == 1 || per_layer == rows);
        let chunk = if tight { packed.len() } else { row as usize };
        for (i, bytes) in packed.chunks_exact(chunk).enumerate() {
            let (l, r) = (i as u64 / rows, i as u64 % rows);
            let at = buffer.offset + (l * per_layer + r) * pitch;
            self.write_buffer_impl(buffer.buffer, at, bytes, WriteMode::Ordered);
        }
    }

    /// A copy between textures of different formats of which one is converted: the source's
    /// texels read back in its own format and written into the destination as its texels.
    pub(crate) fn copy_packed(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (sf, df) = (src.texture.0.desc.format, dst.texture.0.desc.format);
        if sf.block() != df.block() {
            return self.fail("a copy between a converted format and one of another texel size");
        }
        let pitch = packed_bytes(sf, u64::from(extent.width)) as u32;
        let len = pitch as usize * extent.height as usize * extent.depth_or_layers as usize;
        let source = ReadSource::Texture {
            region: again(src),
            extent,
        };
        let Some(texels) = self.read_now(&source, len) else {
            return;
        };
        self.write_texture_impl(
            dst,
            extent,
            &texels,
            pitch,
            extent.height,
            WriteMode::Ordered,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every value of every converted format comes back from its widened form exactly.
    #[test]
    fn packing_the_widened_texels_gives_every_value_back() {
        let all: Vec<u8> = (0..=u16::MAX).flat_map(u16::to_le_bytes).collect();
        let bytes: Vec<u8> = (0..=u8::MAX).collect();
        let pairs: Vec<u8> = (0..=u16::MAX)
            .flat_map(|v| [v as u8, 7, (v >> 8) as u8, 200])
            .collect();
        let bgrx: Vec<u8> = bytes.iter().flat_map(|&v| [v, !v, v / 3, 255]).collect();
        for (format, texels) in [
            (Format::A8Unorm, &bytes),
            (Format::B5g6r5Unorm, &all),
            (Format::Bgr5a1Unorm, &all),
            (Format::Bgra4Unorm, &all),
            (Format::Rgbg8Unorm, &pairs),
            (Format::Grgb8Unorm, &pairs),
            (Format::Bgrx8Unorm, &bgrx),
            (Format::Bgrx8UnormSrgb, &bgrx),
        ] {
            let mut wide = Vec::new();
            widen(format, texels, &mut wide);
            let mut back = Vec::new();
            pack(format, &wide, &mut back);
            assert_eq!(&back, texels, "{format:?}");
        }
    }

    /// Widened channels repeat their bits (3 of 31 is 24, where the nearest byte is 25); a missing
    /// alpha is one, and 8-bit alpha keeps no colour.
    #[test]
    fn widened_channels_repeat_their_bits() {
        let mut out = Vec::new();
        // B5G6R5: blue 3, green 32, red 1.
        widen(
            Format::B5g6r5Unorm,
            &(3u16 | 32 << 5 | 1 << 11).to_le_bytes(),
            &mut out,
        );
        assert_eq!(out, [8, 130, 24, 255]);
        out.clear();
        // B5G5R5A1: alpha clear; B4G4R4A4: alpha 5 of 15.
        widen(Format::Bgr5a1Unorm, &(16u16 << 10).to_le_bytes(), &mut out);
        widen(
            Format::Bgra4Unorm,
            &(5u16 << 12 | 15).to_le_bytes(),
            &mut out,
        );
        widen(Format::A8Unorm, &[7], &mut out);
        assert_eq!(out, [132, 0, 0, 0, 0, 0, 255, 85, 0, 0, 0, 7]);
        out.clear();
        // Two pixels each: R, G0, B, G1 and G0, R, G1, B; BGRX's X read as one.
        widen(Format::Rgbg8Unorm, &[1, 2, 3, 4], &mut out);
        widen(Format::Grgb8Unorm, &[1, 2, 3, 4], &mut out);
        widen(Format::Bgrx8Unorm, &[1, 2, 3, 4], &mut out);
        assert_eq!(
            out,
            [
                1, 2, 3, 255, 1, 4, 3, 255, 2, 1, 4, 255, 2, 3, 4, 255, 1, 2, 3, 255
            ]
        );
    }
}
