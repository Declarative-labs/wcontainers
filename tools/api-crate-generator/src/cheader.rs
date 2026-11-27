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
    /// A bit-field's width (`bool primitive_restart:1`).
    pub bits: Option<String>,
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
    /// The structures declared without a body (`struct wc_spirv_shader;`): each a shader at one place of the bridge's
    /// typed chain, opaque to Rust.
    pub handles: Vec<String>,
    /// Function pointer types (`typedef void (*pipe_draw_func)(...)`), by name.
    pub function_types: BTreeMap<String, CFunction>,
    /// A structure's function pointer members (Gallium's `struct pipe_screen`, `struct pipe_context`), each a function
    /// named by its member.
    pub callbacks: BTreeMap<String, Vec<CFunction>>,
    /// The structures that are unions (a member `union { ... } index;` is recorded as the structure
    /// `pipe_draw_info.index`, a union).
    pub unions: Vec<String>,
}

impl Header {
    /// The handle a type names (`struct wc_made` or `const struct wc_made`), if it is one.
    pub fn handle(&self, base: &str) -> Option<&str> {
        let name = base.trim_start_matches("const ").trim().strip_prefix("struct ")?;
        self.handles.iter().find(|handle| handle.as_str() == name).map(String::as_str)
    }

    /// Whether the bridge declares `<name>_free`, which gives back what it filled into a structure of that name.
    pub fn freed(&self, name: &str) -> bool {
        let free = format!("{name}_free");
        self.functions.iter().any(|function| function.name == free)
    }
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
    // The conditionals' branches kept: a stack of (this branch kept, a branch of this conditional taken).
    let mut branches: Vec<(bool, bool)> = Vec::new();
    for line in out.lines() {
        let trimmed = line.trim();
        let outer = branches.iter().all(|(kept, _)| *kept);
        if let Some(condition) = trimmed.strip_prefix("#ifdef ").or_else(|| trimmed.strip_prefix("#if ")).or_else(|| trimmed.strip_prefix("#ifndef ").map(|_| "1")) {
            let holds = condition_holds(condition.trim());
            branches.push((holds, holds));
            continue;
        }
        if let Some(condition) = trimmed.strip_prefix("#elif ") {
            if let Some((kept, taken)) = branches.last_mut() {
                let holds = !*taken && condition_holds(condition.trim());
                *kept = holds;
                *taken |= holds;
            }
            continue;
        }
        if trimmed.starts_with("#else") {
            if let Some((kept, taken)) = branches.last_mut() {
                *kept = !*taken;
                *taken = true;
            }
            continue;
        }
        if trimmed.starts_with("#endif") {
            branches.pop();
            continue;
        }
        if !outer {
            continue;
        }
        if let Some(define) = trimmed.strip_prefix("#define") {
            let mut words = define.split_whitespace();
            if let (Some(name), Some(value)) = (words.next(), words.next()) {
                defines.insert(name.to_string(), value.to_string());
            }
            continue;
        }
        // `extern "C" {` and its brace are inside `#ifdef __cplusplus`, which is dropped above.
        if trimmed.starts_with('#') {
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// Whether a conditional's condition holds where the headers are built (x86-64 and ARM64 Linux, by GCC or Clang, as C):
/// `__GNUC__`, little-endian and POSIX hold; C++, big-endian and Windows do not; any other condition is taken as
/// holding, so that its first branch is read (the headers read here have none).
fn condition_holds(condition: &str) -> bool {
    let name = condition.trim_start_matches("defined").trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace());
    !matches!(name, "__cplusplus" | "UTIL_ARCH_BIG_ENDIAN" | "_WIN32" | "_MSC_VER" | "DETECT_OS_WINDOWS" | "0")
}

/// `void (*name)(struct pipe_context *, unsigned)` to a function named by the pointer's name.
fn function_pointer(text: &str) -> Option<CFunction> {
    let star = text.find("(*")?;
    let close = star + text[star..].find(')')?;
    let name = text[star + 2..close].trim().to_string();
    let open = close + text[close..].find('(')?;
    let end = text.rfind(')')?;
    let mut ret = declaration(&format!("{} return", text[..star].trim()));
    ret.name = "return".to_string();
    let params = split_parameters(&text[open + 1..end])
        .iter()
        .map(|p| declaration(p))
        .filter(|p| !(p.base == "void" && p.pointers == 0 && p.name.is_empty()))
        .collect();
    Some(CFunction { name, ret, params })
}

/// A parameter list split at its top-level commas.
fn split_parameters(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// `const struct nir_shader *const sources[WC_STAGES]` to its parts.
fn declaration(text: &str) -> CParam {
    let text = text.trim();
    let (text, bits) = match text.rsplit_once(':') {
        Some((before, width)) if !width.trim().is_empty() && width.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !before.contains('(') => (before.trim(), Some(width.trim().to_string())),
        _ => (text, None),
    };
    let (text, array) = match text.split_once('[') {
        Some((before, after)) => (before.trim(), Some(after.trim_end_matches(']').trim().to_string())),
        None => (text, None),
    };
    let pointers = text.matches('*').count();
    let words: Vec<&str> = text.split(|c: char| c.is_whitespace() || c == '*').filter(|w| !w.is_empty()).collect();
    // An unnamed parameter (`struct pipe_context *`, `unsigned`, `const struct pipe_box *`): its last word is the
    // type's own: a basic type's name, or a tag after `struct`, `enum` or `union`.
    let unnamed = |name: &str, base: &[&str]| {
        matches!(name, "void" | "int" | "unsigned" | "char" | "bool" | "float" | "double" | "long" | "short")
            || matches!(base.last(), Some(&("struct" | "enum" | "union")))
    };
    let (name, base) = match words.split_last() {
        Some((name, base)) if !base.is_empty() && !unnamed(name, base) => (name.to_string(), base.join(" ")),
        _ => (String::new(), words.join(" ")),
    };
    // A `const` after the stars (`*const`) is the pointer's own and says nothing of what it points at.
    let base = base.trim_end_matches(" const").to_string();
    CParam { name, base, pointers, array, bits }
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
                if let Some(name) = statement.trim().strip_prefix("struct ")
                    && name.starts_with("wc_")
                    && !name.contains(char::is_whitespace)
                {
                    header.handles.push(name.to_string());
                    continue;
                }
                if let Some(typedef) = statement.trim().strip_prefix("typedef ") {
                    if let Some(function) = function_pointer(typedef) {
                        header.function_types.insert(function.name.clone(), function);
                    }
                    continue;
                }
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
            let close = matching_brace(rest, open);
            let body = &rest[open + 1..close];
            // A definition ends at the semicolon after its body; a function's body (a header's `static inline`) at
            // its brace.
            let defines_a_type = ["struct", "union", "enum", "typedef"].iter().any(|word| head.starts_with(word));
            rest = if defines_a_type { rest[close..].split_once(';').map_or("", |(_, after)| after) } else { rest.get(close + 1..).unwrap_or("") };
            if let Some(name) = head.strip_prefix("enum ") {
                let values = body.split(',').map(|v| v.split('=').next().unwrap_or_default().trim().to_string()).filter(|v| !v.is_empty()).collect();
                header.enumerations.insert(name.trim().to_string(), values);
            } else if let Some((union, name)) = head.strip_prefix("struct ").map(|name| (false, name)).or_else(|| head.strip_prefix("union ").map(|name| (true, name))) {
                if union {
                    header.unions.push(name.trim().to_string());
                }
                let fields = members(&mut header, name.trim(), body);
                header.structures.insert(name.trim().to_string(), fields);
            }
        }
    }
    header
}

/// The place of the brace closing the one at `open`.
fn matching_brace(text: &str, open: usize) -> usize {
    let mut depth = 0_usize;
    for (at, character) in text[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + at;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// A body's members split at its top-level semicolons.
fn split_members(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, character) in body.char_indices() {
        match character {
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            ';' if depth == 0 => {
                out.push(body[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    out.push(body[start..].trim());
    out.into_iter().filter(|member| !member.is_empty()).collect()
}

/// A structure's (or union's) fields, its function pointer members recorded as callbacks, and each nested `struct {
/// ... } name` or `union { ... } name` recorded as the structure `outer.name` (an anonymous one's fields are the outer
/// structure's own).
fn members(header: &mut Header, name: &str, body: &str) -> Vec<CParam> {
    let mut fields = Vec::new();
    let mut callbacks = Vec::new();
    for member in split_members(body) {
        if let Some(open) = member.find('{') {
            let close = matching_brace(member, open);
            let kind = member[..open].trim();
            let union = kind.starts_with("union");
            let inner_name = member[close + 1..].trim();
            if inner_name.is_empty() {
                let inner = members(header, name, &member[open + 1..close]);
                fields.extend(inner);
                continue;
            }
            // One body may declare several members (`struct { ... } dst, src;`): each gets its own copy.
            for declarator in inner_name.split(',') {
                let declared = declaration(&format!("x {}", declarator.trim()));
                let nested = format!("{name}.{}", declared.name);
                let inner = members(header, &nested, &member[open + 1..close]);
                header.structures.insert(nested.clone(), inner);
                if union {
                    header.unions.push(nested.clone());
                }
                fields.push(CParam { base: format!("{} {nested}", if union { "union" } else { "struct" }), ..declared });
            }
            continue;
        }
        if let Some(function) = function_pointer(member) {
            callbacks.push(function);
            continue;
        }
        let field = declaration(member);
        if field.pointers == 0
            && let Some(typed) = header.function_types.get(field.base.trim())
        {
            callbacks.push(CFunction { name: field.name.clone(), ..typed.clone() });
            continue;
        }
        fields.push(field);
    }
    if !callbacks.is_empty() {
        header.callbacks.entry(name.to_string()).or_default().extend(callbacks);
    }
    fields
}

/// One row of the Mesa compiler's mapping rule, for the crate writer.
pub fn row_of(function: &CFunction, param: &CParam, header: &Header) -> Option<&'static str> {
    row(function, param, header)
}

/// One row of the Mesa compiler's mapping rule.
fn row(function: &CFunction, param: &CParam, header: &Header) -> Option<&'static str> {
    let base = param.base.trim_start_matches("const ").trim();
    let is_const = param.base.starts_with("const ");
    if let Some(handle) = header.handle(base) {
        return Some(match (param.pointers, param.array.is_some(), is_const) {
            (1, false, false) if param.name == "return" => "shader made: an owned shader of its place, given back by its free on drop, or None",
            (1, false, true) if param.name != "return" => "shader read: &Shader of its place",
            (1, false, false) if function.name == format!("{handle}_free") => "shader given back: the shader type's drop",
            (1, false, false) if param.name == "taken" => "shader taken: the shader of its place by value, given back by the bridge",
            _ => return None,
        });
    }
    Some(match (base, param.pointers, param.array.is_some(), is_const) {
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
        (structure, 1, false, false) if header.freed(structure.trim_start_matches("struct ")) && function.name.ends_with("_free") => {
            "the bridge's memory given back: inside the copy that reads it"
        }
        (structure, 1, false, false) if header.freed(structure.trim_start_matches("struct ")) => {
            "the bridge's memory, copied out whole into owned Rust values, then given back once: Vec or an owned structure"
        }
        (structure, 1, false, false) if header.structures.contains_key(structure.trim_start_matches("struct ")) => {
            "plain data written during the call, as many as the capacity beside it: &mut [Mirror]"
        }
        _ => return None,
    })
}

/// Gallium's census (AC8): every function pointer member of the structures that hold them, with its parameters'
/// types, and how often each type is taken across them (the driver's objects among them are the kinds a stage-typed
/// trait gives each its own type).
pub fn gallium_census(header: &Header) -> String {
    let mut report = String::new();
    let mut types: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;
    for (structure, callbacks) in &header.callbacks {
        let _ = writeln!(report, "STRUCTURE {structure}: {} function pointers", callbacks.len());
        total += callbacks.len();
        for function in callbacks {
            let shown: Vec<String> = std::iter::once(&function.ret)
                .chain(&function.params)
                .map(|param| {
                    let ty = format!("{}{}{}", param.base, "*".repeat(param.pointers), param.array.as_ref().map(|a| format!("[{a}]")).unwrap_or_default());
                    *types.entry(ty.clone()).or_default() += 1;
                    format!("{}:{ty}", param.name)
                })
                .collect();
            let _ = writeln!(report, "  {}.{} {}", structure, function.name, shown.join(" "));
        }
    }
    let _ = writeln!(report, "FUNCTION POINTERS {total}");
    let mut by_count: Vec<(&String, &usize)> = types.iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let _ = writeln!(report, "TYPES {}", by_count.len());
    for (ty, count) in by_count {
        let _ = writeln!(report, "  {count} {ty}");
    }
    report
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
