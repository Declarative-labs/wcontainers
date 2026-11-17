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
