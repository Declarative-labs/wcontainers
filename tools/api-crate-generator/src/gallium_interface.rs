//! Writes Gallium's trait layer (AC8) from the plan (gallium.rs) and the pinned Mesa's headers: `crates/gallium/
//! interface.rs`, with the trait `Gallium` (the screen: the library, holding every kind of object as an associated
//! type, the shader kinds made from mesa-compiler's received program of their stage) and `GalliumContext` (a rendering
//! context), one method per planned pointer (a pointer taking objects of several kinds is one method per kind), and
//! every plain structure a pointer reads as owned data, each handle in it a reference of its kind.
//!
//!   api-crate-generator gallium <in-scope list> <headers...> <output folder>
//!
//! A parameter or a field no rule maps fails the generator, listed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cheader::{CFunction, CParam, Header};
use crate::gallium::{Entry, PLAN, Role};

/// The first line of every file written.
const HEADER: &str = "// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers (p_defines.h, p_state.h, p_screen.h, p_context.h, u_transfer_helper.h) and the plan in src/gallium.rs. Do not edit.\n";

/// The C handle of each kind's objects, as Mesa passes it inside plain data.
const HANDLE_KINDS: &[(&str, &str)] = &[
    ("struct pipe_resource", "Resource"),
    ("struct pipe_stream_output_target", "StreamOutputTarget"),
    ("struct pipe_context", "Context"),
    ("struct pipe_sampler_view", "SamplerView"),
];

/// Each shader kind's received program, by the mesa-compiler trait's type.
const SHADERS: &[(&str, &str)] = &[
    ("VertexShader", "ReceivedVertex"),
    ("FragmentShader", "ReceivedFragment"),
    ("GeometryShader", "ReceivedGeometry"),
    ("TessControlShader", "ReceivedTessControl"),
    ("TessEvaluationShader", "ReceivedTessEvaluation"),
    ("ComputeShader", "ReceivedCompute"),
];

/// Fields of the plain structures the trait does not carry, each with its reason (READ in p_state.h and at the
/// callers the plan names).
const FIELDS_LEFT_OUT: &[(&str, &str, &str)] = &[
    ("pipe_resource", "screen", "the screen that made it: the glue's own, the implementation is the screen"),
    ("pipe_resource", "reference", "Mesa's count: the glue's, as every count (Role::Counts)"),
    ("pipe_resource", "reference)", "Mesa's count, declared through the EXCLUSIVE_CACHELINE macro (the header reader names it so): the glue's"),
    ("pipe_resource", "clientptr_heap", "the frontend's heap of client pointers, which the driver never reads (the glue's, zero)"),
    ("pipe_resource", "next", "a chain of planes the driver makes itself; resource_create's template has none (st_texture.c:106)"),
    ("pipe_sampler_view", "reference", "Mesa's count: the glue's"),
    ("pipe_sampler_view", "context", "the context the view is made on: the method's receiver"),
    ("pipe_sampler_view", "texture", "the resource viewed: create_sampler_view's own parameter"),
    ("pipe_stream_output_target", "reference", "Mesa's count: the glue's"),
    ("pipe_draw_info.index", "user", "client memory as indices: refused, the driver reports no user index buffers (PIPE_CAP has_user_indices false)"),
    ("pipe_grid_info", "input", "kernel input of OpenCL's frontend, which our driver does not serve (st/mesa passes none)"),
    ("pipe_grid_info", "globals", "global buffers of OpenCL's frontend (none from st/mesa)"),
    ("pipe_grid_info", "num_globals", "the count of `globals`, left out with it"),
    ("pipe_shader_state", "ir", "the program: the method's received program of its stage"),
    ("pipe_shader_state", "type", "the program's form: always NIR from st/mesa (PIPE_SHADER_IR_NIR), the received program's"),
    ("pipe_shader_state", "tokens", "TGSI tokens, which st/mesa does not pass with NIR"),
    ("pipe_shader_state", "error_message", "a text the driver allocates for Mesa: the glue writes none"),
    ("pipe_compute_state", "ir_type", "always NIR from st/mesa, the received program's"),
    ("pipe_compute_state", "prog", "the program: the method's received program"),
    ("pipe_compute_state", "error_message", "a text the driver allocates for Mesa: the glue writes none"),
    ("pipe_vertex_buffer.buffer", "user", "client memory as a vertex buffer: refused, the driver reports no user vertex buffers"),
    ("pipe_constant_buffer", "user_buffer", "client memory as a constant buffer: refused, the driver reports no user constant buffers"),
];

/// A field's declared type: the header reader gives a second declarator of one declaration (`float min_lod, max_lod;`)
/// the first's name with its type (`float min_lod,`), dropped here.
fn declared_type(base: &str) -> String {
    let base = base.trim_start_matches("const ").trim();
    match base.rsplit_once(' ') {
        Some((ty, last)) if last.ends_with(',') => ty.to_string(),
        _ => base.to_string(),
    }
}

/// Parameters Mesa passes as NULL (READ at the callers): a single value becomes an `Option`, a list `Option<&[T]>` with
/// its count kept (None unbinds that many slots).
const NULLABLE: &[(&str, &str, &str)] = &[
    ("draw_vbo", "indirect", "NULL for a direct draw (cso_context.h:314)"),
    ("clear", "scissor_state", "NULL for an unscissored clear (st_cb_clear.c:521)"),
    ("set_constant_buffer", "buf", "NULL unbinds the slot (st_atom_constbuf.c)"),
    ("set_shader_buffers", "buffers", "NULL unbinds the slots (st_atom_storagebuf.c)"),
    ("set_shader_images", "images", "NULL unbinds the slots (st_atom_image.c)"),
    ("set_sampler_views", "views", "NULL unbinds the slots (st_atom_texture.c)"),
];

fn nullable(pointer: &str, param: &str) -> bool {
    NULLABLE.iter().any(|(p, name, _)| *p == pointer && *name == param)
}

/// Whether a parameter is a count of a list given later in the same call.
fn is_count(function: &CFunction, at: usize) -> bool {
    let param = &function.params[at];
    (param.name.starts_with("num_") || param.name == "count" || param.name == "nboxes")
        && function.params[at + 1..].iter().any(|later| later.pointers >= 1 && !later.base.contains("unsigned"))
}

/// Whether a parameter is the count of the list just before it (draw_vbo's `draws, num_draws`).
fn counts_the_list_before(function: &CFunction, at: usize) -> bool {
    let param = &function.params[at];
    param.pointers == 0 && param.name.starts_with("num_") && at > 0 && function.params[at - 1].pointers == 1 && !function.params[at - 1].base.starts_with("const unsigned")
}

/// The number type of a C scalar.
fn number(base: &str) -> Option<&'static str> {
    Some(match base {
        "unsigned" | "uint32_t" | "uint" | "unsigned int" => "u32",
        "int" | "int32_t" => "i32",
        "uint8_t" | "unsigned char" => "u8",
        "int8_t" => "i8",
        "uint16_t" | "unsigned short" => "u16",
        "int16_t" | "short" => "i16",
        "uint64_t" | "uintptr_t" | "size_t" => "u64",
        "int64_t" => "i64",
        "float" => "f32",
        "double" => "f64",
        "bool" => "bool",
        declared if declared.starts_with("enum ") || declared == "mesa_shader_stage" || declared == "gl_shader_stage" => "u32",
        _ => return None,
    })
}

/// `pipe_draw_info.index` to `DrawInfoIndex`; a name Rust's prelude has (`pipe_box`) keeps the `Pipe`.
fn type_name(c: &str) -> String {
    let bare = c.trim_start_matches("struct ").trim_start_matches("union ").trim_start_matches("pipe_");
    let name: String = bare
        .split(['.', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map(|first| first.to_ascii_uppercase().to_string() + letters.as_str()).unwrap_or_default()
        })
        .collect();
    if ["Box", "Option", "Result", "Vec", "String"].contains(&name.as_str()) {
        format!("Pipe{name}")
    } else if crate::gallium::KINDS.iter().any(|(kind, _)| *kind == name) {
        // Mesa's structure of a kind's own handle, passed as const, is a template for one (resource_create's).
        format!("{name}Template")
    } else {
        name
    }
}

/// A Rust keyword gets a trailing underscore.
fn field_name(c: &str) -> String {
    crate::names::escaped(c.to_string())
}

/// The kinds a structure's handles name, in the order met (its type parameters).
fn kinds_named(name: &str, header: &Header, out: &mut Vec<&'static str>, depth: usize) {
    let Some(fields) = header.structures.get(name) else { return };
    for field in fields {
        if left_out(name, &field.name).is_some() {
            continue;
        }
        let base = field.base.trim_start_matches("const ").trim();
        if field.pointers == 1 && let Some((_, kind)) = HANDLE_KINDS.iter().find(|(handle, _)| *handle == base) {
            if !out.contains(kind) {
                out.push(kind);
            }
        } else if field.pointers == 0
            && depth < 6
            && let Some(inner) = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union "))
        {
            kinds_named(inner, header, out, depth + 1);
        }
    }
}

fn left_out(structure: &str, field: &str) -> Option<&'static str> {
    FIELDS_LEFT_OUT.iter().find(|(s, f, _)| *s == structure && *f == field).map(|(_, _, reason)| *reason)
}

/// The owned type of a structure, its handles named `prefix` + kind (`Self::Resource` in the trait, `Resource` inside
/// another structure).
fn owned(name: &str, header: &Header, prefix: &str) -> String {
    let mut kinds = Vec::new();
    kinds_named(name, header, &mut kinds, 0);
    if kinds.is_empty() {
        type_name(name)
    } else {
        let params: Vec<String> = kinds.iter().map(|kind| format!("{prefix}{kind}")).collect();
        format!("{}<'a, {}>", type_name(name), params.join(", "))
    }
}

/// Writes the owned structures reached from `name`, once each.
struct Structures<'h> {
    header: &'h Header,
    written: BTreeSet<String>,
    text: String,
    unmapped: Vec<String>,
    defines: BTreeMap<String, String>,
}

impl Structures<'_> {
    fn length(&self, array: &str) -> Option<usize> {
        if let Ok(number) = array.parse() {
            return Some(number);
        }
        let mut value = self.defines.get(array)?.trim().trim_start_matches('(').trim_end_matches(')').to_string();
        for _ in 0..4 {
            if let Ok(number) = value.parse() {
                return Some(number);
            }
            value = self.defines.get(&value)?.clone();
        }
        None
    }

    /// The Rust type of one field, writing the structures it holds.
    fn field_type(&mut self, structure: &str, field: &CParam) -> Option<String> {
        let base = declared_type(&field.base);
        let base = base.as_str();
        let mut ty = match field.pointers {
            0 => {
                if let Some(ty) = number(base) {
                    ty.to_string()
                } else if let Some(inner) = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union ")) {
                    if inner == "pipe_color_union" || (self.header.unions.iter().any(|union| union == inner) && !inner.contains('.')) {
                        // A union of four-byte values (pipe_color_union): its words.
                        "[u32; 4]".to_string()
                    } else {
                        self.write(inner);
                        owned(inner, self.header, "")
                    }
                } else {
                    self.unmapped.push(format!("{structure}.{}: {base}", field.name));
                    return None;
                }
            }
            1 => match HANDLE_KINDS.iter().find(|(handle, _)| *handle == base) {
                Some((_, kind)) => format!("Option<&'a {kind}>"),
                None => {
                    self.unmapped.push(format!("{structure}.{}: {base}*", field.name));
                    return None;
                }
            },
            _ => {
                self.unmapped.push(format!("{structure}.{}: {base} with {} pointers", field.name, field.pointers));
                return None;
            }
        };
        if let Some(array) = &field.array {
            // `A][B` is an array of arrays: the last length innermost.
            for length in array.split("][").collect::<Vec<_>>().into_iter().rev() {
                match self.length(length.trim()) {
                    Some(length) => ty = format!("[{ty}; {length}]"),
                    None => {
                        self.unmapped.push(format!("{structure}.{}: array length {array}", field.name));
                        return None;
                    }
                }
            }
        }
        Some(ty)
    }

    fn write(&mut self, name: &str) {
        if !self.written.insert(name.to_string()) {
            return;
        }
        let Some(fields) = self.header.structures.get(name).cloned() else {
            self.unmapped.push(format!("{name}: no structure read"));
            return;
        };
        let mut kinds = Vec::new();
        kinds_named(name, self.header, &mut kinds, 0);
        let generics = if kinds.is_empty() { String::new() } else { format!("<'a, {}>", kinds.join(", ")) };
        let union = self.header.unions.iter().any(|union| union == name);
        let mut body = String::new();
        for field in &fields {
            if let Some(reason) = left_out(name, &field.name) {
                let _ = writeln!(body, "    // `{}` is not carried: {reason}.", field.name);
                continue;
            }
            let Some(ty) = self.field_type(name, field) else { continue };
            let bits = field.bits.as_ref().map(|bits| format!(" ({bits} bits in C: a value past them is refused)")).unwrap_or_default();
            let _ = writeln!(body, "    /// The `{}` of `{name}`{bits}.\n    pub {}: {ty},", field.name, field_name(&field.name));
        }
        let what = if union { "union" } else { "struct" };
        let union_note = if union { " A union in C: the glue fills the one member the structure around it selects, the others zero." } else { "" };
        let derive = if kinds.is_empty() { "#[derive(Clone, Copy, Debug, PartialEq)]" } else { "#[derive(Debug)]" };
        let _ = writeln!(
            self.text,
            "/// Mesa's `{what} {name}`, owned{}.{union_note}\n{derive}\npub struct {}{generics} {{\n{body}}}\n",
            if kinds.is_empty() { String::new() } else { ", each object it names a reference of its kind".to_string() },
            type_name(name)
        );
    }
}

/// Clippy's most arguments for a function (`too_many_arguments`).
const MOST_ARGUMENTS: usize = 7;

/// The trait each pointer's method goes on.
fn on_screen(entry: &Entry) -> bool {
    entry.structure == "pipe_screen"
}

/// The kinds of objects a method reads or takes in place of a C handle: one method per kind.
fn split_kinds(entry: &Entry) -> Vec<Option<&'static str>> {
    for role in entry.roles {
        match role {
            Role::GivesBack(kinds) | Role::Makes(kinds) if kinds.len() > 1 => return kinds.iter().map(|kind| Some(*kind)).collect(),
            Role::Reads(kinds) if kinds.len() > 1 => return kinds.iter().map(|kind| Some(*kind)).collect(),
            _ => {}
        }
    }
    vec![None]
}

/// A method's name for one of its kinds: the pointer's, with the kind for the kinds after the first; the transfer
/// helper's vtable (u_transfer_helper.h), whose names are pipe_context's own, as `helper_`.
fn method_name(entry: &Entry, kind: Option<&str>, first: bool) -> String {
    let pointer = if entry.structure == "u_transfer_vtbl" { format!("helper_{}", entry.pointer) } else { entry.pointer.to_string() };
    match kind {
        Some(kind) if !first => format!("{pointer}_{}", crate::names::snake_case(kind)),
        _ => pointer,
    }
}

/// `Self::Resource` in the context's trait is `<Self::Gallium as Gallium>::Resource`.
fn kind_type(kind: &str, screen: bool) -> String {
    if kind == "Context" && !screen {
        return "Self".to_string();
    }
    if screen { format!("Self::{kind}") } else { format!("<Self::Screen as Gallium>::{kind}") }
}

/// The handles' type parameters of an owned structure in a method.
fn owned_in_method(name: &str, header: &Header, screen: bool) -> String {
    let mut kinds = Vec::new();
    kinds_named(name, header, &mut kinds, 0);
    if kinds.is_empty() {
        type_name(name)
    } else {
        let params: Vec<String> = kinds.iter().map(|kind| kind_type(kind, screen)).collect();
        format!("{}<'_, {}>", type_name(name), params.join(", "))
    }
}

/// One method's parameters and return, from the pointer's C form and roles.
fn signature(entry: &Entry, function: &CFunction, kind: Option<&str>, header: &Header, structures: &mut Structures<'_>) -> Result<(Vec<String>, String), String> {
    let screen = on_screen(entry);
    let mut params = Vec::new();
    let mut ret = String::new();
    let made_kind = || {
        entry.roles.iter().find_map(|role| match role {
            Role::Makes(kinds) => Some(kind.unwrap_or(kinds[0])),
            Role::Turns(_, to) => Some(*to),
            _ => None,
        })
    };
    let names: Vec<String> = function
        .params
        .iter()
        .enumerate()
        .map(|(at, param)| if param.name.is_empty() { format!("arg{at}") } else { field_name(&param.name) })
        .collect();
    let mut skip = BTreeSet::new();
    for (at, param) in function.params.iter().enumerate() {
        let base = param.base.trim_start_matches("const ").trim();
        let is_const = param.base.starts_with("const ");
        let name = &names[at];
        if at == 0 {
            continue;
        }
        if skip.contains(&at) {
            continue;
        }
        let param_text = match (base, param.pointers) {
            (b, 0) if number(b).is_some() => {
                // A count is its list's length, unless the list may be NULL (then None unbinds that many slots).
                let list_nullable = function.params[at + 1..].iter().any(|later| later.pointers >= 1 && nullable(entry.pointer, &later.name));
                if (is_count(function, at) && !list_nullable) || counts_the_list_before(function, at) {
                    continue;
                }
                format!("{name}: {}", number(b).unwrap_or("u32"))
            }
            ("void", 1) if !is_const && (entry.pointer == "context_create" || entry.pointer == "flush_frontbuffer") => {
                format!("{name}: u64")
            }
            ("void", 1) if is_const => {
                if let Some(size) = function.params.iter().position(|p| p.name == "size") {
                    skip.insert(size);
                }
                format!("{name}: &[u8]")
            }
            ("void", 1) => {
                // An object of the pointer's kind: given back by value, bound as an option.
                let role_kind = entry.roles.iter().find_map(|role| match role {
                    Role::GivesBack(kinds) => Some((kind.unwrap_or(kinds[0]), true)),
                    Role::Binds(of) => Some((*of, false)),
                    _ => None,
                });
                match role_kind {
                    Some((k, true)) => format!("{name}: {}", kind_type(k, screen)),
                    Some((k, false)) => format!("{name}: Option<&{}>", kind_type(k, screen)),
                    None => return Err(format!("{}.{} {name}: void* with no kind", entry.structure, entry.pointer)),
                }
            }
            ("void", 2) | ("struct pipe_sampler_view" | "struct pipe_stream_output_target", 2) => {
                let k = entry.roles.iter().find_map(|role| if let Role::Binds(of) = role { Some(*of) } else { None }).ok_or("slots of no kind")?;
                if nullable(entry.pointer, &param.name) {
                    format!("{name}: Option<&[Option<&{}>]>", kind_type(k, screen))
                } else {
                    format!("{name}: &[Option<&{}>]", kind_type(k, screen))
                }
            }
            ("struct pipe_fence_handle", 2) => {
                ret = format!("Option<{}>", kind_type("Fence", screen));
                continue;
            }
            ("struct pipe_transfer", 2) => continue,
            ("struct pipe_transfer", 1) => {
                if entry.roles.iter().any(|role| matches!(role, Role::GivesBack(_))) {
                    format!("{name}: {}, written: &[u8]", kind_type("Mapping", screen))
                } else {
                    format!("{name}: &{}, written: &[u8]", kind_type("Mapping", screen))
                }
            }
            ("struct pipe_fence_handle", 1) => format!("{name}: &{}", kind_type("Fence", screen)),
            ("struct pipe_context", 1) => format!("{name}: Option<&{}>", kind_type("Context", screen)),
            ("struct pipe_query", 1) => {
                let gives = entry.roles.iter().any(|role| matches!(role, Role::GivesBack(_)));
                let turns = entry.roles.iter().find_map(|role| if let Role::Turns(from, _) = role { Some(*from) } else { None });
                match (turns, kind) {
                    (Some(from), None) => format!("{name}: {}", kind_type(from, screen)),
                    (Some(_), Some(k)) | (None, Some(k)) if gives => format!("{name}: {}", kind_type(k, screen)),
                    (Some(_), Some(k)) | (None, Some(k)) => format!("{name}: &{}", kind_type(k, screen)),
                    (None, None) => format!("{name}: &{}", kind_type("Query", screen)),
                }
            }
            ("struct pipe_resource", 1) if is_const => {
                structures.write("pipe_resource");
                format!("{name}: &{}", owned_in_method("pipe_resource", header, screen))
            }
            ("struct pipe_resource", 1) => {
                if entry.roles.iter().any(|role| matches!(role, Role::GivesBack(_))) {
                    format!("{name}: {}", kind_type("Resource", screen))
                } else {
                    format!("{name}: &{}", kind_type("Resource", screen))
                }
            }
            ("struct pipe_sampler_view", 1) if is_const => {
                structures.write("pipe_sampler_view");
                format!("{name}: &{}", owned_in_method("pipe_sampler_view", header, screen))
            }
            ("struct pipe_sampler_view", 1) => {
                if entry.roles.iter().any(|role| matches!(role, Role::GivesBack(_))) {
                    format!("{name}: {}", kind_type("SamplerView", screen))
                } else {
                    format!("{name}: &{}", kind_type("SamplerView", screen))
                }
            }
            ("struct pipe_stream_output_target", 1) => format!("{name}: {}", kind_type("StreamOutputTarget", screen)),
            ("struct pipe_shader_state" | "struct pipe_compute_state", 1) => {
                let shader = made_kind().ok_or("a program for no shader kind")?;
                let received = SHADERS.iter().find(|(kind, _)| *kind == shader).map(|(_, received)| *received).ok_or("no received program")?;
                let inner = base.trim_start_matches("struct ");
                structures.write(inner);
                let compiler = if screen { "Self::Compiler".to_string() } else { "<Self::Screen as Gallium>::Compiler".to_string() };
                format!("program: <{compiler} as wcontainers_mesa_compiler::MesaCompiler>::{received}, {name}: &{}", owned_in_method(inner, header, screen))
            }
            ("unsigned", 1) if is_const => format!("{name}: &[u32]"),
            ("float", 1) => {
                ret = "[f32; 2]".to_string();
                continue;
            }
            ("union pipe_query_result", 1) => format!("{name}: &mut [u8]"),
            ("union pipe_color_union", 1) => format!("{name}: &[u32; 4]"),
            ("struct pipe_box", 1) if !is_const => {
                structures.write("pipe_box");
                format!("{name}: Option<&PipeBox>")
            }
            (plain, 1) if plain.starts_with("struct pipe_") || plain.starts_with("union pipe_") => {
                let inner = plain.trim_start_matches("struct ").trim_start_matches("union ");
                structures.write(inner);
                let counted = (0..at).any(|before| function.params[before].pointers == 0 && is_count(function, before))
                    || (at + 1 < function.params.len() && counts_the_list_before(function, at + 1));
                let ty = owned_in_method(inner, header, screen);
                match (counted, nullable(entry.pointer, &param.name)) {
                    (true, true) => format!("{name}: Option<&[{ty}]>"),
                    (true, false) => format!("{name}: &[{ty}]"),
                    (false, true) => format!("{name}: Option<&{ty}>"),
                    (false, false) => format!("{name}: &{ty}"),
                }
            }
            (plain, 0) if plain.starts_with("struct pipe_") => {
                let inner = plain.trim_start_matches("struct ");
                structures.write(inner);
                format!("{name}: {}", owned_in_method(inner, header, screen))
            }
            _ => return Err(format!("{}.{} {name}: {base} with {} pointers: no rule", entry.structure, entry.pointer, param.pointers)),
        };
        params.push(param_text);
    }
    // The return.
    let rbase = function.ret.base.trim_start_matches("const ").trim();
    if ret.is_empty() {
        ret = match (rbase, function.ret.pointers) {
            ("void", 0) => String::new(),
            ("char", 1) => "Vec<u8>".to_string(),
            ("void", 1) if entry.pointer.ends_with("_map") => format!("Option<Mapped<{}>>", kind_type("Mapping", screen)),
            (_, 1) => format!("Option<{}>", kind_type(made_kind().ok_or_else(|| format!("{}: returns an object of no kind", entry.pointer))?, screen)),
            ("bool", 0) if entry.roles.iter().any(|role| matches!(role, Role::Turns(..))) => {
                let to = made_kind().unwrap_or("Query");
                format!("Option<{}>", kind_type(to, screen))
            }
            (b, 0) => number(b).ok_or_else(|| format!("{}: returns {b}", entry.pointer))?.to_string(),
            (b, n) => return Err(format!("{}: returns {b} with {n} pointers", entry.pointer)),
        };
    }
    Ok((params, ret))
}

pub fn generate(in_scope: &str, texts: &[String], out: &std::path::Path) -> Result<String, String> {
    crate::gallium::check(in_scope, &crate::cheader::read(texts))?;
    let header = crate::cheader::read(texts);
    let mut structures = Structures { header: &header, written: BTreeSet::new(), text: String::new(), unmapped: Vec::new(), defines: header.defines.clone() };
    let mut screen_methods = String::new();
    let mut context_methods = String::new();
    let mut failures = Vec::new();
    let mut methods = 0;
    for entry in PLAN {
        // A count is the kind's Clone and Drop, kept by the glue.
        if entry.roles.iter().all(|role| matches!(role, Role::Counts(_))) {
            continue;
        }
        let Some(function) = header.callbacks.get(entry.structure).and_then(|callbacks| callbacks.iter().find(|function| function.name == entry.pointer)) else {
            failures.push(format!("{}.{}: not in the headers", entry.structure, entry.pointer));
            continue;
        };
        for (at, kind) in split_kinds(entry).into_iter().enumerate() {
            match signature(entry, function, kind, &header, &mut structures) {
                Ok((params, ret)) => {
                    let name = method_name(entry, kind, at == 0);
                    // Clippy counts every parameter, the receiver among them, and each `a: A, b: B` of one param text.
                    let arguments = 1 + params.iter().map(|param| param.matches(": ").count()).sum::<usize>();
                    let receiver = std::iter::once("&self".to_string()).chain(params).collect::<Vec<_>>().join(", ");
                    let arrow = if ret.is_empty() { String::new() } else { format!(" -> {ret}") };
                    let target = if on_screen(entry) { &mut screen_methods } else { &mut context_methods };
                    let of_kind = kind.map(|kind| format!(", on a {kind}")).unwrap_or_default();
                    let _ = writeln!(target, "    /// Mesa's `{}.{}`{of_kind}, called first at {}.", entry.structure, entry.pointer, entry.caller);
                    if arguments > MOST_ARGUMENTS {
                        let _ = writeln!(target, "    #[expect(clippy::too_many_arguments, reason = \"the pointer's parameters, as Mesa's header declares them\")]");
                    }
                    let _ = writeln!(target, "    fn {name}({receiver}){arrow};\n");
                    methods += 1;
                }
                Err(error) => failures.push(error),
            }
        }
    }
    failures.extend(structures.unmapped.iter().cloned());
    if !failures.is_empty() {
        return Err(format!("GALLIUM INTERFACE FAILED {}:\n  {}", failures.len(), failures.join("\n  ")));
    }
    let mut interface = String::from(HEADER);
    interface.push_str("//! Gallium, the driver interface Mesa's OpenGL calls in our driver, as safe traits: `Gallium` (the screen) and\n//! `GalliumContext` (a rendering context), limited to the function pointers our driver sets. Every object Mesa\n//! passes is a value of one of the implementation's kinds, made only by the methods that make it and given back only\n//! by those that take it by value; a shader of one stage is a type of its own, made from mesa-compiler's received\n//! program of that stage. Plain data is owned, each object it names a reference of its kind. Declared values are\n//! `u32`, checked by the glue.\n\n");
    let _ = writeln!(interface, "{}", structures.text);
    interface.push_str("/// A resource's bytes mapped for the CPU: the implementation's mapping and the bytes, a copy the glue lends Mesa until the unmap.\n#[derive(Debug)]\npub struct Mapped<Mapping> {\n    /// The implementation's mapping, given back at the unmap.\n    pub mapping: Mapping,\n    /// The mapped bytes as the implementation has them.\n    pub bytes: Vec<u8>,\n    /// Bytes from one row to the next.\n    pub stride: u32,\n    /// Bytes from one layer to the next.\n    pub layer_stride: u64,\n}\n\n");
    let kinds: Vec<&str> = crate::gallium::KINDS.iter().map(|(kind, _)| *kind).filter(|kind| *kind != "Screen").collect();
    let _ = writeln!(interface, "/// The screen: the library Mesa's frontend loads, holding the kinds of its objects.\npub trait Gallium: Sized {{\n    /// The shader compiler the programs come from.\n    type Compiler: wcontainers_mesa_compiler::MesaCompiler;\n");
    for (kind, what) in crate::gallium::KINDS.iter().filter(|(kind, _)| *kind != "Screen") {
        let bound = if *kind == "Context" { ": GalliumContext<Screen = Self>" } else if *kind == "Fence" { ": Clone" } else { "" };
        let _ = writeln!(interface, "    /// {}{}.\n    type {kind}{bound};\n", what[..1].to_uppercase(), &what[1..]);
    }
    interface.push_str(&screen_methods);
    interface.push_str("}\n\n/// A rendering context of a screen.\npub trait GalliumContext: Sized {\n    /// The screen it was made on.\n    type Screen: Gallium<Context = Self>;\n\n");
    interface.push_str(&context_methods);
    interface.push_str("}\n");
    let crate_folder = out.join("crates").join("gallium");
    std::fs::create_dir_all(&crate_folder).map_err(|error| error.to_string())?;
    std::fs::write(crate_folder.join("interface.rs"), &interface).map_err(|error| error.to_string())?;
    std::fs::write(
        crate_folder.join("Cargo.toml"),
        "[package]\nname = \"wcontainers-gallium\"\ndescription = \"Gallium, the driver interface Mesa's OpenGL calls, as safe traits.\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\npublish = false\n\n[lib]\npath = \"interface.rs\"\n\n[dependencies]\nwcontainers-mesa-compiler = { path = \"../mesa-compiler\" }\n\n[lints]\nworkspace = true\n",
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("GALLIUM INTERFACE: {methods} methods, {} structures, {} kinds; left out {} fields\n", structures.written.len(), kinds.len() + 1, FIELDS_LEFT_OUT.len()))
}
