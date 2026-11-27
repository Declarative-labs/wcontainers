// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! The serving glue's first slice: the screen's and the context's life and the five states. Each entry of `Table` is
//! called by its trampoline in serve.c with keys and flat copies by value; it finds the screen and the objects by key,
//! refuses a key not held, and calls the implementation with no lock held. Rust follows no pointer of Mesa's.

use wcontainers_gallium::{Gallium, GalliumContext, values};

use crate::flat::*;
use crate::held::Held;
use crate::registry::{self, Map};

/// An implementation the glue can serve: its objects may be used from Mesa's threads.
pub trait Servable: Gallium<Context: Send + Sync, Blend: Send + Sync, DepthStencilAlpha: Send + Sync, Rasterizer: Send + Sync, Sampler: Send + Sync, VertexElements: Send + Sync> + Send + Sync + 'static {}

impl<G: Gallium<Context: Send + Sync, Blend: Send + Sync, DepthStencilAlpha: Send + Sync, Rasterizer: Send + Sync, Sampler: Send + Sync, VertexElements: Send + Sync> + Send + Sync + 'static> Servable for G {}

/// A bounded list of keys (states at slots), as serve.c's `struct wc_keys`: `count` of them, at most 32.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KeysList {
    /// How many keys are given.
    pub count: u32,
    /// The keys (zero names none).
    pub keys: [usize; 32],
}

/// A bounded list of vertex elements, as serve.c's `struct wc_vertex_elements`: `count` of them, at most 32.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VertexElementsList {
    /// How many elements are given.
    pub count: u32,
    /// The elements.
    pub elements: [FlatVertexElement; 32],
}

/// The Rust functions serve.c's trampolines call, as serve.c's `struct wc_table` (the same order).
#[repr(C)]
pub struct Table {
    /// `pipe_screen.context_create`.
    context_create: extern "C" fn(usize, usize, usize, u32) -> usize,
    /// `pipe_screen.destroy`.
    screen_destroy: extern "C" fn(usize, usize) -> u8,
    /// `pipe_context.destroy`.
    context_destroy: extern "C" fn(usize, usize) -> u8,
    /// `pipe_context.create_blend_state`.
    create_blend_state: extern "C" fn(usize, usize, FlatBlendState) -> usize,
    /// `pipe_context.bind_blend_state`.
    bind_blend_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.delete_blend_state`.
    delete_blend_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.create_depth_stencil_alpha_state`.
    create_depth_stencil_alpha_state: extern "C" fn(usize, usize, FlatDepthStencilAlphaState) -> usize,
    /// `pipe_context.bind_depth_stencil_alpha_state`.
    bind_depth_stencil_alpha_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.delete_depth_stencil_alpha_state`.
    delete_depth_stencil_alpha_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.create_rasterizer_state`.
    create_rasterizer_state: extern "C" fn(usize, usize, FlatRasterizerState) -> usize,
    /// `pipe_context.bind_rasterizer_state`.
    bind_rasterizer_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.delete_rasterizer_state`.
    delete_rasterizer_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.create_sampler_state`.
    create_sampler_state: extern "C" fn(usize, usize, FlatSamplerState) -> usize,
    /// `pipe_context.bind_sampler_states`.
    bind_sampler_states: extern "C" fn(usize, usize, i32, u32, KeysList) -> u8,
    /// `pipe_context.delete_sampler_state`.
    delete_sampler_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.create_vertex_elements_state`.
    create_vertex_elements_state: extern "C" fn(usize, usize, VertexElementsList) -> usize,
    /// `pipe_context.bind_vertex_elements_state`.
    bind_vertex_elements_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_context.delete_vertex_elements_state`.
    delete_vertex_elements_state: extern "C" fn(usize, usize, usize) -> u8,
    /// `pipe_screen.shader_caps`, by stage.
    shader_caps: extern "C" fn(usize, usize, i32) -> FlatShaderCaps,
    /// `pipe_screen.caps`, with the two values Mesa computes from the stages' capabilities.
    caps: extern "C" fn(usize, usize, u32, u8) -> FlatCaps,
    /// `pipe_screen.compute_caps`.
    compute_caps: extern "C" fn(usize, usize) -> FlatComputeCaps,
}

/// A screen being served: the implementation, the objects Mesa holds by key, and the table its trampolines call.
pub struct Served<G: Servable> {
    /// The implementation.
    pub screen: G,
    /// Each context Mesa holds, by key.
    pub contexts: Map<G::Context>,
    /// Each Blend Mesa holds, by key.
    pub blend: Map<G::Blend>,
    /// Each DepthStencilAlpha Mesa holds, by key.
    pub depth_stencil_alpha: Map<G::DepthStencilAlpha>,
    /// Each Rasterizer Mesa holds, by key.
    pub rasterizer: Map<G::Rasterizer>,
    /// Each Sampler Mesa holds, by key.
    pub sampler: Map<G::Sampler>,
    /// Each VertexElements Mesa holds, by key.
    pub vertex_elements: Map<G::VertexElements>,
    /// The table serve.c's trampolines call; it lives as long as the screen is served.
    pub table: Table,
}

impl<G: Servable> Served<G> {
    /// A screen of the implementation, with nothing held yet.
    pub fn new(screen: G) -> Served<G> {
        Served {
            screen,
            contexts: Map::default(),
            blend: Map::default(),
            depth_stencil_alpha: Map::default(),
            rasterizer: Map::default(),
            sampler: Map::default(),
            vertex_elements: Map::default(),
            table: Table {
            context_create: context_create::<G>,
            screen_destroy: screen_destroy::<G>,
            context_destroy: context_destroy::<G>,
            create_blend_state: create_blend_state::<G>,
            bind_blend_state: bind_blend_state::<G>,
            delete_blend_state: delete_blend_state::<G>,
            create_depth_stencil_alpha_state: create_depth_stencil_alpha_state::<G>,
            bind_depth_stencil_alpha_state: bind_depth_stencil_alpha_state::<G>,
            delete_depth_stencil_alpha_state: delete_depth_stencil_alpha_state::<G>,
            create_rasterizer_state: create_rasterizer_state::<G>,
            bind_rasterizer_state: bind_rasterizer_state::<G>,
            delete_rasterizer_state: delete_rasterizer_state::<G>,
            create_sampler_state: create_sampler_state::<G>,
            bind_sampler_states: bind_sampler_states::<G>,
            delete_sampler_state: delete_sampler_state::<G>,
            create_vertex_elements_state: create_vertex_elements_state::<G>,
            bind_vertex_elements_state: bind_vertex_elements_state::<G>,
            delete_vertex_elements_state: delete_vertex_elements_state::<G>,
            shader_caps: shader_caps::<G>,
            caps: caps::<G>,
            compute_caps: compute_caps::<G>,
            },
        }
    }
}

#[cfg(mesa)]
#[expect(unsafe_code, reason = "serve.c defines wc_screen_create as declared here")]
unsafe extern "C" {
    fn wc_screen_create(key: usize, table: *const Table) -> usize;
}

/// Serves an implementation: the address of the `struct pipe_screen` Mesa calls (a number Rust never follows), or None
/// where C could not allocate it.
#[cfg(mesa)]
pub fn serve<G: Servable>(screen: G) -> Option<usize> {
    let served = std::sync::Arc::new(Served::new(screen));
    let key = registry::keep_screen(std::sync::Arc::clone(&served));
    #[expect(unsafe_code, reason = "serve.c: wc_screen_create takes the screen's key and a pointer to its table, which lives as long as the screen is served (the Arc kept in the registry until the screen's destroy), reads only the table's address and allocates the C screen")]
    // SAFETY: our own C function, given a number and a pointer to our own table, kept alive by the registry.
    let address = unsafe { wc_screen_create(key, &raw const served.table) };
    if address == 0 {
        let _ = registry::take_screen::<Served<G>>(key);
        return None;
    }
    Some(address)
}

/// Mesa's `pipe_screen.context_create`, from its trampoline in serve.c.
extern "C" fn context_create<G: Servable>(screen: usize, _context: usize, private: usize, flags: u32) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Ok(private) = u64::try_from(private) else { return 0 };
    served.screen.context_create(private, flags).map_or(0, |made| served.contexts.insert(made))
}

/// Mesa's `pipe_screen.destroy`, from its trampoline in serve.c.
extern "C" fn screen_destroy<G: Servable>(screen: usize, _context: usize) -> u8 {
    let Some(served) = registry::take_screen::<Served<G>>(screen) else { return 0 };
    served.screen.destroy();
    1
}

/// Mesa's `pipe_context.destroy`, from its trampoline in serve.c.
extern "C" fn context_destroy<G: Servable>(screen: usize, context: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.take(context) else { return 0 };
    context.destroy();
    1
}

/// Mesa's `pipe_context.create_blend_state`, from its trampoline in serve.c.
extern "C" fn create_blend_state<G: Servable>(screen: usize, context: usize, state: FlatBlendState) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let held = Held::<G>::new();
    let Some(owned) = state.owned(&held) else { return 0 };
    context.create_blend_state(&owned).map_or(0, |made| served.blend.insert(made))
}

/// Mesa's `pipe_context.bind_blend_state`, from its trampoline in serve.c.
extern "C" fn bind_blend_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let object = if state == 0 { None } else {
        let Some(object) = served.blend.get(state) else { return 0 };
        Some(object)
    };
    context.bind_blend_state(object.as_deref());
    1
}

/// Mesa's `pipe_context.delete_blend_state`, from its trampoline in serve.c.
extern "C" fn delete_blend_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(object) = served.blend.take(state) else { return 0 };
    context.delete_blend_state(object);
    1
}

/// Mesa's `pipe_context.create_depth_stencil_alpha_state`, from its trampoline in serve.c.
extern "C" fn create_depth_stencil_alpha_state<G: Servable>(screen: usize, context: usize, state: FlatDepthStencilAlphaState) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let held = Held::<G>::new();
    let Some(owned) = state.owned(&held) else { return 0 };
    context.create_depth_stencil_alpha_state(&owned).map_or(0, |made| served.depth_stencil_alpha.insert(made))
}

/// Mesa's `pipe_context.bind_depth_stencil_alpha_state`, from its trampoline in serve.c.
extern "C" fn bind_depth_stencil_alpha_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let object = if state == 0 { None } else {
        let Some(object) = served.depth_stencil_alpha.get(state) else { return 0 };
        Some(object)
    };
    context.bind_depth_stencil_alpha_state(object.as_deref());
    1
}

/// Mesa's `pipe_context.delete_depth_stencil_alpha_state`, from its trampoline in serve.c.
extern "C" fn delete_depth_stencil_alpha_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(object) = served.depth_stencil_alpha.take(state) else { return 0 };
    context.delete_depth_stencil_alpha_state(object);
    1
}

/// Mesa's `pipe_context.create_rasterizer_state`, from its trampoline in serve.c.
extern "C" fn create_rasterizer_state<G: Servable>(screen: usize, context: usize, state: FlatRasterizerState) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let held = Held::<G>::new();
    let Some(owned) = state.owned(&held) else { return 0 };
    context.create_rasterizer_state(&owned).map_or(0, |made| served.rasterizer.insert(made))
}

/// Mesa's `pipe_context.bind_rasterizer_state`, from its trampoline in serve.c.
extern "C" fn bind_rasterizer_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let object = if state == 0 { None } else {
        let Some(object) = served.rasterizer.get(state) else { return 0 };
        Some(object)
    };
    context.bind_rasterizer_state(object.as_deref());
    1
}

/// Mesa's `pipe_context.delete_rasterizer_state`, from its trampoline in serve.c.
extern "C" fn delete_rasterizer_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(object) = served.rasterizer.take(state) else { return 0 };
    context.delete_rasterizer_state(object);
    1
}

/// Mesa's `pipe_context.create_sampler_state`, from its trampoline in serve.c.
extern "C" fn create_sampler_state<G: Servable>(screen: usize, context: usize, state: FlatSamplerState) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let held = Held::<G>::new();
    let Some(owned) = state.owned(&held) else { return 0 };
    context.create_sampler_state(&owned).map_or(0, |made| served.sampler.insert(made))
}

/// Mesa's `pipe_context.bind_sampler_states`, from its trampoline in serve.c.
extern "C" fn bind_sampler_states<G: Servable>(screen: usize, context: usize, shader: i32, start_slot: u32, slots: KeysList) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(keys) = slots.keys.get(..usize::try_from(slots.count).unwrap_or(usize::MAX)) else { return 0 };
    let mut objects = Vec::with_capacity(keys.len());
    for &key in keys {
        if key == 0 {
            objects.push(None);
        } else {
            let Some(object) = served.sampler.get(key) else { return 0 };
            objects.push(Some(object));
        }
    }
    let lent: Vec<_> = objects.iter().map(Option::as_deref).collect();
    context.bind_sampler_states(values::MesaShaderStage(shader), start_slot, &lent);
    1
}

/// Mesa's `pipe_context.delete_sampler_state`, from its trampoline in serve.c.
extern "C" fn delete_sampler_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(object) = served.sampler.take(state) else { return 0 };
    context.delete_sampler_state(object);
    1
}

/// Mesa's `pipe_context.create_vertex_elements_state`, from its trampoline in serve.c.
extern "C" fn create_vertex_elements_state<G: Servable>(screen: usize, context: usize, list: VertexElementsList) -> usize {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let held = Held::<G>::new();
    let Some(elements) = list.elements.get(..usize::try_from(list.count).unwrap_or(usize::MAX)) else { return 0 };
    let Some(owned) = elements.iter().map(|element| element.owned(&held)).collect::<Option<Vec<_>>>() else { return 0 };
    context.create_vertex_elements_state(&owned).map_or(0, |made| served.vertex_elements.insert(made))
}

/// Mesa's `pipe_context.bind_vertex_elements_state`, from its trampoline in serve.c.
extern "C" fn bind_vertex_elements_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let object = if state == 0 { None } else {
        let Some(object) = served.vertex_elements.get(state) else { return 0 };
        Some(object)
    };
    context.bind_vertex_elements_state(object.as_deref());
    1
}

/// Mesa's `pipe_context.delete_vertex_elements_state`, from its trampoline in serve.c.
extern "C" fn delete_vertex_elements_state<G: Servable>(screen: usize, context: usize, state: usize) -> u8 {
    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };
    let Some(context) = served.contexts.get(context) else { return 0 };
    let Some(object) = served.vertex_elements.take(state) else { return 0 };
    context.delete_vertex_elements_state(object);
    1
}

/// Mesa's `pipe_screen.shader_caps[stage]`, asked once by serve.c while it makes the screen: zero (Mesa sets no default)
/// as the implementation changes it.
extern "C" fn shader_caps<G: Servable>(screen: usize, _context: usize, stage: i32) -> FlatShaderCaps {
    let mut caps = wcontainers_gallium::ShaderCaps::default();
    if let Some(served) = registry::screen::<Served<G>>(screen) {
        served.screen.shader_caps(values::MesaShaderStage(stage), &mut caps);
    }
    FlatShaderCaps::of(&caps)
}

/// Mesa's `pipe_screen.caps`, asked once by serve.c after the stages': Mesa's defaults (u_init_pipe_screen_caps's), with
/// the two it computes from the stages' capabilities (the fragment stage's first constant buffer's size; the
/// geometry stage's indirect addressing and storage buffers, for selection by a hardware geometry program), as the
/// implementation changes them. Mesa's MESA_HW_ACCEL_SELECT is not read: selection is as Mesa's default.
extern "C" fn caps<G: Servable>(screen: usize, _context: usize, constant_buffer0_size: u32, hardware_select: u8) -> FlatCaps {
    let mut caps = wcontainers_gallium::Caps { max_constant_buffer_size: constant_buffer0_size, hardware_gl_select: hardware_select != 0, ..wcontainers_gallium::Caps::default() };
    if let Some(served) = registry::screen::<Served<G>>(screen) {
        served.screen.caps(&mut caps);
    }
    FlatCaps::of(&caps)
}

/// Mesa's `pipe_screen.compute_caps`, asked once by serve.c: zero (Mesa sets no default) as the implementation changes
/// it.
extern "C" fn compute_caps<G: Servable>(screen: usize, _context: usize) -> FlatComputeCaps {
    let mut caps = wcontainers_gallium::ComputeCaps::default();
    if let Some(served) = registry::screen::<Served<G>>(screen) {
        served.screen.compute_caps(&mut caps);
    }
    FlatComputeCaps::of(&caps)
}

