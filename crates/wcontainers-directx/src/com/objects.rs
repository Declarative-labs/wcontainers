//! The device's children: resources, views, states, input layouts and shaders.

use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Graphics::Direct3D11::*;
use windows::core::OutRef;

use super::counted::{Held, counted};

use wcontainers_direct3d_11::object as front;

use super::*;

/// The front resource behind a resource of ours; none for a foreign object.
pub fn resource_of(r: &ID3D11Resource) -> Option<front::Resource<Chosen>> {
    HeldResource::of(r).map(|h| h.front())
}

/// A resource of ours, held with a library reference.
#[derive(Clone)]
pub enum HeldResource {
    Buffer(Held<Buffer_Impl>),
    Texture1D(Held<Texture1D_Impl>),
    Texture2D(Held<Texture2D_Impl>),
    Texture3D(Held<Texture3D_Impl>),
}

impl HeldResource {
    /// None for a foreign object.
    pub fn of(r: &ID3D11Resource) -> Option<Self> {
        if let Ok(b) = r.cast_object_ref::<Buffer>() {
            return Some(HeldResource::Buffer(Held::new(b)));
        }
        if let Ok(t) = r.cast_object_ref::<Texture2D>() {
            return Some(HeldResource::Texture2D(Held::new(t)));
        }
        if let Ok(t) = r.cast_object_ref::<Texture1D>() {
            return Some(HeldResource::Texture1D(Held::new(t)));
        }
        r.cast_object_ref::<Texture3D>()
            .ok()
            .map(|t| HeldResource::Texture3D(Held::new(t)))
    }

    pub fn front(&self) -> front::Resource<Chosen> {
        match self {
            HeldResource::Buffer(b) => front::Resource::Buffer(b.front.clone()),
            HeldResource::Texture1D(t) => front::Resource::Texture(t.front.clone()),
            HeldResource::Texture2D(t) => front::Resource::Texture(t.front.clone()),
            HeldResource::Texture3D(t) => front::Resource::Texture(t.front.clone()),
        }
    }

    /// The resource with a reference of the program's.
    pub fn public(&self) -> ID3D11Resource {
        match self {
            HeldResource::Buffer(b) => b.public::<ID3D11Buffer>().into(),
            HeldResource::Texture1D(t) => t.public::<ID3D11Texture1D>().into(),
            HeldResource::Texture2D(t) => t.public::<ID3D11Texture2D>().into(),
            HeldResource::Texture3D(t) => t.public::<ID3D11Texture3D>().into(),
        }
    }
}

pub struct Buffer {
    pub front: Locked<Rc<front::Buffer<Chosen>>>,
    pub(super) child: Child,
    priority: Cell<u32>,
}

counted!(Buffer => Buffer_Impl { main: ID3D11Buffer, ID3D11Buffer_Vtbl });

impl Buffer {
    pub fn new(front: Rc<front::Buffer<Chosen>>, device: ID3D11Device) -> Self {
        Buffer {
            front: Locked::new(front),
            child: Child::new(device),
            priority: Cell::new(0),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_BUFFER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::buffer_back(&self.front.desc);
        }
    }

    pub(super) fn get_type(&self, out: *mut D3D11_RESOURCE_DIMENSION) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_RESOURCE_DIMENSION_BUFFER;
        }
    }

    pub(super) fn set_eviction_priority(&self, p: u32) {
        self.priority.set(p);
    }

    pub(super) fn get_eviction_priority(&self) -> u32 {
        self.priority.get()
    }
}

/// A texture of one dimension: its interface, description structure, conversion and resource type.
macro_rules! texture {
    ($name:ident, $imp:ident, $iface:ident, $vtbl:ident, $desc:ident, $back:path, $dim:ident) => {
        pub struct $name {
            pub front: Locked<Rc<front::Texture<Chosen>>>,
            pub(super) child: Child,
            priority: Cell<u32>,
        }

        counted!($name => $imp { main: $iface, $vtbl });

        impl $name {
            pub fn new(front: Rc<front::Texture<Chosen>>, device: ID3D11Device) -> Self {
                $name {
                    front: Locked::new(front),
                    child: Child::new(device),
                    priority: Cell::new(0),
                }
            }

            pub(super) fn get_desc(&self, out: *mut $desc) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = $back(&self.front.desc);
                }
            }

            pub(super) fn get_type(&self, out: *mut D3D11_RESOURCE_DIMENSION) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = $dim;
                }
            }

            pub(super) fn set_eviction_priority(&self, p: u32) {
                self.priority.set(p);
            }

            pub(super) fn get_eviction_priority(&self) -> u32 {
                self.priority.get()
            }
        }
    };
}

texture!(
    Texture1D,
    Texture1D_Impl,
    ID3D11Texture1D,
    ID3D11Texture1D_Vtbl,
    D3D11_TEXTURE1D_DESC,
    convert::texture1d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE1D
);
texture!(
    Texture2D,
    Texture2D_Impl,
    ID3D11Texture2D,
    ID3D11Texture2D_Vtbl,
    D3D11_TEXTURE2D_DESC,
    convert::texture2d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE2D
);
texture!(
    Texture3D,
    Texture3D_Impl,
    ID3D11Texture3D,
    ID3D11Texture3D_Vtbl,
    D3D11_TEXTURE3D_DESC,
    convert::texture3d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE3D
);

/// A view: the front view and the resource it views (held with a library reference).
pub struct RenderTargetView {
    pub front: Locked<Rc<front::View<Chosen>>>,
    resource: HeldResource,
    pub(super) child: Child,
}

counted!(RenderTargetView => RenderTargetView_Impl { main: ID3D11RenderTargetView, ID3D11RenderTargetView_Vtbl });

impl RenderTargetView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        RenderTargetView {
            front: Locked::new(front),
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_RENDER_TARGET_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::rtv_back(&self.front.desc);
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A view: the front view and the resource it views (held with a library reference).
pub struct DepthStencilView {
    pub front: Locked<Rc<front::View<Chosen>>>,
    resource: HeldResource,
    pub(super) child: Child,
}

counted!(DepthStencilView => DepthStencilView_Impl { main: ID3D11DepthStencilView, ID3D11DepthStencilView_Vtbl });

impl DepthStencilView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilView {
            front: Locked::new(front),
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_DEPTH_STENCIL_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::dsv_back(&self.front.desc);
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A view: the front view and the resource it views (held with a library reference).
pub struct ShaderResourceView {
    pub front: Locked<Rc<front::View<Chosen>>>,
    resource: HeldResource,
    pub(super) child: Child,
}

counted!(ShaderResourceView => ShaderResourceView_Impl { main: ID3D11ShaderResourceView, ID3D11ShaderResourceView_Vtbl });

impl ShaderResourceView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        ShaderResourceView {
            front: Locked::new(front),
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_SHADER_RESOURCE_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::srv_back(&self.front.desc);
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct SamplerState {
    pub front: Locked<Rc<front::Sampler<Chosen>>>,
    desc: D3D11_SAMPLER_DESC,
    pub(super) child: Child,
}

counted!(SamplerState => SamplerState_Impl { main: ID3D11SamplerState, ID3D11SamplerState_Vtbl });

impl SamplerState {
    pub fn new(
        front: Rc<front::Sampler<Chosen>>,
        desc: D3D11_SAMPLER_DESC,
        device: ID3D11Device,
    ) -> Self {
        SamplerState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_SAMPLER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct BlendState {
    pub front: Locked<Rc<front::Blend<Chosen>>>,
    desc: D3D11_BLEND_DESC,
    pub(super) child: Child,
}

counted!(BlendState => BlendState_Impl { main: ID3D11BlendState, ID3D11BlendState_Vtbl });

impl BlendState {
    pub fn new(
        front: Rc<front::Blend<Chosen>>,
        desc: D3D11_BLEND_DESC,
        device: ID3D11Device,
    ) -> Self {
        BlendState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_BLEND_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct DepthStencilState {
    pub front: Locked<Rc<front::DepthStencil<Chosen>>>,
    desc: D3D11_DEPTH_STENCIL_DESC,
    pub(super) child: Child,
}

counted!(DepthStencilState => DepthStencilState_Impl { main: ID3D11DepthStencilState, ID3D11DepthStencilState_Vtbl });

impl DepthStencilState {
    pub fn new(
        front: Rc<front::DepthStencil<Chosen>>,
        desc: D3D11_DEPTH_STENCIL_DESC,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_DEPTH_STENCIL_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct RasterizerState {
    pub front: Locked<Rc<front::Rasterizer<Chosen>>>,
    desc: D3D11_RASTERIZER_DESC,
    pub(super) child: Child,
}

counted!(RasterizerState => RasterizerState_Impl { main: ID3D11RasterizerState, ID3D11RasterizerState_Vtbl });

impl RasterizerState {
    pub fn new(
        front: Rc<front::Rasterizer<Chosen>>,
        desc: D3D11_RASTERIZER_DESC,
        device: ID3D11Device,
    ) -> Self {
        RasterizerState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_RASTERIZER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

pub struct InputLayout {
    pub front: Locked<Rc<front::InputLayout>>,
    pub(super) child: Child,
}

counted!(InputLayout => InputLayout_Impl { main: ID3D11InputLayout, ID3D11InputLayout_Vtbl });

impl InputLayout {
    pub fn new(front: Rc<front::InputLayout>, device: ID3D11Device) -> Self {
        InputLayout {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

pub struct VertexShader {
    pub front: Locked<Rc<front::Shader<Chosen>>>,
    pub(super) child: Child,
}

counted!(VertexShader => VertexShader_Impl { main: ID3D11VertexShader, ID3D11VertexShader_Vtbl });

impl VertexShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        VertexShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

pub struct PixelShader {
    pub front: Locked<Rc<front::Shader<Chosen>>>,
    pub(super) child: Child,
}

counted!(PixelShader => PixelShader_Impl { main: ID3D11PixelShader, ID3D11PixelShader_Vtbl });

impl PixelShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        PixelShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

/// A query or predicate: its interface, and whether `QueryInterface` answers `ID3D11Predicate`.
macro_rules! asynchronous {
    ($name:ident, $imp:ident, $iface:ident, $vtbl:ident) => {
        pub struct $name {
            pub front: Locked<Rc<wcontainers_direct3d_11::query::Query<Chosen>>>,
            pub(super) child: Child,
        }

        counted!($name => $imp { main: $iface, $vtbl });

        impl $name {
            pub fn new(
                front: Rc<wcontainers_direct3d_11::query::Query<Chosen>>,
                device: ID3D11Device,
            ) -> Self {
                $name {
                    front: Locked::new(front),
                    child: Child::new(device),
                }
            }

            pub(super) fn get_desc(&self, out: *mut D3D11_QUERY_DESC) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = D3D11_QUERY_DESC {
                        Query: D3D11_QUERY(self.front.kind as i32),
                        MiscFlags: self.front.misc,
                    };
                }
            }

            pub(super) fn get_data_size(&self) -> u32 {
                wcontainers_direct3d_11::query::data_size(self.front.kind).unwrap_or(0)
            }
        }
    };
}

asynchronous!(Query, Query_Impl, ID3D11Query, ID3D11Query_Vtbl);
asynchronous!(
    Predicate,
    Predicate_Impl,
    ID3D11Predicate,
    ID3D11Predicate_Vtbl
);

/// The front query behind a query or predicate of ours.
pub fn query_of(
    a: Option<&ID3D11Asynchronous>,
) -> Option<Rc<wcontainers_direct3d_11::query::Query<Chosen>>> {
    let a = a?;
    if let Ok(q) = a.cast_object_ref::<Query>() {
        return Some(q.front.clone());
    }
    a.cast_object_ref::<Predicate>()
        .ok()
        .map(|p| p.front.clone())
}
