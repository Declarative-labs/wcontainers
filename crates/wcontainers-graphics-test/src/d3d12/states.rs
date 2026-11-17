//! The pipeline-state scenes of Direct3D 11 (blending, depth, stencil, render target formats,
//! samplers) drawn through Direct3D 12: the same cells, each combination of state made into its own
//! pipeline state object; and the helper that draws Direct3D 11's way on a Direct3D 12 list, which
//! the texture, rasterizer, multisampling and multiple-target scenes share.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::mem::transmute_copy;

use anyhow::Result;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

use super::{Gpu, bytecode, element, footprint_location, tex2d_desc, texture_location, transition};
use crate::d3d11::scenes::blend::{self, CONSTANT, no_blend};
use crate::d3d11::scenes::depth::FUNCS;
use crate::d3d11::scenes::formats::{FORMATS, Kind};
use crate::d3d11::scenes::samplers;
use crate::d3d11::scenes::stencil::{op, stencil, triangle};
use crate::d3d11::scenes::{Colored, depth_test, hue, quad};
use crate::d3d11::{cell, checker, solid};
use crate::dxil;
use crate::output::Output;

pub(super) const RT: D3D12_RESOURCE_STATES = D3D12_RESOURCE_STATE_RENDER_TARGET;
const DEPTH: D3D12_RESOURCE_STATES = D3D12_RESOURCE_STATE_DEPTH_WRITE;

/// Direct3D 11's way of drawing on a Direct3D 12 list: state set piece by piece, and a pipeline
/// made (once per combination) at each draw. The root signature starts as four root constants at
/// b0.
pub(super) struct Immediate<'a> {
    gpu: &'a Gpu,
    pub root: ID3D12RootSignature,
    /// Root constants set after the root signature at each draw, as parameter 0, when any.
    pub constants: Vec<f32>,
    pub vs: &'static [u8],
    pub hs: Option<&'static [u8]>,
    pub ds: Option<&'static [u8]>,
    pub gs: Option<&'static [u8]>,
    pub ps: &'static [u8],
    pub topology: D3D_PRIMITIVE_TOPOLOGY,
    pub cut: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE,
    pub colored: bool,
    pub blend: D3D11_BLEND_DESC,
    pub sample_mask: u32,
    pub depth: D3D11_DEPTH_STENCIL_DESC,
    pub raster: D3D11_RASTERIZER_DESC,
    pub targets: Vec<DXGI_FORMAT>,
    pub depth_format: DXGI_FORMAT,
    pub samples: DXGI_SAMPLE_DESC,
    /// Descriptor tables set after the root signature at each draw: parameter and handle.
    pub tables: Vec<(u32, D3D12_GPU_DESCRIPTOR_HANDLE)>,
    pipelines: RefCell<HashMap<String, ID3D12PipelineState>>,
}

impl<'a> Immediate<'a> {
    pub fn new(gpu: &'a Gpu) -> Result<Self> {
        Ok(Immediate {
            gpu,
            root: constants_root(gpu, 4)?,
            constants: Vec::new(),
            vs: dxil!("common.vs_color"),
            hs: None,
            ds: None,
            gs: None,
            ps: dxil!("common.ps_color"),
            topology: D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            cut: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            colored: true,
            blend: no_blend(),
            sample_mask: u32::MAX,
            depth: D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: false.into(),
                ..depth_test(D3D11_COMPARISON_ALWAYS, false)
            },
            raster: solid(),
            targets: vec![DXGI_FORMAT_R8G8B8A8_UNORM],
            depth_format: DXGI_FORMAT_UNKNOWN,
            samples: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            tables: Vec::new(),
            pipelines: RefCell::new(HashMap::new()),
        })
    }

    /// Binds an 8-bit colour target and, if given, a depth target: cleared, with the whole of them
    /// as viewport and scissor.
    pub fn begin(
        &mut self,
        rtv: Option<&ID3D12DescriptorHeap>,
        dsv: Option<&ID3D12DescriptorHeap>,
        format: DXGI_FORMAT,
        width: u32,
        height: u32,
    ) {
        let list = &self.gpu.list;
        let rtv = rtv.map(|heap| self.gpu.cpu(heap, 0));
        let dsv = dsv.map(|heap| self.gpu.cpu(heap, 0));
        unsafe {
            if let Some(rtv) = rtv {
                list.ClearRenderTargetView(rtv, &crate::scene::CLEAR, None);
            }
            if let Some(dsv) = dsv {
                list.ClearDepthStencilView(
                    dsv,
                    D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL,
                    1.0,
                    0,
                    None,
                );
            }
            list.OMSetRenderTargets(
                rtv.is_some() as u32,
                rtv.as_ref().map(|r| r as *const _),
                false,
                dsv.as_ref().map(|d| d as *const _),
            );
            list.RSSetScissorRects(&[RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            }]);
        }
        self.targets = if rtv.is_some() {
            vec![DXGI_FORMAT_R8G8B8A8_UNORM]
        } else {
            Vec::new()
        };
        self.depth_format = format;
        self.viewport(D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        });
    }

    pub fn viewport(&self, v: D3D11_VIEWPORT) {
        unsafe { self.gpu.list.RSSetViewports(&[transmute_copy(&v)]) };
    }

    fn stencil_ref(&self, value: u32) {
        unsafe { self.gpu.list.OMSetStencilRef(value) };
    }

    /// The pipeline of the state set now.
    fn pipeline(&self) -> Result<ID3D12PipelineState> {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 12),
        ];
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: unsafe { transmute_copy(&self.root) },
            VS: bytecode(self.vs),
            HS: self.hs.map(bytecode).unwrap_or_default(),
            DS: self.ds.map(bytecode).unwrap_or_default(),
            GS: self.gs.map(bytecode).unwrap_or_default(),
            PS: bytecode(self.ps),
            BlendState: blend12(&self.blend),
            SampleMask: self.sample_mask,
            RasterizerState: raster12(&self.raster),
            DepthStencilState: depth12(&self.depth),
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: if self.colored { 2 } else { 0 },
            },
            IBStripCutValue: self.cut,
            PrimitiveTopologyType: topology_type(self.topology),
            NumRenderTargets: self.targets.len() as u32,
            DSVFormat: self.depth_format,
            SampleDesc: self.samples,
            ..Default::default()
        };
        desc.RTVFormats[..self.targets.len()].copy_from_slice(&self.targets);
        if self.depth_format == DXGI_FORMAT_UNKNOWN {
            desc.DepthStencilState.DepthEnable = false.into();
            desc.DepthStencilState.StencilEnable = false.into();
        }
        // Debug's text of each block (and the shaders' and root's addresses) names the combination.
        let key = format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?} {:?} {:?} {:?} {:?}",
            self.root.as_raw(),
            self.vs.as_ptr(),
            self.hs.map(<[u8]>::as_ptr),
            self.ds.map(<[u8]>::as_ptr),
            self.gs.map(<[u8]>::as_ptr),
            self.ps.as_ptr(),
            desc.PrimitiveTopologyType,
            self.cut,
            self.colored,
            self.blend,
            self.sample_mask,
            self.depth,
            self.raster,
            self.targets,
            self.depth_format,
            self.samples
        );
        if let Some(pipeline) = self.pipelines.borrow().get(&key) {
            return Ok(pipeline.clone());
        }
        let pipeline: ID3D12PipelineState =
            unsafe { self.gpu.device.CreateGraphicsPipelineState(&desc)? };
        self.pipelines.borrow_mut().insert(key, pipeline.clone());
        Ok(pipeline)
    }

    fn bind(&self) -> Result<()> {
        let list = &self.gpu.list;
        unsafe {
            list.SetPipelineState(&self.pipeline()?);
            list.SetGraphicsRootSignature(&self.root);
            if !self.constants.is_empty() {
                list.SetGraphicsRoot32BitConstants(
                    0,
                    self.constants.len() as u32,
                    self.constants.as_ptr().cast(),
                    0,
                );
            }
            for &(parameter, handle) in &self.tables {
                list.SetGraphicsRootDescriptorTable(parameter, handle);
            }
            list.IASetPrimitiveTopology(self.topology);
        }
        Ok(())
    }

    /// Draws coloured vertices in the topology set.
    pub fn draw_colored(&self, vertices: &[Colored]) -> Result<()> {
        self.bind()?;
        self.vertices(vertices)?;
        unsafe { self.gpu.list.DrawInstanced(vertices.len() as u32, 1, 0, 0) };
        Ok(())
    }

    /// Draws coloured vertices by `indices` (of `format`) in the topology set.
    pub fn draw_indexed(
        &self,
        vertices: &[Colored],
        indices: &[u8],
        format: DXGI_FORMAT,
    ) -> Result<()> {
        self.bind()?;
        self.vertices(vertices)?;
        let buffer = self.gpu.upload(indices)?;
        let size = if format == DXGI_FORMAT_R16_UINT { 2 } else { 4 };
        unsafe {
            self.gpu
                .list
                .IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: buffer.GetGPUVirtualAddress(),
                    SizeInBytes: indices.len() as u32,
                    Format: format,
                }));
            self.gpu
                .list
                .DrawIndexedInstanced(indices.len() as u32 / size, 1, 0, 0, 0);
        }
        self.gpu.keep(&buffer);
        Ok(())
    }

    /// Binds coloured vertices as slot 0.
    fn vertices(&self, vertices: &[Colored]) -> Result<()> {
        let buffer = self.gpu.upload(bytemuck::cast_slice(vertices))?;
        unsafe {
            self.gpu.list.IASetVertexBuffers(
                0,
                Some(&[D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: buffer.GetGPUVirtualAddress(),
                    SizeInBytes: (vertices.len() * 28) as u32,
                    StrideInBytes: 28,
                }]),
            );
        }
        self.gpu.keep(&buffer);
        Ok(())
    }

    /// Draws `count` vertices without a vertex buffer, with the four root constants.
    pub fn draw(&self, count: u32, constants: [f32; 4]) -> Result<()> {
        self.bind()?;
        unsafe {
            self.gpu
                .list
                .SetGraphicsRoot32BitConstants(0, 4, constants.as_ptr().cast(), 0);
            self.gpu.list.DrawInstanced(count, 1, 0, 0);
        }
        Ok(())
    }

    /// A depth target and its DSV heap, in the depth write state.
    fn depth_target(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<(ID3D12Resource, ID3D12DescriptorHeap)> {
        let clear = D3D12_CLEAR_VALUE {
            Format: format,
            Anonymous: D3D12_CLEAR_VALUE_0 {
                DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                    Depth: 1.0,
                    Stencil: 0,
                },
            },
        };
        let texture = self.gpu.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(
                width,
                height,
                format,
                D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
            ),
            DEPTH,
            Some(&clear),
        )?;
        let heap = self
            .gpu
            .descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 1, false)?;
        unsafe {
            self.gpu
                .device
                .CreateDepthStencilView(&texture, None, self.gpu.cpu(&heap, 0))
        };
        Ok((texture, heap))
    }
}

/// A root signature of `count` root constants at b0, seen by every stage.
pub(super) fn constants_root(gpu: &Gpu, count: u32) -> Result<ID3D12RootSignature> {
    let constants = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: 0,
                RegisterSpace: 0,
                Num32BitValues: count,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    };
    gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 1,
                pParameters: &constants,
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                ..Default::default()
            },
        },
    })
}

/// The kind of primitive a topology draws, which a pipeline is made for.
fn topology_type(topology: D3D_PRIMITIVE_TOPOLOGY) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
    match topology {
        D3D_PRIMITIVE_TOPOLOGY_POINTLIST => D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        D3D_PRIMITIVE_TOPOLOGY_LINELIST
        | D3D_PRIMITIVE_TOPOLOGY_LINESTRIP
        | D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ
        | D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        t if t.0 >= D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 => {
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH
        }
        _ => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
    }
}

/// Direct3D 11's blend description as Direct3D 12's: the values of each enumeration are the same.
pub(super) fn blend12(d: &D3D11_BLEND_DESC) -> D3D12_BLEND_DESC {
    let mut out = D3D12_BLEND_DESC {
        AlphaToCoverageEnable: d.AlphaToCoverageEnable,
        IndependentBlendEnable: d.IndependentBlendEnable,
        ..Default::default()
    };
    for (o, t) in out.RenderTarget.iter_mut().zip(&d.RenderTarget) {
        *o = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: t.BlendEnable,
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND(t.SrcBlend.0),
            DestBlend: D3D12_BLEND(t.DestBlend.0),
            BlendOp: D3D12_BLEND_OP(t.BlendOp.0),
            SrcBlendAlpha: D3D12_BLEND(t.SrcBlendAlpha.0),
            DestBlendAlpha: D3D12_BLEND(t.DestBlendAlpha.0),
            BlendOpAlpha: D3D12_BLEND_OP(t.BlendOpAlpha.0),
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: t.RenderTargetWriteMask,
        };
    }
    out
}

fn face12(f: &D3D11_DEPTH_STENCILOP_DESC) -> D3D12_DEPTH_STENCILOP_DESC {
    D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D12_STENCIL_OP(f.StencilFailOp.0),
        StencilDepthFailOp: D3D12_STENCIL_OP(f.StencilDepthFailOp.0),
        StencilPassOp: D3D12_STENCIL_OP(f.StencilPassOp.0),
        StencilFunc: D3D12_COMPARISON_FUNC(f.StencilFunc.0),
    }
}

fn depth12(d: &D3D11_DEPTH_STENCIL_DESC) -> D3D12_DEPTH_STENCIL_DESC {
    D3D12_DEPTH_STENCIL_DESC {
        DepthEnable: d.DepthEnable,
        DepthWriteMask: D3D12_DEPTH_WRITE_MASK(d.DepthWriteMask.0),
        DepthFunc: D3D12_COMPARISON_FUNC(d.DepthFunc.0),
        StencilEnable: d.StencilEnable,
        StencilReadMask: d.StencilReadMask,
        StencilWriteMask: d.StencilWriteMask,
        FrontFace: face12(&d.FrontFace),
        BackFace: face12(&d.BackFace),
    }
}

/// Direct3D 12 has no scissor switch: the scenes drawn here set the scissor to the whole target.
pub(super) fn raster12(r: &D3D11_RASTERIZER_DESC) -> D3D12_RASTERIZER_DESC {
    D3D12_RASTERIZER_DESC {
        FillMode: D3D12_FILL_MODE(r.FillMode.0),
        CullMode: D3D12_CULL_MODE(r.CullMode.0),
        FrontCounterClockwise: r.FrontCounterClockwise,
        DepthBias: r.DepthBias,
        DepthBiasClamp: r.DepthBiasClamp,
        SlopeScaledDepthBias: r.SlopeScaledDepthBias,
        DepthClipEnable: r.DepthClipEnable,
        MultisampleEnable: r.MultisampleEnable,
        AntialiasedLineEnable: r.AntialiasedLineEnable,
        ForcedSampleCount: 0,
        ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
    }
}

/// Direct3D 11's blend scene: each cell's destination gradient, then the dual-source colours with
/// that cell's blend and the blend factor.
pub fn blend(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut im = Immediate::new(gpu)?;
    im.vs = dxil!("blend.vs");
    im.colored = false;
    let mut outputs = Vec::new();
    for (part, columns, size, cells) in blend::parts() {
        // Set again for each list: a list's reset sets the blend factor back to zero.
        unsafe { gpu.list.OMSetBlendFactor(Some(&CONSTANT)) };
        let (target, rtv) = gpu.target(blend::SIZE, blend::SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
        im.begin(
            Some(&rtv),
            None,
            DXGI_FORMAT_UNKNOWN,
            blend::SIZE,
            blend::SIZE,
        );
        for (i, desc) in cells.into_iter().enumerate() {
            let viewport = cell(i as u32, columns, size as f32);
            let rect = [
                viewport.TopLeftX,
                viewport.TopLeftY,
                viewport.Width,
                viewport.Height,
            ];
            im.viewport(viewport);
            im.blend = no_blend();
            im.ps = dxil!("blend.ps_dest");
            im.draw(3, rect)?;
            im.blend = desc;
            im.ps = dxil!("blend.ps_dual");
            im.draw(3, rect)?;
        }
        outputs.push(Output::picture(part, gpu.read(&target, 0, RT)?));
    }
    Ok(outputs)
}

const RED: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
const GREEN: [f32; 4] = [0.2, 0.85, 0.3, 1.0];
const BLUE: [f32; 4] = [0.2, 0.4, 0.95, 1.0];
const YELLOW: [f32; 4] = [0.95, 0.85, 0.2, 1.0];

/// Direct3D 11's depth scene: every comparison function, depth bias on a 24-bit buffer, clipping
/// against clamping and a narrowed depth range, and depth written by the pixel shader.
pub fn depth(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut im = Immediate::new(gpu)?;
    let mut outputs = Vec::new();

    let (target, rtv) = gpu.target(256, 128, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    // A descriptor holds no reference: the texture is kept until the work is done.
    let (depth, dsv) = im.depth_target(256, 128, DXGI_FORMAT_D32_FLOAT)?;
    gpu.keep(&depth);
    im.begin(Some(&rtv), Some(&dsv), DXGI_FORMAT_D32_FLOAT, 256, 128);
    for (i, &func) in FUNCS.iter().enumerate() {
        im.viewport(cell(i as u32, 4, 64.0));
        im.depth = depth_test(D3D11_COMPARISON_ALWAYS, true);
        im.draw_colored(&quad(-0.9, -0.9, 0.9, 0.9, 0.1, 0.9, RED))?;
        im.depth = depth_test(func, false);
        im.draw_colored(&quad(-0.7, -0.5, 0.7, 0.5, 0.5, 0.5, GREEN))?;
    }
    outputs.push(Output::picture("funcs", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(256, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = im.depth_target(256, 64, DXGI_FORMAT_D24_UNORM_S8_UINT)?;
    gpu.keep(&depth);
    im.begin(
        Some(&rtv),
        Some(&dsv),
        DXGI_FORMAT_D24_UNORM_S8_UINT,
        256,
        64,
    );
    let biases = [
        (0, 0.0, 0.0),
        (-3, 0.0, 0.0),
        (0, -1.5, 0.0),
        (-3000, -1.5, -0.00002),
    ];
    for (i, (bias, slope, clamp)) in biases.into_iter().enumerate() {
        im.viewport(cell(i as u32, 4, 64.0));
        im.raster = solid();
        im.depth = depth_test(D3D11_COMPARISON_LESS, true);
        im.draw_colored(&quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, BLUE))?;
        im.raster = D3D11_RASTERIZER_DESC {
            DepthBias: bias,
            SlopeScaledDepthBias: slope,
            DepthBiasClamp: clamp,
            ..solid()
        };
        im.draw_colored(&quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, YELLOW))?;
    }
    outputs.push(Output::picture("bias", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(192, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = im.depth_target(192, 64, DXGI_FORMAT_D32_FLOAT)?;
    im.begin(Some(&rtv), Some(&dsv), DXGI_FORMAT_D32_FLOAT, 192, 64);
    im.depth = depth_test(D3D11_COMPARISON_ALWAYS, true);
    for (i, clip) in [true, false].into_iter().enumerate() {
        im.raster = D3D11_RASTERIZER_DESC {
            DepthClipEnable: clip.into(),
            ..solid()
        };
        im.viewport(cell(i as u32, 3, 64.0));
        im.draw_colored(&quad(-0.9, -0.9, 0.9, 0.9, -0.5, 1.5, GREEN))?;
    }
    im.raster = solid();
    im.viewport(D3D11_VIEWPORT {
        MinDepth: 0.25,
        MaxDepth: 0.75,
        ..cell(2, 3, 64.0)
    });
    im.draw_colored(&quad(-0.9, -0.9, 0.9, 0.9, 0.0, 1.0, GREEN))?;
    outputs.push(Output::picture("clip", gpu.read(&target, 0, RT)?));
    outputs.push(Output::picture("clip_depth", gpu.read(&depth, 0, DEPTH)?));

    let (target, rtv) = gpu.target(192, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = im.depth_target(192, 64, DXGI_FORMAT_D32_FLOAT)?;
    im.begin(Some(&rtv), Some(&dsv), DXGI_FORMAT_D32_FLOAT, 192, 64);
    im.depth = depth_test(D3D11_COMPARISON_LESS_EQUAL, true);
    for (i, ps) in [
        dxil!("depth.ps_depth"),
        dxil!("depth.ps_greater"),
        dxil!("depth.ps_less"),
    ]
    .into_iter()
    .enumerate()
    {
        im.ps = ps;
        im.viewport(cell(i as u32, 3, 64.0));
        im.draw_colored(&quad(-0.8, -0.8, 0.8, 0.8, 0.3, 0.6, YELLOW))?;
    }
    outputs.push(Output::picture("shader", gpu.read(&target, 0, RT)?));
    outputs.push(Output::picture("shader_depth", gpu.read(&depth, 0, DEPTH)?));
    Ok(outputs)
}

/// Direct3D 11's stencil scene: separate front and back operations, replace, saturating and
/// wrapping steps, invert, a write mask, the depth-fail operation and a read mask, shown as colours
/// by equality tests, and the stencil plane read back.
pub fn stencil_scene(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = 192;
    let mut im = Immediate::new(gpu)?;
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = im.depth_target(SIZE, SIZE, DXGI_FORMAT_D24_UNORM_S8_UINT)?;
    im.begin(
        Some(&rtv),
        Some(&dsv),
        DXGI_FORMAT_D24_UNORM_S8_UINT,
        SIZE,
        SIZE,
    );
    let mut masked = no_blend();
    masked.RenderTarget[0].RenderTargetWriteMask = 0;
    im.blend = masked;
    let set = |im: &mut Immediate, desc: D3D11_DEPTH_STENCIL_DESC, reference: u32| {
        im.depth = desc;
        im.stencil_ref(reference);
    };

    set(
        &mut im,
        stencil(
            op(D3D11_STENCIL_OP_INCR, D3D11_COMPARISON_ALWAYS),
            op(D3D11_STENCIL_OP_DECR, D3D11_COMPARISON_ALWAYS),
            0xff,
            0xff,
        ),
        0,
    );
    let mut fan = Vec::new();
    for k in 0..6 {
        let angle = k as f32 * std::f32::consts::TAU / 6.0;
        fan.extend(triangle(
            0.25 * angle.cos(),
            0.25 * angle.sin(),
            0.45,
            k % 2 == 0,
        ));
    }
    im.draw_colored(&fan)?;

    let always = |pass| op(pass, D3D11_COMPARISON_ALWAYS);
    let keep = always(D3D11_STENCIL_OP_KEEP);
    let rect = |x: f32, y: f32| quad(x, y, x + 0.5, y + 0.5, 0.5, 0.5, [1.0; 4]);
    set(
        &mut im,
        stencil(always(D3D11_STENCIL_OP_REPLACE), keep, 0xff, 0xff),
        0x80,
    );
    im.draw_colored(&rect(-0.95, 0.45))?;
    set(
        &mut im,
        stencil(always(D3D11_STENCIL_OP_INCR_SAT), keep, 0xff, 0xff),
        0,
    );
    for _ in 0..3 {
        im.draw_colored(&rect(0.45, 0.45))?;
    }
    set(
        &mut im,
        stencil(always(D3D11_STENCIL_OP_INVERT), keep, 0xff, 0xff),
        0,
    );
    im.draw_colored(&rect(-0.95, -0.95))?;
    set(
        &mut im,
        stencil(always(D3D11_STENCIL_OP_DECR_SAT), keep, 0xff, 0xff),
        0,
    );
    im.draw_colored(&rect(0.45, -0.95))?;
    set(
        &mut im,
        stencil(always(D3D11_STENCIL_OP_REPLACE), keep, 0xff, 0x0f),
        0xf7,
    );
    im.draw_colored(&quad(-0.2, -0.95, 0.2, 0.95, 0.5, 0.5, [1.0; 4]))?;
    set(&mut im, depth_test(D3D11_COMPARISON_LESS, true), 0);
    im.draw_colored(&quad(-0.9, -0.15, -0.3, 0.15, 0.2, 0.2, [1.0; 4]))?;
    let mut depth_fail = stencil(
        always(D3D11_STENCIL_OP_INCR),
        always(D3D11_STENCIL_OP_INCR),
        0xff,
        0xff,
    );
    depth_fail.DepthFunc = D3D11_COMPARISON_LESS;
    depth_fail.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_REPLACE;
    set(&mut im, depth_fail, 0x40);
    im.draw_colored(&quad(-0.95, -0.25, -0.05, 0.25, 0.8, 0.8, [1.0; 4]))?;

    im.blend = no_blend();
    let values: [u32; 12] = [
        1, 2, 3, 0xff, 0xfe, 0xfd, 0x80, 0x7f, 0x40, 0x41, 0x07, 0xf8,
    ];
    let equal = op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL);
    for (i, &value) in values.iter().enumerate() {
        let [r, g, b] = hue(i as u32, values.len() as u32);
        set(&mut im, stencil(equal, equal, 0xff, 0), value);
        im.draw_colored(&quad(-1.0, -1.0, 1.0, 1.0, 0.5, 0.5, [r, g, b, 1.0]))?;
    }
    set(&mut im, stencil(equal, equal, 0x0f, 0), 0x07);
    im.draw_colored(&quad(-1.0, -1.0, 1.0, -0.6, 0.5, 0.5, [0.5, 0.5, 0.5, 1.0]))?;
    Ok(vec![
        Output::picture("", gpu.read(&target, 0, RT)?),
        // Subresource 1 is the stencil plane.
        Output::picture("values", gpu.read(&depth, 1, DEPTH)?),
    ])
}

/// Direct3D 11's formats scene: a gradient drawn into each render target format (floats past 0 and
/// 1, integers into integer formats), what the device says each supports, and blending in sRGB.
pub fn formats(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = 64;
    let mut im = Immediate::new(gpu)?;
    im.vs = dxil!("common.vs_full");
    im.colored = false;
    let mut outputs = Vec::new();
    let mut support = String::new();
    for (name, format, kind) in FORMATS {
        let mut query = D3D12_FEATURE_DATA_FORMAT_SUPPORT {
            Format: format,
            ..Default::default()
        };
        unsafe {
            gpu.device.CheckFeatureSupport(
                D3D12_FEATURE_FORMAT_SUPPORT,
                (&mut query as *mut D3D12_FEATURE_DATA_FORMAT_SUPPORT).cast(),
                size_of_val(&query) as u32,
            )?
        };
        let has = |flag: D3D12_FORMAT_SUPPORT1| u8::from(query.Support1.0 & flag.0 != 0);
        writeln!(
            support,
            "{name} render target {} blendable {} multisample render target {} shader sample {}",
            has(D3D12_FORMAT_SUPPORT1_RENDER_TARGET),
            has(D3D12_FORMAT_SUPPORT1_BLENDABLE),
            has(D3D12_FORMAT_SUPPORT1_MULTISAMPLE_RENDERTARGET),
            has(D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE),
        )?;
        let (texture, rtv) = gpu.target(SIZE, SIZE, format)?;
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
        im.targets = vec![format];
        im.ps = match kind {
            Kind::Float => dxil!("formats.ps_float"),
            Kind::Uint => dxil!("formats.ps_uint"),
            Kind::Sint => dxil!("formats.ps_sint"),
        };
        im.draw(3, [0.0; 4])?;
        outputs.push(Output::picture(name, gpu.read(&texture, 0, RT)?));
    }
    outputs.push(Output::text("support", support));

    let srgb = DXGI_FORMAT_R8G8B8A8_UNORM_SRGB;
    let (texture, rtv) = gpu.target(SIZE, SIZE, srgb)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
    im.targets = vec![srgb];
    im.ps = dxil!("formats.ps_float");
    im.draw(3, [0.0; 4])?;
    im.blend.RenderTarget[0] = blend::target(
        D3D11_BLEND_SRC_ALPHA,
        D3D11_BLEND_INV_SRC_ALPHA,
        D3D11_BLEND_OP_ADD,
        D3D11_BLEND_ONE,
        D3D11_BLEND_ZERO,
        D3D11_BLEND_OP_ADD,
    );
    im.vs = dxil!("common.vs_color");
    im.ps = dxil!("common.ps_color");
    im.colored = true;
    im.draw_colored(&quad(-0.8, -0.8, 0.8, 0.8, 0.5, 0.5, [0.2, 0.9, 0.5, 0.5]))?;
    outputs.push(Output::picture("srgb_blend", gpu.read(&texture, 0, RT)?));
    Ok(outputs)
}

fn sampler12(d: &D3D11_SAMPLER_DESC) -> D3D12_SAMPLER_DESC {
    D3D12_SAMPLER_DESC {
        Filter: D3D12_FILTER(d.Filter.0),
        AddressU: D3D12_TEXTURE_ADDRESS_MODE(d.AddressU.0),
        AddressV: D3D12_TEXTURE_ADDRESS_MODE(d.AddressV.0),
        AddressW: D3D12_TEXTURE_ADDRESS_MODE(d.AddressW.0),
        MipLODBias: d.MipLODBias,
        MaxAnisotropy: d.MaxAnisotropy,
        ComparisonFunc: D3D12_COMPARISON_FUNC(d.ComparisonFunc.0),
        BorderColor: d.BorderColor,
        MinLOD: d.MinLOD,
        MaxLOD: d.MaxLOD,
    }
}

/// Records copies of the first subresources of a texture now in the copy destination state, one
/// for each of `levels`, whose texels (or blocks) are packed in rows and slices.
pub(super) fn fill_levels(gpu: &Gpu, texture: &ID3D12Resource, levels: &[Vec<u8>]) -> Result<()> {
    let desc = unsafe { texture.GetDesc() };
    let n = levels.len();
    let mut footprints = vec![D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); n];
    let mut rows = vec![0u32; n];
    let mut row_sizes = vec![0u64; n];
    let mut total = 0;
    unsafe {
        gpu.device.GetCopyableFootprints(
            &desc,
            0,
            n as u32,
            0,
            Some(footprints.as_mut_ptr()),
            Some(rows.as_mut_ptr()),
            Some(row_sizes.as_mut_ptr()),
            Some(&mut total),
        )
    };
    let mut staged = vec![0u8; total as usize];
    for (i, bytes) in levels.iter().enumerate() {
        let (f, size, rows) = (footprints[i], row_sizes[i] as usize, rows[i] as usize);
        // Rows of every slice in turn: a slice's rows follow the last one's in both.
        for row in 0..rows * f.Footprint.Depth as usize {
            let at = f.Offset as usize + row * f.Footprint.RowPitch as usize;
            staged[at..at + size].copy_from_slice(&bytes[row * size..(row + 1) * size]);
        }
    }
    let upload = gpu.upload(&staged)?;
    for (i, &footprint) in footprints.iter().enumerate() {
        unsafe {
            gpu.list.CopyTextureRegion(
                &texture_location(texture, i as u32),
                0,
                0,
                0,
                &footprint_location(&upload, footprint),
                None,
            )
        };
    }
    gpu.keep(&upload);
    Ok(())
}

/// A texture of RGBA8 levels, filled, in the pixel shader resource state, with its view at
/// `index` of `heap`.
fn sampled_texture(
    gpu: &Gpu,
    size: u32,
    levels: &[Vec<u8>],
    heap: &ID3D12DescriptorHeap,
    index: u32,
) -> Result<ID3D12Resource> {
    let mut desc = tex2d_desc(
        size,
        size,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D12_RESOURCE_FLAG_NONE,
    );
    desc.MipLevels = levels.len() as u16;
    let texture = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &desc,
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    fill_levels(gpu, &texture, levels)?;
    unsafe {
        gpu.list.ResourceBarrier(&[transition(
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        gpu.device
            .CreateShaderResourceView(&texture, None, gpu.cpu(heap, index));
    }
    Ok(texture)
}

/// Direct3D 11's samplers scene: one cell per sampler (from the same tables), every filter,
/// address mode with a border colour, gathering, and comparison sampling of a depth texture.
pub fn samplers_scene(gpu: &Gpu) -> Result<Vec<Output>> {
    let views = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 3, true)?;
    let ranges = [
        D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 1,
            ..Default::default()
        },
        D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
            NumDescriptors: 2,
            ..Default::default()
        },
    ];
    let table = |range: &D3D12_DESCRIPTOR_RANGE| D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                NumDescriptorRanges: 1,
                pDescriptorRanges: range,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
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
        table(&ranges[0]),
        table(&ranges[1]),
    ];
    let root = gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: parameters.len() as u32,
                pParameters: parameters.as_ptr(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                ..Default::default()
            },
        },
    })?;
    let mut im = Immediate::new(gpu)?;

    // The same textures: six levels each a checker of its own colour, an 8 by 8 checker, and a
    // depth texture holding a slope.
    let levels: Vec<Vec<u8>> = (0..6).map(|m| checker(32 >> m, 32 >> m, m, 6)).collect();
    let mips = sampled_texture(gpu, 32, &levels, &views, 0)?;
    let small = sampled_texture(gpu, 8, &[checker(8, 8, 1, 3)], &views, 1)?;
    let clear = D3D12_CLEAR_VALUE {
        Format: DXGI_FORMAT_D32_FLOAT,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                Depth: 1.0,
                Stencil: 0,
            },
        },
    };
    let depth = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(
            32,
            32,
            DXGI_FORMAT_R32_TYPELESS,
            D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
        ),
        DEPTH,
        Some(&clear),
    )?;
    let dsv = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 1, false)?;
    unsafe {
        gpu.device.CreateDepthStencilView(
            &depth,
            Some(&D3D12_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            }),
            gpu.cpu(&dsv, 0),
        )
    };
    im.begin(None, Some(&dsv), DXGI_FORMAT_D32_FLOAT, 32, 32);
    im.depth = depth_test(D3D11_COMPARISON_ALWAYS, true);
    im.draw_colored(&quad(-0.8, -1.0, 1.0, 0.7, 0.05, 0.95, [1.0; 4]))?;
    let mut view = D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R32_FLOAT,
        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        ..Default::default()
    };
    view.Anonymous.Texture2D.MipLevels = 1;
    unsafe {
        gpu.list.ResourceBarrier(&[transition(
            &depth,
            DEPTH,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        gpu.device
            .CreateShaderResourceView(&depth, Some(&view), gpu.cpu(&views, 2));
    }
    gpu.submit()?;
    let _textures = (mips, small, depth);

    im.root = root;
    im.depth = depth_test(D3D11_COMPARISON_ALWAYS, false);
    im.vs = dxil!("common.vs_full");
    im.colored = false;
    let mut outputs = Vec::new();
    for (part, ps, texture, columns, size, table) in [
        (
            "filters",
            dxil!("samplers.ps_sample"),
            0,
            4,
            64,
            samplers::filters(),
        ),
        (
            "address",
            dxil!("samplers.ps_sample"),
            1,
            5,
            48,
            samplers::address(),
        ),
        (
            "gather",
            dxil!("samplers.ps_gather"),
            1,
            5,
            48,
            samplers::address(),
        ),
        (
            "compare",
            dxil!("samplers.ps_compare"),
            2,
            4,
            48,
            samplers::compare(),
        ),
        (
            "compare_zero",
            dxil!("samplers.ps_compare_zero"),
            2,
            4,
            48,
            samplers::compare(),
        ),
    ] {
        // Each cell's sampler in both of its table's slots (s0 and s1).
        let heap = gpu.descriptor_heap(
            D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            2 * table.len() as u32,
            true,
        )?;
        for (i, desc) in table.iter().enumerate() {
            for k in 0..2 {
                unsafe {
                    gpu.device
                        .CreateSampler(&sampler12(desc), gpu.cpu(&heap, 2 * i as u32 + k))
                };
            }
        }
        let rows = (table.len() as u32).div_ceil(columns);
        let (target, rtv) = gpu.target(columns * size, rows * size, DXGI_FORMAT_R8G8B8A8_UNORM)?;
        im.begin(
            Some(&rtv),
            None,
            DXGI_FORMAT_UNKNOWN,
            columns * size,
            rows * size,
        );
        im.ps = ps;
        unsafe {
            gpu.list
                .SetDescriptorHeaps(&[Some(views.clone()), Some(heap.clone())])
        };
        for i in 0..table.len() as u32 {
            let viewport = cell(i, columns, size as f32);
            im.viewport(viewport);
            im.tables = vec![(1, gpu.gpu(&views, texture)), (2, gpu.gpu(&heap, 2 * i))];
            im.draw(
                3,
                [
                    viewport.TopLeftX,
                    viewport.TopLeftY,
                    viewport.Width,
                    viewport.Height,
                ],
            )?;
        }
        outputs.push(Output::picture(part, gpu.read(&target, 0, RT)?));
    }
    Ok(outputs)
}
