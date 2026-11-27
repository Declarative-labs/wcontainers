//! Gallium's serving glue (AC8), first layer: the plain data. Mesa hands a driver its structures with bit-fields and
//! unions, which Rust cannot read without unsafe code; so the direct crate's C (generated here, `flat.h` and the
//! flattening functions of `serve.c`) reads each structure a planned pointer takes into a flat copy of our own (every
//! bit-field a whole field, every union's members each a field, every object a key: the address Mesa knows it by), and
//! the Rust side (`flat.rs`) mirrors those flat copies, lists the keys and client memory each names, and turns a flat
//! copy into the trait's owned structure once the objects it names are held. The layouts of the flat copies are ours
//! (no bit-field, no union), each checked at run time against C's `sizeof`.
//!
//! Written with the trait layer by `api-crate-generator gallium` into `crates/gallium/implementation/direct/`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use crate::cheader::{CParam, Header};
use crate::gallium::COUNTED;
use crate::gallium_interface::{HANDLE_KINDS, client_memory, declared_type, field_name, kinds_named, left_out, number, type_name};

/// The first line of every file written.
const HEADER_RUST: &str = "// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.\n";
const HEADER_C: &str = "/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */\n";

/// How many bytes of client memory a field names, read by the glue during the call (`gallium_interface::CLIENT_MEMORY`
/// says why each is carried): an expression over the flat copy's fields, or the length the call gives.
const MEMORY_LENGTH: &[(&str, &str, MemoryLength)] = &[
    ("pipe_draw_info.index", "user", MemoryLength::IndexBytes),
    ("pipe_constant_buffer", "user_buffer", MemoryLength::Fields("buffer_offset", "buffer_size")),
];

#[derive(Clone, Copy)]
enum MemoryLength {
    /// The indices the draws of the call read, from index 0 (draw_vbo computes it from its draws).
    IndexBytes,
    /// `size` bytes from `offset`, both fields of the structure.
    Fields(&'static str, &'static str),
}

/// What one field of a flat copy holds.
#[derive(Clone)]
enum Kind {
    /// A number of the Rust type, a bit-field's width if it is one, and the trait's type of a declared value it holds.
    Number { rust: &'static str, bits: Option<u32>, value: Option<String> },
    /// A C `bool` (a byte, 0 or 1).
    Flag,
    /// An object of the kind, by its key.
    Object(&'static str),
    /// Client memory, by its address.
    Memory(MemoryLength),
    /// `union pipe_color_union`: its four words.
    Color,
    /// A structure held by value (or a union's members).
    Nested(String),
}

struct Field {
    c: String,
    rust: String,
    kind: Kind,
    /// Array lengths, outermost first.
    dims: Vec<usize>,
}

/// The flat name of a structure: `pipe_draw_info.index` to `wc_flat_pipe_draw_info__index`.
fn c_flat(name: &str) -> String {
    format!("wc_flat_{}", name.replace('.', "__"))
}

/// Its Rust mirror: `FlatDrawInfoIndex`.
fn rust_flat(name: &str) -> String {
    format!("Flat{}", type_name(name))
}

fn c_number(rust: &str) -> &'static str {
    match rust {
        "u8" => "uint8_t",
        "i8" => "int8_t",
        "u16" => "uint16_t",
        "i16" => "int16_t",
        "u32" => "uint32_t",
        "i32" => "int32_t",
        "u64" => "uint64_t",
        "i64" => "int64_t",
        "f32" => "float",
        "f64" => "double",
        _ => "uintptr_t",
    }
}

fn array_length(header: &Header, text: &str) -> Option<usize> {
    if let Ok(number) = text.parse() {
        return Some(number);
    }
    if let Some(known) = header.known_numbers.get(text) {
        return usize::try_from(*known).ok();
    }
    let mut value = header.defines.get(text)?.trim().trim_start_matches('(').trim_end_matches(')').to_string();
    for _ in 0..4 {
        if let Ok(number) = value.parse() {
            return Some(number);
        }
        value = header.defines.get(&value)?.clone();
    }
    None
}

/// The fields of a structure as its flat copy holds them (the fields the trait leaves out are left out here too).
fn fields(name: &str, header: &Header) -> Result<Vec<Field>, String> {
    let Some(declared) = header.structures.get(name) else { return Err(format!("{name}: no structure read")) };
    let mut out = Vec::new();
    for field in declared {
        if left_out(name, &field.name).is_some() {
            continue;
        }
        let kind = kind_of(name, field, header)?;
        let dims = match &field.array {
            Some(array) => array.split("][").map(|length| array_length(header, length.trim()).ok_or_else(|| format!("{name}.{}: array length {array}", field.name))).collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        out.push(Field { c: field.name.clone(), rust: field_name(&field.name), kind, dims });
    }
    Ok(out)
}

fn kind_of(structure: &str, field: &CParam, header: &Header) -> Result<Kind, String> {
    let base = declared_type(&field.base);
    let base = base.as_str();
    let bits = match &field.bits {
        Some(bits) => Some(array_length(header, bits.trim()).and_then(|bits| u32::try_from(bits).ok()).ok_or_else(|| format!("{structure}.{}: bits {bits}", field.name))?),
        None => None,
    };
    Ok(match field.pointers {
        0 => {
            if let Some((ty, rust)) = header.field_value_type(structure, &field.name).or_else(|| header.value_type(base)) {
                Kind::Number { rust, bits, value: Some(ty) }
            } else if let Some(rust) = number(base) {
                if rust == "bool" { Kind::Flag } else { Kind::Number { rust, bits, value: None } }
            } else if let Some(inner) = base.strip_prefix("struct ").or_else(|| base.strip_prefix("union ")) {
                if inner == "pipe_color_union" || (header.unions.iter().any(|union| union == inner) && !inner.contains('.')) {
                    Kind::Color
                } else {
                    Kind::Nested(inner.to_string())
                }
            } else {
                return Err(format!("{structure}.{}: {base}", field.name));
            }
        }
        1 if client_memory(structure, &field.name).is_some() => {
            let length = MEMORY_LENGTH.iter().find(|(s, f, _)| *s == structure && *f == field.name).map(|(_, _, length)| *length).ok_or_else(|| format!("{structure}.{}: client memory of no length", field.name))?;
            Kind::Memory(length)
        }
        1 => match HANDLE_KINDS.iter().find(|(handle, _)| *handle == base) {
            Some((_, kind)) => Kind::Object(kind),
            None => return Err(format!("{structure}.{}: {base}*", field.name)),
        },
        _ => return Err(format!("{structure}.{}: {base} with {} pointers", field.name, field.pointers)),
    })
}

/// The Rust type of a flat field.
fn rust_field_type(field: &Field) -> String {
    let mut ty = match &field.kind {
        Kind::Number { rust, .. } => (*rust).to_string(),
        Kind::Flag => "u8".to_string(),
        Kind::Object(_) | Kind::Memory(_) => "usize".to_string(),
        Kind::Color => "[u32; 4]".to_string(),
        Kind::Nested(inner) => rust_flat(inner),
    };
    for length in field.dims.iter().rev() {
        ty = format!("[{ty}; {length}]");
    }
    ty
}

/// The C declaration of a flat field.
fn c_field(field: &Field) -> String {
    let ty = match &field.kind {
        Kind::Number { rust, .. } => c_number(rust).to_string(),
        Kind::Flag => "uint8_t".to_string(),
        Kind::Object(_) | Kind::Memory(_) => "uintptr_t".to_string(),
        Kind::Color => "uint32_t".to_string(),
        Kind::Nested(inner) => format!("struct {}", c_flat(inner)),
    };
    let mut dims: String = field.dims.iter().map(|length| format!("[{length}]")).collect();
    if matches!(field.kind, Kind::Color) {
        dims.push_str("[4]");
    }
    format!("{ty} {}{dims};", field.c)
}

/// What the generator writes, gathered structure by structure (nested ones first).
struct Writer<'h> {
    header: &'h Header,
    done: BTreeSet<String>,
    c_types: String,
    rust: String,
    failures: Vec<String>,
}

impl Writer<'_> {
    fn structure(&mut self, name: &str) {
        if !self.done.insert(name.to_string()) {
            return;
        }
        let fields = match fields(name, self.header) {
            Ok(fields) => fields,
            Err(error) => {
                self.failures.push(error);
                return;
            }
        };
        for field in &fields {
            if let Kind::Nested(inner) = &field.kind {
                self.structure(inner);
            }
        }
        // C.
        let _ = writeln!(self.c_types, "/* Mesa's `{name}`, flat. */\nstruct {} {{", c_flat(name));
        if fields.is_empty() {
            let _ = writeln!(self.c_types, "    uint8_t empty;");
        }
        for field in &fields {
            let _ = writeln!(self.c_types, "    {}", c_field(field));
        }
        let _ = writeln!(self.c_types, "}};\n");
        // Rust: the mirror.
        let flat = rust_flat(name);
        let _ = writeln!(self.rust, "/// Mesa's `{name}` as the glue's C copies it (`struct {}` of flat.h).\n#[repr(C)]\n#[derive(Clone, Copy, Debug)]\npub struct {flat} {{", c_flat(name));
        if fields.is_empty() {
            let _ = writeln!(self.rust, "    pub empty: u8,");
        }
        for field in &fields {
            let _ = writeln!(self.rust, "    pub {}: {},", field.rust, rust_field_type(field));
        }
        let _ = writeln!(self.rust, "}}\n");
        // Rust: the keys and client memory it names.
        let mut keys = String::new();
        for field in fields.iter().filter(|field| names_keys(&field.kind, self.header, 0)) {
            keys_of(field, &format!("self.{}", field.rust), &fields, 0, &mut keys);
        }
        let keys_parameter = if keys.is_empty() { "_keys" } else { "keys" };
        let _ = writeln!(self.rust, "impl {flat} {{\n    /// Lists the objects and the client memory this copy names.\n    pub fn keys(&self, {keys_parameter}: &mut Keys) {{\n{keys}    }}\n");
        // Rust: the owned structure.
        let mut kinds = Vec::new();
        kinds_named(name, self.header, &mut kinds, 0);
        let owned = if kinds.is_empty() {
            format!("wcontainers_gallium::{}", type_name(name))
        } else {
            format!("wcontainers_gallium::{}<'a, {}>", type_name(name), kinds.iter().map(|kind| format!("G::{kind}")).collect::<Vec<_>>().join(", "))
        };
        let borrows = !kinds.is_empty() || fields.iter().any(|field| names_keys(&field.kind, self.header, 0));
        let held = if borrows || fields.iter().any(|field| matches!(field.kind, Kind::Nested(_))) { "held" } else { "_held" };
        let (lifetime, lent) = if borrows { ("'a, ", "&'a ") } else { ("", "&") };
        let mut body = String::new();
        for field in &fields {
            let value = owned_value(field, &format!("self.{}", field.rust), &fields, 0);
            let _ = writeln!(body, "            {}: {value},", field.rust);
        }
        let _ = writeln!(
            self.rust,
            "    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names\n    /// an object not held.\n    pub fn owned<{lifetime}G: Gallium>(&self, {held}: {lent}Held<G>) -> Option<{owned}> {{\n        Some(wcontainers_gallium::{} {{\n{body}        }})\n    }}\n}}\n",
            type_name(name)
        );
    }
}

/// Whether a field names any object or client memory (else its flat copy lists no keys).
fn names_keys(kind: &Kind, header: &Header, depth: usize) -> bool {
    match kind {
        Kind::Object(_) | Kind::Memory(_) => true,
        Kind::Nested(inner) => depth < 8 && fields(inner, header).is_ok_and(|fields| fields.iter().any(|field| names_keys(&field.kind, header, depth + 1))),
        _ => false,
    }
}

/// The statements listing one field's keys (`place` the field's Rust expression; inside a loop, a reference).
fn keys_of(field: &Field, place: &str, siblings: &[Field], depth: usize, out: &mut String) {
    let indent = "        ".to_string() + &"    ".repeat(depth);
    if depth < field.dims.len() {
        let element = format!("element{depth}");
        let from = if depth == 0 { format!("&{place}") } else { place.to_string() };
        let _ = writeln!(out, "{indent}for {element} in {from} {{");
        keys_of(field, &element, siblings, depth + 1, out);
        let _ = writeln!(out, "{indent}}}");
        return;
    }
    let value = if depth > 0 { format!("*{place}") } else { place.to_string() };
    match &field.kind {
        Kind::Object(kind) => {
            let _ = writeln!(out, "{indent}keys.object(ObjectKind::{kind}, {value});");
        }
        Kind::Memory(MemoryLength::IndexBytes) => {
            let _ = writeln!(out, "{indent}keys.memory({value}, 0, keys.index_bytes);");
        }
        Kind::Memory(MemoryLength::Fields(offset, size)) => {
            let sibling = |name: &str| siblings.iter().find(|field| field.c == name).map(|field| field.rust.clone()).unwrap_or_default();
            let _ = writeln!(out, "{indent}keys.memory({value}, usize::try_from(self.{}).unwrap_or(usize::MAX), usize::try_from(self.{}).unwrap_or(usize::MAX));", sibling(offset), sibling(size));
        }
        Kind::Nested(_) => {
            let _ = writeln!(out, "{indent}{place}.keys(keys);");
        }
        _ => {}
    }
}

/// The expression of one field's owned value (`place` its Rust expression; inside a closure, a reference).
fn owned_value(field: &Field, place: &str, siblings: &[Field], depth: usize) -> String {
    if depth < field.dims.len() {
        // A plain array of numbers without a width to check is copied whole.
        if depth == 0 && matches!(field.kind, Kind::Number { bits: None, value: None, .. } | Kind::Color) {
            return place.to_string();
        }
        let element = format!("element{depth}");
        let inner = owned_value(field, &element, siblings, depth + 1);
        let from = if depth == 0 { format!("&{place}") } else { place.to_string() };
        let made = match inner.strip_suffix('?') {
            Some(fallible) => fallible.to_string(),
            None => format!("Some({inner})"),
        };
        return format!("each({from}, |{element}| {made})?");
    }
    let value = if depth > 0 { format!("*{place}") } else { place.to_string() };
    match &field.kind {
        Kind::Number { rust, bits: Some(bits), value: declared } if !rust.starts_with('i') && *bits < 64 => match declared {
            Some(ty) => format!("wcontainers_gallium::{ty}(within({value}, {bits})?)"),
            None => format!("within({value}, {bits})?"),
        },
        Kind::Number { value: Some(ty), .. } => format!("wcontainers_gallium::{ty}({value})"),
        Kind::Number { .. } | Kind::Color => value,
        Kind::Flag => format!("flag({value})?"),
        Kind::Object(kind) => format!("held.{}({value})?", crate::names::snake_case(kind)),
        Kind::Memory(MemoryLength::IndexBytes) => format!("held.memory({value}, 0, held.index_bytes)?"),
        Kind::Memory(MemoryLength::Fields(offset, size)) => {
            let sibling = |name: &str| siblings.iter().find(|field| field.c == name).map(|field| field.rust.clone()).unwrap_or_default();
            format!("held.memory({value}, usize::try_from(self.{}).unwrap_or(usize::MAX), usize::try_from(self.{}).unwrap_or(usize::MAX))?", sibling(offset), sibling(size))
        }
        Kind::Nested(_) => format!("{place}.owned(held)?"),
    }
}

/// The C statements copying one field into its flat copy (`from` and `to` its C expressions, `indent` how deep the
/// loops around it go, which also names their indices).
fn c_copy(field: &Field, from: &str, to: &str, header: &Header, indent: usize, out: &mut String) -> Result<(), String> {
    c_copy_at(field, from, to, header, 0, indent, out)
}

fn c_copy_at(field: &Field, from: &str, to: &str, header: &Header, dim: usize, indent: usize, out: &mut String) -> Result<(), String> {
    let pad = "    ".repeat(indent + 1);
    if dim < field.dims.len() {
        let index = format!("i{indent}");
        let _ = writeln!(out, "{pad}for (unsigned {index} = 0; {index} < {}; {index}++) {{", field.dims[dim]);
        c_copy_at(field, &format!("{from}[{index}]"), &format!("{to}[{index}]"), header, dim + 1, indent + 1, out)?;
        let _ = writeln!(out, "{pad}}}");
        return Ok(());
    }
    match &field.kind {
        Kind::Number { .. } | Kind::Flag => {
            let _ = writeln!(out, "{pad}{to} = {from};");
        }
        Kind::Object(_) | Kind::Memory(_) => {
            let _ = writeln!(out, "{pad}{to} = (uintptr_t){from};");
        }
        Kind::Color => {
            let _ = writeln!(out, "{pad}memcpy({to}, &{from}, sizeof {to});");
        }
        Kind::Nested(inner) => {
            for nested in fields(inner, header)? {
                c_copy(&nested, &format!("{from}.{}", nested.c), &format!("{to}.{}", nested.c), header, indent, out)?;
            }
        }
    }
    Ok(())
}

/// Writes flat.h, flat.c (the flattening functions) and flat.rs for the structures the trait layer wrote.
pub fn generate(header: &Header, written: &BTreeSet<String>, values: &crate::gallium_values::Values, crate_folder: &Path) -> Result<String, String> {
    let mut writer = Writer { header, done: BTreeSet::new(), c_types: String::new(), rust: String::new(), failures: Vec::new() };
    for name in written {
        writer.structure(name);
    }
    // The flattening functions, one per structure a pointer takes by pointer (the named ones: an inner structure or a
    // union is copied inside the function of the structure holding it).
    let mut functions = String::new();
    let mut sizes = String::new();
    let mut size_names = Vec::new();
    for name in &writer.done {
        let _ = writeln!(sizes, "        case {}: return sizeof(struct {});", size_names.len(), c_flat(name));
        size_names.push(name.clone());
        if name.contains('.') {
            continue;
        }
        let mut body = String::new();
        let fields = fields(name, header)?;
        for field in &fields {
            c_copy(field, &format!("in->{}", field.c), &format!("out->{}", field.c), header, 0, &mut body)?;
        }
        let _ = writeln!(functions, "void {}_from(const struct {name} *in, struct {} *out)\n{{\n    memset(out, 0, sizeof *out);\n{body}}}\n", c_flat(name), c_flat(name));
    }
    let (layout_cases, field_rows, left_rows, field_count) = field_layouts(header, &writer.done)?;
    if !writer.failures.is_empty() {
        return Err(format!("GALLIUM GLUE FAILED {}:\n  {}", writer.failures.len(), writer.failures.join("\n  ")));
    }
    let direct = crate_folder.join("implementation").join("direct");
    std::fs::create_dir_all(&direct).map_err(|error| error.to_string())?;
    let write = |path: std::path::PathBuf, text: &str| std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()));
    let mut prototypes = String::new();
    for name in writer.done.iter().filter(|name| !name.contains('.')) {
        let _ = writeln!(prototypes, "void {}_from(const struct {name} *in, struct {} *out);", c_flat(name), c_flat(name));
    }
    write(
        direct.join("flat.h"),
        &format!("{HEADER_C}/* Mesa's structures as the glue copies them: no bit-field, no union, every object its address. */\n#ifndef WC_GALLIUM_FLAT_H\n#define WC_GALLIUM_FLAT_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n{}\n#endif\n", writer.c_types),
    )?;
    write(
        direct.join("flat.c"),
        &format!("{HEADER_C}/* Each structure Mesa passes, copied into its flat form; and each flat form's size, which the Rust side checks. */\n#include <string.h>\n\n#include \"pipe/p_context.h\"\n#include \"pipe/p_defines.h\"\n#include \"pipe/p_screen.h\"\n#include \"pipe/p_state.h\"\n#include \"util/u_transfer_helper.h\"\n#include \"util/blend.h\"\n#include \"compiler/shader_enums.h\"\n\n#include \"flat.h\"\n\n{prototypes}\nsize_t wc_flat_size(unsigned which);\nsize_t wc_flat_layout(unsigned which, unsigned what);\nint64_t wc_value(unsigned which);\n\n{functions}size_t wc_flat_size(unsigned which)\n{{\n    switch (which) {{\n{sizes}        default: return 0;\n    }}\n}}\n\n/* Field `which` of FIELDS in flat.rs: `what` 0 its offset in the flat form, 1 its size there, 2 its offset in Mesa's\n * structure (from the structure holding it, for a nested one), 3 its size there; a bit-field's Mesa offset and size are\n * WC_BIT_FIELD (no offsetof); a field not listed, WC_NO_FIELD. */\n#define WC_BIT_FIELD ((size_t)-1)\n#define WC_NO_FIELD ((size_t)-2)\nsize_t wc_flat_layout(unsigned which, unsigned what)\n{{\n    switch (which * 4 + what) {{\n{layout_cases}        default: return WC_NO_FIELD;\n    }}\n}}\n\n/* Value `which` of values() in flat.rs, as C computes it from Mesa's headers. */\nint64_t wc_value(unsigned which)\n{{\n    switch (which) {{\n{}        default: return INT64_MIN;\n    }}\n}}\n", crate::gallium_values::c_cases(values)),
    )?;
    let mut size_checks = String::new();
    for (at, name) in size_names.iter().enumerate() {
        let _ = writeln!(size_checks, "    ({at}, core::mem::size_of::<{}>()),", rust_flat(name));
    }
    write(
        direct.join("flat.rs"),
        &format!("{HEADER_RUST}//! Mesa's structures as the glue's C copies them (flat.h): mirrors with C's layout (no bit-field, no union, each\n//! object its key), the keys and client memory each names, and the trait's owned structure made from each.\n\nuse wcontainers_gallium::Gallium;\n\nuse crate::FieldLayout;\nuse crate::held::{{Held, Keys, ObjectKind, each, flag, within}};\n\n{}/// Each flat form's index in flat.c's `wc_flat_size` and its size in Rust.\npub const SIZES: &[(u32, usize)] = &[\n{size_checks}];\n\n/// Every field of every flat form, in flat.c's `wc_flat_layout` order: its Mesa structure, its name, its offset and size\n/// in the Rust mirror, its width if Mesa declares it a bit-field, and whether its size in Mesa is its size here (a\n/// number, a flag, an object, client memory or a color; not a structure, flattened, nor a bit-field), or at least its\n/// size there (an enumeration, which Mesa may pack).\npub const FIELDS: &[FieldLayout] = &[\n{field_rows}];\n\n/// The fields of Mesa's structures the flat forms do not carry (`FIELDS_LEFT_OUT` of the generator, each with its reason\n/// in interface.rs): with FIELDS, every field of each structure.\npub const LEFT_OUT: &[(&str, &str)] = &[\n{left_rows}];\n\n/// Every declared value of the trait (values.rs) with its C name, in flat.c's `wc_value` order.\npub fn values() -> Vec<(&'static str, i64)> {{\n    vec![\n{}    ]\n}}\n", writer.rust, crate::gallium_values::rust_list(values)),
    )?;
    write(direct.join("implementation.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/implementation.rs")))?;
    write(direct.join("held.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/held.rs")))?;
    write(direct.join("build.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/build.rs")))?;
    write(direct.join("Cargo.toml"), include_str!("../templates/gallium/direct.toml"))?;
    std::fs::create_dir_all(direct.join("tests")).map_err(|error| error.to_string())?;
    write(direct.join("tests").join("layouts.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/layouts.rs")))?;
    let counted: Vec<&str> = COUNTED.to_vec();
    Ok(format!("GALLIUM GLUE: {} flat structures ({} with a flattening function), {field_count} fields; counted kinds {}\n", writer.done.len(), writer.done.iter().filter(|name| !name.contains('.')).count(), counted.join(" ")))
}

/// The C type of a Mesa structure by the generator's name (`pipe_draw_info.index` is reached from `struct pipe_draw_info`).
fn mesa_top(header: &Header, name: &str) -> (String, String) {
    let (top, path) = name.split_once('.').map_or((name, ""), |(top, path)| (top, path));
    let keyword = if header.unions.iter().any(|union| union == top) { "union" } else { "struct" };
    (format!("{keyword} {top}"), path.to_string())
}

/// For every field of every flat form: flat.c's `wc_flat_layout` cases (offset and size in the flat form and in Mesa's
/// structure, by C's own offsetof and sizeof), flat.rs's FIELDS rows (the same in Rust by offset_of and size_of) and
/// LEFT_OUT rows, and the count of fields.
fn field_layouts(header: &Header, done: &BTreeSet<String>) -> Result<(String, String, String, usize), String> {
    let (mut cases, mut rows, mut left) = (String::new(), String::new(), String::new());
    let mut which = 0_usize;
    for name in done {
        let (mesa, path) = mesa_top(header, name);
        let flat = c_flat(name);
        let declared = header.structures.get(name).ok_or_else(|| format!("{name}: no structure read"))?;
        for field in declared.iter().filter(|field| left_out(name, &field.name).is_some()) {
            let _ = writeln!(left, "    (\"{name}\", \"{}\"),", field.name);
        }
        for field in fields(name, header)? {
            let bits = declared.iter().find(|declared| declared.name == field.c).and_then(|declared| declared.bits.as_ref()).and_then(|bits| array_length(header, bits.trim()));
            let member = if path.is_empty() { field.c.clone() } else { format!("{path}.{}", field.c) };
            let mesa_offset = if path.is_empty() { format!("offsetof({mesa}, {member})") } else { format!("offsetof({mesa}, {member}) - offsetof({mesa}, {path})") };
            let at = which * 4;
            let _ = writeln!(cases, "        case {}: return offsetof(struct {flat}, {});", at, field.c);
            let _ = writeln!(cases, "        case {}: return sizeof(((struct {flat} *)0)->{});", at + 1, field.c);
            if bits.is_some() {
                let _ = writeln!(cases, "        case {}: case {}: return WC_BIT_FIELD;", at + 2, at + 3);
            } else {
                let _ = writeln!(cases, "        case {}: return {mesa_offset};", at + 2);
                let _ = writeln!(cases, "        case {}: return sizeof((({mesa} *)0)->{member});", at + 3);
            }
            let same_size = bits.is_none() && !matches!(field.kind, Kind::Nested(_));
            // An enumeration's size is the compiler's (Mesa packs some: `enum mesa_prim` is one byte under GCC and
            // Clang, p_state.h:864); the flat form holds it in 32 bits, so its size there is at least Mesa's.
            let declared_base = declared.iter().find(|declared| declared.name == field.c).map(|declared| declared_type(&declared.base)).unwrap_or_default();
            let at_least = declared_base.starts_with("enum ") || declared_base == "mesa_shader_stage" || declared_base == "gl_shader_stage";
            let _ = writeln!(
                rows,
                "    FieldLayout {{ structure: \"{name}\", field: \"{}\", flat_offset: core::mem::offset_of!({}, {}), flat_size: core::mem::size_of::<{}>(), bits: {}, same_size: {same_size}, at_least: {at_least} }},",
                field.c,
                rust_flat(name),
                field.rust,
                rust_field_type(&field),
                bits.unwrap_or(0)
            );
            which += 1;
        }
    }
    Ok((cases, rows, left, which))
}
