//! Writes the Mesa compiler's crates from the C bridge's headers (crates/mesa-compiler/implementation/direct/bridge's bridge.h and flat.h): the trait
//! `MesaCompiler` in `crates/mesa-compiler/interface.rs`, owned plain data for every structure, and the
//! `implementation/direct` crate, which calls the bridge's functions with every pointer made from its own memory or
//! the caller's slices, copies the bridge's memory out whole into owned values and gives it back once.
//!
//!   api-crate-generator mesa-compiler <flat.h> <bridge.h> <output folder>
//!
//! Every parameter's kind is the row cheader.rs gives it; a structure's fields map by the rules of `field_kind`, and
//! a structure or a field of no rule fails the generator.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::cheader::{CFunction, CParam, Header};

/// The first line of every file written.
const HEADER: &str = "// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/mesa-compiler/implementation/direct/bridge). Do not edit.\n";

/// Functions the trait does not offer, each with its reason.
const NOT_OFFERED: &[(&str, &str)] = &[
    ("wc_init", "called once by the implementation's `new`: it takes a reference on Mesa's type tables for the process"),
    ("wc_finish", "never called: the type tables stay for the process, as every compiler of the process shares them"),
    ("wc_compiler_options", "the address of Mesa's options for a Gallium screen, which only Mesa reads: AC8's Gallium trait (NEEDED B1)"),
    ("wc_nir_free", "the shader type's drop"),
    ("wc_bytes_free", "inside the copy that reads the bytes"),
    ("wc_words_free", "inside the copy that reads the words"),
    ("wc_bindings_free", "inside the copy that reads the bindings"),
    ("wc_glsl_program_free", "inside the copy that reads the program, after its shaders are taken"),
    ("wc_flat_free", "inside the copy that reads the flat form"),
];

/// A structure holding only bridge memory and its count: read out as a `Vec` of its element.
fn container(name: &str) -> Option<&'static str> {
    match name {
        "wc_bytes" => Some("u8"),
        "wc_words" => Some("u32"),
        "wc_bindings" => Some("Binding"),
        "wc_glsl_names" => Some("GlslName"),
        _ => None,
    }
}

/// `wc_flat_type` to `FlatType`.
fn type_name(c: &str) -> String {
    crate::names::pascal_case_of_upper_snake(c.trim_start_matches("struct ").trim_start_matches("wc_"))
}

/// `wc_nir_geometry_vertices_out` to `geometry_vertices_out`.
fn method_name(c: &str) -> String {
    // `clone` would be Clone's method on any implementation that is Clone.
    if c == "wc_nir_clone" {
        return "cloned".to_string();
    }
    crate::names::escaped(c.strip_prefix("wc_nir_").or_else(|| c.strip_prefix("wc_")).unwrap_or(c).to_string())
}

/// The number an array length names: a number, a define, or an enumerator's place (`WC_STAGES` is the third).
fn length(header: &Header, name: &str) -> Result<usize, String> {
    if let Ok(number) = name.parse() {
        return Ok(number);
    }
    if let Some(value) = header.defines.get(name) {
        return value.parse().map_err(|_| format!("{name}: the define {value} is no number"));
    }
    header
        .enumerations
        .values()
        .find_map(|values| values.iter().position(|value| value == name))
        .ok_or_else(|| format!("{name}: no define or enumerator"))
}

/// How a structure's field is kept: in the C mirror and in the owned value.
#[derive(Clone, Debug)]
enum FieldKind {
    /// A number (an enumeration as `u32`, which the caller converts by a checked `TryFrom`).
    Number { mirror: &'static str, owned: &'static str },
    /// C's `bool`: a byte in the mirror, which the glue reads as `!= 0`.
    Bool,
    /// Another structure by value.
    Structure(String),
    /// A zero-ended text the bridge owns or Mesa keeps, or null: copied out.
    Text,
    /// The bridge's array of `count` elements (the field after it): copied out.
    Array { element: Box<FieldKind>, count: String },
    /// A shader the bridge made: taken by the owned value, the field set to null before the bridge frees the rest.
    Shader,
    /// A fixed array of a kind.
    Fixed { element: Box<FieldKind>, length: usize },
}

fn number(base: &str) -> Option<(&'static str, &'static str)> {
    Some(match base {
        "uint32_t" | "unsigned" => ("u32", "u32"),
        "int32_t" => ("i32", "i32"),
        "uint8_t" => ("u8", "u8"),
        "uint64_t" => ("u64", "u64"),
        "float" => ("f32", "f32"),
        "size_t" => ("usize", "u64"),
        enumeration if enumeration.starts_with("enum ") => ("u32", "u32"),
        _ => return None,
    })
}

fn scalar_kind(base: &str, header: &Header) -> Result<FieldKind, String> {
    let base = base.trim_start_matches("const ").trim();
    if base == "bool" {
        return Ok(FieldKind::Bool);
    }
    if let Some((mirror, owned)) = number(base) {
        return Ok(FieldKind::Number { mirror, owned });
    }
    if let Some(name) = base.strip_prefix("struct ")
        && header.structures.contains_key(name)
    {
        return Ok(FieldKind::Structure(name.to_string()));
    }
    Err(format!("{base}: no rule"))
}

/// The kinds of a structure's fields, a pointer's count field consumed with it.
fn fields(name: &str, header: &Header) -> Result<Vec<(String, FieldKind)>, String> {
    let fields = header.structures.get(name).ok_or_else(|| format!("{name}: no structure"))?;
    let mut out = Vec::new();
    let mut at = 0;
    while at < fields.len() {
        let field = &fields[at];
        let base = field.base.trim_start_matches("const ").trim();
        let mut kind = match (field.pointers, base) {
            (0, _) => scalar_kind(base, header)?,
            (1, "char") => FieldKind::Text,
            (1, "struct nir_shader") => FieldKind::Shader,
            (1, _) => {
                let count = fields.get(at + 1).filter(|next| next.pointers == 0 && (next.name.ends_with("count") || next.name.ends_with("size")));
                let Some(count) = count else { return Err(format!("{name}.{}: a pointer with no count after it", field.name)) };
                at += 1;
                FieldKind::Array { element: Box::new(scalar_kind(base, header)?), count: count.name.clone() }
            }
            _ => return Err(format!("{name}.{}: {} pointers", field.name, field.pointers)),
        };
        if let Some(array) = &field.array {
            kind = FieldKind::Fixed { element: Box::new(kind), length: length(header, array)? };
        }
        out.push((field.name.clone(), kind));
        at += 1;
    }
    Ok(out)
}

/// Whether the kind holds a shader, which makes the owned structure generic over the shader type.
fn holds_shader(kind: &FieldKind, header: &Header) -> bool {
    match kind {
        FieldKind::Shader => true,
        FieldKind::Fixed { element, .. } | FieldKind::Array { element, .. } => holds_shader(element, header),
        FieldKind::Structure(name) => fields(name, header).is_ok_and(|fields| fields.iter().any(|(_, kind)| holds_shader(kind, header))),
        _ => false,
    }
}

fn generic(name: &str, header: &Header) -> bool {
    fields(name, header).is_ok_and(|fields| fields.iter().any(|(_, kind)| holds_shader(kind, header)))
}

fn mirror_type(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror, .. } => (*mirror).to_string(),
        FieldKind::Bool => "u8".to_string(),
        FieldKind::Structure(name) => format!("{}Mirror", type_name(name)),
        FieldKind::Text => "*mut u8".to_string(),
        FieldKind::Array { element, .. } => format!("*mut {}", mirror_type(element)),
        FieldKind::Shader => "*mut c_void".to_string(),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", mirror_type(element)),
    }
}

fn owned_type(kind: &FieldKind, header: &Header, shader: &str) -> String {
    match kind {
        FieldKind::Number { owned, .. } => (*owned).to_string(),
        FieldKind::Bool => "bool".to_string(),
        FieldKind::Structure(name) => match container(name) {
            Some(element) => format!("Vec<{element}>"),
            None if generic(name, header) => format!("{}<{shader}>", type_name(name)),
            None => type_name(name),
        },
        FieldKind::Text => "Option<Vec<u8>>".to_string(),
        FieldKind::Array { element, .. } => format!("Vec<{}>", owned_type(element, header, shader)),
        FieldKind::Shader => format!("Option<{shader}>"),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", owned_type(element, header, shader)),
    }
}

/// Whether the kind's mirror holds a pointer (so its zero is written, not derived).
fn holds_pointer(kind: &FieldKind, header: &Header) -> bool {
    match kind {
        FieldKind::Number { .. } | FieldKind::Bool => false,
        FieldKind::Structure(name) => fields(name, header).is_ok_and(|fields| fields.iter().any(|(_, kind)| holds_pointer(kind, header))),
        FieldKind::Fixed { element, .. } => holds_pointer(element, header),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Shader => true,
    }
}

/// Whether reading the kind out of a mirror needs no pointer read (so the conversion is safe code).
fn plain(kind: &FieldKind, header: &Header) -> bool {
    match kind {
        FieldKind::Number { .. } | FieldKind::Bool => true,
        FieldKind::Structure(name) => fields(name, header).is_ok_and(|fields| fields.iter().all(|(_, kind)| plain(kind, header))),
        FieldKind::Fixed { element, .. } => plain(element, header),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Shader => false,
    }
}

/// The comment lines before each structure and field in the header text, for the owned types' docs.
fn comments(texts: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for text in texts {
        let mut pending = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("/*") {
                pending = rest.trim_end_matches("*/").trim().to_string();
                if trimmed.ends_with("*/") {
                    continue;
                }
                continue;
            }
            if trimmed.starts_with('*') && !trimmed.starts_with("*/") {
                let _ = write!(pending, " {}", trimmed.trim_start_matches('*').trim_end_matches("*/").trim());
                continue;
            }
            if trimmed.starts_with("*/") {
                continue;
            }
            if !pending.is_empty() {
                let key = trimmed.trim_end_matches(['{', ';']).trim().to_string();
                out.insert(key, std::mem::take(&mut pending).trim().to_string());
            }
        }
    }
    out
}

fn doc(comments: &BTreeMap<String, String>, key: &str, fallback: &str, indent: &str) -> String {
    let text = comments.get(key).map_or(fallback.to_string(), |c| c.clone());
    let mut sentence = text.trim().to_string();
    if !sentence.ends_with('.') {
        sentence.push('.');
    }
    format!("{indent}/// {}\n", sentence.replace('\n', " "))
}

/// The structures every function reaches, in the order written.
fn reached(header: &Header) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    fn visit(name: &str, header: &Header, out: &mut Vec<String>) {
        if out.iter().any(|seen| seen == name) || container(name).is_some_and(|_| false) {
            return;
        }
        if let Ok(fields) = fields(name, header) {
            for (_, kind) in &fields {
                let mut kind = kind.clone();
                loop {
                    match kind {
                        FieldKind::Fixed { element, .. } | FieldKind::Array { element, .. } => kind = *element,
                        _ => break,
                    }
                }
                if let FieldKind::Structure(inner) = kind {
                    visit(&inner, header, out);
                }
            }
        }
        out.push(name.to_string());
    }
    for function in &header.functions {
        for param in std::iter::once(&function.ret).chain(&function.params) {
            if let Some(name) = param.base.trim_start_matches("const ").trim().strip_prefix("struct ")
                && header.structures.contains_key(name)
            {
                visit(name, header, &mut out);
            }
        }
    }
    out
}

/// The structures the glue reads out of the bridge's memory: those of the out parameters, and every one inside them.
fn read_out(header: &Header) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    fn visit(name: &str, header: &Header, out: &mut Vec<String>) {
        if out.iter().any(|seen| seen == name) {
            return;
        }
        out.push(name.to_string());
        for (_, kind) in fields(name, header).unwrap_or_default() {
            let mut kind = kind;
            while let FieldKind::Fixed { element, .. } | FieldKind::Array { element, .. } = kind {
                kind = *element;
            }
            if let FieldKind::Structure(inner) = kind {
                visit(&inner, header, out);
            }
        }
    }
    for function in &header.functions {
        for param in &function.params {
            let row = crate::cheader::row_of(function, param, header).unwrap_or_default();
            if row.starts_with("the bridge's memory, copied out") || row.starts_with("plain data written") {
                visit(param.base.trim_start_matches("const ").trim().trim_start_matches("struct "), header, &mut out);
            }
        }
    }
    out
}

/// The owned structures and their mirrors.
fn write_structures(header: &Header, comments: &BTreeMap<String, String>, interface: &mut String, mirrors: &mut String) -> Result<(), String> {
    let read_out = read_out(header);
    for name in reached(header) {
        let fields = fields(&name, header)?;
        let owned = type_name(&name);
        let (generics, shader) = if generic(&name, header) { ("<S>", "S") } else { ("", "S") };
        // The mirror: the C layout exactly.
        let pointers = fields.iter().any(|(_, kind)| holds_pointer(kind, header));
        let derives = if pointers { "Clone, Copy" } else { "Clone, Copy, Default" };
        let _ = writeln!(mirrors, "/// `struct {name}` as the bridge lays it out.\n#[repr(C)]\n#[derive({derives})]\npub(crate) struct {owned}Mirror {{");
        for (field, kind) in &fields {
            let _ = writeln!(mirrors, "    pub(crate) {}: {},", crate::names::escaped(field.clone()), mirror_type(kind));
            if let FieldKind::Array { count, .. } = kind {
                let count_base = header.structures[&name].iter().find(|f| &f.name == count).map(|f| f.base.clone()).unwrap_or_default();
                let ty = if count_base == "size_t" { "usize" } else { "u32" };
                let _ = writeln!(mirrors, "    pub(crate) {count}: {ty},");
            }
        }
        let _ = writeln!(mirrors, "}}\n");
        // Zeroed: what the bridge's free takes back as nothing.
        let zeros: Vec<String> = fields
            .iter()
            .flat_map(|(field, kind)| {
                let mut out = vec![format!("{}: {}", crate::names::escaped(field.clone()), zero(kind))];
                if let FieldKind::Array { count, .. } = kind {
                    out.push(format!("{count}: 0"));
                }
                out
            })
            .collect();
        if pointers {
            let _ = writeln!(mirrors, "impl Default for {owned}Mirror {{\n    fn default() -> {owned}Mirror {{\n        {owned}Mirror {{ {} }}\n    }}\n}}\n", zeros.join(", "));
        }
        // Read out into owned values; the shaders taken, their fields set to null.
        let owned_ty = match container(&name) {
            Some(element) => format!("Vec<{element}>"),
            None if generic(&name, header) => format!("{owned}<Shader>"),
            None => owned.clone(),
        };
        let body = match container(&name) {
            Some(_) => {
                let (field, kind) = &fields[0];
                convert(kind, &format!("self.{field}"))
            }
            None => {
                let parts: Vec<String> = fields.iter().map(|(field, kind)| format!("{}: {}", crate::names::escaped(field.clone()), convert(kind, &format!("self.{}", crate::names::escaped(field.clone()))))).collect();
                format!("{owned} {{ {} }}", parts.join(", "))
            }
        };
        if read_out.contains(&name) {
            let _ = writeln!(mirrors, "impl {owned}Mirror {{\n    /// The values read out (the bridge's arrays and texts copied, its shaders taken).\n    pub(crate) fn owned(&mut self) -> {owned_ty} {{\n        {body}\n    }}\n}}\n");
        }
        if fields.iter().all(|(_, kind)| plain(kind, header)) && container(&name).is_none() {
            let parts: Vec<String> = fields.iter().map(|(field, kind)| format!("{0}: {1}", crate::names::escaped(field.clone()), back(kind, &format!("value.{}", crate::names::escaped(field.clone()))))).collect();
            let _ = writeln!(mirrors, "impl From<&{owned}> for {owned}Mirror {{\n    fn from(value: &{owned}) -> {owned}Mirror {{\n        {owned}Mirror {{ {} }}\n    }}\n}}\n", parts.join(", "));
        }
        if container(&name).is_some() {
            continue;
        }
        let _ = write!(interface, "{}", doc(comments, &format!("struct {name}"), &format!("The bridge's `struct {name}`, owned"), ""));
        let derives = if generic(&name, header) { "#[derive(Debug)]" } else { "#[derive(Clone, Debug, PartialEq)]" };
        let _ = writeln!(interface, "{derives}\npub struct {owned}{generics} {{");
        for (field, kind) in &fields {
            let key = header.structures[&name].iter().find(|f| &f.name == field).map(|f| {
                let stars = "*".repeat(f.pointers);
                let array = f.array.as_ref().map(|a| format!("[{a}]")).unwrap_or_default();
                format!("{}{}{}{}", f.base, if stars.is_empty() { " " } else { " " }, if stars.is_empty() { String::new() } else { stars.clone() }, format!("{}{array}", f.name)).replace("  ", " ")
            });
            let _ = write!(interface, "{}", doc(comments, &key.unwrap_or_default(), &format!("The bridge's `{field}`"), "    "));
            let _ = writeln!(interface, "    pub {}: {},", crate::names::escaped(field.clone()), owned_type(kind, header, shader));
        }
        let _ = writeln!(interface, "}}\n");
        let _ = plain;
    }
    Ok(())
}

/// A mirror field's zero.
fn zero(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror: "f32", .. } => "0.0".to_string(),
        FieldKind::Number { .. } | FieldKind::Bool => "0".to_string(),
        FieldKind::Structure(name) => format!("{}Mirror::default()", type_name(name)),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Shader => "core::ptr::null_mut()".to_string(),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", zero(element)),
    }
}

/// The owned value of a mirror's place.
fn convert(kind: &FieldKind, place: &str) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => format!("u64::try_from({place}).unwrap_or(u64::MAX)"),
        FieldKind::Number { .. } => place.to_string(),
        FieldKind::Bool => format!("{place} != 0"),
        FieldKind::Structure(_) => format!("{place}.owned()"),
        FieldKind::Text => format!("text({place})"),
        FieldKind::Shader => format!("Shader::owning(core::mem::replace(&mut {place}, core::ptr::null_mut()))"),
        FieldKind::Fixed { element, .. } => format!("{place}.each_mut().map(|element| {})", convert_element(element)),
        FieldKind::Array { element, count } => {
            let owner = place.rsplit_once('.').map_or("self", |(owner, _)| owner);
            match element.as_ref() {
                FieldKind::Number { mirror, .. } if *mirror != "usize" => format!("array({place}, {owner}.{count})"),
                _ => format!("array({place}, {owner}.{count}).iter_mut().map(|element| {}).collect()", convert_element(element)),
            }
        }
    }
}

/// The owned value of `element`, a `&mut` to one mirror element.
fn convert_element(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => "u64::try_from(*element).unwrap_or(u64::MAX)".to_string(),
        FieldKind::Number { .. } => "*element".to_string(),
        FieldKind::Bool => "*element != 0".to_string(),
        FieldKind::Structure(_) => "element.owned()".to_string(),
        FieldKind::Text => "text(*element)".to_string(),
        FieldKind::Shader => "Shader::owning(core::mem::replace(element, core::ptr::null_mut()))".to_string(),
        FieldKind::Fixed { .. } | FieldKind::Array { .. } => "compile_error!(\"an array of arrays\")".to_string(),
    }
}

/// The mirror's value of an owned place (plain kinds only).
fn back(kind: &FieldKind, place: &str) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => format!("usize::try_from({place}).unwrap_or(usize::MAX)"),
        FieldKind::Number { .. } => place.to_string(),
        FieldKind::Bool => format!("u8::from({place})"),
        FieldKind::Structure(name) => format!("{}Mirror::from(&{place})", type_name(name)),
        FieldKind::Fixed { element, .. } => format!("{place}.map(|element| {})", back(element, "element")),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Shader => "core::ptr::null_mut()".to_string(),
    }
}

/// One trait parameter and what the glue does for it.
struct Param {
    /// The trait's parameter (`name: type`), or none when the glue makes the C argument itself.
    signature: Option<String>,
    /// Statements before the call.
    before: Vec<String>,
    /// The C argument.
    argument: String,
    /// The C parameter's type in the `extern` declaration.
    c: String,
    /// What the mark says of it.
    fact: Option<String>,
}

/// What a function returns, read from its C return and its out parameter.
struct Out {
    ty: String,
    after: Vec<String>,
}

/// Whether a name is an array length anywhere in the header.
fn uses_as_length(header: &Header, name: &str) -> bool {
    let of = |param: &CParam| param.array.as_deref() == Some(name);
    header.structures.values().flatten().any(of) || header.functions.iter().flat_map(|f| &f.params).any(of)
}

fn c_scalar(base: &str) -> Option<&'static str> {
    number(base.trim_start_matches("const ").trim()).map(|(mirror, _)| mirror).or_else(|| (base == "bool").then_some("bool"))
}

fn parameters(function: &CFunction, header: &Header) -> Result<(Vec<Param>, Option<Out>, Vec<String>), String> {
    let mut params = Vec::new();
    let mut out = None;
    let mut written = Vec::new();
    let names: Vec<String> = function.params.iter().map(|p| crate::names::escaped(p.name.clone())).collect();
    // A count parameter beside a slice is the slice's length.
    let counted_by = |at: usize| -> Option<String> {
        let previous = function.params.get(at.wrapping_sub(1))?;
        let row = crate::cheader::row_of(function, previous, header)?;
        (row.contains("count beside") || row.contains("capacity beside")).then(|| names[at - 1].clone())
    };
    for (at, param) in function.params.iter().enumerate() {
        let name = &names[at];
        let base = param.base.trim_start_matches("const ").trim();
        let is_const = param.base.starts_with("const ");
        let row = crate::cheader::row_of(function, param, header).ok_or_else(|| format!("{}({}): unmapped", function.name, param.name))?;
        let p = match row.split(':').next().unwrap_or(row) {
            "shader changed" | "shader read and changed" => Param {
                signature: Some(format!("{name}: &mut Self::Shader")),
                before: vec![],
                argument: format!("{name}.nir.as_ptr()"),
                c: "*mut c_void".to_string(),
                fact: Some(format!("{name} is a shader of ours, changed by the call alone")),
            },
            "shader read" => Param {
                signature: Some(format!("{name}: &Self::Shader")),
                before: vec![],
                argument: format!("{name}.nir.as_ptr()"),
                c: "*const c_void".to_string(),
                fact: Some(format!("{name} is a shader of ours, read by the call alone")),
            },
            "number or bool by value" => {
                let ty = c_scalar(base).ok_or_else(|| format!("{}({}): {base}", function.name, param.name))?;
                if let Some(slice) = counted_by(at).filter(|_| ty == "usize" || ty == "u32") {
                    // The count beside a slice (`count`, `capacity`, `buffers`).
                    let ty_name = if ty == "usize" { "usize" } else { "u32" };
                    Param {
                        signature: None,
                        before: if ty_name == "u32" {
                            vec![format!("let Ok({name}) = u32::try_from({slice}.len()) else {{ return Default::default(); }};")]
                        } else {
                            vec![]
                        },
                        argument: if ty_name == "u32" { name.clone() } else { format!("{slice}.len()") },
                        c: ty_name.to_string(),
                        fact: Some(format!("{name} is {slice}'s length")),
                    }
                } else {
                    let rust = match ty {
                        "usize" => "u64",
                        other => other,
                    };
                    Param {
                        signature: Some(format!("{name}: {rust}")),
                        before: if ty == "usize" { vec![format!("let Ok({name}) = usize::try_from({name}) else {{ return Default::default(); }};")] } else { vec![] },
                        argument: if ty == "bool" { format!("u8::from({name})") } else { name.clone() },
                        c: if ty == "bool" { "u8".to_string() } else { ty.to_string() },
                        fact: None,
                    }
                }
            }
            "a declared value" => {
                let enumeration = base.trim_start_matches("enum ");
                let values = header.enumerations.get(enumeration).ok_or_else(|| format!("{enumeration}: no enumeration"))?;
                // The last enumerator is the count when the header uses it as an array length (`WC_STAGES`).
                let sentinel = values.last().is_some_and(|last| uses_as_length(header, last));
                let count = values.len() - usize::from(sentinel);
                Param {
                    signature: Some(format!("{name}: u32")),
                    before: vec![format!("if {name} >= {count} {{ return Default::default(); }}")],
                    argument: name.clone(),
                    c: "u32".to_string(),
                    fact: Some(format!("{name} is one of enum {enumeration}'s {count} values")),
                }
            }
            "plain data by value" => {
                let structure = base.trim_start_matches("struct ");
                Param {
                    signature: Some(format!("{name}: {}", type_name(structure))),
                    before: vec![],
                    argument: format!("{}Mirror::from(&{name})", type_name(structure)),
                    c: format!("{}Mirror", type_name(structure)),
                    fact: None,
                }
            }
            "plain data read during the call, with the count beside it" => {
                let structure = base.trim_start_matches("struct ");
                Param {
                    signature: Some(format!("{name}: &[{}]", type_name(structure))),
                    before: vec![format!("let {name}: Vec<{0}Mirror> = {name}.iter().map({0}Mirror::from).collect();", type_name(structure))],
                    argument: format!("{name}.as_ptr()"),
                    c: format!("*const {}Mirror", type_name(structure)),
                    fact: Some(format!("{name} holds as many elements as the count beside it, in our memory")),
                }
            }
            "plain data read during the call" => {
                let structure = base.trim_start_matches("struct ");
                let n = length(header, param.array.as_deref().unwrap_or("0"))?;
                Param {
                    signature: Some(format!("{name}: &[{}; {n}]", type_name(structure))),
                    before: vec![format!("let {name}: [{0}Mirror; {n}] = {name}.each_ref().map({0}Mirror::from);", type_name(structure))],
                    argument: format!("{name}.as_ptr()"),
                    c: format!("*const {}Mirror", type_name(structure)),
                    fact: Some(format!("{name} is {n} elements of ours, as the header's array length says")),
                }
            }
            "plain data written during the call, as many as the capacity beside it" => {
                let structure = base.trim_start_matches("struct ");
                written.push(format!("for (to, from) in {name}.iter_mut().zip({name}_room.iter_mut()) {{ *to = from.owned(); }}"));
                Param {
                    signature: Some(format!("{name}: &mut [{}]", type_name(structure))),
                    before: vec![format!("let mut {name}_room: Vec<{0}Mirror> = vec![{0}Mirror::default(); {name}.len()];", type_name(structure))],
                    argument: format!("{name}_room.as_mut_ptr()"),
                    c: format!("*mut {}Mirror", type_name(structure)),
                    fact: Some(format!("{name}_room holds as many elements as the capacity beside it, in our memory")),
                }
            }
            "words read during the call, with the count beside them" => Param {
                signature: Some(format!("{name}: &[u32]")),
                before: vec![],
                argument: format!("{name}.as_ptr()"),
                c: "*const u32".to_string(),
                fact: Some(format!("{name} holds as many words as the count beside it")),
            },
            "words written during the call, as many as the count beside them" => Param {
                signature: Some(format!("{name}: &mut [u32]")),
                before: vec![],
                argument: format!("{name}.as_mut_ptr()"),
                c: "*mut u32".to_string(),
                fact: Some(format!("{name} holds as many words as the count beside it")),
            },
            "zero-ended text read during the call" => Param {
                signature: Some(format!("{name}: &core::ffi::CStr")),
                before: vec![],
                argument: format!("{name}.as_ptr()"),
                c: "*const core::ffi::c_char".to_string(),
                fact: Some(format!("{name} is zero-ended")),
            },
            "texts by stage, each zero-ended or none" => {
                let n = length(header, param.array.as_deref().unwrap_or("0"))?;
                Param {
                    signature: Some(format!("{name}: [Option<&core::ffi::CStr>; {n}]")),
                    before: vec![format!("let {name}: [*const core::ffi::c_char; {n}] = {name}.map(|text| text.map_or(core::ptr::null(), core::ffi::CStr::as_ptr));")],
                    argument: format!("{name}.as_ptr()"),
                    c: "*const *const core::ffi::c_char".to_string(),
                    fact: Some(format!("{name} is {n} texts of ours, each zero-ended or null")),
                }
            }
            "shaders made into the caller's array" => {
                let n = length(header, param.array.as_deref().unwrap_or("0"))?;
                out = Some(Out {
                    ty: format!("Option<[Option<Self::Shader>; {n}]>"),
                    after: vec![format!("let made = {name}.map(Shader::owning);"), "result.then_some(made)".to_string(), String::new()],
                });
                Param {
                    signature: None,
                    before: vec![format!("let mut {name} = [core::ptr::null_mut::<c_void>(); {n}];")],
                    argument: format!("{name}.as_mut_ptr()"),
                    c: "*mut *mut c_void".to_string(),
                    fact: Some(format!("{name} is room for {n} shaders, ours")),
                }
            }
            "the bridge's memory, copied out whole into owned Rust values, then given back once" => {
                let structure = base.trim_start_matches("struct ");
                let free = format!("{}_free", structure);
                let owned = match container(structure) {
                    Some(element) => format!("Vec<{element}>"),
                    None if generic(structure, header) => format!("{}<Self::Shader>", type_name(structure)),
                    None => type_name(structure),
                };
                out = Some(Out { ty: owned, after: vec![String::new(), free] });
                Param {
                    signature: None,
                    before: vec![format!("let mut {name} = {}Mirror::default();", type_name(structure))],
                    argument: format!("&raw mut {name}"),
                    c: format!("*mut {}Mirror", type_name(structure)),
                    fact: Some(format!("{name} is one zeroed struct {structure} of ours")),
                }
            }
            // Only in functions the trait does not offer: declared, called by the glue alone.
            "the bridge's memory given back" => Param {
                signature: None,
                before: vec![],
                argument: String::new(),
                c: format!("*mut {}Mirror", type_name(base.trim_start_matches("struct "))),
                fact: None,
            },
            "shader given back" => Param { signature: None, before: vec![], argument: String::new(), c: "*mut c_void".to_string(), fact: None },
            other => return Err(format!("{}({}): the row {other} has no glue", function.name, param.name)),
        };
        let _ = is_const;
        params.push(p);
    }
    Ok((params, out, written))
}

/// The C return type in the declaration and what the method returns, before an out parameter.
fn return_of(function: &CFunction, header: &Header) -> Result<(String, Option<String>), String> {
    let ret = &function.ret;
    let base = ret.base.trim_start_matches("const ").trim();
    Ok(match (base, ret.pointers) {
        ("void", 0) => (String::new(), None),
        ("struct nir_shader", 1) => (" -> *mut c_void".to_string(), Some("Option<Self::Shader>".to_string())),
        // Mesa's options for a Gallium screen: declared, not offered.
        ("struct nir_shader_compiler_options", 1) => (" -> *const c_void".to_string(), None),
        ("bool", 0) => (" -> u8".to_string(), Some("bool".to_string())),
        (enumeration, 0) if enumeration.starts_with("enum ") => (" -> u32".to_string(), Some("u32".to_string())),
        (number_type, 0) => {
            let ty = c_scalar(number_type).ok_or_else(|| format!("{}: returns {number_type}", function.name))?;
            (format!(" -> {ty}"), Some(ty.to_string()))
        }
        _ => {
            let _ = header;
            return Err(format!("{}: returns {base} with {} pointers", function.name, ret.pointers));
        }
    })
}

pub fn generate(texts: &[String], out: &std::path::Path) -> Result<String, String> {
    let header = crate::cheader::read(texts);
    let comments = comments(texts);
    let mut report = String::new();
    let mut interface = String::from(HEADER);
    interface.push_str(
        "//! Mesa's shader compiler as our C bridge offers it (crates/mesa-compiler/implementation/direct/bridge: NIR made from SPIR-V or GLSL, changed for the\n//! routes, read as a flat form, written as SPIR-V), as a safe trait. Every enumeration the bridge writes is a `u32` the\n//! caller converts by a checked `TryFrom`.\n\n",
    );
    let mut mirrors = String::new();
    write_structures(&header, &comments, &mut interface, &mut mirrors)?;
    // Constants: the defines and the enumerations' values, as numbers.
    let mut constants = String::from(HEADER);
    constants.push_str("//! The bridge's numbers: its defines and every enumeration's values.\n\n");
    for (name, value) in &header.defines {
        if let Ok(number) = value.parse::<u64>() {
            let _ = writeln!(constants, "/// `{name}` of the bridge's headers.\npub const {}: u32 = {number};", name.trim_start_matches("WC_"));
        }
    }
    for (enumeration, values) in &header.enumerations {
        for (at, value) in values.iter().enumerate() {
            if value.contains('(') || value.contains(' ') {
                continue;
            }
            let _ = writeln!(constants, "/// `{value}` of `enum {enumeration}`.\npub const {}: u32 = {at};", value.trim_start_matches("WC_"));
        }
    }
    // The trait.
    let _ = writeln!(interface, "pub mod constants;\n");
    let _ = writeln!(
        interface,
        "/// Mesa's compiler through the bridge: one method a bridge function, each shader a value of the implementation's\n/// own type that owns one NIR shader and gives it back when dropped.\npub trait MesaCompiler: Sized {{\n    /// One NIR shader, owned.\n    type Shader;\n\n    /// The compiler, ready (Mesa's type tables taken for the process).\n    fn new() -> Self;\n"
    );
    let mut calls = String::new();
    let mut externs = String::new();
    let mut offered = 0;
    let mut marks = 0;
    for function in &header.functions {
        let (c_ret, ret) = return_of(function, &header)?;
        let (params, out_param, written) = parameters(function, &header)?;
        let c_params: Vec<String> = params.iter().map(|p| p.c.clone()).collect();
        let names: Vec<String> = function.params.iter().map(|p| crate::names::escaped(p.name.clone())).collect();
        let declared = !matches!(function.name.as_str(), "wc_finish" | "wc_compiler_options");
        if declared {
            let _ = writeln!(externs, "    fn {}({}){c_ret};", function.name, names.iter().zip(&c_params).map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", "));
        }
        if let Some((_, reason)) = NOT_OFFERED.iter().find(|(name, _)| *name == function.name) {
            let _ = writeln!(report, "  NOT OFFERED {}: {reason}", function.name);
            continue;
        }
        offered += 1;
        let signature_params: Vec<String> = params.iter().filter_map(|p| p.signature.clone()).collect();
        let method_ret = match (&ret, &out_param) {
            (_, Some(out)) if function.name == "wc_glsl_to_nir" => format!("Result<{}, Vec<u8>>", out.ty),
            (Some(r), Some(out)) if r == "u32" => format!("Result<{}, u32>", out.ty),
            (Some(r), Some(out)) if r == "bool" && out.ty.starts_with("Option<") => out.ty.clone(),
            (Some(r), Some(out)) if r == "bool" => format!("Option<{}>", out.ty),
            (None, Some(out)) => out.ty.clone(),
            (Some(r), None) => r.clone(),
            (None, None) => String::new(),
            (Some(r), Some(out)) => return Err(format!("{}: returns {r} with {}", function.name, out.ty)),
        };
        let arrow = if method_ret.is_empty() { String::new() } else { format!(" -> {method_ret}") };
        let method = method_name(&function.name);
        let receiver = std::iter::once("&self".to_string()).chain(signature_params.iter().cloned()).collect::<Vec<_>>().join(", ");
        let comment = comments.get(&format!(
            "{}{}",
            function.ret.base,
            if function.ret.pointers > 0 { format!(" {}", "*".repeat(function.ret.pointers)) } else { String::new() }
        ));
        let _ = comment;
        let _ = writeln!(interface, "    /// Calls the bridge's `{}`.\n    fn {method}({receiver}){arrow};\n", function.name);
        // The glue.
        let facts: Vec<String> = params.iter().filter_map(|p| p.fact.clone()).collect();
        let reason = format!(
            "bridge.h: {} takes the types declared there{}",
            function.name,
            if facts.is_empty() { "; plain values only".to_string() } else { format!("; {}", facts.join("; ")) }
        );
        let typed_params: Vec<String> = signature_params.iter().map(|p| p.replace("Self::Shader", "Shader")).collect();
        let receiver = std::iter::once("&self".to_string()).chain(typed_params).collect::<Vec<_>>().join(", ");
        let typed_ret = arrow.replace("Self::Shader", "Shader");
        let _ = writeln!(calls, "    fn {method}({receiver}){typed_ret} {{");
        for p in &params {
            for line in &p.before {
                let _ = writeln!(calls, "        {line}");
            }
        }
        let arguments: Vec<String> = params.iter().map(|p| p.argument.clone()).collect();
        let plain_return = matches!((&ret, &out_param), (Some(_), None)) && function.ret.pointers == 0 && written.is_empty();
        let tail_only = plain_return && c_ret != " -> u8";
        let binding = if c_ret.is_empty() || tail_only { "" } else { "let result = " };
        let _ = writeln!(calls, "        #[expect(unsafe_code, reason = \"{}\")]", reason.replace('"', "'"));
        let _ = writeln!(calls, "        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.");
        let end = if tail_only { "" } else { ";" };
        let _ = writeln!(calls, "        {binding}unsafe {{ {}({}) }}{end}", function.name, arguments.join(", "));
        marks += 1;
        if c_ret == " -> u8" && plain_return {
            let _ = writeln!(calls, "        result != 0");
        } else if c_ret == " -> u8" {
            let _ = writeln!(calls, "        let result = result != 0;");
        }
        for line in &written {
            let _ = writeln!(calls, "        {line}");
        }
        match (&ret, &out_param, function.ret.pointers) {
            (Some(_), None, 1) => {
                let _ = writeln!(calls, "        Shader::owning(result)");
            }
            (Some(_), None, _) if !tail_only && !(plain_return && c_ret == " -> u8") => {
                let _ = writeln!(calls, "        result");
            }
            (_, Some(out), _) if out.after.len() == 2 && out.after[0].is_empty() => {
                // Copied out whole, then given back once.
                let free = &out.after[1];
                let mirror_name = params.iter().find(|p| p.argument.starts_with("&raw mut ")).map(|p| p.argument.trim_start_matches("&raw mut ").to_string()).unwrap_or_default();
                let _ = writeln!(calls, "        let copied = {mirror_name}.owned();");
                let _ = writeln!(calls, "        #[expect(unsafe_code, reason = \"bridge.h: {free} gives back what the bridge allocated into {mirror_name} (null pointers are given back as none: free(NULL))\")]");
                let _ = writeln!(calls, "        // SAFETY: {mirror_name} is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.");
                let _ = writeln!(calls, "        unsafe {{ {free}(&raw mut {mirror_name}) }};");
                marks += 1;
                let tail = match ret.as_deref() {
                    _ if function.name == "wc_glsl_to_nir" => "if result { Ok(copied) } else { Err(copied.log.unwrap_or_default()) }".to_string(),
                    Some("u32") => "if result == 0 { Ok(copied) } else { Err(result) }".to_string(),
                    Some("bool") => "result.then_some(copied)".to_string(),
                    _ => "copied".to_string(),
                };
                let _ = writeln!(calls, "        {tail}");
            }
            (_, Some(out), _) => {
                for line in out.after.iter().filter(|line| !line.is_empty()) {
                    let _ = writeln!(calls, "        {line}");
                }
            }
            _ => {}
        }
        // Copies back what the bridge wrote into the glue's room.
        let _ = calls;
        let _ = writeln!(calls, "    }}\n");
    }
    let _ = writeln!(interface, "}}");
    let _ = writeln!(report, "MESA COMPILER: functions {}, offered {offered}, marks {marks}", header.functions.len());
    let crate_folder = out.join("crates").join("mesa-compiler");
    let direct = crate_folder.join("implementation").join("direct");
    std::fs::create_dir_all(&direct).map_err(|error| error.to_string())?;
    let write = |path: std::path::PathBuf, text: &str| std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()));
    write(crate_folder.join("interface.rs"), &interface)?;
    write(crate_folder.join("constants.rs"), &constants)?;
    write(
        crate_folder.join("Cargo.toml"),
        "[package]\nname = \"wcontainers-mesa-compiler\"\ndescription = \"Mesa's shader compiler, as our C bridge offers it, as a safe trait.\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\npublish = false\n\n[lib]\npath = \"interface.rs\"\n\n[lints]\nworkspace = true\n",
    )?;
    let template = include_str!("../templates/mesa-compiler/direct.rs");
    let direct_text = format!(
        "{HEADER}{template}\n// ----- The bridge's structures as it lays them out. -----\n\n{mirrors}// ----- The bridge's functions. -----\n\n#[expect(unsafe_code, reason = \"bridge.h declares these functions; each is reached only through the method that marks its call\")]\nunsafe extern \"C\" {{\n{externs}}}\n\nimpl wcontainers_mesa_compiler::MesaCompiler for Bridge {{\n    type Shader = Shader;\n\n    fn new() -> Bridge {{\n        Bridge::started()\n    }}\n\n{calls}}}\n"
    );
    write(direct.join("implementation.rs"), &direct_text)?;
    write(direct.join("Cargo.toml"), include_str!("../templates/mesa-compiler/direct.toml"))?;
    Ok(report)
}
