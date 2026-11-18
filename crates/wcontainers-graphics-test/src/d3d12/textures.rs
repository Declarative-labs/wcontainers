//! Direct3D 11's texture-kind scene drawn through Direct3D 12: the same textures (1D with mips, 1D
//! and 2D arrays, 3D, cubes, cube arrays, a 2D mip chain and every block-compressed format) as
//! committed resources filled from an upload heap, each read through a descriptor table with a
//! static sampler.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::states::{Immediate, RT, fill_levels};
use super::{Gpu, transition};
use crate::d3d11::cell;
use crate::d3d11::scenes::textures::{BC, BC_PARTS, SIZE, bc_blocks, bc67, subresources};
use crate::dxil;
use crate::output::Output;

/// A texture of `desc` holding `levels` (its first subresources), in the pixel shader resource
/// state.
pub(super) fn filled_texture(
    gpu: &Gpu,
    desc: &D3D12_RESOURCE_DESC,
    levels: &[Vec<u8>],
) -> Result<ID3D12Resource> {
    let texture = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        desc,
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    fill_levels(gpu, &texture, levels)?;
    unsafe {
        gpu.list.ResourceBarrier(&[transition(
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )])
    };
    Ok(texture)
}

/// Four root constants at b0, a table of one shader resource view at t0, and a static linear
/// sampler at s0 with `address` on every axis.
pub(super) fn texture_root(
    gpu: &Gpu,
    address: D3D12_TEXTURE_ADDRESS_MODE,
) -> Result<ID3D12RootSignature> {
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        ..Default::default()
    };
    let parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    Num32BitValues: 4,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];
    let sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: address,
        AddressV: address,
        AddressW: address,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        MaxLOD: f32::MAX,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        ..Default::default()
    };
    gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: parameters.len() as u32,
                pParameters: parameters.as_ptr(),
                NumStaticSamplers: 1,
                pStaticSamplers: &sampler,
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            },
        },
    })
}

pub(super) fn texture_desc(
    dimension: D3D12_RESOURCE_DIMENSION,
    size: [u32; 3],
    mips: u32,
    format: DXGI_FORMAT,
) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: dimension,
        Width: size[0] as u64,
        Height: size[1],
        DepthOrArraySize: size[2] as u16,
        MipLevels: mips as u16,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    }
}

/// A view of a whole cube or cube array; other kinds take the view the resource implies.
fn cube_view(cubes: u32) -> D3D12_SHADER_RESOURCE_VIEW_DESC {
    let mut view = D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        ..Default::default()
    };
    if cubes == 1 {
        view.ViewDimension = D3D12_SRV_DIMENSION_TEXTURECUBE;
        view.Anonymous.TextureCube.MipLevels = 1;
    } else {
        view.ViewDimension = D3D12_SRV_DIMENSION_TEXTURECUBEARRAY;
        view.Anonymous.TextureCubeArray.MipLevels = 1;
        view.Anonymous.TextureCubeArray.NumCubes = cubes;
    }
    view
}

pub fn textures(gpu: &Gpu) -> Result<Vec<Output>> {
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;
    let tex1d = D3D12_RESOURCE_DIMENSION_TEXTURE1D;
    let tex2d = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    // Part, resource, its subresources' data (width, height, depth, mips, layers), view, shader.
    let kinds = [
        (
            "1d",
            texture_desc(tex1d, [64, 1, 1], 7, rgba),
            subresources(64, 1, 1, 7, 1).0,
            None,
            dxil!("textures.ps_1d"),
        ),
        (
            "1d_array",
            texture_desc(tex1d, [64, 1, 4], 1, rgba),
            subresources(64, 1, 1, 1, 4).0,
            None,
            dxil!("textures.ps_1d_array"),
        ),
        (
            "2d_array",
            texture_desc(tex2d, [16, 16, 4], 1, rgba),
            subresources(16, 16, 1, 1, 4).0,
            None,
            dxil!("textures.ps_2d_array"),
        ),
        (
            "cube",
            texture_desc(tex2d, [16, 16, 6], 1, rgba),
            subresources(16, 16, 1, 1, 6).0,
            Some(cube_view(1)),
            dxil!("textures.ps_cube"),
        ),
        (
            "cube_array",
            texture_desc(tex2d, [16, 16, 12], 1, rgba),
            subresources(16, 16, 1, 1, 12).0,
            Some(cube_view(2)),
            dxil!("textures.ps_cube_array"),
        ),
        (
            "3d",
            texture_desc(D3D12_RESOURCE_DIMENSION_TEXTURE3D, [16, 16, 8], 1, rgba),
            subresources(16, 16, 8, 1, 1).0,
            None,
            dxil!("textures.ps_3d"),
        ),
        (
            "mips",
            texture_desc(tex2d, [64, 64, 1], 7, rgba),
            subresources(64, 64, 1, 7, 1).0,
            None,
            dxil!("textures.ps_mips"),
        ),
    ];
    let bc: Vec<_> = BC
        .iter()
        .zip(bc_blocks())
        .map(|(&(format, _), blocks)| (texture_desc(tex2d, [32, 32, 1], 1, format), vec![blocks]))
        .collect();

    // Every texture filled and given its view in one heap, before any drawing.
    let views = gpu.descriptor_heap(
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        (kinds.len() + bc.len()) as u32,
        true,
    )?;
    let mut textures = Vec::new();
    let sources = kinds
        .iter()
        .map(|(_, desc, data, view, _)| (desc, data, view.as_ref()))
        .chain(bc.iter().map(|(desc, data)| (desc, data, None)));
    for (i, (desc, data, view)) in sources.enumerate() {
        let texture = filled_texture(gpu, desc, data)?;
        unsafe {
            gpu.device.CreateShaderResourceView(
                &texture,
                view.map(|v| v as *const _),
                gpu.cpu(&views, i as u32),
            );
        }
        textures.push(texture);
    }

    let mut im = Immediate::new(gpu)?;
    im.root = texture_root(gpu, D3D12_TEXTURE_ADDRESS_MODE_WRAP)?;
    im.vs = dxil!("common.vs_full");
    im.colored = false;
    let mut outputs = Vec::new();
    for (i, (part, _, _, _, ps)) in kinds.iter().enumerate() {
        let (width, height) = match *part {
            "2d_array" | "cube" | "cube_array" => (SIZE * 2, SIZE),
            "mips" => (SIZE, SIZE * 2),
            _ => (SIZE, SIZE),
        };
        let (target, rtv) = gpu.target(width, height, rgba)?;
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, width, height);
        im.ps = *ps;
        im.tables = vec![(1, gpu.gpu(&views, i as u32))];
        unsafe { gpu.list.SetDescriptorHeaps(&[Some(views.clone())]) };
        im.draw(3, [0.0; 4])?;
        outputs.push(Output::picture(part, gpu.read(&target, 0, RT)?));
    }

    // Block-compressed: each format's texels read exactly into a cell of a float target, in
    // Direct3D 11's parts.
    let float = DXGI_FORMAT_R32G32B32A32_FLOAT;
    for (part, newer) in BC_PARTS {
        let (target, rtv) = gpu.target(128, 128, float)?;
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 128, 128);
        im.targets = vec![float];
        im.ps = dxil!("textures.ps_load");
        unsafe { gpu.list.SetDescriptorHeaps(&[Some(views.clone())]) };
        let formats = (0..bc.len()).filter(|&i| bc67(BC[i].0) == newer);
        for (k, i) in formats.enumerate() {
            im.viewport(cell(k as u32, 4, 32.0));
            im.tables = vec![(1, gpu.gpu(&views, (kinds.len() + i) as u32))];
            im.draw(3, [0.0; 4])?;
        }
        outputs.push(Output::picture(part, gpu.read(&target, 0, RT)?));
    }
    Ok(outputs)
}
