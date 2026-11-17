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
    if let Ok(b) = r.cast_object_ref::<Buffer>() {
        return Some(front::Resource::Buffer(b.front.clone()));
    }
    r.cast_object_ref::<Texture2D>()
        .ok()
        .map(|t| front::Resource::Texture2D(t.front.clone()))
}

/// A resource of ours, held with a library reference.
#[derive(Clone)]
pub enum HeldResource {
    Buffer(Held<Buffer_Impl>),
    Texture2D(Held<Texture2D_Impl>),
}

impl HeldResource {
    /// None for a foreign object.
    pub fn of(r: &ID3D11Resource) -> Option<Self> {
        if let Ok(b) = r.cast_object_ref::<Buffer>() {
            return Some(HeldResource::Buffer(Held::new(b)));
        }
        r.cast_object_ref::<Texture2D>()
            .ok()
            .map(|t| HeldResource::Texture2D(Held::new(t)))
    }

    pub fn front(&self) -> front::Resource<Chosen> {
        match self {
            HeldResource::Buffer(b) => front::Resource::Buffer(b.front.clone()),
            HeldResource::Texture2D(t) => front::Resource::Texture2D(t.front.clone()),
        }
    }

    /// The resource with a reference of the program's.
    pub fn public(&self) -> ID3D11Resource {
        match self {
            HeldResource::Buffer(b) => b.public::<ID3D11Buffer>().into(),
            HeldResource::Texture2D(t) => t.public::<ID3D11Texture2D>().into(),
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

pub struct Texture2D {
    pub front: Locked<Rc<front::Texture2D<Chosen>>>,
    pub(super) child: Child,
    priority: Cell<u32>,
}

counted!(Texture2D => Texture2D_Impl { main: ID3D11Texture2D, ID3D11Texture2D_Vtbl });

impl Texture2D {
    pub fn new(front: Rc<front::Texture2D<Chosen>>, device: ID3D11Device) -> Self {
        Texture2D {
            front: Locked::new(front),
            child: Child::new(device),
            priority: Cell::new(0),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_TEXTURE2D_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::texture2d_back(&self.front.desc);
        }
    }

    pub(super) fn get_type(&self, out: *mut D3D11_RESOURCE_DIMENSION) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_RESOURCE_DIMENSION_TEXTURE2D;
        }
    }

    pub(super) fn set_eviction_priority(&self, p: u32) {
        self.priority.set(p);
    }

    pub(super) fn get_eviction_priority(&self) -> u32 {
        self.priority.get()
    }
}

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
