//! Depth: every comparison function, constant, slope-scaled and clamped bias on a 24-bit buffer,
//! clipping against clamping and a narrowed depth range, depth written by the pixel shader (any,
//! greater-equal and less-equal), and each depth format read back.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{colored_program, depth_test, draw_colored, quad};
use crate::d3d11::{Gpu, cell, solid};
use crate::dxbc;
use crate::output::Output;

const RED: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
const GREEN: [f32; 4] = [0.2, 0.85, 0.3, 1.0];
const BLUE: [f32; 4] = [0.2, 0.4, 0.95, 1.0];
const YELLOW: [f32; 4] = [0.95, 0.85, 0.2, 1.0];

pub const FUNCS: [D3D11_COMPARISON_FUNC; 8] = [
    D3D11_COMPARISON_NEVER,
    D3D11_COMPARISON_LESS,
    D3D11_COMPARISON_EQUAL,
    D3D11_COMPARISON_LESS_EQUAL,
    D3D11_COMPARISON_GREATER,
    D3D11_COMPARISON_NOT_EQUAL,
    D3D11_COMPARISON_GREATER_EQUAL,
    D3D11_COMPARISON_ALWAYS,
];

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    colored_program(gpu)?;
    let c = &gpu.context;
    let solid_state = gpu.rasterizer(&solid())?;

    // Each function tests a flat quad at 0.5 against a slope from 0.1 to 0.9.
    let (target, rtv) = gpu.target(256, 128, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (_, dsv) = gpu.depth(256, 128, DXGI_FORMAT_D32_FLOAT)?;
    gpu.begin(&rtv, Some(&dsv), 256, 128);
    unsafe { c.RSSetState(&solid_state) };
    for (i, &func) in FUNCS.iter().enumerate() {
        unsafe {
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
            c.OMSetDepthStencilState(
                &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_ALWAYS, true))?,
                0,
            );
        }
        draw_colored(gpu, &quad(-0.9, -0.9, 0.9, 0.9, 0.1, 0.9, RED))?;
        unsafe { c.OMSetDepthStencilState(&gpu.depth_stencil(&depth_test(func, false))?, 0) };
        draw_colored(gpu, &quad(-0.7, -0.5, 0.7, 0.5, 0.5, 0.5, GREEN))?;
    }
    outputs.push(Output::picture("funcs", gpu.read(&target, 0, 0)?));

    // A quad drawn again over itself, pulled forward by each bias; the second wins where the bias
    // beats the test.
    let (target, rtv) = gpu.target(256, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = gpu.depth(256, 64, DXGI_FORMAT_D24_UNORM_S8_UINT)?;
    gpu.begin(&rtv, Some(&dsv), 256, 64);
    let biases = [
        (0, 0.0, 0.0),
        (-3, 0.0, 0.0),
        (0, -1.5, 0.0),
        (-3000, -1.5, -0.00002),
    ];
    for (i, (bias, slope, clamp)) in biases.into_iter().enumerate() {
        unsafe {
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
            c.RSSetState(&solid_state);
            c.OMSetDepthStencilState(
                &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_LESS, true))?,
                0,
            );
        }
        draw_colored(gpu, &quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, BLUE))?;
        let biased = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            DepthBias: bias,
            SlopeScaledDepthBias: slope,
            DepthBiasClamp: clamp,
            ..solid()
        })?;
        unsafe { c.RSSetState(&biased) };
        draw_colored(gpu, &quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, YELLOW))?;
    }
    outputs.push(Output::picture("bias", gpu.read(&target, 0, 0)?));
    outputs.push(Output::picture("bias_depth", gpu.read(&depth, 0, 0)?));

    // A quad from depth -0.5 to 1.5: clipped, then clamped; then 0 to 1 into the range 0.25 to 0.75.
    let (target, rtv) = gpu.target(192, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = gpu.depth(192, 64, DXGI_FORMAT_D32_FLOAT)?;
    gpu.begin(&rtv, Some(&dsv), 192, 64);
    unsafe {
        c.OMSetDepthStencilState(
            &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_ALWAYS, true))?,
            0,
        )
    };
    for (i, clip) in [true, false].into_iter().enumerate() {
        let state = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            DepthClipEnable: clip.into(),
            ..solid()
        })?;
        unsafe {
            c.RSSetState(&state);
            c.RSSetViewports(Some(&[cell(i as u32, 3, 64.0)]));
        }
        draw_colored(gpu, &quad(-0.9, -0.9, 0.9, 0.9, -0.5, 1.5, GREEN))?;
    }
    unsafe {
        c.RSSetState(&solid_state);
        c.RSSetViewports(Some(&[D3D11_VIEWPORT {
            MinDepth: 0.25,
            MaxDepth: 0.75,
            ..cell(2, 3, 64.0)
        }]));
    }
    draw_colored(gpu, &quad(-0.9, -0.9, 0.9, 0.9, 0.0, 1.0, GREEN))?;
    outputs.push(Output::picture("clip", gpu.read(&target, 0, 0)?));
    outputs.push(Output::picture("clip_depth", gpu.read(&depth, 0, 0)?));

    // Depth from the pixel shader.
    let (target, rtv) = gpu.target(192, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = gpu.depth(192, 64, DXGI_FORMAT_D32_FLOAT)?;
    gpu.begin(&rtv, Some(&dsv), 192, 64);
    unsafe {
        c.RSSetState(&solid_state);
        c.OMSetDepthStencilState(
            &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_LESS_EQUAL, true))?,
            0,
        );
    }
    for (i, ps) in [
        dxbc!("depth.ps_depth"),
        dxbc!("depth.ps_greater"),
        dxbc!("depth.ps_less"),
    ]
    .into_iter()
    .enumerate()
    {
        unsafe {
            c.PSSetShader(&gpu.ps(ps)?, None);
            c.RSSetViewports(Some(&[cell(i as u32, 3, 64.0)]));
        }
        draw_colored(gpu, &quad(-0.8, -0.8, 0.8, 0.8, 0.3, 0.6, YELLOW))?;
    }
    outputs.push(Output::picture("shader", gpu.read(&target, 0, 0)?));
    outputs.push(Output::picture("shader_depth", gpu.read(&depth, 0, 0)?));

    // Each depth format, cleared with a stencil value, then a slope drawn into it without colour.
    colored_program(gpu)?;
    for (name, format) in [
        ("d16", DXGI_FORMAT_D16_UNORM),
        ("d24s8", DXGI_FORMAT_D24_UNORM_S8_UINT),
        ("d32", DXGI_FORMAT_D32_FLOAT),
        ("d32s8", DXGI_FORMAT_D32_FLOAT_S8X24_UINT),
    ] {
        let (depth, dsv) = gpu.depth(64, 64, format)?;
        unsafe {
            c.ClearDepthStencilView(
                &dsv,
                D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
                0.75,
                0x5a,
            );
            c.OMSetRenderTargets(None, &dsv);
            c.OMSetDepthStencilState(
                &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_LESS, true))?,
                0,
            );
        }
        gpu.viewport(64, 64);
        draw_colored(gpu, &quad(-0.9, -0.6, 0.9, 0.6, 0.0, 1.0, GREEN))?;
        outputs.push(Output::picture(
            &format!("format_{name}"),
            gpu.read(&depth, 0, 0)?,
        ));
    }
    Ok(outputs)
}
