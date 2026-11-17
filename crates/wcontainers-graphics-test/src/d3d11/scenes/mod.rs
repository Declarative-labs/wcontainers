//! Every Direct3D 11 scene by name. Each is deterministic: fixed data, no clocks, seeded when random.

mod basic;
mod blend;
mod draws;
mod topology;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::D3D11_INPUT_ELEMENT_DESC;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
};
use windows::core::s;

use super::{Gpu, element};
use crate::output::Output;

pub type Scene = fn(&Gpu) -> Result<Vec<Output>>;

pub const SCENES: &[(&str, Scene)] = &[
    ("triangle", basic::triangle),
    ("quads", basic::quads),
    ("instanced", basic::instanced),
    ("topology", topology::run),
    ("draws", draws::run),
    ("blend", blend::run),
];

pub fn find(name: &str) -> Option<Scene> {
    SCENES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// A vertex of common.hlsl's `vs_color`: position x, y, z and colour r, g, b, a.
pub type Colored = [f32; 7];

pub fn colored_layout() -> [D3D11_INPUT_ELEMENT_DESC; 2] {
    [
        element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, 0),
    ]
}

/// A colour of a hue wheel, for telling vertices and cells apart.
pub fn hue(i: u32, n: u32) -> [f32; 3] {
    let h = i as f32 / n as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}
