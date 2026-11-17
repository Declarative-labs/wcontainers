//! The device's children: resources, views, states, input layouts and shaders.

use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Graphics::Direct3D11::*;
use windows::core::{OutRef, implement};

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

#[implement(ID3D11Buffer)]
pub struct Buffer {
    pub front: Rc<front::Buffer<Chosen>>,
    pub(super) child: Child,
    priority: Cell<u32>,
}

impl Buffer {
    pub fn new(front: Rc<front::Buffer<Chosen>>, device: ID3D11Device) -> Self {
        Buffer {
            front,
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

#[implement(ID3D11Texture2D)]
pub struct Texture2D {
    pub front: Rc<front::Texture2D<Chosen>>,
    pub(super) child: Child,
    priority: Cell<u32>,
}

impl Texture2D {
    pub fn new(front: Rc<front::Texture2D<Chosen>>, device: ID3D11Device) -> Self {
        Texture2D {
            front,
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

/// A view: the front view and the resource it views (held with a reference).
#[implement(ID3D11RenderTargetView)]
pub struct RenderTargetView {
    pub front: Rc<front::View<Chosen>>,
    resource: ID3D11Resource,
    pub(super) child: Child,
}

impl RenderTargetView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: ID3D11Resource,
        device: ID3D11Device,
    ) -> Self {
        RenderTargetView {
            front,
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
        let _ = out.write(Some(self.resource.clone()));
    }
}

/// A view: the front view and the resource it views (held with a reference).
#[implement(ID3D11DepthStencilView)]
pub struct DepthStencilView {
    pub front: Rc<front::View<Chosen>>,
    resource: ID3D11Resource,
    pub(super) child: Child,
}

impl DepthStencilView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: ID3D11Resource,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilView {
            front,
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
        let _ = out.write(Some(self.resource.clone()));
    }
}

/// A view: the front view and the resource it views (held with a reference).
#[implement(ID3D11ShaderResourceView)]
pub struct ShaderResourceView {
    pub front: Rc<front::View<Chosen>>,
    resource: ID3D11Resource,
    pub(super) child: Child,
}

impl ShaderResourceView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: ID3D11Resource,
        device: ID3D11Device,
    ) -> Self {
        ShaderResourceView {
            front,
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
        let _ = out.write(Some(self.resource.clone()));
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
#[implement(ID3D11SamplerState)]
pub struct SamplerState {
    pub front: Rc<front::Sampler<Chosen>>,
    desc: D3D11_SAMPLER_DESC,
    pub(super) child: Child,
}

impl SamplerState {
    pub fn new(
        front: Rc<front::Sampler<Chosen>>,
        desc: D3D11_SAMPLER_DESC,
        device: ID3D11Device,
    ) -> Self {
        SamplerState {
            front,
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
#[implement(ID3D11BlendState)]
pub struct BlendState {
    pub front: Rc<front::Blend<Chosen>>,
    desc: D3D11_BLEND_DESC,
    pub(super) child: Child,
}

impl BlendState {
    pub fn new(
        front: Rc<front::Blend<Chosen>>,
        desc: D3D11_BLEND_DESC,
        device: ID3D11Device,
    ) -> Self {
        BlendState {
            front,
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
#[implement(ID3D11DepthStencilState)]
pub struct DepthStencilState {
    pub front: Rc<front::DepthStencil<Chosen>>,
    desc: D3D11_DEPTH_STENCIL_DESC,
    pub(super) child: Child,
}

impl DepthStencilState {
    pub fn new(
        front: Rc<front::DepthStencil<Chosen>>,
        desc: D3D11_DEPTH_STENCIL_DESC,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilState {
            front,
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
#[implement(ID3D11RasterizerState)]
pub struct RasterizerState {
    pub front: Rc<front::Rasterizer<Chosen>>,
    desc: D3D11_RASTERIZER_DESC,
    pub(super) child: Child,
}

impl RasterizerState {
    pub fn new(
        front: Rc<front::Rasterizer<Chosen>>,
        desc: D3D11_RASTERIZER_DESC,
        device: ID3D11Device,
    ) -> Self {
        RasterizerState {
            front,
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

#[implement(ID3D11InputLayout)]
pub struct InputLayout {
    pub front: Rc<front::InputLayout>,
    pub(super) child: Child,
}

impl InputLayout {
    pub fn new(front: Rc<front::InputLayout>, device: ID3D11Device) -> Self {
        InputLayout {
            front,
            child: Child::new(device),
        }
    }
}

#[implement(ID3D11VertexShader)]
pub struct VertexShader {
    pub front: Rc<front::Shader<Chosen>>,
    pub(super) child: Child,
}

impl VertexShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        VertexShader {
            front,
            child: Child::new(device),
        }
    }
}

#[implement(ID3D11PixelShader)]
pub struct PixelShader {
    pub front: Rc<front::Shader<Chosen>>,
    pub(super) child: Child,
}

impl PixelShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        PixelShader {
            front,
            child: Child::new(device),
        }
    }
}
