//! Direct3D 10's interfaces on Direct3D 11's objects, as Direct3D 11 offers them: the same object
//! described in Direct3D 10's structures. Their device is the device's Direct3D 10 one, offered
//! once a context state was made; their maps and queries run on the immediate context.

use std::ffi::c_void;
use windows::Win32::Graphics::Direct3D::D3D_SRV_DIMENSION_BUFFEREX;
use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};

use windows::core::{IUnknownImpl, Interface, OutRef, Result};

use super::*;

impl Child {
    pub(crate) fn get_device10(&self, out: OutRef<ID3D10Device>) {
        let _ = out.write(self.device().cast().ok());
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

/// The device a child of ours was made by.
fn device_of(child: &Child) -> &Device_Impl {
    child
        .device()
        .cast_object_ref::<Device>()
        .expect("our device")
}

impl Buffer_Impl {
    /// Direct3D 10's `Map`, on the immediate context while Direct3D 10's behaviour is active.
    pub(super) fn map10(&self, kind: D3D10_MAP, flags: u32, out: *mut *mut c_void) -> Result<()> {
        let me: ID3D11Resource = self.to_interface::<ID3D11Buffer>().into();
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        let r = device_of(&self.child).map10(&me, 0, kind, flags, &mut m);
        put(
            out,
            if r.is_ok() {
                m.pData
            } else {
                std::ptr::null_mut()
            },
        );
        r
    }

    pub(super) fn unmap10(&self) {
        let me: ID3D11Resource = self.to_interface::<ID3D11Buffer>().into();
        device_of(&self.child).unmap10(&me, 0);
    }
}

/// Direct3D 10's `Map` and `Unmap` of a texture type, through Direct3D 11's.
macro_rules! map10 {
    ($outer:ident, $iface:ident) => {
        impl $outer {
            fn map_any10(
                &self,
                sub: u32,
                kind: D3D10_MAP,
                flags: u32,
            ) -> Result<D3D11_MAPPED_SUBRESOURCE> {
                let me: ID3D11Resource = self.to_interface::<$iface>().into();
                let mut m = D3D11_MAPPED_SUBRESOURCE::default();
                device_of(&self.child).map10(&me, sub, kind, flags, &mut m)?;
                Ok(m)
            }

            pub(super) fn unmap10(&self, sub: u32) {
                let me: ID3D11Resource = self.to_interface::<$iface>().into();
                device_of(&self.child).unmap10(&me, sub);
            }
        }
    };
}

map10!(Texture1D_Impl, ID3D11Texture1D);
map10!(Texture2D_Impl, ID3D11Texture2D);
map10!(Texture3D_Impl, ID3D11Texture3D);

impl Texture1D_Impl {
    pub(super) fn map10(
        &self,
        sub: u32,
        kind: D3D10_MAP,
        flags: u32,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let r = self.map_any10(sub, kind, flags);
        put(out, r.as_ref().map_or(std::ptr::null_mut(), |m| m.pData));
        r.map(|_| ())
    }
}

impl Texture2D_Impl {
    pub(super) fn map10(
        &self,
        sub: u32,
        kind: D3D10_MAP,
        flags: u32,
    ) -> Result<D3D10_MAPPED_TEXTURE2D> {
        let m = self.map_any10(sub, kind, flags)?;
        Ok(D3D10_MAPPED_TEXTURE2D {
            pData: m.pData,
            RowPitch: m.RowPitch,
        })
    }
}

impl Texture3D_Impl {
    pub(super) fn map10(
        &self,
        sub: u32,
        kind: D3D10_MAP,
        flags: u32,
    ) -> Result<D3D10_MAPPED_TEXTURE3D> {
        let m = self.map_any10(sub, kind, flags)?;
        Ok(D3D10_MAPPED_TEXTURE3D {
            pData: m.pData,
            RowPitch: m.RowPitch,
            DepthPitch: m.DepthPitch,
        })
    }
}

/// `D3D10_QUERY_PIPELINE_STATISTICS`'s answer: Direct3D 11's without its last three counts
/// (the hull, domain and compute stages').
const STATISTICS10: u32 = 8 * 8;

/// Direct3D 10's asynchronous calls on a query type, run on the immediate context.
macro_rules! asynchronous10 {
    ($outer:ident, $iface:ident) => {
        impl $outer {
            fn me11(&self) -> ID3D11Asynchronous {
                self.to_interface::<$iface>().into()
            }

            fn statistics(&self) -> bool {
                self.front.kind == D3D11_QUERY_PIPELINE_STATISTICS.0 as u32
            }

            pub(super) fn begin10(&self) {
                device_of(&self.child).begin10(&self.me11());
            }

            pub(super) fn end10(&self) {
                device_of(&self.child).end10(&self.me11());
            }

            pub(super) fn get_data10(
                &self,
                data: *mut c_void,
                size: u32,
                flags: u32,
            ) -> Result<()> {
                let device = device_of(&self.child);
                if !self.statistics() || data.is_null() {
                    return device.get_data10(&self.me11(), data, size, flags);
                }
                if size != STATISTICS10 {
                    return Err(windows::Win32::Foundation::E_INVALIDARG.into());
                }
                let mut all = D3D11_QUERY_DATA_PIPELINE_STATISTICS::default();
                let r = device.get_data10(
                    &self.me11(),
                    (&mut all as *mut D3D11_QUERY_DATA_PIPELINE_STATISTICS).cast(),
                    size_of_val(&all) as u32,
                    flags,
                );
                if r.is_ok() {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            (&all as *const D3D11_QUERY_DATA_PIPELINE_STATISTICS).cast::<u8>(),
                            data.cast::<u8>(),
                            STATISTICS10 as usize,
                        )
                    };
                }
                r
            }

            pub(super) fn get_data_size10(&self) -> u32 {
                if self.statistics() {
                    STATISTICS10
                } else {
                    self.get_data_size()
                }
            }

            pub(super) fn get_desc10(&self, out: *mut D3D10_QUERY_DESC) {
                put(
                    out,
                    D3D10_QUERY_DESC {
                        Query: D3D10_QUERY(self.front.kind as i32),
                        MiscFlags: self.front.misc,
                    },
                );
            }
        }
    };
}

asynchronous10!(Query_Impl, ID3D11Query1);
asynchronous10!(Predicate_Impl, ID3D11Predicate);
