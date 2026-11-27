//! The generator's front end for a C header (AC11's Mesa compiler bridge, and AC8's Gallium headers next): reads the
//! subset of C those headers are written in (functions, structures, enumerations, numeric defines) and classifies
//! every parameter and return of every function by one row of the Mesa compiler's mapping rule, or lists it unmapped.
//!
//!   api-crate-generator c-header <header>...

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One parameter (or a function's return, named `return`) as C writes it.
#[derive(Clone, Debug, Default)]
pub struct CParam {
    pub name: String,
    /// The type with `const`, `struct` and `enum` kept, stars and array length removed (`const struct nir_shader`).
    pub base: String,
    pub pointers: usize,
    /// `[N]` after the name: the number or the define's name.
    pub array: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CFunction {
    pub name: String,
    pub ret: CParam,
    pub params: Vec<CParam>,
}

#[derive(Debug, Default)]
pub struct Header {
    pub functions: Vec<CFunction>,
    pub structures: BTreeMap<String, Vec<CParam>>,
    pub enumerations: BTreeMap<String, Vec<String>>,
    pub defines: BTreeMap<String, String>,
}

/// The text without comments, preprocessor lines (the defines kept apart) and `extern "C"` braces.
fn clean(text: &str, defines: &mut BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, after)| after);
        } else if let Some(after) = rest.strip_prefix("//") {
            rest = after.split_once('\n').map_or("", |(_, after)| after);
        } else {
            let mut characters = rest.chars();
            if let Some(character) = characters.next() {
                out.push(character);
            }
            rest = characters.as_str();
        }
    }
    let mut kept = String::new();
    for line in out.lines() {
        let trimmed = line.trim();
        if let Some(define) = trimmed.strip_prefix("#define") {
            let mut words = define.split_whitespace();
            if let (Some(name), Some(value)) = (words.next(), words.next()) {
                defines.insert(name.to_string(), value.to_string());
            }
            continue;
        }
        if trimmed.starts_with('#') || trimmed == "extern \"C\" {" || trimmed == "}" {
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// `const struct nir_shader *const sources[WC_STAGES]` to its parts.
fn declaration(text: &str) -> CParam {
    let text = text.trim();
    let (text, array) = match text.split_once('[') {
        Some((before, after)) => (before.trim(), Some(after.trim_end_matches(']').trim().to_string())),
        None => (text, None),
    };
    let pointers = text.matches('*').count();
    let words: Vec<&str> = text.split(|c: char| c.is_whitespace() || c == '*').filter(|w| !w.is_empty()).collect();
    let (name, base) = match words.split_last() {
        Some((name, base)) if !base.is_empty() && !matches!(*name, "void") => (name.to_string(), base.join(" ")),
        _ => (String::new(), words.join(" ")),
    };
    // A `const` after the stars (`*const`) is the pointer's own and says nothing of what it points at.
    let base = base.trim_end_matches(" const").to_string();
    CParam { name, base, pointers, array }
}

pub fn read(texts: &[String]) -> Header {
    let mut header = Header::default();
    for text in texts {
        let cleaned = clean(text, &mut header.defines);
        let mut rest = cleaned.as_str();
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            let end = rest.find(';').unwrap_or(rest.len());
            let open = rest.find('{').filter(|&open| open < end);
            let Some(open) = open else {
                let statement = &rest[..end];
                rest = rest.get(end + 1..).unwrap_or("");
                if let (Some(paren), Some(close)) = (statement.find('('), statement.rfind(')')) {
                    let mut ret = declaration(&statement[..paren]);
                    let name = std::mem::take(&mut ret.name);
                    ret.name = "return".to_string();
                    let params = statement[paren + 1..close]
                        .split(',')
                        .map(declaration)
                        .filter(|p| !(p.base == "void" && p.pointers == 0 && p.name.is_empty()))
                        .collect();
                    header.functions.push(CFunction { name, ret, params });
                }
                continue;
            };
            let head = rest[..open].trim().to_string();
            let close = rest[open..].find('}').map_or(rest.len(), |close| open + close);
            let body = &rest[open + 1..close];
            rest = rest[close..].split_once(';').map_or("", |(_, after)| after);
            if let Some(name) = head.strip_prefix("enum ") {
                let values = body.split(',').map(|v| v.split('=').next().unwrap_or_default().trim().to_string()).filter(|v| !v.is_empty()).collect();
                header.enumerations.insert(name.trim().to_string(), values);
            } else if let Some(name) = head.strip_prefix("struct ") {
                let fields = body.split(';').map(str::trim).filter(|f| !f.is_empty()).map(declaration).collect();
                header.structures.insert(name.trim().to_string(), fields);
            }
        }
    }
    header
}

/// One row of the Mesa compiler's mapping rule.
fn row(function: &CFunction, param: &CParam, header: &Header) -> Option<&'static str> {
    let base = param.base.trim_start_matches("const ").trim();
    let is_const = param.base.starts_with("const ");
    Some(match (base, param.pointers, param.array.is_some(), is_const) {
        ("struct nir_shader", 1, false, true) if param.name == "return" => return None,
        ("struct nir_shader", 1, false, false) if param.name == "return" => "shader made: an owned shader, freed by wc_nir_free on drop, or None",
        ("struct nir_shader", 1, false, true) => "shader read: &Shader",
        ("struct nir_shader", 1, false, false) if function.name == "wc_nir_free" => "shader given back: the shader type's drop",
        ("struct nir_shader", 1, false, false) if function.name == "wc_nir_to_spirv" => "shader read and changed: &mut Shader",
        ("struct nir_shader", 1, false, false) => "shader changed: &mut Shader",
        ("struct nir_shader", 1, true, false) => "shaders made into the caller's array: [Option<Shader>; N]",
        ("struct nir_shader_compiler_options", 1, false, true) => "the compiler's options for a Gallium screen: AC8's Gallium trait",
        ("void" | "", 0, false, _) => "nothing",
        ("bool" | "uint32_t" | "uint64_t" | "int32_t" | "unsigned" | "float" | "size_t" | "uint8_t", 0, false, _) => "number or bool by value",
        (enumeration, 0, false, _) if enumeration.starts_with("enum ") && header.enumerations.contains_key(enumeration.trim_start_matches("enum ")) => {
            if param.name == "return" { "a declared value returned: read as a number and checked" } else { "a declared value: a Rust enumeration" }
        }
        (structure, 0, false, _) if structure.starts_with("struct ") && header.structures.contains_key(structure.trim_start_matches("struct ")) => {
            "plain data by value: a #[repr(C)] mirror"
        }
        ("char", 1, false, true) => "zero-ended text read during the call: &CStr",
        ("char", 1, true, true) => "texts by stage, each zero-ended or none: [Option<&CStr>; N]",
        ("uint32_t", 1, false, true) => "words read during the call, with the count beside them: &[u32]",
        ("uint32_t", 1, false, false) => "words written during the call, as many as the count beside them: &mut [u32]",
        (structure, 1, false, true) if header.structures.contains_key(structure.trim_start_matches("struct ")) && param.array.is_none() => {
            "plain data read during the call, with the count beside it: &[Mirror]"
        }
        (structure, 0, true, true) if header.structures.contains_key(structure.trim_start_matches("struct ")) => "plain data read during the call: [Mirror; N]",
        ("struct wc_bytes" | "struct wc_words" | "struct wc_bindings" | "struct wc_glsl_program" | "struct wc_flat", 1, false, false)
            if function.name.ends_with("_free") =>
        {
            "the bridge's memory given back: inside the copy that reads it"
        }
        ("struct wc_bytes" | "struct wc_words" | "struct wc_bindings" | "struct wc_glsl_program" | "struct wc_flat", 1, false, false) => {
            "the bridge's memory, copied out whole into owned Rust values, then given back once: Vec or an owned structure"
        }
        (structure, 1, false, false) if header.structures.contains_key(structure.trim_start_matches("struct ")) => {
            "plain data written during the call, as many as the capacity beside it: &mut [Mirror]"
        }
        _ => return None,
    })
}

pub fn census(header: &Header) -> String {
    let mut report = String::new();
    let _ = writeln!(
        report,
        "C HEADER: functions {}, structures {}, enumerations {}, defines {}",
        header.functions.len(),
        header.structures.len(),
        header.enumerations.len(),
        header.defines.len()
    );
    let mut rows: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unmapped = Vec::new();
    for function in &header.functions {
        let mut line = format!("FUNCTION {}:", function.name);
        for param in std::iter::once(&function.ret).chain(&function.params) {
            match row(function, param, header) {
                Some(row) => {
                    *rows.entry(row).or_default() += 1;
                    let _ = write!(line, " {}={}", param.name, row.split(':').next().unwrap_or(row));
                }
                None => {
                    unmapped.push(format!("{}({}): {} pointers {} array {:?}", function.name, param.name, param.base, param.pointers, param.array));
                    let _ = write!(line, " {}=UNMAPPED", param.name);
                }
            }
        }
        let _ = writeln!(report, "{line}");
    }
    let _ = writeln!(report, "ROWS");
    for (row, count) in &rows {
        let _ = writeln!(report, "  {count} {row}");
    }
    let _ = writeln!(report, "UNMAPPED {}", unmapped.len());
    for line in &unmapped {
        let _ = writeln!(report, "  {line}");
    }
    for (name, fields) in &header.structures {
        let _ = writeln!(report, "STRUCTURE {name}: {} fields", fields.len());
    }
    report
}
