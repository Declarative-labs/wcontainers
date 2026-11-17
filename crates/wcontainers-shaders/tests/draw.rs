//! The crate's own shaders (tests/own) drawn by real Direct3D 11 and, converted by `convert`, through
//! the WebGPU back end (wgpu on Direct3D 12) and the WebGL 2 back end (on ANGLE) on the same GPU. Each
//! picture must equal Direct3D's texel for texel, which checks that the converted programs mean what the
//! bytecode means.
//!
//! Windows only, and needs a GPU: run as a GPU job (docs/status-graphics-shaders.md). The WebGL 2 back
//! end loads ANGLE from `WCONTAINERS_ANGLE` (default C:\wc\tools\angle-mozangle-0.7.1, which the WebGL
//! track's angle\build.cmd makes).

#![cfg(windows)]

use std::path::Path;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendDesc, BorderColor, BufferDesc, BufferRange,
    BufferUsage, ColorAttachment, ColorWrites, CullMode, DepthStencilDesc, DrawCall, DrawRange,
    Extent, Features, FillMode, Filter, Format, FrontFace, Interpolation, MapIntent, Origin,
    PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend, SamplerDesc,
    SlotKind, StageSet, StepMode, StoreOp, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, Topology, VariantKey, VertexAttribute, VertexBinding,
    VertexFormat, VertexLayoutDesc, VertexStream, ViewDimension, Viewport, WriteMode,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

const SIZE: u32 = 64;
const TEXTURE_SIZE: u32 = 8;

/// One picture: a vertex and a pixel shader of tests/own, drawn into a target of `format`.
struct Case {
    vs: &'static str,
    ps: &'static str,
    format: Format,
    /// The constant buffer's tint (`ps_loop` takes its trip count from `w`).
    tint: [f32; 4],
    /// Drawn from the vertex grid; otherwise three vertices without a buffer.
    grid: bool,
    /// Why the WebGL 2 back end cannot draw it, if it cannot.
    not_on_webgl: Option<&'static str>,
    /// A known failure of the WebGL 2 back end, outside this crate: drawn and reported, not failed on.
    known_on_webgl: Option<&'static str>,
}

const CASES: [Case; 9] = [
    Case {
        vs: "vs_transform",
        ps: "ps_textured",
        format: Format::Rgba8Unorm,
        tint: [1.0, 0.75, 0.5, 1.5],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: None,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_loop",
        format: Format::Rgba8Unorm,
        tint: [1.0, 1.0, 1.0, 5.0],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: None,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_integer",
        format: Format::Rgba32Uint,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: Some("GLSL ES 3.00 has no bitfield operations"),
        known_on_webgl: None,
    },
    Case {
        vs: "vs_fullscreen",
        ps: "ps_window",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: false,
        not_on_webgl: Some("GLSL ES 3.00 has no bitfield operations (vs_fullscreen inserts bits)"),
        known_on_webgl: None,
    },
    Case {
        vs: "vs_shade",
        ps: "ps_flat",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: None,
    },
    Case {
        vs: "vs_shade",
        ps: "ps_shade",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: Some("GLSL ES 3.00 has no noperspective"),
        known_on_webgl: None,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_depth",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: Some("the fragment's z arrives as (z + 1) / 2, the back end's clip depth"),
    },
    Case {
        vs: "vs_transform",
        ps: "ps_local",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: None,
    },
    Case {
        vs: "vs_transform",
        ps: "ps_size",
        format: Format::Rgba8Unorm,
        tint: [1.0; 4],
        grid: true,
        not_on_webgl: None,
        known_on_webgl: None,
    },
];

fn bytecode(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/own/{name}.dxbc"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A 4 by 4 grid of quads over the target, two triangles each: position (x, y, 0) and a texture
/// coordinate reaching past [0, 1] so the clamp shows.
fn grid() -> Vec<f32> {
    let mut v = Vec::new();
    let n = 4;
    let at = |i: u32, j: u32| {
        let (x, y) = (
            i as f32 / n as f32 * 2.0 - 1.0,
            1.0 - j as f32 / n as f32 * 2.0,
        );
        [x, y, 0.0, (x + 1.0) * 0.75 - 0.25, (1.0 - y) * 0.75 - 0.25]
    };
    for j in 0..n {
        for i in 0..n {
            for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 0), (1, 1), (0, 1)] {
                v.extend(at(i + di, j + dj));
            }
        }
    }
    v
}

/// The constant buffer: `mul(float4(p, 1), m)` with the matrix stored column by column (HLSL's default
/// packing), so register j holds what output component j is a dot product with. w varies with x, so
/// interpolation is perspective-correct or visibly not.
fn constants(tint: [f32; 4]) -> Vec<u8> {
    let columns: [[f32; 4]; 4] = [
        [0.75, -0.2, 0.0, 0.05],
        [0.2, 0.75, 0.0, -0.05],
        [0.0, 0.0, 0.25, 0.5],
        [0.25, 0.0, 0.0, 1.0],
    ];
    columns
        .iter()
        .flatten()
        .chain(tint.iter())
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// An 8 by 8 RGBA8 texture whose alpha crosses `ps_textured`'s clip threshold.
fn texels() -> Vec<u8> {
    let mut t = Vec::new();
    for y in 0..TEXTURE_SIZE {
        for x in 0..TEXTURE_SIZE {
            t.extend([
                (x * 36) as u8,
                (y * 36) as u8,
                ((x ^ y) * 36) as u8,
                ((x + y) % 3 * 110) as u8,
            ]);
        }
    }
    t
}

const CLEAR: [f32; 4] = [0.125, 0.25, 0.5, 1.0];

fn dxgi(format: Format) -> DXGI_FORMAT {
    match format {
        Format::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        Format::Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        f => panic!("no case draws into {f:?}"),
    }
}

/// Draws a case with real Direct3D 11; returns the target's texels, rows tightly packed.
fn direct3d(gpu: &Gpu, case: &Case) -> Vec<u8> {
    let (vs, ps) = (bytecode(case.vs), bytecode(case.ps));
    let elements = if case.grid {
        vec![
            d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
            d3d11::element(s!("TEXCOORD"), 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12, 0),
        ]
    } else {
        Vec::new()
    };
    gpu.program(&vs, &ps, &elements).unwrap();
    let vertices = grid();
    let _buffer = case
        .grid
        .then(|| gpu.vertices(0, bytes_of(&vertices), 20).unwrap());
    let constants = gpu
        .immutable(&constants(case.tint), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let texels = texels();
    let texture = gpu
        .texture2d(
            &d3d11::tex2d(
                TEXTURE_SIZE,
                TEXTURE_SIZE,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D11_BIND_SHADER_RESOURCE,
            ),
            &[d3d11::data(&texels, TEXTURE_SIZE * 4, 0)],
        )
        .unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let sampler = gpu
        .sampler(&D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        })
        .unwrap();
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, dxgi(case.format)).unwrap();
    unsafe {
        let c = &gpu.context;
        c.VSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
        c.PSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
        c.PSSetShaderResources(0, Some(&[Some(view)]));
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
        c.RSSetState(&raster);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        if case.format == Format::Rgba32Uint {
            c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        } else {
            c.ClearRenderTargetView(&rtv, &CLEAR);
        }
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        gpu.viewport(SIZE, SIZE);
        c.Draw(if case.grid { 96 } else { 3 }, 0);
    }
    read(gpu, &target, case.format)
}

/// Reads a target back through a staging texture.
fn read(gpu: &Gpu, texture: &ID3D11Texture2D, format: Format) -> Vec<u8> {
    let row = (SIZE * format.block().2) as usize;
    let mut desc = d3d11::tex2d(SIZE, SIZE, dxgi(format), D3D11_BIND_FLAG(0));
    desc.Usage = D3D11_USAGE_STAGING;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let staging = gpu.texture2d(&desc, &[]).unwrap();
    let mut out = Vec::with_capacity(row * SIZE as usize);
    unsafe {
        gpu.context.CopyResource(&staging, texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        for y in 0..SIZE as usize {
            let start = mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize);
            out.extend_from_slice(std::slice::from_raw_parts(start, row));
        }
        gpu.context.Unmap(&staging, 0);
    }
    out
}

fn bytes_of(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and every byte pattern is a valid u8.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}

fn buffer<B: Backend>(b: &mut B, data: &[u8], usage: BufferUsage) -> B::Buffer {
    let buf = b
        .create_buffer(&BufferDesc {
            size: data.len() as u64,
            usage: usage | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    b.write_buffer(&buf, 0, data, WriteMode::Ordered);
    buf
}

fn texture<B: Backend>(
    b: &mut B,
    format: Format,
    size: u32,
    usage: TextureUsage,
) -> (B::Texture, B::TextureView) {
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: size,
            height: size,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let v = b
        .create_view(
            &t,
            &TextureViewDesc {
                format,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .unwrap();
    (t, v)
}

/// One draw through a back end: the shaders, the vertices and the target.
struct Draw<'a> {
    vs: Source<'a>,
    ps: Source<'a>,
    format: Format,
    size: u32,
    /// Vertices of `stride` bytes; an input of the vertex shader finds its offset and format by
    /// semantic in `attributes`.
    vertices: &'a [f32],
    stride: u32,
    attributes: &'a [(&'a str, u32, VertexFormat)],
    count: u32,
    constants: Vec<u8>,
    clear: [u32; 4],
}

impl<'a> Draw<'a> {
    /// One of the cases of tests/own; `code` holds its vertex and pixel shader's bytecode.
    fn own(case: &Case, code: &'a [Vec<u8>; 2], grid: &'a [f32]) -> Draw<'a> {
        const ATTRIBUTES: &[(&str, u32, VertexFormat)] = &[
            ("POSITION", 0, VertexFormat::Float32x3),
            ("TEXCOORD", 12, VertexFormat::Float32x2),
        ];
        Draw {
            vs: Source::Dxbc(&code[0]),
            ps: Source::Dxbc(&code[1]),
            format: case.format,
            size: SIZE,
            vertices: if case.grid { grid } else { &[] },
            stride: 20,
            attributes: ATTRIBUTES,
            count: if case.grid { 96 } else { 3 },
            constants: constants(case.tint),
            clear: if case.format == Format::Rgba32Uint {
                [0; 4]
            } else {
                CLEAR.map(f32::to_bits)
            },
        }
    }
}

/// Draws through a back end with the converted shaders, binding by their reflection (every slot is
/// register 0: the constants, the texture of [`texels`] and a linear clamping sampler); returns the
/// target's texels. `provoking` is the back end's own (it only decides `ps_integer`'s value).
fn ours<B: Backend>(b: &mut B, draw: &Draw<'_>, provoking: ProvokingVertex) -> Vec<u8> {
    // As a front links the stages: the vertex program writes each output as the pixel program reads it.
    let ps = convert(draw.ps, &VariantKey::default(), None).unwrap();
    let key = VariantKey {
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..VariantKey::default()
    };
    let vs = convert(draw.vs, &key, None).unwrap();
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();

    let constants = buffer(b, &draw.constants, BufferUsage::UNIFORM);
    let (texture_object, view) = texture(
        b,
        Format::Rgba8Unorm,
        TEXTURE_SIZE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    b.write_texture(
        &TextureRegion {
            texture: &texture_object,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_layers: 1,
        },
        &texels(),
        TEXTURE_SIZE * 4,
        TEXTURE_SIZE,
        WriteMode::Ordered,
    );
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Linear,
            mag: Filter::Linear,
            mip: Filter::Linear,
            address: [wcontainers_gpu::AddressMode::Clamp; 3],
            lod_range: [0, f32::MAX.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .unwrap();

    // Each stage's slots, from its reflection: vertex slots in set 0, pixel slots in set 1.
    let mut sets: [Vec<BindingEntry<'_, B>>; 2] = [Vec::new(), Vec::new()];
    for (reflection, stages) in [
        (&vs.reflection, StageSet::VERTEX),
        (&ps.reflection, StageSet::FRAGMENT),
    ] {
        for r in &reflection.bindings {
            assert_eq!(r.register, 0, "{r:?}");
            let resource = match r.kind {
                SlotKind::Uniform => BindingResource::UniformBuffer(BufferRange {
                    buffer: &constants,
                    offset: 0,
                    size: draw.constants.len() as u64,
                }),
                SlotKind::Texture => BindingResource::Texture(&view),
                SlotKind::Sampler => BindingResource::Sampler(&sampler),
                k => panic!("no case binds {k:?}"),
            };
            sets[r.set as usize].push(BindingEntry {
                binding: r.binding,
                stages,
                resource,
            });
        }
    }
    let sets =
        sets.map(|entries| (!entries.is_empty()).then(|| b.create_binding_set(&entries).unwrap()));

    // The vertex layout: each input of the vertex shader at its location, by semantic.
    let mut layout = VertexLayoutDesc {
        streams: [None; 32],
        attributes: [None; 32],
    };
    if !draw.vertices.is_empty() {
        layout.streams[0] = Some(VertexStream {
            stride: draw.stride,
            step: StepMode::Vertex,
        });
        for input in vs.reflection.inputs.iter().filter(|i| i.system.is_none()) {
            let &(_, offset, format) = draw
                .attributes
                .iter()
                .find(|a| a.0 == input.semantic)
                .unwrap_or_else(|| panic!("no vertex data for {}", input.semantic));
            layout.attributes[input.location as usize] = Some(VertexAttribute {
                location: input.location,
                stream: 0,
                offset,
                format,
            });
        }
    }
    let vertex_layout = b.create_vertex_layout(&layout).unwrap();
    let vertex_buffer = (!draw.vertices.is_empty())
        .then(|| buffer(b, bytes_of(draw.vertices), BufferUsage::VERTEX));
    let blend = b
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; 8],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let raster = b
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking,
            depth_minus_one: false,
            sample_mask: !0,
        })
        .unwrap();

    let (target, target_view) = texture(
        b,
        draw.format,
        draw.size,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: None,
        load: wcontainers_gpu::LoadOp::Clear(draw.clear),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let streams: Vec<_> = vertex_buffer
        .iter()
        .map(|buffer| Some(VertexBinding { buffer, offset: 0 }))
        .collect();
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [sets[0].as_ref(), sets[1].as_ref(), None, None],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: (draw.size as f32).to_bits(),
            height: (draw.size as f32).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: draw.count,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let mut out = vec![0; (draw.size * draw.size * draw.format.block().2) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: draw.size,
                height: draw.size,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

/// How many texels differ, and the largest difference of any channel (8-bit channels, or 32-bit
/// integers).
fn differ(a: &[u8], b: &[u8], format: Format) -> (usize, u64) {
    let channel = if format == Format::Rgba32Uint { 4 } else { 1 };
    let texel = 4 * channel;
    let mut count = 0;
    let mut largest = 0;
    for (x, y) in a.chunks(texel).zip(b.chunks(texel)) {
        let d = x
            .chunks(channel)
            .zip(y.chunks(channel))
            .map(|(p, q)| {
                let v = |c: &[u8]| c.iter().rev().fold(0u64, |n, b| n << 8 | *b as u64);
                v(p).abs_diff(v(q))
            })
            .max()
            .unwrap();
        if d > 0 {
            count += 1;
            largest = largest.max(d);
        }
    }
    (count, largest)
}

fn webgpu() -> wcontainers_gpu_webgpu::WebGpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a Direct3D 12 adapter");
    let info = adapter.get_info();
    assert_ne!(info.device_type, wgpu::DeviceType::Cpu, "{}", info.name);
    eprintln!("webgpu: {} ({:?})", info.name, info.backend);
    let (device, queue) = pollster::block_on(
        adapter.request_device(&wcontainers_gpu_webgpu::WebGpu::device_descriptor(&adapter)),
    )
    .unwrap();
    wcontainers_gpu_webgpu::WebGpu::new(&adapter, device, queue)
}

fn webgl() -> wcontainers_gpu_webgl::WebGl {
    let dir = std::env::var("WCONTAINERS_ANGLE")
        .unwrap_or_else(|_| r"C:\wc\tools\angle-mozangle-0.7.1".into());
    let gl = wcontainers_gpu_webgl::WebGl::on_angle(Path::new(&dir), &[]).unwrap();
    eprintln!("webgl: {}", gl.renderer());
    gl
}

/// Saves a picture for looking at when `WCONTAINERS_SHADERS_OUT` names a directory.
fn keep(name: &str, texels: &[u8], format: Format) {
    let Ok(dir) = std::env::var("WCONTAINERS_SHADERS_OUT") else {
        return;
    };
    let dir = Path::new(&dir).join("draw");
    std::fs::create_dir_all(&dir).unwrap();
    let ext = if format == Format::Rgba32Uint {
        "rgba32ui"
    } else {
        "rgba"
    };
    std::fs::write(dir.join(format!("{name}.{ext}")), texels).unwrap();
}

#[test]
fn own_shaders_draw_as_direct3d_does() {
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    eprintln!("direct3d 11: {}", gpu.adapter().unwrap());
    let mut wgpu_backend = webgpu();
    let mut gl = webgl();
    let mut failures = Vec::new();
    let grid = grid();
    for case in &CASES {
        let name = format!("{}+{}", case.vs, case.ps);
        let reference = direct3d(&gpu, case);
        keep(&format!("{name}.d3d11"), &reference, case.format);
        let code = [bytecode(case.vs), bytecode(case.ps)];
        let draw = Draw::own(case, &code, &grid);
        let mut runs = vec![(
            "webgpu",
            ours(&mut wgpu_backend, &draw, ProvokingVertex::First),
        )];
        // Flat values come from the first vertex, as Direct3D's, where the back end offers it.
        let provoking = if gl.caps().has(Features::PROVOKING_VERTEX_FIRST) {
            ProvokingVertex::First
        } else {
            ProvokingVertex::Last
        };
        match case.not_on_webgl {
            None => runs.push(("webgl", ours(&mut gl, &draw, provoking))),
            Some(why) => eprintln!("{name} webgl: not drawn: {why}"),
        }
        for (back_end, texels) in runs {
            keep(&format!("{name}.{back_end}"), &texels, case.format);
            let (count, largest) = differ(&reference, &texels, case.format);
            eprintln!(
                "{name} {back_end}: {count} of {} texels differ, largest difference {largest}",
                SIZE * SIZE
            );
            if count > 0 {
                match case.known_on_webgl.filter(|_| back_end == "webgl") {
                    Some(why) => eprintln!("{name} {back_end}: a known failure: {why}"),
                    None => failures.push(format!("{name} {back_end}")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "differ from Direct3D 11: {failures:?}");
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
