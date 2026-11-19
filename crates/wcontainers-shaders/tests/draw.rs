//! The crate's own shaders (tests/own) drawn by real Direct3D 11 (fxc's builds) and 12 (dxc's builds)
//! and, converted by `convert`, through the WebGPU back end (wgpu on Direct3D 12) and the WebGL 2 back
//! end (on ANGLE) on the same GPU. Each picture must equal Direct3D's of the same bytecode texel for
//! texel, which checks that the converted programs mean what the bytecode means.
//!
//! Windows only, and needs a GPU: run as a GPU job (docs/status-graphics-shaders.md). The WebGL 2 back
//! end loads ANGLE from `WCONTAINERS_ANGLE` (default C:\wc\tools\angle-mozangle-0.7.1, which the WebGL
//! track's angle\build.cmd makes).

#![cfg(windows)]

mod common;

use common::*;
use std::path::Path;
use wcontainers_gpu::{Backend, Features, Format, ProvokingVertex, VariantKey, VertexFormat};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

/// One picture: a vertex and a pixel shader of tests/own, drawn into a target of `format`.
struct Case {
    vs: &'static str,
    ps: &'static str,
    format: Format,
    /// The constant buffer's tint (`ps_loop` takes its trip count from `w`, `ps_ftoi` its special
    /// values).
    tint: [f32; 4],
    /// Drawn from the vertex grid; otherwise three vertices without a buffer.
    grid: bool,
    /// Why the WebGL 2 back end cannot draw the fxc build and the dxc build, where it cannot.
    not_on_webgl: [Option<&'static str>; 2],
    /// A known failure of the WebGL 2 back end, outside this crate: drawn and reported, not failed on.
    known_on_webgl: Option<&'static str>,
    /// Also compiled by dxc (`<name>.dxil`): drawn by the DXIL route too, against real Direct3D 12's
    /// picture of that build.
    dxil: bool,
    /// The DXIL build needs a shader model this desktop's Direct3D 12 refuses: held to Direct3D 11's
    /// picture of the fxc build instead.
    dxil_against_11: bool,
}

const CASES: [Case; 28] = [
    Case {
        vs: "vs_transform",
        ps: "ps_twospaces",
        format: Format::Rgba8Unorm,
        tint: [1.0, 0.75, 0.5, 1.0],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_textured",
        format: Format::Rgba8Unorm,
        tint: [1.0, 0.75, 0.5, 1.5],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_loop",
        format: Format::Rgba8Unorm,
        tint: [1.0, 1.0, 1.0, 5.0],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_integer",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [Some("GLSL ES 3.00 has no bitfield operations"), None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_fullscreen",
        ps: "ps_window",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: false,
        not_on_webgl: [
            Some("GLSL ES 3.00 has no bitfield operations (vs_fullscreen inserts bits)"),
            None,
        ],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_shade",
        ps: "ps_flat",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_shade",
        ps: "ps_shade",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [Some("GLSL ES 3.00 has no noperspective"), None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_depth",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_local",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_size",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: false,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_signed",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_offset",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_special",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_cases",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [
            Some("GLSL ES 3.00 has no bitfield operations (fxc extracts bits)"),
            None,
        ],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_rows",
        format: Format::Rgba8Unorm,
        tint: [1.0, 0.75, 0.5, 1.5],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_clip",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_carry",
        ps: "ps_textured",
        format: Format::Rgba8Unorm,
        tint: [1.0, 0.75, 0.5, 1.5],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_ftoi",
        format: Format::Rgba32Uint,
        tint: [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2_147_483_648.0],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_table",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_deriv",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [Some("GLSL ES 3.00 has no fine derivatives"); 2],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_coarse",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_negate",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_gatheroff",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [
            Some("GLSL ES 3.00 has no textureGather"),
            Some("GLSL ES 3.00 has no textureGather"),
        ],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_helper",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [
            Some("GLSL ES 3.00 has no fine derivatives"),
            Some("GLSL ES 3.00 has no fine derivatives"),
        ],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: true,
    },
    Case {
        vs: "vs_pack",
        ps: "ps_pack",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_rows_io",
        ps: "ps_rows_io",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_narrow",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [
            Some("GLSL ES 3.00 has no bitfield operations (fxc inserts bits)"),
            None,
        ],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_component",
        format: Format::Rgba32Uint,
        tint: [0.25, 0.5, 0.75, 1.5],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    },
];

/// Draws a case with real Direct3D 11; returns the target's texels, rows tightly packed.
fn direct3d(gpu: &Gpu, case: &Case) -> Vec<u8> {
    let plain = Reference::default();
    direct3d_11(
        gpu,
        case.vs,
        case.ps,
        case.format,
        case.tint,
        case.grid,
        &plain,
    )
    .0
}

/// Draws a case's DXIL build with real Direct3D 12, bound as Direct3D 11 binds the DXBC build: the
/// constants at b0 for both stages, the texture at t0 (in register spaces 0 and 1) and a linear
/// clamping sampler at s0. Returns the target's texels, rows tightly packed.
fn direct3d_12(gpu: &wcontainers_graphics_test::d3d12::Gpu, case: &Case) -> Vec<u8> {
    use wcontainers_graphics_test::d3d12::{
        area, buffer_desc, bytecode as code, element, footprint_location, tex2d_desc,
        texture_location, transition,
    };
    use windows::Win32::Graphics::Direct3D12::*;
    let (vs, ps) = (
        bytecode(&format!("{}.dxil", case.vs)),
        bytecode(&format!("{}.dxil", case.ps)),
    );
    let table = [0, 1].map(|space| D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        RegisterSpace: space,
        OffsetInDescriptorsFromTableStart: space,
        ..Default::default()
    });
    let parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table.len() as u32,
                    pDescriptorRanges: table.as_ptr(),
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];
    let sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        MaxLOD: f32::MAX,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        ..Default::default()
    };
    let root = gpu
        .root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
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
        .unwrap();
    let layout = [
        element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
        element(s!("TEXCOORD"), DXGI_FORMAT_R32G32_FLOAT, 12),
    ];
    let format = dxgi(case.format);
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
        VS: code(&vs),
        PS: code(&ps),
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            DepthClipEnable: true.into(),
            ..Default::default()
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: layout.as_ptr(),
            NumElements: if case.grid { layout.len() as u32 } else { 0 },
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };
    desc.RTVFormats[0] = format;
    desc.BlendState.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_ZERO,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_ZERO,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: 0xf,
        ..Default::default()
    };
    let pipeline: ID3D12PipelineState =
        unsafe { gpu.device.CreateGraphicsPipelineState(&desc) }.unwrap();
    drop(std::mem::ManuallyDrop::into_inner(desc.pRootSignature));

    let constants = gpu.upload(&constants(case.tint)).unwrap();
    let vertices = grid();
    let vertex_buffer = gpu.upload(bytes_of(&vertices)).unwrap();
    let texture = gpu
        .committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(
                TEXTURE_SIZE,
                TEXTURE_SIZE,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D12_RESOURCE_FLAG_NONE,
            ),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )
        .unwrap();
    gpu.fill_texture(&texture, &texels(), TEXTURE_SIZE * 4)
        .unwrap();
    let views = gpu
        .descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 2, true)
        .unwrap();
    for i in 0..2 {
        unsafe {
            gpu.device
                .CreateShaderResourceView(&texture, None, gpu.cpu(&views, i))
        };
    }
    let target = gpu
        .committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(SIZE, SIZE, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET),
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            None,
        )
        .unwrap();
    let rtvs = gpu
        .descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)
        .unwrap();
    let rtv = gpu.cpu(&rtvs, 0);
    unsafe { gpu.device.CreateRenderTargetView(&target, None, rtv) };
    let clear = if case.format == Format::Rgba32Uint {
        [0.0; 4]
    } else {
        CLEAR
    };
    let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let mut total = 0;
    let target_desc = unsafe { target.GetDesc() };
    unsafe {
        gpu.device.GetCopyableFootprints(
            &target_desc,
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut total),
        )
    };
    let readback = gpu
        .committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(total),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )
        .unwrap();
    let l = &gpu.list;
    unsafe {
        l.ResourceBarrier(&[transition(
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        l.SetPipelineState(&pipeline);
        l.SetGraphicsRootSignature(&root);
        l.SetDescriptorHeaps(&[Some(views.clone())]);
        l.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
        l.SetGraphicsRootDescriptorTable(1, gpu.gpu(&views, 0));
        l.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        if case.grid {
            l.IASetVertexBuffers(
                0,
                Some(&[D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: vertex_buffer.GetGPUVirtualAddress(),
                    SizeInBytes: std::mem::size_of_val(vertices.as_slice()) as u32,
                    StrideInBytes: 20,
                }]),
            );
        }
        l.ClearRenderTargetView(rtv, &clear, None);
        l.OMSetRenderTargets(1, Some(&rtv), false, None);
        area(l, 0, 0, SIZE, SIZE);
        l.DrawInstanced(if case.grid { 96 } else { 3 }, 1, 0, 0);
        l.ResourceBarrier(&[transition(
            &target,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        l.CopyTextureRegion(
            &footprint_location(&readback, footprint),
            0,
            0,
            0,
            &texture_location(&target, 0),
            None,
        );
    }
    gpu.submit().unwrap();
    let row = (SIZE * case.format.block().2) as usize;
    let pitch = footprint.Footprint.RowPitch as usize;
    let mut out = Vec::with_capacity(row * SIZE as usize);
    unsafe {
        let mut mapped = std::ptr::null_mut();
        readback
            .Map(
                0,
                Some(&D3D12_RANGE {
                    Begin: 0,
                    End: total as usize,
                }),
                Some(&mut mapped),
            )
            .unwrap();
        let bytes = std::slice::from_raw_parts(mapped.cast::<u8>(), total as usize);
        for y in 0..SIZE as usize {
            out.extend_from_slice(&bytes[y * pitch..y * pitch + row]);
        }
        readback.Unmap(0, Some(&D3D12_RANGE::default()));
    }
    out
}

impl<'a> Draw<'a> {
    /// One of the cases of tests/own; `code` holds its vertex and pixel shader's DXBC, or with `dxil`
    /// their DXIL.
    fn own(case: &Case, code: &'a [Vec<u8>; 2], dxil: bool, grid: &'a [f32]) -> Draw<'a> {
        const ATTRIBUTES: &[(&str, u32, VertexFormat)] = &[
            ("POSITION", 0, VertexFormat::Float32x3),
            ("TEXCOORD", 12, VertexFormat::Float32x2),
        ];
        Draw {
            vs: if dxil {
                Source::Dxil(&code[0])
            } else {
                Source::Dxbc(&code[0])
            },
            ps: if dxil {
                Source::Dxil(&code[1])
            } else {
                Source::Dxbc(&code[1])
            },
            format: case.format,
            size: SIZE,
            vertices: if case.grid { grid } else { &[] },
            stride: 20,
            attributes: ATTRIBUTES,
            count: if case.grid { 96 } else { 3 },
            constants: constants(case.tint),
            setup: Setup::plain(SIZE),
            clear: if case.format == Format::Rgba32Uint {
                [0; 4]
            } else {
                CLEAR.map(f32::to_bits)
            },
        }
    }
}

#[test]
fn own_shaders_draw_as_direct3d_does() {
    let mut wgpu_backend = webgpu();
    let mut gl = webgl();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    eprintln!("direct3d 11: {}", gpu.adapter().unwrap());
    let gpu12 = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let mut failures = Vec::new();
    let grid = grid();
    for case in &CASES {
        let name = format!("{}+{}", case.vs, case.ps);
        let reference = direct3d(&gpu, case);
        keep(&format!("{name}.d3d11"), &reference, case.format);
        // Each DXIL build is held to Direct3D 12 drawing that same DXIL.
        let reference_12 = (case.dxil && !case.dxil_against_11).then(|| {
            let texels = direct3d_12(&gpu12, case);
            keep(&format!("{name}.d3d12"), &texels, case.format);
            let (count, largest) = differ(&reference, &texels, case.format);
            eprintln!(
                "{name} d3d12-dxil against d3d11-dxbc: {count} of {} texels differ, largest difference {largest}",
                SIZE * SIZE
            );
            texels
        });
        // Flat values come from the first vertex, as Direct3D's, where the back end offers it.
        let provoking = if gl.caps().has(Features::PROVOKING_VERTEX_FIRST) {
            ProvokingVertex::First
        } else {
            ProvokingVertex::Last
        };
        let mut runs = Vec::new();
        for (route, dxil) in [("dxbc", false), ("dxil", true)] {
            if dxil && !case.dxil {
                continue;
            }
            let code = [
                bytecode(&format!("{}.{route}", case.vs)),
                bytecode(&format!("{}.{route}", case.ps)),
            ];
            let draw = Draw::own(case, &code, dxil, &grid);
            runs.push((
                format!("webgpu-{route}"),
                ours(&mut wgpu_backend, &draw, ProvokingVertex::First),
            ));
            match case.not_on_webgl[usize::from(dxil)] {
                None => runs.push((format!("webgl-{route}"), ours(&mut gl, &draw, provoking))),
                Some(why) => eprintln!("{name} webgl {route}: not drawn: {why}"),
            }
        }
        for (back_end, texels) in runs {
            keep(&format!("{name}.{back_end}"), &texels, case.format);
            let (against, reference) = match &reference_12 {
                Some(r) if back_end.ends_with("dxil") => ("direct3d 12", r),
                _ => ("direct3d 11", &reference),
            };
            let (count, largest) = differ(reference, &texels, case.format);
            eprintln!(
                "{name} {back_end}: {count} of {} texels differ from {against}, largest difference {largest}",
                SIZE * SIZE
            );
            if count > 0 {
                match case
                    .known_on_webgl
                    .filter(|_| back_end.starts_with("webgl"))
                {
                    Some(why) => eprintln!("{name} {back_end}: a known failure: {why}"),
                    None => failures.push(format!("{name} {back_end}")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "differ from Direct3D: {failures:?}");
}

/// Wave operations (`ps_wave`, shader model 6.0, dxc only) drawn through the WebGPU back end with
/// subgroups against real Direct3D 12's picture of the same DXIL, texel for texel: lane counts,
/// indices, sums, prefixes, ballots and reads of other lanes, with discarded pixels and the helper
/// lanes at the triangles' edges taking no part. wgpu on Vulkan, as wgpu's Direct3D 12 path gives
/// subgroups only through dxc.
#[test]
fn wave_operations_draw_as_direct3d_12_does() {
    let case = Case {
        vs: "vs_transform",
        ps: "ps_wave",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [Some("WebGL 2 has no subgroups"); 2],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    };
    let gpu12 = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let reference = direct3d_12(&gpu12, &case);
    keep("vs_transform+ps_wave.d3d12", &reference, case.format);
    let mut backend = webgpu_with(wgpu::Backends::VULKAN, wgpu::Features::SUBGROUP);
    let code = [bytecode("vs_transform.dxil"), bytecode("ps_wave.dxil")];
    let grid = grid();
    let draw = Draw::own(&case, &code, true, &grid);
    let texels = ours(&mut backend, &draw, ProvokingVertex::First);
    keep("vs_transform+ps_wave.webgpu-dxil", &texels, case.format);
    let drawn = reference
        .chunks(16)
        .filter(|t| t.iter().any(|&b| b != 0))
        .count();
    let (count, largest) = differ(&reference, &texels, case.format);
    eprintln!(
        "ps_wave webgpu-dxil: {count} of {} texels differ from direct3d 12 ({drawn} drawn), largest difference {largest}",
        SIZE * SIZE
    );
    assert!(drawn > 0);
    assert_eq!(count, 0);
}

/// Native 16-bit integers (`ps_narrow16`, shader model 6.2 with -enable-16bit-types, dxc only),
/// whose results wrap at 16 bits, drawn through the WebGPU and WebGL 2 back ends against real
/// Direct3D 12's picture of the same DXIL, texel for texel.
#[test]
fn native_16_bit_integers_draw_as_direct3d_12_does() {
    let case = Case {
        vs: "vs_transform",
        ps: "ps_narrow16",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    };
    // The back ends first: wgpu then finds the GPU's adapter whatever the reference draw does to
    // the process's Direct3D 12 device.
    let mut wgpu_backend = webgpu();
    let mut gl = webgl();
    let gpu12 = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let reference = direct3d_12(&gpu12, &case);
    keep("vs_transform+ps_narrow16.d3d12", &reference, case.format);
    let code = [bytecode("vs_transform.dxil"), bytecode("ps_narrow16.dxil")];
    let grid = grid();
    let draw = Draw::own(&case, &code, true, &grid);
    let provoking = if gl.caps().has(Features::PROVOKING_VERTEX_FIRST) {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    };
    let runs = [
        (
            "webgpu",
            ours(&mut wgpu_backend, &draw, ProvokingVertex::First),
        ),
        ("webgl", ours(&mut gl, &draw, provoking)),
    ];
    let mut failures = Vec::new();
    for (back_end, texels) in runs {
        keep(
            &format!("vs_transform+ps_narrow16.{back_end}-dxil"),
            &texels,
            case.format,
        );
        let (count, largest) = differ(&reference, &texels, case.format);
        eprintln!(
            "ps_narrow16 {back_end}-dxil: {count} of {} texels differ from direct3d 12, largest difference {largest}",
            SIZE * SIZE
        );
        if count > 0 {
            failures.push(back_end);
        }
    }
    assert!(failures.is_empty(), "differ from Direct3D 12: {failures:?}");
}

/// A DXIL target of two components into a four-component format (`ps_two`, a `uint2` into
/// `R32G32B32A32_UINT`), through both back ends against real Direct3D 12: the two components it
/// writes are exact; Direct3D leaves the other two undefined, so they are not compared.
#[test]
fn narrower_targets_draw_as_direct3d_12_does() {
    let case = Case {
        vs: "vs_transform",
        ps: "ps_two",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [None, None],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    };
    let mut wgpu_backend = webgpu();
    let mut gl = webgl();
    let gpu12 = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let texel = case.format.block().2 as usize;
    // `ps_two` writes a `uint2`.
    let written = texel / case.format.components() as usize * 2;
    // Only the components the program writes.
    let written_only = |mut texels: Vec<u8>| {
        for t in texels.chunks_mut(texel) {
            t[written..].fill(0);
        }
        texels
    };
    let reference = written_only(direct3d_12(&gpu12, &case));
    let code = [bytecode("vs_transform.dxil"), bytecode("ps_two.dxil")];
    let grid = grid();
    let draw = Draw::own(&case, &code, true, &grid);
    let provoking = if gl.caps().has(Features::PROVOKING_VERTEX_FIRST) {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    };
    let runs = [
        (
            "webgpu",
            ours(&mut wgpu_backend, &draw, ProvokingVertex::First),
        ),
        ("webgl", ours(&mut gl, &draw, provoking)),
    ];
    let mut failures = Vec::new();
    for (back_end, texels) in runs {
        let (count, largest) = differ(&reference, &written_only(texels), case.format);
        eprintln!(
            "ps_two {back_end}-dxil: {count} of {} texels differ from direct3d 12, largest difference {largest}",
            SIZE * SIZE
        );
        if count > 0 {
            failures.push(back_end);
        }
    }
    assert!(failures.is_empty(), "differ from Direct3D 12: {failures:?}");
}

/// The primitive index in a pixel shader without a geometry shader (`ps_primid`), natively through
/// the WebGPU back end on a device with wgpu's `PRIMITIVE_INDEX`, from the fxc build against real
/// Direct3D 11 and from the dxc build against real Direct3D 12. Exact.
#[test]
fn primitive_index_draws_as_direct3d_does() {
    let case = Case {
        vs: "vs_transform",
        ps: "ps_primid",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: [Some("GLSL ES 3.00 has no primitive index"); 2],
        known_on_webgl: None,
        dxil: true,
        dxil_against_11: false,
    };
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let gpu12 = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let mut backend = webgpu_with(wgpu::Backends::DX12, wgpu::Features::PRIMITIVE_INDEX);
    let grid = grid();
    for (dxil, ext) in [(false, "dxbc"), (true, "dxil")] {
        let reference = match dxil {
            false => direct3d(&gpu, &case),
            true => direct3d_12(&gpu12, &case),
        };
        let code = [
            bytecode(&format!("vs_transform.{ext}")),
            bytecode(&format!("ps_primid.{ext}")),
        ];
        let draw = Draw::own(&case, &code, dxil, &grid);
        let texels = ours(&mut backend, &draw, ProvokingVertex::First);
        keep(
            &format!("vs_transform+ps_primid.webgpu-{ext}"),
            &texels,
            case.format,
        );
        let primitives: std::collections::BTreeSet<_> = reference
            .chunks(16)
            .filter(|t| t[12] == 7)
            .map(|t| t[..4].to_vec())
            .collect();
        let (count, largest) = differ(&reference, &texels, case.format);
        eprintln!(
            "ps_primid webgpu-{ext}: {count} of {} texels differ ({} primitives drawn), largest \
             difference {largest}",
            SIZE * SIZE,
            primitives.len()
        );
        assert!(primitives.len() > 1);
        assert_eq!(count, 0);
    }
}

/// The rig's triangle scene drawn from its DXIL (dxc) and its DXBC (fxc) through both back ends, each
/// against the picture real Direct3D 12 drew, with the rig's allowance (one step, as WARP).
#[test]
fn rig_triangle_from_dxil_and_dxbc_draws_as_direct3d_12_does() {
    use wcontainers_graphics_test::scene;
    let rig = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test");
    let read = |name: &str| std::fs::read(rig.join("shaders").join(name)).unwrap();
    let reference = wcontainers_graphics_test::load(&rig.join("references/d3d12/triangle.png"))
        .unwrap()
        .to_rgba8()
        .into_raw();
    let dxil = [read("dxil/triangle.vs.dxil"), read("dxil/triangle.ps.dxil")];
    let dxbc = [read("dxbc/triangle.vs.dxbc"), read("dxbc/triangle.ps.dxbc")];
    let vertices: Vec<f32> = scene::TRIANGLE.iter().flatten().copied().collect();
    assert_eq!(scene::WIDTH, scene::HEIGHT);
    let draw = |vs, ps| Draw {
        vs,
        ps,
        format: Format::Rgba8Unorm,
        size: scene::WIDTH,
        vertices: &vertices,
        stride: 20,
        attributes: &[
            ("POSITION", 0, VertexFormat::Float32x2),
            ("COLOR", 8, VertexFormat::Float32x3),
        ],
        count: 3,
        constants: vec![0; 16],
        clear: scene::CLEAR.map(f32::to_bits),
        setup: Setup::plain(scene::WIDTH),
    };
    let mut wgpu_backend = webgpu();
    let mut gl = webgl();
    let mut failures = Vec::new();
    for (route, code) in [("dxil", &dxil), ("dxbc", &dxbc)] {
        let (vs, ps) = match route {
            "dxil" => (Source::Dxil(&code[0]), Source::Dxil(&code[1])),
            _ => (Source::Dxbc(&code[0]), Source::Dxbc(&code[1])),
        };
        let d = draw(vs, ps);
        for (back_end, texels) in [
            (
                "webgpu",
                ours(&mut wgpu_backend, &d, ProvokingVertex::First),
            ),
            ("webgl", ours(&mut gl, &d, ProvokingVertex::Last)),
        ] {
            keep(&format!("triangle.{route}.{back_end}"), &texels, d.format);
            let beyond = reference
                .chunks(4)
                .zip(texels.chunks(4))
                .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 1))
                .count();
            let (count, largest) = differ(&reference, &texels, d.format);
            eprintln!(
                "triangle {route} {back_end}: {beyond} texels beyond 1, {count} differ, largest difference {largest}"
            );
            if beyond > 0 {
                failures.push(format!("{route} {back_end}"));
            }
        }
    }
    assert!(failures.is_empty(), "differ from Direct3D 12: {failures:?}");
}

/// Real Direct3D refuses a container whose checksum does not match its bytes, as `convert` does: DXBC
/// through Direct3D 11's shader creation, DXIL through Direct3D 12's pipeline creation.
#[test]
fn direct3d_refuses_a_wrong_checksum_as_convert_does() {
    let rig = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders");
    let corrupt = |mut b: Vec<u8>| {
        let at = b.len() - 8;
        b[at] ^= 1;
        b
    };
    let dxbc = std::fs::read(rig.join("dxbc/triangle.ps.dxbc")).unwrap();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let create = |code: &[u8]| unsafe { gpu.device.CreatePixelShader(code, None, None) };
    assert!(create(&dxbc).is_ok());
    let bad = corrupt(dxbc);
    let refused = create(&bad).unwrap_err().code();
    eprintln!("direct3d 11: {refused:?}");
    assert_eq!(refused, windows::Win32::Foundation::E_INVALIDARG);
    assert!(matches!(
        convert(Source::Dxbc(&bad), &VariantKey::default(), None),
        Err(wcontainers_shaders::ConvertError::Rejected(_))
    ));

    use windows::Win32::Graphics::Direct3D12::*;
    let dxil = std::fs::read(rig.join("dxil/triangle.vs.dxil")).unwrap();
    let gpu = wcontainers_graphics_test::d3d12::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
    )
    .unwrap();
    let root = gpu
        .root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                    ..Default::default()
                },
            },
        })
        .unwrap();
    let layout = [
        wcontainers_graphics_test::d3d12::element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0),
        wcontainers_graphics_test::d3d12::element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 8),
    ];
    let pipeline = |vs: &[u8]| {
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
            VS: wcontainers_graphics_test::d3d12::bytecode(vs),
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: layout.len() as u32,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        desc.BlendState.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            SrcBlend: D3D12_BLEND_ONE,
            DestBlend: D3D12_BLEND_ZERO,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_ZERO,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: 0xf,
            ..Default::default()
        };
        let made: windows::core::Result<ID3D12PipelineState> =
            unsafe { gpu.device.CreateGraphicsPipelineState(&desc) };
        drop(std::mem::ManuallyDrop::into_inner(desc.pRootSignature));
        made
    };
    assert!(pipeline(&dxil).is_ok());
    let bad = corrupt(dxil);
    let refused = pipeline(&bad).unwrap_err().code();
    eprintln!("direct3d 12: {refused:?}");
    assert_eq!(refused, windows::Win32::Foundation::E_INVALIDARG);
    assert!(matches!(
        convert(Source::Dxil(&bad), &VariantKey::default(), None),
        Err(wcontainers_shaders::ConvertError::Rejected(_))
    ));
}
