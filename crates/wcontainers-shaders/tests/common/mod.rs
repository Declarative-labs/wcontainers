//! What the drawing tests share: the own shaders' inputs, one draw through a back end, and comparing
//! pictures.

#![allow(dead_code)]

use std::path::Path;
use wcontainers_gpu::{
    AddedBinding, Backend, BindingEntry, BindingResource, BlendDesc, BorderColor, BufferDesc,
    BufferRange, BufferUsage, ColorAttachment, ColorWrites, CullMode, DepthStencilDesc, DrawCall,
    DrawRange, Extent, FillMode, Filter, Format, FrontFace, HelperValue, Interpolation, MapIntent,
    Origin, PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend,
    SamplerDesc, SlotKind, StageSet, StepMode, StoreOp, TEXEL_BUFFER_TEXTURE_WIDTH,
    TexelBufferRoute, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    Topology, VariantKey, VertexAttribute, VertexBinding, VertexFormat, VertexLayoutDesc,
    VertexStream, ViewDimension, Viewport, WriteMode,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

pub const SIZE: u32 = 64;
/// A vertex of position (x, y, z) and colour (r, g, b, a): the position's bytes, where the colour
/// starts, and the vertex's.
pub const POSITION_BYTES: u32 = size_of::<[f32; 3]>() as u32;
pub const COLOURED_VERTEX_BYTES: u32 = POSITION_BYTES + size_of::<[f32; 4]>() as u32;
pub const TEXTURE_SIZE: u32 = 8;
/// The 16-bit index that ends a strip.
pub const RESTART: u16 = u16::MAX;

/// Whether 16-bit `indices` hold, at the places in `drawn`, `vertices` indices in a row with no
/// restart among them: a strip's first primitive, which a seeded scene must have for its comparison
/// to show anything.
pub fn whole_strip_primitive(indices: &[u8], drawn: std::ops::Range<u32>, vertices: u32) -> bool {
    let mut run = 0;
    drawn.into_iter().any(|i| {
        let at = size_of::<u16>() * i as usize;
        run = match u16::from_le_bytes([indices[at], indices[at + 1]]) {
            RESTART => 0,
            _ => run + 1,
        };
        run >= vertices
    })
}

/// Index lists drawn for one indexed scene before one that Direct3D draws nothing of is compared
/// (and fails).
pub const SCENE_ATTEMPTS: u32 = 16;

/// A seeded scene's indices (`list` gives the seed's lists in order) and Direct3D's result for them
/// (`reference`). An indexed scene whose result `drew` says is empty (restarts that leave no whole
/// primitive, degenerate primitives, or primitives the shaders drop) is drawn again with the seed's
/// next list, since an empty scene compares little; ours is drawn on each list passed over and must
/// be `alike` Direct3D's empty result there.
pub fn scene<T>(
    indexed: bool,
    list: impl Fn(u32) -> Vec<u8>,
    mut reference: impl FnMut(&[u8]) -> T,
    drew: impl Fn(&T) -> bool,
    mut alike: impl FnMut(&[u8], &T) -> bool,
) -> Result<(Vec<u8>, T), String> {
    let mut attempt = 0;
    loop {
        let indices = list(attempt);
        let result = reference(&indices);
        if drew(&result) || !indexed || attempt + 1 == SCENE_ATTEMPTS {
            return Ok((indices, result));
        }
        if !alike(&indices, &result) {
            return Err(format!(
                "index list {attempt}: direct3d drew nothing of it, ours something"
            ));
        }
        attempt += 1;
    }
}

/// Texels a picture has drawn: those unlike the corner, which no case draws.
pub fn drawn_texels(picture: &[u8]) -> usize {
    picture.chunks(4).filter(|t| *t != &picture[..4]).count()
}

/// The container a program is read from: fxc's DXBC or dxc's DXIL.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Dxbc,
    Dxil,
}

impl Route {
    pub const BOTH: [Route; 2] = [Route::Dxbc, Route::Dxil];

    /// The container's file extension, which is also the rig's folder of its builds.
    pub fn extension(self) -> &'static str {
        match self {
            Route::Dxbc => "dxbc",
            Route::Dxil => "dxil",
        }
    }

    pub fn source(self, code: &[u8]) -> Source<'_> {
        match self {
            Route::Dxbc => Source::Dxbc(code),
            Route::Dxil => Source::Dxil(code),
        }
    }
}

pub fn bytecode(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/own/{name}"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A 4 by 4 grid of quads over the target, two triangles each: position (x, y, 0) and a texture
/// coordinate reaching past [0, 1] so the clamp shows.
pub fn grid() -> Vec<f32> {
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
pub fn constants(tint: [f32; 4]) -> Vec<u8> {
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
pub fn texels() -> Vec<u8> {
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

pub const CLEAR: [f32; 4] = [0.125, 0.25, 0.5, 1.0];

/// The difference allowed a filtered sample against Direct3D's: a filter's weights at 8 bits on
/// either side, on values of at most one.
pub const FILTER_ALLOWANCE: f32 = 1.0 / 128.0;

pub fn dxgi(format: Format) -> DXGI_FORMAT {
    match format {
        Format::R8Unorm => DXGI_FORMAT_R8_UNORM,
        Format::R8Snorm => DXGI_FORMAT_R8_SNORM,
        Format::R8Uint => DXGI_FORMAT_R8_UINT,
        Format::R8Sint => DXGI_FORMAT_R8_SINT,
        Format::R16Unorm => DXGI_FORMAT_R16_UNORM,
        Format::R16Snorm => DXGI_FORMAT_R16_SNORM,
        Format::R16Uint => DXGI_FORMAT_R16_UINT,
        Format::R16Sint => DXGI_FORMAT_R16_SINT,
        Format::R16Float => DXGI_FORMAT_R16_FLOAT,
        Format::Rg8Unorm => DXGI_FORMAT_R8G8_UNORM,
        Format::Rg8Snorm => DXGI_FORMAT_R8G8_SNORM,
        Format::Rg8Uint => DXGI_FORMAT_R8G8_UINT,
        Format::Rg8Sint => DXGI_FORMAT_R8G8_SINT,
        Format::R32Uint => DXGI_FORMAT_R32_UINT,
        Format::R32Sint => DXGI_FORMAT_R32_SINT,
        Format::R32Float => DXGI_FORMAT_R32_FLOAT,
        Format::Rg16Unorm => DXGI_FORMAT_R16G16_UNORM,
        Format::Rg16Snorm => DXGI_FORMAT_R16G16_SNORM,
        Format::Rg16Uint => DXGI_FORMAT_R16G16_UINT,
        Format::Rg16Sint => DXGI_FORMAT_R16G16_SINT,
        Format::Rg16Float => DXGI_FORMAT_R16G16_FLOAT,
        Format::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        Format::Rgba8Snorm => DXGI_FORMAT_R8G8B8A8_SNORM,
        Format::Rgba8Uint => DXGI_FORMAT_R8G8B8A8_UINT,
        Format::Rgba8Sint => DXGI_FORMAT_R8G8B8A8_SINT,
        Format::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        Format::Rgb10a2Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
        Format::Rgb10a2Uint => DXGI_FORMAT_R10G10B10A2_UINT,
        Format::Rg11b10Float => DXGI_FORMAT_R11G11B10_FLOAT,
        Format::Rg32Uint => DXGI_FORMAT_R32G32_UINT,
        Format::Rg32Sint => DXGI_FORMAT_R32G32_SINT,
        Format::Rg32Float => DXGI_FORMAT_R32G32_FLOAT,
        Format::Rgba16Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
        Format::Rgba16Snorm => DXGI_FORMAT_R16G16B16A16_SNORM,
        Format::Rgba16Uint => DXGI_FORMAT_R16G16B16A16_UINT,
        Format::Rgba16Sint => DXGI_FORMAT_R16G16B16A16_SINT,
        Format::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        Format::Rgb32Uint => DXGI_FORMAT_R32G32B32_UINT,
        Format::Rgb32Sint => DXGI_FORMAT_R32G32B32_SINT,
        Format::Rgb32Float => DXGI_FORMAT_R32G32B32_FLOAT,
        Format::Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        Format::Rgba32Sint => DXGI_FORMAT_R32G32B32A32_SINT,
        Format::Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        Format::Depth32Float => DXGI_FORMAT_D32_FLOAT,
        Format::Depth24PlusStencil8 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        f => panic!("no case uses {f:?}"),
    }
}

/// A typed buffer the pixel program reads at texture slot n (its index): the view's format, the
/// buffer's bytes, and the view's first element and length.
#[derive(Clone)]
pub struct TexelData {
    pub format: Format,
    pub bytes: Vec<u8>,
    pub first: u32,
    pub count: u32,
}

/// What a Direct3D 11 reference draw changes from the plain one.
#[derive(Default)]
pub struct Reference {
    /// Levels of the sampled texture as [`level_texels`] makes them (one when zero).
    pub levels: u32,
    /// The sampler's MipLODBias.
    pub lod_bias: f32,
    /// Target 0's blending.
    pub blend: Option<D3D11_RENDER_TARGET_BLEND_DESC>,
    /// Counts the samples that pass, with an occlusion query.
    pub count: bool,
    /// With a D32_FLOAT depth buffer cleared to 1 and written by every fragment, and this viewport
    /// depth range: the depth buffer's texels are returned instead of the target's.
    pub depth: Option<[f32; 2]>,
    /// With `depth`: a D24_UNORM_S8_UINT depth buffer instead, drawn with this depth bias (units,
    /// slope scale, clamp); its texels are returned as 24-bit codes, one u32 each.
    pub depth_bias: Option<(i32, f32, f32)>,
    /// The draw's range, where not the whole grid from vertex 0.
    pub bases: Option<Bases>,
    /// The sampled texture's per-resource minimum level-of-detail clamp (`SetResourceMinLOD`).
    pub min_lod: Option<f32>,
    /// Depth clamped instead of clipped (the rasteriser's `DepthClipEnable` false).
    pub depth_clip_off: bool,
    /// The sampler's address modes, where not clamping.
    pub address: Option<[D3D11_TEXTURE_ADDRESS_MODE; 3]>,
    /// Typed buffers bound at the pixel shader's texture slots instead of the texture.
    pub texel_buffers: Vec<TexelData>,
    /// The target's sample count, where more than one: it is resolved for reading.
    pub samples: u32,
    /// The pixel shader's texture slot left empty.
    pub unbound: bool,
    /// The target cleared to zero instead of [`CLEAR`].
    pub cleared_to_zero: bool,
    /// The target cleared to this instead of [`CLEAR`].
    pub clear: Option<[f32; 4]>,
    /// The blend constant.
    pub blend_factor: [f32; 4],
    /// Drawn without a pixel shader.
    pub no_pixel_shader: bool,
}

/// A draw's range with bases, direct or from an argument buffer; an indexed draw's indices are 0, 1,
/// 2 and so on.
#[derive(Clone, Copy, Debug)]
pub struct Bases {
    pub count: u32,
    /// The first vertex, or an indexed draw's first index.
    pub first: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
    pub instances: u32,
    pub indexed: bool,
    /// Arguments read from a buffer, [`ARGUMENTS_WORD`] words into it.
    pub indirect: bool,
}

/// Where an indirect draw's arguments start in their buffer, in words.
pub const ARGUMENTS_WORD: u32 = 4;

impl Bases {
    /// The argument buffer of an indirect draw, as Direct3D and WebGPU both lay it out.
    pub fn arguments(&self) -> Vec<u8> {
        let mut words = vec![0xdead_beef; ARGUMENTS_WORD as usize];
        words.extend(if self.indexed {
            vec![
                self.count,
                self.instances,
                self.first,
                self.base_vertex as u32,
                self.first_instance,
            ]
        } else {
            vec![self.count, self.instances, self.first, self.first_instance]
        });
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    pub fn indices(&self) -> Vec<u8> {
        (0..self.first + self.count)
            .flat_map(|i| i.to_le_bytes())
            .collect()
    }
}

/// Draws the own shaders `vs` and `ps` (fxc's builds) with real Direct3D 11 as the drawing tests bind
/// them; returns the target's texels, rows tightly packed, and the query's count.
pub fn direct3d_11(
    gpu: &Gpu,
    vs: &str,
    ps: &str,
    format: Format,
    tint: [f32; 4],
    grid_drawn: bool,
    reference: &Reference,
) -> (Vec<u8>, Option<u64>) {
    let (vs, ps) = (
        bytecode(&format!("{vs}.dxbc")),
        bytecode(&format!("{ps}.dxbc")),
    );
    let elements = if grid_drawn {
        vec![
            d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
            d3d11::element(s!("TEXCOORD"), 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12, 0),
        ]
    } else {
        Vec::new()
    };
    gpu.program(&vs, &ps, &elements).unwrap();
    if reference.no_pixel_shader {
        unsafe { gpu.context.PSSetShader(None, None) };
    }
    let vertices = grid();
    let _buffer = grid_drawn.then(|| gpu.vertices(0, bytes_of(&vertices), 20).unwrap());
    let constants = gpu
        .immutable(&constants(tint), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let levels = reference.levels.max(1);
    let level_data: Vec<Vec<u8>> = (0..levels).map(level_texels).collect();
    let mut desc = d3d11::tex2d(
        TEXTURE_SIZE,
        TEXTURE_SIZE,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_SHADER_RESOURCE,
    );
    desc.MipLevels = levels;
    if reference.min_lod.is_some() {
        desc.MiscFlags = D3D11_RESOURCE_MISC_RESOURCE_CLAMP.0 as u32;
    }
    let data: Vec<_> = level_data
        .iter()
        .enumerate()
        .map(|(n, t)| d3d11::data(t, (TEXTURE_SIZE >> n).max(1) * 4, 0))
        .collect();
    let texture = gpu.texture2d(&desc, &data).unwrap();
    if let Some(clamp) = reference.min_lod {
        let resource: ID3D11Resource = texture.cast().unwrap();
        unsafe { gpu.context.SetResourceMinLOD(&resource, clamp) };
    }
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let views: Vec<Option<ID3D11ShaderResourceView>> = if reference.unbound {
        vec![None]
    } else if reference.texel_buffers.is_empty() {
        vec![Some(view)]
    } else {
        reference
            .texel_buffers
            .iter()
            .map(|t| {
                let buffer = gpu.immutable(&t.bytes, D3D11_BIND_SHADER_RESOURCE).unwrap();
                let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: dxgi(t.format),
                    ViewDimension: D3D_SRV_DIMENSION_BUFFER,
                    Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                        Buffer: D3D11_BUFFER_SRV {
                            Anonymous1: D3D11_BUFFER_SRV_0 {
                                FirstElement: t.first,
                            },
                            Anonymous2: D3D11_BUFFER_SRV_1 {
                                NumElements: t.count,
                            },
                        },
                    },
                };
                Some(gpu.srv(&buffer.cast().unwrap(), Some(&desc)).unwrap())
            })
            .collect()
    };
    let address = reference
        .address
        .unwrap_or([D3D11_TEXTURE_ADDRESS_CLAMP; 3]);
    let sampler = gpu
        .sampler(&D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: address[0],
            AddressV: address[1],
            AddressW: address[2],
            MipLODBias: reference.lod_bias,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        })
        .unwrap();
    let mut raster = d3d11::solid();
    if let Some((units, slope, clamp)) = reference.depth_bias {
        raster.DepthBias = units;
        raster.SlopeScaledDepthBias = slope;
        raster.DepthBiasClamp = clamp;
    }
    raster.DepthClipEnable = (!reference.depth_clip_off).into();
    let raster = gpu.rasterizer(&raster).unwrap();
    let (target, rtv) = match reference.samples {
        0 | 1 => gpu.target(SIZE, SIZE, dxgi(format)).unwrap(),
        samples => {
            let mut desc = d3d11::tex2d(SIZE, SIZE, dxgi(format), D3D11_BIND_RENDER_TARGET);
            desc.SampleDesc.Count = samples;
            let texture = gpu.texture2d(&desc, &[]).unwrap();
            let view = gpu.rtv(&texture.cast().unwrap(), None).unwrap();
            (texture, view)
        }
    };
    let blend = reference.blend.map(|target| {
        let mut desc = D3D11_BLEND_DESC::default();
        desc.RenderTarget[0] = target;
        gpu.blend(&desc).unwrap()
    });
    let query = reference.count.then(|| {
        let desc = D3D11_QUERY_DESC {
            Query: D3D11_QUERY_OCCLUSION,
            MiscFlags: 0,
        };
        d3d11::made(|q| unsafe { gpu.device.CreateQuery(&desc, Some(q)) }).unwrap()
    });
    let depth = reference.depth.map(|_| {
        let format = match reference.depth_bias {
            Some(_) => DXGI_FORMAT_D24_UNORM_S8_UINT,
            None => DXGI_FORMAT_D32_FLOAT,
        };
        let (texture, view) = gpu.depth(SIZE, SIZE, format).unwrap();
        let state = gpu
            .depth_stencil(&D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                ..Default::default()
            })
            .unwrap();
        (texture, view, state)
    });
    let mut count = None;
    unsafe {
        let c = &gpu.context;
        c.VSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
        c.PSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
        c.PSSetShaderResources(0, Some(&views));
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
        c.RSSetState(&raster);
        c.OMSetBlendState(blend.as_ref(), Some(&reference.blend_factor), !0);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        if let Some(clear) = reference.clear {
            c.ClearRenderTargetView(&rtv, &clear);
        } else if format == Format::Rgba32Uint || reference.cleared_to_zero {
            c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        } else {
            c.ClearRenderTargetView(&rtv, &CLEAR);
        }
        c.OMSetRenderTargets(Some(&[Some(rtv)]), depth.as_ref().map(|d| &d.1));
        gpu.viewport(SIZE, SIZE);
        if let (Some((_, view, state)), Some([near, far])) = (&depth, reference.depth) {
            c.ClearDepthStencilView(view, D3D11_CLEAR_DEPTH.0, 1.0, 0);
            c.OMSetDepthStencilState(state, 0);
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: SIZE as f32,
                Height: SIZE as f32,
                MinDepth: near,
                MaxDepth: far,
            }]));
        }
        if let Some(q) = &query {
            c.Begin(q);
        }
        let mut kept = Vec::new();
        match reference.bases {
            None => c.Draw(if grid_drawn { 96 } else { 3 }, 0),
            Some(d) => {
                if d.indexed {
                    let indices = gpu
                        .immutable(&d.indices(), D3D11_BIND_INDEX_BUFFER)
                        .unwrap();
                    c.IASetIndexBuffer(&indices, DXGI_FORMAT_R32_UINT, 0);
                    kept.push(indices);
                }
                let arguments = d.indirect.then(|| {
                    let bytes = d.arguments();
                    gpu.buffer(
                        &D3D11_BUFFER_DESC {
                            ByteWidth: bytes.len() as u32,
                            Usage: D3D11_USAGE_DEFAULT,
                            MiscFlags: D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
                            ..Default::default()
                        },
                        Some(&bytes),
                    )
                    .unwrap()
                });
                let at = ARGUMENTS_WORD * 4;
                match (d.indexed, &arguments) {
                    (false, None) => {
                        c.DrawInstanced(d.count, d.instances, d.first, d.first_instance)
                    }
                    (true, None) => c.DrawIndexedInstanced(
                        d.count,
                        d.instances,
                        d.first,
                        d.base_vertex,
                        d.first_instance,
                    ),
                    (false, Some(a)) => c.DrawInstancedIndirect(a, at),
                    (true, Some(a)) => c.DrawIndexedInstancedIndirect(a, at),
                }
                kept.extend(arguments);
            }
        }
        if let Some(q) = &query {
            c.End(q);
            let mut n = 0u64;
            loop {
                // The raw call: its S_FALSE (not ready yet) is a success the wrapper hides.
                let hr = (Interface::vtable(c).GetData)(
                    Interface::as_raw(c),
                    Interface::as_raw(q),
                    (&mut n as *mut u64).cast(),
                    8,
                    0,
                );
                if hr == windows::Win32::Foundation::S_OK {
                    break;
                }
                hr.ok().unwrap();
            }
            count = Some(n);
        }
        c.OMSetBlendState(None, None, !0);
        c.OMSetDepthStencilState(None, 0);
    }
    if let Some((texture, _, _)) = &depth {
        if reference.depth_bias.is_some() {
            let codes = read(gpu, texture, Format::Depth24PlusStencil8)
                .chunks(4)
                .flat_map(|t| (u32::from_le_bytes(t.try_into().unwrap()) & 0xff_ffff).to_le_bytes())
                .collect();
            return (codes, count);
        }
        return (read(gpu, texture, Format::Depth32Float), count);
    }
    if reference.samples > 1 {
        let (resolved, _) = gpu.target(SIZE, SIZE, dxgi(format)).unwrap();
        unsafe {
            gpu.context
                .ResolveSubresource(&resolved, 0, &target, 0, dxgi(format))
        };
        return (read(gpu, &resolved, format), count);
    }
    (read(gpu, &target, format), count)
}

/// Reads a target back through a staging texture.
pub fn read(gpu: &Gpu, texture: &ID3D11Texture2D, format: Format) -> Vec<u8> {
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

pub fn bytes_of(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and every byte pattern is a valid u8.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}

pub fn buffer<B: Backend>(b: &mut B, data: &[u8], usage: BufferUsage) -> B::Buffer {
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

pub fn texture<B: Backend>(
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

/// A 2D texture of `levels` levels and `samples` samples, and a view of all of it.
pub fn texture_levels<B: Backend>(
    b: &mut B,
    format: Format,
    size: u32,
    levels: u32,
    samples: u32,
    usage: TextureUsage,
) -> (B::Texture, B::TextureView) {
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: size,
            height: size,
            depth_or_layers: 1,
            mip_levels: levels,
            samples,
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
                mip_count: levels,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .unwrap();
    (t, v)
}

/// One draw through a back end: the shaders, the vertices and the target.
#[derive(Clone)]
pub struct Draw<'a> {
    pub vs: Source<'a>,
    pub ps: Source<'a>,
    pub format: Format,
    pub size: u32,
    /// Vertices of `stride` bytes; an input of the vertex shader finds its offset and format by
    /// semantic in `attributes`.
    pub vertices: &'a [f32],
    pub stride: u32,
    pub attributes: &'a [(&'a str, u32, VertexFormat)],
    pub count: u32,
    pub constants: Vec<u8>,
    pub clear: [u32; 4],
    pub setup: Setup,
}

/// What a draw changes from the plain one, for the variant tests.
#[derive(Clone)]
pub struct Setup {
    /// The keys `ours` asks for each stage (it adds the interpolation that links them).
    pub vs_key: VariantKey,
    pub ps_key: VariantKey,
    /// Each stage's helper block (vertex, fragment), bound in set 3 where not empty.
    pub helpers: [Vec<u8>; 2],
    /// x, y, width and height in pixels, then the depth range.
    pub viewport: [f32; 6],
    /// The target's sample count; a multisampled target is resolved for reading.
    pub samples: u32,
    pub sample_mask: u32,
    /// With a Depth32Float target cleared to 1 and written by every fragment: `ours` returns its
    /// texels instead of the colour target's.
    pub depth: bool,
    pub depth_minus_one: bool,
    pub blend: Option<(
        wcontainers_gpu::BlendComponent,
        wcontainers_gpu::BlendComponent,
    )>,
    pub blend_constant: [f32; 4],
    /// Levels of the sampled texture: level 0 is [`texels`], level n the solid [`level_colour`].
    pub levels: u32,
    /// The draw's range, where not `count` vertices from vertex 0; an argument buffer is also bound
    /// at [`wcontainers_gpu::DRAW_ARGUMENTS_BINDING`] when the vertex program reads its bases there.
    pub bases: Option<Bases>,
    /// The sampled texture's minimum level-of-detail clamp: the pixel program's samples go through a
    /// merged sampler whose range starts at it, bound at
    /// [`wcontainers_gpu::CLAMPED_SAMPLER_BINDING`] plus each pair's index.
    pub min_lod: Option<f32>,
    /// The sampler's address modes.
    pub address: [wcontainers_gpu::AddressMode; 3],
    /// Typed buffers the pixel program reads at texture slots, by the route its key names; their
    /// views' places and lengths are put in its helper block.
    pub texel_buffers: Vec<TexelData>,
    /// Layers of a multisampled array texture bound at texture slot 0 ([`multisampled_layers`]), a
    /// texture per layer as `VariantKey::texture_layers` binds them; none where 0.
    pub layers: u32,
    /// A component mapping applied to the sampled texture's texels as they are uploaded: what a view
    /// with that mapping reads, for comparing `VariantKey::swizzled` against.
    pub texel_map: Option<[wcontainers_gpu::ViewComponent; 4]>,
    /// Drawn as a pipeline without a pixel shader: the pixel program is the built-in one for
    /// `ps_key` (`builtin::pixel_program_for_variant`), writing no colour.
    pub no_pixel_shader: bool,
    /// Texels of a copy of target 0 before the draw, rows tightly packed, bound at
    /// [`wcontainers_gpu::TARGET_COPY_BINDING`] for a pixel program combining with it.
    pub target_copy: Option<Vec<u8>>,
}

/// `texels` (Rgba8Unorm) with each texel's components mapped by `map`.
pub fn mapped(texels: &[u8], map: [wcontainers_gpu::ViewComponent; 4]) -> Vec<u8> {
    use wcontainers_gpu::ViewComponent as C;
    texels
        .chunks(4)
        .flat_map(|t| {
            map.map(|c| match c {
                C::Zero => 0,
                C::One => 255,
                c => t[c as usize],
            })
        })
        .collect()
}

impl Setup {
    pub fn plain(size: u32) -> Setup {
        Setup {
            vs_key: VariantKey::default(),
            ps_key: VariantKey::default(),
            helpers: [Vec::new(), Vec::new()],
            viewport: [0.0, 0.0, size as f32, size as f32, 0.0, 1.0],
            samples: 1,
            sample_mask: !0,
            depth: false,
            depth_minus_one: false,
            blend: None,
            blend_constant: [0.0; 4],
            levels: 1,
            bases: None,
            min_lod: None,
            address: [wcontainers_gpu::AddressMode::Clamp; 3],
            texel_buffers: Vec::new(),
            layers: 0,
            texel_map: None,
            no_pixel_shader: false,
            target_copy: None,
        }
    }
}

/// The sample count of [`multisampled_layers`].
pub const LAYER_SAMPLES: u32 = 4;

/// Real Direct3D 11's constants for `ps_fill` drawing layer `layer`.
pub fn fill_constants(layer: u32) -> Vec<u8> {
    constants([(layer + 1) as f32 / 4.0, 0.0, 0.0, 0.0])
}

/// `layers` Rgba8Unorm textures of [`TEXTURE_SIZE`] and [`LAYER_SAMPLES`] samples, each drawn by
/// `ps_fill` (a value per sample) with [`fill_constants`] of its layer, as a front holds a
/// multisampled array texture.
pub fn multisampled_layers<B: Backend>(
    b: &mut B,
    layers: u32,
) -> Vec<(B::Texture, B::TextureView)> {
    if layers == 0 {
        return Vec::new();
    }
    let vs = convert(
        Source::Dxbc(&bytecode("vs_fullscreen.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let ps = convert(
        Source::Dxbc(&bytecode("ps_fill.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let vertex_layout = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: [None; 32],
            attributes: [None; 32],
        })
        .unwrap();
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
            provoking: ProvokingVertex::First,
            depth_minus_one: false,
            sample_mask: !0,
        })
        .unwrap();
    let size = TEXTURE_SIZE as f32;
    let mut out = Vec::new();
    for layer in 0..layers {
        let made = texture_levels(
            b,
            Format::Rgba8Unorm,
            TEXTURE_SIZE,
            1,
            LAYER_SAMPLES,
            TextureUsage::RENDER | TextureUsage::SAMPLE,
        );
        let bytes = fill_constants(layer);
        let constants = buffer(b, &bytes, BufferUsage::UNIFORM);
        let entries: Vec<BindingEntry<'_, B>> = ps
            .reflection
            .bindings
            .iter()
            .map(|r| {
                assert_eq!(r.kind, SlotKind::Uniform, "{r:?}");
                BindingEntry {
                    binding: r.binding,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &constants,
                        offset: 0,
                        size: bytes.len() as u64,
                    }),
                }
            })
            .collect();
        let set = b.create_binding_set(&entries).unwrap();
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &made.1,
            depth_slice: 0,
            resolve: None,
            load: wcontainers_gpu::LoadOp::Clear([0; 4]),
            store: StoreOp::Store,
        });
        b.begin_pass(&pass);
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &vertex_layout,
            bindings: [None, Some(&set), None, None],
            vertex_buffers: &[],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &[Viewport {
                x: 0,
                y: 0,
                width: size.to_bits(),
                height: size.to_bits(),
                min_depth: 0,
                max_depth: 1.0f32.to_bits(),
            }],
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 3,
                first_instance: 0,
                instances: 1,
            },
        });
        b.end_pass();
        out.push(made);
    }
    out
}

/// The colour of texture level n past 0.
pub fn level_colour(n: u32) -> [u8; 4] {
    [
        (n * 70 % 256) as u8,
        (255 - n * 60) as u8,
        (n * 110 % 256) as u8,
        255,
    ]
}

/// The texels of level `n` of the sampled texture.
pub fn level_texels(n: u32) -> Vec<u8> {
    if n == 0 {
        return texels();
    }
    let size = (TEXTURE_SIZE >> n).max(1);
    level_colour(n).repeat((size * size) as usize)
}

fn buffer_size(bases: &Option<Bases>) -> u64 {
    bases.map_or(0, |d| d.arguments().len() as u64)
}

/// A constant buffer's bytes in the texture `AddedBinding::ConstantsAsTexture` reads: one 16-byte row
/// per RGBA32 unsigned texel, `CONSTANTS_TEXTURE_WIDTH` rows to a texture row.
pub fn constants_texture<B: Backend>(b: &mut B, bytes: &[u8]) -> (B::Texture, B::TextureView) {
    let width = wcontainers_gpu::CONSTANTS_TEXTURE_WIDTH;
    let rows = bytes.len().div_ceil(16) as u32;
    let (w, h) = (rows.min(width), rows.div_ceil(width));
    let mut texels = bytes.to_vec();
    texels.resize((w * h * 16) as usize, 0);
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba32Uint,
            width: w,
            height: h,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    b.write_texture(
        &TextureRegion {
            texture: &t,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: w,
            height: h,
            depth_or_layers: 1,
        },
        &texels,
        w * 16,
        h,
        WriteMode::Ordered,
    );
    let v = b
        .create_view(
            &t,
            &TextureViewDesc {
                format: Format::Rgba32Uint,
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

/// Draws through a back end with the converted shaders, binding by their reflection (every slot is
/// register 0: the constants, the texture of [`texels`] and a linear clamping sampler); returns the
/// target's texels. `provoking` is the back end's own (it only decides `ps_integer`'s value).
pub fn ours<B: Backend + 'static>(
    b: &mut B,
    draw: &Draw<'_>,
    provoking: ProvokingVertex,
) -> Vec<u8> {
    // As a front links the stages: the vertex program writes each output as the pixel program reads it.
    let setup = &draw.setup;
    let ps = if setup.no_pixel_shader {
        wcontainers_shaders::builtin::pixel_program_for_variant(&setup.ps_key).unwrap()
    } else {
        convert(draw.ps, &setup.ps_key, None).unwrap()
    };
    let key = VariantKey {
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..setup.vs_key.clone()
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
    let (texture_object, view) = texture_levels(
        b,
        Format::Rgba8Unorm,
        TEXTURE_SIZE,
        setup.levels,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    for level in 0..setup.levels {
        let size = (TEXTURE_SIZE >> level).max(1);
        b.write_texture(
            &TextureRegion {
                texture: &texture_object,
                mip: level,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: size,
                height: size,
                depth_or_layers: 1,
            },
            &match setup.texel_map {
                Some(map) => mapped(&level_texels(level), map),
                None => level_texels(level),
            },
            size * 4,
            size,
            WriteMode::Ordered,
        );
    }
    let sampler_from = |b: &mut B, min_lod: f32| {
        b.create_sampler(&SamplerDesc {
            min: Filter::Linear,
            mag: Filter::Linear,
            mip: Filter::Linear,
            address: setup.address,
            lod_range: [min_lod.to_bits(), f32::MAX.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .unwrap()
    };
    let sampler = sampler_from(b, 0.0);
    let merged = setup.min_lod.map(|clamp| sampler_from(b, clamp));
    let layers = multisampled_layers(b, setup.layers);
    // The constants as a front copies them for programs that read them through a texture.
    let through_texture = vs
        .reflection
        .bindings
        .iter()
        .chain(&ps.reflection.bindings)
        .any(|r| matches!(r.added, Some(AddedBinding::ConstantsAsTexture(_))));
    let constants_texture = through_texture.then(|| constants_texture(b, &draw.constants));

    // Typed buffers by the pixel key's route: a storage buffer of the bytes, or a texture of the
    // view's format holding them, element e at texel (e % width, e / width).
    let route = setup.ps_key.texel_buffers;
    let texel_storage: Vec<_> = setup
        .texel_buffers
        .iter()
        .map(|t| buffer(b, &t.bytes, BufferUsage::STORAGE))
        .collect();
    let texel_textures: Vec<_> = setup
        .texel_buffers
        .iter()
        .filter(|_| route == TexelBufferRoute::Texture)
        .map(|t| {
            let size = t.format.block().2;
            let elements = t.bytes.len() as u32 / size;
            let width = elements.min(TEXEL_BUFFER_TEXTURE_WIDTH);
            let height = elements.div_ceil(TEXEL_BUFFER_TEXTURE_WIDTH);
            let mut texels = t.bytes.clone();
            texels.resize((width * height * size) as usize, 0);
            let texture = b
                .create_texture(&TextureDesc {
                    dimension: TextureDimension::D2,
                    format: t.format,
                    width,
                    height,
                    depth_or_layers: 1,
                    mip_levels: 1,
                    samples: 1,
                    usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
                    cube: false,
                    reinterpret: false,
                })
                .unwrap();
            let view = b
                .create_view(
                    &texture,
                    &TextureViewDesc {
                        format: t.format,
                        dimension: ViewDimension::D2,
                        base_mip: 0,
                        mip_count: 1,
                        base_layer: 0,
                        layer_count: 1,
                        stencil: false,
                    },
                )
                .unwrap();
            b.write_texture(
                &TextureRegion {
                    texture: &texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width,
                    height,
                    depth_or_layers: 1,
                },
                &texels,
                width * size,
                height,
                WriteMode::Ordered,
            );
            (texture, view)
        })
        .collect();
    let mut pixel_helpers = setup.helpers[1].clone();
    for h in &ps.reflection.helpers {
        let HelperValue::TexelBufferView { set, binding } = h.value else {
            continue;
        };
        let r = ps
            .reflection
            .bindings
            .iter()
            .find(|r| r.set == set && r.binding == binding)
            .unwrap();
        let t = &setup.texel_buffers[r.register as usize];
        pixel_helpers.resize(
            pixel_helpers.len().max(ps.reflection.helper_size as usize),
            0,
        );
        let at = h.offset as usize;
        let offset = t.first * t.format.block().2;
        pixel_helpers[at..at + 4].copy_from_slice(&offset.to_le_bytes());
        pixel_helpers[at + 4..at + 8].copy_from_slice(&t.count.to_le_bytes());
    }

    // Each stage's slots, from its reflection: vertex slots in set 0, pixel slots in set 1.
    let mut sets: [Vec<BindingEntry<'_, B>>; 2] = [Vec::new(), Vec::new()];
    for (reflection, stages) in [
        (&vs.reflection, StageSet::VERTEX),
        (&ps.reflection, StageSet::FRAGMENT),
    ] {
        for r in &reflection.bindings {
            let texel = r.kind == SlotKind::Texture && !setup.texel_buffers.is_empty();
            assert!(r.register == 0 || texel, "{r:?}");
            let i = r.register as usize;
            let resource = match r.kind {
                _ if texel && route == TexelBufferRoute::Storage => {
                    BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer: &texel_storage[i],
                            offset: 0,
                            size: setup.texel_buffers[i].bytes.len() as u64,
                        },
                        writable: false,
                    }
                }
                _ if texel => BindingResource::Texture(&texel_textures[i].1),
                SlotKind::Texture
                    if matches!(r.added, Some(AddedBinding::ConstantsAsTexture(_))) =>
                {
                    BindingResource::Texture(&constants_texture.as_ref().unwrap().1)
                }
                SlotKind::Texture if !layers.is_empty() => {
                    let layer = match r.added {
                        Some(AddedBinding::TextureLayer(_, n)) => n as usize,
                        _ => 0,
                    };
                    BindingResource::Texture(&layers[layer].1)
                }
                SlotKind::Uniform => BindingResource::UniformBuffer(BufferRange {
                    buffer: &constants,
                    offset: 0,
                    size: draw.constants.len() as u64,
                }),
                SlotKind::Storage => BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &constants,
                        offset: 0,
                        size: draw.constants.len() as u64,
                    },
                    writable: false,
                },
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
    // The helper blocks, in set 3.
    let mut helper_buffers = Vec::new();
    for (bytes, reflection, stage) in [
        (
            &setup.helpers[0],
            &vs.reflection,
            wcontainers_gpu::ShaderStage::Vertex,
        ),
        (
            &pixel_helpers,
            &ps.reflection,
            wcontainers_gpu::ShaderStage::Fragment,
        ),
    ] {
        assert!(
            bytes.len() >= reflection.helper_size as usize,
            "{:?}",
            reflection.helpers
        );
        if !bytes.is_empty() {
            helper_buffers.push((buffer(b, bytes, BufferUsage::UNIFORM), bytes.len(), stage));
        }
    }
    let arguments = setup.bases.filter(|d| d.indirect).map(|d| {
        buffer(
            b,
            &d.arguments(),
            BufferUsage::INDIRECT | BufferUsage::STORAGE,
        )
    });
    let reads_arguments = vs
        .reflection
        .helpers
        .iter()
        .any(|h| h.value == wcontainers_gpu::HelperValue::DrawArguments);
    let mut helper_entries: Vec<BindingEntry<'_, B>> = helper_buffers
        .iter()
        .map(|(buffer, len, stage)| BindingEntry {
            binding: wcontainers_gpu::helper_binding(*stage),
            stages: match stage {
                wcontainers_gpu::ShaderStage::Vertex => StageSet::VERTEX,
                _ => StageSet::FRAGMENT,
            },
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer,
                offset: 0,
                size: *len as u64,
            }),
        })
        .collect();
    if let Some(merged) = &merged {
        for n in 0..ps.reflection.sampled_pairs.len() as u32 {
            helper_entries.push(BindingEntry {
                binding: wcontainers_gpu::CLAMPED_SAMPLER_BINDING + n,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(merged),
            });
        }
    }
    if reads_arguments {
        let buffer = arguments.as_ref().expect("an indirect draw");
        helper_entries.push(BindingEntry {
            binding: wcontainers_gpu::DRAW_ARGUMENTS_BINDING,
            stages: StageSet::VERTEX,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer,
                    offset: 0,
                    size: buffer_size(&setup.bases),
                },
                writable: false,
            },
        });
    }
    let copy = setup.target_copy.as_ref().map(|texels| {
        let (texture, view) = texture(
            b,
            draw.format,
            draw.size,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        );
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: draw.size,
                height: draw.size,
                depth_or_layers: 1,
            },
            texels,
            texels.len() as u32 / draw.size,
            draw.size,
            WriteMode::Ordered,
        );
        (texture, view)
    });
    if let Some((_, view)) = &copy {
        helper_entries.push(BindingEntry {
            binding: wcontainers_gpu::TARGET_COPY_BINDING,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(view),
        });
    }
    let helper_set =
        (!helper_entries.is_empty()).then(|| b.create_binding_set(&helper_entries).unwrap());

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
                blend: setup.blend,
                writes: if setup.no_pixel_shader {
                    ColorWrites::empty()
                } else {
                    ColorWrites::all()
                },
            }; 8],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: setup.depth.then_some(wcontainers_gpu::CompareFn::Always),
            depth_write: setup.depth,
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
            depth_minus_one: setup.depth_minus_one,
            sample_mask: setup.sample_mask,
        })
        .unwrap();

    let samples = setup.samples;
    let (target, target_view) = texture_levels(
        b,
        draw.format,
        draw.size,
        1,
        samples,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let resolved = (samples > 1).then(|| {
        texture(
            b,
            draw.format,
            draw.size,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        )
    });
    let depth = setup.depth.then(|| {
        texture_levels(
            b,
            Format::Depth32Float,
            draw.size,
            1,
            samples,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        )
    });
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: depth
            .as_ref()
            .map(|(_, view)| wcontainers_gpu::DepthAttachment {
                view,
                depth: Some(wcontainers_gpu::AspectOps {
                    load: wcontainers_gpu::LoadOp::Clear(1.0f32.to_bits()),
                    store: StoreOp::Store,
                    read_only: false,
                }),
                stencil: None,
            }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: resolved.as_ref().map(|(_, view)| view),
        load: wcontainers_gpu::LoadOp::Clear(draw.clear),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let streams: Vec<_> = vertex_buffer
        .iter()
        .map(|buffer| Some(VertexBinding { buffer, offset: 0 }))
        .collect();
    let indices = setup
        .bases
        .filter(|d| d.indexed)
        .map(|d| buffer(b, &d.indices(), BufferUsage::INDEX));
    let at = (ARGUMENTS_WORD * 4) as u64;
    let range = match (setup.bases, &arguments) {
        (None, _) => DrawRange::Vertices {
            first: 0,
            count: draw.count,
            first_instance: 0,
            instances: 1,
        },
        (Some(d), None) if d.indexed => DrawRange::Indexed {
            first: d.first,
            count: d.count,
            base_vertex: d.base_vertex,
            first_instance: d.first_instance,
            instances: d.instances,
        },
        (Some(d), None) => DrawRange::Vertices {
            first: d.first,
            count: d.count,
            first_instance: d.first_instance,
            instances: d.instances,
        },
        (Some(d), Some(args)) if d.indexed => DrawRange::IndexedIndirect { args, offset: at },
        (Some(_), Some(args)) => DrawRange::Indirect { args, offset: at },
    };
    let [x, y, width, height, min_depth, max_depth] = setup.viewport.map(f32::to_bits);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [
            sets[0].as_ref(),
            sets[1].as_ref(),
            None,
            helper_set.as_ref(),
        ],
        vertex_buffers: &streams,
        index: indices
            .as_ref()
            .map(|buffer| wcontainers_gpu::IndexBinding {
                buffer,
                offset: 0,
                format: wcontainers_gpu::IndexFormat::U32,
            }),
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x,
            y,
            width,
            height,
            min_depth,
            max_depth,
        }],
        scissors: &[],
        blend_constant: setup.blend_constant.map(f32::to_bits),
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    });
    b.end_pass();
    let (target, format) = match (&depth, &resolved) {
        (Some((depth, _)), _) => (depth, Format::Depth32Float),
        (None, Some((resolved, _))) => (resolved, draw.format),
        (None, None) => (&target, draw.format),
    };
    let mut out = vec![0; (draw.size * draw.size * format.block().2) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: target,
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
    assert_eq!(
        b.status(),
        wcontainers_gpu::DeviceStatus::Ok,
        "{}vertex {:?}, pixel {:?}, {} samples, depth {}, viewport {:?}, format {:?}",
        (b as &dyn std::any::Any)
            .downcast_ref::<wcontainers_gpu_webgl::WebGl>()
            .map(|gl| format!("{}: ", gl.last_log()))
            .unwrap_or_default(),
        setup.vs_key.flags,
        setup.ps_key,
        setup.samples,
        setup.depth,
        setup.viewport,
        draw.format
    );
    out
}

/// How many texels differ, and the largest difference of any channel (8-bit channels, or 32-bit
/// integers).
pub fn differ(a: &[u8], b: &[u8], format: Format) -> (usize, u64) {
    use Format as F;
    let (channel, texel) = match format {
        Format::Rgba32Uint => (4, 16),
        // Positive f32 bits count steps apart.
        Format::Depth32Float => (4, 4),
        // Narrow integer formats (normalised ones held as integers) by their own channels.
        F::R16Uint
        | F::Rg16Uint
        | F::Rgba16Uint
        | F::R16Sint
        | F::Rg16Sint
        | F::Rgba16Sint
        | F::R8Sint
        | F::Rg8Sint
        | F::Rgba8Sint => {
            let texel = format.block().2 as usize;
            (texel / format.components().count_ones() as usize, texel)
        }
        _ => (1, 4),
    };
    let signed = format.kind() == wcontainers_gpu::FormatKind::Sint && channel < 4;
    let mut count = 0;
    let mut largest = 0;
    for (x, y) in a.chunks(texel).zip(b.chunks(texel)) {
        let d = x
            .chunks(channel)
            .zip(y.chunks(channel))
            .map(|(p, q)| {
                let v = |c: &[u8]| {
                    let raw = c.iter().rev().fold(0u64, |n, b| n << 8 | *b as u64);
                    let unused = u64::BITS - u8::BITS * c.len() as u32;
                    match signed {
                        true => ((raw << unused) as i64 >> unused) as i128,
                        false => raw as i128,
                    }
                };
                v(p).abs_diff(v(q)) as u64
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

pub fn webgpu() -> wcontainers_gpu_webgpu::WebGpu {
    webgpu_on(wgpu::Backends::DX12)
}

/// The WebGPU back end on wgpu's `backends`: Vulkan for what wgpu's Direct3D 12 path lacks (clip
/// distances), as Chrome's WebGPU has it on Direct3D 12.
pub fn webgpu_on(backends: wgpu::Backends) -> wcontainers_gpu_webgpu::WebGpu {
    webgpu_with(backends, wgpu::Features::empty())
}

/// The WebGPU back end on wgpu's `backends`, its device made with `more` wgpu features than the back
/// end asks for (those a test needs before the back end asks for them).
pub fn webgpu_with(
    backends: wgpu::Backends,
    more: wgpu::Features,
) -> wcontainers_gpu_webgpu::WebGpu {
    webgpu_adapter(backends, more, false)
}

/// The WebGPU back end on wgpu's Direct3D 12 path on WARP, Microsoft's software rasteriser, to draw
/// against Direct3D on WARP where the device chooses (the order of a patch's primitives).
pub fn webgpu_on_warp() -> wcontainers_gpu_webgpu::WebGpu {
    webgpu_adapter(wgpu::Backends::DX12, wgpu::Features::empty(), true)
}

/// The WebGPU back end on wgpu's `backends` with `more` features: on the GPU, or with `software` on
/// the system's software adapter.
fn webgpu_adapter(
    backends: wgpu::Backends,
    more: wgpu::Features,
    software: bool,
) -> wcontainers_gpu_webgpu::WebGpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: software,
        ..Default::default()
    }))
    .expect("an adapter");
    let info = adapter.get_info();
    assert_eq!(
        info.device_type == wgpu::DeviceType::Cpu,
        software,
        "{}",
        info.name
    );
    eprintln!("webgpu: {} ({:?})", info.name, info.backend);
    let mut device_desc = wcontainers_gpu_webgpu::WebGpu::device_descriptor(&adapter);
    assert!(adapter.features().contains(more), "{more:?}");
    device_desc.required_features |= more;
    let (device, queue) = pollster::block_on(adapter.request_device(&device_desc)).unwrap();
    wcontainers_gpu_webgpu::WebGpu::new(&adapter, device, queue)
}

pub fn webgl() -> wcontainers_gpu_webgl::WebGl {
    webgl_without(&[])
}

/// The WebGL 2 back end made with the WebGL extensions `off` switched off.
pub fn webgl_without(off: &[&str]) -> wcontainers_gpu_webgl::WebGl {
    let dir = std::env::var("WCONTAINERS_ANGLE")
        .unwrap_or_else(|_| r"C:\wc\tools\angle-mozangle-0.7.1".into());
    let gl = wcontainers_gpu_webgl::WebGl::on_angle(Path::new(&dir), off).unwrap();
    eprintln!("webgl: {}", gl.renderer());
    gl
}

/// Saves a picture for looking at when `WCONTAINERS_SHADERS_OUT` names a directory.
pub fn keep(name: &str, texels: &[u8], format: Format) {
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
