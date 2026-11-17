//! Feature levels: a device asked for each level from 9_1 to 11_1 alone and for lists of them, and
//! what comes back; then a triangle drawn on a 9_1 device with level 9_1 shaders.

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{colored_layout, draw_colored};
use crate::d3d11::{Gpu, create, solid};
use crate::dxbc;
use crate::output::Output;

const LEVELS: [(&str, D3D_FEATURE_LEVEL); 7] = [
    ("9_1", D3D_FEATURE_LEVEL_9_1),
    ("9_2", D3D_FEATURE_LEVEL_9_2),
    ("9_3", D3D_FEATURE_LEVEL_9_3),
    ("10_0", D3D_FEATURE_LEVEL_10_0),
    ("10_1", D3D_FEATURE_LEVEL_10_1),
    ("11_0", D3D_FEATURE_LEVEL_11_0),
    ("11_1", D3D_FEATURE_LEVEL_11_1),
];

fn name(level: D3D_FEATURE_LEVEL) -> String {
    LEVELS
        .iter()
        .find(|(_, l)| *l == level)
        .map_or_else(|| format!("{:#x}", level.0), |(n, _)| n.to_string())
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut text = String::new();
    let mut ask = |label: &str, levels: &[D3D_FEATURE_LEVEL]| -> Result<()> {
        match create(gpu.driver, gpu.debug, levels) {
            Ok(device) => writeln!(text, "{label}: {}", name(device.level))?,
            Err(e) => writeln!(text, "{label}: refused ({})", e.root_cause())?,
        }
        Ok(())
    };
    for (label, level) in LEVELS {
        ask(&format!("only {label}"), &[level])?;
    }
    let all: Vec<D3D_FEATURE_LEVEL> = LEVELS.iter().rev().map(|(_, l)| *l).collect();
    ask("11_1 down to 9_1", &all)?;
    ask("11_0 down to 9_1", &all[1..])?;
    ask("9_1 up to 11_1", &LEVELS.map(|(_, l)| l))?;
    ask(
        "10_1 and 10_0",
        &[D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0],
    )?;

    let small = create(gpu.driver, gpu.debug, &[D3D_FEATURE_LEVEL_9_1])?;
    let (target, rtv) = small.target(64, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    small.begin(&rtv, None, 64, 64);
    small.program(
        dxbc!("common.vs_color_9_1"),
        dxbc!("common.ps_color_9_1"),
        &colored_layout(),
    )?;
    unsafe { small.context.RSSetState(&small.rasterizer(&solid())?) };
    let corners = [
        [0.0, 0.8, 0.5, 1.0, 0.5, 0.0, 1.0],
        [0.8, -0.7, 0.5, 0.0, 1.0, 0.5, 1.0],
        [-0.8, -0.7, 0.5, 0.5, 0.0, 1.0, 1.0],
    ];
    draw_colored(&small, &corners)?;
    let picture = small.read(&target, 0, 0)?;
    small.check_debug()?;
    Ok(vec![
        Output::text("results", text),
        Output::picture("9_1", picture),
    ])
}
