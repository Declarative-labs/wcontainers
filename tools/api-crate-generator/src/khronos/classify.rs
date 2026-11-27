//! Default deny for a Khronos API: every parameter and return of every command a set carries gets one kind, by its
//! C type and the registry's attributes (`len`, `class`) or by an exception line of src/khronos/sets.rs. A
//! parameter of no kind fails the generator.

use std::collections::BTreeMap;

use super::registry::{Command, Param, Registry};
use super::sets::{Form, Set};

/// A plain C value and the Rust type the trait gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scalar {
    pub rust: &'static str,
    pub c: &'static str,
}

/// The plain value types of both registries; anything else is a handle, a pointer or unknown.
pub fn scalar(ptype: &str) -> Option<Scalar> {
    let same = |t: &'static str| Some(Scalar { rust: t, c: t });
    match ptype {
        "GLenum" | "GLbitfield" | "GLuint" | "EGLenum" => same("u32"),
        "GLint" | "GLsizei" | "GLfixed" | "GLclampx" | "EGLint" => same("i32"),
        "GLbyte" => same("i8"),
        "GLubyte" | "GLchar" | "char" => same("u8"),
        "GLshort" => same("i16"),
        "GLushort" | "GLhalf" => same("u16"),
        "GLfloat" | "GLclampf" => same("f32"),
        "GLdouble" | "GLclampd" => same("f64"),
        "GLint64" | "GLint64EXT" => same("i64"),
        "GLuint64" | "GLuint64EXT" | "EGLTime" => same("u64"),
        "GLboolean" => Some(Scalar { rust: "bool", c: "u8" }),
        "EGLBoolean" => Some(Scalar { rust: "bool", c: "u32" }),
        // Pointer-sized offsets and sizes: i64 in the trait (the same on every target), checked into isize.
        "GLintptr" | "GLsizeiptr" | "GLintptrARB" | "GLsizeiptrARB" => Some(Scalar { rust: "i64", c: "isize" }),
        _ => None,
    }
}

/// The handle kinds: opaque pointers the implementation makes, carried as values of the glue's own types.
fn handle(ptype: &str) -> Option<&'static str> {
    match ptype {
        "GLsync" => Some("Fence"),
        "EGLDisplay" => Some("Display"),
        "EGLConfig" => Some("Config"),
        "EGLContext" => Some("Context"),
        "EGLSurface" => Some("Surface"),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub enum Part {
    /// A value passed as it is (a bool as C's 0 or 1, an offset checked into isize).
    Value(Scalar),
    /// A handle of the glue's kind; `optional` when the caller may leave it out.
    Handle { kind: &'static str, optional: bool },
    /// Left out of the trait: the count of the slice at `slice`, its length over `multiplier`.
    CountOf { slice: usize, multiplier: usize },
    /// Values the call reads: `count` names the parameter that tells C how many.
    SliceIn { elem: Scalar },
    /// Values the call writes, at most as many as `count` tells C.
    SliceOut { elem: Scalar },
    /// One value the call writes; returned.
    SingleOut(Scalar),
    /// A zero-ended text the call reads.
    TextIn,
    /// Texts the call reads: with their lengths in the parameter `lengths`, or zero-ended.
    TextsIn { lengths: Option<usize> },
    /// Left out: the lengths of the texts at `texts`.
    LengthsOf { texts: usize },
    /// Bytes the call reads, as many as `count` says.
    BytesIn,
    GuardedOut { elem: Scalar, by: usize, guard: usize, refused: Vec<String> },
    BoundedIn { elem: Scalar, most: usize },
    PixelUnpack { width: usize, height: usize, depth: Option<usize>, format: usize, type_: usize },
    PixelPack { width: usize, height: usize, format: usize, type_: usize },
    CompressedUnpack,
    /// Left out: the size of the compressed bytes at `data`.
    SizeOf { data: usize },
    ArrayBufferOffset,
    ElementBufferOffset,
    /// Bytes that may be absent; `size` names the size parameter, which the trait keeps.
    OptionalBytes { size: usize },
    AttributeList,
    HandlesOut { kind: &'static str, count: usize, written: usize },
    /// Left out: how many handles the call wrote, returned.
    WrittenOf,
    NullOnly,
}

#[derive(Clone, Debug)]
pub enum Return {
    Unit,
    Value(Scalar),
    /// EGL's EGLBoolean: success, which the method returns as `bool`, or as `Option` of what the call wrote.
    Success,
    Handle(&'static str),
    /// A zero-ended text the implementation keeps (`glGetString`): copied out.
    Text,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub command: Command,
    pub parts: Vec<Part>,
    pub ret: Return,
    /// The extensions that carry the command when it is not in the core of the set's version.
    pub extensions: Vec<String>,
    /// Checked before the call: each enabled vertex attribute has a buffer.
    pub draw: bool,
}

pub struct Classified {
    pub methods: Vec<Method>,
    /// The count of parts per kind and of returns per kind, for the report.
    pub counts: BTreeMap<String, usize>,
}

fn position(command: &Command, name: &str) -> Result<usize, String> {
    command.params.iter().position(|p| p.name == name).ok_or_else(|| format!("{}: no parameter {name}", command.name))
}

/// `count`, `count*4`, `n`: the parameter named and the multiplier.
fn len_of(command: &Command, len: &str) -> Option<(usize, usize)> {
    let (name, multiplier) = match len.split_once('*') {
        Some((name, by)) => (name, by.parse().ok()?),
        None => (len, 1),
    };
    let at = command.params.iter().position(|p| p.name == name)?;
    Some((at, multiplier))
}

fn form_of(set: &Set, command: &str, param: &str) -> Option<Form> {
    set.forms.iter().find(|(c, p, _)| *c == command && *p == param).map(|(_, _, form)| *form)
}

fn classify_param(set: &Set, command: &Command, index: usize, parts: &mut [Option<Part>]) -> Result<(), String> {
    let param: &Param = &command.params[index];
    let unmapped = || format!("UNMAPPED {}({}): {} pointers {} const {} len {:?}", command.name, param.name, param.ptype, param.pointers, param.is_const, param.len);
    if let Some(form) = form_of(set, &command.name, &param.name) {
        let part = match form {
            Form::GuardedOut { by, guard, refused } => {
                let elem = scalar(&param.ptype).ok_or_else(unmapped)?;
                Part::GuardedOut { elem, by: position(command, by)?, guard, refused: refused.iter().map(|r| r.to_string()).collect() }
            }
            Form::BoundedIn { most } => Part::BoundedIn { elem: scalar(&param.ptype).ok_or_else(unmapped)?, most },
            Form::PixelUnpack { depth } => Part::PixelUnpack {
                width: position(command, "width")?,
                height: position(command, "height")?,
                depth: if depth { Some(position(command, "depth")?) } else { None },
                format: position(command, "format")?,
                type_: position(command, "type")?,
            },
            Form::PixelPack => Part::PixelPack {
                width: position(command, "width")?,
                height: position(command, "height")?,
                format: position(command, "format")?,
                type_: position(command, "type")?,
            },
            Form::CompressedUnpack => {
                let size = position(command, param.len.as_deref().ok_or_else(unmapped)?)?;
                parts[size] = Some(Part::SizeOf { data: index });
                Part::CompressedUnpack
            }
            Form::ArrayBufferOffset => Part::ArrayBufferOffset,
            Form::ElementBufferOffset => Part::ElementBufferOffset,
            Form::OptionalBytes => Part::OptionalBytes { size: position(command, param.len.as_deref().ok_or_else(unmapped)?)? },
            Form::AttributeList => Part::AttributeList,
            Form::SingleOut => Part::SingleOut(scalar(&param.ptype).ok_or_else(unmapped)?),
            Form::HandlesOut { count, written } => {
                let kind = handle(&param.ptype).ok_or_else(unmapped)?;
                let (count, written) = (position(command, count)?, position(command, written)?);
                parts[count] = Some(Part::CountOf { slice: index, multiplier: 1 });
                parts[written] = Some(Part::WrittenOf);
                Part::HandlesOut { kind, count, written }
            }
            Form::OptionalHandle => Part::Handle { kind: handle(&param.ptype).ok_or_else(unmapped)?, optional: true },
            Form::NullOnly => Part::NullOnly,
        };
        parts[index] = Some(part);
        return Ok(());
    }
    if parts[index].is_some() {
        return Ok(());
    }
    let part = match (param.pointers, param.ptype.as_str()) {
        (0, ptype) => {
            if let Some(kind) = handle(ptype) {
                Part::Handle { kind, optional: false }
            } else {
                Part::Value(scalar(ptype).ok_or_else(unmapped)?)
            }
        }
        // A zero-ended text with no length: `const GLchar *name`.
        (1, "GLchar" | "char") if param.is_const && matches!(param.len.as_deref(), None | Some("COMPSIZE()")) => Part::TextIn,
        // Texts: `const GLchar *const*string` of `count`, with `const GLint *length` of the same count, or alone.
        (2, "GLchar") if param.is_const && param.inner_const => {
            let (count, _) = len_of(command, param.len.as_deref().ok_or_else(unmapped)?).ok_or_else(unmapped)?;
            let lengths = command.params.iter().position(|p| p.name == "length" && p.is_const && p.pointers == 1 && p.len == param.len);
            if let Some(lengths) = lengths {
                parts[lengths] = Some(Part::LengthsOf { texts: index });
            }
            parts[count] = Some(Part::CountOf { slice: index, multiplier: 1 });
            Part::TextsIn { lengths }
        }
        // Bytes read with a size: `const void *data` of `size`.
        (1, "void") if param.is_const => {
            let (count, multiplier) = len_of(command, param.len.as_deref().ok_or_else(unmapped)?).ok_or_else(unmapped)?;
            if multiplier != 1 {
                return Err(unmapped());
            }
            parts[count] = Some(Part::CountOf { slice: index, multiplier: 1 });
            Part::BytesIn
        }
        (1, ptype) => {
            let elem = scalar(ptype).ok_or_else(unmapped)?;
            let len = param.len.as_deref().ok_or_else(unmapped)?;
            if len == "1" && !param.is_const {
                Part::SingleOut(elem)
            } else {
                let (count, multiplier) = len_of(command, len).ok_or_else(unmapped)?;
                parts[count] = Some(Part::CountOf { slice: index, multiplier });
                if param.is_const { Part::SliceIn { elem } } else { Part::SliceOut { elem } }
            }
        }
        _ => return Err(unmapped()),
    };
    parts[index] = Some(part);
    Ok(())
}

fn classify_return(command: &Command) -> Result<Return, String> {
    let ret = &command.ret;
    let unmapped = || format!("UNMAPPED {}(return): {} pointers {}", command.name, ret.ptype, ret.pointers);
    Ok(match (ret.pointers, ret.ptype.as_str()) {
        (0, "void") | (0, "") => Return::Unit,
        (0, "EGLBoolean") => Return::Success,
        (0, ptype) => match handle(ptype) {
            Some(kind) => Return::Handle(kind),
            None => Return::Value(scalar(ptype).ok_or_else(unmapped)?),
        },
        (1, "GLubyte" | "char") if ret.is_const => Return::Text,
        _ => return Err(unmapped()),
    })
}

/// Classifies every command of the set, in the set's order; every failure is listed, then the generator fails.
pub fn classify(registry: &Registry, set: &Set) -> Result<Classified, String> {
    let mut methods = Vec::new();
    let mut failures = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in set.commands {
        let Some(command) = registry.commands.get(*name) else {
            failures.push(format!("{name}: not in the registry"));
            continue;
        };
        let extensions = if registry.in_core(name, set.api, set.core_up_to) { Vec::new() } else { registry.extensions_of(name, set.api) };
        if extensions.is_empty() && !registry.in_core(name, set.api, set.core_up_to) {
            failures.push(format!("{name}: neither in {} {} nor in an extension of it", set.api, set.core_up_to));
            continue;
        }
        let mut parts: Vec<Option<Part>> = vec![None; command.params.len()];
        let mut failed = false;
        // Forms and arrays first mark the parameters they take over (counts, lengths, sizes).
        for index in 0..command.params.len() {
            if let Err(error) = classify_param(set, command, index, &mut parts) {
                failures.push(error);
                failed = true;
            }
        }
        // A count left of an array seen later was classified as a value first: the array's pass marks it again.
        for index in 0..command.params.len() {
            if matches!(parts[index], Some(Part::SliceIn { .. } | Part::SliceOut { .. } | Part::TextsIn { .. } | Part::BytesIn)) {
                let _ = classify_param(set, command, index, &mut parts);
            }
        }
        let ret = match classify_return(command) {
            Ok(ret) => ret,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        if failed {
            continue;
        }
        let Some(parts) = parts.into_iter().collect::<Option<Vec<Part>>>() else {
            failures.push(format!("{name}: a parameter of no kind"));
            continue;
        };
        for part in &parts {
            let kind = format!("{part:?}");
            let kind = kind.split([' ', '(', '{']).next().unwrap_or_default().to_string();
            *counts.entry(format!("part {kind}")).or_default() += 1;
        }
        let ret_kind = format!("{ret:?}");
        *counts.entry(format!("return {}", ret_kind.split(['(', ' ']).next().unwrap_or_default())).or_default() += 1;
        methods.push(Method { command: command.clone(), parts, ret, extensions, draw: set.draws.contains(name) });
    }
    // Every exception line must name a command and a parameter of the set.
    for (command, param, _) in set.forms {
        if !set.commands.contains(command) || registry.commands.get(*command).is_none_or(|c| !c.params.iter().any(|p| p.name == *param)) {
            failures.push(format!("stale exception line: {command}({param})"));
        }
    }
    if failures.is_empty() { Ok(Classified { methods, counts }) } else { Err(failures.join("\n")) }
}
