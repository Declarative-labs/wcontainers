//! The scenes drawn with Direct3D 11 into an offscreen target, with the debug layer checking every call.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
use windows::core::{Interface, PCSTR, s};

use crate::scene::{self, HEIGHT, Scene, WIDTH};
use crate::{Driver, Picture, debug, hlsl, picture_from_rows};

pub fn render(scene: Scene, driver: Driver) -> Result<Picture> {
    let driver_type = match driver {
        Driver::Hardware => D3D_DRIVER_TYPE_HARDWARE,
        Driver::Warp => D3D_DRIVER_TYPE_WARP,
    };
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver_type,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_DEBUG,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .context("D3D11CreateDevice (the debug layer needs the Graphics Tools feature)")?;
    let (device, context): (ID3D11Device, ID3D11DeviceContext) =
        (device.unwrap(), context.unwrap());
    let adapter = unsafe { device.cast::<IDXGIDevice>()?.GetAdapter()?.GetDesc()? };
    let adapter = String::from_utf16_lossy(&adapter.Description)
        .trim_end_matches('\0')
        .to_owned();

    let gpu = Gpu { device, context };
    let image = unsafe { gpu.draw(scene)? };
    debug::check(unsafe { gpu.debug_messages()? })?;
    Ok(Picture { adapter, image })
}

struct Gpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

impl Gpu {
    unsafe fn draw(&self, scene: Scene) -> Result<image::RgbaImage> {
        let (device, context) = (&self.device, &self.context);
        unsafe {
            let target = self.texture(
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D11_BIND_RENDER_TARGET,
                D3D11_USAGE_DEFAULT,
                0,
            )?;
            let depth = self.texture(
                DXGI_FORMAT_D32_FLOAT,
                D3D11_BIND_DEPTH_STENCIL,
                D3D11_USAGE_DEFAULT,
                0,
            )?;
            let mut rtv = None;
            device.CreateRenderTargetView(&target, None, Some(&mut rtv))?;
            let mut dsv = None;
            device.CreateDepthStencilView(&depth, None, Some(&mut dsv))?;
            let (rtv, dsv) = (rtv.unwrap(), dsv.unwrap());

            let rasterizer_desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            };
            let mut rasterizer = None;
            device.CreateRasterizerState(&rasterizer_desc, Some(&mut rasterizer))?;

            context.ClearRenderTargetView(&rtv, &scene::CLEAR);
            context.ClearDepthStencilView(&dsv, D3D11_CLEAR_DEPTH.0, 1.0, 0);
            context.OMSetRenderTargets(Some(&[Some(rtv)]), &dsv);
            context.RSSetState(rasterizer.as_ref());
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: WIDTH as f32,
                Height: HEIGHT as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            match scene {
                Scene::Triangle => {
                    self.program(
                        scene::TRIANGLE_HLSL,
                        &[
                            element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0),
                            element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 8),
                        ],
                    )?;
                    self.vertices(bytemuck::cast_slice(&scene::TRIANGLE), 20)?;
                    context.Draw(3, 0);
                }
                Scene::Quads => {
                    self.program(
                        scene::QUADS_HLSL,
                        &[
                            element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
                            element(s!("TEXCOORD"), DXGI_FORMAT_R32G32_FLOAT, 12),
                            element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 20),
                        ],
                    )?;
                    self.vertices(bytemuck::cast_slice(&scene::QUADS), 36)?;
                    let indices = self.buffer(
                        bytemuck::cast_slice(&scene::QUAD_INDICES),
                        D3D11_BIND_INDEX_BUFFER,
                    )?;
                    context.IASetIndexBuffer(&indices, DXGI_FORMAT_R16_UINT, 0);

                    let texels = scene::texture();
                    let texture = self.texture_sized(
                        scene::TEXTURE_SIZE,
                        scene::TEXTURE_SIZE,
                        DXGI_FORMAT_R8G8B8A8_UNORM,
                        D3D11_BIND_SHADER_RESOURCE,
                        D3D11_USAGE_IMMUTABLE,
                        0,
                        Some(&D3D11_SUBRESOURCE_DATA {
                            pSysMem: texels.as_ptr().cast(),
                            SysMemPitch: scene::TEXTURE_SIZE * 4,
                            SysMemSlicePitch: 0,
                        }),
                    )?;
                    let mut view = None;
                    device.CreateShaderResourceView(&texture, None, Some(&mut view))?;
                    context.PSSetShaderResources(0, Some(&[view]));

                    let sampler_desc = D3D11_SAMPLER_DESC {
                        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                        AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                        AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                        AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                        MaxAnisotropy: 1,
                        ComparisonFunc: D3D11_COMPARISON_NEVER,
                        MaxLOD: f32::MAX,
                        ..Default::default()
                    };
                    let mut sampler = None;
                    device.CreateSamplerState(&sampler_desc, Some(&mut sampler))?;
                    context.PSSetSamplers(0, Some(&[sampler]));

                    let mut blend_desc = D3D11_BLEND_DESC::default();
                    blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: true.into(),
                        SrcBlend: D3D11_BLEND_SRC_ALPHA,
                        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOp: D3D11_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D11_BLEND_ONE,
                        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOpAlpha: D3D11_BLEND_OP_ADD,
                        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    };
                    let mut blend = None;
                    device.CreateBlendState(&blend_desc, Some(&mut blend))?;
                    context.OMSetBlendState(blend.as_ref(), None, u32::MAX);

                    let depth_desc = D3D11_DEPTH_STENCIL_DESC {
                        DepthEnable: true.into(),
                        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                        DepthFunc: D3D11_COMPARISON_LESS,
                        ..Default::default()
                    };
                    let mut depth_state = None;
                    device.CreateDepthStencilState(&depth_desc, Some(&mut depth_state))?;
                    context.OMSetDepthStencilState(depth_state.as_ref(), 0);

                    context.DrawIndexed(scene::QUAD_INDICES.len() as u32, 0, 0);
                }
                Scene::Instanced => {
                    self.program(
                        scene::INSTANCED_HLSL,
                        &[element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0)],
                    )?;
                    self.vertices(bytemuck::cast_slice(&scene::INSTANCE_SHAPE), 8)?;
                    let constants = self.buffer(
                        bytemuck::cast_slice(&scene::instance_constants()),
                        D3D11_BIND_CONSTANT_BUFFER,
                    )?;
                    context.VSSetConstantBuffers(0, Some(&[Some(constants)]));
                    context.DrawInstanced(3, scene::INSTANCES, 0, 0);
                }
            }

            let staging = self.texture(
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D11_BIND_FLAG(0),
                D3D11_USAGE_STAGING,
                D3D11_CPU_ACCESS_READ.0 as u32,
            )?;
            context.CopyResource(&staging, &target);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let pitch = mapped.RowPitch as usize;
            let rows =
                std::slice::from_raw_parts(mapped.pData.cast::<u8>(), pitch * HEIGHT as usize);
            let image = picture_from_rows(WIDTH, HEIGHT, rows, pitch);
            context.Unmap(&staging, 0);
            Ok(image)
        }
    }

    /// A target-sized 2D texture.
    unsafe fn texture(
        &self,
        format: DXGI_FORMAT,
        bind: D3D11_BIND_FLAG,
        usage: D3D11_USAGE,
        cpu: u32,
    ) -> Result<ID3D11Texture2D> {
        unsafe { self.texture_sized(WIDTH, HEIGHT, format, bind, usage, cpu, None) }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn texture_sized(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        bind: D3D11_BIND_FLAG,
        usage: D3D11_USAGE,
        cpu: u32,
        data: Option<&D3D11_SUBRESOURCE_DATA>,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: cpu,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, data.map(|d| d as *const _), Some(&mut texture))?
        };
        Ok(texture.unwrap())
    }

    unsafe fn buffer(&self, data: &[u8], bind: D3D11_BIND_FLAG) -> Result<ID3D11Buffer> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: data.len() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: bind.0 as u32,
            ..Default::default()
        };
        let init = D3D11_SUBRESOURCE_DATA {
            pSysMem: data.as_ptr().cast(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut buffer = None;
        unsafe {
            self.device
                .CreateBuffer(&desc, Some(&init), Some(&mut buffer))?
        };
        Ok(buffer.unwrap())
    }

    unsafe fn vertices(&self, data: &[u8], stride: u32) -> Result<()> {
        let buffer = unsafe { self.buffer(data, D3D11_BIND_VERTEX_BUFFER)? };
        unsafe {
            self.context
                .IASetVertexBuffers(0, 1, Some(&Some(buffer)), Some(&stride), Some(&0))
        };
        Ok(())
    }

    /// Compiles and binds the vertex and pixel shaders `vs` and `ps` of `source`, with the input layout.
    unsafe fn program(&self, source: &str, layout: &[D3D11_INPUT_ELEMENT_DESC]) -> Result<()> {
        let vs_code = hlsl::compile(source, "vs", "vs_5_0")?;
        let ps_code = hlsl::compile(source, "ps", "ps_5_0")?;
        let (mut vs, mut ps, mut input) = (None, None, None);
        unsafe {
            self.device
                .CreateVertexShader(&vs_code, None::<&ID3D11ClassLinkage>, Some(&mut vs))?;
            self.device
                .CreatePixelShader(&ps_code, None::<&ID3D11ClassLinkage>, Some(&mut ps))?;
            self.device
                .CreateInputLayout(layout, &vs_code, Some(&mut input))?;
            self.context.VSSetShader(vs.as_ref(), None);
            self.context.PSSetShader(ps.as_ref(), None);
            self.context.IASetInputLayout(input.as_ref());
        }
        Ok(())
    }

    unsafe fn debug_messages(&self) -> Result<Vec<(i32, String)>> {
        let queue: ID3D11InfoQueue = self.device.cast()?;
        let mut messages = Vec::new();
        unsafe {
            for i in 0..queue.GetNumStoredMessages() {
                let mut length = 0;
                queue.GetMessage(i, None, &mut length)?;
                let mut storage = vec![0u64; length.div_ceil(8)];
                let message = storage.as_mut_ptr().cast::<D3D11_MESSAGE>();
                queue.GetMessage(i, Some(message), &mut length)?;
                let message = &*message;
                messages.push((
                    message.Severity.0,
                    debug::text(message.pDescription, message.DescriptionByteLength),
                ));
            }
        }
        Ok(messages)
    }
}

fn element(name: PCSTR, format: DXGI_FORMAT, offset: u32) -> D3D11_INPUT_ELEMENT_DESC {
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: 0,
        Format: format,
        InputSlot: 0,
        AlignedByteOffset: offset,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    }
}
