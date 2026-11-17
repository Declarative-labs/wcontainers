//! The graphics test rig: reference scenes, the renderers that draw them, and picture comparison.

pub mod compare;
pub mod scene;
pub mod webgpu;

#[cfg(windows)]
pub mod d3d11;
#[cfg(windows)]
pub mod d3d12;
#[cfg(windows)]
mod debug;
#[cfg(windows)]
mod hlsl;

use std::path::Path;

use anyhow::Context;
use image::RgbaImage;

/// Which device draws a Direct3D reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    /// The first adapter the system lists, the real GPU.
    Hardware,
    /// Microsoft's software rasteriser.
    Warp,
}

impl Driver {
    pub fn parse(arg: &str) -> anyhow::Result<Driver> {
        match arg {
            "hardware" => Ok(Driver::Hardware),
            "warp" => Ok(Driver::Warp),
            _ => anyhow::bail!("unknown driver {arg}; drivers are hardware and warp"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Driver::Hardware => "hardware",
            Driver::Warp => "warp",
        }
    }
}

/// A drawn scene and the adapter that drew it.
pub struct Picture {
    pub adapter: String,
    pub image: RgbaImage,
}

/// Builds a picture from rows of RGBA8 texels that may be padded to `pitch` bytes.
pub fn picture_from_rows(width: u32, height: u32, rows: &[u8], pitch: usize) -> RgbaImage {
    let row = width as usize * 4;
    let mut texels = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        texels.extend_from_slice(&rows[y * pitch..y * pitch + row]);
    }
    RgbaImage::from_raw(width, height, texels).expect("rows hold the whole picture")
}

pub fn save(image: &RgbaImage, path: &Path) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn load(path: &Path) -> anyhow::Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("reading {}", path.display()))?
        .into_rgba8())
}
