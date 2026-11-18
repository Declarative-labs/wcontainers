//! Direct3D 10's interfaces on Direct3D 11's objects, as Direct3D 11 offers them: the same object
//! described in Direct3D 10's structures. Their device is a Direct3D 10 device, which a device made
//! by Direct3D 11 is not, so they name none.

use windows::Win32::Graphics::Direct3D::D3D_SRV_DIMENSION_BUFFEREX;
use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::core::{Interface, OutRef};

use super::*;

impl Child {
    pub(crate) fn get_device10(&self, out: OutRef<ID3D10Device>) {
        let _ = out.write(None);
    }
}

/// Direct3D 10's misc flags of a resource made with Direct3D 11's: those Direct3D 10 has, at its
/// values.
fn misc10(misc: u32) -> u32 {
    let kept = (D3D11_RESOURCE_MISC_GENERATE_MIPS.0
        | D3D11_RESOURCE_MISC_SHARED.0
        | D3D11_RESOURCE_MISC_TEXTURECUBE.0) as u32;
    let mut m = misc & kept;
    if misc & D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32 != 0 {
        m |= D3D10_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32;
    }
    if misc & D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32 != 0 {
        m |= D3D10_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32;
    }
    m
}

/// Writes `value` through the program's pointer, if any.
fn put<T>(out: *mut T, value: T) {
    if let Some(out) = unsafe { out.as_mut() } {
        *out = value;
    }
}

impl Buffer {
    /// Structured buffers have no Direct3D 10 interface.
    pub(super) fn offers_d3d10(&self) -> bool {
        self.front.desc.misc_flags & D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32 == 0
    }

    pub(super) fn get_desc10(&self, out: *mut D3D10_BUFFER_DESC) {
        let d = &self.front.desc;
        put(
            out,
            D3D10_BUFFER_DESC {
                ByteWidth: d.byte_width,
                Usage: D3D10_USAGE(d.usage as i32),
                BindFlags: d.bind_flags,
                CPUAccessFlags: d.cpu_access_flags,
                MiscFlags: misc10(d.misc_flags),
            },
        );
    }

    pub(super) fn get_type10(&self, out: *mut D3D10_RESOURCE_DIMENSION) {
        put(out, D3D10_RESOURCE_DIMENSION_BUFFER);
    }
}

/// The Direct3D 10 side of a texture type.
macro_rules! texture10 {
    ($name:ident, $desc:ident, $dim:ident, |$d:ident| $make:expr) => {
        impl $name {
            /// Textures under a resource clamp have no Direct3D 10 interface.
            pub(super) fn offers_d3d10(&self) -> bool {
                self.front.desc.misc_flags & D3D11_RESOURCE_MISC_RESOURCE_CLAMP.0 as u32 == 0
            }

            pub(super) fn get_desc10(&self, out: *mut $desc) {
                let $d = &self.front.desc;
                put(out, $make);
            }

            pub(super) fn get_type10(&self, out: *mut D3D10_RESOURCE_DIMENSION) {
                put(out, $dim);
            }
        }
    };
}

texture10!(
    Texture1D,
    D3D10_TEXTURE1D_DESC,
    D3D10_RESOURCE_DIMENSION_TEXTURE1D,
    |d| D3D10_TEXTURE1D_DESC {
        Width: d.width,
        MipLevels: d.mip_levels,
        ArraySize: d.array_size,
        Format: DXGI_FORMAT(d.format as i32),
        Usage: D3D10_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: misc10(d.misc_flags),
    }
);
texture10!(
    Texture2D,
    D3D10_TEXTURE2D_DESC,
    D3D10_RESOURCE_DIMENSION_TEXTURE2D,
    |d| D3D10_TEXTURE2D_DESC {
        Width: d.width,
        Height: d.height,
        MipLevels: d.mip_levels,
        ArraySize: d.array_size,
        Format: DXGI_FORMAT(d.format as i32),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: d.sample_count,
            Quality: d.sample_quality,
        },
        Usage: D3D10_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: misc10(d.misc_flags),
    }
);
texture10!(
    Texture3D,
    D3D10_TEXTURE3D_DESC,
    D3D10_RESOURCE_DIMENSION_TEXTURE3D,
    |d| D3D10_TEXTURE3D_DESC {
        Width: d.width,
        Height: d.height,
        Depth: d.depth,
        MipLevels: d.mip_levels,
        Format: DXGI_FORMAT(d.format as i32),
        Usage: D3D10_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: misc10(d.misc_flags),
    }
);

/// The viewed resource's Direct3D 10 interface, where it has one.
fn resource10(r: ID3D11Resource, out: OutRef<ID3D10Resource>) {
    let _ = out.write(r.cast().ok());
}

impl RenderTargetView {
    pub(super) fn get_desc10(&self, out: *mut D3D10_RENDER_TARGET_VIEW_DESC) {
        let mut d = D3D11_RENDER_TARGET_VIEW_DESC::default();
        self.get_desc(&mut d);
        // Both name the format, dimension and the same members of each dimension.
        put(out, unsafe {
            std::mem::transmute::<D3D11_RENDER_TARGET_VIEW_DESC, D3D10_RENDER_TARGET_VIEW_DESC>(d)
        });
    }

    pub(super) fn get_resource10(&self, out: OutRef<ID3D10Resource>) {
        resource10(self.resource.public(), out);
    }
}

impl DepthStencilView {
    pub(super) fn get_desc10(&self, out: *mut D3D10_DEPTH_STENCIL_VIEW_DESC) {
        let mut d = D3D11_DEPTH_STENCIL_VIEW_DESC::default();
        self.get_desc(&mut d);
        let mut v = D3D10_DEPTH_STENCIL_VIEW_DESC {
            Format: d.Format,
            ViewDimension: D3D10_DSV_DIMENSION(d.ViewDimension.0),
            ..Default::default()
        };
        // Direct3D 11's adds read-only flags before the dimension's members, which are alike.
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&raw const d.Anonymous).cast::<u8>(),
                (&raw mut v.Anonymous).cast::<u8>(),
                size_of_val(&v.Anonymous),
            )
        };
        put(out, v);
    }

    pub(super) fn get_resource10(&self, out: OutRef<ID3D10Resource>) {
        resource10(self.resource.public(), out);
    }
}

impl ShaderResourceView {
    /// Views of raw buffers have no Direct3D 10 interface.
    pub(super) fn offers_d3d10(&self) -> bool {
        self.desc.ViewDimension != D3D_SRV_DIMENSION_BUFFEREX
    }

    pub(super) fn get_desc10(&self, out: *mut D3D10_SHADER_RESOURCE_VIEW_DESC) {
        // Direct3D 10's members of each dimension are the first of Direct3D 11's.
        let mut v = D3D10_SHADER_RESOURCE_VIEW_DESC {
            Format: self.desc.Format,
            ViewDimension: self.desc.ViewDimension,
            ..Default::default()
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&raw const self.desc.Anonymous).cast::<u8>(),
                (&raw mut v.Anonymous).cast::<u8>(),
                size_of_val(&v.Anonymous),
            )
        };
        put(out, v);
    }

    pub(super) fn get_desc110(&self, out: *mut D3D10_SHADER_RESOURCE_VIEW_DESC1) {
        let mut v = D3D10_SHADER_RESOURCE_VIEW_DESC1 {
            Format: self.desc.Format,
            ViewDimension: self.desc.ViewDimension,
            ..Default::default()
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&raw const self.desc.Anonymous).cast::<u8>(),
                (&raw mut v.Anonymous).cast::<u8>(),
                size_of_val(&v.Anonymous),
            )
        };
        put(out, v);
    }

    pub(super) fn get_resource10(&self, out: OutRef<ID3D10Resource>) {
        resource10(self.resource.public(), out);
    }
}

impl SamplerState {
    pub(super) fn get_desc10(&self, out: *mut D3D10_SAMPLER_DESC) {
        let mut d = D3D11_SAMPLER_DESC::default();
        self.get_desc(&mut d);
        // One layout, one numbering.
        put(out, unsafe {
            std::mem::transmute::<D3D11_SAMPLER_DESC, D3D10_SAMPLER_DESC>(d)
        });
    }
}

impl BlendState {
    pub(super) fn get_desc10(&self, out: *mut D3D10_BLEND_DESC) {
        let mut d = D3D11_BLEND_DESC::default();
        self.get_desc(&mut d);
        // Direct3D 10 blends every target as the first, enabled and masked per target.
        let t = &d.RenderTarget[0];
        put(
            out,
            D3D10_BLEND_DESC {
                AlphaToCoverageEnable: d.AlphaToCoverageEnable,
                BlendEnable: d.RenderTarget.map(|t| t.BlendEnable),
                SrcBlend: D3D10_BLEND(t.SrcBlend.0),
                DestBlend: D3D10_BLEND(t.DestBlend.0),
                BlendOp: D3D10_BLEND_OP(t.BlendOp.0),
                SrcBlendAlpha: D3D10_BLEND(t.SrcBlendAlpha.0),
                DestBlendAlpha: D3D10_BLEND(t.DestBlendAlpha.0),
                BlendOpAlpha: D3D10_BLEND_OP(t.BlendOpAlpha.0),
                RenderTargetWriteMask: d.RenderTarget.map(|t| t.RenderTargetWriteMask),
            },
        );
    }

    pub(super) fn get_desc110(&self, out: *mut D3D10_BLEND_DESC1) {
        let mut d = D3D11_BLEND_DESC::default();
        self.get_desc(&mut d);
        // One layout, one numbering.
        put(out, unsafe {
            std::mem::transmute::<D3D11_BLEND_DESC, D3D10_BLEND_DESC1>(d)
        });
    }
}

impl DepthStencilState {
    pub(super) fn get_desc10(&self, out: *mut D3D10_DEPTH_STENCIL_DESC) {
        let mut d = D3D11_DEPTH_STENCIL_DESC::default();
        self.get_desc(&mut d);
        // One layout, one numbering.
        put(out, unsafe {
            std::mem::transmute::<D3D11_DEPTH_STENCIL_DESC, D3D10_DEPTH_STENCIL_DESC>(d)
        });
    }
}

impl RasterizerState {
    pub(super) fn get_desc10(&self, out: *mut D3D10_RASTERIZER_DESC) {
        let mut d = D3D11_RASTERIZER_DESC::default();
        self.get_desc(&mut d);
        // One layout, one numbering.
        put(out, unsafe {
            std::mem::transmute::<D3D11_RASTERIZER_DESC, D3D10_RASTERIZER_DESC>(d)
        });
    }
}
