//! Gallium's serving glue (AC8 step 2), its first slice: the screen's and the context's life and the five states
//! (made, bound, given back). For each pointer, a C trampoline (serve.c) that Mesa calls, which flattens what Mesa
//! passes into plain data of our own and calls one Rust function through the screen's table, by value: keys and flat
//! copies, never a pointer Rust follows (a bounded list goes as a fixed array with its count, refused past its bound in
//! C); and that Rust function (serve.rs), which finds the screen and the objects by key in the registry, refuses a key
//! it does not hold, makes the trait's owned values, and calls the implementation with no lock held. C frees an object
//! it allocated only when Rust answers that it was live.
//!
//! Written with the rest of the direct crate by `api-crate-generator gallium`.

use std::fmt::Write as _;

use crate::cheader::Header;
use crate::gallium::{PLAN, Role};
use crate::gallium_glue::{c_flat, rust_flat};

/// How the slice's pointers are served.
#[derive(Clone, Copy)]
enum Served {
    /// pipe_screen.context_create.
    ContextCreate,
    /// pipe_screen.destroy.
    ScreenDestroy,
    /// pipe_context.destroy.
    ContextDestroy,
    /// A state made from one plain structure: (kind, structure).
    Make(&'static str, &'static str),
    /// A state made from a bounded list of plain structures: (kind, structure, bound).
    MakeList(&'static str, &'static str, &'static str),
    /// A state bound (or none).
    Bind(&'static str),
    /// States bound at slots, a bounded list: (kind, bound).
    BindSlots(&'static str, &'static str),
    /// A state given back.
    Delete(&'static str),
}

/// The slice, in the table's order: (structure, pointer, how).
const SLICE: &[(&str, &str, Served)] = &[
    ("pipe_screen", "context_create", Served::ContextCreate),
    ("pipe_screen", "destroy", Served::ScreenDestroy),
    ("pipe_context", "destroy", Served::ContextDestroy),
    ("pipe_context", "create_blend_state", Served::Make("Blend", "pipe_blend_state")),
    ("pipe_context", "bind_blend_state", Served::Bind("Blend")),
    ("pipe_context", "delete_blend_state", Served::Delete("Blend")),
    ("pipe_context", "create_depth_stencil_alpha_state", Served::Make("DepthStencilAlpha", "pipe_depth_stencil_alpha_state")),
    ("pipe_context", "bind_depth_stencil_alpha_state", Served::Bind("DepthStencilAlpha")),
    ("pipe_context", "delete_depth_stencil_alpha_state", Served::Delete("DepthStencilAlpha")),
    ("pipe_context", "create_rasterizer_state", Served::Make("Rasterizer", "pipe_rasterizer_state")),
    ("pipe_context", "bind_rasterizer_state", Served::Bind("Rasterizer")),
    ("pipe_context", "delete_rasterizer_state", Served::Delete("Rasterizer")),
    ("pipe_context", "create_sampler_state", Served::Make("Sampler", "pipe_sampler_state")),
    ("pipe_context", "bind_sampler_states", Served::BindSlots("Sampler", "PIPE_MAX_SAMPLERS")),
    ("pipe_context", "delete_sampler_state", Served::Delete("Sampler")),
    ("pipe_context", "create_vertex_elements_state", Served::MakeList("VertexElements", "pipe_vertex_element", "PIPE_MAX_ATTRIBS")),
    ("pipe_context", "bind_vertex_elements_state", Served::Bind("VertexElements")),
    ("pipe_context", "delete_vertex_elements_state", Served::Delete("VertexElements")),
];

/// The kinds the slice holds, each a map of the registry: (kind, field).
const HELD: &[(&str, &str)] = &[
    ("Context", "contexts"),
    ("Blend", "blend"),
    ("DepthStencilAlpha", "depth_stencil_alpha"),
    ("Rasterizer", "rasterizer"),
    ("Sampler", "sampler"),
    ("VertexElements", "vertex_elements"),
];

fn field_of(kind: &str) -> &'static str {
    HELD.iter().find(|(held, _)| *held == kind).map_or("contexts", |(_, field)| field)
}

/// The table entry's name: the pointer's, the screen's destroy `screen_destroy`, the context's `context_destroy`.
fn entry_name(structure: &str, pointer: &str) -> String {
    match (structure, pointer) {
        ("pipe_screen", "destroy") => "screen_destroy".to_string(),
        ("pipe_context", "destroy") => "context_destroy".to_string(),
        _ => pointer.to_string(),
    }
}

/// The C and Rust types of one table entry: (C return, C parameters after the two keys, Rust parameters after the two
/// keys, Rust return).
fn entry_types(how: Served) -> (String, String, String, &'static str) {
    match how {
        Served::ContextCreate => ("uintptr_t".into(), "uintptr_t priv, uint32_t flags".into(), "private: usize, flags: u32".into(), "usize"),
        Served::ScreenDestroy | Served::ContextDestroy => ("uint8_t".into(), String::new(), String::new(), "u8"),
        Served::Make(_, structure) => ("uintptr_t".into(), format!("struct {} state", c_flat(structure)), format!("state: {}", rust_flat(structure)), "usize"),
        Served::MakeList(..) => ("uintptr_t".into(), "struct wc_vertex_elements list".into(), "list: VertexElementsList".into(), "usize"),
        Served::Bind(_) | Served::Delete(_) => ("uint8_t".into(), "uintptr_t state".into(), "state: usize".into(), "u8"),
        Served::BindSlots(..) => ("uint8_t".into(), "int32_t shader, uint32_t start_slot, struct wc_keys slots".into(), "shader: i32, start_slot: u32, slots: KeysList".into(), "u8"),
    }
}

/// The Rust function one entry points at, generic over the implementation.
fn rust_function(structure: &str, pointer: &str, how: Served) -> String {
    let name = entry_name(structure, pointer);
    let (_, _, params, ret) = entry_types(how);
    let params = if params.is_empty() { String::new() } else { format!(", {params}") };
    let (key_params, find_context) = match how {
        Served::ContextCreate | Served::ScreenDestroy => ("screen: usize, _context: usize", ""),
        _ => ("screen: usize, context: usize", "    let Some(context) = served.contexts.get(context) else { return 0 };\n"),
    };
    let find_screen = "    let Some(served) = registry::screen::<Served<G>>(screen) else { return 0 };\n";
    let body = match how {
        Served::ContextCreate => format!("{find_screen}    let Ok(private) = u64::try_from(private) else {{ return 0 }};\n    served.screen.context_create(private, flags).map_or(0, |made| served.contexts.insert(made))\n"),
        Served::ScreenDestroy => "    let Some(served) = registry::take_screen::<Served<G>>(screen) else { return 0 };\n    served.screen.destroy();\n    1\n".to_string(),
        Served::ContextDestroy => format!("{find_screen}    let Some(context) = served.contexts.take(context) else {{ return 0 }};\n    context.destroy();\n    1\n"),
        Served::Make(kind, _) => format!(
            "{find_screen}{find_context}    let held = Held::<G>::new();\n    let Some(owned) = state.owned(&held) else {{ return 0 }};\n    context.{pointer}(&owned).map_or(0, |made| served.{}.insert(made))\n",
            field_of(kind)
        ),
        Served::MakeList(kind, _, _) => format!(
            "{find_screen}{find_context}    let held = Held::<G>::new();\n    let Some(elements) = list.elements.get(..usize::try_from(list.count).unwrap_or(usize::MAX)) else {{ return 0 }};\n    let Some(owned) = elements.iter().map(|element| element.owned(&held)).collect::<Option<Vec<_>>>() else {{ return 0 }};\n    context.{pointer}(&owned).map_or(0, |made| served.{}.insert(made))\n",
            field_of(kind)
        ),
        Served::Bind(kind) => format!(
            "{find_screen}{find_context}    let object = if state == 0 {{ None }} else {{\n        let Some(object) = served.{}.get(state) else {{ return 0 }};\n        Some(object)\n    }};\n    context.{pointer}(object.as_deref());\n    1\n",
            field_of(kind)
        ),
        Served::BindSlots(kind, _) => format!(
            "{find_screen}{find_context}    let Some(keys) = slots.keys.get(..usize::try_from(slots.count).unwrap_or(usize::MAX)) else {{ return 0 }};\n    let mut objects = Vec::with_capacity(keys.len());\n    for &key in keys {{\n        if key == 0 {{\n            objects.push(None);\n        }} else {{\n            let Some(object) = served.{}.get(key) else {{ return 0 }};\n            objects.push(Some(object));\n        }}\n    }}\n    let lent: Vec<_> = objects.iter().map(Option::as_deref).collect();\n    context.{pointer}(values::MesaShaderStage(shader), start_slot, &lent);\n    1\n",
            field_of(kind)
        ),
        Served::Delete(kind) => format!("{find_screen}{find_context}    let Some(object) = served.{}.take(state) else {{ return 0 }};\n    context.{pointer}(object);\n    1\n", field_of(kind)),
    };
    format!("/// Mesa's `{structure}.{pointer}`, from its trampoline in serve.c.\nextern \"C\" fn {name}<G: Servable>({key_params}{params}) -> {ret} {{\n{body}}}\n")
}

/// The C trampoline of one pointer.
fn c_trampoline(structure: &str, pointer: &str, how: Served) -> String {
    let name = entry_name(structure, pointer);
    match how {
        Served::ContextCreate => format!(
            "static struct pipe_context *wc_{name}(struct pipe_screen *screen, void *priv, unsigned flags)\n{{\n    struct wc_screen *s = (struct wc_screen *)screen;\n    const uintptr_t key = s->table->context_create(s->key, 0, (uintptr_t)priv, flags);\n    if (!key)\n        return NULL;\n    struct wc_context *c = calloc(1, sizeof *c);\n    if (!c) {{\n        s->table->context_destroy(s->key, key);\n        return NULL;\n    }}\n    c->key = key;\n    c->screen = s;\n    c->base.screen = screen;\n    c->base.priv = priv;\n    wc_context_pointers(&c->base);\n    return &c->base;\n}}\n"
        ),
        Served::ScreenDestroy => format!("static void wc_{name}(struct pipe_screen *screen)\n{{\n    struct wc_screen *s = (struct wc_screen *)screen;\n    /* The table is Rust's and ends with the screen: nothing of it is read after the call. */\n    if (s->table->screen_destroy(s->key, 0))\n        free(s);\n}}\n"),
        Served::ContextDestroy => format!("static void wc_{name}(struct pipe_context *pipe)\n{{\n    struct wc_context *c = (struct wc_context *)pipe;\n    if (c->screen->table->context_destroy(c->screen->key, c->key))\n        free(c);\n}}\n"),
        Served::Make(_, structure) => format!(
            "static void *wc_{name}(struct pipe_context *pipe, const struct {structure} *state)\n{{\n    struct wc_context *c = (struct wc_context *)pipe;\n    if (!state)\n        return NULL;\n    struct {flat} flat;\n    {flat}_from(state, &flat);\n    return (void *)c->screen->table->{name}(c->screen->key, c->key, flat);\n}}\n",
            flat = c_flat(structure)
        ),
        Served::MakeList(_, structure, bound) => format!(
            "static void *wc_{name}(struct pipe_context *pipe, unsigned count, const struct {structure} *elements)\n{{\n    struct wc_context *c = (struct wc_context *)pipe;\n    if (count > {bound} || (count && !elements))\n        return NULL;\n    struct wc_vertex_elements list;\n    memset(&list, 0, sizeof list);\n    list.count = count;\n    for (unsigned i = 0; i < count; i++)\n        {flat}_from(&elements[i], &list.elements[i]);\n    return (void *)c->screen->table->{name}(c->screen->key, c->key, list);\n}}\n",
            flat = c_flat(structure)
        ),
        Served::Bind(_) | Served::Delete(_) => format!("static void wc_{name}(struct pipe_context *pipe, void *state)\n{{\n    struct wc_context *c = (struct wc_context *)pipe;\n    c->screen->table->{name}(c->screen->key, c->key, (uintptr_t)state);\n}}\n"),
        Served::BindSlots(_, bound) => format!(
            "static void wc_{name}(struct pipe_context *pipe, mesa_shader_stage shader, unsigned start_slot, unsigned count, void **states)\n{{\n    struct wc_context *c = (struct wc_context *)pipe;\n    if (count > {bound})\n        return;\n    struct wc_keys slots;\n    memset(&slots, 0, sizeof slots);\n    slots.count = count;\n    for (unsigned i = 0; i < count; i++)\n        slots.keys[i] = states ? (uintptr_t)states[i] : 0;\n    c->screen->table->{name}(c->screen->key, c->key, (int32_t)shader, start_slot, slots);\n}}\n"
        ),
    }
}

/// serve.rs and serve.c, and the report line.
/// The table's entries for the screen's capabilities (G5): Rust's, then C's.
const CAPS_TABLE_RUST: &str = "    /// `pipe_screen.shader_caps`, by stage.\n    shader_caps: extern \"C\" fn(usize, usize, i32) -> FlatShaderCaps,\n    /// `pipe_screen.caps`, with the two values Mesa computes from the stages' capabilities.\n    caps: extern \"C\" fn(usize, usize, u32, u8) -> FlatCaps,\n    /// `pipe_screen.compute_caps`.\n    compute_caps: extern \"C\" fn(usize, usize) -> FlatComputeCaps,\n";
const CAPS_TABLE_C: &str = "    struct wc_flat_pipe_shader_caps (*shader_caps)(uintptr_t screen, uintptr_t context, int32_t stage);\n    struct wc_flat_pipe_caps (*caps)(uintptr_t screen, uintptr_t context, uint32_t constant_buffer0_size, uint8_t hardware_select);\n    struct wc_flat_pipe_compute_caps (*compute_caps)(uintptr_t screen, uintptr_t context);\n";

/// The Rust entries of the screen's capabilities: each starts from Mesa's defaults (the trait's `Default`, which the
/// direct crate's test compares with what u_init_pipe_screen_caps writes) and hands them to the implementation.
const CAPS_RUST: &str = "/// Mesa's `pipe_screen.shader_caps[stage]`, asked once by serve.c while it makes the screen: zero (Mesa sets no default)\n/// as the implementation changes it.\nextern \"C\" fn shader_caps<G: Servable>(screen: usize, _context: usize, stage: i32) -> FlatShaderCaps {\n    let mut caps = wcontainers_gallium::ShaderCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(screen) {\n        served.screen.shader_caps(values::MesaShaderStage(stage), &mut caps);\n    }\n    FlatShaderCaps::of(&caps)\n}\n\n/// Mesa's `pipe_screen.caps`, asked once by serve.c after the stages': Mesa's defaults (u_init_pipe_screen_caps's), with\n/// the two it computes from the stages' capabilities (the fragment stage's first constant buffer's size; the\n/// geometry stage's indirect addressing and storage buffers, for selection by a hardware geometry program), as the\n/// implementation changes them. Mesa's MESA_HW_ACCEL_SELECT is not read: selection is as Mesa's default.\nextern \"C\" fn caps<G: Servable>(screen: usize, _context: usize, constant_buffer0_size: u32, hardware_select: u8) -> FlatCaps {\n    let mut caps = wcontainers_gallium::Caps { max_constant_buffer_size: constant_buffer0_size, hardware_gl_select: hardware_select != 0, ..wcontainers_gallium::Caps::default() };\n    if let Some(served) = registry::screen::<Served<G>>(screen) {\n        served.screen.caps(&mut caps);\n    }\n    FlatCaps::of(&caps)\n}\n\n/// Mesa's `pipe_screen.compute_caps`, asked once by serve.c: zero (Mesa sets no default) as the implementation changes\n/// it.\nextern \"C\" fn compute_caps<G: Servable>(screen: usize, _context: usize) -> FlatComputeCaps {\n    let mut caps = wcontainers_gallium::ComputeCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(screen) {\n        served.screen.compute_caps(&mut caps);\n    }\n    FlatComputeCaps::of(&caps)\n}\n\n";

/// serve.c's part of making the screen: the capabilities written into Mesa's screen (its const fields, which a driver
/// fills once), each stage's first, as u_init_pipe_screen_caps reads them.
const CAPS_C: &str = "    for (int32_t stage = 0; stage < MESA_SHADER_MESH_STAGES; stage++) {\n        const struct wc_flat_pipe_shader_caps flat = table->shader_caps(key, 0, stage);\n        wc_flat_pipe_shader_caps_to(&flat, (struct pipe_shader_caps *)&screen->shader_caps[stage]);\n    }\n    const struct pipe_shader_caps *geometry = &screen->shader_caps[MESA_SHADER_GEOMETRY];\n    const struct wc_flat_pipe_caps caps = table->caps(key, 0, screen->shader_caps[MESA_SHADER_FRAGMENT].max_const_buffer0_size, geometry->indirect_temp_addr && geometry->max_shader_buffers);\n    wc_flat_pipe_caps_to(&caps, (struct pipe_caps *)&screen->caps);\n    const struct wc_flat_pipe_compute_caps compute = table->compute_caps(key, 0);\n    wc_flat_pipe_compute_caps_to(&compute, (struct pipe_compute_caps *)&screen->compute_caps);\n";

pub fn generate(header: &Header) -> Result<(String, String, String), String> {
    let bound = |name: &str| header.known_numbers.get(name).copied().ok_or_else(|| format!("no value for {name}"));
    for (structure, pointer, _) in SLICE {
        let planned = PLAN.iter().find(|entry| entry.structure == *structure && entry.pointer == *pointer).ok_or_else(|| format!("{structure}.{pointer}: not planned"))?;
        if planned.roles.iter().any(|role| matches!(role, Role::MesaFills(_) | Role::SameAs(_))) {
            return Err(format!("{structure}.{pointer}: not the implementation's"));
        }
    }
    let samplers = bound("PIPE_MAX_SAMPLERS")?;
    let attributes = bound("PIPE_MAX_ATTRIBS")?;
    // Rust.
    let mut rust = String::new();
    let mut table = String::new();
    let mut filled = String::new();
    for (structure, pointer, how) in SLICE {
        let name = entry_name(structure, pointer);
        let (_, _, params, ret) = entry_types(*how);
        let params = if params.is_empty() { String::new() } else { format!(", {}", params.split(", ").map(|param| param.split(": ").nth(1).unwrap_or_default()).collect::<Vec<_>>().join(", ")) };
        let _ = writeln!(table, "    /// `{structure}.{pointer}`.\n    {name}: extern \"C\" fn(usize, usize{params}) -> {ret},");
        let _ = writeln!(filled, "            {name}: {name}::<G>,");
        rust.push_str(&rust_function(structure, pointer, *how));
        rust.push('\n');
    }
    // The screen's capabilities (G5), asked once while serve.c makes the screen.
    table.push_str(CAPS_TABLE_RUST);
    filled.push_str("            shader_caps: shader_caps::<G>,\n            caps: caps::<G>,\n            compute_caps: compute_caps::<G>,\n");
    rust.push_str(CAPS_RUST);
    let kinds_bound: Vec<String> = HELD.iter().map(|(kind, _)| format!("{kind}: Send + Sync")).collect();
    let maps: String = HELD.iter().skip(1).map(|(kind, field)| format!("    /// Each {kind} Mesa holds, by key.\n    pub {field}: Map<G::{kind}>,\n")).collect();
    let new_maps: String = HELD.iter().skip(1).map(|(_, field)| format!("            {field}: Map::default(),\n")).collect();
    let header_rust = "// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.\n";
    let rust_file = format!(
        "{header_rust}//! The serving glue's first slice: the screen's and the context's life and the five states. Each entry of `Table` is\n//! called by its trampoline in serve.c with keys and flat copies by value; it finds the screen and the objects by key,\n//! refuses a key not held, and calls the implementation with no lock held. Rust follows no pointer of Mesa's.\n\nuse wcontainers_gallium::{{Gallium, GalliumContext, values}};\n\nuse crate::flat::*;\nuse crate::held::Held;\nuse crate::registry::{{self, Map}};\n\n/// An implementation the glue can serve: its objects may be used from Mesa's threads.\npub trait Servable: Gallium<{}> + Send + Sync + 'static {{}}\n\nimpl<G: Gallium<{}> + Send + Sync + 'static> Servable for G {{}}\n\n/// A bounded list of keys (states at slots), as serve.c's `struct wc_keys`: `count` of them, at most {samplers}.\n#[repr(C)]\n#[derive(Clone, Copy, Debug)]\npub struct KeysList {{\n    /// How many keys are given.\n    pub count: u32,\n    /// The keys (zero names none).\n    pub keys: [usize; {samplers}],\n}}\n\n/// A bounded list of vertex elements, as serve.c's `struct wc_vertex_elements`: `count` of them, at most {attributes}.\n#[repr(C)]\n#[derive(Clone, Copy, Debug)]\npub struct VertexElementsList {{\n    /// How many elements are given.\n    pub count: u32,\n    /// The elements.\n    pub elements: [{}; {attributes}],\n}}\n\n/// The Rust functions serve.c's trampolines call, as serve.c's `struct wc_table` (the same order).\n#[repr(C)]\npub struct Table {{\n{table}}}\n\n/// A screen being served: the implementation, the objects Mesa holds by key, and the table its trampolines call.\npub struct Served<G: Servable> {{\n    /// The implementation.\n    pub screen: G,\n    /// Each context Mesa holds, by key.\n    pub contexts: Map<G::Context>,\n{maps}    /// The table serve.c's trampolines call; it lives as long as the screen is served.\n    pub table: Table,\n}}\n\nimpl<G: Servable> Served<G> {{\n    /// A screen of the implementation, with nothing held yet.\n    pub fn new(screen: G) -> Served<G> {{\n        Served {{\n            screen,\n            contexts: Map::default(),\n{new_maps}            table: Table {{\n{filled}            }},\n        }}\n    }}\n}}\n\n#[cfg(mesa)]\n#[expect(unsafe_code, reason = \"serve.c defines wc_screen_create as declared here\")]\nunsafe extern \"C\" {{\n    fn wc_screen_create(key: usize, table: *const Table) -> usize;\n}}\n\n/// Serves an implementation: the address of the `struct pipe_screen` Mesa calls (a number Rust never follows), or None\n/// where C could not allocate it.\n#[cfg(mesa)]\npub fn serve<G: Servable>(screen: G) -> Option<usize> {{\n    let served = std::sync::Arc::new(Served::new(screen));\n    let key = registry::keep_screen(std::sync::Arc::clone(&served));\n    #[expect(unsafe_code, reason = \"serve.c: wc_screen_create takes the screen's key and a pointer to its table, which lives as long as the screen is served (the Arc kept in the registry until the screen's destroy), reads only the table's address and allocates the C screen\")]\n    // SAFETY: our own C function, given a number and a pointer to our own table, kept alive by the registry.\n    let address = unsafe {{ wc_screen_create(key, &raw const served.table) }};\n    if address == 0 {{\n        let _ = registry::take_screen::<Served<G>>(key);\n        return None;\n    }}\n    Some(address)\n}}\n\n{rust}",
        kinds_bound.join(", "),
        kinds_bound.join(", "),
        rust_flat("pipe_vertex_element"),
    );
    // C.
    let mut c = String::new();
    let _ = writeln!(c, "/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */\n/* The serving glue's first slice: one trampoline per pointer, flattening what Mesa passes and calling Rust's table\n * by value; C frees what it allocated only when Rust answers that it was live. */\n#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\n\n#include \"pipe/p_context.h\"\n#include \"pipe/p_defines.h\"\n#include \"pipe/p_screen.h\"\n#include \"pipe/p_state.h\"\n\n#include \"flat.h\"\n");
    let _ = writeln!(c, "struct wc_keys {{\n    uint32_t count;\n    uintptr_t keys[PIPE_MAX_SAMPLERS];\n}};\n\nstruct wc_vertex_elements {{\n    uint32_t count;\n    struct {} elements[PIPE_MAX_ATTRIBS];\n}};\n", c_flat("pipe_vertex_element"));
    let mut c_table = String::new();
    for (structure, pointer, how) in SLICE {
        let name = entry_name(structure, pointer);
        let (ret, params, _, _) = entry_types(*how);
        let params = if params.is_empty() { String::new() } else { format!(", {params}") };
        let _ = writeln!(c_table, "    {ret} (*{name})(uintptr_t screen, uintptr_t context{params});");
    }
    c_table.push_str(CAPS_TABLE_C);
    let _ = writeln!(c, "/* Rust's table (serve.rs `Table`, the same order). */\nstruct wc_table {{\n{c_table}}};\n\nstruct wc_screen {{\n    struct pipe_screen base;\n    uintptr_t key;\n    const struct wc_table *table;\n}};\n\nstruct wc_context {{\n    struct pipe_context base;\n    uintptr_t key;\n    struct wc_screen *screen;\n}};\n\nstatic void wc_context_pointers(struct pipe_context *pipe);\nuintptr_t wc_screen_create(uintptr_t key, const struct wc_table *table);\nsize_t wc_table_size(void);\n");
    let mut context_pointers = String::new();
    let mut screen_pointers = String::new();
    for (structure, pointer, how) in SLICE {
        c.push_str(&c_trampoline(structure, pointer, *how));
        c.push('\n');
        let line = format!("    pipe->{pointer} = wc_{};\n", entry_name(structure, pointer));
        if *structure == "pipe_screen" { screen_pointers.push_str(&line.replace("pipe->", "screen->")) } else { context_pointers.push_str(&line) }
    }
    let _ = writeln!(c, "static void wc_context_pointers(struct pipe_context *pipe)\n{{\n{context_pointers}}}\n\nuintptr_t wc_screen_create(uintptr_t key, const struct wc_table *table)\n{{\n    struct wc_screen *s = calloc(1, sizeof *s);\n    if (!s || !table)\n        return 0;\n    s->key = key;\n    s->table = table;\n    struct pipe_screen *screen = &s->base;\n{screen_pointers}{CAPS_C}    return (uintptr_t)screen;\n}}\n\n/* The table's size, which the Rust side checks against its own. */\nsize_t wc_table_size(void)\n{{\n    return sizeof(struct wc_table);\n}}");
    Ok((rust_file, c, format!("GALLIUM SERVED: {} pointers of the slice (of {} planned)\n", SLICE.len(), PLAN.len())))
}
