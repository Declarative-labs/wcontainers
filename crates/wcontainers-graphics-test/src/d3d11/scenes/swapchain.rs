//! Swap chains on a window that is never shown, both models games use: the flip model through
//! CreateSwapChainForHwnd and the older blit model through CreateSwapChain. Each draws into its
//! back buffer and reads it before presenting, presents, resizes its buffers and draws again.
//! Records whether each call succeeded and the sizes and counts it reports; needs a session with
//! a desktop (`gpu-job --desktop`).

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use windows::core::{Interface, w};

use super::{colored_program, draw_colored, quad};
use crate::d3d11::{Gpu, solid};
use crate::output::Output;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let factory: IDXGIFactory2 = unsafe {
        gpu.device
            .cast::<IDXGIDevice>()?
            .GetAdapter()?
            .GetParent()?
    };
    let mut outputs = Vec::new();
    let mut text = String::new();
    for model in ["flip", "blit"] {
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("swap chain"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                200,
                200,
                None,
                None,
                None,
                None,
            )?
        };
        let result = chain(gpu, &factory, window, model, &mut outputs, &mut text);
        unsafe { DestroyWindow(window)? };
        result?;
    }
    outputs.push(Output::text("results", text));
    Ok(outputs)
}

fn chain(
    gpu: &Gpu,
    factory: &IDXGIFactory2,
    window: HWND,
    model: &str,
    outputs: &mut Vec<Output>,
    text: &mut String,
) -> Result<()> {
    let chain: IDXGISwapChain = if model == "flip" {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: 128,
            Height: 96,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            Scaling: DXGI_SCALING_STRETCH,
            ..Default::default()
        };
        unsafe { factory.CreateSwapChainForHwnd(&gpu.device, window, &desc, None, None)? }.cast()?
    } else {
        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: 128,
                Height: 96,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 1,
            OutputWindow: window,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            Flags: 0,
        };
        let mut chain = None;
        unsafe {
            factory
                .CreateSwapChain(&gpu.device, &desc, &mut chain)
                .ok()?
        };
        chain.ok_or_else(|| anyhow::anyhow!("CreateSwapChain gave no swap chain"))?
    };
    colored_program(gpu)?;
    for (part, (width, height)) in [("first", (128, 96)), ("resized", (64, 80))] {
        if part == "resized" {
            unsafe {
                chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?
            };
        }
        let back: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
        let rtv = gpu.rtv(&back.cast()?, None)?;
        gpu.begin(&rtv, None, width, height);
        unsafe { gpu.context.RSSetState(&gpu.rasterizer(&solid())?) };
        draw_colored(
            gpu,
            &quad(-0.7, -0.6, 0.5, 0.8, 0.5, 0.5, [0.9, 0.5, 0.2, 1.0]),
        )?;
        draw_colored(
            gpu,
            &quad(-0.2, -0.9, 0.9, 0.1, 0.5, 0.5, [0.1, 0.4, 0.8, 1.0]),
        )?;
        outputs.push(Output::picture(
            &format!("{model}-{part}"),
            gpu.read(&back, 0, 0)?,
        ));
        unsafe { gpu.context.OMSetRenderTargets(None, None) };
        drop((rtv, back));
        // Success codes (an occluded window's included) are what a program sees as success.
        let presented = unsafe { chain.Present(0, DXGI_PRESENT(0)) };
        let desc = unsafe { chain.GetDesc()? };
        writeln!(
            text,
            "{model} {part}: present {}, buffers {} by {}, format {}, count {}",
            if presented.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
            desc.BufferDesc.Width,
            desc.BufferDesc.Height,
            desc.BufferDesc.Format.0,
            desc.BufferCount
        )?;
    }
    Ok(())
}
