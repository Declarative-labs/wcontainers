//! The Direct3D 11 scenes: a device, the helpers the scenes share, and the list of scenes. Each scene
//! draws into offscreen targets and reads them back as outputs.

pub mod scenes;

use anyhow::{Context, Result};
use image::DynamicImage;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIAdapter1, IDXGIDevice};
use windows::core::{Interface, PCSTR};

use crate::{Driver, debug, texels};

/// A device and its immediate context, with what made them.
pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub driver: Driver,
    pub debug: bool,
    pub level: D3D_FEATURE_LEVEL,
}

/// Creates a device on `driver` asking for `levels`, with the debug layer when `debug`.
pub fn create(driver: Driver, debug: bool, levels: &[D3D_FEATURE_LEVEL]) -> Result<Gpu> {
    let driver_type = match driver {
        Driver::Hardware => D3D_DRIVER_TYPE_HARDWARE,
        Driver::Warp => D3D_DRIVER_TYPE_WARP,
    };
    let flags = if debug {
        D3D11_CREATE_DEVICE_DEBUG
    } else {
        D3D11_CREATE_DEVICE_FLAG(0)
    };
    let (mut device, mut context, mut level) = (None, None, D3D_FEATURE_LEVEL::default());
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver_type,
            HMODULE::default(),
            flags,
            Some(levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut level),
            Some(&mut context),
        )
    }
    .context("D3D11CreateDevice (the debug layer needs the Graphics Tools feature)")?;
    let gpu = Gpu {
        device: device.context("no device")?,
        context: context.context("no context")?,
        driver,
        debug,
        level,
    };
    gpu.adapter()?;
    Ok(gpu)
}

/// Makes an object through a call that writes it out.
pub fn made<T>(f: impl FnOnce(*mut Option<T>) -> windows::core::Result<()>) -> Result<T> {
    let mut out = None;
    f(&mut out)?;
    out.context("the call made no object")
}

/// Compiled DXBC of an entry point of shaders/<file>.hlsl, named "<file>.<entry>".
#[macro_export]
macro_rules! dxbc {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/dxbc/", $name, ".dxbc")).as_slice()
    };
}

/// An input element of slot `slot`, per vertex, or per instance with `step` > 0.
pub fn element(
    name: PCSTR,
    index: u32,
    format: DXGI_FORMAT,
    slot: u32,
    offset: u32,
    step: u32,
) -> D3D11_INPUT_ELEMENT_DESC {
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: index,
        Format: format,
        InputSlot: slot,
        AlignedByteOffset: offset,
        InputSlotClass: if step > 0 {
            D3D11_INPUT_PER_INSTANCE_DATA
        } else {
            D3D11_INPUT_PER_VERTEX_DATA
        },
        InstanceDataStepRate: step,
    }
}

/// Solid filling, no culling, depth clipping on.
pub fn solid() -> D3D11_RASTERIZER_DESC {
    D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        DepthClipEnable: true.into(),
        ..Default::default()
    }
}

/// A 2D texture description with one mip, one layer and one sample.
pub fn tex2d(
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    bind: D3D11_BIND_FLAG,
) -> D3D11_TEXTURE2D_DESC {
    D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: bind.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    }
}

/// Initial data of a subresource with rows `pitch` bytes apart and slices `slice` bytes apart.
pub fn data(bytes: &[u8], pitch: u32, slice: u32) -> D3D11_SUBRESOURCE_DATA {
    D3D11_SUBRESOURCE_DATA {
        pSysMem: bytes.as_ptr().cast(),
        SysMemPitch: pitch,
        SysMemSlicePitch: slice,
    }
}

/// Viewport of cell `i` of a grid of `columns` cells `size` pixels square.
pub fn cell(i: u32, columns: u32, size: f32) -> D3D11_VIEWPORT {
    D3D11_VIEWPORT {
        TopLeftX: (i % columns) as f32 * size,
        TopLeftY: (i / columns) as f32 * size,
        Width: size,
        Height: size,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }
}

/// The colour a picture is cleared to: each channel an exact 8-bit value.
pub const CLEAR: [f32; 4] = crate::scene::CLEAR;

impl Gpu {
    /// The adapter's name; fails when a hardware device landed on a software adapter.
    pub fn adapter(&self) -> Result<String> {
        let desc = unsafe {
            self.device
                .cast::<IDXGIDevice>()?
                .GetAdapter()?
                .cast::<IDXGIAdapter1>()?
                .GetDesc1()?
        };
        crate::adapter_name(self.driver, &desc)
    }

    /// Fails on any error or corruption the debug layer reported, printing all its messages.
    pub fn check_debug(&self) -> Result<()> {
        if !self.debug {
            return Ok(());
        }
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
            queue.ClearStoredMessages();
        }
        debug::check(messages)
    }

    pub fn buffer(&self, desc: &D3D11_BUFFER_DESC, init: Option<&[u8]>) -> Result<ID3D11Buffer> {
        let init = init.map(|b| data(b, 0, 0));
        made(|o| unsafe {
            self.device
                .CreateBuffer(desc, init.as_ref().map(|d| d as *const _), Some(o))
        })
    }

    /// An immutable buffer holding `bytes`.
    pub fn immutable(&self, bytes: &[u8], bind: D3D11_BIND_FLAG) -> Result<ID3D11Buffer> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: bytes.len() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: bind.0 as u32,
            ..Default::default()
        };
        self.buffer(&desc, Some(bytes))
    }

    pub fn texture2d(
        &self,
        desc: &D3D11_TEXTURE2D_DESC,
        init: &[D3D11_SUBRESOURCE_DATA],
    ) -> Result<ID3D11Texture2D> {
        let init = (!init.is_empty()).then_some(init.as_ptr());
        made(|o| unsafe { self.device.CreateTexture2D(desc, init, Some(o)) })
    }

    pub fn rtv(
        &self,
        resource: &ID3D11Resource,
        desc: Option<&D3D11_RENDER_TARGET_VIEW_DESC>,
    ) -> Result<ID3D11RenderTargetView> {
        made(|o| unsafe {
            self.device
                .CreateRenderTargetView(resource, desc.map(|d| d as *const _), Some(o))
        })
    }

    pub fn dsv(
        &self,
        resource: &ID3D11Resource,
        desc: Option<&D3D11_DEPTH_STENCIL_VIEW_DESC>,
    ) -> Result<ID3D11DepthStencilView> {
        made(|o| unsafe {
            self.device
                .CreateDepthStencilView(resource, desc.map(|d| d as *const _), Some(o))
        })
    }

    pub fn srv(
        &self,
        resource: &ID3D11Resource,
        desc: Option<&D3D11_SHADER_RESOURCE_VIEW_DESC>,
    ) -> Result<ID3D11ShaderResourceView> {
        made(|o| unsafe {
            self.device
                .CreateShaderResourceView(resource, desc.map(|d| d as *const _), Some(o))
        })
    }

    pub fn uav(
        &self,
        resource: &ID3D11Resource,
        desc: Option<&D3D11_UNORDERED_ACCESS_VIEW_DESC>,
    ) -> Result<ID3D11UnorderedAccessView> {
        made(|o| unsafe {
            self.device
                .CreateUnorderedAccessView(resource, desc.map(|d| d as *const _), Some(o))
        })
    }

    /// A render target of `format` and its view.
    pub fn target(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<(ID3D11Texture2D, ID3D11RenderTargetView)> {
        let texture =
            self.texture2d(&tex2d(width, height, format, D3D11_BIND_RENDER_TARGET), &[])?;
        let view = self.rtv(&texture.cast()?, None)?;
        Ok((texture, view))
    }

    /// A depth buffer of `format` and its view.
    pub fn depth(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<(ID3D11Texture2D, ID3D11DepthStencilView)> {
        let texture =
            self.texture2d(&tex2d(width, height, format, D3D11_BIND_DEPTH_STENCIL), &[])?;
        let view = self.dsv(&texture.cast()?, None)?;
        Ok((texture, view))
    }

    pub fn blend(&self, desc: &D3D11_BLEND_DESC) -> Result<ID3D11BlendState> {
        made(|o| unsafe { self.device.CreateBlendState(desc, Some(o)) })
    }

    pub fn depth_stencil(
        &self,
        desc: &D3D11_DEPTH_STENCIL_DESC,
    ) -> Result<ID3D11DepthStencilState> {
        made(|o| unsafe { self.device.CreateDepthStencilState(desc, Some(o)) })
    }

    pub fn rasterizer(&self, desc: &D3D11_RASTERIZER_DESC) -> Result<ID3D11RasterizerState> {
        made(|o| unsafe { self.device.CreateRasterizerState(desc, Some(o)) })
    }

    pub fn sampler(&self, desc: &D3D11_SAMPLER_DESC) -> Result<ID3D11SamplerState> {
        made(|o| unsafe { self.device.CreateSamplerState(desc, Some(o)) })
    }

    pub fn vs(&self, code: &[u8]) -> Result<ID3D11VertexShader> {
        made(|o| unsafe {
            self.device
                .CreateVertexShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn ps(&self, code: &[u8]) -> Result<ID3D11PixelShader> {
        made(|o| unsafe {
            self.device
                .CreatePixelShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn gs(&self, code: &[u8]) -> Result<ID3D11GeometryShader> {
        made(|o| unsafe {
            self.device
                .CreateGeometryShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn hs(&self, code: &[u8]) -> Result<ID3D11HullShader> {
        made(|o| unsafe {
            self.device
                .CreateHullShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn ds(&self, code: &[u8]) -> Result<ID3D11DomainShader> {
        made(|o| unsafe {
            self.device
                .CreateDomainShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn cs(&self, code: &[u8]) -> Result<ID3D11ComputeShader> {
        made(|o| unsafe {
            self.device
                .CreateComputeShader(code, None::<&ID3D11ClassLinkage>, Some(o))
        })
    }

    pub fn layout(
        &self,
        elements: &[D3D11_INPUT_ELEMENT_DESC],
        vs: &[u8],
    ) -> Result<ID3D11InputLayout> {
        made(|o| unsafe { self.device.CreateInputLayout(elements, vs, Some(o)) })
    }

    /// Binds a vertex and a pixel shader, and the input layout when `elements` is not empty.
    pub fn program(
        &self,
        vs: &[u8],
        ps: &[u8],
        elements: &[D3D11_INPUT_ELEMENT_DESC],
    ) -> Result<()> {
        let (v, p) = (self.vs(vs)?, self.ps(ps)?);
        let layout = if elements.is_empty() {
            None
        } else {
            Some(self.layout(elements, vs)?)
        };
        unsafe {
            self.context.VSSetShader(&v, None);
            self.context.PSSetShader(&p, None);
            self.context.IASetInputLayout(layout.as_ref());
        }
        Ok(())
    }

    /// Binds one vertex buffer holding `bytes` at slot `slot`.
    pub fn vertices(&self, slot: u32, bytes: &[u8], stride: u32) -> Result<ID3D11Buffer> {
        let buffer = self.immutable(bytes, D3D11_BIND_VERTEX_BUFFER)?;
        unsafe {
            self.context.IASetVertexBuffers(
                slot,
                1,
                Some(&Some(buffer.clone())),
                Some(&stride),
                Some(&0),
            )
        };
        Ok(buffer)
    }

    /// Sets the viewport to the whole of a `width` by `height` target.
    pub fn viewport(&self, width: u32, height: u32) {
        unsafe {
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: width as f32,
                Height: height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]))
        };
    }

    /// Reads back mip `mip` of layer `layer` of a 2D texture as a picture.
    pub fn read(&self, texture: &ID3D11Texture2D, mip: u32, layer: u32) -> Result<DynamicImage> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };
        let (width, height) = ((desc.Width >> mip).max(1), (desc.Height >> mip).max(1));
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let staging = self.texture2d(&staging_desc, &[])?;
        unsafe {
            self.context.CopySubresourceRegion(
                &staging,
                0,
                0,
                0,
                0,
                texture,
                mip + layer * desc.MipLevels,
                None,
            );
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let pitch = mapped.RowPitch as usize;
            let rows = std::slice::from_raw_parts(
                mapped.pData.cast::<u8>(),
                pitch * (height as usize - 1) + width as usize * texels::texel_size(desc.Format)?,
            );
            let picture = texels::picture(desc.Format, width, height, rows, pitch);
            self.context.Unmap(&staging, 0);
            picture
        }
    }

    /// Reads back the bytes of a buffer.
    pub fn read_buffer(&self, buffer: &ID3D11Buffer) -> Result<Vec<u8>> {
        let mut desc = D3D11_BUFFER_DESC::default();
        unsafe { buffer.GetDesc(&mut desc) };
        let staging = self.buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: desc.ByteWidth,
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..Default::default()
            },
            None,
        )?;
        unsafe {
            self.context.CopyResource(&staging, buffer);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let bytes =
                std::slice::from_raw_parts(mapped.pData.cast::<u8>(), desc.ByteWidth as usize)
                    .to_vec();
            self.context.Unmap(&staging, 0);
            Ok(bytes)
        }
    }

    /// Clears a target and a depth buffer and binds them with a full viewport.
    pub fn begin(
        &self,
        rtv: &ID3D11RenderTargetView,
        dsv: Option<&ID3D11DepthStencilView>,
        width: u32,
        height: u32,
    ) {
        unsafe {
            self.context.ClearRenderTargetView(rtv, &CLEAR);
            if let Some(dsv) = dsv {
                self.context.ClearDepthStencilView(
                    dsv,
                    D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
                    1.0,
                    0,
                );
            }
            self.context
                .OMSetRenderTargets(Some(&[Some(rtv.clone())]), dsv);
        }
        self.viewport(width, height);
    }
}

/// A seeded generator for a scene's data: the same bytes on every run.
pub fn seeded(seed: u64) -> rand_chacha::ChaCha8Rng {
    rand::SeedableRng::seed_from_u64(seed)
}

/// Texels of a `width` by `height` RGBA8 image: a checker of squares `width / 4` texels wide in
/// colour `index` of `count` and a dark tone, so mips, layers and faces tell apart.
pub fn checker(width: u32, height: u32, index: u32, count: u32) -> Vec<u8> {
    let [r, g, b] = crate::d3d11::scenes::hue(index, count);
    let light = [r, g, b, 1.0].map(|c| (c * 255.0) as u8);
    let dark = [r * 0.3, g * 0.3, b * 0.3, 1.0].map(|c| (c * 255.0) as u8);
    let square = (width / 4).max(1);
    let mut texels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let on = (x / square + y / square).is_multiple_of(2);
            texels.extend_from_slice(if on { &light } else { &dark });
        }
    }
    texels
}

/// The bytes as lines of 32 hexadecimal bytes.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .chunks(32)
        .map(|line| line.iter().map(|b| format!("{b:02x}")).collect::<String>() + "\n")
        .collect()
}
