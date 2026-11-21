//! Feature levels: a device asked for each level from 9_1 to 11_1 alone and for lists of them, and
//! what comes back (answers a less capable device may give lower, and rules every device keeps);
//! then a triangle drawn on a 9_1 device with level 9_1 shaders.

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{colored_layout, draw_colored};
use crate::d3d11::{Gpu, LEVELS, create, level_name, solid};
use crate::dxbc;
use crate::output::Output;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    // What each request gives: a level, or None when refused.
    let ask = |levels: &[D3D_FEATURE_LEVEL]| -> Option<D3D_FEATURE_LEVEL> {
        create(gpu.driver, gpu.debug, levels).ok().map(|d| d.level)
    };
    let answer = |got: Option<D3D_FEATURE_LEVEL>| got.map_or("refused".to_owned(), level_name);
    let (mut caps, mut rules) = (String::new(), String::new());
    let mut taken = Vec::new();
    for (label, level) in LEVELS {
        let got = ask(&[level]);
        writeln!(caps, "only {label}: {}", answer(got))?;
        writeln!(
            rules,
            "only {label}: made at that level or refused: {}",
            got.is_none_or(|g| g == level)
        )?;
        if got == Some(level) {
            taken.push(level);
        }
    }
    let all: Vec<D3D_FEATURE_LEVEL> = LEVELS.iter().rev().map(|(_, l)| *l).collect();
    let lists: [(&str, &[D3D_FEATURE_LEVEL]); 4] = [
        ("11_1 down to 9_1", &all),
        ("11_0 down to 9_1", &all[1..]),
        ("9_1 up to 11_1", &LEVELS.map(|(_, l)| l)),
        (
            "10_1 and 10_0",
            &[D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0],
        ),
    ];
    for (label, levels) in lists {
        let got = ask(levels);
        writeln!(caps, "{label}: {}", answer(got))?;
        let first = levels.iter().copied().find(|l| taken.contains(l));
        writeln!(
            rules,
            "{label}: the first level of the list taken alone: {}",
            got == first
        )?;
    }
    let mut outputs = vec![Output::caps("results", caps), Output::text("rules", rules)];
    if !taken.contains(&D3D_FEATURE_LEVEL_9_1) {
        outputs.push(Output::absent(
            "9_1",
            "feature level 9_1, which the device refuses".to_owned(),
        ));
        return Ok(outputs);
    }

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
    outputs.push(Output::picture("9_1", picture));
    Ok(outputs)
}
