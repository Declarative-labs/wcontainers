//! Gallium's trait layer, planned (AC8): the driver interface Mesa calls in our OpenGL driver (p_screen.h, p_context.h
//! and u_transfer_helper.h's vtable), limited to the function pointers our driver sets (graphics-implementations
//! 92cfc5a76, crates/opengl/src). Each pointer gets its roles over the kinds of object the trait types: a kind is made
//! only by the pointers that make it and given back only by those that take it by value, so a Rust caller can write
//! no other order; the serving glue maps Mesa's handles onto them and refuses a handle of another kind or none.
//! Each pointer names the place in Mesa 26.3.1 (fec8830e9) that calls it, the order the trait follows.
//!
//!   api-crate-generator gallium-plan <in-scope list> <p_defines.h> <p_state.h> <p_screen.h> <p_context.h> [u_transfer_helper.h]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cheader;

/// The kinds of object the trait types, each an associated type: the library (the screen), what it makes, and the
/// places a query and a mapping pass through.
pub const KINDS: &[(&str, &str)] = &[
    ("Screen", "the library: made by the loader, given back by destroy"),
    ("Context", "a rendering context"),
    ("Resource", "a buffer or texture, counted by Mesa (pipe_reference) and given back by resource_destroy at zero"),
    ("Fence", "a point in a context's commands, counted by fence_reference"),
    ("Blend", "a blend state"),
    ("DepthStencilAlpha", "a depth, stencil and alpha state"),
    ("Rasterizer", "a rasterizer state"),
    ("Sampler", "a sampler state"),
    ("VertexElements", "a vertex elements state"),
    ("VertexShader", "a vertex program, made from mesa-compiler's Received of that stage"),
    ("FragmentShader", "a fragment program, from a Received of that stage"),
    ("GeometryShader", "a geometry program, from a Received of that stage"),
    ("TessControlShader", "a tessellation control program, from a Received of that stage"),
    ("TessEvaluationShader", "a tessellation evaluation program, from a Received of that stage"),
    ("ComputeShader", "a compute program, from a Received of that stage"),
    ("SamplerView", "a view of a resource for sampling"),
    ("StreamOutputTarget", "a range of a buffer that streamed outputs are written to"),
    ("Query", "a query that is begun and ended (begin_query, end_query)"),
    ("ActiveQuery", "a query between its begin and its end"),
    ("EndOnlyQuery", "a query only ended (PIPE_QUERY_TIMESTAMP, PIPE_QUERY_GPU_FINISHED: queryobj.c:269 to 277)"),
    ("Mapping", "a resource's bytes mapped for the CPU, between a map and its unmap"),
];

/// What one function pointer does with the kinds.
#[derive(Debug)]
pub enum Role {
    /// Returns an object of one of the kinds, by value.
    Makes(&'static [&'static str]),
    /// Takes an object of one of the kinds by value: its end.
    GivesBack(&'static [&'static str]),
    /// Binds objects of the kind, or none, at slots.
    Binds(&'static str),
    /// Takes an object of the first kind by value and returns it as the second.
    Turns(&'static str, &'static str),
    /// Reads objects of the kinds by reference (the rest of its arguments plain data).
    Reads(&'static [&'static str]),
    /// Counts references to an object of the kind (the glue's clone and drop); the last release ends it.
    Counts(&'static str),
}

pub struct Entry {
    pub structure: &'static str,
    pub pointer: &'static str,
    pub roles: &'static [Role],
    /// The first place in Mesa that calls it (file:line under src/).
    pub caller: &'static str,
}

/// Every function pointer in scope, with its roles and its caller.
pub const PLAN: &[Entry] = &[
    Entry { structure: "pipe_screen", pointer: "context_create", roles: &[Role::Makes(&["Context"])], caller: "mesa/state_tracker/st_manager.c:1023" },
    Entry { structure: "pipe_screen", pointer: "destroy", roles: &[Role::GivesBack(&["Screen"])], caller: "gallium/frontends/dri/dri_screen.c:580" },
    Entry { structure: "pipe_screen", pointer: "fence_finish", roles: &[Role::Reads(&["Fence"])], caller: "mesa/state_tracker/st_manager.c:819" },
    Entry { structure: "pipe_screen", pointer: "fence_reference", roles: &[Role::Counts("Fence")], caller: "mesa/state_tracker/st_interop.c:440" },
    Entry { structure: "pipe_screen", pointer: "flush_frontbuffer", roles: &[Role::Reads(&["Resource"])], caller: "gallium/frontends/dri/drisw.c:173" },
    Entry { structure: "pipe_screen", pointer: "get_device_vendor", roles: &[Role::Reads(&[])], caller: "gallium/frontends/dri/dri_util.c:1140" },
    Entry { structure: "pipe_screen", pointer: "get_name", roles: &[Role::Reads(&[])], caller: "mesa/main/getstring.c:149" },
    Entry { structure: "pipe_screen", pointer: "get_vendor", roles: &[Role::Reads(&[])], caller: "mesa/main/getstring.c:143" },
    Entry { structure: "pipe_screen", pointer: "is_format_supported", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_cb_eglimage.c:48" },
    Entry { structure: "pipe_screen", pointer: "resource_create", roles: &[Role::Makes(&["Resource"])], caller: "mesa/state_tracker/st_texture.c:106" },
    Entry { structure: "pipe_screen", pointer: "resource_destroy", roles: &[Role::GivesBack(&["Resource"])], caller: "gallium/auxiliary/util/u_inlines.h:183" },
    Entry { structure: "pipe_context", pointer: "begin_query", roles: &[Role::Turns("Query", "ActiveQuery")], caller: "mesa/main/performance_monitor.c:198" },
    Entry { structure: "pipe_context", pointer: "bind_blend_state", roles: &[Role::Binds("Blend")], caller: "gallium/auxiliary/cso_cache/cso_context.c:449" },
    Entry { structure: "pipe_context", pointer: "bind_compute_state", roles: &[Role::Binds("ComputeShader")], caller: "gallium/auxiliary/cso_cache/cso_context.c:525" },
    Entry { structure: "pipe_context", pointer: "bind_depth_stencil_alpha_state", roles: &[Role::Binds("DepthStencilAlpha")], caller: "gallium/auxiliary/cso_cache/cso_context.c:510" },
    Entry { structure: "pipe_context", pointer: "bind_fs_state", roles: &[Role::Binds("FragmentShader")], caller: "gallium/auxiliary/cso_cache/cso_context.c:513" },
    Entry { structure: "pipe_context", pointer: "bind_gs_state", roles: &[Role::Binds("GeometryShader")], caller: "mesa/state_tracker/st_context.c:957" },
    Entry { structure: "pipe_context", pointer: "bind_rasterizer_state", roles: &[Role::Binds("Rasterizer")], caller: "gallium/auxiliary/cso_cache/cso_context.c:450" },
    Entry { structure: "pipe_context", pointer: "bind_sampler_states", roles: &[Role::Binds("Sampler")], caller: "gallium/auxiliary/cso_cache/cso_context.c:493" },
    Entry { structure: "pipe_context", pointer: "bind_tcs_state", roles: &[Role::Binds("TessControlShader")], caller: "gallium/auxiliary/cso_cache/cso_context.c:521" },
    Entry { structure: "pipe_context", pointer: "bind_tes_state", roles: &[Role::Binds("TessEvaluationShader")], caller: "gallium/auxiliary/cso_cache/cso_context.c:522" },
    Entry { structure: "pipe_context", pointer: "bind_vertex_elements_state", roles: &[Role::Binds("VertexElements")], caller: "gallium/auxiliary/cso_cache/cso_context.c:531" },
    Entry { structure: "pipe_context", pointer: "bind_vs_state", roles: &[Role::Binds("VertexShader")], caller: "gallium/auxiliary/cso_cache/cso_context.c:515" },
    Entry { structure: "pipe_context", pointer: "blit", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_cb_readpixels.c:350" },
    Entry { structure: "pipe_context", pointer: "buffer_map", roles: &[Role::Makes(&["Mapping"]), Role::Reads(&["Resource"])], caller: "gallium/auxiliary/util/u_inlines.h:530" },
    Entry { structure: "pipe_context", pointer: "buffer_subdata", roles: &[Role::Reads(&["Resource"])], caller: "mesa/main/bufferobj.c:116" },
    Entry { structure: "pipe_context", pointer: "buffer_unmap", roles: &[Role::GivesBack(&["Mapping"])], caller: "gallium/auxiliary/util/u_inlines.h:565" },
    Entry { structure: "pipe_context", pointer: "clear", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_cb_clear.c:521" },
    Entry { structure: "pipe_context", pointer: "clear_depth_stencil", roles: &[Role::Reads(&["Resource"])], caller: "gallium/auxiliary/util/u_threaded_context.c:5469" },
    Entry { structure: "pipe_context", pointer: "clear_render_target", roles: &[Role::Reads(&["Resource"])], caller: "gallium/auxiliary/util/u_threaded_context.c:5422" },
    Entry { structure: "pipe_context", pointer: "create_blend_state", roles: &[Role::Makes(&["Blend"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:649" },
    Entry { structure: "pipe_context", pointer: "create_compute_state", roles: &[Role::Makes(&["ComputeShader"])], caller: "gallium/auxiliary/util/u_tests.c:844" },
    Entry { structure: "pipe_context", pointer: "create_depth_stencil_alpha_state", roles: &[Role::Makes(&["DepthStencilAlpha"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:709" },
    Entry { structure: "pipe_context", pointer: "create_fs_state", roles: &[Role::Makes(&["FragmentShader"])], caller: "mesa/state_tracker/st_program.c:573" },
    Entry { structure: "pipe_context", pointer: "create_gs_state", roles: &[Role::Makes(&["GeometryShader"])], caller: "mesa/state_tracker/st_program.c:570" },
    Entry { structure: "pipe_context", pointer: "create_query", roles: &[Role::Makes(&["Query", "EndOnlyQuery"])], caller: "mesa/main/performance_monitor.c:125" },
    Entry { structure: "pipe_context", pointer: "create_rasterizer_state", roles: &[Role::Makes(&["Rasterizer"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:777" },
    Entry { structure: "pipe_context", pointer: "create_sampler_state", roles: &[Role::Makes(&["Sampler"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:1591" },
    Entry { structure: "pipe_context", pointer: "create_sampler_view", roles: &[Role::Makes(&["SamplerView"]), Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_sampler_view.c:524" },
    Entry { structure: "pipe_context", pointer: "create_stream_output_target", roles: &[Role::Makes(&["StreamOutputTarget"]), Role::Reads(&["Resource"])], caller: "mesa/main/transformfeedback.c:445" },
    Entry { structure: "pipe_context", pointer: "create_tcs_state", roles: &[Role::Makes(&["TessControlShader"])], caller: "mesa/state_tracker/st_program.c:564" },
    Entry { structure: "pipe_context", pointer: "create_tes_state", roles: &[Role::Makes(&["TessEvaluationShader"])], caller: "mesa/state_tracker/st_program.c:567" },
    Entry { structure: "pipe_context", pointer: "create_vertex_elements_state", roles: &[Role::Makes(&["VertexElements"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:1409" },
    Entry { structure: "pipe_context", pointer: "create_vs_state", roles: &[Role::Makes(&["VertexShader"])], caller: "mesa/state_tracker/st_program.c:561" },
    Entry { structure: "pipe_context", pointer: "delete_blend_state", roles: &[Role::GivesBack(&["Blend"])], caller: "gallium/auxiliary/cso_cache/cso_cache.c:46" },
    Entry { structure: "pipe_context", pointer: "delete_compute_state", roles: &[Role::GivesBack(&["ComputeShader"])], caller: "mesa/state_tracker/st_context.c:312" },
    Entry { structure: "pipe_context", pointer: "delete_depth_stencil_alpha_state", roles: &[Role::GivesBack(&["DepthStencilAlpha"])], caller: "gallium/auxiliary/cso_cache/cso_cache.c:52" },
    Entry { structure: "pipe_context", pointer: "delete_fs_state", roles: &[Role::GivesBack(&["FragmentShader"])], caller: "mesa/state_tracker/st_context.c:296" },
    Entry { structure: "pipe_context", pointer: "delete_gs_state", roles: &[Role::GivesBack(&["GeometryShader"])], caller: "mesa/state_tracker/st_context.c:300" },
    Entry { structure: "pipe_context", pointer: "delete_rasterizer_state", roles: &[Role::GivesBack(&["Rasterizer"])], caller: "gallium/auxiliary/cso_cache/cso_cache.c:56" },
    Entry { structure: "pipe_context", pointer: "delete_sampler_state", roles: &[Role::GivesBack(&["Sampler"])], caller: "gallium/auxiliary/cso_cache/cso_cache.c:49" },
    Entry { structure: "pipe_context", pointer: "delete_tcs_state", roles: &[Role::GivesBack(&["TessControlShader"])], caller: "mesa/state_tracker/st_context.c:304" },
    Entry { structure: "pipe_context", pointer: "delete_tes_state", roles: &[Role::GivesBack(&["TessEvaluationShader"])], caller: "mesa/state_tracker/st_context.c:308" },
    Entry { structure: "pipe_context", pointer: "delete_vertex_elements_state", roles: &[Role::GivesBack(&["VertexElements"])], caller: "gallium/auxiliary/cso_cache/cso_cache.c:59" },
    Entry { structure: "pipe_context", pointer: "delete_vs_state", roles: &[Role::GivesBack(&["VertexShader"])], caller: "mesa/state_tracker/st_context.c:292" },
    Entry { structure: "pipe_context", pointer: "destroy", roles: &[Role::GivesBack(&["Context"])], caller: "mesa/state_tracker/st_context.c:370" },
    Entry { structure: "pipe_context", pointer: "destroy_query", roles: &[Role::GivesBack(&["Query", "EndOnlyQuery", "ActiveQuery"])], caller: "mesa/main/performance_monitor.c:159" },
    Entry { structure: "pipe_context", pointer: "draw_vbo", roles: &[Role::Reads(&["Resource"])], caller: "gallium/auxiliary/cso_cache/cso_context.h:314" },
    Entry { structure: "pipe_context", pointer: "end_query", roles: &[Role::Turns("ActiveQuery", "Query"), Role::Reads(&["EndOnlyQuery"])], caller: "mesa/main/performance_monitor.c:223" },
    Entry { structure: "pipe_context", pointer: "flush", roles: &[Role::Makes(&["Fence"])], caller: "mesa/state_tracker/st_interop.c:438" },
    Entry { structure: "pipe_context", pointer: "flush_resource", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_interop.c:373" },
    Entry { structure: "pipe_context", pointer: "generate_mipmap", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_gen_mipmap.c:145" },
    Entry { structure: "pipe_context", pointer: "get_device_reset_status", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_cb_flush.c:162" },
    Entry { structure: "pipe_context", pointer: "get_query_result", roles: &[Role::Reads(&["Query", "EndOnlyQuery"])], caller: "mesa/main/performance_monitor.c:259" },
    Entry { structure: "pipe_context", pointer: "get_sample_position", roles: &[Role::Reads(&[])], caller: "mesa/main/multisample.c:96" },
    Entry { structure: "pipe_context", pointer: "get_timestamp", roles: &[Role::Reads(&[])], caller: "mesa/main/queryobj.c:417" },
    Entry { structure: "pipe_context", pointer: "launch_grid", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_pbo_compute.c:1170" },
    Entry { structure: "pipe_context", pointer: "memory_barrier", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_cb_readpixels.c:245" },
    Entry { structure: "pipe_context", pointer: "render_condition", roles: &[Role::Reads(&["Query", "EndOnlyQuery"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:1084" },
    Entry { structure: "pipe_context", pointer: "resource_copy_region", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_texture.c:423" },
    Entry { structure: "pipe_context", pointer: "resource_release", roles: &[Role::Counts("Resource")], caller: "gallium/auxiliary/util/u_inlines.h:209" },
    Entry { structure: "pipe_context", pointer: "sampler_view_destroy", roles: &[Role::GivesBack(&["SamplerView"])], caller: "gallium/auxiliary/util/u_inlines.h:248" },
    Entry { structure: "pipe_context", pointer: "sampler_view_release", roles: &[Role::Counts("SamplerView")], caller: "mesa/state_tracker/st_sampler_view.c:92" },
    Entry { structure: "pipe_context", pointer: "set_active_query_state", roles: &[Role::Reads(&[])], caller: "gallium/auxiliary/cso_cache/cso_context.c:1919" },
    Entry { structure: "pipe_context", pointer: "set_blend_color", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_blend.c:367" },
    Entry { structure: "pipe_context", pointer: "set_clip_state", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_clip.c:67" },
    Entry { structure: "pipe_context", pointer: "set_constant_buffer", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_atom_constbuf.c:60" },
    Entry { structure: "pipe_context", pointer: "set_framebuffer_state", roles: &[Role::Reads(&["Resource"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:538" },
    Entry { structure: "pipe_context", pointer: "set_patch_vertices", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_tess.c:53" },
    Entry { structure: "pipe_context", pointer: "set_polygon_stipple", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_stipple.c:86" },
    Entry { structure: "pipe_context", pointer: "set_sample_mask", roles: &[Role::Reads(&[])], caller: "gallium/auxiliary/cso_cache/cso_context.c:573" },
    Entry { structure: "pipe_context", pointer: "set_sampler_views", roles: &[Role::Binds("SamplerView")], caller: "mesa/state_tracker/st_cb_readpixels.c:184" },
    Entry { structure: "pipe_context", pointer: "set_scissor_states", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_scissor.c:106" },
    Entry { structure: "pipe_context", pointer: "set_shader_buffers", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_draw_hw_select.c:703" },
    Entry { structure: "pipe_context", pointer: "set_shader_images", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_atom_image.c:161" },
    Entry { structure: "pipe_context", pointer: "set_stencil_ref", roles: &[Role::Reads(&[])], caller: "gallium/auxiliary/cso_cache/cso_context.c:512" },
    Entry { structure: "pipe_context", pointer: "set_stream_output_targets", roles: &[Role::Binds("StreamOutputTarget")], caller: "gallium/auxiliary/cso_cache/cso_context.c:535" },
    Entry { structure: "pipe_context", pointer: "set_vertex_buffers", roles: &[Role::Reads(&["Resource"])], caller: "gallium/auxiliary/cso_cache/cso_context.c:532" },
    Entry { structure: "pipe_context", pointer: "set_viewport_states", roles: &[Role::Reads(&[])], caller: "mesa/state_tracker/st_atom_viewport.c:81" },
    Entry { structure: "pipe_context", pointer: "stream_output_target_destroy", roles: &[Role::GivesBack(&["StreamOutputTarget"])], caller: "gallium/auxiliary/util/u_inlines.h:293" },
    Entry { structure: "pipe_context", pointer: "texture_barrier", roles: &[Role::Reads(&[])], caller: "mesa/main/barrier.c:118" },
    Entry { structure: "pipe_context", pointer: "texture_map", roles: &[Role::Makes(&["Mapping"]), Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_texture.c:364" },
    Entry { structure: "pipe_context", pointer: "texture_subdata", roles: &[Role::Reads(&["Resource"])], caller: "mesa/state_tracker/st_texcompress_compute.c:534" },
    Entry { structure: "pipe_context", pointer: "texture_unmap", roles: &[Role::GivesBack(&["Mapping"])], caller: "mesa/state_tracker/st_texture.c:368" },
    Entry { structure: "pipe_context", pointer: "transfer_flush_region", roles: &[Role::Reads(&["Mapping"])], caller: "gallium/auxiliary/util/u_inlines.h:589" },
    Entry { structure: "u_transfer_vtbl", pointer: "transfer_map", roles: &[Role::Makes(&["Mapping"]), Role::Reads(&["Resource"])], caller: "gallium/auxiliary/util/u_transfer_helper.c:376" },
    Entry { structure: "u_transfer_vtbl", pointer: "transfer_unmap", roles: &[Role::GivesBack(&["Mapping"])], caller: "gallium/auxiliary/util/u_transfer_helper.c:519" },
    Entry { structure: "u_transfer_vtbl", pointer: "transfer_flush_region", roles: &[Role::Reads(&["Mapping"])], caller: "gallium/auxiliary/util/u_inlines.h:589" },
];

/// Checks the plan against the list of pointers in scope and the headers' census, and prints it by kind: every pointer
/// in scope placed once, each found in its structure, every kind made and given back by some pointer.
pub fn check(in_scope: &str, header: &cheader::Header) -> Result<String, String> {
    let mut report = String::new();
    let mut problems = Vec::new();
    let planned: BTreeSet<(&str, &str)> = PLAN.iter().map(|entry| (entry.structure, entry.pointer)).collect();
    if planned.len() != PLAN.len() {
        problems.push("a pointer is planned twice".to_string());
    }
    let mut listed = BTreeSet::new();
    for line in in_scope.lines().filter(|line| !line.trim().is_empty()) {
        let mut words = line.split_whitespace();
        let (Some(place), Some(pointer)) = (words.next(), words.next()) else { continue };
        let structure = match place {
            "screen" => "pipe_screen",
            "context" => "pipe_context",
            _ => "u_transfer_vtbl",
        };
        listed.insert((structure, pointer));
        if !planned.contains(&(structure, pointer)) {
            problems.push(format!("in scope, not planned: {structure}.{pointer}"));
        }
    }
    for entry in PLAN {
        if !listed.contains(&(entry.structure, entry.pointer)) {
            problems.push(format!("planned, not in scope: {}.{}", entry.structure, entry.pointer));
        }
        match header.callbacks.get(entry.structure) {
            Some(callbacks) if callbacks.iter().any(|function| function.name == entry.pointer) => {}
            Some(_) => problems.push(format!("not in the header: {}.{}", entry.structure, entry.pointer)),
            None => problems.push(format!("no structure {} read", entry.structure)),
        }
    }
    let kinds: BTreeSet<&str> = KINDS.iter().map(|(kind, _)| *kind).collect();
    let mut made: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut ended: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut used: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for entry in PLAN {
        for role in entry.roles {
            let (makes, ends, uses): (Vec<&str>, Vec<&str>, Vec<&str>) = match role {
                Role::Makes(of) => (of.to_vec(), vec![], vec![]),
                Role::GivesBack(of) => (vec![], of.to_vec(), vec![]),
                Role::Turns(from, to) => (vec![to], vec![from], vec![]),
                Role::Binds(of) => (vec![], vec![], vec![of]),
                // A count's last release is the object's end (fence_reference with no new fence).
                Role::Counts(of) => (vec![], vec![of], vec![of]),
                Role::Reads(of) => (vec![], vec![], of.to_vec()),
            };
            for kind in makes.iter().chain(&ends).chain(&uses) {
                if !kinds.contains(kind) {
                    problems.push(format!("{}.{} names no kind {kind}", entry.structure, entry.pointer));
                }
            }
            for kind in makes {
                made.entry(kind).or_default().push(entry.pointer);
            }
            for kind in ends {
                ended.entry(kind).or_default().push(entry.pointer);
            }
            for kind in uses {
                used.entry(kind).or_default().push(entry.pointer);
            }
        }
    }
    for (kind, what) in KINDS {
        let makers = made.get(kind).map_or_else(String::new, |pointers| pointers.join(" "));
        let enders = ended.get(kind).map_or_else(String::new, |pointers| pointers.join(" "));
        let users = used.get(kind).map_or(0, Vec::len);
        // The screen is made by the loader (the driver's own entry point), not by a pointer of the interface.
        if makers.is_empty() && *kind != "Screen" {
            problems.push(format!("kind {kind} is made by no pointer"));
        }
        if enders.is_empty() {
            problems.push(format!("kind {kind} is given back by no pointer"));
        }
        let _ = writeln!(report, "KIND {kind} ({what}): made by [{makers}] given back by [{enders}] used by {users}");
    }
    let total: usize = header.callbacks.get("pipe_screen").map_or(0, Vec::len) + header.callbacks.get("pipe_context").map_or(0, Vec::len);
    let in_headers = PLAN.iter().filter(|entry| entry.structure != "u_transfer_vtbl").count();
    let _ = writeln!(report, "GALLIUM PLAN: {} pointers planned ({in_headers} of the {total} of pipe_screen and pipe_context, {} of u_transfer_vtbl), {} kinds", PLAN.len(), PLAN.len() - in_headers, KINDS.len());
    for entry in PLAN {
        let _ = writeln!(report, "  {}.{} {:?} called at {}", entry.structure, entry.pointer, entry.roles, entry.caller);
    }
    let _ = writeln!(report, "GALLIUM PLAIN DATA: each structure a planned pointer reads, its fields as read, the objects it names");
    for (name, (fields, bits, handles)) in plain_structures(header) {
        let union = if header.unions.contains(&name) { " (a union)" } else { "" };
        let _ = writeln!(report, "  {name}{union}: {fields} fields ({bits} bit-fields), names objects at [{}]", handles.join(" "));
    }
    let (rows, unmapped) = mapping(header);
    let _ = writeln!(report, "GALLIUM MAPPING: parameters and returns by row");
    for (row, count) in &rows {
        let _ = writeln!(report, "  {count} {row}");
    }
    let _ = writeln!(report, "UNMAPPED {}", unmapped.len());
    for item in &unmapped {
        let _ = writeln!(report, "  {item}");
        problems.push(format!("unmapped: {item}"));
    }
    if problems.is_empty() { Ok(report) } else { Err(format!("{report}PROBLEMS {}\n  {}", problems.len(), problems.join("\n  "))) }
}


/// Kinds that a pointer makes, gives back, binds or reads by its roles.
fn kinds_of(entry: &Entry) -> Vec<&'static str> {
    entry
        .roles
        .iter()
        .flat_map(|role| match role {
            Role::Makes(of) | Role::GivesBack(of) | Role::Reads(of) => of.to_vec(),
            Role::Binds(of) | Role::Counts(of) => vec![*of],
            Role::Turns(from, to) => vec![*from, *to],
        })
        .collect()
}

/// The handles Mesa passes for an object of a kind: a structure pointer of its own, or `void *` (the states).
const HANDLES: &[&str] = &[
    "struct pipe_resource",
    "struct pipe_query",
    "struct pipe_sampler_view",
    "struct pipe_stream_output_target",
    "struct pipe_fence_handle",
    "struct pipe_transfer",
    "struct pipe_context",
];

/// Parameters whose C form says less than what Mesa passes (READ at the caller each names), each with its row.
const EXCEPTIONS: &[(&str, &str, &str)] = &[
    ("flush_frontbuffer", "subbox", "plain data read during the call, copied into owned values (not const in C; drisw.c:173 passes a box it does not read back)"),
    ("clear_render_target", "dst", "a surface: plain data naming a resource, read during the call (u_threaded_context.c:5422)"),
    ("clear_depth_stencil", "dst", "a surface: plain data naming a resource, read during the call (u_threaded_context.c:5469)"),
    ("context_create", "priv", "the frontend's own value: carried as a number, never read (st_manager.c:1023 passes its own pointer)"),
    ("flush_frontbuffer", "winsys_drawable_handle", "the frontend's own value: carried as a number, never read (drisw.c:173)"),
];

/// One row of the Gallium mapping rule for a parameter (or the return) of a planned pointer.
fn row(entry: &Entry, param: &cheader::CParam, first: bool, header: &cheader::Header) -> Option<&'static str> {
    if let Some((_, _, row)) = EXCEPTIONS.iter().find(|(pointer, name, _)| *pointer == entry.pointer && *name == param.name) {
        return Some(row);
    }
    let is_const = param.base.starts_with("const ");
    let base = param.base.trim_start_matches("const ").trim();
    let returned = param.name == "return";
    let makes = entry.roles.iter().any(|role| matches!(role, Role::Makes(_)));
    let kinds = kinds_of(entry);
    Some(match (base, param.pointers) {
        ("void", 0) if returned => "nothing",
        ("struct pipe_context", 1) if first => "the context: &self",
        ("struct pipe_screen", 1) if first => "the library: &self",
        ("void" | "struct pipe_resource" | "struct pipe_query" | "struct pipe_sampler_view" | "struct pipe_stream_output_target" | "struct pipe_context", 1)
            if returned && makes && !entry.pointer.ends_with("_map") =>
        {
            "an object made: returned by value, None where refused"
        }
        ("void", 1) if returned => "the mapped bytes: a shadow the glue owns, lent to Mesa until the unmap",
        ("void", 1) if is_const => "bytes read during the call, the count beside them",
        ("void", 1) if !kinds.is_empty() => "an object of the pointer's kind: by value where it gives it back, Option<&> where it binds",
        ("void", 2) | ("struct pipe_sampler_view" | "struct pipe_stream_output_target", 2) => "objects of a kind at slots, with their count",
        ("struct pipe_fence_handle", 2) => "a fence's place: the fence made by a flush, or the count's old and new fence",
        ("struct pipe_transfer", 2) => "a mapping out: returned with the mapped bytes",
        // A handle's own structure passed as const is a template (resource_create's, create_sampler_view's).
        ("struct pipe_resource" | "struct pipe_sampler_view", 1) if is_const => "a template naming objects, read during the call: each handle a reference of its kind, or none",
        (handle, 1) if HANDLES.contains(&handle) && !returned => "an object of the pointer's kinds: by reference, or by value where it gives it back",
        ("unsigned" | "bool" | "uint64_t" | "uint32_t" | "uint8_t" | "double" | "uint" | "int" | "uintptr_t", 0) => "number by value",
        ("unsigned", 1) if is_const => "numbers read during the call, the count beside them",
        (declared, 0) if declared.starts_with("enum ") || declared == "mesa_shader_stage" => "a declared value, checked",
        ("struct pipe_shader_state" | "struct pipe_compute_state", 1) if is_const => "a program: mesa-compiler's Received taken from it, the rest plain data",
        (plain, 1) if is_const && (plain.starts_with("struct pipe_") || plain.starts_with("union pipe_")) && names_objects(header, plain) => {
            "plain data naming objects, read during the call: each handle a reference of its kind, refused when it is none"
        }
        (plain, 1) if is_const && (plain.starts_with("struct pipe_") || plain.starts_with("union pipe_")) => "plain data read during the call, copied into owned values",
        (plain, 0) if plain.starts_with("struct pipe_") => "plain data by value",
        ("char", 1) if is_const && returned => "text out: the driver's own text, copied",
        ("float", 1) | ("union pipe_query_result", 1) if !is_const => "plain data out: written from the owned value returned",
        _ => return None,
    })
}

/// Every parameter and return of every planned pointer of the two headers, by row; the unmapped ones listed.
fn mapping(header: &cheader::Header) -> (BTreeMap<&'static str, usize>, Vec<String>) {
    let mut rows = BTreeMap::new();
    let mut unmapped = Vec::new();
    for entry in PLAN {
        let Some(function) = header.callbacks.get(entry.structure).and_then(|callbacks| callbacks.iter().find(|function| function.name == entry.pointer)) else { continue };
        for (at, param) in std::iter::once(&function.ret).chain(&function.params).enumerate() {
            match row(entry, param, at == 1, header) {
                Some(row) => *rows.entry(row).or_default() += 1,
                None => unmapped.push(format!("{}.{} {}: {}{}", entry.structure, entry.pointer, param.name, param.base, "*".repeat(param.pointers))),
            }
        }
    }
    (rows, unmapped)
}

/// The structures a planned pointer reads as plain data, each with its field count as read and the fields (reached
/// through structures held by value) that hold a handle of one of the kinds.
fn plain_structures(header: &cheader::Header) -> BTreeMap<String, (usize, usize, Vec<String>)> {
    let mut out = BTreeMap::new();
    for entry in PLAN {
        let Some(function) = header.callbacks.get(entry.structure).and_then(|callbacks| callbacks.iter().find(|function| function.name == entry.pointer)) else { continue };
        for param in &function.params {
            let base = param.base.trim_start_matches("const ").trim();
            let Some(name) = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union ")) else { continue };
            // A handle's own structure passed as const is a template (resource_create's, create_sampler_view's).
            if (HANDLES.contains(&base) && !param.base.starts_with("const ")) || name == "pipe_screen" {
                continue;
            }
            let mut handles = Vec::new();
            collect_handles(header, name, "", 0, &mut handles);
            let fields = header.structures.get(name).map_or(0, Vec::len);
            let bits = header.structures.get(name).map_or(0, |fields| fields.iter().filter(|field| field.bits.is_some()).count());
            out.insert(name.to_string(), (fields, bits, handles));
        }
    }
    out
}

/// Whether a structure (or a structure it holds by value) holds a handle of one of the kinds.
fn names_objects(header: &cheader::Header, base: &str) -> bool {
    let name = base.trim_start_matches("struct ").trim_start_matches("union ");
    let mut handles = Vec::new();
    collect_handles(header, name, "", 0, &mut handles);
    !handles.is_empty()
}

/// The fields of a structure, and of those it holds by value, that hold a handle of one of the kinds.
fn collect_handles(header: &cheader::Header, name: &str, path: &str, depth: usize, out: &mut Vec<String>) {
    let Some(fields) = header.structures.get(name) else { return };
    for field in fields {
        let base = field.base.trim_start_matches("const ").trim();
        let at = if path.is_empty() { field.name.clone() } else { format!("{path}.{}", field.name) };
        if field.pointers >= 1 && HANDLES.contains(&base) {
            out.push(format!("{at}:{base}"));
        } else if field.pointers == 0
            && depth < 4
            && let Some(inner) = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union "))
        {
            collect_handles(header, inner, &at, depth + 1, out);
        }
    }
}
