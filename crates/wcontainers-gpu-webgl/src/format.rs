//! How each interface format is stored in GL ES 3.0, and what GL lets it do.

use glow::*;
use wcontainers_gpu::{Format, FormatUse};

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
    /// Normalised fixed point: filterable, and renderable and blendable where `render` is set.
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
        // Direct3D's B5G6R5 has red in the top bits, as GL's 5_6_5 does.
        F::B5g6r5Unorm => f(RGB565, RGB, UNSIGNED_SHORT_5_6_5, norm),
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

/// Whether the interface's bytes of `format` hold blue first, where GL stores red first
/// ([`wcontainers_gpu::SlowPath::FormatConverted`] on the way in and out).
pub fn swapped(format: Format) -> bool {
    matches!(format, Format::Bgra8Unorm | Format::Bgra8UnormSrgb)
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
}

impl FormatExtensions {
    pub fn found(ext: &std::collections::BTreeSet<&'static str>) -> FormatExtensions {
        FormatExtensions {
            color_buffer_float: ext.contains("EXT_color_buffer_float"),
            float_linear: ext.contains("OES_texture_float_linear"),
            float_blend: ext.contains("EXT_float_blend"),
            norm16: ext.contains("EXT_texture_norm16"),
        }
    }
}

/// What `format` can be used for on a context with `ext`.
pub fn uses(format: Format, ext: FormatExtensions) -> FormatUse {
    // Block-compressed formats are sampled natively or decoded, and read back from the host copy.
    if crate::host::compressed(format).is_some() {
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
    // Depth and stencil read back by draws into a colour texture; stencil is sampled with
    // WEBGL_stencil_texturing or from a copy filled by draws.
    // Formats GL cannot read back are read from the host copy.
    if read_as(format, ext).is_some() || gl.class == Class::Depth || crate::host::kept(format) {
        uses |= FormatUse::COPY_OUT;
    }
    // The canvas's drawing buffer is RGBA8, which surfaces present into; BGRA8 is stored as RGBA8.
    if matches!(format, Format::Rgba8Unorm | Format::Bgra8Unorm) {
        uses |= FormatUse::PRESENT;
    }
    uses
}

/// How `readPixels` reads a format's texels: always four components of `ty` (`comp_read` bytes each),
/// of which the texel keeps the first `n`, each the low `comp_kept` bytes, red and blue swapped
/// where `swap` is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadAs {
    pub format: u32,
    pub ty: u32,
    pub comp_read: usize,
    pub comp_kept: usize,
    pub n: usize,
    pub swap: bool,
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

    /// Keeps each texel's stored bytes of `read`, texels as `readPixels` wrote them, in `kept`.
    pub fn repack(self, read: &[u8], kept: &mut [u8]) {
        if self.swap {
            let n = read.len().min(kept.len());
            kept[..n].copy_from_slice(&read[..n]);
            return swap_red_blue(&mut kept[..n]);
        }
        let (r, k) = (self.texel_read(), self.texel_kept());
        for (from, to) in read.chunks_exact(r).zip(kept.chunks_exact_mut(k)) {
            for c in 0..self.n {
                to[c * self.comp_kept..(c + 1) * self.comp_kept].copy_from_slice(
                    &from[c * self.comp_read..c * self.comp_read + self.comp_kept],
                );
            }
        }
    }
}

/// How GLES 3.0's `readPixels` reads `format` back exactly (its always-supported pairs: normalised
/// bytes as RGBA bytes, integers as RGBA 32-bit integers, and floats as RGBA floats with
/// EXT_color_buffer_float; 16-bit normalised ones as RGBA shorts with EXT_texture_norm16), or None
/// where it cannot: formats that are not colour-renderable, packed ones and 16-bit floats.
pub fn read_as(format: Format, ext: FormatExtensions) -> Option<ReadAs> {
    let gl = gl_format(format)?;
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
    let (_, _, texel) = format.block();
    Some(ReadAs {
        format: format_read,
        ty,
        comp_read,
        comp_kept,
        n: texel as usize / comp_kept,
        swap: swapped(format),
    })
}
