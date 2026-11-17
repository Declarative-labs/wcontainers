//! The outputs DXGI enumerates: the host's displays, as the host describes them, and their modes.

/// One display mode (`DXGI_MODE_DESC`): size, refresh rate as a fraction, and format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_numerator: u32,
    pub refresh_denominator: u32,
    /// A `DXGI_FORMAT` value.
    pub format: u32,
}

/// A host display, as the host's graphics hook (a native shell: the system's monitor) reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDesc {
    /// The device name DXGI reports (`\\.\DISPLAY1` on Windows).
    pub name: String,
    /// The desktop area it shows: left, top, right, bottom.
    pub desktop: [i32; 4],
    pub refresh_numerator: u32,
    pub refresh_denominator: u32,
}

impl OutputDesc {
    pub fn width(&self) -> u32 {
        (self.desktop[2] - self.desktop[0]) as u32
    }

    pub fn height(&self) -> u32 {
        (self.desktop[3] - self.desktop[1]) as u32
    }

    /// The modes of `format` (`GetDisplayModeList`). The display's size cannot be changed from a
    /// container or a browser page, so the one mode is the current one; a format that is not a back
    /// buffer format has none.
    pub fn modes(&self, format: u32) -> Vec<DisplayMode> {
        if !crate::format::BACK_BUFFER_FORMATS.contains(&format) {
            return Vec::new();
        }
        vec![DisplayMode {
            width: self.width(),
            height: self.height(),
            refresh_numerator: self.refresh_numerator,
            refresh_denominator: self.refresh_denominator,
            format,
        }]
    }

    /// The mode closest to `wanted` (`FindClosestMatchingMode`): the current one, in the wanted
    /// format where that is a back buffer format and otherwise in `R8G8B8A8_UNORM`.
    pub fn closest_mode(&self, wanted: &DisplayMode) -> DisplayMode {
        let format = if crate::format::BACK_BUFFER_FORMATS.contains(&wanted.format) {
            wanted.format
        } else {
            crate::format::BACK_BUFFER_FORMATS[2]
        };
        self.modes(format)[0]
    }
}

/// `DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709`: the colour space every output shows, SDR sRGB.
pub const COLOR_SPACE_SRGB: u32 = 0;
/// `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709`: linear light, which 16-bit float back buffers hold.
pub const COLOR_SPACE_LINEAR: u32 = 1;

/// Whether a swap chain with back buffers of `format` (a `DXGI_FORMAT`) presents in colour space
/// `space` (`CheckColorSpaceSupport`'s present bit): linear light from 16-bit float buffers, sRGB
/// from the others, as Windows answers on an SDR display. No HDR or studio-range space is
/// presented: values beyond one are clipped, as on an SDR display.
pub fn presents_in(format: u32, space: u32) -> bool {
    const R16G16B16A16_FLOAT: u32 = 10;
    if format == R16G16B16A16_FLOAT {
        space == COLOR_SPACE_LINEAR
    } else {
        space == COLOR_SPACE_SRGB
    }
}

/// What `IDXGIOutput6::GetDesc1` reports of every output beyond its place: an SDR display with
/// sRGB's primaries and white point (BT.709, D65), and the luminance Windows reports for an SDR
/// display. The host says nothing more of its displays.
pub mod sdr {
    pub const BITS_PER_COLOR: u32 = 8;
    /// Red, green, blue and white, as CIE 1931 x and y.
    pub const PRIMARIES: [[f32; 2]; 4] =
        [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06], [0.3127, 0.3290]];
    pub const MIN_LUMINANCE: f32 = 0.5;
    pub const MAX_LUMINANCE: f32 = 270.0;
    pub const MAX_FULL_FRAME_LUMINANCE: f32 = 270.0;
}
