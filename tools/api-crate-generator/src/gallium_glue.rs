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
const MEMORY_LENGTH: &[(&str, MemoryLength)] = &[
    ("pipe_draw_info.index.user", MemoryLength::IndexBytes),
    ("pipe_constant_buffer.user_buffer", MemoryLength::Fields("buffer_offset", "buffer_size")),
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
pub(crate) fn c_flat(name: &str) -> String {
    format!("wc_flat_{}", name.replace('.', "__"))
}

/// Its Rust mirror: `FlatDrawInfoIndex`.
pub(crate) fn rust_flat(name: &str) -> String {
    format!("Flat{}", type_name(name))
}

fn c_number(rust: &str) -> &'static str {
    match rust {
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "i16" => "int16_t",
        "u32" => "uint32_t",
        "i32" => "int32_t",
        "u64" => "uint64_t",
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
                if inner == "pipe_color_union" {
                    Kind::Color
                } else {
                    Kind::Nested(inner.to_string())
                }
            } else {
                return Err(format!("{structure}.{}: {base}", field.name));
            }
        }
        1 if client_memory(structure, &field.name).is_some() => {
            let place = format!("{structure}.{}", field.name);
            let length = MEMORY_LENGTH.iter().find(|(at, _)| *at == place).map(|(_, length)| *length).ok_or_else(|| format!("{place}: client memory of no length"))?;
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
        for field in fields.iter().filter(|field| names_keys(&field.kind, self.header, &[])) {
            keys_of(field, &format!("self.{}", field.rust), &fields, 0, &mut keys);
        }
        let keys_parameter = if keys.is_empty() { "_keys" } else { "keys" };
        let _ = writeln!(self.rust, "impl {flat} {{\n    /// Lists the objects and the client memory this copy names.\n    pub fn keys(&self, {keys_parameter}: &mut Keys) {{\n{keys}    }}\n");
        // Rust: the owned structure.
        let mut kinds = Vec::new();
        kinds_named(name, self.header, &mut kinds);
        let owned = if kinds.is_empty() {
            format!("wcontainers_gallium::{}", type_name(name))
        } else {
            format!("wcontainers_gallium::{}<'a, {}>", type_name(name), kinds.iter().map(|kind| format!("G::{kind}")).collect::<Vec<_>>().join(", "))
        };
        // The owned structure borrows exactly when it names objects (client memory sits beside an object in each).
        let borrows = !kinds.is_empty();
        let held = if borrows || fields.iter().any(|field| matches!(field.kind, Kind::Nested(_))) { "held" } else { "_held" };
        let (lifetime, lent) = if borrows { ("'a, ", "&'a ") } else { ("", "&") };
        let mut body = String::new();
        for field in &fields {
            let value = owned_value(field, &format!("self.{}", field.rust), &fields, 0);
            let _ = writeln!(body, "            {}: {value},", field.rust);
        }
        // A structure the driver fills (G5) also goes back: the flat copy of the trait's owned structure, which serve.c
        // writes into Mesa's screen.
        let back = if filled_or_held(name) {
            let mut fields_back = String::new();
            for field in &fields {
                let _ = writeln!(fields_back, "            {}: {},", field.rust, flat_value(field, &format!("owned.{}", field.rust), 0));
            }
            format!("\n    /// The flat copy of the trait's owned structure (what serve.c writes into Mesa's screen).\n    pub fn of(owned: &{owned}) -> {flat} {{\n        {flat} {{\n{fields_back}        }}\n    }}\n")
        } else {
            String::new()
        };
        let _ = writeln!(
            self.rust,
            "    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names\n    /// an object not held.\n    pub fn owned<{lifetime}G: Gallium>(&self, {held}: {lent}Held<G>) -> Option<{owned}> {{\n        Some(wcontainers_gallium::{} {{\n{body}        }})\n    }}\n{back}}}\n",
            type_name(name)
        );
    }
}

/// Whether a structure is one the driver fills (G5), or held by value inside one: it goes back to Mesa too.
fn filled_or_held(name: &str) -> bool {
    crate::gallium::FILLED.iter().any(|filled| filled.structure == name) || crate::gallium_interface::HELD_IN_FILLED.contains(&name)
}

/// The flat value of one field of the trait's owned structure (`place` its Rust expression; inside a closure, the
/// element).
fn flat_value(field: &Field, place: &str, depth: usize) -> String {
    if depth < field.dims.len() {
        if matches!(field.kind, Kind::Number { value: None, .. } | Kind::Color) {
            return place.to_string();
        }
        let element = format!("element{depth}");
        return format!("{place}.map(|{element}| {})", flat_value(field, &element, depth + 1));
    }
    match &field.kind {
        Kind::Number { value: Some(_), .. } => format!("{place}.0"),
        Kind::Flag => format!("u8::from({place})"),
        Kind::Nested(inner) => format!("{}::of(&{place})", rust_flat(inner)),
        _ => place.to_string(),
    }
}

/// Whether a field names any object or client memory (else its flat copy lists no keys); `inside`: the structures
/// around it, each entered once, so that a header whose structure holds itself ends.
fn names_keys(kind: &Kind, header: &Header, inside: &[&str]) -> bool {
    match kind {
        Kind::Object(_) | Kind::Memory(_) => true,
        Kind::Nested(inner) => {
            !inside.contains(&inner.as_str()) && fields(inner, header).is_ok_and(|fields| fields.iter().any(|field| names_keys(&field.kind, header, &[inside, &[inner.as_str()]].concat())))
        }
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
        let fields = fields(name, header)?;
        // A structure the driver fills only goes to Mesa (its `_to` below): no copy from Mesa.
        if !filled_or_held(name) {
            let mut body = String::new();
            for field in &fields {
                c_copy(field, &format!("in->{}", field.c), &format!("out->{}", field.c), header, 0, &mut body)?;
            }
            let _ = writeln!(functions, "void {}_from(const struct {name} *in, struct {} *out)\n{{\n    memset(out, 0, sizeof *out);\n{body}}}\n", c_flat(name), c_flat(name));
        }
        if crate::gallium::FILLED.iter().any(|filled| filled.structure == name) {
            let mut back = String::new();
            for field in &fields {
                c_copy(field, &format!("in->{}", field.c), &format!("out->{}", field.c), header, 0, &mut back)?;
            }
            let _ = writeln!(functions, "void {}_to(const struct {} *in, struct {name} *out)\n{{\n{back}}}\n", c_flat(name), c_flat(name));
        }
    }
    let (layout_cases, field_rows, left_rows, field_count) = field_layouts(header, &writer.done)?;
    if !writer.failures.is_empty() {
        return Err(format!("GALLIUM GLUE FAILED {}:\n  {}", writer.failures.len(), writer.failures.join("\n  ")));
    }
    let direct = crate_folder.join("implementation").join("direct");
    std::fs::create_dir_all(&direct).map_err(|error| error.to_string())?;
    let write = |path: std::path::PathBuf, text: &str| std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()));
    let mut prototypes = String::new();
    for name in writer.done.iter().filter(|name| !name.contains('.') && !filled_or_held(name)) {
        let _ = writeln!(prototypes, "void {}_from(const struct {name} *in, struct {} *out);", c_flat(name), c_flat(name));
    }
    write(
        direct.join("flat.h"),
        &format!("{HEADER_C}/* Mesa's structures as the glue copies them: no bit-field, no union, every object its address. */\n#ifndef WC_GALLIUM_FLAT_H\n#define WC_GALLIUM_FLAT_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n{}\n/* Each structure Mesa passes, flattened (flat.c). */\n{}\n#endif\n", writer.c_types, writer.done.iter().filter(|name| !name.contains('.')).map(|name| { let back = if crate::gallium::FILLED.iter().any(|filled| filled.structure == *name) { format!("void {}_to(const struct {} *in, struct {name} *out);\n", c_flat(name), c_flat(name)) } else { String::new() }; let from = if filled_or_held(name) { String::new() } else { format!("void {}_from(const struct {name} *in, struct {} *out);\n", c_flat(name), c_flat(name)) }; format!("struct {name};\n{from}{back}") }).collect::<String>()),
    )?;
    let flat_c = format!("{HEADER_C}/* Each structure Mesa passes, copied into its flat form; and each flat form's size, which the Rust side checks. */\n#include <string.h>\n\n#include \"pipe/p_context.h\"\n#include \"pipe/p_defines.h\"\n#include \"pipe/p_screen.h\"\n#include \"pipe/p_state.h\"\n#include \"util/u_transfer_helper.h\"\n#include \"util/blend.h\"\n#include \"compiler/shader_enums.h\"\n\n#include \"flat.h\"\n\n{prototypes}\nsize_t wc_flat_size(unsigned which);\nsize_t wc_flat_layout(unsigned which, unsigned what);\nint64_t wc_value(unsigned which);\n\n{functions}size_t wc_flat_size(unsigned which)\n{{\n    switch (which) {{\n{sizes}        default: return 0;\n    }}\n}}\n\n/* Field `which` of FIELDS in flat.rs: `what` 0 its offset in the flat form, 1 its size there, 2 its offset in Mesa's\n * structure (from the structure holding it, for a nested one), 3 its size there; a bit-field's Mesa offset and size are\n * WC_BIT_FIELD (no offsetof); a field not listed, WC_NO_FIELD. */\n#define WC_BIT_FIELD ((size_t)-1)\n#define WC_NO_FIELD ((size_t)-2)\nsize_t wc_flat_layout(unsigned which, unsigned what)\n{{\n    switch (which * 4 + what) {{\n{layout_cases}        default: return WC_NO_FIELD;\n    }}\n}}\n\n/* Value `which` of values() in flat.rs, as C computes it from Mesa's headers. */\nint64_t wc_value(unsigned which)\n{{\n    switch (which) {{\n{}        default: return INT64_MIN;\n    }}\n}}\n", crate::gallium_values::c_cases(values));
    write(direct.join("flat.c"), &flat_c)?;
    let mut size_checks = String::new();
    for (at, name) in size_names.iter().enumerate() {
        let _ = writeln!(size_checks, "    ({at}, core::mem::size_of::<{}>()),", rust_flat(name));
    }
    let flat_rs = format!("{HEADER_RUST}//! Mesa's structures as the glue's C copies them (flat.h): mirrors with C's layout (no bit-field, no union, each\n//! object its key), the keys and client memory each names, and the trait's owned structure made from each.\n\nuse wcontainers_gallium::Gallium;\n\nuse crate::FieldLayout;\nuse crate::held::{{Held, Keys, ObjectKind, each, flag, within}};\n\n{}/// Each flat form's index in flat.c's `wc_flat_size` and its size in Rust.\npub const SIZES: &[(u32, usize)] = &[\n{size_checks}];\n\n/// Every field of every flat form, in flat.c's `wc_flat_layout` order: its Mesa structure, its name, its offset and size\n/// in the Rust mirror, its width if Mesa declares it a bit-field, and whether its size in Mesa is its size here (a\n/// number, a flag, an object, client memory or a color; not a structure, flattened, nor a bit-field), or at least its\n/// size there (an enumeration, which Mesa may pack).\npub const FIELDS: &[FieldLayout] = &[\n{field_rows}];\n\n/// The fields of Mesa's structures the flat forms do not carry (`FIELDS_LEFT_OUT` of the generator, each with its reason\n/// in interface.rs): with FIELDS, every field of each structure.\npub const LEFT_OUT: &[(&str, &str)] = &[\n{left_rows}];\n\n/// Every declared value of the trait (values.rs) with its C name, in flat.c's `wc_value` order.\npub fn values() -> Vec<(&'static str, i64)> {{\n    vec![\n{}    ]\n}}\n", writer.rust, crate::gallium_values::rust_list(values));
    write(direct.join("flat.rs"), &flat_rs)?;
    write(direct.join("implementation.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/implementation.rs")))?;
    std::fs::create_dir_all(direct.join("tests")).map_err(|error| error.to_string())?;
    write(direct.join("held.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/held.rs")))?;
    write(direct.join("registry.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/registry.rs")))?;
    let (serve_rust, serve_c, served) = crate::gallium_serve::generate(header)?;
    write(direct.join("serve.rs"), &serve_rust)?;
    write(direct.join("serve.c"), &serve_c)?;
    let (fills_c, fills_rust) = caller_fills(header)?;
    let (caps_c, caps_checks, _) = caps_filled(header)?;
    write(direct.join("caller.c"), &format!("{HEADER_C}{}", include_str!("../templates/gallium/caller.c").replace("/* FILLED */\n", &fills_c).replace("/* CAPS FILLED */\n", &caps_c)))?;
    write(direct.join("tests").join("served.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/served.rs").replace("// FILLED\n", &fills_rust).replace("// CAPS CHECKS\n", &caps_checks)))?;
    write(direct.join("caps-plants.txt"), &caps_plants(&flat_c, &flat_rs, &serve_c, &serve_rust)?)?;
    write(direct.join("build.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/build.rs")))?;
    write(direct.join("Cargo.toml"), include_str!("../templates/gallium/direct.toml"))?;
    write(direct.join("tests").join("layouts.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/layouts.rs")))?;
    std::fs::create_dir_all(direct.join("tests").join("clang_layouts")).map_err(|error| error.to_string())?;
    write(direct.join("tests").join("clang_layouts").join("mod.rs"), &format!("{HEADER_RUST}{}", include_str!("../templates/gallium/clang_layouts.rs")))?;
    let counted: Vec<&str> = COUNTED.to_vec();
    Ok(format!("GALLIUM GLUE: {} flat structures ({} with a flattening function), {field_count} fields; counted kinds {}\n{served}", writer.done.len(), writer.done.iter().filter(|name| !name.contains('.')).count(), counted.join(" ")))
}

/// The served states the hostile caller's first case makes, each filled field by field: (the C function, its
/// parameter, the structure, how many it fills, the method and argument the trait receives them as).
const FILLED_STATES: &[(&str, &str, &str, usize, &str)] = &[
    ("filled_blend", "blend", "pipe_blend_state", 1, "create_blend_state arg1"),
    ("filled_depth_stencil_alpha", "dsa", "pipe_depth_stencil_alpha_state", 1, "create_depth_stencil_alpha_state arg1"),
    ("filled_rasterizer", "rasterizer", "pipe_rasterizer_state", 1, "create_rasterizer_state arg1"),
    ("filled_sampler_first", "sampler", "pipe_sampler_state", 1, "create_sampler_state arg1"),
    ("filled_sampler_second", "sampler", "pipe_sampler_state", 1, "create_sampler_state arg1"),
    ("filled_vertex_elements", "elements", "pipe_vertex_element", 2, "create_vertex_elements_state arg2"),
];

/// The hostile caller's filling functions (caller.c) and the calls the implementation must receive for them
/// (tests/served.rs's `FILLED`): every field the trait carries a value of its own, stated in full.
fn caller_fills(header: &Header) -> Result<(String, String), String> {
    let mut c = String::from("/* Every field the trait carries of each served state set to a value of its own (written by the generator, which\n * states in tests/served.rs the owned structure each must reach the implementation as). */\n");
    let mut rust = String::from("/// The calls the first case's states must reach the implementation as, in full: every field caller.c's `filled_*`\n/// sets, with the value it sets.\nconst FILLED: &[&str] = &[\n");
    let mut counter = 0_u64;
    for (function, parameter, structure, count, call) in FILLED_STATES {
        let mut body = String::new();
        let mut debugs = Vec::new();
        for at in 0..*count {
            let path = if *count == 1 { format!("{parameter}->") } else { format!("{parameter}[{at}].") };
            let (assignments, debug, _) = crate::gallium_interface::filled(header, structure, &path, &mut counter)?;
            body.push_str(&assignments);
            debugs.push(debug);
        }
        let (declared, size) = if *count == 1 { (format!("struct {structure} *{parameter}"), format!("sizeof *{parameter}")) } else { (format!("struct {structure} {parameter}[{count}]"), format!("{count} * sizeof {parameter}[0]")) };
        let _ = writeln!(c, "static void {function}({declared})\n{{\n    memset({parameter}, 0, {size});\n{body}}}\n");
        let received = if *count == 1 { debugs.join("") } else { format!("[{}]", debugs.join(", ")) };
        let _ = writeln!(rust, "    {:?},", format!("{call} {received}"));
    }
    rust.push_str("];\n");
    Ok((c, rust))
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

/// The seeds of the capabilities test, each the counter's start: every field of every capability structure takes a
/// value of its own, the second seed's two past the first's, so that each flag is true in one seed and false in the
/// other (`filled` turns a flag every second field).
const CAPS_SEEDS: [u64; 2] = [0, 2];

/// One numbered check of caller.c's `wc_caller_caps_filled` for each assignment `filled` wrote, and its name in
/// served.rs's `CAPS_CHECKS`.
fn caps_checks(assignments: &str, seed: usize, number: &mut usize, c: &mut String, names: &mut String) {
    for line in assignments.lines() {
        if let Some((place, value)) = line.trim().strip_suffix(';').and_then(|statement| statement.split_once(" = ")) {
            *number += 1;
            let _ = writeln!(c, "        if (status == 0 && {place} != {value})\n            status = {number};");
            let _ = writeln!(names, "    \"seed {seed}: {place} = {value}\",");
        }
    }
}

/// For the capabilities test: caller.c's `wc_caller_caps_filled`, which holds Mesa's screen to the values the
/// recording implementation sets for a seed, each check numbered; served.rs's `CAPS_CHECKS`, naming each check; and the
/// recording crate's functions giving those values (`filled_shader_caps`, `filled_caps`, `filled_compute_caps`).
pub(crate) fn caps_filled(header: &Header) -> Result<(String, String, String), String> {
    let stages = header.known_numbers.get("MESA_SHADER_MESH_STAGES").copied().ok_or("no value for MESA_SHADER_MESH_STAGES")?;
    let mut c = String::from(
        "/* Mesa's screen held, field by field, to the capabilities the recording implementation sets for `seed`\n * (Recording::filling_caps): 0 when every field holds its value, else the number of the first that does not\n * (tests/served.rs's CAPS_CHECKS names each). The screen is destroyed. */\nint wc_caller_caps_filled(uintptr_t address, int seed);\nint wc_caller_caps_filled(uintptr_t address, int seed)\n{\n    struct pipe_screen *screen = (struct pipe_screen *)address;\n    int status = 0;\n",
    );
    let mut names = String::from("/// What each check of caller.c's `wc_caller_caps_filled` holds, by its number less one.\nconst CAPS_CHECKS: &[&str] = &[\n");
    let (mut shader, mut caps, mut compute) = (String::new(), String::new(), String::new());
    let mut number = 0;
    for (seed, start) in CAPS_SEEDS.iter().enumerate() {
        let mut counter = *start;
        let _ = writeln!(c, "    if (seed == {seed}) {{");
        for stage in 0..stages {
            let (assignments, _, rust) = crate::gallium_interface::filled(header, "pipe_shader_caps", &format!("screen->shader_caps[{stage}]."), &mut counter)?;
            caps_checks(&assignments, seed, &mut number, &mut c, &mut names);
            let _ = writeln!(shader, "        ({seed}, {stage}) => {rust},");
        }
        let (assignments, _, rust) = crate::gallium_interface::filled(header, "pipe_caps", "screen->caps.", &mut counter)?;
        caps_checks(&assignments, seed, &mut number, &mut c, &mut names);
        let _ = writeln!(caps, "        {seed} => {rust},");
        let (assignments, _, rust) = crate::gallium_interface::filled(header, "pipe_compute_caps", "screen->compute_caps.", &mut counter)?;
        caps_checks(&assignments, seed, &mut number, &mut c, &mut names);
        let _ = writeln!(compute, "        {seed} => {rust},");
        let _ = writeln!(c, "    }}");
    }
    c.push_str("    screen->destroy(screen);\n    return status;\n}\n");
    names.push_str("];\n");
    let rust = format!(
        "\n/// The seeds `filling_caps` takes.\npub const CAPS_SEEDS: usize = {};\n\n/// Every field of a stage's `ShaderCaps` set to a value of its own, by seed (the direct crate's caller.c holds Mesa's\n/// screen to the same values); Mesa's defaults past the seeds and the stages.\npub fn filled_shader_caps(seed: usize, stage: usize) -> ShaderCaps {{\n    match (seed, stage) {{\n{shader}        _ => ShaderCaps::default(),\n    }}\n}}\n\n/// Every field of `Caps` set to a value of its own, by seed; Mesa's defaults past the seeds.\npub fn filled_caps(seed: usize) -> Caps {{\n    match seed {{\n{caps}        _ => Caps::default(),\n    }}\n}}\n\n/// Every field of `ComputeCaps` set to a value of its own, by seed; Mesa's defaults past the seeds.\npub fn filled_compute_caps(seed: usize) -> ComputeCaps {{\n    match seed {{\n{compute}        _ => ComputeCaps::default(),\n    }}\n}}\n",
        CAPS_SEEDS.len()
    );
    Ok((c, names, rust))
}

/// The head of the capabilities' plant list.
const CAPS_PLANTS_HEAD: &str = "# Gallium's capabilities glue, its plant list (written by tools/api-crate-generator, src/gallium_glue.rs `caps_plants`):\n# one plant per field of each copy toward Mesa's screen (flat.c's wc_flat_*_to: the field dropped; flat.rs's `of`: its\n# value replaced by its default), one per argument of the capabilities' glue (serve.c's asks and copies, serve.rs's\n# functions) and one per place serve.c reads a stage's capability. Every plant must make tests/served.rs fail\n# (every_cap_reaches_mesas_screen holds every field of Mesa's screen to the values the implementation set, in two seeds,\n# and the two capabilities Mesa computes from the stages to Mesa's rule). Run from the repository's root, with\n# WCONTAINERS_MESA set:\n#   python3 -I tools/plants/run-plants.py crates/gallium/implementation/direct/caps-plants.txt --workers 10\ntest: cargo test -q --features hostile -p wcontainers-gallium-direct --test served\n";

/// The capabilities' glue arguments and the places serve.c reads a stage's capability, each a plant: (file, name, the
/// text, its replacement).
const CAPS_GLUE_PLANTS: &[(&str, &str, &str, &str)] = &[
    ("serve.c", "the stage asked of the implementation", "table->shader_caps(key, 0, stage)", "table->shader_caps(key, 0, 0)"),
    ("serve.c", "the screen's key in each stage's ask", "table->shader_caps(key, 0, stage)", "table->shader_caps(0, 0, stage)"),
    ("serve.c", "each stage's copy from the implementation's", "wc_flat_pipe_shader_caps_to(&flat,", "wc_flat_pipe_shader_caps_to(&(const struct wc_flat_pipe_shader_caps){0},"),
    ("serve.c", "each stage written into its own place", "(struct pipe_shader_caps *)&screen->shader_caps[stage]", "(struct pipe_shader_caps *)&screen->shader_caps[0]"),
    ("serve.c", "the fragment stage's first constant buffer size", "screen->shader_caps[MESA_SHADER_FRAGMENT].max_const_buffer0_size", "screen->shader_caps[MESA_SHADER_VERTEX].max_const_buffer0_size"),
    ("serve.c", "hardware selection needs both of the geometry stage's", "geometry->indirect_temp_addr && geometry->max_shader_buffers", "geometry->indirect_temp_addr || geometry->max_shader_buffers"),
    ("serve.c", "the screen's key in the caps' ask", "table->caps(key, 0,", "table->caps(0, 0,"),
    ("serve.c", "the caps' copy from the implementation's", "wc_flat_pipe_caps_to(&caps,", "wc_flat_pipe_caps_to(&(const struct wc_flat_pipe_caps){0},"),
    ("serve.c", "the screen's key in the compute caps' ask", "table->compute_caps(key, 0)", "table->compute_caps(0, 0)"),
    ("serve.c", "the compute caps' copy from the implementation's", "wc_flat_pipe_compute_caps_to(&compute,", "wc_flat_pipe_compute_caps_to(&(const struct wc_flat_pipe_compute_caps){0},"),
    ("serve.rs", "the stage handed to the implementation", "served.screen.shader_caps(values::MesaShaderStage(stage), &mut caps);", "served.screen.shader_caps(values::MesaShaderStage(0), &mut caps);"),
    (
        "serve.rs",
        "the screen's key in shader_caps",
        "    let mut caps = wcontainers_gallium::ShaderCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(screen) {",
        "    let mut caps = wcontainers_gallium::ShaderCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(0) {",
    ),
    ("serve.rs", "the screen's key in caps", "..wcontainers_gallium::Caps::default() };\n    if let Some(served) = registry::screen::<Served<G>>(screen) {", "..wcontainers_gallium::Caps::default() };\n    if let Some(served) = registry::screen::<Served<G>>(0) {"),
    (
        "serve.rs",
        "the screen's key in compute_caps",
        "    let mut caps = wcontainers_gallium::ComputeCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(screen) {",
        "    let mut caps = wcontainers_gallium::ComputeCaps::default();\n    if let Some(served) = registry::screen::<Served<G>>(0) {",
    ),
    ("serve.rs", "the fragment stage's constant buffer size handed on", "max_constant_buffer_size: constant_buffer0_size,", "max_constant_buffer_size: 0,"),
    ("serve.rs", "hardware selection handed on as false", "hardware_gl_select: hardware_select != 0,", "hardware_gl_select: false,"),
    ("serve.rs", "hardware selection handed on as true", "hardware_gl_select: hardware_select != 0,", "hardware_gl_select: true,"),
    ("serve.rs", "each stage's caps as the implementation set them", "FlatShaderCaps::of(&caps)", "FlatShaderCaps::of(&wcontainers_gallium::ShaderCaps::default())"),
    ("serve.rs", "the caps as the implementation set them", "FlatCaps::of(&caps)", "FlatCaps::of(&wcontainers_gallium::Caps::default())"),
    ("serve.rs", "the compute caps as the implementation set them", "FlatComputeCaps::of(&caps)", "FlatComputeCaps::of(&wcontainers_gallium::ComputeCaps::default())"),
];

/// One plant changing line `at` of `text` (the file `file` of the direct crate) to `new`: the text changed is that line
/// with as many lines before it as make it occur once in the file.
fn line_plant(out: &mut String, file: &str, name: &str, text: &str, lines: &[&str], at: usize, new: &str) -> Result<(), String> {
    let mut first = at;
    while text.matches(&lines[first..=at].join("\n")).count() != 1 {
        first = first.checked_sub(1).ok_or_else(|| format!("{file}: {name}: no text before it makes it occur once"))?;
    }
    let before: String = lines[first..at].iter().map(|line| format!("{line}\n")).collect();
    let _ = writeln!(out, "\nplant: {file} {name}\nfile: crates/gallium/implementation/direct/{file}\nold:\n{before}{}\nnew:\n{before}{new}\nend", lines[at]);
    Ok(())
}

/// The capabilities' plant list (crates/gallium/implementation/direct/caps-plants.txt), from the files written.
fn caps_plants(flat_c: &str, flat_rs: &str, serve_c: &str, serve_rs: &str) -> Result<String, String> {
    let mut out = String::from(CAPS_PLANTS_HEAD);
    let c_lines: Vec<&str> = flat_c.lines().collect();
    for filled in crate::gallium::FILLED {
        let opening = format!("void {}_to(", c_flat(filled.structure));
        let start = c_lines.iter().position(|line| line.starts_with(&opening)).ok_or_else(|| format!("flat.c: no {opening}"))?;
        for at in start + 2..c_lines.len() {
            let line = c_lines[at];
            if line == "}" {
                break;
            }
            if line.ends_with(';') {
                let indent = &line[..line.len() - line.trim_start().len()];
                line_plant(&mut out, "flat.c", &format!("{}_to drops {}", c_flat(filled.structure), line.trim()), flat_c, &c_lines, at, &format!("{indent}/* dropped */"))?;
            }
        }
    }
    let rust_lines: Vec<&str> = flat_rs.lines().collect();
    let structures = crate::gallium::FILLED.iter().map(|filled| filled.structure).chain(crate::gallium_interface::HELD_IN_FILLED.iter().copied());
    for structure in structures {
        let opening = format!("    pub fn of(owned: &wcontainers_gallium::{}) -> {} {{", type_name(structure), rust_flat(structure));
        let start = rust_lines.iter().position(|line| *line == opening).ok_or_else(|| format!("flat.rs: no {opening}"))?;
        for at in start + 2..rust_lines.len() {
            let line = rust_lines[at];
            let Some((field, value)) = line.trim().strip_suffix(',').and_then(|field| field.split_once(": ")) else { break };
            let replaced = if value.contains("::of(&owned.") { value.replace(&format!("&owned.{field}"), "&Default::default()") } else { "Default::default()".to_string() };
            line_plant(&mut out, "flat.rs", &format!("{}::of takes no {field}", rust_flat(structure)), flat_rs, &rust_lines, at, &format!("            {field}: {replaced},"))?;
        }
    }
    for (file, name, old, new) in CAPS_GLUE_PLANTS {
        let text = if *file == "serve.c" { serve_c } else { serve_rs };
        if text.matches(old).count() != 1 {
            return Err(format!("{file}: {name}: its text is not there once"));
        }
        let _ = writeln!(out, "\nplant: {file} {name}\nfile: crates/gallium/implementation/direct/{file}\nold:\n{old}\nnew:\n{new}\nend");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Field, Kind, field_layouts, flat_value, keys_of, names_keys, owned_value};
    use crate::cheader::read;

    fn field(kind: Kind, dims: &[usize]) -> Field {
        Field { c: "x".to_string(), rust: "x".to_string(), kind, dims: dims.to_vec() }
    }

    /// An array of flags goes back to Mesa element by element, is read element by element (each refused past 1), and
    /// an array of objects lists each element's key.
    #[test]
    fn arrays_element_by_element() {
        let flags = field(Kind::Flag, &[2]);
        assert_eq!(flat_value(&flags, "owned.x", 0), "owned.x.map(|element0| u8::from(element0))");
        assert_eq!(owned_value(&flags, "self.x", &[], 0), "each(&self.x, |element0| flag(*element0))?");
        let mut keys = String::new();
        keys_of(&field(Kind::Object("Resource"), &[2]), "self.x", &[], 0, &mut keys);
        assert_eq!(keys, "        for element0 in &self.x {\n            keys.object(ObjectKind::Resource, *element0);\n        }\n");
    }

    /// A bit-field's value is checked against its width only where the width is narrower than the number and the
    /// number unsigned (a signed bit-field's value is the number's own).
    #[test]
    fn widths_checked() {
        let number = |rust, bits| field(Kind::Number { rust, bits: Some(bits), value: None }, &[]);
        assert_eq!(owned_value(&number("u32", 3), "self.x", &[], 0), "within(self.x, 3)?");
        assert_eq!(owned_value(&number("i32", 3), "self.x", &[], 0), "self.x");
        assert_eq!(owned_value(&number("u64", 64), "self.x", &[], 0), "self.x");
    }

    /// An enumeration (and the two stage types) may be packed by the compiler: its size in Mesa is at most the flat one.
    #[test]
    fn declared_values_are_at_least() {
        let header = read(&["enum e { A };\nstruct s { enum e a; mesa_shader_stage b; gl_shader_stage c; unsigned d; };".to_string()]);
        let (_, rows, _, count) = field_layouts(&header, &BTreeSet::from(["s".to_string()])).expect("laid out");
        assert_eq!(count, 4);
        let at_least: Vec<bool> = rows.lines().map(|row| row.ends_with("at_least: true },")).collect();
        assert_eq!(at_least, [true, true, true, false]);
    }

    /// Keys are found through structures held by value, and a structure holding itself (a header outside C) ends.
    #[test]
    fn keys_through_structures_held() {
        let header = read(&["struct a { struct a again; int n; };\nstruct b { struct a inner; struct pipe_resource *r; };".to_string()]);
        assert!(!names_keys(&Kind::Nested("a".to_string()), &header, &[]));
        assert!(names_keys(&Kind::Nested("b".to_string()), &header, &[]));
    }
}
