//! How each interface format is stored in GL ES 3.0, and what GL lets it do.

use glow::*;
use wcontainers_gpu::{FORMAT_FACTS, Format, FormatUse};

/// A format's GL sized internal format and the format and type its texels are written in.
#[derive(Clone, Copy, Debug)]
pub struct GlFormat {
    pub internal: u32,
    pub format: u32,
    pub ty: u32,
    /// What GLES 3.0's own table (texture-filterable, colour-renderable) and the extensions give.
    pub class: Class,
}

/// Which row of GLES 3.0's format table a format is in, which decides its uses with the extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Normalised fixed point: filterable, and renderable and blendable where `render` is set (signed
    /// ones with EXT_render_snorm).
    Norm {
        render: bool,
    },
    /// 16-bit float: filterable; renderable and blendable with EXT_color_buffer_float.
    Half,
    /// 32-bit float: filterable with OES_texture_float_linear, renderable with EXT_color_buffer_float,
    /// blendable with EXT_float_blend as well.
    Single,
    /// R11F_G11F_B10F: filterable; renderable and blendable with EXT_color_buffer_float.
    SmallFloat,
    /// RGB9_E5: filterable, never renderable.
    SharedExponent,
    /// Integer: renderable where `render` is set, never filtered or blended.
    Int {
        render: bool,
    },
    Depth,
}

/// The GL storage of `format`, or None when GL ES 3.0 does not hold it as it is.
pub fn gl_format(format: Format) -> Option<GlFormat> {
    use Class::*;
    use Format as F;
    let f = |internal, format, ty, class| {
        Some(GlFormat {
            internal,
            format,
            ty,
            class,
        })
    };
    let norm = Norm { render: true };
    let snorm = Norm { render: false };
    let int = Int { render: true };
    let int3 = Int { render: false };
    match format {
        F::R8Unorm => f(R8, RED, UNSIGNED_BYTE, norm),
        F::R8Snorm => f(R8_SNORM, RED, BYTE, snorm),
        F::R8Uint => f(R8UI, RED_INTEGER, UNSIGNED_BYTE, int),
        F::R8Sint => f(R8I, RED_INTEGER, BYTE, int),
        F::R16Uint => f(R16UI, RED_INTEGER, UNSIGNED_SHORT, int),
        F::R16Sint => f(R16I, RED_INTEGER, SHORT, int),
        F::R16Float => f(R16F, RED, HALF_FLOAT, Half),
        // 16-bit normalised formats with EXT_texture_norm16 ([`uses`] gives none without it).
        F::R16Unorm => f(R16, RED, UNSIGNED_SHORT, norm),
        F::R16Snorm => f(R16_SNORM, RED, SHORT, snorm),
        F::Rg16Unorm => f(RG16, RG, UNSIGNED_SHORT, norm),
        F::Rg16Snorm => f(RG16_SNORM, RG, SHORT, snorm),
        F::Rgba16Unorm => f(RGBA16, RGBA, UNSIGNED_SHORT, norm),
        F::Rgba16Snorm => f(RGBA16_SNORM, RGBA, SHORT, snorm),
        F::Rg8Unorm => f(RG8, RG, UNSIGNED_BYTE, norm),
        F::Rg8Snorm => f(RG8_SNORM, RG, BYTE, snorm),
        F::Rg8Uint => f(RG8UI, RG_INTEGER, UNSIGNED_BYTE, int),
        F::Rg8Sint => f(RG8I, RG_INTEGER, BYTE, int),
        F::R32Uint => f(R32UI, RED_INTEGER, UNSIGNED_INT, int),
        F::R32Sint => f(R32I, RED_INTEGER, INT, int),
        F::R32Float => f(R32F, RED, FLOAT, Single),
        F::Rg16Uint => f(RG16UI, RG_INTEGER, UNSIGNED_SHORT, int),
        F::Rg16Sint => f(RG16I, RG_INTEGER, SHORT, int),
        F::Rg16Float => f(RG16F, RG, HALF_FLOAT, Half),
        F::Rgba8Unorm => f(RGBA8, RGBA, UNSIGNED_BYTE, norm),
        F::Rgba8UnormSrgb => f(SRGB8_ALPHA8, RGBA, UNSIGNED_BYTE, norm),
        // Stored as RGBA: red and blue are swapped on the host on the way in and out.
        F::Bgra8Unorm => f(RGBA8, RGBA, UNSIGNED_BYTE, norm),
        F::Bgra8UnormSrgb => f(SRGB8_ALPHA8, RGBA, UNSIGNED_BYTE, norm),
        // BGRX as BGRA with alpha one: written one by the host and by clears, never by draws.
        F::Bgrx8Unorm => f(RGBA8, RGBA, UNSIGNED_BYTE, norm),
        F::Bgrx8UnormSrgb => f(SRGB8_ALPHA8, RGBA, UNSIGNED_BYTE, norm),
        F::Rgba8Snorm => f(RGBA8_SNORM, RGBA, BYTE, snorm),
        F::Rgba8Uint => f(RGBA8UI, RGBA_INTEGER, UNSIGNED_BYTE, int),
        F::Rgba8Sint => f(RGBA8I, RGBA_INTEGER, BYTE, int),
        // Direct3D's packed formats put the first channel in the lowest bits, as GL's _REV types do.
        F::Rgb10a2Unorm => f(RGB10_A2, RGBA, UNSIGNED_INT_2_10_10_10_REV, norm),
        F::Rgb10a2Uint => f(RGB10_A2UI, RGBA_INTEGER, UNSIGNED_INT_2_10_10_10_REV, int),
        F::Rg11b10Float => f(
            R11F_G11F_B10F,
            RGB,
            UNSIGNED_INT_10F_11F_11F_REV,
            SmallFloat,
        ),
        F::Rgb9e5Float => f(RGB9_E5, RGB, UNSIGNED_INT_5_9_9_9_REV, SharedExponent),
        // Direct3D's B5G6R5 has red in the top bits, as GL's 5_6_5 does. Its B5G5R5A1 and
        // B4G4R4A4 have alpha in the top bits, which GL's 5_5_5_1 and 4_4_4_4 put lowest.
        F::B5g6r5Unorm => f(RGB565, RGB, UNSIGNED_SHORT_5_6_5, norm),
        F::Bgr5a1Unorm => f(RGB5_A1, RGBA, UNSIGNED_SHORT_5_5_5_1, norm),
        F::Bgra4Unorm => f(RGBA4, RGBA, UNSIGNED_SHORT_4_4_4_4, norm),
        // 8-bit alpha as RGBA with zero colour: widened by the host, cleared zero, never drawn.
        F::A8Unorm => f(RGBA8, RGBA, UNSIGNED_BYTE, norm),
        F::Rg32Uint => f(RG32UI, RG_INTEGER, UNSIGNED_INT, int),
        F::Rg32Sint => f(RG32I, RG_INTEGER, INT, int),
        F::Rg32Float => f(RG32F, RG, FLOAT, Single),
        F::Rgba16Uint => f(RGBA16UI, RGBA_INTEGER, UNSIGNED_SHORT, int),
        F::Rgba16Sint => f(RGBA16I, RGBA_INTEGER, SHORT, int),
        F::Rgba16Float => f(RGBA16F, RGBA, HALF_FLOAT, Half),
        F::Rgb32Uint => f(RGB32UI, RGB_INTEGER, UNSIGNED_INT, int3),
        F::Rgb32Sint => f(RGB32I, RGB_INTEGER, INT, int3),
        F::Rgb32Float => f(RGB32F, RGB, FLOAT, Single),
        F::Rgba32Uint => f(RGBA32UI, RGBA_INTEGER, UNSIGNED_INT, int),
        F::Rgba32Sint => f(RGBA32I, RGBA_INTEGER, INT, int),
        F::Rgba32Float => f(RGBA32F, RGBA, FLOAT, Single),
        F::Depth16Unorm => f(DEPTH_COMPONENT16, DEPTH_COMPONENT, UNSIGNED_SHORT, Depth),
        F::Depth24Plus => f(DEPTH_COMPONENT24, DEPTH_COMPONENT, UNSIGNED_INT, Depth),
        F::Depth24PlusStencil8 => f(DEPTH24_STENCIL8, DEPTH_STENCIL, UNSIGNED_INT_24_8, Depth),
        F::Depth32Float => f(DEPTH_COMPONENT32F, DEPTH_COMPONENT, FLOAT, Depth),
        F::Depth32FloatStencil8 => f(
            DEPTH32F_STENCIL8,
            DEPTH_STENCIL,
            FLOAT_32_UNSIGNED_INT_24_8_REV,
            Depth,
        ),
        _ => None,
    }
}

/// Whether `format` is signed normalised, whose smallest value a draw reads as the one above it.
pub fn signed_normalised(format: Format) -> bool {
    gl_format(format).is_some_and(|g| g.class == Class::Norm { render: false })
}

/// Whether `format` holds 16-bit or 32-bit floats, whose NaN payloads and denormals a copy keeps.
pub fn float_bits(format: Format) -> bool {
    gl_format(format).is_some_and(|g| matches!(g.class, Class::Half | Class::Single))
}

/// Whether the interface's bytes of `format` hold blue first, where GL stores red first
/// ([`wcontainers_gpu::SlowPath::FormatConverted`] on the way in and out).
pub fn swapped(format: Format) -> bool {
    matches!(
        format,
        Format::Bgra8Unorm | Format::Bgra8UnormSrgb | Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb
    )
}

/// The channels of RGBA storage, red, green, blue and alpha.
pub const CHANNELS: usize = 4;

/// The channels of its RGBA storage that `format` lacks, a bit each from red: a missing alpha held
/// as one (BGRX), missing colour held as zero (8-bit alpha). Host writes and clears write them,
/// draws leave them, and passes never discard them.
pub fn unwritten(format: Format) -> u8 {
    let held_in_rgba = matches!(
        format,
        Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb | Format::A8Unorm
    );
    let rgba = FORMAT_FACTS[Format::Rgba8Unorm as usize].components;
    if held_in_rgba {
        (rgba & !FORMAT_FACTS[format as usize].components) as u8
    } else {
        0
    }
}

/// A clear colour as a target of `format` keeps it.
pub fn clear_value(format: Format, mut value: [u32; 4]) -> [u32; 4] {
    let missing = unwritten(format);
    for (c, v) in value.iter_mut().enumerate() {
        if missing & (1 << c) != 0 {
            *v = if c == CHANNELS - 1 { 1f32 } else { 0f32 }.to_bits();
        }
    }
    value
}

/// The format GL stores texels of `format` as where the host widens them: 8-bit alpha as RGBA.
pub fn widened(format: Format) -> Option<Format> {
    (format == Format::A8Unorm).then_some(Format::Rgba8Unorm)
}

/// The bit widths of a 16-bit packed format's blue, green, red and alpha fields, from its lowest
/// bits up, as Direct3D packs them (zero for a missing alpha).
pub type Fields = [u32; CHANNELS];

const B5G6R5_FIELDS: Fields = [5, 6, 5, 0];
const B5G5R5A1_FIELDS: Fields = [5, 5, 5, 1];
const B4G4R4A4_FIELDS: Fields = [4, 4, 4, 4];

/// The fields of a 16-bit packed format.
pub fn packed_fields(format: Format) -> Option<Fields> {
    match format {
        Format::B5g6r5Unorm => Some(B5G6R5_FIELDS),
        Format::Bgr5a1Unorm => Some(B5G5R5A1_FIELDS),
        Format::Bgra4Unorm => Some(B4G4R4A4_FIELDS),
        _ => None,
    }
}

/// Whether host bytes of `format` are converted before GL takes them ([`stored_texels`]).
pub fn converts_writes(format: Format) -> bool {
    swapped(format)
        || unwritten(format) != 0
        || packed_fields(format).is_some_and(|[.., alpha]| alpha > 0)
}

/// Appends the texels GL stores for `texels`, whole texels of `format` that [`converts_writes`]:
/// red and blue swapped (BGRX's alpha one), 8-bit alpha as (0, 0, 0, a), and a 16-bit packed
/// format's alpha moved from its highest bits to its lowest, where GL packs it.
pub fn stored_texels(format: Format, texels: &[u8], out: &mut Vec<u8>) {
    if format == Format::A8Unorm {
        return out.extend(texels.iter().flat_map(|&a| [0, 0, 0, a]));
    }
    if let Some([.., alpha]) = packed_fields(format) {
        for t in texels.as_chunks::<2>().0 {
            out.extend(u16::from_le_bytes(*t).rotate_left(alpha).to_le_bytes());
        }
        return;
    }
    let at = out.len();
    out.extend_from_slice(texels);
    swap_red_blue(&mut out[at..]);
    if unwritten(format) != 0 {
        for [.., alpha] in out[at..].as_chunks_mut::<CHANNELS>().0 {
            *alpha = u8::MAX;
        }
    }
}

/// Swaps red and blue in each 4-byte texel of `texels`.
pub fn swap_red_blue(texels: &mut [u8]) {
    for t in texels.as_chunks_mut::<4>().0 {
        t.swap(0, 2);
    }
}

/// The extensions that change what formats can do.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormatExtensions {
    pub color_buffer_float: bool,
    pub float_linear: bool,
    pub float_blend: bool,
    pub norm16: bool,
    pub render_snorm: bool,
}

impl FormatExtensions {
    pub fn found(ext: &std::collections::BTreeSet<&'static str>) -> FormatExtensions {
        FormatExtensions {
            color_buffer_float: ext.contains("EXT_color_buffer_float"),
            float_linear: ext.contains("OES_texture_float_linear"),
            float_blend: ext.contains("EXT_float_blend"),
            norm16: ext.contains("EXT_texture_norm16"),
            render_snorm: ext.contains("EXT_render_snorm"),
        }
    }
}

/// The unsigned integer twin of a float format GL does not read back (16-bit floats, and the packed
/// R11G11B10 in one 32-bit word), whose bits a draw copies into it so they can be read back.
pub fn bits_twin(format: Format) -> Option<Format> {
    Some(match format {
        Format::R16Float => Format::R16Uint,
        Format::Rg16Float => Format::Rg16Uint,
        Format::Rgba16Float => Format::Rgba16Uint,
        Format::Rg11b10Float => Format::R32Uint,
        _ => return None,
    })
}

/// What `format` can be used for on a context with `ext`.
pub fn uses(format: Format, ext: FormatExtensions) -> FormatUse {
    // Block-compressed formats are sampled natively or decoded, and read back from the host copy.
    if crate::host::decoded(format).is_some() {
        return FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
    }
    let Some(gl) = gl_format(format) else {
        return FormatUse::empty();
    };
    let norm16 = matches!(gl.class, Class::Norm { .. }) && matches!(gl.ty, UNSIGNED_SHORT | SHORT);
    if norm16 && !ext.norm16 {
        return FormatUse::empty();
    }
    let target = FormatUse::RENDER | FormatUse::BLEND;
    let mut uses = FormatUse::SAMPLE
        | match gl.class {
            Class::Norm { render: true } => FormatUse::FILTER | target,
            Class::Norm { render: false } if ext.render_snorm => FormatUse::FILTER | target,
            Class::Norm { render: false } | Class::SharedExponent => FormatUse::FILTER,
            Class::Half | Class::SmallFloat if ext.color_buffer_float => FormatUse::FILTER | target,
            Class::Half | Class::SmallFloat => FormatUse::FILTER,
            Class::Single => {
                let mut u = FormatUse::empty();
                u.set(FormatUse::FILTER, ext.float_linear);
                // RGB32F is never colour-renderable.
                if ext.color_buffer_float && gl.format != RGB {
                    u |= FormatUse::RENDER;
                    u.set(FormatUse::BLEND, ext.float_blend);
                }
                u
            }
            Class::Int { render } if render => FormatUse::RENDER,
            Class::Int { .. } => FormatUse::empty(),
            Class::Depth => FormatUse::DEPTH_STENCIL,
        };
    // B4G4R4A4 is sampled only, as Direct3D 11 hardware and the WebGPU back end have it.
    if format == Format::Bgra4Unorm {
        uses -= target;
    }
    // Depth and stencil read back by draws into a colour texture; stencil is sampled with
    // WEBGL_stencil_texturing or from a copy filled by draws.
    // Formats GL cannot read back are read from the host copy, which draws into signed normalised
    // formats leave behind.
    let held = crate::host::kept(format) && !(signed_normalised(format) && ext.render_snorm);
    // 16-bit and packed floats read back through the bits of their integer twin, drawn into it.
    let twin = bits_twin(format).is_some();
    if read_as(format, ext).is_some() || gl.class == Class::Depth || held || twin {
        uses |= FormatUse::COPY_OUT;
    }
    // The canvas's drawing buffer is RGBA8, which surfaces present into; BGRA8 is stored as RGBA8.
    if matches!(format, Format::Rgba8Unorm | Format::Bgra8Unorm) {
        uses |= FormatUse::PRESENT;
    }
    uses
}

/// How `readPixels` reads a format's texels: always four components of `ty` (`comp_read` bytes each),
/// of which the texel keeps `n` from component `first`, each the low `comp_kept` bytes, red and
/// blue swapped where `swap` is set; or, where `fields` is set, RGBA bytes packed again into a
/// 16-bit packed format's fields ([`packed_fields`]; 8 bits keep every narrower value apart, so the
/// packing is exact).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadAs {
    pub format: u32,
    pub ty: u32,
    pub comp_read: usize,
    pub comp_kept: usize,
    pub n: usize,
    pub first: usize,
    pub swap: bool,
    pub fields: Option<Fields>,
}

impl ReadAs {
    pub fn texel_read(self) -> usize {
        4 * self.comp_read
    }

    pub fn texel_kept(self) -> usize {
        self.n * self.comp_kept
    }

    /// Whether `readPixels`' bytes differ from the texels' own.
    pub fn repacks(self) -> bool {
        self.texel_read() != self.texel_kept() || self.swap
    }

    /// Whether the host converts the values read, not only keeps part of each texel's bytes
    /// ([`wcontainers_gpu::SlowPath::FormatConverted`]).
    pub fn converts(self) -> bool {
        self.swap || self.fields.is_some()
    }

    /// Keeps each texel's stored bytes of `read`, texels as `readPixels` wrote them, in `kept`.
    pub fn repack(self, read: &[u8], kept: &mut [u8]) {
        let (r, k) = (self.texel_read(), self.texel_kept());
        if let Some(fields) = self.fields {
            // Each byte rounded to the nearest step of its field, blue's field lowest.
            let byte = u32::from(u8::MAX);
            for (from, to) in read.chunks_exact(r).zip(kept.chunks_exact_mut(k)) {
                let &[r, g, b, a] = from else { continue };
                let bgra = [b, g, r, a];
                let (packed, _) =
                    bgra.iter()
                        .zip(fields)
                        .fold((0, 0), |(packed, at), (&v, bits)| {
                            let max = (1 << bits) - 1;
                            (
                                packed | ((u32::from(v) * max + byte / 2) / byte) << at,
                                at + bits,
                            )
                        });
                to.copy_from_slice(&(packed as u16).to_le_bytes());
            }
            return;
        }
        if self.swap {
            let n = read.len().min(kept.len());
            kept[..n].copy_from_slice(&read[..n]);
            return swap_red_blue(&mut kept[..n]);
        }
        for (from, to) in read.chunks_exact(r).zip(kept.chunks_exact_mut(k)) {
            for c in 0..self.n {
                let at = (self.first + c) * self.comp_read;
                to[c * self.comp_kept..(c + 1) * self.comp_kept]
                    .copy_from_slice(&from[at..at + self.comp_kept]);
            }
        }
    }
}

/// How GLES 3.0's `readPixels` reads `format` back exactly (its always-supported pairs: normalised
/// bytes as RGBA bytes, integers as RGBA 32-bit integers, and floats as RGBA floats with
/// EXT_color_buffer_float; 16-bit normalised ones as RGBA shorts with EXT_texture_norm16), or None
/// where it cannot: formats that are not colour-renderable, packed floats, 16-bit floats, and signed
/// normalised ones, whose -128 `readPixels` returns as -127 even with EXT_render_snorm.
pub fn read_as(format: Format, ext: FormatExtensions) -> Option<ReadAs> {
    let gl = gl_format(format)?;
    // GLES 3.0 reads 10-10-10-2 surfaces in their own packing as well.
    if gl.ty == UNSIGNED_INT_2_10_10_10_REV {
        return Some(ReadAs {
            format: gl.format,
            ty: gl.ty,
            comp_read: 1,
            comp_kept: 1,
            n: 4,
            first: 0,
            swap: false,
            fields: None,
        });
    }
    // The 16-bit packed formats render, and read back as normalised bytes, packed again on the host.
    if let Some(fields) = packed_fields(format) {
        return Some(ReadAs {
            format: RGBA,
            ty: UNSIGNED_BYTE,
            comp_read: 1,
            comp_kept: 2,
            n: 1,
            first: 0,
            swap: false,
            fields: Some(fields),
        });
    }
    let comp_kept = match gl.ty {
        UNSIGNED_BYTE | BYTE => 1,
        UNSIGNED_SHORT | SHORT => 2,
        UNSIGNED_INT | INT | FLOAT => 4,
        _ => return None,
    };
    let (format_read, ty, comp_read) = match gl.class {
        Class::Norm { render: true } if gl.ty == UNSIGNED_BYTE => (RGBA, UNSIGNED_BYTE, 1),
        Class::Norm { render: true } if gl.ty == UNSIGNED_SHORT && ext.norm16 => {
            (RGBA, UNSIGNED_SHORT, 2)
        }
        Class::Int { render: true } if matches!(gl.ty, BYTE | SHORT | INT) => {
            (RGBA_INTEGER, INT, 4)
        }
        Class::Int { render: true } => (RGBA_INTEGER, UNSIGNED_INT, 4),
        Class::Single if ext.color_buffer_float && gl.format != RGB => (RGBA, FLOAT, 4),
        _ => return None,
    };
    let (_, _, texel) = FORMAT_FACTS[format as usize].block;
    Some(ReadAs {
        format: format_read,
        ty,
        comp_read,
        comp_kept,
        n: texel as usize / comp_kept,
        // 8-bit alpha keeps the fourth component alone.
        first: FORMAT_FACTS[format as usize].components.trailing_zeros() as usize,
        swap: swapped(format),
        fields: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every 16-bit packed texel written as GL stores it, then read as `readPixels` gives GL's
    /// fields (red highest, alpha lowest, each `v * 255 / max` rounded), packs back to itself.
    #[test]
    fn packed_texels_written_and_read_back_keep_their_bits() {
        let ext = FormatExtensions::default();
        for format in [Format::B5g6r5Unorm, Format::Bgr5a1Unorm, Format::Bgra4Unorm] {
            let read = read_as(format, ext).unwrap();
            let [b, g, r, a] = packed_fields(format).unwrap();
            for v in 0..=u16::MAX {
                let mut stored = Vec::new();
                if converts_writes(format) {
                    stored_texels(format, &v.to_le_bytes(), &mut stored);
                } else {
                    stored.extend(v.to_le_bytes());
                }
                let gl = u32::from(u16::from_le_bytes([stored[0], stored[1]]));
                let (mut bytes, mut at) = ([u8::MAX; 4], 0);
                for (c, bits) in [(3, a), (2, b), (1, g), (0, r)] {
                    if bits > 0 {
                        let max = (1 << bits) - 1;
                        let x = (gl >> at) & max;
                        bytes[c] = ((x * 255 + max / 2) / max) as u8;
                        at += bits;
                    }
                }
                let mut kept = [0; 2];
                read.repack(&bytes, &mut kept);
                let used = (1u32 << (b + g + r + a)) - 1;
                assert_eq!(
                    u32::from(u16::from_le_bytes(kept)),
                    u32::from(v) & used,
                    "{format:?}"
                );
            }
        }
    }

    /// 8-bit alpha is stored as (0, 0, 0, a), and read back as its alpha alone.
    #[test]
    fn alpha_texels_widened_and_read_back() {
        let mut stored = Vec::new();
        stored_texels(Format::A8Unorm, &[7, 200], &mut stored);
        assert_eq!(stored, [0, 0, 0, 7, 0, 0, 0, 200]);
        let read = read_as(Format::A8Unorm, FormatExtensions::default()).unwrap();
        let mut kept = [0; 2];
        read.repack(&[1, 2, 3, 7, 4, 5, 6, 200], &mut kept);
        assert_eq!(kept, [7, 200]);
        let clear = clear_value(Format::A8Unorm, [1f32.to_bits(); 4]);
        assert_eq!(clear, [0, 0, 0, 1f32.to_bits()]);
    }
}
