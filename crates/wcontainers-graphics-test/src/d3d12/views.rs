//! Direct3D 11's views scene drawn through Direct3D 12: shader resource views of a mip range, of a
//! layer range, of a typeless texture as unsigned normalised and as sRGB, of floats as their bits,
//! and of the depth and stencil planes of a depth buffer; and render target views of one mip and
//! of one layer.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::states::{Immediate, RT};
use super::textures::{filled_texture, texture_desc, texture_root};
use super::{Gpu, transition};
use crate::d3d11::cell;
use crate::d3d11::scenes::depth_test;
use crate::d3d11::scenes::views as views11;
use crate::dxil;
use crate::output::Output;

const RGBA: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;
const TEX2D: D3D12_RESOURCE_DIMENSION = D3D12_RESOURCE_DIMENSION_TEXTURE2D;

fn srv_2d(
    format: DXGI_FORMAT,
    first_mip: u32,
    mips: u32,
    plane: u32,
) -> D3D12_SHADER_RESOURCE_VIEW_DESC {
    let mut view = D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: format,
        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        ..Default::default()
    };
    view.Anonymous.Texture2D = D3D12_TEX2D_SRV {
        MostDetailedMip: first_mip,
        MipLevels: mips,
        PlaneSlice: plane,
        ResourceMinLODClamp: 0.0,
    };
    view
}

/// A render target of `desc` with the clear colour as its optimised clear value.
fn render_target(gpu: &Gpu, desc: &D3D12_RESOURCE_DESC) -> Result<ID3D12Resource> {
    let clear = D3D12_CLEAR_VALUE {
        Format: desc.Format,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            Color: crate::scene::CLEAR,
        },
    };
    let mut desc = *desc;
    desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    gpu.committed(D3D12_HEAP_TYPE_DEFAULT, &desc, RT, Some(&clear))
}

pub fn views(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut im = Immediate::new(gpu)?;
    let heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 8, true)?;
    let mut shown: Vec<(
        &'static [u8],
        ID3D12Resource,
        Option<D3D12_SHADER_RESOURCE_VIEW_DESC>,
    )> = Vec::new();

    // Six mips, each its own colour: all of them, and from mip 2 on.
    let mips = filled_texture(
        gpu,
        &texture_desc(TEX2D, [32, 32, 1], 6, RGBA),
        &views11::mip_levels(),
    )?;
    shown.push((dxil!("views.ps_level0"), mips.clone(), None));
    shown.push((dxil!("views.ps_level0"), mips, Some(srv_2d(RGBA, 2, 3, 0))));

    // Four layers, viewed from layer 1 for three.
    let array = filled_texture(
        gpu,
        &texture_desc(TEX2D, [16, 16, 4], 1, RGBA),
        &views11::layers(),
    )?;
    let mut range = D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: RGBA,
        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2DARRAY,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        ..Default::default()
    };
    range.Anonymous.Texture2DArray = D3D12_TEX2D_ARRAY_SRV {
        MipLevels: 1,
        FirstArraySlice: 1,
        ArraySize: 3,
        ..Default::default()
    };
    shown.push((dxil!("views.ps_array"), array, Some(range)));

    // A typeless 4 by 4 gradient read as unsigned normalised and as sRGB, linearly filtered.
    let typeless = filled_texture(
        gpu,
        &texture_desc(TEX2D, [4, 4, 1], 1, DXGI_FORMAT_R8G8B8A8_TYPELESS),
        &[views11::gradient()],
    )?;
    shown.push((
        dxil!("views.ps_level0"),
        typeless.clone(),
        Some(srv_2d(RGBA, 0, 1, 0)),
    ));
    shown.push((
        dxil!("views.ps_level0"),
        typeless,
        Some(srv_2d(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, 0, 1, 0)),
    ));

    // Floats viewed as their bits.
    let bits = filled_texture(
        gpu,
        &texture_desc(TEX2D, [16, 16, 1], 1, DXGI_FORMAT_R32_TYPELESS),
        &[bytemuck::cast_slice(&views11::floats()).to_vec()],
    )?;
    shown.push((
        dxil!("views.ps_uint"),
        bits,
        Some(srv_2d(DXGI_FORMAT_R32_UINT, 0, 1, 0)),
    ));

    // A depth buffer drawn with a slope and stencil values, read as depth and as stencil: its
    // planes 0 and 1.
    let mut desc = texture_desc(TEX2D, [32, 32, 1], 1, DXGI_FORMAT_R24G8_TYPELESS);
    desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    let clear = D3D12_CLEAR_VALUE {
        Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                Depth: 1.0,
                Stencil: 3,
            },
        },
    };
    let depth = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &desc,
        D3D12_RESOURCE_STATE_DEPTH_WRITE,
        Some(&clear),
    )?;
    let dsv = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 1, false)?;
    unsafe {
        gpu.device.CreateDepthStencilView(
            &depth,
            Some(&D3D12_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
                ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            }),
            gpu.cpu(&dsv, 0),
        )
    };
    im.begin(None, Some(&dsv), DXGI_FORMAT_D24_UNORM_S8_UINT, 32, 32);
    unsafe {
        gpu.list.ClearDepthStencilView(
            gpu.cpu(&dsv, 0),
            D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL,
            1.0,
            3,
            None,
        )
    };
    let mut write_stencil = depth_test(D3D11_COMPARISON_ALWAYS, true);
    write_stencil.StencilEnable = true.into();
    write_stencil.FrontFace.StencilPassOp = D3D11_STENCIL_OP_INCR_SAT;
    write_stencil.BackFace.StencilPassOp = D3D11_STENCIL_OP_INCR_SAT;
    im.depth = write_stencil;
    im.draw_colored(&views11::steps())?;
    unsafe {
        gpu.list.ResourceBarrier(&[transition(
            &depth,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )])
    };
    shown.push((
        dxil!("views.ps_depth"),
        depth.clone(),
        Some(srv_2d(DXGI_FORMAT_R24_UNORM_X8_TYPELESS, 0, 1, 0)),
    ));
    shown.push((
        dxil!("views.ps_stencil"),
        depth,
        Some(srv_2d(DXGI_FORMAT_X24_TYPELESS_G8_UINT, 0, 1, 1)),
    ));

    let (target, rtv) = gpu.target(256, 128, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 256, 128);
    im.depth = depth_test(D3D11_COMPARISON_ALWAYS, false);
    im.root = texture_root(gpu, D3D12_TEXTURE_ADDRESS_MODE_CLAMP)?;
    im.vs = dxil!("common.vs_full");
    im.colored = false;
    unsafe { gpu.list.SetDescriptorHeaps(&[Some(heap.clone())]) };
    for (i, (ps, texture, view)) in shown.iter().enumerate() {
        unsafe {
            gpu.device.CreateShaderResourceView(
                texture,
                view.as_ref().map(|v| v as *const _),
                gpu.cpu(&heap, i as u32),
            )
        };
        im.viewport(cell(i as u32, 4, 64.0));
        im.ps = *ps;
        im.tables = vec![(1, gpu.gpu(&heap, i as u32))];
        im.draw(3, [0.0; 4])?;
    }
    let mut outputs = vec![Output::picture("", gpu.read(&target, 0, RT)?)];

    // Render target views of mip 1 of a mip chain and of layer 2 of an array.
    let chain = render_target(gpu, &texture_desc(TEX2D, [64, 64, 1], 3, RGBA))?;
    let mut mip_view = D3D12_RENDER_TARGET_VIEW_DESC {
        Format: RGBA,
        ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    mip_view.Anonymous.Texture2D.MipSlice = 1;
    let layered = render_target(gpu, &texture_desc(TEX2D, [32, 32, 4], 1, RGBA))?;
    let mut layer_view = D3D12_RENDER_TARGET_VIEW_DESC {
        Format: RGBA,
        ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2DARRAY,
        ..Default::default()
    };
    layer_view.Anonymous.Texture2DArray = D3D12_TEX2D_ARRAY_RTV {
        MipSlice: 0,
        FirstArraySlice: 2,
        ArraySize: 1,
        PlaneSlice: 0,
    };
    let mut im = Immediate::new(gpu)?;
    // Subresource 1 is mip 1 of the chain; subresource 2 is layer 2 of the single-mip array.
    for (part, texture, view, subresource) in [
        ("rtv_mip", &chain, mip_view, 1),
        ("rtv_layer", &layered, layer_view, 2),
    ] {
        let rtv = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
        unsafe {
            gpu.device
                .CreateRenderTargetView(texture, Some(&view), gpu.cpu(&rtv, 0))
        };
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 32, 32);
        im.draw_colored(&views11::patch())?;
        outputs.push(Output::picture(part, gpu.read(texture, subresource, RT)?));
    }
    Ok(outputs)
}
