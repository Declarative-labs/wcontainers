//! The graphics test rig: reference scenes, the renderers that draw them, and picture comparison.

pub mod check;
pub mod compare;
pub mod output;
pub mod scene;
pub mod webgpu;

#[cfg(windows)]
pub mod d3d11;
#[cfg(windows)]
pub mod d3d12;
#[cfg(windows)]
mod debug;
#[cfg(windows)]
pub mod texels;

/// Compiled DXIL of an entry point of shaders/<file>.hlsl, named "<file>.<entry>".
#[cfg(windows)]
#[macro_export]
macro_rules! dxil {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/dxil/", $name, ".dxil")).as_slice()
    };
}

use std::path::Path;

use anyhow::Context;
use image::{DynamicImage, RgbaImage};

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

/// The adapter's name, or an error when a hardware device landed on a software adapter (WARP or the
/// Microsoft Basic Render Driver), whose pictures would pass every comparison with WARP.
#[cfg(windows)]
fn adapter_name(
    driver: Driver,
    desc: &windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1,
) -> anyhow::Result<String> {
    use windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_FLAG_SOFTWARE;
    let name = String::from_utf16_lossy(&desc.Description)
        .trim_end_matches('\0')
        .to_owned();
    let software = desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0;
    anyhow::ensure!(
        driver == Driver::Warp || !software,
        "a hardware device was asked for, but the adapter is the software {name}"
    );
    Ok(name)
}

/// The Direct3D libraries a replacement may stand in for.
pub const LIBRARIES: [&str; 3] = ["d3d11.dll", "d3d12.dll", "dxgi.dll"];

/// Where this process loaded each Direct3D library from ("<name> from <path>" or "<name> from not
/// loaded"), for showing which ones a program really runs through.
#[cfg(windows)]
pub fn library_paths() -> String {
    use wcontainers_win32::{StandaloneWin32, Win32};
    let win32 = wcontainers_win32_windows::WindowsWin32::new(());
    let mut paths = Vec::new();
    for name in LIBRARIES {
        let path = win32
            .module_path(name)
            .unwrap_or_else(|| "not loaded".to_owned());
        paths.push(format!("{name} from {path}"));
    }
    paths.join(", ")
}

/// A drawn scene and the adapter that drew it.
pub struct Picture {
    pub adapter: String,
    pub image: DynamicImage,
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

/// The file extension a picture is saved under without loss: PNG holds 8 and 16-bit channels,
/// OpenEXR 32-bit floats.
pub fn extension(image: &DynamicImage) -> anyhow::Result<&'static str> {
    Ok(match compare::Depth::of(image)? {
        compare::Depth::Eight | compare::Depth::Sixteen => "png",
        compare::Depth::Float => "exr",
    })
}

/// Saves a picture losslessly; the path's extension must be the one `extension` gives.
pub fn save(image: &DynamicImage, path: &Path) -> anyhow::Result<()> {
    let wanted = extension(image)?;
    anyhow::ensure!(
        path.extension().is_some_and(|e| e == wanted),
        "{} must be saved as .{wanted}",
        path.display()
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

/// Loads a picture at its own depth.
pub fn load(path: &Path) -> anyhow::Result<DynamicImage> {
    image::open(path).with_context(|| format!("reading {}", path.display()))
}
