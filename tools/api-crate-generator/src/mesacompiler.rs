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
];

/// Shaders whose stage is part of their type, so that a shader of one stage cannot be written where another is taken:
/// the trait has one type of each per stage of `enum wc_stage`, and a function taking or making one is offered once
/// per stage it takes. What the glue relies on for it is `STAGE_FACT`.
const BY_STAGE: &[&str] = &["wc_received", "wc_prepared"];

/// The bridge's facts a shader's stage type rests on (glsl.cpp, wc_glsl_to_nir; bridge.c, wc_received_prepare).
const STAGE_FACT: &str = "of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes)";

/// Functions of a shader by stage that take only some stages: each stage taken, with the method offered for it. Every
/// other function of a shader by stage is offered for every stage.
const STAGES_TAKEN: &[(&str, &[(&str, &str)])] = &[
    (
        "wc_prepared_points_as_squares",
        &[("WC_STAGE_VERTEX", "prepared_vertex_points_as_squares"), ("WC_STAGE_TESS_EVALUATION", "prepared_tess_evaluation_points_as_squares")],
    ),
    ("wc_prepared_geometry_points_as_squares", &[("WC_STAGE_GEOMETRY", "prepared_geometry_points_as_squares")]),
];

/// The enumeration naming the stages, and the prefix of its stages (its last value, `WC_STAGES`, is their count).
const STAGE_ENUMERATION: &str = "wc_stage";
const STAGE_PREFIX: &str = "WC_STAGE_";
/// Where a type of a shader by stage names its stage, before the stage is known.
const STAGE_MARK: &str = "@";
/// Where a method's name goes, and its stage in its summary, before each is known.
const METHOD: &str = "@METHOD@";
const STAGE_DOC: &str = "@STAGE_DOC@";

/// The trait of `stage` the stages a function of `STAGES_TAKEN` takes implement: `TakesPreparedPointsAsSquares`.
fn taken_marker(function: &str) -> String {
    format!("Takes{}", type_name(function))
}

fn by_stage(handle: &str) -> bool {
    BY_STAGE.contains(&handle)
}

/// One stage of `enum wc_stage`.
struct Stage {
    /// `WC_STAGE_TESS_EVALUATION`.
    enumerator: String,
    /// `tess_evaluation`.
    snake: String,
    /// `TessEvaluation`.
    pascal: String,
}

/// The stages, in the enumeration's order (its count left out).
fn stages(header: &Header) -> Vec<Stage> {
    header
        .enumerations
        .get(STAGE_ENUMERATION)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let word = value.strip_prefix(STAGE_PREFIX)?;
            Some(Stage { enumerator: value.clone(), snake: word.to_lowercase(), pascal: crate::names::pascal_case_of_upper_snake(word) })
        })
        .collect()
}

/// A shader type as a signature names it: a shader by stage carries `STAGE_MARK` where its stage goes.
fn handle_type(handle: &str) -> String {
    if by_stage(handle) { format!("{}{STAGE_MARK}", type_name(handle)) } else { type_name(handle) }
}

/// The stages a function of a shader by stage is offered for, each with its method.
fn offered_stages(function: &str, header: &Header) -> Result<Vec<(Stage, String)>, String> {
    if let Some((_, taken)) = STAGES_TAKEN.iter().find(|(name, _)| *name == function) {
        return taken
            .iter()
            .map(|(enumerator, method)| {
                stages(header)
                    .into_iter()
                    .find(|stage| stage.enumerator == *enumerator)
                    .map(|stage| (stage, (*method).to_string()))
                    .ok_or_else(|| format!("{function}: no stage {enumerator}"))
            })
            .collect();
    }
    // The handle's word in the function's name followed by the stage: wc_received_prepare to received_vertex_prepare.
    let name = method_name(function);
    let word = BY_STAGE
        .iter()
        .map(|handle| handle.trim_start_matches("wc_"))
        .find(|word| name.starts_with(&format!("{word}_")))
        .ok_or_else(|| format!("{function}: a function of a shader by stage whose name starts with no such shader"))?;
    Ok(stages(header)
        .into_iter()
        .map(|stage| {
            let method = format!("{word}_{}{}", stage.snake, &name[word.len()..]);
            (stage, method)
        })
        .collect())
}

/// Why a function is not offered: one of `NOT_OFFERED`, or a free (a shader type's drop, or inside the copy that reads
/// what the bridge filled).
fn not_offered(name: &str) -> Option<&'static str> {
    NOT_OFFERED.iter().find(|(offered, _)| *offered == name).map(|(_, reason)| *reason).or_else(|| {
        name.ends_with("_free").then_some("a free: a shader type's drop, or inside the copy that reads what the bridge filled")
    })
}

/// A structure holding only bridge memory and its count: read out as a `Vec` of its element.
fn container(name: &str) -> Option<&'static str> {
    match name {
        "wc_bytes" => Some("u8"),
        "wc_bindings" => Some("Binding"),
        "wc_glsl_names" => Some("GlslName"),
        _ => None,
    }
}

/// `wc_flat_type` to `FlatType`.
fn type_name(c: &str) -> String {
    crate::names::pascal_case_of_upper_snake(c.trim_start_matches("struct ").trim_start_matches("wc_"))
}

/// `wc_prepared_variant` to `prepared_variant`.
fn method_name(c: &str) -> String {
    crate::names::escaped(c.strip_prefix("wc_").unwrap_or(c).to_string())
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
    /// A shader of the typed chain the bridge made (the handle's name): taken by the owned value, the field set to
    /// null before the bridge frees the rest.
    Handle(String),
    /// A fixed array of a kind.
    Fixed { element: Box<FieldKind>, length: usize },
    /// A shader by stage for each stage (`stages[WC_STAGES]`): one field of the owned value per stage, each of its
    /// stage's type.
    ByStage { handle: String, length: usize },
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
            (1, handle) if header.handle(handle).is_some() => FieldKind::Handle(handle.trim_start_matches("struct ").to_string()),
            (1, _) => {
                let count = fields.get(at + 1).filter(|next| next.pointers == 0 && (next.name.ends_with("count") || next.name.ends_with("size")));
                let Some(count) = count else { return Err(format!("{name}.{}: a pointer with no count after it", field.name)) };
                at += 1;
                FieldKind::Array { element: Box::new(scalar_kind(base, header)?), count: count.name.clone() }
            }
            _ => return Err(format!("{name}.{}: {} pointers", field.name, field.pointers)),
        };
        if let Some(array) = &field.array {
            let count = length(header, array)?;
            kind = match kind {
                FieldKind::Handle(handle) if by_stage(&handle) => {
                    if count != stages(header).len() {
                        return Err(format!("{name}.{}: shaders by stage in an array of {count}, not one a stage", field.name));
                    }
                    FieldKind::ByStage { handle, length: count }
                }
                kind => FieldKind::Fixed { element: Box::new(kind), length: count },
            };
        }
        out.push((field.name.clone(), kind));
        at += 1;
    }
    Ok(out)
}

/// The handles the kind holds, in the order met (each a generic parameter of the owned structure).
fn handles_of(kind: &FieldKind, header: &Header, out: &mut Vec<String>) {
    match kind {
        FieldKind::Handle(handle) => {
            if !out.contains(handle) {
                out.push(handle.clone());
            }
        }
        FieldKind::Fixed { element, .. } | FieldKind::Array { element, .. } => handles_of(element, header, out),
        FieldKind::ByStage { handle, .. } => {
            for stage in stages(header) {
                let named = format!("{handle}_{}", stage.snake);
                if !out.contains(&named) {
                    out.push(named);
                }
            }
        }
        FieldKind::Structure(name) => {
            for handle in handles(name, header) {
                if !out.contains(&handle) {
                    out.push(handle);
                }
            }
        }
        _ => {}
    }
}

/// The handles a structure holds: its generic parameters.
fn handles(name: &str, header: &Header) -> Vec<String> {
    let mut out = Vec::new();
    for (_, kind) in fields(name, header).unwrap_or_default() {
        handles_of(&kind, header, &mut out);
    }
    out
}

fn generic(name: &str, header: &Header) -> bool {
    !handles(name, header).is_empty()
}

/// `SpirvCommonForm<Self::SpirvCommon>` (with `prefix` "Self::"), the owned type of a structure; with a prefix, a
/// shader by stage carries `STAGE_MARK`.
fn owned_structure(name: &str, header: &Header, prefix: &str) -> String {
    let params: Vec<String> = handles(name, header)
        .iter()
        .map(|handle| if prefix.is_empty() { type_name(handle) } else { format!("{prefix}{}", handle_type(handle)) })
        .collect();
    if params.is_empty() { type_name(name) } else { format!("{}<{}>", type_name(name), params.join(", ")) }
}

fn mirror_type(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror, .. } => (*mirror).to_string(),
        FieldKind::Bool => "u8".to_string(),
        FieldKind::Structure(name) => format!("{}Mirror", type_name(name)),
        FieldKind::Text => "*mut u8".to_string(),
        FieldKind::Array { element, .. } => format!("*mut {}", mirror_type(element)),
        FieldKind::Handle(_) => "*mut c_void".to_string(),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", mirror_type(element)),
        FieldKind::ByStage { length, .. } => format!("[*mut c_void; {length}]"),
    }
}

/// The owned type of a field; handles named with `prefix` ("" in the structure, "Self::" in the trait).
fn owned_type(kind: &FieldKind, header: &Header, prefix: &str) -> String {
    match kind {
        FieldKind::Number { owned, .. } => (*owned).to_string(),
        FieldKind::Bool => "bool".to_string(),
        FieldKind::Structure(name) => match container(name) {
            Some(element) => format!("Vec<{element}>"),
            None => owned_structure(name, header, prefix),
        },
        FieldKind::Text => "Option<Vec<u8>>".to_string(),
        FieldKind::Array { element, .. } => format!("Vec<{}>", owned_type(element, header, prefix)),
        FieldKind::Handle(handle) => format!("Option<{prefix}{}>", type_name(handle)),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", owned_type(element, header, prefix)),
        // Written as one field per stage by `write_structures`.
        FieldKind::ByStage { .. } => "compile_error!(\"shaders by stage are one field a stage\")".to_string(),
    }
}

/// Whether the kind's mirror holds a pointer (so its zero is written, not derived).
fn holds_pointer(kind: &FieldKind, header: &Header) -> bool {
    match kind {
        FieldKind::Number { .. } | FieldKind::Bool => false,
        FieldKind::Structure(name) => fields(name, header).is_ok_and(|fields| fields.iter().any(|(_, kind)| holds_pointer(kind, header))),
        FieldKind::Fixed { element, .. } => holds_pointer(element, header),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Handle(_) | FieldKind::ByStage { .. } => true,
    }
}

/// Whether reading the kind out of a mirror needs no pointer read (so the conversion is safe code).
fn plain(kind: &FieldKind, header: &Header) -> bool {
    match kind {
        FieldKind::Number { .. } | FieldKind::Bool => true,
        FieldKind::Structure(name) => fields(name, header).is_ok_and(|fields| fields.iter().all(|(_, kind)| plain(kind, header))),
        FieldKind::Fixed { element, .. } => plain(element, header),
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Handle(_) | FieldKind::ByStage { .. } => false,
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
        let params: Vec<String> = handles(&name, header).iter().map(|handle| type_name(handle)).collect();
        let generics = if params.is_empty() { String::new() } else { format!("<{}>", params.join(", ")) };
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
            None => owned_structure(&name, header, ""),
        };
        let body = match container(&name) {
            Some(_) => {
                let (field, kind) = &fields[0];
                convert(kind, &format!("self.{field}"))
            }
            None => {
                let parts: Vec<String> = fields
                    .iter()
                    .flat_map(|(field, kind)| {
                        let place = format!("self.{}", crate::names::escaped(field.clone()));
                        match kind {
                            // One field a stage, each its stage's shader, taken from the stage's place.
                            FieldKind::ByStage { handle, .. } => stages(header)
                                .iter()
                                .enumerate()
                                .map(|(at, stage)| {
                                    format!("{}: {}{}::owning(core::mem::replace(&mut {place}[{at}], core::ptr::null_mut()))", stage.snake, type_name(handle), stage.pascal)
                                })
                                .collect::<Vec<_>>(),
                            kind => vec![format!("{}: {}", crate::names::escaped(field.clone()), convert(kind, &place))],
                        }
                    })
                    .collect();
                format!("{owned} {{ {} }}", parts.join(", "))
            }
        };
        if read_out.contains(&name) {
            // A shader by stage held whole (not one a stage) is a type parameter, made by a function the caller gives:
            // its stage is the caller's.
            let staged: Vec<String> = handles(&name, header).into_iter().filter(|handle| by_stage(handle)).collect();
            if !staged.is_empty() && fields.iter().any(|(_, kind)| matches!(kind, FieldKind::Structure(inner) if handles(inner, header).iter().any(|handle| by_stage(handle)))) {
                return Err(format!("{name}: a structure holding shaders by stage inside another"));
            }
            let generics = if staged.is_empty() { String::new() } else { format!("<{}>", staged.iter().map(|handle| type_name(handle)).collect::<Vec<_>>().join(", ")) };
            let makers: String = staged.iter().map(|handle| format!(", {}: fn(*mut c_void) -> Option<{}>", owning_parameter(handle), type_name(handle))).collect();
            let _ = writeln!(mirrors, "impl {owned}Mirror {{\n    /// The values read out (the bridge's arrays and texts copied, its shaders taken).\n    pub(crate) fn owned{generics}(&mut self{makers}) -> {owned_ty} {{\n        {body}\n    }}\n}}\n");
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
            if let FieldKind::ByStage { handle, .. } = kind {
                for stage in stages(header) {
                    let _ = writeln!(interface, "    /// The bridge's `{field}[{}]`: the {} stage, if any.", stage.enumerator, stage.snake.replace('_', " "));
                    let _ = writeln!(interface, "    pub {}: Option<{}{}>,", stage.snake, type_name(handle), stage.pascal);
                }
                continue;
            }
            let _ = write!(interface, "{}", doc(comments, &key.unwrap_or_default(), &format!("The bridge's `{field}`"), "    "));
            let _ = writeln!(interface, "    pub {}: {},", crate::names::escaped(field.clone()), owned_type(kind, header, ""));
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
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Handle(_) => "core::ptr::null_mut()".to_string(),
        FieldKind::Fixed { element, length } => format!("[{}; {length}]", zero(element)),
        FieldKind::ByStage { length, .. } => format!("[core::ptr::null_mut(); {length}]"),
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
        FieldKind::Handle(handle) if by_stage(handle) => format!("{}(core::mem::replace(&mut {place}, core::ptr::null_mut()))", owning_parameter(handle)),
        FieldKind::Handle(handle) => format!("{}::owning(core::mem::replace(&mut {place}, core::ptr::null_mut()))", type_name(handle)),
        FieldKind::Fixed { element, .. } => format!("{place}.each_mut().map(|element| {})", convert_element(element)),
        // Read out one field a stage by `write_structures`.
        FieldKind::ByStage { .. } => "compile_error!(\"shaders by stage are one field a stage\")".to_string(),
        FieldKind::Array { element, count } => {
            let owner = place.rsplit_once('.').map_or("self", |(owner, _)| owner);
            match element.as_ref() {
                FieldKind::Number { mirror, .. } if *mirror != "usize" => format!("array({place}, {owner}.{count})"),
                _ => format!("array({place}, {owner}.{count}).iter_mut().map(|element| {}).collect()", convert_element(element)),
            }
        }
    }
}

/// The parameter of a mirror's `owned` that makes a shader by stage of the caller's stage (`owning_prepared`).
fn owning_parameter(handle: &str) -> String {
    format!("owning_{}", handle.trim_start_matches("wc_"))
}

/// The owned value of `element`, a `&mut` to one mirror element.
fn convert_element(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => "u64::try_from(*element).unwrap_or(u64::MAX)".to_string(),
        FieldKind::Number { .. } => "*element".to_string(),
        FieldKind::Bool => "*element != 0".to_string(),
        FieldKind::Structure(_) => "element.owned()".to_string(),
        FieldKind::Text => "text(*element)".to_string(),
        FieldKind::Handle(handle) => format!("{}::owning(core::mem::replace(element, core::ptr::null_mut()))", type_name(handle)),
        FieldKind::Fixed { .. } | FieldKind::Array { .. } | FieldKind::ByStage { .. } => "compile_error!(\"an array of arrays\")".to_string(),
    }
}

/// The mirror's value of an owned place (plain kinds only).
fn back(kind: &FieldKind, place: &str) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => format!("usize::try_from({place}).unwrap_or(usize::MAX)"),
        FieldKind::Number { .. } => place.to_string(),
        FieldKind::Bool => format!("u8::from({place})"),
        FieldKind::Structure(name) => format!("{}Mirror::from(&{place})", type_name(name)),
        FieldKind::Fixed { element, .. } => match element.as_ref() {
            FieldKind::Structure(name) => format!("{place}.each_ref().map({}Mirror::from)", type_name(name)),
            element => format!("{place}.each_ref().map(|element| {})", back_element(element)),
        },
        FieldKind::Text | FieldKind::Array { .. } | FieldKind::Handle(_) => "core::ptr::null_mut()".to_string(),
        FieldKind::ByStage { length, .. } => format!("[core::ptr::null_mut(); {length}]"),
    }
}

/// The mirror's value of `element`, a `&` to one element of an owned fixed array (plain kinds only).
fn back_element(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Number { mirror: "usize", .. } => "usize::try_from(*element).unwrap_or(usize::MAX)".to_string(),
        FieldKind::Number { .. } => "*element".to_string(),
        FieldKind::Bool => "u8::from(*element)".to_string(),
        FieldKind::Structure(name) => format!("{}Mirror::from(element)", type_name(name)),
        FieldKind::Fixed { .. } | FieldKind::Text | FieldKind::Array { .. } | FieldKind::Handle(_) | FieldKind::ByStage { .. } => {
            "compile_error!(\"no plain mirror\")".to_string()
        }
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
    /// The structure copied out.
    structure: String,
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
            "shader read" => {
                let c_handle = header.handle(base).unwrap_or_default();
                let handle = handle_type(c_handle);
                let staged = if by_stage(c_handle) { format!(", {STAGE_FACT}") } else { String::new() };
                Param {
                    signature: Some(format!("{name}: &Self::{handle}")),
                    before: vec![],
                    argument: format!("{name}.nir.as_ptr()"),
                    c: "*const c_void".to_string(),
                    fact: Some(format!("{name} is a {handle} of ours, of the type the function names{staged}, read by the call alone")),
                }
            }
            "shader taken" => {
                let c_handle = header.handle(base).unwrap_or_default();
                let handle = handle_type(c_handle);
                let staged = if by_stage(c_handle) { format!(", {STAGE_FACT}") } else { String::new() };
                Param {
                    signature: Some(format!("{name}: Self::{handle}")),
                    before: vec![],
                    argument: format!("{name}.into_raw()"),
                    c: "*mut c_void".to_string(),
                    fact: Some(format!("{name} is a {handle} of ours, of the type the function names{staged}, which the bridge takes and gives back itself")),
                }
            }
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
            "the bridge's memory, copied out whole into owned Rust values, then given back once" => {
                let structure = base.trim_start_matches("struct ");
                let free = format!("{}_free", structure);
                let owned = match container(structure) {
                    Some(element) => format!("Vec<{element}>"),
                    None => owned_structure(structure, header, "Self::"),
                };
                out = Some(Out { ty: owned, after: vec![String::new(), free], structure: structure.to_string() });
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
        (handle, 1) if header.handle(handle).is_some() => (" -> *mut c_void".to_string(), Some(format!("Option<Self::{}>", handle_type(header.handle(handle).unwrap_or_default())))),
        // Mesa's options for a Gallium screen: declared, not offered.
        ("struct nir_shader_compiler_options", 1) => (" -> *const c_void".to_string(), None),
        ("bool", 0) => (" -> u8".to_string(), Some("bool".to_string())),
        (enumeration, 0) if enumeration.starts_with("enum ") => (" -> u32".to_string(), Some("u32".to_string())),
        (number_type, 0) => {
            let ty = c_scalar(number_type).ok_or_else(|| format!("{}: returns {number_type}", function.name))?;
            (format!(" -> {ty}"), Some(ty.to_string()))
        }
        _ => {
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
        "//! Mesa's shader compiler as our C bridge offers it (crates/mesa-compiler/implementation/direct/bridge: NIR made from SPIR-V or GLSL, changed for the\n//! routes, read as a flat form), as a safe trait. Every enumeration the bridge writes is a `u32` the\n//! caller converts by a checked `TryFrom`.\n\n",
    );
    let mut mirrors = String::new();
    write_structures(&header, &comments, &mut interface, &mut mirrors)?;
    let mut calls = String::new();
    let mut externs = String::new();
    let mut offered = 0;
    let mut marks = 0;
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
        "/// Mesa's compiler through the bridge: one method a bridge function. A shader is a value of one of the\n/// implementation's types, one a place of the bridge's typed chain (bridge.h), each owning one NIR shader and giving it\n/// back when dropped; a method takes only the type of its place, so a shader goes only through the orders the bridge's\n/// callers take.\npub trait MesaCompiler: Sized {{"
    );
    let mut glue_types = String::new();
    let mut type_bindings = String::new();
    for handle in &header.handles {
        // Only a shader some function takes is given away whole.
        let taken = header.functions.iter().any(|function| {
            !function.name.ends_with("_free") && function.params.iter().any(|param| param.name == "taken" && header.handle(&param.base) == Some(handle.as_str()))
        });
        let into_raw = if taken {
            "\n\n    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.\n    fn into_raw(self) -> *mut c_void {\n        let kept = core::mem::ManuallyDrop::new(self);\n        kept.nir.as_ptr()\n    }\n"
        } else {
            "\n"
        };
        let said = comments.get(&format!("struct {handle}")).cloned().unwrap_or_else(|| format!("The bridge's `struct {handle}`, owned"));
        // A shader by stage is one type a stage; any other one type.
        let kinds: Vec<(String, String, String)> = if by_stage(handle) {
            stages(&header)
                .iter()
                .map(|stage| {
                    let words = stage.snake.replace('_', " ");
                    (format!("{}{}", type_name(handle), stage.pascal), format!("{} Of the {words} stage.", said.trim()), format!(" of the {words} stage"))
                })
                .collect()
        } else {
            vec![(type_name(handle), said.trim().to_string(), String::new())]
        };
        if by_stage(handle) {
            // One type generic over the stage (a zero-sized type of `stage`), one drop, one mark; a type a stage aliases it.
            let ty = type_name(handle);
            let _ = writeln!(
                glue_types,
                "/// A `struct {handle}` the bridge made, of the stage `S` names, given back by `{handle}_free` when dropped. Not `Send`:\n/// no fact of the bridge's is relied on for a shader moved between threads.\n#[derive(Debug)]\npub struct {ty}<S> {{\n    nir: NonNull<c_void>,\n    _stage: PhantomData<fn() -> S>,\n    _thread: PhantomData<*const ()>,\n}}\n\nimpl<S> {ty}<S> {{\n    /// Takes a shader the bridge made; None for null.\n    fn owning(nir: *mut c_void) -> Option<{ty}<S>> {{\n        NonNull::new(nir).map(|nir| {ty} {{ nir, _stage: PhantomData, _thread: PhantomData }})\n    }}{into_raw}}}\n\nimpl<S> Drop for {ty}<S> {{\n    fn drop(&mut self) {{\n        #[expect(unsafe_code, reason = \"bridge.h: {handle}_free gives back a {handle} the bridge made, of any stage\")]\n        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.\n        unsafe {{ {handle}_free(self.nir.as_ptr()) }};\n    }}\n}}\n"
            );
            marks += 1;
            for (alias, said, of_stage) in kinds {
                let stage = alias.trim_start_matches(&ty);
                let _ = write!(interface, "{}", doc(&BTreeMap::new(), "", &said, "    "));
                let _ = writeln!(interface, "    type {alias};\n");
                let _ = writeln!(type_bindings, "    type {alias} = {alias};");
                let _ = writeln!(glue_types, "/// A `struct {handle}`{of_stage}.\npub type {alias} = {ty}<stage::{stage}>;\n");
            }
            continue;
        }
        for (ty, said, of_stage) in kinds {
            let _ = write!(interface, "{}", doc(&BTreeMap::new(), "", &said, "    "));
            let _ = writeln!(interface, "    type {ty};\n");
            let _ = writeln!(type_bindings, "    type {ty} = {ty};");
            let _ = writeln!(
                glue_types,
                "/// A `struct {handle}`{of_stage} the bridge made, given back by `{handle}_free` when dropped. Not `Send`: no fact of the\n/// bridge's is relied on for a shader moved between threads.\n#[derive(Debug)]\npub struct {ty} {{\n    nir: NonNull<c_void>,\n    _thread: PhantomData<*const ()>,\n}}\n\nimpl {ty} {{\n    /// Takes a shader the bridge made; None for null.\n    fn owning(nir: *mut c_void) -> Option<{ty}> {{\n        NonNull::new(nir).map(|nir| {ty} {{ nir, _thread: PhantomData }})\n    }}{into_raw}}}\n\nimpl Drop for {ty} {{\n    fn drop(&mut self) {{\n        #[expect(unsafe_code, reason = \"bridge.h: {handle}_free gives back a {handle} the bridge made\")]\n        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.\n        unsafe {{ {handle}_free(self.nir.as_ptr()) }};\n    }}\n}}\n"
            );
            marks += 1;
        }
    }
    let _ = writeln!(interface, "    /// The compiler, ready (Mesa's type tables taken for the process).\n    fn new() -> Self;\n");
    let mut stage_module = String::from("/// The stages of `enum wc_stage`, each a zero-sized type a shader by stage names, and for each function that takes\n/// only some stages a trait those stages implement.\npub mod stage {\n");
    for stage in stages(&header) {
        let _ = writeln!(stage_module, "    /// `{}`.\n    #[derive(Debug)]\n    pub struct {};\n", stage.enumerator, stage.pascal);
    }
    for (function, taken) in STAGES_TAKEN {
        let marker = taken_marker(function);
        let _ = writeln!(stage_module, "    /// The stages `{function}` takes.\n    pub trait {marker} {{}}\n");
        for (enumerator, _) in *taken {
            let stage = stages(&header).into_iter().find(|stage| stage.enumerator == *enumerator).ok_or_else(|| format!("{function}: no stage {enumerator}"))?;
            let _ = writeln!(stage_module, "    impl {marker} for {} {{}}\n", stage.pascal);
        }
    }
    stage_module.push_str("}\n");
    glue_types.insert_str(0, &stage_module);
    glue_types.insert(stage_module.len(), '\n');
    let mut generic_glue = String::new();
    for function in &header.functions {
        let (c_ret, ret) = return_of(function, &header)?;
        let (params, out_param, written) = parameters(function, &header)?;
        let c_params: Vec<String> = params.iter().map(|p| p.c.clone()).collect();
        let names: Vec<String> = function.params.iter().map(|p| crate::names::escaped(p.name.clone())).collect();
        let declared = !matches!(function.name.as_str(), "wc_finish" | "wc_compiler_options");
        if declared {
            let _ = writeln!(externs, "    fn {}({}){c_ret};", function.name, names.iter().zip(&c_params).map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", "));
        }
        if let Some(reason) = not_offered(&function.name) {
            let _ = writeln!(report, "  NOT OFFERED {}: {reason}", function.name);
            continue;
        }
        let marks_before = marks;
        let mut method_text = String::new();
        let mut glue_text = String::new();
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
        let method = METHOD.to_string();
        let receiver = std::iter::once("&self".to_string()).chain(signature_params.iter().cloned()).collect::<Vec<_>>().join(", ");
        // The header's comment on the function (the callers it mirrors), then the function it calls.
        let opening = format!("{}(", function.name);
        let said = comments.iter().find(|(key, _)| key.contains(&opening)).map(|(_, text)| text.clone());
        if let Some(said) = said {
            let _ = writeln!(method_text, "    /// {}.\n    ///", said.trim().trim_end_matches('.').replace('\n', " "));
        }
        let _ = writeln!(method_text, "    /// Calls the bridge's `{}`{STAGE_DOC}.\n    fn {method}({receiver}){arrow};\n", function.name);
        // The glue.
        let facts: Vec<String> = params.iter().filter_map(|p| p.fact.clone()).collect();
        let reason = format!(
            "bridge.h: {} takes the types declared there{}",
            function.name,
            if facts.is_empty() { "; plain values only".to_string() } else { format!("; {}", facts.join("; ")) }
        );
        let typed_params: Vec<String> = signature_params.iter().map(|p| p.replace("Self::", "")).collect();
        let receiver = std::iter::once("&self".to_string()).chain(typed_params).collect::<Vec<_>>().join(", ");
        let typed_ret = arrow.replace("Self::", "");
        let _ = writeln!(glue_text, "    fn {method}({receiver}){typed_ret} {{");
        for p in &params {
            for line in &p.before {
                let _ = writeln!(glue_text, "        {line}");
            }
        }
        let arguments: Vec<String> = params.iter().map(|p| p.argument.clone()).collect();
        let plain_return = matches!((&ret, &out_param), (Some(_), None)) && function.ret.pointers == 0 && written.is_empty();
        let tail_only = plain_return && c_ret != " -> u8";
        let binding = if c_ret.is_empty() || tail_only { "" } else { "let result = " };
        let _ = writeln!(glue_text, "        #[expect(unsafe_code, reason = \"{}\")]", reason.replace('"', "'"));
        let _ = writeln!(glue_text, "        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.");
        let end = if tail_only { "" } else { ";" };
        let _ = writeln!(glue_text, "        {binding}unsafe {{ {}({}) }}{end}", function.name, arguments.join(", "));
        marks += 1;
        if c_ret == " -> u8" && plain_return {
            let _ = writeln!(glue_text, "        result != 0");
        } else if c_ret == " -> u8" {
            let _ = writeln!(glue_text, "        let result = result != 0;");
        }
        for line in &written {
            let _ = writeln!(glue_text, "        {line}");
        }
        match (&ret, &out_param, function.ret.pointers) {
            (Some(_), None, 1) => {
                let handle = header.handle(&function.ret.base).map(handle_type).unwrap_or_default();
                let _ = writeln!(glue_text, "        {handle}::owning(result)");
            }
            (Some(_), None, _) if !tail_only && !(plain_return && c_ret == " -> u8") => {
                let _ = writeln!(glue_text, "        result");
            }
            (_, Some(out), _) if out.after.len() == 2 && out.after[0].is_empty() => {
                // Copied out whole, then given back once.
                let free = &out.after[1];
                let mirror_name = params.iter().find(|p| p.argument.starts_with("&raw mut ")).map(|p| p.argument.trim_start_matches("&raw mut ").to_string()).unwrap_or_default();
                let makers: Vec<String> = handles(out.structure.as_str(), &header).iter().filter(|handle| by_stage(handle)).map(|handle| format!("{}::owning", handle_type(handle))).collect();
                let _ = writeln!(glue_text, "        let copied = {mirror_name}.owned({});", makers.join(", "));
                let _ = writeln!(glue_text, "        #[expect(unsafe_code, reason = \"bridge.h: {free} gives back what the bridge allocated into {mirror_name} (null pointers are given back as none: free(NULL))\")]");
                let _ = writeln!(glue_text, "        // SAFETY: {mirror_name} is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.");
                let _ = writeln!(glue_text, "        unsafe {{ {free}(&raw mut {mirror_name}) }};");
                marks += 1;
                let tail = match ret.as_deref() {
                    _ if function.name == "wc_glsl_to_nir" => "if result { Ok(copied) } else { Err(copied.log.unwrap_or_default()) }".to_string(),
                    Some("u32") => "if result == 0 { Ok(copied) } else { Err(result) }".to_string(),
                    Some("bool") => "result.then_some(copied)".to_string(),
                    _ => "copied".to_string(),
                };
                let _ = writeln!(glue_text, "        {tail}");
            }
            (_, Some(out), _) => {
                for line in out.after.iter().filter(|line| !line.is_empty()) {
                    let _ = writeln!(glue_text, "        {line}");
                }
            }
            _ => {}
        }
        let _ = writeln!(glue_text, "    }}\n");
        // Once, or once a stage for a function of a shader by stage.
        let bare = |text: &str| text.replace(METHOD, "").replace(STAGE_DOC, "");
        let staged = bare(&method_text).contains(STAGE_MARK) || bare(&glue_text).contains(STAGE_MARK);
        let instances: Vec<(String, String, String)> = if staged {
            offered_stages(&function.name, &header)?
                .into_iter()
                .map(|(stage, method)| (method, stage.pascal, format!(" on a shader of the {} stage", stage.snake.replace('_', " "))))
                .collect()
        } else {
            if STAGES_TAKEN.iter().any(|(name, _)| *name == function.name) {
                return Err(format!("{}: listed in STAGES_TAKEN but takes no shader by stage", function.name));
            }
            vec![(method_name(&function.name), String::new(), String::new())]
        };
        if instances.is_empty() {
            return Err(format!("{}: offered for no stage", function.name));
        }
        let _ = marks_before;
        if staged {
            // One glue function generic over the stage, its marks once; each stage's method calls it.
            let bound = if STAGES_TAKEN.iter().any(|(name, _)| *name == function.name) { format!(": stage::{}", taken_marker(&function.name)) } else { String::new() };
            let generic = |text: &str| text.replace(&format!("{STAGE_MARK}::"), "::<S>::").replace(STAGE_MARK, "<S>");
            let mut lines = glue_text.lines();
            let first = lines.next().unwrap_or_default();
            let glue_name = method_name(&function.name);
            let header_line = generic(&first.replacen(&format!("fn {METHOD}(&self, "), &format!("fn {glue_name}<S{bound}>("), 1));
            let _ = writeln!(generic_glue, "/// The glue of the bridge's `{}`, for a shader of any stage it takes.\n{}", function.name, header_line.trim_start());
            for line in lines {
                let mut line = generic(line.strip_prefix("    ").unwrap_or(line));
                if !bound.is_empty() {
                    // The bound is what keeps a stage the function does not take out: the mark names it.
                    line = line.replace("of the stage its type names (", &format!("of the stage its type names, one that `stage::{}` is implemented for, the stages STAGES_TAKEN lists for it (", taken_marker(&function.name)));
                }
                let _ = writeln!(generic_glue, "{line}");
            }
            let arguments: Vec<String> = signature_params.iter().map(|param| param.split(':').next().unwrap_or_default().trim().to_string()).collect();
            let call_header = first.to_string();
            let call_text = format!("{call_header}\n        {glue_name}({})\n    }}\n\n", arguments.join(", "));
            for (method, stage, stage_doc) in &instances {
                offered += 1;
                let fill = |text: &str| text.replace(METHOD, method).replace(STAGE_DOC, stage_doc).replace(STAGE_MARK, stage);
                interface.push_str(&fill(&method_text));
                calls.push_str(&fill(&call_text));
            }
        } else {
            for (method, stage, stage_doc) in &instances {
                offered += 1;
                let fill = |text: &str| text.replace(METHOD, method).replace(STAGE_DOC, stage_doc).replace(STAGE_MARK, stage);
                interface.push_str(&fill(&method_text));
                calls.push_str(&fill(&glue_text));
            }
        }
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
        "{HEADER}{template}\n// ----- The bridge's structures as it lays them out. -----\n\n{mirrors}// ----- The bridge's functions. -----\n\n#[expect(unsafe_code, reason = \"bridge.h declares these functions; each is reached only through the method that marks its call\")]\nunsafe extern \"C\" {{\n{externs}}}\n\n{glue_types}\n{generic_glue}impl wcontainers_mesa_compiler::MesaCompiler for Bridge {{\n{type_bindings}\n    fn new() -> Bridge {{\n        Bridge::started()\n    }}\n\n{calls}}}\n"
    );
    write(direct.join("implementation.rs"), &direct_text)?;
    write(direct.join("Cargo.toml"), include_str!("../templates/mesa-compiler/direct.toml"))?;
    Ok(report)
}
