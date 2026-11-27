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
use crate::gallium::{COUNTED, Entry, PLAN, Role};

/// The first line of every file written.
const HEADER: &str = "// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers (p_defines.h, p_state.h, p_screen.h, p_context.h, u_transfer_helper.h; blend.h, shader_enums.h, u_formats.h and the build's u_format_gen.h for the values) and the plan in src/gallium.rs. Do not edit.\n";

/// The C handle of each kind's objects, as Mesa passes it inside plain data.
pub(crate) const HANDLE_KINDS: &[(&str, &str)] = &[
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
    ("pipe_resource", "__cl_space", "the padding of the EXCLUSIVE_CACHELINE macro around the count (util/u_memory.h:103): no value"),
    ("pipe_resource", "clientptr_heap", "the frontend's heap of client pointers, which the driver never reads (the glue's, zero)"),
    ("pipe_resource", "next", "a chain of planes the driver makes itself; resource_create's template has none (st_texture.c:106)"),
    ("pipe_sampler_view", "reference", "Mesa's count: the glue's"),
    ("pipe_sampler_view", "context", "the context the view is made on: the method's receiver"),
    ("pipe_sampler_view", "texture", "the resource viewed: create_sampler_view's own parameter"),
    ("pipe_stream_output_target", "reference", "Mesa's count: the glue's"),
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
];

/// Fields holding client memory that Mesa may pass whatever the driver reports, each carried as the bytes read during
/// the call (the glue copies them: their length from the call, as given), with the reason it cannot be refused.
const CLIENT_MEMORY: &[(&str, &str, &str)] = &[
    (
        "pipe_draw_info.index",
        "user",
        "the indices in client memory, when `has_user_indices`: from index 0 to the end of the furthest draw (start plus count, times index_size). No capability turns them off: st/mesa sets has_user_indices whenever no element buffer is bound (main/draw.c:1662, 2043; MEASURED on llvmpipe)",
    ),
    (
        "pipe_constant_buffer",
        "user_buffer",
        "the constants in client memory, `buffer_size` bytes from `buffer_offset`. st/mesa uploads its own when the driver reports prefer_real_buffer_in_constbuf0 (u_inlines.h:749, st_atom_constbuf.c:124), but Mesa's blitter, a helper the driver calls, passes its clear color so (u_blitter.c:1607, 2501, 2939)",
    ),
];

pub(crate) fn client_memory(structure: &str, field: &str) -> Option<&'static str> {
    CLIENT_MEMORY.iter().find(|(s, f, _)| *s == structure && *f == field).map(|(_, _, reason)| *reason)
}

/// The handle of an object of a kind as plain data and the methods name it: a counted kind's `Arc`, which the
/// implementation may clone to keep, any other by reference for the call.
fn handle(kind: &str, ty: &str) -> String {
    if COUNTED.contains(&kind) { format!("Arc<{ty}>") } else { ty.to_string() }
}

/// A field's declared type, its `const` dropped.
pub(crate) fn declared_type(base: &str) -> String {
    base.trim_start_matches("const ").trim().to_string()
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
    // A query of the pointer's first kind only: render_condition's method for a Query takes None, which ends the
    // condition; its method for an EndOnlyQuery always has one.
    ("render_condition", "query", "NULL ends the condition (main/condrender.c:99, st_cb_readpixels.c:148)"),
];

/// Parameters declared `unsigned` that hold an enumeration's values (READ at each in Mesa 26.3.1's headers): each
/// takes that enumeration's type in the trait.
const PARAM_VALUES: &[(&str, &str, &str, &str)] = &[
    ("create_query", "query_type", "pipe_query_type", "PIPE_QUERY_x (p_context.h:253, the values of enum pipe_query_type)"),
    ("buffer_map", "usage", "pipe_map_flags", "a combination of PIPE_MAP_x (p_context.h:920)"),
    ("buffer_subdata", "usage", "pipe_map_flags", "a combination of PIPE_MAP_x (p_context.h:979)"),
    ("texture_subdata", "usage", "pipe_map_flags", "a combination of PIPE_MAP_x (p_context.h:987)"),
    ("transfer_map", "usage", "pipe_map_flags", "a combination of PIPE_MAP_x (u_transfer_helper.h, as pipe_context's)"),
    ("flush", "flags", "pipe_flush_flags", "PIPE_FLUSH_x (p_context.h:840, the values of enum pipe_flush_flags)"),
];

/// The trait's type of a parameter `PARAM_VALUES` names.
fn param_value(header: &Header, pointer: &str, param: &str) -> Option<String> {
    let (_, _, enumeration, _) = PARAM_VALUES.iter().find(|(p, name, _, _)| *p == pointer && *name == param)?;
    header.value_type(enumeration).map(|(ty, _)| ty)
}

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
pub(crate) fn number(base: &str) -> Option<&'static str> {
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
pub(crate) fn type_name(c: &str) -> String {
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
pub(crate) fn field_name(c: &str) -> String {
    crate::names::escaped(c.to_string())
}

/// The kinds a structure's handles name, in the order met (its type parameters).
pub(crate) fn kinds_named(name: &str, header: &Header, out: &mut Vec<&'static str>, depth: usize) {
    let Some(fields) = header.structures.get(name) else { return };
    for field in fields {
        if left_out(name, &field.name).is_some() {
            continue;
        }
        let base = field.base.trim_start_matches("const ").trim();
        if field.pointers == 1
            && client_memory(name, &field.name).is_none()
            && let Some((_, kind)) = HANDLE_KINDS.iter().find(|(handle, _)| *handle == base)
        {
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

pub(crate) fn left_out(structure: &str, field: &str) -> Option<&'static str> {
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
        if let Some(known) = self.header.known_numbers.get(array) {
            return usize::try_from(*known).ok();
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
                if let Some((ty, _)) = self.header.field_value_type(structure, &field.name).or_else(|| self.header.value_type(base)) {
                    ty
                } else if let Some(ty) = number(base) {
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
            1 if client_memory(structure, &field.name).is_some() => "Option<&'a [u8]>".to_string(),
            1 => match HANDLE_KINDS.iter().find(|(handle, _)| *handle == base) {
                Some((_, kind)) => format!("Option<&'a {}>", handle(kind, kind)),
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
        let mut carried: Vec<(String, String)> = Vec::new();
        for field in &fields {
            if let Some(reason) = left_out(name, &field.name) {
                let _ = writeln!(body, "    // `{}` is not carried: {reason}.", field.name);
                continue;
            }
            let Some(ty) = self.field_type(name, field) else { continue };
            carried.push((field_name(&field.name), ty.clone()));
            let bits = field.bits.as_ref().map(|bits| format!(" ({bits} bits in C: a value past them is refused)")).unwrap_or_default();
            let memory = client_memory(name, &field.name).map(|reason| format!(": {reason}")).unwrap_or_default();
            let _ = writeln!(body, "    /// The `{}` of `{name}`{bits}{memory}.\n    pub {}: {ty},", field.name, field_name(&field.name));
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
        if let Some(filled) = crate::gallium::FILLED.iter().find(|filled| filled.structure == name) {
            self.write_defaults(filled.structure, filled.defaults, &carried);
        } else if HELD_IN_FILLED.contains(&name) {
            self.write_defaults(name, "zero (Mesa sets none)", &carried);
        }
    }

    /// `impl Default` of a structure the driver fills (G5): Mesa's defaults, from `inputs/gallium/caps-defaults.txt`
    /// for pipe_caps (u_init_pipe_screen_caps's values), zero for the others (Mesa sets none).
    fn write_defaults(&mut self, structure: &str, defaults: &str, carried: &[(String, String)]) {
        let table: BTreeMap<&str, &str> = if structure == "pipe_caps" {
            CAPS_DEFAULTS.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).filter_map(|line| line.split_once(" = ")).map(|(field, value)| (field.trim(), value.trim())).collect()
        } else {
            BTreeMap::new()
        };
        for field in table.keys() {
            if !carried.iter().any(|(name, _)| name == field) {
                self.unmapped.push(format!("{structure}: the default of {field}, which the structure does not carry"));
            }
        }
        let name = type_name(structure);
        let mut fields = String::new();
        for (field, ty) in carried {
            let value = table.get(field.as_str()).map_or_else(|| zero_of(ty), |value| (*value).to_string());
            let _ = writeln!(fields, "            {field}: {value},");
        }
        let _ = writeln!(self.text, "/// Mesa's defaults of `{structure}`: {defaults}.\nimpl Default for {name} {{\n    fn default() -> {name} {{\n        {name} {{\n{fields}        }}\n    }}\n}}\n");
    }
}

/// For the hostile C caller (caller.c) and its test (tests/served.rs): C assignments that set every field of Mesa's
/// structure `name` the trait carries, through `path` (`blend.`), to a value of its own (each value its counter's next,
/// within the field's width: a bit-field of one bit is 1), and the Debug text of the owned structure the trait must
/// receive for it. The fields left out are left zero by the caller's memset.
pub fn filled(header: &Header, name: &str, path: &str, counter: &mut u64) -> Result<(String, String), String> {
    let mut structures = Structures { header, written: BTreeSet::new(), text: String::new(), unmapped: Vec::new(), defines: header.defines.clone() };
    let mut assignments = String::new();
    let debug = structures.filled(name, path, counter, &mut assignments)?;
    Ok((assignments, debug))
}

impl Structures<'_> {
    fn filled(&mut self, name: &str, path: &str, counter: &mut u64, assignments: &mut String) -> Result<String, String> {
        let fields = self.header.structures.get(name).cloned().ok_or_else(|| format!("{name}: no structure read"))?;
        let mut parts = Vec::new();
        for field in &fields {
            if left_out(name, &field.name).is_some() {
                continue;
            }
            let ty = self.field_type(name, field).ok_or_else(|| format!("{name}.{}: no type", field.name))?;
            let lengths: Vec<usize> = match &field.array {
                Some(array) => array.split("][").map(|length| self.length(length.trim()).ok_or_else(|| format!("{name}.{}: array length {array}", field.name))).collect::<Result<_, _>>()?,
                None => Vec::new(),
            };
            let element = ty.trim_start_matches('[').split(';').next().unwrap_or_default().trim_start_matches('[').to_string();
            let base = declared_type(&field.base);
            let nested = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union ")).map(str::to_string);
            // A width may be a define's name (`clip_plane_enable:PIPE_MAX_CLIP_PLANES`).
            let width: Option<u32> = match &field.bits {
                Some(bits) => Some(self.length(bits.trim()).and_then(|width| u32::try_from(width).ok()).ok_or_else(|| format!("{name}.{}: width {bits}", field.name))?),
                None => None,
            };
            let debug = self.filled_value(name, &field.name, &format!("{path}{}", field.name), &lengths, &element, nested.as_deref(), width, counter, assignments)?;
            parts.push(format!("{}: {debug}", field_name(&field.name)));
        }
        Ok(format!("{} {{ {} }}", type_name(name), parts.join(", ")))
    }

    /// One field (an element of it where `lengths` remain): the C assignments and the Debug text.
    #[expect(clippy::too_many_arguments, reason = "the field, where it is, its shape and the counter")]
    fn filled_value(&mut self, structure: &str, field: &str, place: &str, lengths: &[usize], element: &str, nested: Option<&str>, width: Option<u32>, counter: &mut u64, assignments: &mut String) -> Result<String, String> {
        if let Some((first, rest)) = lengths.split_first() {
            let mut items = Vec::new();
            for at in 0..*first {
                items.push(self.filled_value(structure, field, &format!("{place}[{at}]"), rest, element, nested, width, counter, assignments)?);
            }
            return Ok(format!("[{}]", items.join(", ")));
        }
        if nested.is_some_and(|inner| inner == "pipe_color_union" || (self.header.unions.iter().any(|union| union == inner) && !inner.contains('.'))) {
            // A union of four words (pipe_color_union): its words.
            let mut words = Vec::new();
            for at in 0..4 {
                *counter += 1;
                let _ = writeln!(assignments, "    {place}.ui[{at}] = {}u;", 1000 + *counter);
                words.push((1000 + *counter).to_string());
            }
            return Ok(format!("[{}]", words.join(", ")));
        }
        if let Some(inner) = nested {
            return self.filled(inner, &format!("{place}."), counter, assignments);
        }
        *counter += 1;
        let value = match width {
            Some(bits) if bits < 32 => 1 + *counter % ((1_u64 << bits) - 1),
            _ => 1000 + *counter,
        };
        let numeric = element.trim();
        let (c, debug) = match numeric {
            "f32" | "f64" => (format!("{value}.25f"), format!("{value}.25")),
            // Flags alternate, so that both values cross the glue.
            "bool" if counter.is_multiple_of(2) => ("0".to_string(), "false".to_string()),
            "bool" => ("1".to_string(), "true".to_string()),
            "u8" | "i8" if width.is_none() => {
                let small = 1 + *counter % 100;
                (small.to_string(), small.to_string())
            }
            "u8" | "i8" => (value.to_string(), value.to_string()),
            values if values.starts_with("values::") => (value.to_string(), format!("{}({value})", values.trim_start_matches("values::"))),
            "u32" | "i32" | "u16" | "i16" | "u64" | "i64" => (value.to_string(), value.to_string()),
            other => return Err(format!("{structure}.{field}: no value for a field of {other}")),
        };
        let _ = writeln!(assignments, "    {place} = {c};");
        Ok(debug)
    }
}

/// The structures a filled structure holds by value, which take defaults too (pipe_caps holds pipe_mesh_caps).
pub(crate) const HELD_IN_FILLED: &[&str] = &["pipe_mesh_caps"];

/// Mesa's defaults of pipe_caps (u_init_pipe_screen_caps), one field a line.
const CAPS_DEFAULTS: &str = include_str!("../inputs/gallium/caps-defaults.txt");

/// Whether a Rust type is one of the numbers `number` gives.
fn is_number(ty: &str) -> bool {
    matches!(ty, "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64")
}

/// The zero of a field's Rust type.
fn zero_of(ty: &str) -> String {
    if let Some(inner) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']'))
        && let Some((element, length)) = inner.rsplit_once("; ")
    {
        return format!("[{}; {length}]", zero_of(element));
    }
    match ty {
        "bool" => "false".to_string(),
        "f32" | "f64" => "0.0".to_string(),
        values if values.starts_with("values::") => format!("{values}(0)"),
        number if is_number(number) => "0".to_string(),
        // A structure held by value: its own defaults (written with it).
        structure => format!("{structure}::default()"),
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
    // A turn that also reads another kind (end_query: an active query turned back, an end-only query only ended) is one
    // method for the turned kind and one for the read kind.
    let turned = entry.roles.iter().find_map(|role| if let Role::TurnsOrGivesBack(from, _, _) = role { Some(*from) } else { None });
    let read = entry.roles.iter().find_map(|role| if let Role::Reads([kind]) = role { Some(*kind) } else { None });
    if let (Some(turned), Some(read)) = (turned, read) {
        return vec![Some(turned), Some(read)];
    }
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

/// A kind's handle in a method: `Arc<..>` for a counted kind.
fn handle_type(kind: &str, screen: bool) -> String {
    handle(kind, &kind_type(kind, screen))
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
            Role::TurnsOrGivesBack(_, to, _) => Some(*to),
            _ => None,
        })
    };
    // The kind a turn takes by value, when this method is the turn's (not the read kind's of the same pointer).
    let turns = entry.roles.iter().find_map(|role| match role {
        Role::TurnsOrGivesBack(from, to, _) if kind.is_none_or(|k| k == *from) => Some((*from, *to)),
        _ => None,
    });
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
            (_, 0) if param_value(header, entry.pointer, &param.name).is_some() => format!("{name}: {}", param_value(header, entry.pointer, &param.name).unwrap_or_default()),
            (b, 0) if header.value_type(b).is_some() => format!("{name}: {}", header.value_type(b).map(|(ty, _)| ty).unwrap_or_default()),
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
                    format!("{name}: Option<&[Option<&{}>]>", handle_type(k, screen))
                } else {
                    format!("{name}: &[Option<&{}>]", handle_type(k, screen))
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
            ("struct pipe_fence_handle", 1) => format!("{name}: &{}", handle_type("Fence", screen)),
            ("struct pipe_context", 1) => format!("{name}: Option<&{}>", kind_type("Context", screen)),
            ("struct pipe_query", 1) => {
                let gives = entry.roles.iter().any(|role| matches!(role, Role::GivesBack(_)));
                match (turns, kind) {
                    (Some((from, _)), _) => format!("{name}: {}", kind_type(from, screen)),
                    (None, Some(k)) if gives => format!("{name}: {}", kind_type(k, screen)),
                    (None, Some(k)) if nullable(entry.pointer, &param.name) && split_kinds(entry).first() == Some(&Some(k)) => format!("{name}: Option<&{}>", kind_type(k, screen)),
                    (None, Some(k)) => format!("{name}: &{}", kind_type(k, screen)),
                    (None, None) => format!("{name}: &{}", kind_type("Query", screen)),
                }
            }
            ("struct pipe_resource", 1) if is_const => {
                structures.write("pipe_resource");
                format!("{name}: &{}", owned_in_method("pipe_resource", header, screen))
            }
            ("struct pipe_resource", 1) => {
                format!("{name}: &{}", handle_type("Resource", screen))
            }
            ("struct pipe_sampler_view", 1) if is_const => {
                structures.write("pipe_sampler_view");
                format!("{name}: &{}", owned_in_method("pipe_sampler_view", header, screen))
            }
            ("struct pipe_sampler_view", 1) => {
                format!("{name}: &{}", handle_type("SamplerView", screen))
            }
            ("struct pipe_stream_output_target", 1) => format!("{name}: &{}", handle_type("StreamOutputTarget", screen)),
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
            // A turn that fails hands back what it took: Mesa keeps its handle (each turn's reason in the plan).
            ("bool", 0) => match turns {
                Some((from, to)) => format!("Result<{}, {}>", kind_type(to, screen), kind_type(from, screen)),
                None => "bool".to_string(),
            },
            (b, 0) => match header.value_type(b) {
                Some((ty, _)) => ty,
                None => number(b).ok_or_else(|| format!("{}: returns {b}", entry.pointer))?.to_string(),
            },
            (b, n) => return Err(format!("{}: returns {b} with {n} pointers", entry.pointer)),
        };
    }
    Ok((params, ret))
}

pub fn generate(in_scope: &str, texts: &[String], out: &std::path::Path) -> Result<String, String> {
    crate::gallium::check(in_scope, &crate::cheader::read(texts))?;
    let mut header = crate::cheader::read(texts);
    let values = crate::gallium_values::collect(&header)?;
    header.value_types = values.types();
    header.known_numbers = values.known.clone();
    header.field_value_types = values.field_types(&header.field_families);
    let header = header;
    let mut structures = Structures { header: &header, written: BTreeSet::new(), text: String::new(), unmapped: Vec::new(), defines: header.defines.clone() };
    let mut screen_methods = String::new();
    let mut context_methods = String::new();
    let mut failures = Vec::new();
    let mut methods = 0;
    // Each method as written (on the screen's trait or the context's, name, parameters, return), for the recording
    // implementation.
    let mut written: Vec<(bool, String, Vec<String>, String)> = Vec::new();
    for entry in PLAN {
        // A count is the kind's Clone and Drop, kept by the glue.
        // A pointer Mesa's own function fills, or the same function as another, is the direct crate's, not a method.
        if entry.roles.iter().all(|role| matches!(role, Role::Counts(_) | Role::MesaFills(_) | Role::SameAs(_))) {
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
                    written.push((on_screen(entry), name.clone(), params.clone(), ret.clone()));
                    let receiver = std::iter::once("&self".to_string()).chain(params).collect::<Vec<_>>().join(", ");
                    let arrow = if ret.is_empty() { String::new() } else { format!(" -> {ret}") };
                    let target = if on_screen(entry) { &mut screen_methods } else { &mut context_methods };
                    let of_kind = kind.map(|kind| format!(", on a {kind}")).unwrap_or_default();
                    let _ = writeln!(target, "    /// Mesa's `{}.{}`{of_kind}, called first at {}.", entry.structure, entry.pointer, entry.caller);
                    if let Some(why) = entry.roles.iter().find_map(|role| match role {
                        Role::TurnsOrGivesBack(from, _, why) if kind.is_none_or(|k| k == *from) => Some(*why),
                        _ => None,
                    }) {
                        let _ = writeln!(target, "    ///\n    /// A failure hands back what it took: {why}.");
                    }
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
    // The fields the driver fills once (G5), each a method changing Mesa's defaults in place.
    for filled in crate::gallium::FILLED {
        structures.write(filled.structure);
        let name = type_name(filled.structure);
        let stage = if filled.by_stage { "stage: values::MesaShaderStage, " } else { "" };
        let of_stage = if filled.by_stage { ", one by shader stage (MESA_SHADER_*, up to MESA_SHADER_MESH_STAGES)" } else { "" };
        let _ = writeln!(
            screen_methods,
            "    /// Mesa's `pipe_screen.{}`{of_stage}, filled once when the screen is made and read from then on (first at {}): `caps`\n    /// holds {} for the implementation to change.\n    fn {}(&self, {stage}caps: &mut {name});\n",
            filled.field, filled.reader, filled.defaults, filled.field
        );
        let mut filled_params: Vec<String> = Vec::new();
        if filled.by_stage {
            filled_params.push("stage: values::MesaShaderStage".to_string());
        }
        filled_params.push(format!("caps: &mut {name}"));
        written.push((true, filled.field.to_string(), filled_params, String::new()));
        methods += 1;
    }
    failures.extend(structures.unmapped.iter().cloned());
    if !failures.is_empty() {
        return Err(format!("GALLIUM INTERFACE FAILED {}:\n  {}", failures.len(), failures.join("\n  ")));
    }
    let mut interface = String::from(HEADER);
    interface.push_str("//! Gallium, the driver interface Mesa's OpenGL calls in our driver, as safe traits: `Gallium` (the screen) and\n//! `GalliumContext` (a rendering context), limited to the function pointers our driver sets. Every object Mesa\n//! passes is a value of one of the implementation's kinds, made only by the methods that make it and given back only\n//! by those that take it by value; a shader of one stage is a type of its own, made from mesa-compiler's received\n//! program of that stage. Plain data is owned, each object it names a reference of its kind. A kind Mesa counts\n//! (Resource, Fence, SamplerView, StreamOutputTarget) is passed as an `Arc` of it: an implementation keeps one past\n//! the call by cloning the handle, and the object ends where its last handle is dropped. Declared values are\n//! `u32`, checked by the glue.\n\nuse std::sync::Arc;\n\npub mod values;\n\n");
    let _ = writeln!(interface, "{}", structures.text);
    interface.push_str("/// A resource's bytes mapped for the CPU: the implementation's mapping and the bytes, a copy the glue lends Mesa until the unmap.\n#[derive(Debug)]\npub struct Mapped<Mapping> {\n    /// The implementation's mapping, given back at the unmap.\n    pub mapping: Mapping,\n    /// The mapped bytes as the implementation has them.\n    pub bytes: Vec<u8>,\n    /// Bytes from one row to the next.\n    pub stride: u32,\n    /// Bytes from one layer to the next.\n    pub layer_stride: u64,\n}\n\n");
    let kinds: Vec<&str> = crate::gallium::KINDS.iter().map(|(kind, _)| *kind).filter(|kind| *kind != "Screen").collect();
    let _ = writeln!(interface, "/// The screen: the library Mesa's frontend loads, holding the kinds of its objects.\npub trait Gallium: Sized {{\n    /// The shader compiler the programs come from.\n    type Compiler: wcontainers_mesa_compiler::MesaCompiler;\n");
    for (kind, what) in crate::gallium::KINDS.iter().filter(|(kind, _)| *kind != "Screen") {
        let bound = if *kind == "Context" { ": GalliumContext<Screen = Self>" } else { "" };
        let _ = writeln!(interface, "    /// {}{}.\n    type {kind}{bound};\n", what[..1].to_uppercase(), &what[1..]);
    }
    interface.push_str(&screen_methods);
    interface.push_str("}\n\n/// A rendering context of a screen.\npub trait GalliumContext: Sized {\n    /// The screen it was made on.\n    type Screen: Gallium<Context = Self>;\n\n");
    interface.push_str(&context_methods);
    interface.push_str("}\n");
    let crate_folder = out.join("crates").join("gallium");
    std::fs::create_dir_all(&crate_folder).map_err(|error| error.to_string())?;
    let glue_report = crate::gallium_glue::generate(&header, &structures.written, &values, &crate_folder)?;
    let recording_report = crate::gallium_recording::generate(&written, &crate_folder)?;
    std::fs::write(crate_folder.join("interface.rs"), &interface).map_err(|error| error.to_string())?;
    std::fs::write(crate_folder.join("values.rs"), crate::gallium_values::rust(&values, HEADER)).map_err(|error| error.to_string())?;
    std::fs::write(
        crate_folder.join("Cargo.toml"),
        "[package]\nname = \"wcontainers-gallium\"\ndescription = \"Gallium, the driver interface Mesa's OpenGL calls, as safe traits.\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\npublish = false\n\n[lib]\npath = \"interface.rs\"\n\n[dependencies]\nwcontainers-mesa-compiler = { path = \"../mesa-compiler\" }\n\n[lints]\nworkspace = true\n",
    )
    .map_err(|error| error.to_string())?;
    let value_count: usize = values.enumerations.iter().map(|enumeration| enumeration.values.len()).sum::<usize>() + values.loose.len();
    Ok(format!(
        "GALLIUM INTERFACE: {methods} methods, {} structures, {} kinds; left out {} fields; values: {} enumerations, {value_count} values\n{glue_report}{recording_report}",
        structures.written.len(),
        kinds.len() + 1,
        FIELDS_LEFT_OUT.len(),
        values.enumerations.len()
    ))
}
