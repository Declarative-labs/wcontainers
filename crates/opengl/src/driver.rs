//! The driver on the Gallium trait (`wcontainers-gallium`): the screen, [`Driver`], and its
//! contexts, [`DriverContext`], in safe Rust over a `Gpu` back end. The move from the bindings is
//! made state by state: the kinds and calls not yet moved are [`Unmoved`], a kind no value has, and
//! a call that would make one is refused by name, as a call the driver does not take.

use std::cell::RefCell;
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use wcontainers_gallium::{
    BlendColor, BlendState, BlitInfo, Caps as GalliumCaps, ClipState, ComputeCaps, ComputeState,
    ConstantBuffer, DepthStencilAlphaState, DrawIndirectInfo, DrawInfo, DrawStartCountBias,
    FramebufferState, Gallium, GalliumContext, GridInfo, ImageView, Mapped, PipeBox, PolyStipple,
    RasterizerState, ResourceTemplate, SamplerState, SamplerViewTemplate, ScissorState,
    ShaderBuffer, ShaderCaps, ShaderState, StencilRef, Surface, VertexBuffer, VertexElement,
    ViewportState, values,
};
use wcontainers_gpu::{
    ClearValue, Gpu, Object, caps::Caps, caps::Features, draw::ScissorRect, error::DeviceStatus,
    passes::PassTracker,
};
use wcontainers_mesa_compiler::MesaCompiler;
use wcontainers_mesa_compiler_direct::Bridge;

use crate::device::HostWindows;
use crate::framebuffer::{self, Framebuffer};
use crate::guest::{self, guest_call, refuse};
use crate::pipe::*;
use crate::resources::{self, Mapping, Resource};
use crate::screen::{DRIVER_NAME, whole};
use crate::shaders::{self, Shader};
use crate::state::Stage;
use crate::states::{self, Blend, DepthStencil, Elements, Raster, Retired, Sampler, retire};
use crate::views::{self, SamplerView};
use wcontainers_nir::mesa::Staged;

/// A kind of the Gallium trait whose calls are not yet moved onto it: no value of it is ever made,
/// so every call taking one cannot be made, and every call making one is refused.
#[derive(Clone, Debug)]
pub enum Unmoved {}

/// Refuses a call whose part of the driver is not yet on the Gallium trait (GI8 part B), by name.
fn not_moved(call: &str) -> ! {
    refuse(format_args!("{call}: not yet moved onto the Gallium trait"))
}

/// What the screen and every context made on it share: the back end and what is given back to it.
pub struct Shared<B: Gpu> {
    backend: Mutex<B>,
    /// Objects whose holders went, given back to the back end when it is next used.
    retired: Retired<B>,
    caps: Caps,
    /// The device's name, as OpenGL's renderer string ends.
    name: CString,
}

impl<B: Gpu> Shared<B> {
    /// The back end, for one call of the driver's. Once its device is lost the call is refused
    /// here, so that no call of any context records work for a lost device.
    pub fn backend(&self) -> MutexGuard<'_, B> {
        let mut backend = whole(&self.backend);
        if let DeviceStatus::Lost(reason) = backend.status() {
            drop(backend);
            guest::DEVICE_LOST.store(true, Ordering::Relaxed);
            refuse(format_args!("the device was lost: {reason:?}"));
        }
        let retired = std::mem::take(&mut *whole(&self.retired));
        for object in retired {
            backend.destroy(object);
        }
        crate::screen::warn_slow_paths(&mut *backend);
        backend
    }
}

/// The screen: Gallium's view of the device over the back end `B`.
pub struct Driver<B: HostWindows> {
    shared: Arc<Shared<B>>,
}

impl<B: HostWindows> Driver<B> {
    /// The screen over `backend`, whose device is named `name`.
    pub fn new(backend: B, name: CString) -> Self {
        let caps = backend.caps().clone();
        Driver {
            shared: Arc::new(Shared {
                backend: Mutex::new(backend),
                retired: Arc::new(Mutex::new(Vec::new())),
                caps,
                name,
            }),
        }
    }
}

/// The screen on the GPU the back end finds, or none (Mesa then tries its next driver).
pub fn driver() -> Option<Driver<wcontainers_gpu_webgpu::WebGpu>> {
    guest_call(guest::SCREEN, None, || {
        crate::device::webgpu().map(|(backend, name)| Driver::new(backend, name))
    })
}

/// The keys of contexts, each made once in the process's life (0 is the screen's).
static NEXT_CONTEXT: AtomicUsize = AtomicUsize::new(guest::SCREEN + 1);

/// The state st/mesa sets between draws that the trait's calls moved so far keep.
struct Set<B: Gpu> {
    blend: Option<Arc<Blend>>,
    depth_stencil: Option<Arc<DepthStencil<B>>>,
    raster: Option<Arc<Raster<B>>>,
    elements: Option<Arc<Elements<B>>>,
    /// Each stage's samplers, by slot.
    samplers: [Vec<Option<Arc<Sampler<B>>>>; Stage::ALL.len()],
    /// Each stage's sampler views, by slot.
    views: [Vec<Option<Arc<SamplerView<B>>>>; Stage::ALL.len()],
    /// Each stage's bound program.
    shaders: [Option<Arc<Shader>>; Stage::ALL.len()],
    blend_color: [u32; states::COLOR_CHANNELS],
    stencil_ref: [u8; 2],
    sample_mask: u32,
    viewport: Option<ViewportState>,
    scissor: Option<ScissorState>,
    /// The control points of a patch (`set_patch_vertices`).
    patch_vertices: u8,
    /// The passes recorded, closed where later work must see what they wrote.
    tracker: PassTracker<B>,
    framebuffer: Framebuffer<B>,
}

impl<B: Gpu> Default for Set<B> {
    fn default() -> Self {
        Set {
            blend: None,
            depth_stencil: None,
            raster: None,
            elements: None,
            samplers: std::array::from_fn(|_| vec![None; PIPE_MAX_SAMPLERS as usize]),
            views: std::array::from_fn(|_| vec![None; PIPE_MAX_SHADER_SAMPLER_VIEWS as usize]),
            shaders: Default::default(),
            blend_color: [0; states::COLOR_CHANNELS],
            stencil_ref: [0; 2],
            sample_mask: u32::MAX,
            viewport: None,
            scissor: None,
            patch_vertices: 0,
            tracker: PassTracker::default(),
            framebuffer: Framebuffer::default(),
        }
    }
}

/// One context: the state st/mesa sets and the work it records, on the screen it was made on.
pub struct DriverContext<B: HostWindows> {
    shared: Arc<Shared<B>>,
    /// Its key among the guest calls' contexts ([`guest::guest_call`]).
    key: usize,
    set: RefCell<Set<B>>,
}

impl<B: HostWindows> DriverContext<B> {
    /// Runs one call of this context as a guest call ([`guest::guest_call`]).
    fn call<T>(&self, dropped: T, call: impl FnOnce() -> T) -> T {
        guest_call(self.key, dropped, call)
    }

    /// Closes the open pass, as every read back, copy and texture write needs (a target's clears
    /// wait in the pass tracker for its next pass, which must come first).
    fn close_pass(&self, backend: &mut B) {
        if let Err(error) = self.set.borrow_mut().tracker.close(backend) {
            refuse(format_args!("closing a pass: {error:?}"));
        }
    }

    /// Maps a box of one level of `resource`, reading it back first where the map reads.
    fn map(
        &self,
        resource: &Arc<Resource<B>>,
        level: u32,
        usage: u32,
        area: &PipeBox,
    ) -> Option<Mapped<Mapping<B>>> {
        let mut backend = self.shared.backend();
        if usage & PIPE_MAP_READ as u32 != 0 {
            self.close_pass(&mut backend);
        }
        let mapped = resources::map(&mut *backend, resource, level, usage, area);
        if mapped.is_none() {
            crate::context::end_if_lost(&mut *backend);
        }
        mapped
    }

    /// Clears a rectangle of one surface, outside any pass.
    fn clear_surface(
        &self,
        surface: &Surface<'_, Resource<B>>,
        value: ClearValue,
        rect: ScissorRect,
    ) {
        let Some(surface) = framebuffer::Attached::of(surface) else {
            return;
        };
        let mut backend = self.shared.backend();
        self.close_pass(&mut backend);
        let Some((view, format)) =
            framebuffer::surface_view(&mut *backend, &surface, surface.first_layer)
        else {
            refuse(format_args!(
                "a clear of a surface of no texture or of a format the device lacks"
            ));
        };
        let cleared = backend.clear_rects(&view, format, value, &[rect]);
        backend.destroy(Object::TextureView(view));
        if let Err(error) = cleared {
            refuse(format_args!("a clear: {error:?}"));
        }
    }

    /// Writes what `mapping` mapped for writing from `written`, unless the caller flushed its parts
    /// itself.
    fn unmap(&self, mapping: &Mapping<B>, written: &[u8]) {
        if !mapping.unmap_writes() {
            return;
        }
        let mut backend = self.shared.backend();
        if mapping.resource().texture().is_some() {
            self.close_pass(&mut backend);
        }
        resources::write_mapped(
            &mut *backend,
            &self.shared.retired,
            mapping,
            written,
            &mapping.whole(),
        );
    }
}

/// The f32 bits of a depth clear value.
fn depth_bits(depth: f64) -> u32 {
    (depth as f32).to_bits()
}

/// The stencil bits of a clear value.
fn stencil_bits(stencil: u32) -> u8 {
    (stencil & u32::from(u8::MAX)) as u8
}

/// The driver's stage of a Gallium stage, for the stages it takes.
pub fn stage(stage: values::MesaShaderStage) -> Option<Stage> {
    use values::MesaShaderStage as S;
    match stage {
        S::MESA_SHADER_VERTEX => Some(Stage::Vertex),
        S::MESA_SHADER_FRAGMENT => Some(Stage::Fragment),
        S::MESA_SHADER_COMPUTE => Some(Stage::Compute),
        S::MESA_SHADER_GEOMETRY => Some(Stage::Geometry),
        S::MESA_SHADER_TESS_CTRL => Some(Stage::TessControl),
        S::MESA_SHADER_TESS_EVAL => Some(Stage::TessEvaluation),
        _ => None,
    }
}

impl<B: HostWindows> Gallium for Driver<B> {
    type Compiler = Bridge;
    type Context = DriverContext<B>;
    type Resource = Resource<B>;
    type Fence = Unmoved;
    type Blend = Arc<Blend>;
    type DepthStencilAlpha = Arc<DepthStencil<B>>;
    type Rasterizer = Arc<Raster<B>>;
    type Sampler = Arc<Sampler<B>>;
    type VertexElements = Arc<Elements<B>>;
    type VertexShader = Arc<Shader>;
    type FragmentShader = Arc<Shader>;
    type GeometryShader = Arc<Shader>;
    type TessControlShader = Arc<Shader>;
    type TessEvaluationShader = Arc<Shader>;
    type ComputeShader = Arc<Shader>;
    type SamplerView = SamplerView<B>;
    type StreamOutputTarget = Unmoved;
    type Query = Unmoved;
    type ActiveQuery = Unmoved;
    type EndOnlyQuery = Unmoved;
    type Mapping = Mapping<B>;

    fn context_create(&self, _: u64, _: u32) -> Option<Self::Context> {
        Some(DriverContext {
            shared: Arc::clone(&self.shared),
            key: NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed),
            set: RefCell::new(Set::default()),
        })
    }

    /// Reports every slow path the device took, with how many times, at the end of its life.
    fn destroy(&self) {
        guest_call(guest::SCREEN, (), || {
            let mut backend = self.shared.backend();
            for (reason, count) in crate::screen::slow_path_counts(&mut *backend) {
                eprintln!(
                    "{}: slow path taken {count} times: {reason}",
                    DRIVER_NAME.to_string_lossy()
                );
            }
        });
    }

    fn fence_finish(&self, _: Option<&Self::Context>, fence: &Arc<Self::Fence>, _: u64) -> bool {
        match **fence {}
    }

    fn flush_frontbuffer(
        &self,
        _: Option<&Self::Context>,
        _: &Arc<Resource<B>>,
        _: u32,
        _: u32,
        _: u64,
        _: Option<&PipeBox>,
    ) {
        guest_call(guest::SCREEN, (), || not_moved("flush_frontbuffer"));
    }

    fn get_device_vendor(&self) -> Vec<u8> {
        self.shared.name.to_bytes().to_vec()
    }

    fn get_name(&self) -> Vec<u8> {
        DRIVER_NAME.to_bytes().to_vec()
    }

    fn get_vendor(&self) -> Vec<u8> {
        DRIVER_NAME.to_bytes().to_vec()
    }

    fn is_format_supported(
        &self,
        format: values::Format,
        target: values::TextureTarget,
        sample_count: u32,
        storage_sample_count: u32,
        bindings: u32,
    ) -> bool {
        guest_call(guest::SCREEN, false, || {
            crate::format::supported(
                &self.shared.caps.formats,
                format.0,
                target.0,
                sample_count,
                storage_sample_count,
                bindings,
            )
        })
    }

    /// The interface's limits over Mesa's defaults ([`crate::caps::fill`]).
    fn caps(&self, caps: &mut GalliumCaps) {
        crate::caps::fill(caps, &self.shared.caps);
    }

    /// The stages the driver takes programs for; the geometry and tessellation stages only where
    /// the back end runs compute work, which the stage helper runs them as.
    fn shader_caps(&self, stage: values::MesaShaderStage, caps: &mut ShaderCaps) {
        let staged = self.shared.caps.features.contains(Features::COMPUTE);
        if self::stage(stage).is_some() && (staged || !crate::caps::STAGED.contains(&stage)) {
            crate::caps::fill_shader(caps, stage, &self.shared.caps);
        }
    }

    /// Compute work's limits, where the back end runs it.
    fn compute_caps(&self, caps: &mut ComputeCaps) {
        if self.shared.caps.features.contains(Features::COMPUTE) {
            crate::caps::fill_compute(caps, &self.shared.caps);
        }
    }

    fn resource_create(&self, template: &ResourceTemplate) -> Option<Resource<B>> {
        guest_call(guest::SCREEN, None, || {
            let mut backend = self.shared.backend();
            resources::create(
                &mut *backend,
                &self.shared.caps,
                &self.shared.retired,
                template,
            )
        })
    }
}

/// The compiler whose received programs the shader states are made from.
type Compiled = Bridge;

impl<B: HostWindows> GalliumContext for DriverContext<B> {
    type Screen = Driver<B>;

    fn begin_query(&self, q: Unmoved) -> Result<Unmoved, Unmoved> {
        match q {}
    }

    fn bind_blend_state(&self, state: Option<&Arc<Blend>>) {
        self.set.borrow_mut().blend = state.cloned();
    }

    fn bind_compute_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::Compute as usize] = state.cloned();
    }

    fn bind_depth_stencil_alpha_state(&self, state: Option<&Arc<DepthStencil<B>>>) {
        self.set.borrow_mut().depth_stencil = state.cloned();
    }

    fn bind_fs_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::Fragment as usize] = state.cloned();
    }

    fn bind_gs_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::Geometry as usize] = state.cloned();
    }

    fn bind_rasterizer_state(&self, state: Option<&Arc<Raster<B>>>) {
        self.set.borrow_mut().raster = state.cloned();
    }

    fn bind_sampler_states(
        &self,
        shader: values::MesaShaderStage,
        start_slot: u32,
        samplers: &[Option<&Arc<Sampler<B>>>],
    ) {
        self.call((), || {
            let Some(stage) = stage(shader) else {
                return;
            };
            let mut set = self.set.borrow_mut();
            let slots = &mut set.samplers[stage as usize];
            let start = start_slot as usize;
            let Some(slots) = slots.get_mut(start..start + samplers.len()) else {
                refuse(format_args!(
                    "samplers past the {} the screen reports",
                    PIPE_MAX_SAMPLERS
                ));
            };
            for (slot, sampler) in slots.iter_mut().zip(samplers) {
                *slot = sampler.cloned();
            }
        });
    }

    fn bind_tcs_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::TessControl as usize] = state.cloned();
    }

    fn bind_tes_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::TessEvaluation as usize] = state.cloned();
    }

    fn bind_vertex_elements_state(&self, state: Option<&Arc<Elements<B>>>) {
        self.set.borrow_mut().elements = state.cloned();
    }

    fn bind_vs_state(&self, state: Option<&Arc<Shader>>) {
        self.set.borrow_mut().shaders[Stage::Vertex as usize] = state.cloned();
    }

    fn blit(&self, _: &BlitInfo<'_, Resource<B>>) {
        self.call((), || not_moved("blit"));
    }

    fn buffer_map(
        &self,
        resource: &Arc<Resource<B>>,
        level: u32,
        usage: values::MapFlags,
        area: &PipeBox,
    ) -> Option<Mapped<Mapping<B>>> {
        self.call(None, || self.map(resource, level, usage.0, area))
    }

    /// Written from the caller's bytes, with no copy before.
    fn buffer_subdata(
        &self,
        resource: &Arc<Resource<B>>,
        usage: values::MapFlags,
        offset: u32,
        size: u32,
        data: &[u8],
    ) {
        self.call((), || {
            let Some(buffer) = resource.buffer() else {
                return;
            };
            let Some(data) = data.get(..size as usize) else {
                refuse(format_args!(
                    "buffer_subdata of {size} bytes from {} given",
                    data.len()
                ));
            };
            self.shared.backend().write_buffer(
                buffer,
                u64::from(offset),
                data,
                resources::write_mode(usage.0),
            );
        });
    }

    /// Writes what was mapped for writing, unless the caller flushed its parts itself.
    fn buffer_unmap(&self, transfer: Mapping<B>, written: &[u8]) {
        self.call((), || self.unmap(&transfer, written));
    }

    /// Clears the framebuffer's targets `buffers` names, every layer of a layered one, a layer at
    /// a time. The render condition always holds: a query, which sets one, is not yet moved.
    fn clear(
        &self,
        buffers: u32,
        _: u32,
        _: u8,
        scissor: Option<&ScissorState>,
        color: &[u32; 4],
        depth: f64,
        stencil: u32,
    ) {
        self.call((), || {
            if scissor.is_some() {
                refuse(format_args!("a scissored clear (the screen reports none)"));
            }
            let mut backend = self.shared.backend();
            let mut set = self.set.borrow_mut();
            let set = &mut *set;
            let depth = (buffers & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth));
            let stencil = (buffers & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil));
            let layers = set.framebuffer.layer_count();
            let kept = set.tracker.targets().cloned();
            let mut made = Vec::new();
            let mut done = Ok(());
            for layer in 0..layers {
                if layer > 0 {
                    let mut twins = Vec::new();
                    let (targets, views, _) =
                        set.framebuffer
                            .layer_targets(&mut *backend, layer, &mut twins);
                    if !twins.is_empty() {
                        refuse(format_args!(
                            "a clear of a layered target drawn through a twin (not built)"
                        ));
                    }
                    made.extend(views);
                    done = done.and_then(|()| set.tracker.set_targets(&mut *backend, targets));
                }
                for target in 0..set.framebuffer.colors.len() {
                    if buffers & (PIPE_CLEAR_COLOR0 << target) != 0 {
                        done = done
                            .and_then(|()| set.tracker.clear_color(&mut *backend, target, *color));
                    }
                }
                if depth.is_some() || stencil.is_some() {
                    done = done.and_then(|()| {
                        set.tracker
                            .clear_depth_stencil(&mut *backend, depth, stencil)
                    });
                }
            }
            if layers > 1 {
                if let Some(kept) = kept {
                    done = done.and_then(|()| set.tracker.set_targets(&mut *backend, kept));
                }
                for view in made {
                    backend.destroy(Object::TextureView(view));
                }
            }
            if let Err(error) = done {
                refuse(format_args!("a clear: {error:?}"));
            }
            if let Err(error) = set
                .framebuffer
                .copy_back_twins(&mut set.tracker, &mut backend)
            {
                refuse(format_args!("a clear: {error}"));
            }
        });
    }

    /// The render condition always holds: a query, which sets one, is not yet moved.
    fn clear_depth_stencil(
        &self,
        dst: &Surface<'_, Resource<B>>,
        clear_flags: u32,
        depth: f64,
        stencil: u32,
        dstx: u32,
        dsty: u32,
        width: u32,
        height: u32,
        _: bool,
    ) {
        self.call((), || {
            let value = ClearValue::DepthStencil {
                depth: (clear_flags & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth)),
                stencil: (clear_flags & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil)),
            };
            let rect = ScissorRect {
                x: dstx,
                y: dsty,
                width,
                height,
            };
            self.clear_surface(dst, value, rect);
        });
    }

    /// The render condition always holds: a query, which sets one, is not yet moved.
    fn clear_render_target(
        &self,
        dst: &Surface<'_, Resource<B>>,
        color: &[u32; 4],
        dstx: u32,
        dsty: u32,
        width: u32,
        height: u32,
        _: bool,
    ) {
        self.call((), || {
            let rect = ScissorRect {
                x: dstx,
                y: dsty,
                width,
                height,
            };
            self.clear_surface(dst, ClearValue::Color(*color), rect);
        });
    }

    fn create_blend_state(&self, state: &BlendState) -> Option<Arc<Blend>> {
        self.call(None, || Some(Arc::new(states::blend(state))))
    }

    fn create_compute_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedCompute,
        _: &ComputeState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || Some(shaders::create(Staged::Compute(program))))
    }

    fn create_depth_stencil_alpha_state(
        &self,
        state: &DepthStencilAlphaState,
    ) -> Option<Arc<DepthStencil<B>>> {
        self.call(None, || {
            let mut backend = self.shared.backend();
            Some(Arc::new(states::depth_stencil(
                &mut *backend,
                &self.shared.retired,
                state,
            )))
        })
    }

    fn create_fs_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedFragment,
        _: &ShaderState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || Some(shaders::create(Staged::Fragment(program))))
    }

    fn create_gs_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedGeometry,
        _: &ShaderState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || Some(shaders::create(Staged::Geometry(program))))
    }

    fn create_query(&self, _: values::QueryType, _: u32) -> Option<Unmoved> {
        self.call(None, || not_moved("create_query"))
    }

    fn create_query_end_only_query(&self, _: values::QueryType, _: u32) -> Option<Unmoved> {
        self.call(None, || not_moved("create_query (end only)"))
    }

    fn create_rasterizer_state(&self, state: &RasterizerState) -> Option<Arc<Raster<B>>> {
        self.call(None, || {
            let mut backend = self.shared.backend();
            Some(Arc::new(states::raster(
                &mut *backend,
                &self.shared.retired,
                state,
            )))
        })
    }

    fn create_sampler_state(&self, state: &SamplerState) -> Option<Arc<Sampler<B>>> {
        self.call(None, || {
            let mut backend = self.shared.backend();
            Some(Arc::new(states::sampler(
                &mut *backend,
                &self.shared.retired,
                &self.shared.caps,
                state,
            )))
        })
    }

    fn create_sampler_view(
        &self,
        texture: &Arc<Resource<B>>,
        templ: &SamplerViewTemplate,
    ) -> Option<SamplerView<B>> {
        self.call(None, || {
            let mut backend = self.shared.backend();
            Some(views::create(
                &mut *backend,
                &self.shared.retired,
                texture,
                templ,
            ))
        })
    }

    fn create_stream_output_target(&self, _: &Arc<Resource<B>>, _: u32, _: u32) -> Option<Unmoved> {
        self.call(None, || not_moved("create_stream_output_target"))
    }

    fn create_tcs_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedTessControl,
        _: &ShaderState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || Some(shaders::create(Staged::TessControl(program))))
    }

    fn create_tes_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedTessEvaluation,
        _: &ShaderState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || {
            Some(shaders::create(Staged::TessEvaluation(program)))
        })
    }

    fn create_vertex_elements_state(&self, elements: &[VertexElement]) -> Option<Arc<Elements<B>>> {
        self.call(None, || {
            let mut backend = self.shared.backend();
            Some(Arc::new(states::elements(
                &mut *backend,
                &self.shared.retired,
                elements,
            )))
        })
    }

    fn create_vs_state(
        &self,
        program: <Compiled as MesaCompiler>::ReceivedVertex,
        _: &ShaderState,
    ) -> Option<Arc<Shader>> {
        self.call(None, || Some(shaders::create(Staged::Vertex(program))))
    }

    /// The state goes when its last holder drops it: a bound state stays until unbound.
    fn delete_blend_state(&self, state: Arc<Blend>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_compute_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    fn delete_depth_stencil_alpha_state(&self, state: Arc<DepthStencil<B>>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_fs_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_gs_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    fn delete_rasterizer_state(&self, state: Arc<Raster<B>>) {
        drop(state);
    }

    fn delete_sampler_state(&self, state: Arc<Sampler<B>>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_tcs_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_tes_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    fn delete_vertex_elements_state(&self, state: Arc<Elements<B>>) {
        drop(state);
    }

    /// The program goes when its last holder drops it: a bound program stays until unbound.
    fn delete_vs_state(&self, state: Arc<Shader>) {
        drop(state);
    }

    /// The context's states go with it; a destroyed context's reset is forgotten.
    fn destroy(&self) {
        let mut old = std::mem::take(&mut *self.set.borrow_mut());
        old.framebuffer
            .release(|object| retire(&self.shared.retired, object));
        guest::forget_reset(self.key);
    }

    fn destroy_query(&self, q: Unmoved) {
        match q {}
    }

    fn destroy_query_end_only_query(&self, q: Unmoved) {
        match q {}
    }

    fn destroy_query_active_query(&self, q: Unmoved) {
        match q {}
    }

    fn draw_vbo(
        &self,
        _: &DrawInfo<'_, Resource<B>>,
        _: u32,
        _: Option<&DrawIndirectInfo<'_, Resource<B>, Unmoved>>,
        _: &[DrawStartCountBias],
    ) {
        self.call((), || not_moved("draw_vbo"));
    }

    fn end_query(&self, q: Unmoved) -> Result<Unmoved, Unmoved> {
        match q {}
    }

    fn end_query_end_only_query(&self, q: &Unmoved) -> bool {
        match *q {}
    }

    fn flush(&self, _: values::FlushFlags) -> Option<Unmoved> {
        self.call(None, || not_moved("flush"))
    }

    /// A resource about to be shown or shared needs nothing more: every write is already ordered.
    fn flush_resource(&self, _: &Arc<Resource<B>>) {}

    fn generate_mipmap(
        &self,
        _: &Arc<Resource<B>>,
        _: values::Format,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
    ) -> bool {
        self.call(false, || not_moved("generate_mipmap"))
    }

    fn get_device_reset_status(&self) -> values::ResetStatus {
        values::ResetStatus(guest::reset_status(self.key))
    }

    fn get_query_result(&self, q: &Unmoved, _: bool, _: &mut [u8]) -> bool {
        match *q {}
    }

    fn get_query_result_end_only_query(&self, q: &Unmoved, _: bool, _: &mut [u8]) -> bool {
        match *q {}
    }

    fn get_timestamp(&self) -> u64 {
        self.call(0, || not_moved("get_timestamp"))
    }

    fn launch_grid(&self, _: &GridInfo<'_, Resource<B>>) {
        self.call((), || not_moved("launch_grid"));
    }

    /// Later work sees what earlier work wrote, to targets or storage: the open pass ends (the host
    /// orders passes and dispatches after the ones before).
    fn memory_barrier(&self, _: u32) {
        self.texture_barrier(0);
    }

    /// No condition can be set: a query is not yet moved; ending one holds always.
    fn render_condition(&self, query: Option<&Unmoved>, _: bool, _: values::RenderCondFlag) {
        if let Some(query) = query {
            match *query {}
        }
    }

    fn render_condition_end_only_query(&self, query: &Unmoved, _: bool, _: values::RenderCondFlag) {
        match *query {}
    }

    fn resource_copy_region(
        &self,
        _: &Arc<Resource<B>>,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &Arc<Resource<B>>,
        _: u32,
        _: &PipeBox,
    ) {
        self.call((), || not_moved("resource_copy_region"));
    }

    fn set_active_query_state(&self, _: bool) {
        self.call((), || not_moved("set_active_query_state"));
    }

    fn set_blend_color(&self, color: &BlendColor) {
        self.set.borrow_mut().blend_color = color.color.map(f32::to_bits);
    }

    /// User clip planes are lowered into the programs by st/mesa (the screen reports none).
    fn set_clip_state(&self, _: &ClipState) {}

    fn set_constant_buffer(
        &self,
        _: values::MesaShaderStage,
        _: u32,
        _: Option<&ConstantBuffer<'_, Resource<B>>>,
    ) {
        self.call((), || not_moved("set_constant_buffer"));
    }

    /// The framebuffer's surfaces held and their targets set; the old ones' views and twins go
    /// when the device is next used (the old twins were copied back after their work).
    fn set_framebuffer_state(&self, state: &FramebufferState<'_, Resource<B>>) {
        self.call((), || {
            let mut backend = self.shared.backend();
            let mut set = self.set.borrow_mut();
            let set = &mut *set;
            let mut old = std::mem::replace(&mut set.framebuffer, Framebuffer::of(state));
            old.release(|object| retire(&self.shared.retired, object));
            let mut twins = Vec::new();
            let (targets, views, samples) =
                set.framebuffer.layer_targets(&mut *backend, 0, &mut twins);
            set.framebuffer.target_samples = samples;
            set.framebuffer.twins = twins;
            set.framebuffer.views = views;
            if let Err(error) = set
                .framebuffer
                .copy_into_twins(&mut set.tracker, &mut backend)
            {
                refuse(format_args!("framebuffer: {error}"));
            }
            if let Err(error) = set.tracker.set_targets(&mut *backend, targets) {
                refuse(format_args!("framebuffer: {error:?}"));
            }
        });
    }

    fn set_patch_vertices(&self, patch_vertices: u8) {
        self.set.borrow_mut().patch_vertices = patch_vertices;
    }

    /// The stipple pattern applies only to a rasterizer state that stipples, which is refused by
    /// name.
    fn set_polygon_stipple(&self, _: &PolyStipple) {}

    fn set_sample_mask(&self, sample_mask: u32) {
        self.set.borrow_mut().sample_mask = sample_mask;
    }

    fn set_sampler_views(
        &self,
        shader: values::MesaShaderStage,
        start_slot: u32,
        num_views: u32,
        unbind_num_trailing_slots: u32,
        views: Option<&[Option<&Arc<SamplerView<B>>>]>,
    ) {
        self.call((), || {
            let Some(stage) = stage(shader) else {
                return;
            };
            let mut set = self.set.borrow_mut();
            let slots = &mut set.views[stage as usize];
            let (start, count) = (start_slot as usize, num_views as usize);
            let end = start + count + unbind_num_trailing_slots as usize;
            let Some(slots) = slots.get_mut(start..end) else {
                refuse(format_args!(
                    "sampler views past the {} the screen reports",
                    PIPE_MAX_SHADER_SAMPLER_VIEWS
                ));
            };
            // The views given (none for each where none are), then the trailing slots emptied.
            for (at, slot) in slots.iter_mut().enumerate() {
                *slot = match views {
                    Some(views) if at < count => views.get(at).copied().flatten().cloned(),
                    _ => None,
                };
            }
        });
    }

    /// The first scissor: the host has one.
    fn set_scissor_states(&self, start_slot: u32, scissors: &[ScissorState]) {
        if start_slot == 0
            && let Some(first) = scissors.first()
        {
            self.set.borrow_mut().scissor = Some(*first);
        }
    }

    fn set_shader_buffers(
        &self,
        _: values::MesaShaderStage,
        _: u32,
        _: u32,
        _: Option<&[ShaderBuffer<'_, Resource<B>>]>,
        _: u32,
    ) {
        self.call((), || not_moved("set_shader_buffers"));
    }

    fn set_shader_images(
        &self,
        _: values::MesaShaderStage,
        _: u32,
        _: u32,
        _: u32,
        _: Option<&[ImageView<'_, Resource<B>>]>,
    ) {
        self.call((), || not_moved("set_shader_images"));
    }

    fn set_stencil_ref(&self, value: StencilRef) {
        self.set.borrow_mut().stencil_ref = value.ref_value;
    }

    fn set_stream_output_targets(
        &self,
        targets: &[Option<&Arc<Unmoved>>],
        _: &[u32],
        _: values::MesaPrim,
    ) {
        if let Some(target) = targets.iter().flatten().next() {
            match ***target {}
        }
        self.call((), || not_moved("set_stream_output_targets"));
    }

    fn set_vertex_buffers(&self, _: &[VertexBuffer<'_, Resource<B>>]) {
        self.call((), || not_moved("set_vertex_buffers"));
    }

    /// The first viewport: the host has one.
    fn set_viewport_states(&self, start_slot: u32, viewports: &[ViewportState]) {
        if start_slot == 0
            && let Some(first) = viewports.first()
        {
            self.set.borrow_mut().viewport = Some(*first);
        }
    }

    /// Later draws see what earlier ones wrote, to targets or storage: the open pass ends (the host
    /// orders passes and dispatches after the ones before).
    fn texture_barrier(&self, _: u32) {
        self.call((), || {
            let mut backend = self.shared.backend();
            self.close_pass(&mut backend);
        });
    }

    /// Written from the caller's bytes.
    fn texture_subdata(
        &self,
        resource: &Arc<Resource<B>>,
        level: u32,
        usage: values::MapFlags,
        area: &PipeBox,
        data: &[u8],
        stride: u32,
        layer_stride: u64,
    ) {
        self.call((), || {
            let mut backend = self.shared.backend();
            self.close_pass(&mut backend);
            resources::subdata(
                &mut *backend,
                &self.shared.retired,
                resource,
                level,
                usage.0,
                area,
                data,
                stride,
                layer_stride,
            );
        });
    }

    /// The transfer helper's map of a single-sampled texture (it maps a multisampled one through
    /// one) or a buffer.
    fn helper_transfer_map(
        &self,
        prsc: &Arc<Resource<B>>,
        level: u32,
        usage: values::MapFlags,
        box_: &PipeBox,
    ) -> Option<Mapped<Mapping<B>>> {
        self.call(None, || self.map(prsc, level, usage.0, box_))
    }

    /// Writes what was mapped for writing, unless the caller flushed its parts itself.
    fn helper_transfer_unmap(&self, ptrans: Mapping<B>, written: &[u8]) {
        self.call((), || self.unmap(&ptrans, written));
    }

    /// Writes the part `box_` (relative to the map) of what was mapped.
    fn helper_transfer_flush_region(&self, ptrans: &Mapping<B>, written: &[u8], box_: &PipeBox) {
        self.call((), || {
            let mut backend = self.shared.backend();
            if ptrans.resource().texture().is_some() {
                self.close_pass(&mut backend);
            }
            resources::write_mapped(&mut *backend, &self.shared.retired, ptrans, written, box_);
        });
    }
}
