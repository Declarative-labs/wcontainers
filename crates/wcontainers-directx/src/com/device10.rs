//! Direct3D 10's device (`ID3D10Device1`) on the Direct3D 11 device, as Direct3D 11 offers it
//! once a context state has been made: creation through Direct3D 11's calls with Direct3D 10's
//! descriptions, and the context calls run on the immediate context while a state emulating
//! Direct3D 10 is active there. While Direct3D 11's behaviour is active they do nothing, as
//! Direct3D's own do (`SwapDeviceContextState`).

use std::ffi::c_void;

use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::core::{BOOL, Interface, OutRef, Ref, Result};

use super::context::Context;
use super::device::as_one;
use super::*;

/// The program's Direct3D 10 objects as their Direct3D 11 interfaces, a foreign object as none;
/// none for a null list.
fn up<I: Interface, J: Interface>(list: *const Option<I>, n: u32) -> Option<Vec<Option<J>>> {
    (!list.is_null()).then(|| {
        unsafe { std::slice::from_raw_parts(list, n as usize) }
            .iter()
            .map(|o| o.as_ref().and_then(|o| o.cast().ok()))
            .collect()
    })
}

/// A list made by [`up`] as the pointer Direct3D 11's call takes.
fn raw<T>(list: &Option<Vec<T>>) -> *const T {
    list.as_ref().map_or(std::ptr::null(), |l| l.as_ptr())
}

fn one<I: Interface, J: Interface>(object: Ref<I>) -> Option<J> {
    object.as_ref().and_then(|o| o.cast().ok())
}

/// Runs a Direct3D 11 getter of `n` objects and writes them to the program's `out` as their
/// Direct3D 10 interfaces.
fn down_list<J: Interface, I: Interface>(
    n: u32,
    out: *mut Option<I>,
    get: impl FnOnce(*mut Option<J>),
) {
    if out.is_null() {
        return;
    }
    let mut got: Vec<Option<J>> = vec![None; n as usize];
    get(got.as_mut_ptr());
    for (i, g) in got.into_iter().enumerate() {
        unsafe { out.add(i).write(g.and_then(|g| g.cast().ok())) };
    }
}

/// Runs a Direct3D 11 getter of one object and writes it to `out` as its Direct3D 10 interface.
fn down<J, I>(out: OutRef<I>, get: impl FnOnce(OutRef<J>))
where
    J: Interface + windows::core::Type<J, Default = Option<J>>,
    I: Interface + windows::core::Type<I, Default = Option<I>>,
{
    if out.is_null() {
        return;
    }
    let mut got: Option<J> = None;
    get(OutRef::from(&mut got));
    let _ = out.write(got.and_then(|g| g.cast().ok()));
}

/// An `OutRef` the windows crate gives for what is an array of `n` objects, as its pointer.
fn array<I: windows::core::Type<I, Default = Option<I>>>(out: OutRef<I>) -> *mut Option<I> {
    // SAFETY: `OutRef` is a transparent pointer to the program's slots.
    unsafe { std::mem::transmute_copy::<OutRef<I>, *mut Option<I>>(&out) }
}

/// Direct3D 11's misc flags of a resource described with Direct3D 10's: those Direct3D 10 has, at
/// Direct3D 11's values.
fn misc11(misc: u32) -> u32 {
    let kept = (D3D10_RESOURCE_MISC_GENERATE_MIPS.0
        | D3D10_RESOURCE_MISC_SHARED.0
        | D3D10_RESOURCE_MISC_TEXTURECUBE.0) as u32;
    let mut m = misc & kept;
    if misc & D3D10_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32 != 0 {
        m |= D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32;
    }
    if misc & D3D10_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32 != 0 {
        m |= D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32;
    }
    m
}

/// The CPU access a Direct3D 10 texture is made with: Direct3D 10's runtime takes a default
/// texture bound as a shader resource or render target with CPU access, and makes it without any
/// (its maps then fail as on any default texture).
fn cpu_access10(usage: D3D11_USAGE, bind: u32, cpu: u32) -> u32 {
    let ignored = (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32;
    if usage == D3D11_USAGE_DEFAULT && bind & ignored != 0 {
        0
    } else {
        cpu
    }
}

fn viewport11(v: &D3D10_VIEWPORT) -> D3D11_VIEWPORT {
    D3D11_VIEWPORT {
        TopLeftX: v.TopLeftX as f32,
        TopLeftY: v.TopLeftY as f32,
        Width: v.Width as f32,
        Height: v.Height as f32,
        MinDepth: v.MinDepth,
        MaxDepth: v.MaxDepth,
    }
}

fn viewport10(v: &D3D11_VIEWPORT) -> D3D10_VIEWPORT {
    D3D10_VIEWPORT {
        TopLeftX: v.TopLeftX as i32,
        TopLeftY: v.TopLeftY as i32,
        Width: v.Width as u32,
        Height: v.Height as u32,
        MinDepth: v.MinDepth,
        MaxDepth: v.MaxDepth,
    }
}

/// Direct3D 10's blend description in Direct3D 11's: one blend for every target, each target
/// enabled and masked on its own (independent where they differ).
fn blend11(d: &D3D10_BLEND_DESC) -> D3D11_BLEND_DESC {
    let mut b = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: d.AlphaToCoverageEnable,
        IndependentBlendEnable: BOOL::from(
            d.BlendEnable.iter().any(|e| *e != d.BlendEnable[0])
                || d.RenderTargetWriteMask
                    .iter()
                    .any(|m| *m != d.RenderTargetWriteMask[0]),
        ),
        ..Default::default()
    };
    for (i, t) in b.RenderTarget.iter_mut().enumerate() {
        *t = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: d.BlendEnable[i],
            SrcBlend: D3D11_BLEND(d.SrcBlend.0),
            DestBlend: D3D11_BLEND(d.DestBlend.0),
            BlendOp: D3D11_BLEND_OP(d.BlendOp.0),
            SrcBlendAlpha: D3D11_BLEND(d.SrcBlendAlpha.0),
            DestBlendAlpha: D3D11_BLEND(d.DestBlendAlpha.0),
            BlendOpAlpha: D3D11_BLEND_OP(d.BlendOpAlpha.0),
            RenderTargetWriteMask: d.RenderTargetWriteMask[i],
        };
    }
    b
}

impl Device_Impl {
    /// The immediate context while Direct3D 10's behaviour is active on it.
    fn context10(&self) -> Option<ID3D11DeviceContext3> {
        let c = self.immediate();
        let on = c.cast_object_ref::<Context>().is_ok_and(|c| c.emulates10());
        on.then_some(c)
    }

    /// Runs `f` on the immediate context while Direct3D 10's behaviour is active; does nothing
    /// otherwise.
    fn on10(&self, f: impl FnOnce(&Context)) {
        if self.context10().is_some() {
            self.get10(f);
        }
    }

    /// Runs a getter on the immediate context as Direct3D 10's: it reads the default state while
    /// Direct3D 11's behaviour is active.
    fn get10(&self, f: impl FnOnce(&Context)) {
        let c = self.immediate();
        let c = c.cast_object_ref::<Context>().expect("ours");
        c.calling10.set(true);
        f(c);
        c.calling10.set(false);
    }

    pub(super) fn vs_set_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10Buffer>,
    ) {
        let l = up::<_, ID3D11Buffer>(list, n);
        self.on10(|c| c.vs_set_constant_buffers(start, n, raw(&l)));
    }

    pub(super) fn ps_set_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10Buffer>,
    ) {
        let l = up::<_, ID3D11Buffer>(list, n);
        self.on10(|c| c.ps_set_constant_buffers(start, n, raw(&l)));
    }

    pub(super) fn gs_set_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10Buffer>,
    ) {
        let l = up::<_, ID3D11Buffer>(list, n);
        self.on10(|c| c.gs_set_constant_buffers(start, n, raw(&l)));
    }

    pub(super) fn vs_set_shader_resources10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10ShaderResourceView>,
    ) {
        let l = up::<_, ID3D11ShaderResourceView>(list, n);
        self.on10(|c| c.vs_set_shader_resources(start, n, raw(&l)));
    }

    pub(super) fn ps_set_shader_resources10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10ShaderResourceView>,
    ) {
        let l = up::<_, ID3D11ShaderResourceView>(list, n);
        self.on10(|c| c.ps_set_shader_resources(start, n, raw(&l)));
    }

    pub(super) fn gs_set_shader_resources10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10ShaderResourceView>,
    ) {
        let l = up::<_, ID3D11ShaderResourceView>(list, n);
        self.on10(|c| c.gs_set_shader_resources(start, n, raw(&l)));
    }

    pub(super) fn vs_set_samplers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10SamplerState>,
    ) {
        let l = up::<_, ID3D11SamplerState>(list, n);
        self.on10(|c| c.vs_set_samplers(start, n, raw(&l)));
    }

    pub(super) fn ps_set_samplers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10SamplerState>,
    ) {
        let l = up::<_, ID3D11SamplerState>(list, n);
        self.on10(|c| c.ps_set_samplers(start, n, raw(&l)));
    }

    pub(super) fn gs_set_samplers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10SamplerState>,
    ) {
        let l = up::<_, ID3D11SamplerState>(list, n);
        self.on10(|c| c.gs_set_samplers(start, n, raw(&l)));
    }

    pub(super) fn vs_set_shader10(&self, shader: Ref<ID3D10VertexShader>) {
        let s = one::<_, ID3D11VertexShader>(shader);
        self.on10(|c| c.vs_set_shader(Ref::from(&s), std::ptr::null(), 0));
    }

    pub(super) fn ps_set_shader10(&self, shader: Ref<ID3D10PixelShader>) {
        let s = one::<_, ID3D11PixelShader>(shader);
        self.on10(|c| c.ps_set_shader(Ref::from(&s), std::ptr::null(), 0));
    }

    pub(super) fn gs_set_shader10(&self, shader: Ref<ID3D10GeometryShader>) {
        let s = one::<_, ID3D11GeometryShader>(shader);
        self.on10(|c| c.gs_set_shader(Ref::from(&s), std::ptr::null(), 0));
    }

    pub(super) fn draw_indexed10(&self, count: u32, start: u32, base: i32) {
        self.on10(|c| c.draw_indexed(count, start, base));
    }

    pub(super) fn draw10(&self, count: u32, start: u32) {
        self.on10(|c| c.draw(count, start));
    }

    pub(super) fn draw_indexed_instanced10(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        base: i32,
        first_instance: u32,
    ) {
        self.on10(|c| c.draw_indexed_instanced(count, instances, start, base, first_instance));
    }

    pub(super) fn draw_instanced10(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        first_instance: u32,
    ) {
        self.on10(|c| c.draw_instanced(count, instances, start, first_instance));
    }

    pub(super) fn draw_auto10(&self) {
        self.on10(|c| c.draw_auto());
    }

    pub(super) fn ia_set_input_layout10(&self, layout: Ref<ID3D10InputLayout>) {
        let l = one::<_, ID3D11InputLayout>(layout);
        self.on10(|c| c.ia_set_input_layout(Ref::from(&l)));
    }

    pub(super) fn ia_set_vertex_buffers10(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D10Buffer>,
        strides: *const u32,
        offsets: *const u32,
    ) {
        let l = up::<_, ID3D11Buffer>(list, n);
        self.on10(|c| c.ia_set_vertex_buffers(start, n, raw(&l), strides, offsets));
    }

    pub(super) fn ia_set_index_buffer10(
        &self,
        buffer: Ref<ID3D10Buffer>,
        format: DXGI_FORMAT,
        offset: u32,
    ) {
        let b = one::<_, ID3D11Buffer>(buffer);
        self.on10(|c| c.ia_set_index_buffer(Ref::from(&b), format, offset));
    }

    pub(super) fn ia_set_primitive_topology10(&self, topology: D3D_PRIMITIVE_TOPOLOGY) {
        self.on10(|c| c.ia_set_primitive_topology(topology));
    }

    pub(super) fn set_predication10(&self, predicate: Ref<ID3D10Predicate>, value: BOOL) {
        let p = one::<_, ID3D11Predicate>(predicate);
        self.on10(|c| c.set_predication(Ref::from(&p), value));
    }

    pub(super) fn om_set_render_targets10(
        &self,
        n: u32,
        list: *const Option<ID3D10RenderTargetView>,
        depth: Ref<ID3D10DepthStencilView>,
    ) {
        let l = up::<_, ID3D11RenderTargetView>(list, n);
        let d = one::<_, ID3D11DepthStencilView>(depth);
        self.on10(|c| c.om_set_render_targets(n, raw(&l), Ref::from(&d)));
    }

    pub(super) fn om_set_blend_state10(
        &self,
        state: Ref<ID3D10BlendState>,
        factor: *const f32,
        mask: u32,
    ) {
        let s = one::<_, ID3D11BlendState>(state);
        self.on10(|c| c.om_set_blend_state(Ref::from(&s), factor, mask));
    }

    pub(super) fn om_set_depth_stencil_state10(
        &self,
        state: Ref<ID3D10DepthStencilState>,
        reference: u32,
    ) {
        let s = one::<_, ID3D11DepthStencilState>(state);
        self.on10(|c| c.om_set_depth_stencil_state(Ref::from(&s), reference));
    }

    pub(super) fn so_set_targets10(
        &self,
        n: u32,
        list: *const Option<ID3D10Buffer>,
        offsets: *const u32,
    ) {
        let l = up::<_, ID3D11Buffer>(list, n);
        self.on10(|c| c.so_set_targets(n, raw(&l), offsets));
    }

    pub(super) fn rs_set_state10(&self, state: Ref<ID3D10RasterizerState>) {
        let s = one::<_, ID3D11RasterizerState>(state);
        self.on10(|c| c.rs_set_state(Ref::from(&s)));
    }

    pub(super) fn rs_set_viewports10(&self, n: u32, viewports: *const D3D10_VIEWPORT) {
        let v: Vec<D3D11_VIEWPORT> = if viewports.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(viewports, n as usize) }
                .iter()
                .map(viewport11)
                .collect()
        };
        let p = if viewports.is_null() {
            std::ptr::null()
        } else {
            v.as_ptr()
        };
        self.on10(|c| c.rs_set_viewports(n, p));
    }

    pub(super) fn rs_set_scissor_rects10(&self, n: u32, rects: *const RECT) {
        self.on10(|c| c.rs_set_scissor_rects(n, rects));
    }

    #[allow(clippy::too_many_arguments, reason = "the COM method's own arguments")]
    pub(super) fn copy_subresource_region10(
        &self,
        dst: Ref<ID3D10Resource>,
        dst_sub: u32,
        x: u32,
        y: u32,
        z: u32,
        src: Ref<ID3D10Resource>,
        src_sub: u32,
        src_box: *const D3D10_BOX,
    ) {
        let (d, s) = (one::<_, ID3D11Resource>(dst), one::<_, ID3D11Resource>(src));
        if into_depth_from_colour(&d, &s) {
            return;
        }
        self.on10(|c| {
            c.copy_subresource_region(
                Ref::from(&d),
                dst_sub,
                x,
                y,
                z,
                Ref::from(&s),
                src_sub,
                src_box.cast(),
            )
        });
    }

    pub(super) fn copy_resource10(&self, dst: Ref<ID3D10Resource>, src: Ref<ID3D10Resource>) {
        let (d, s) = (one::<_, ID3D11Resource>(dst), one::<_, ID3D11Resource>(src));
        if into_depth_from_colour(&d, &s) {
            return;
        }
        self.on10(|c| c.copy_resource(Ref::from(&d), Ref::from(&s)));
    }

    pub(super) fn update_subresource10(
        &self,
        dst: Ref<ID3D10Resource>,
        sub: u32,
        dst_box: *const D3D10_BOX,
        data: *const c_void,
        row_pitch: u32,
        depth_pitch: u32,
    ) {
        let d = one::<_, ID3D11Resource>(dst);
        self.on10(|c| {
            c.update_subresource(
                Ref::from(&d),
                sub,
                dst_box.cast(),
                data,
                row_pitch,
                depth_pitch,
            )
        });
    }

    pub(super) fn clear_render_target_view10(
        &self,
        view: Ref<ID3D10RenderTargetView>,
        color: *const f32,
    ) {
        let v = one::<_, ID3D11RenderTargetView>(view);
        self.on10(|c| c.clear_render_target_view(Ref::from(&v), color));
    }

    pub(super) fn clear_depth_stencil_view10(
        &self,
        view: Ref<ID3D10DepthStencilView>,
        flags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let v = one::<_, ID3D11DepthStencilView>(view);
        self.on10(|c| c.clear_depth_stencil_view(Ref::from(&v), flags, depth, stencil));
    }

    pub(super) fn generate_mips10(&self, view: Ref<ID3D10ShaderResourceView>) {
        let v = one::<_, ID3D11ShaderResourceView>(view);
        self.on10(|c| c.generate_mips(Ref::from(&v)));
    }

    pub(super) fn resolve_subresource10(
        &self,
        dst: Ref<ID3D10Resource>,
        dst_sub: u32,
        src: Ref<ID3D10Resource>,
        src_sub: u32,
        format: DXGI_FORMAT,
    ) {
        let (d, s) = (one::<_, ID3D11Resource>(dst), one::<_, ID3D11Resource>(src));
        self.on10(|c| {
            c.resolve_subresource(Ref::from(&d), dst_sub, Ref::from(&s), src_sub, format)
        });
    }

    pub(super) fn vs_get_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10Buffer>,
    ) {
        down_list::<ID3D11Buffer, _>(n, out, |o| {
            self.get10(|c| c.vs_get_constant_buffers(start, n, o))
        });
    }

    pub(super) fn ps_get_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10Buffer>,
    ) {
        down_list::<ID3D11Buffer, _>(n, out, |o| {
            self.get10(|c| c.ps_get_constant_buffers(start, n, o))
        });
    }

    pub(super) fn gs_get_constant_buffers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10Buffer>,
    ) {
        down_list::<ID3D11Buffer, _>(n, out, |o| {
            self.get10(|c| c.gs_get_constant_buffers(start, n, o))
        });
    }

    pub(super) fn vs_get_shader_resources10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10ShaderResourceView>,
    ) {
        down_list::<ID3D11ShaderResourceView, _>(n, out, |o| {
            self.get10(|c| c.vs_get_shader_resources(start, n, o))
        });
    }

    pub(super) fn ps_get_shader_resources10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10ShaderResourceView>,
    ) {
        down_list::<ID3D11ShaderResourceView, _>(n, out, |o| {
            self.get10(|c| c.ps_get_shader_resources(start, n, o))
        });
    }

    pub(super) fn gs_get_shader_resources10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10ShaderResourceView>,
    ) {
        down_list::<ID3D11ShaderResourceView, _>(n, out, |o| {
            self.get10(|c| c.gs_get_shader_resources(start, n, o))
        });
    }

    pub(super) fn vs_get_samplers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10SamplerState>,
    ) {
        down_list::<ID3D11SamplerState, _>(n, out, |o| {
            self.get10(|c| c.vs_get_samplers(start, n, o))
        });
    }

    pub(super) fn ps_get_samplers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10SamplerState>,
    ) {
        down_list::<ID3D11SamplerState, _>(n, out, |o| {
            self.get10(|c| c.ps_get_samplers(start, n, o))
        });
    }

    pub(super) fn gs_get_samplers10(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D10SamplerState>,
    ) {
        down_list::<ID3D11SamplerState, _>(n, out, |o| {
            self.get10(|c| c.gs_get_samplers(start, n, o))
        });
    }

    pub(super) fn vs_get_shader10(&self, out: OutRef<ID3D10VertexShader>) {
        down::<ID3D11VertexShader, _>(out, |o| {
            self.get10(|c| c.vs_get_shader(o, OutRef::default(), std::ptr::null_mut()))
        });
    }

    pub(super) fn ps_get_shader10(&self, out: OutRef<ID3D10PixelShader>) {
        down::<ID3D11PixelShader, _>(out, |o| {
            self.get10(|c| c.ps_get_shader(o, OutRef::default(), std::ptr::null_mut()))
        });
    }

    pub(super) fn gs_get_shader10(&self, out: OutRef<ID3D10GeometryShader>) {
        down::<ID3D11GeometryShader, _>(out, |o| {
            self.get10(|c| c.gs_get_shader(o, OutRef::default(), std::ptr::null_mut()))
        });
    }

    pub(super) fn ia_get_input_layout10(&self, out: OutRef<ID3D10InputLayout>) {
        down::<ID3D11InputLayout, _>(out, |o| self.get10(|c| c.ia_get_input_layout(o)));
    }

    pub(super) fn ia_get_vertex_buffers10(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D10Buffer>,
        strides: *mut u32,
        offsets: *mut u32,
    ) {
        let out = array(out);
        let mut got: Vec<Option<ID3D11Buffer>> = vec![None; n as usize];
        let list = if out.is_null() {
            OutRef::default()
        } else {
            // SAFETY: as `array`, the other way: a pointer to `n` slots.
            unsafe {
                std::mem::transmute::<*mut Option<ID3D11Buffer>, OutRef<ID3D11Buffer>>(
                    got.as_mut_ptr(),
                )
            }
        };
        self.get10(|c| c.ia_get_vertex_buffers(start, n, list, strides, offsets));
        if !out.is_null() {
            for (i, g) in got.into_iter().enumerate() {
                unsafe { out.add(i).write(g.and_then(|g| g.cast().ok())) };
            }
        }
    }

    pub(super) fn ia_get_index_buffer10(
        &self,
        out: OutRef<ID3D10Buffer>,
        format: *mut DXGI_FORMAT,
        offset: *mut u32,
    ) {
        let mut b: Option<ID3D11Buffer> = None;
        let mine = if out.is_null() {
            OutRef::default()
        } else {
            OutRef::from(&mut b)
        };
        self.get10(|c| c.ia_get_index_buffer(mine, format, offset));
        if !out.is_null() {
            let _ = out.write(b.and_then(|b| b.cast().ok()));
        }
    }

    pub(super) fn ia_get_primitive_topology10(&self, out: *mut D3D_PRIMITIVE_TOPOLOGY) {
        self.get10(|c| c.ia_get_primitive_topology(out));
    }

    pub(super) fn get_predication10(&self, out: OutRef<ID3D10Predicate>, value: *mut BOOL) {
        down::<ID3D11Predicate, _>(out, |o| self.on10(|c| c.get_predication(o, value)));
    }

    pub(super) fn om_get_render_targets10(
        &self,
        n: u32,
        targets: *mut Option<ID3D10RenderTargetView>,
        depth: OutRef<ID3D10DepthStencilView>,
    ) {
        let mut d: Option<ID3D11DepthStencilView> = None;
        let wants_depth = !depth.is_null();
        down_list::<ID3D11RenderTargetView, _>(n, targets, |t| {
            let mine = if wants_depth {
                OutRef::from(&mut d)
            } else {
                OutRef::default()
            };
            self.get10(|c| c.om_get_render_targets(n, t, mine))
        });
        if targets.is_null() {
            let mine = if wants_depth {
                OutRef::from(&mut d)
            } else {
                OutRef::default()
            };
            self.get10(|c| c.om_get_render_targets(0, std::ptr::null_mut(), mine));
        }
        if wants_depth {
            let _ = depth.write(d.and_then(|d| d.cast().ok()));
        }
    }

    pub(super) fn om_get_blend_state10(
        &self,
        out: OutRef<ID3D10BlendState>,
        factor: *mut f32,
        mask: *mut u32,
    ) {
        let mut s: Option<ID3D11BlendState> = None;
        let wants = !out.is_null();
        let mine = if wants {
            OutRef::from(&mut s)
        } else {
            OutRef::default()
        };
        self.get10(|c| c.om_get_blend_state(mine, factor, mask));
        if wants {
            let _ = out.write(s.and_then(|s| s.cast().ok()));
        }
    }

    pub(super) fn om_get_depth_stencil_state10(
        &self,
        out: OutRef<ID3D10DepthStencilState>,
        reference: *mut u32,
    ) {
        let mut s: Option<ID3D11DepthStencilState> = None;
        let wants = !out.is_null();
        let mine = if wants {
            OutRef::from(&mut s)
        } else {
            OutRef::default()
        };
        self.get10(|c| c.om_get_depth_stencil_state(mine, reference));
        if wants {
            let _ = out.write(s.and_then(|s| s.cast().ok()));
        }
    }

    pub(super) fn so_get_targets10(&self, n: u32, out: OutRef<ID3D10Buffer>, offsets: *mut u32) {
        down_list::<ID3D11Buffer, _>(n, array(out), |o| self.get10(|c| c.so_get_targets(n, o)));
        if !offsets.is_null() {
            self.get10(|c| c.so_get_offsets(n, offsets));
        }
    }

    pub(super) fn rs_get_state10(&self, out: OutRef<ID3D10RasterizerState>) {
        down::<ID3D11RasterizerState, _>(out, |o| self.get10(|c| c.rs_get_state(o)));
    }

    pub(super) fn rs_get_viewports10(&self, count: *mut u32, out: *mut D3D10_VIEWPORT) {
        let Some(n) = (unsafe { count.as_ref() }).copied() else {
            return;
        };
        let mut v = vec![D3D11_VIEWPORT::default(); if out.is_null() { 0 } else { n as usize }];
        let p = if out.is_null() {
            std::ptr::null_mut()
        } else {
            v.as_mut_ptr()
        };
        self.get10(|c| c.rs_get_viewports(count, p));
        // Every entry asked for is written, zero past those bound, as Direct3D 11's getter does.
        if !out.is_null() {
            for (i, v) in v.iter().enumerate() {
                unsafe { out.add(i).write(viewport10(v)) };
            }
        }
    }

    pub(super) fn rs_get_scissor_rects10(&self, count: *mut u32, out: *mut RECT) {
        self.get10(|c| c.rs_get_scissor_rects(count, out));
    }

    pub(super) fn clear_state10(&self) {
        self.on10(|c| c.clear_state());
    }

    pub(super) fn flush10(&self) {
        self.on10(|c| c.flush());
    }

    pub(super) fn create_buffer10(
        &self,
        desc: *const D3D10_BUFFER_DESC,
        init: *const D3D10_SUBRESOURCE_DATA,
        out: OutRef<ID3D10Buffer>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let d = D3D11_BUFFER_DESC {
            ByteWidth: d.ByteWidth,
            Usage: D3D11_USAGE(d.Usage.0),
            BindFlags: d.BindFlags,
            CPUAccessFlags: d.CPUAccessFlags,
            MiscFlags: misc11(d.MiscFlags),
            StructureByteStride: 0,
        };
        as_one(out, |o| self.create_buffer(&d, init.cast(), o))
    }

    pub(super) fn create_texture1_d10(
        &self,
        desc: *const D3D10_TEXTURE1D_DESC,
        init: *const D3D10_SUBRESOURCE_DATA,
    ) -> Result<ID3D10Texture1D> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let mut d: D3D11_TEXTURE1D_DESC = unsafe { convert::same_fields(d) };
        d.MiscFlags = misc11(d.MiscFlags);
        d.CPUAccessFlags = cpu_access10(d.Usage, d.BindFlags, d.CPUAccessFlags);
        let mut t = None;
        self.create_texture1_d(&d, init.cast(), OutRef::from(&mut t))?;
        t.ok_or(E_INVALIDARG)?.cast()
    }

    pub(super) fn create_texture2_d10(
        &self,
        desc: *const D3D10_TEXTURE2D_DESC,
        init: *const D3D10_SUBRESOURCE_DATA,
    ) -> Result<ID3D10Texture2D> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let mut d: D3D11_TEXTURE2D_DESC = unsafe { convert::same_fields(d) };
        d.MiscFlags = misc11(d.MiscFlags);
        d.CPUAccessFlags = cpu_access10(d.Usage, d.BindFlags, d.CPUAccessFlags);
        let mut t = None;
        self.create_texture2_d(&d, init.cast(), OutRef::from(&mut t))?;
        t.ok_or(E_INVALIDARG)?.cast()
    }

    pub(super) fn create_texture3_d10(
        &self,
        desc: *const D3D10_TEXTURE3D_DESC,
        init: *const D3D10_SUBRESOURCE_DATA,
    ) -> Result<ID3D10Texture3D> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let mut d: D3D11_TEXTURE3D_DESC = unsafe { convert::same_fields(d) };
        d.MiscFlags = misc11(d.MiscFlags);
        d.CPUAccessFlags = cpu_access10(d.Usage, d.BindFlags, d.CPUAccessFlags);
        let mut t = None;
        self.create_texture3_d(&d, init.cast(), OutRef::from(&mut t))?;
        t.ok_or(E_INVALIDARG)?.cast()
    }

    /// Direct3D 10's shader resource view descriptions are Direct3D 11's without `BUFFEREX`, in
    /// the same layout.
    pub(super) fn create_shader_resource_view10(
        &self,
        resource: Ref<ID3D10Resource>,
        desc: *const D3D10_SHADER_RESOURCE_VIEW_DESC,
        out: OutRef<ID3D10ShaderResourceView>,
    ) -> Result<()> {
        let d: Option<D3D11_SHADER_RESOURCE_VIEW_DESC> =
            unsafe { desc.as_ref() }.map(|d| unsafe { convert::same_fields(d) });
        let r = one::<_, ID3D11Resource>(resource);
        as_one(out, |o| {
            self.create_shader_resource_view(
                Ref::from(&r),
                d.as_ref().map_or(std::ptr::null(), |d| d as *const _),
                o,
            )
        })
    }

    pub(super) fn create_shader_resource_view110(
        &self,
        resource: Ref<ID3D10Resource>,
        desc: *const D3D10_SHADER_RESOURCE_VIEW_DESC1,
        out: OutRef<ID3D10ShaderResourceView1>,
    ) -> Result<()> {
        let d: Option<D3D11_SHADER_RESOURCE_VIEW_DESC> =
            unsafe { desc.as_ref() }.map(|d| unsafe { convert::same_fields(d) });
        let r = one::<_, ID3D11Resource>(resource);
        as_one(out, |o| {
            self.create_shader_resource_view(
                Ref::from(&r),
                d.as_ref().map_or(std::ptr::null(), |d| d as *const _),
                o,
            )
        })
    }

    pub(super) fn create_render_target_view10(
        &self,
        resource: Ref<ID3D10Resource>,
        desc: *const D3D10_RENDER_TARGET_VIEW_DESC,
        out: OutRef<ID3D10RenderTargetView>,
    ) -> Result<()> {
        let d: Option<D3D11_RENDER_TARGET_VIEW_DESC> =
            unsafe { desc.as_ref() }.map(|d| unsafe { convert::same_fields(d) });
        let r = one::<_, ID3D11Resource>(resource);
        as_one(out, |o| {
            self.create_render_target_view(
                Ref::from(&r),
                d.as_ref().map_or(std::ptr::null(), |d| d as *const _),
                o,
            )
        })
    }

    /// Direct3D 11's depth-stencil view description has its flags between the dimension and the
    /// union; Direct3D 10's has none.
    pub(super) fn create_depth_stencil_view10(
        &self,
        resource: Ref<ID3D10Resource>,
        desc: *const D3D10_DEPTH_STENCIL_VIEW_DESC,
        out: OutRef<ID3D10DepthStencilView>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.map(|d| D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: d.Format,
            ViewDimension: D3D11_DSV_DIMENSION(d.ViewDimension.0),
            Flags: 0,
            Anonymous: unsafe { convert::same_fields(&d.Anonymous) },
        });
        let r = one::<_, ID3D11Resource>(resource);
        as_one(out, |o| {
            self.create_depth_stencil_view(
                Ref::from(&r),
                d.as_ref().map_or(std::ptr::null(), |d| d as *const _),
                o,
            )
        })
    }

    pub(super) fn create_input_layout10(
        &self,
        elements: *const D3D10_INPUT_ELEMENT_DESC,
        count: u32,
        bytecode: *const c_void,
        length: usize,
        out: OutRef<ID3D10InputLayout>,
    ) -> Result<()> {
        as_one(out, |o| {
            self.create_input_layout(elements.cast(), count, bytecode, length, o)
        })
    }

    pub(super) fn create_vertex_shader10(
        &self,
        bytecode: *const c_void,
        length: usize,
        out: OutRef<ID3D10VertexShader>,
    ) -> Result<()> {
        as_one(out, |o| {
            self.create_vertex_shader(bytecode, length, Ref::default(), o)
        })
    }

    pub(super) fn create_pixel_shader10(
        &self,
        bytecode: *const c_void,
        length: usize,
        out: OutRef<ID3D10PixelShader>,
    ) -> Result<()> {
        as_one(out, |o| {
            self.create_pixel_shader(bytecode, length, Ref::default(), o)
        })
    }

    pub(super) fn create_geometry_shader10(
        &self,
        bytecode: *const c_void,
        length: usize,
        out: OutRef<ID3D10GeometryShader>,
    ) -> Result<()> {
        as_one(out, |o| {
            self.create_geometry_shader(bytecode, length, Ref::default(), o)
        })
    }

    /// Direct3D 10's entries are stream 0's; each buffer's stride is `stride` where every entry
    /// writes the first buffer, else the components its entries write.
    pub(super) fn create_geometry_shader_with_stream_output10(
        &self,
        bytecode: *const c_void,
        length: usize,
        entries: *const D3D10_SO_DECLARATION_ENTRY,
        count: u32,
        stride: u32,
        out: OutRef<ID3D10GeometryShader>,
    ) -> Result<()> {
        let list = if entries.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(entries, count as usize) }
        };
        let e: Vec<D3D11_SO_DECLARATION_ENTRY> = list
            .iter()
            .map(|e| D3D11_SO_DECLARATION_ENTRY {
                Stream: 0,
                SemanticName: e.SemanticName,
                SemanticIndex: e.SemanticIndex,
                StartComponent: e.StartComponent,
                ComponentCount: e.ComponentCount,
                OutputSlot: e.OutputSlot,
            })
            .collect();
        // The stride is the first buffer's, which alone is written; entries naming other buffers
        // are packed into them, the stride then zero. Without entries and stride the shader
        // writes no stream.
        let several = list.iter().any(|e| e.OutputSlot != 0);
        if several && stride != 0 {
            return Err(E_INVALIDARG.into());
        }
        let strides: Vec<u32> = if several || (list.is_empty() && stride == 0) {
            Vec::new()
        } else {
            vec![stride]
        };
        as_one(out, |o| {
            self.create_geometry_shader_with_stream_output(
                bytecode,
                length,
                if entries.is_null() {
                    std::ptr::null()
                } else {
                    e.as_ptr()
                },
                count,
                if strides.is_empty() {
                    std::ptr::null()
                } else {
                    strides.as_ptr()
                },
                strides.len() as u32,
                0,
                Ref::default(),
                o,
            )
        })
    }

    pub(super) fn create_blend_state10(
        &self,
        desc: *const D3D10_BLEND_DESC,
        out: OutRef<ID3D10BlendState>,
    ) -> Result<()> {
        let d = blend11(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        as_one(out, |o| self.create_blend_state(&d, o))
    }

    /// Direct3D 10.1's blend description is Direct3D 11's, in the same layout.
    pub(super) fn create_blend_state110(
        &self,
        desc: *const D3D10_BLEND_DESC1,
        out: OutRef<ID3D10BlendState1>,
    ) -> Result<()> {
        let d: D3D11_BLEND_DESC =
            unsafe { convert::same_fields(desc.as_ref().ok_or(E_INVALIDARG)?) };
        as_one(out, |o| self.create_blend_state(&d, o))
    }

    pub(super) fn create_depth_stencil_state10(
        &self,
        desc: *const D3D10_DEPTH_STENCIL_DESC,
        out: OutRef<ID3D10DepthStencilState>,
    ) -> Result<()> {
        as_one(out, |o| self.create_depth_stencil_state(desc.cast(), o))
    }

    pub(super) fn create_rasterizer_state10(
        &self,
        desc: *const D3D10_RASTERIZER_DESC,
        out: OutRef<ID3D10RasterizerState>,
    ) -> Result<()> {
        as_one(out, |o| self.create_rasterizer_state(desc.cast(), o))
    }

    pub(super) fn create_sampler_state10(
        &self,
        desc: *const D3D10_SAMPLER_DESC,
        out: OutRef<ID3D10SamplerState>,
    ) -> Result<()> {
        as_one(out, |o| self.create_sampler_state(desc.cast(), o))
    }

    pub(super) fn create_query10(
        &self,
        desc: *const D3D10_QUERY_DESC,
        out: OutRef<ID3D10Query>,
    ) -> Result<()> {
        as_one(out, |o| self.create_query(desc.cast(), o))
    }

    pub(super) fn create_predicate10(
        &self,
        desc: *const D3D10_QUERY_DESC,
        out: OutRef<ID3D10Predicate>,
    ) -> Result<()> {
        as_one(out, |o| self.create_predicate(desc.cast(), o))
    }

    /// The device's level, as Direct3D answers it (11_0 too, which Direct3D 10.1 does not name).
    pub(super) fn get_feature_level10(&self) -> D3D10_FEATURE_LEVEL1 {
        D3D10_FEATURE_LEVEL1(self.get_feature_level().0)
    }

    /// The size Direct3D's text filter is asked for (by D3DX10's fonts), kept and answered.
    pub(super) fn set_text_filter_size10(&self, width: u32, height: u32) {
        self.text_filter.set((width, height));
    }

    pub(super) fn get_text_filter_size10(&self, width: *mut u32, height: *mut u32) {
        let (w, h) = self.text_filter.get();
        if let Some(o) = unsafe { width.as_mut() } {
            *o = w;
        }
        if let Some(o) = unsafe { height.as_mut() } {
            *o = h;
        }
    }

    pub(super) fn enter10(&self) {
        self.on_immediate(|c| c.enter());
    }

    pub(super) fn leave10(&self) {
        self.on_immediate(|c| c.leave());
    }

    pub(super) fn set_multithread_protected10(&self, on: BOOL) -> BOOL {
        let mut was = BOOL(0);
        self.on_immediate(|c| was = c.set_multithread_protected(on));
        was
    }

    pub(super) fn get_multithread_protected10(&self) -> BOOL {
        let mut on = BOOL(0);
        self.on_immediate(|c| on = c.get_multithread_protected());
        on
    }

    /// Direct3D 10's device holds no reference on what it binds: an object the program released
    /// for the last time leaves every slot of the immediate context.
    pub(super) fn unbind_released(&self, address: usize) {
        if !self.immediate_kept.emulates10() {
            return;
        }
        let _lock = lock();
        self.on_immediate(|c| c.unbind_released(address));
    }

    fn on_immediate(&self, f: impl FnOnce(&Context)) {
        let c = self.immediate();
        f(c.cast_object_ref::<Context>().expect("ours"));
    }

    /// Direct3D 10's asynchronous calls on a query of this device, run on the immediate context
    /// while Direct3D 10's behaviour is active.
    pub(super) fn begin10(&self, a: &ID3D11Asynchronous) {
        self.on10(|c| c.begin(Ref::from(&Some(a.clone()))));
    }

    pub(super) fn end10(&self, a: &ID3D11Asynchronous) {
        self.on10(|c| c.end(Ref::from(&Some(a.clone()))));
    }

    pub(super) fn get_data10(
        &self,
        a: &ID3D11Asynchronous,
        data: *mut c_void,
        size: u32,
        flags: u32,
    ) -> Result<()> {
        let mut r = Err(E_INVALIDARG.into());
        self.on10(|c| r = c.get_data(Ref::from(&Some(a.clone())), data, size, flags));
        r
    }

    pub(super) fn map10(
        &self,
        resource: &ID3D11Resource,
        sub: u32,
        kind: D3D10_MAP,
        flags: u32,
        out: *mut D3D11_MAPPED_SUBRESOURCE,
    ) -> Result<()> {
        let mut r = Err(E_INVALIDARG.into());
        self.on10(|c| {
            r = c.map(
                Ref::from(&Some(resource.clone())),
                sub,
                D3D11_MAP(kind.0),
                flags,
                out,
            )
        });
        r
    }

    pub(super) fn unmap10(&self, resource: &ID3D11Resource, sub: u32) {
        self.on10(|c| c.unmap(Ref::from(&Some(resource.clone())), sub));
    }
}

/// Whether a copy goes into a depth-stencil texture from one that is not: Direct3D 10's device
/// copies nothing then, where Direct3D 11's copies the bits.
fn into_depth_from_colour(dst: &Option<ID3D11Resource>, src: &Option<ID3D11Resource>) -> bool {
    let depth = |r: &Option<ID3D11Resource>| match r.as_ref().and_then(super::objects::resource_of)
    {
        Some(wcontainers_direct3d_11::object::Resource::Texture(t)) => {
            t.desc.bind_flags & D3D11_BIND_DEPTH_STENCIL.0 as u32 != 0
        }
        _ => false,
    };
    depth(dst) && !depth(src)
}
