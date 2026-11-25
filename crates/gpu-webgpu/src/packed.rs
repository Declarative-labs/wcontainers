//! Formats WebGPU lacks, held in an 8-bit RGBA format (plan 5.7, `FormatConverted`). Drawn into
//! with only their own channels written: 8-bit alpha as (0, 0, 0, a), and BGRX as BGRA with alpha
//! one. The 16-bit packed formats (`B5G6R5`, `B5G5R5A1`, `B4G4R4A4`) hold each channel widened to 8
//! bits; the two 5-bit ones are drawn, blended and resolved at 8 bits, and each channel rounded to
//! its own bits when the pass or resolve ends (`PackedTargetQuantised`) and before a blend reads
//! it (`PackedTargetReadByBlend`). Sampled and filtered only:
//! `B4G4R4A4`, and the two-pixel formats (`R8G8_B8G8`, `G8R8_G8B8`), each pixel its own texel
//! (`BlockDecoded`'s formats, converted the same way). The three-channel 32-bit formats (`R32G32B32_*`) are held in their
//! four-channel twins with alpha one, sampled (filtered where the twin is) and copied. Writes are
//! widened on the host and read backs packed again on the host, exactly, since 8 bits keep every
//! narrower value apart; copies between a buffer and such a texture widen and pack the same way on
//! the GPU, by a draw.

use wcontainers_gpu::{
    CopyBufferTexture, Extent, FORMAT_FACTS, Format, Gpu, Origin, SlowPath, TextureDesc,
    TextureDimension, TextureRegion, TextureUsage, WriteMode,
};

use crate::memory::DX12_PLACEMENT_ALIGNMENT;
use crate::{Texture, WebGpu};

/// The format a texture of `format` is held in, where the back end converts it.
pub(crate) fn held(format: Format) -> Option<Format> {
    use Format::*;
    Some(match format {
        A8Unorm | B5g6r5Unorm | Bgr5a1Unorm | Bgra4Unorm | Rgbg8Unorm | Grgb8Unorm => Rgba8Unorm,
        Bgrx8Unorm => Bgra8Unorm,
        Bgrx8UnormSrgb => Bgra8UnormSrgb,
        Rgb32Float => Rgba32Float,
        Rgb32Uint => Rgba32Uint,
        Rgb32Sint => Rgba32Sint,
        _ => return None,
    })
}

/// The channels a draw into a converted format writes, where it can be drawn into: the others hold
/// what the format reads as (zero colour for 8-bit alpha, alpha one for BGRX).
pub(crate) fn drawn(format: Format) -> Option<wgpu::ColorWrites> {
    match format {
        Format::A8Unorm => Some(wgpu::ColorWrites::ALPHA),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb | Format::B5g6r5Unorm => {
            Some(wgpu::ColorWrites::COLOR)
        }
        Format::Bgr5a1Unorm => Some(wgpu::ColorWrites::ALL),
        _ => None,
    }
}

/// Whether what a pass or resolve leaves in a target of `format` is rounded to the format's own
/// bits when it ends: the 16-bit packed formats drawn into (`PackedTargetQuantised`).
pub(crate) fn quantised(format: Format) -> bool {
    matches!(format, Format::B5g6r5Unorm | Format::Bgr5a1Unorm)
}

/// Whether programs store into a converted format through its held format: 8-bit alpha, whose
/// stores the shaders crate makes write `(0, 0, 0, a)` (its `storage_formats`).
pub(crate) fn stored(format: Format) -> bool {
    format == Format::A8Unorm
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
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb | Format::B5g6r5Unorm => {
            wgpu::Color { a: 1.0, ..c }
        }
        _ => c,
    }
}

/// The integer format holding a converted format's raw bytes on the GPU: one texel a block, or a
/// 32-bit texel a channel of a three-channel 32-bit format.
pub(crate) fn raw(format: Format) -> Format {
    [Format::R8Uint, Format::R16Uint]
        .into_iter()
        .find(|r| FORMAT_FACTS[*r as usize].block.2 == FORMAT_FACTS[format as usize].block.2)
        .unwrap_or(Format::R32Uint)
}

/// Texels of [`raw`] a row of `width` texels of `format` takes.
pub(crate) fn raw_width(format: Format, width: u32) -> u32 {
    let (bw, _, bytes) = FORMAT_FACTS[format as usize].block;
    width.div_ceil(bw) * bytes.div_ceil(FORMAT_FACTS[Format::R32Uint as usize].block.2)
}

/// Bytes of one texel of the format a converted `format` is held in.
pub(crate) fn held_bytes(format: Format) -> u32 {
    FORMAT_FACTS[held(format).unwrap_or(Format::Rgba8Unorm) as usize]
        .block
        .2
}

/// Whether `format` is a three-channel 32-bit format, held with a fourth channel.
fn three_channel(format: Format) -> bool {
    matches!(
        format,
        Format::Rgb32Float | Format::Rgb32Uint | Format::Rgb32Sint
    )
}

/// The WGSL scalar type of a three-channel 32-bit format's channels.
fn scalar(format: Format) -> &'static str {
    match format {
        Format::Rgb32Uint => "u32",
        Format::Rgb32Sint => "i32",
        _ => "f32",
    }
}

/// The copy programs of a three-channel 32-bit format: each channel's bits moved as they are, the
/// fourth channel one.
fn three_channel_program(format: Format, into: bool, three_d: bool) -> String {
    let t = scalar(format);
    // WGSL has no bitcast of a type to itself.
    let (to_t, to_u) = match t {
        "u32" => ("", ""),
        _ => ("bitcast<{t}>", "bitcast<vec4u>"),
    };
    let to_t = to_t.replace("{t}", t);
    if into {
        format!(
            "@group(0) @binding(0) var<uniform> at: vec4<u32>;\n\
             @group(0) @binding(1) var src: texture_2d<u32>;\n\
             fn bits(i: u32, j: u32) -> {t} {{ return {to_t}(textureLoad(src, vec2u(i, j), 0).x); }}\n\
             @fragment fn convert(@builtin(position) p: vec4f) -> @location(0) vec4<{t}> {{\n\
                 let i = 3u * (u32(p.x) - at.x);\n\
                 let j = u32(p.y) - at.y;\n\
                 return vec4<{t}>(bits(i, j), bits(i + 1u, j), bits(i + 2u, j), {t}(1));\n\
             }}\n"
        )
    } else {
        let (texture, at) = if three_d {
            (
                format!("texture_3d<{t}>"),
                "vec3u(at.x + i / 3u, at.y + j, at.z)",
            )
        } else {
            (format!("texture_2d<{t}>"), "vec2u(at.x + i / 3u, at.y + j)")
        };
        format!(
            "@group(0) @binding(0) var<uniform> at: vec4<u32>;\n\
             @group(0) @binding(1) var src: {texture};\n\
             @fragment fn convert(@builtin(position) p: vec4f) -> @location(0) vec4u {{\n\
                 let i = u32(p.x);\n\
                 let j = u32(p.y);\n\
                 let t = {to_u}(textureLoad(src, {at}, 0));\n\
                 let k = i % 3u;\n\
                 return vec4u(select(select(t.x, t.y, k == 1u), t.z, k == 2u), 0u, 0u, 0u);\n\
             }}\n"
        )
    }
}

/// The sample type of the held texels a copy program of `format` reads.
pub(crate) fn held_sample_type(format: Format) -> wgpu::TextureSampleType {
    match format {
        Format::Rgb32Uint => wgpu::TextureSampleType::Uint,
        Format::Rgb32Sint => wgpu::TextureSampleType::Sint,
        _ => wgpu::TextureSampleType::Float { filterable: false },
    }
}

/// WGSL making `c`, the held texel's bytes in RGBA order, from `w`, the raw block, and `i`, the
/// texel's column from the region's start; as [`widen`] does.
fn widen_wgsl(format: Format) -> String {
    let bytes = "let b = vec4u(w & 255u, (w >> 8u) & 255u, (w >> 16u) & 255u, w >> 24u);";
    let odd = "(i & 1u) == 1u";
    match format {
        Format::A8Unorm => "let c = vec4u(0u, 0u, 0u, w);".into(),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            format!("{bytes} let c = vec4u(b.z, b.y, b.x, 255u);")
        }
        Format::Rgbg8Unorm => {
            format!("{bytes} let c = vec4u(b.x, select(b.y, b.w, {odd}), b.z, 255u);")
        }
        Format::Grgb8Unorm => {
            format!("{bytes} let c = vec4u(b.y, select(b.x, b.z, {odd}), b.w, 255u);")
        }
        _ => {
            let (mut at, mut c) = (
                0,
                [
                    String::from("255u"),
                    "255u".into(),
                    "255u".into(),
                    "255u".into(),
                ],
            );
            for (slot, bits) in [2, 1, 0, 3]
                .into_iter()
                .zip(fields(format))
                .filter(|(_, b)| *b > 0)
            {
                c[slot] = format!("repeat((w >> {at}u) & {}u, {bits}u)", (1u32 << bits) - 1);
                at += bits;
            }
            format!("let c = vec4u({});", c.join(", "))
        }
    }
}

/// WGSL making `w`, the raw block, from `texel(x, y)`, the held texels' bytes in RGBA order, at
/// column `i` and row `j` of the raw texture; the region is `at.w` texels wide. As [`pack`] does.
fn pack_wgsl(format: Format) -> String {
    let pair = "let p0 = texel(2u * i, j); let p1 = texel(min(2u * i + 1u, at.w - 1u), j);";
    match format {
        Format::A8Unorm => "let w = texel(i, j).w;".into(),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            "let t = texel(i, j); let w = t.z | (t.y << 8u) | (t.x << 16u) | (255u << 24u);".into()
        }
        Format::Rgbg8Unorm => {
            format!("{pair} let w = p0.x | (p0.y << 8u) | (p0.z << 16u) | (p1.y << 24u);")
        }
        Format::Grgb8Unorm => {
            format!("{pair} let w = p0.y | (p0.x << 8u) | (p1.y << 16u) | (p0.z << 24u);")
        }
        _ => {
            let (mut at, mut parts) = (0, Vec::new());
            for (c, bits) in ["t.z", "t.y", "t.x", "t.w"].into_iter().zip(fields(format)) {
                let max = (1u32 << bits) - 1;
                parts.push(format!("((({c} * {max}u + 127u) / 255u) << {at}u)"));
                at += bits;
            }
            format!("let t = texel(i, j); let w = {};", parts.join(" | "))
        }
    }
}

/// The fragment program of a copy by a draw between a converted format's raw blocks and its held
/// texels (`into` the held texture, or out of a 3D or 2D one), its entry point `convert`.
pub(crate) fn program(format: Format, into: bool, three_d: bool) -> String {
    if three_channel(format) {
        return three_channel_program(format, into, three_d);
    }
    if into {
        format!(
            "@group(0) @binding(0) var<uniform> at: vec4<u32>;\n\
             @group(0) @binding(1) var src: texture_2d<u32>;\n\
             {REPEAT_WGSL}\
             @fragment fn convert(@builtin(position) p: vec4f) -> @location(0) vec4f {{\n\
                 let i = u32(p.x) - at.x;\n\
                 let j = u32(p.y) - at.y;\n\
                 let w = textureLoad(src, vec2u(i / {per}u, j), 0).x;\n\
                 {body}\n\
                 return vec4f(c) / 255.0;\n\
             }}\n",
            per = FORMAT_FACTS[format as usize].block.0,
            body = widen_wgsl(format),
        )
    } else {
        let (texture, at) = if three_d {
            ("texture_3d<f32>", "vec3u(at.x + x, at.y + y, at.z)")
        } else {
            ("texture_2d<f32>", "vec2u(at.x + x, at.y + y)")
        };
        format!(
            "@group(0) @binding(0) var<uniform> at: vec4<u32>;\n\
             @group(0) @binding(1) var src: {texture};\n\
             fn texel(x: u32, y: u32) -> vec4u {{\n\
                 return vec4u(round(textureLoad(src, {at}, 0) * 255.0));\n\
             }}\n\
             @fragment fn convert(@builtin(position) p: vec4f) -> @location(0) vec4u {{\n\
                 let i = u32(p.x);\n\
                 let j = u32(p.y);\n\
                 {body}\n\
                 return vec4u(w, 0u, 0u, 0u);\n\
             }}\n",
            body = pack_wgsl(format),
        )
    }
}

/// WGSL widening `x`, a code of `n` bits, to 8 by repeating its bits, as [`widen`] does.
const REPEAT_WGSL: &str = "
fn repeat(x: u32, n: u32) -> u32 {
    var wide = 0u;
    var filled = 0u;
    loop {
        wide = (wide << n) | x;
        filled += n;
        if filled >= 8u { break; }
    }
    return wide >> (filled - 8u);
}
";

/// The fragment program, entry point `quantise`, rounding each channel of a target of the packed
/// `format`, read from a copy of it bound at binding 0 (multisampled: per sample), to its own bits
/// and widening it back to 8 as [`widen`] does: `round(v * (2^n - 1))`, its bits repeated. The
/// 8-bit value it starts from adds at most `(2^n - 1) / 510` of a step to the half step of exact
/// rounding: 0.56 of a step for 5 bits, inside the specification's 0.6, and 0.62 for 6 bits.
pub(crate) fn quantise_program(format: Format, multisampled: bool) -> String {
    let (texture, sample, index) = QUANTISE_READS[usize::from(multisampled)];
    let [b, g, r, a] = fields(format);
    format!(
        "@group(0) @binding(0) var src: {texture};\n\
         {REPEAT_WGSL}\
         fn q(x: f32, n: u32) -> f32 {{\n\
             if n == 0u {{ return 1.0; }}\n\
             let top = (1u << n) - 1u;\n\
             return f32(repeat(u32(round(x * f32(top))), n)) / 255.0;\n\
         }}\n\
         @fragment fn quantise(@builtin(position) p: vec4f{sample}) -> @location(0) vec4f {{\n\
             let v = textureLoad(src, vec2u(p.xy), {index});\n\
             return vec4f(q(v.x, {r}u), q(v.y, {g}u), q(v.z, {b}u), q(v.w, {a}u));\n\
         }}\n"
    )
}

/// What a rounding program binds, the inputs its entry point adds and the sample it loads:
/// single-sampled, then multisampled (one invocation a sample).
const QUANTISE_READS: [(&str, &str, &str); 2] = [
    ("texture_2d<f32>", "", "0"),
    (
        "texture_multisampled_2d<f32>",
        ", @builtin(sample_index) s: u32",
        "s",
    ),
];

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
    if three_channel(format) {
        let one = match format {
            Format::Rgb32Float => 1f32.to_bits(),
            _ => 1,
        };
        for t in packed.as_chunks::<12>().0 {
            out.extend_from_slice(t);
            out.extend(one.to_le_bytes());
        }
        return;
    }
    match format {
        Format::A8Unorm => out.extend(packed.iter().flat_map(|&a| [0, 0, 0, a])),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            out.extend(
                packed
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|t| [t[0], t[1], t[2], u8::MAX]),
            );
        }
        Format::Rgbg8Unorm | Format::Grgb8Unorm => {
            for p in packed.as_chunks::<4>().0 {
                let [r, g0, b, g1] = match format {
                    Format::Rgbg8Unorm => *p,
                    _ => [p[1], p[0], p[3], p[2]],
                };
                out.extend([r, g0, b, u8::MAX, r, g1, b, u8::MAX]);
            }
        }
        _ => {
            let f = fields(format);
            for t in packed.as_chunks::<2>().0 {
                let v = u32::from(u16::from_le_bytes(*t));
                let (mut bgra, mut at) = ([u8::MAX; 4], 0);
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
    if three_channel(format) {
        for t in texels.as_chunks::<16>().0 {
            out.extend_from_slice(&t[..12]);
        }
        return;
    }
    let texels = texels.as_chunks::<4>().0;
    match format {
        Format::A8Unorm => out.extend(texels.iter().map(|t| t[3])),
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb => {
            out.extend(texels.iter().flat_map(|t| [t[0], t[1], t[2], u8::MAX]));
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
                    let full = u32::from(u8::MAX);
                    v |= ((u32::from(c) * max + full / 2) / full) << at;
                    at += bits;
                }
                out.extend((v as u16).to_le_bytes());
            }
        }
    }
}

/// Bytes of `texels` texels of `format` packed: whole blocks.
pub(crate) fn packed_bytes(format: Format, texels: u64) -> u64 {
    let (bw, _, bytes) = FORMAT_FACTS[format as usize].block;
    texels.div_ceil(u64::from(bw)) * u64::from(bytes)
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
        if !region
            .origin
            .x
            .is_multiple_of(FORMAT_FACTS[format as usize].block.0)
        {
            return self.fail("a region of a two-pixel format starting inside a pair");
        }
        let row = packed_bytes(format, u64::from(extent.width)) as usize;
        let texel = held_bytes(format);
        let wide = (extent.width * texel) as usize;
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
        self.record_slow_path(SlowPath::FormatConverted);
        self.write_texels(
            region,
            extent,
            &texels,
            extent.width * texel,
            extent.height,
            mode,
        );
    }

    /// The first slice of `region` of a converted texture, after checking that the region lies
    /// inside its level and starts on a block.
    fn first_slice(&mut self, region: &TextureRegion<'_, Self>, extent: Extent) -> Option<u32> {
        let t = &region.texture.0;
        if !region
            .origin
            .x
            .is_multiple_of(FORMAT_FACTS[t.desc.format as usize].block.0)
        {
            self.fail("a region of a two-pixel format starting inside a pair");
            return None;
        }
        let size = |n: u32| (n >> region.mip).max(1);
        let (first, slices) = if t.desc.dimension == TextureDimension::D3 {
            (region.origin.z, size(t.desc.depth_or_layers))
        } else {
            (region.layer, t.desc.depth_or_layers)
        };
        if region.origin.x + extent.width > size(t.desc.width)
            || region.origin.y + extent.height > size(t.desc.height)
            || first + extent.depth_or_layers > slices
        {
            self.fail("a copy reaching past a converted texture's level");
            return None;
        }
        Some(first)
    }

    /// A texture for the raw blocks of `extent` texels of `format`, one layer a slice.
    fn raw_blocks(&mut self, format: Format, extent: Extent) -> Option<Texture> {
        let Ok(staged) = self.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: raw(format),
            width: raw_width(format, extent.width),
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST
                | TextureUsage::SAMPLE
                | TextureUsage::RENDER,
            cube: false,
            reinterpret: false,
        }) else {
            self.fail("the texture a converted copy goes through could not be made");
            return None;
        };
        self.record_slow_path(SlowPath::FormatConverted);
        Some(staged)
    }

    /// Widens the raw blocks in `staged` into `region` of a converted texture (`into`), or packs
    /// the region's texels into them.
    fn convert_region(
        &mut self,
        region: &TextureRegion<'_, Self>,
        first: u32,
        staged: &Texture,
        extent: Extent,
        into: bool,
    ) {
        let t = &region.texture.0;
        t.used_in.set(self.serial);
        let held = (
            &t.raw(),
            region.mip,
            first,
            [region.origin.x, region.origin.y],
        );
        self.convert_by_draw(t.desc.format, into, held, &staged.0.raw(), extent);
    }

    /// Texels of `format` one copy through a texture of raw blocks spans: whole blocks whose raw
    /// bytes are a whole number of Direct3D 12 placements, so every run after the first starts
    /// placed as Direct3D 12 wants; and few enough that the raw texture, widened by the texels of
    /// one placement (where Direct3D 12 implementations move an unplaced copy back to a placed
    /// offset), stays within the device's widest 2D texture.
    fn raw_run(&self, format: Format) -> u32 {
        let bw = FORMAT_FACTS[format as usize].block.0;
        let placement = (DX12_PLACEMENT_ALIGNMENT
            / u64::from(FORMAT_FACTS[raw(format) as usize].block.2)) as u32;
        let widest = self.device.limits().max_texture_dimension_2d;
        let blocks = widest.saturating_sub(placement) / raw_width(format, bw);
        (blocks / placement * placement).max(1) * bw
    }

    /// A copy between a buffer and a converted texture, on the GPU, in runs of columns as wide as
    /// [`Self::raw_run`] allows: the buffer's bytes copied into a texture of their own ([`raw`])
    /// and widened into the texture by a draw, or the texture's texels packed into one by a draw
    /// and copied out.
    pub(crate) fn copy_buffer_packed(
        &mut self,
        buffer: &CopyBufferTexture<'_, Self>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        to_texture: bool,
    ) {
        let format = region.texture.0.desc.format;
        let run = self.raw_run(format);
        let mut x = 0;
        while x < extent.width {
            let width = run.min(extent.width - x);
            let part = CopyBufferTexture {
                buffer: buffer.buffer,
                offset: buffer.offset + packed_bytes(format, u64::from(x)),
                row_pitch: buffer.row_pitch,
                rows_per_layer: buffer.rows_per_layer,
            };
            let at = moved(region, x);
            self.copy_buffer_packed_run(&part, &at, Extent { width, ..extent }, to_texture);
            x += width;
        }
    }

    fn copy_buffer_packed_run(
        &mut self,
        buffer: &CopyBufferTexture<'_, Self>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        to_texture: bool,
    ) {
        if extent.width == 0 || extent.height == 0 || extent.depth_or_layers == 0 {
            return;
        }
        let format = region.texture.0.desc.format;
        let Some(first) = self.first_slice(region, extent) else {
            return;
        };
        let Some(staged) = self.raw_blocks(format, extent) else {
            return;
        };
        let blocks = Extent {
            width: raw_width(format, extent.width),
            ..extent
        };
        let at = at_origin(&staged);
        if to_texture {
            self.copy_buffer_texture(buffer, &at, blocks, true);
            self.convert_region(region, first, &staged, extent, true);
        } else {
            self.convert_region(region, first, &staged, extent, false);
            self.copy_buffer_texture(buffer, &at, blocks, false);
        }
    }

    /// A copy between textures of different formats of equal blocks of which one is converted, on
    /// the GPU: through a texture of the raw blocks, packed into it from a converted source or
    /// copied from another, and widened from it into a converted destination or copied into
    /// another.
    pub(crate) fn copy_packed(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (sf, df) = (src.texture.0.desc.format, dst.texture.0.desc.format);
        if FORMAT_FACTS[sf as usize].block != FORMAT_FACTS[df as usize].block {
            return self.fail("a copy between a converted format and one of another texel size");
        }
        let run = self.raw_run(sf);
        let mut x = 0;
        while x < extent.width {
            let width = run.min(extent.width - x);
            self.copy_packed_run(&moved(src, x), &moved(dst, x), Extent { width, ..extent });
            x += width;
        }
    }

    fn copy_packed_run(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (sf, df) = (src.texture.0.desc.format, dst.texture.0.desc.format);
        if extent.width == 0 || extent.height == 0 || extent.depth_or_layers == 0 {
            return;
        }
        let (from, to) = (held(sf).is_some(), held(df).is_some());
        let Some(src_first) = (if from {
            self.first_slice(src, extent)
        } else {
            Some(0)
        }) else {
            return;
        };
        let Some(dst_first) = (if to {
            self.first_slice(dst, extent)
        } else {
            Some(0)
        }) else {
            return;
        };
        let Some(staged) = self.raw_blocks(sf, extent) else {
            return;
        };
        let at = at_origin(&staged);
        if from {
            self.convert_region(src, src_first, &staged, extent, false);
        } else {
            self.copy_texture_impl(src, &at, extent);
        }
        if to {
            self.convert_region(dst, dst_first, &staged, extent, true);
        } else {
            self.copy_texture_impl(&at, dst, extent);
        }
    }
}

/// `region` moved `x` texels to the right.
fn moved<'a>(region: &TextureRegion<'a, WebGpu>, x: u32) -> TextureRegion<'a, WebGpu> {
    TextureRegion {
        texture: region.texture,
        mip: region.mip,
        layer: region.layer,
        origin: Origin {
            x: region.origin.x + x,
            ..region.origin
        },
        stencil: region.stencil,
    }
}

/// The whole of a texture's first level and layer, as a copy names it.
/// One primitive of a draw, drawn alone where a blend reads a packed target, so that each
/// primitive's blend reads what the one before it left rounded to the format's bits
/// (`PackedTargetReadByBlend`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    /// Its vertices, or its indices in an indexed draw.
    pub range: std::ops::Range<u32>,
    pub instance: u32,
    /// A strip's odd triangle, whose facing the strip reverses and a draw of its own does not.
    pub odd: bool,
}

/// The vertices each primitive of `topology` takes.
fn vertices_per_primitive(topology: wgpu::PrimitiveTopology) -> u32 {
    use wgpu::PrimitiveTopology as T;
    /// A triangle's corners.
    const CORNERS: u32 = 3;
    match topology {
        T::PointList => 1,
        T::LineList | T::LineStrip => 2,
        T::TriangleList | T::TriangleStrip => CORNERS,
    }
}

/// The primitives of `count` vertices or indices from `first`, instance by instance, in the order
/// they are rasterised. `cuts` marks a strip's restart indices, by position from `first`.
pub(crate) fn pieces(
    topology: wgpu::PrimitiveTopology,
    first: u32,
    count: u32,
    instances: std::ops::Range<u32>,
    cuts: &[bool],
) -> Vec<Piece> {
    let n = vertices_per_primitive(topology);
    let strip = topology.is_strip();
    let mut starts = Vec::new();
    if strip {
        let mut run = 0;
        for p in 0..count {
            if cuts.get(p as usize) == Some(&true) {
                run = 0;
                continue;
            }
            run += 1;
            if run >= n {
                starts.push((
                    first + p + 1 - n,
                    topology == wgpu::PrimitiveTopology::TriangleStrip
                        && !(run - n).is_multiple_of(2),
                ));
            }
        }
    } else {
        starts.extend((0..count / n).map(|i| (first + i * n, false)));
    }
    instances
        .flat_map(|instance| {
            starts.iter().map(move |&(start, odd)| Piece {
                range: start..start + n,
                instance,
                odd,
            })
        })
        .collect()
}

fn at_origin(texture: &Texture) -> TextureRegion<'_, WebGpu> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lists split into whole primitives, strips into every primitive they make, restarting at a
    /// cut, with odd triangles counted from the strip's start; instances follow one another.
    #[test]
    fn draws_split_into_their_primitives_in_order() {
        use wgpu::PrimitiveTopology as T;
        let ranges = |p: Vec<Piece>| -> Vec<(u32, u32, u32, bool)> {
            p.into_iter()
                .map(|p| (p.range.start, p.range.end, p.instance, p.odd))
                .collect()
        };
        assert_eq!(
            ranges(pieces(T::TriangleList, 2, 7, 0..2, &[])),
            [
                (2, 5, 0, false),
                (5, 8, 0, false),
                (2, 5, 1, false),
                (5, 8, 1, false)
            ]
        );
        assert_eq!(
            ranges(pieces(T::TriangleStrip, 0, 5, 3..4, &[])),
            [(0, 3, 3, false), (1, 4, 3, true), (2, 5, 3, false)]
        );
        let cut = [false, false, false, true, false, false, false, false];
        assert_eq!(
            ranges(pieces(T::TriangleStrip, 10, 8, 0..1, &cut)),
            [(10, 13, 0, false), (14, 17, 0, false), (15, 18, 0, true)]
        );
        assert_eq!(
            ranges(pieces(T::LineStrip, 0, 3, 0..1, &[])),
            [(0, 2, 0, false), (1, 3, 0, false)]
        );
        assert_eq!(ranges(pieces(T::PointList, 4, 2, 0..1, &[])).len(), 2);
    }

    /// Every value of every converted format comes back from its widened form exactly.
    #[test]
    fn packing_the_widened_texels_gives_every_value_back() {
        let all: Vec<u8> = (0..=u16::MAX).flat_map(u16::to_le_bytes).collect();
        let bytes: Vec<u8> = (0..=u8::MAX).collect();
        let pairs: Vec<u8> = (0..=u16::MAX)
            .flat_map(|v| [v as u8, 7, (v >> 8) as u8, 200])
            .collect();
        let bgrx: Vec<u8> = bytes.iter().flat_map(|&v| [v, !v, v / 3, 255]).collect();
        let triples: Vec<u8> = (0..12 * 256).map(|i| (i * 7 % 251) as u8).collect();
        for (format, texels) in [
            (Format::A8Unorm, &bytes),
            (Format::B5g6r5Unorm, &all),
            (Format::Bgr5a1Unorm, &all),
            (Format::Bgra4Unorm, &all),
            (Format::Rgbg8Unorm, &pairs),
            (Format::Grgb8Unorm, &pairs),
            (Format::Bgrx8Unorm, &bgrx),
            (Format::Bgrx8UnormSrgb, &bgrx),
            (Format::Rgb32Float, &triples),
            (Format::Rgb32Uint, &triples),
            (Format::Rgb32Sint, &triples),
        ] {
            let mut wide = Vec::new();
            widen(format, texels, &mut wide);
            let mut back = Vec::new();
            pack(format, &wide, &mut back);
            assert_eq!(&back, texels, "{format:?}");
        }
    }

    /// Every copy program of every converted format is valid WGSL for WebGPU.
    #[test]
    fn copy_programs_are_valid() {
        use Format::*;
        for format in [
            A8Unorm,
            Bgrx8Unorm,
            Bgrx8UnormSrgb,
            B5g6r5Unorm,
            Bgr5a1Unorm,
            Bgra4Unorm,
            Rgbg8Unorm,
            Grgb8Unorm,
            Rgb32Float,
            Rgb32Uint,
            Rgb32Sint,
        ] {
            for (into, three_d) in [(true, false), (false, false), (false, true)] {
                let source = program(format, into, three_d);
                let module = naga::front::wgsl::parse_str(&source)
                    .unwrap_or_else(|e| panic!("{format:?} {into}: {}\n{source}", e.message()));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("{format:?} {into}: {e:?}\n{source}"));
            }
        }
    }

    /// Three-channel 32-bit texels keep their bytes and read one in the fourth channel.
    #[test]
    fn three_channels_widen_with_alpha_one() {
        let texel: Vec<u8> = (1..=12).collect();
        for (format, one) in [
            (Format::Rgb32Float, 1f32.to_bits()),
            (Format::Rgb32Uint, 1),
            (Format::Rgb32Sint, 1),
        ] {
            let mut out = Vec::new();
            widen(format, &texel, &mut out);
            assert_eq!(&out[..12], &texel[..], "{format:?}");
            assert_eq!(out[12..], one.to_le_bytes(), "{format:?}");
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
