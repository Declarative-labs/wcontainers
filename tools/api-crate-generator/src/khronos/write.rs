//! Writes a Khronos API's crates from its classified commands: `crates/<api>/interface.rs` with its values, the
//! `implementation/direct` crate (the calls into the C library through the entry points EGL gives, every check of
//! src/khronos/classify.rs's kinds before them), and `conformance/` (a suite generic over the traits).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::classify::{Classified, Method, Part, Return, Scalar};
use super::registry::{Param, Registry};
use super::sets::Set;
use crate::names;

/// Where the pinned inputs live, named in every generated file's first line and in each value's link.
pub struct Sources {
    pub gl: &'static str,
    pub egl: &'static str,
    pub angle: &'static str,
}

pub const SOURCES: Sources = Sources {
    gl: "https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml",
    egl: "https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml",
    angle: "https://github.com/google/angle/blob/6df81ae13d4ce5c42f9b158f014e862948dec678/scripts/gl_angle_ext.xml",
};

fn source_of(set: &Set) -> &'static str {
    if set.api == "egl" { SOURCES.egl } else { SOURCES.gl }
}

fn header(set: &Set) -> String {
    format!(
        "// Written by tools/api-crate-generator from Khronos's {} (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.\n",
        if set.api == "egl" { "egl.xml" } else { "gl.xml" }
    )
}

/// `glGetShaderInfoLog` to `get_shader_info_log`, `glUniformMatrix2x3fv` to `uniform_matrix2x3fv`.
pub fn method_name(set: &Set, command: &str) -> String {
    let stripped = command.strip_prefix(set.command_prefix).unwrap_or(command);
    // The vendor suffix is a word of its own: `ClipControlEXT` to `clip_control_ext`.
    let (body, vendor) = match ["EXT", "OES", "ANGLE", "KHR"].iter().find(|v| stripped.ends_with(*v)) {
        Some(vendor) => (&stripped[..stripped.len() - vendor.len()], Some(vendor.to_ascii_lowercase())),
        None => (stripped, None),
    };
    let mut name = names::snake_case(body);
    if let Some(vendor) = vendor {
        name = format!("{}_{vendor}", name.trim_end_matches('_'));
    }
    names::escaped(name)
}

fn param_name(param: &Param) -> String {
    names::escaped(names::snake_case(&param.name))
}

/// The value's Rust name: `GL_TEXTURE_2D` to `TEXTURE_2D`; one that would start with a digit keeps its prefix.
fn value_name(set: &Set, name: &str) -> String {
    let stripped = name.strip_prefix(set.value_prefix).unwrap_or(name);
    // `EGL_COLORSPACE_sRGB` to `COLORSPACE_SRGB`, `GL_COMPRESSED_RGBA_ASTC_4x4` to `COMPRESSED_RGBA_ASTC_4X4`.
    let kept = if stripped.starts_with(|c: char| c.is_ascii_digit()) { name } else { stripped };
    kept.to_ascii_uppercase()
}

/// The values of the core versions and the listed extensions, and every value a group of a carried parameter or a
/// refusal names.
fn values(registry: &Registry, set: &Set) -> BTreeSet<String> {
    let mut wanted = BTreeSet::new();
    for feature in registry.features.iter().filter(|f| f.api == set.api && super::registry::version_at_most(&f.number, set.core_up_to)) {
        wanted.extend(feature.requires.enums.iter().cloned());
    }
    for extension in registry.extensions.iter().filter(|e| set.extensions.contains(&e.name.as_str()) || e.requires.commands.iter().any(|c| set.commands.contains(&c.as_str()))) {
        wanted.extend(extension.requires.enums.iter().cloned());
    }
    for forced in set.forced {
        wanted.insert(forced.attribute.to_string());
        wanted.insert(forced.value.to_string());
    }
    wanted.extend(set.values.iter().map(|(name, _)| (*name).to_string()));
    wanted
}

/// The Rust text of a value: a number of the type the value's suffix gives, or none for a pointer or a cast this
/// crate does not carry.
fn value_text(set: &Set, value: &str, suffix: Option<&str>) -> Option<(&'static str, String)> {
    let ty: &'static str = match (set.api, suffix) {
        (_, Some("ull")) => "u64",
        ("egl", _) => "i32",
        _ => "u32",
    };
    if let Some(inner) = value.strip_prefix("EGL_CAST(EGLint,").and_then(|v| v.strip_suffix(')')) {
        return Some(("i32", inner.to_string()));
    }
    if value.starts_with("EGL_CAST") || value.starts_with('(') {
        return None;
    }
    let number = if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()?
    } else if let Some(negative) = value.strip_prefix('-') {
        return Some(("i32", format!("-{}", negative.parse::<u64>().ok()?)));
    } else {
        value.parse::<u64>().ok()?
    };
    // A value past i32 in EGL (EGL_FOREVER is `ull`) or past u32 in GL (only `ull` ones) keeps its own width.
    let ty = match ty {
        "i32" if number > i32::MAX as u64 => "u32",
        other => other,
    };
    Some((ty, format!("{:#x}", number)))
}

fn write_constants(registry: &Registry, set: &Set, report: &mut String) -> Result<String, String> {
    let mut text = header(set);
    let _ = writeln!(text, "//! The values of {} the methods take and give, as the registry declares them.\n", set.top);
    let mut written = 0;
    let mut seen_names = BTreeSet::new();
    for name in values(registry, set) {
        let Some(value) = registry.value(&name, set.api) else { continue };
        let Some((ty, number)) = value_text(set, &value.value, value.suffix.as_deref()) else { continue };
        let rust = value_name(set, &name);
        if !seen_names.insert(rust.clone()) {
            continue;
        }
        let link = if registry.commands.keys().any(|c| c.ends_with("ANGLE")) && name.ends_with("_ANGLE") { SOURCES.angle } else { source_of(set) };
        let _ = writeln!(text, "/// Declared as [`{name}`]({link}).\npub const {rust}: {ty} = {number};");
        written += 1;
    }
    let _ = writeln!(report, "VALUES {}: {written}", set.folder);
    for (name, reason) in set.values {
        let declared = registry.value(name, set.api).ok_or_else(|| format!("{}: the value {name} carried by name is not declared", set.folder))?;
        let _ = writeln!(report, "  VALUE BY NAME {name} {}: {reason}", declared.value);
    }
    Ok(text)
}

/// The trait's type of a part, or none when the part is left out of the trait.
fn trait_type(part: &Part, scalar_of: &Param) -> Option<String> {
    let _ = scalar_of;
    Some(match part {
        Part::Value(s) => s.rust.to_string(),
        Part::Handle { kind, optional: false } => format!("&Self::{kind}"),
        Part::Handle { kind, optional: true } => format!("Option<&Self::{kind}>"),
        Part::SliceIn { elem, .. } | Part::BoundedIn { elem, .. } => format!("&[{}]", elem.rust),
        Part::SliceOut { elem, .. } | Part::GuardedOut { elem, .. } => format!("&mut [{}]", elem.rust),
        Part::TextIn => "&core::ffi::CStr".to_string(),
        Part::TextsIn { lengths: Some(_), .. } => "&[&[u8]]".to_string(),
        Part::TextsIn { lengths: None, .. } => "&[&core::ffi::CStr]".to_string(),
        Part::BytesIn => "&[u8]".to_string(),
        Part::PixelUnpack { .. } => "PixelUnpack<'_>".to_string(),
        Part::PixelPack { .. } => "PixelPack<'_>".to_string(),
        Part::CompressedUnpack => "CompressedPixelUnpack<'_>".to_string(),
        Part::ArrayBufferOffset | Part::ElementBufferOffset => "u64".to_string(),
        Part::OptionalBytes { .. } => "Option<&[u8]>".to_string(),
        Part::AttributeList => "&[i32]".to_string(),
        Part::HandlesOut { kind, .. } => format!("&mut [Option<Self::{kind}>]"),
        Part::CountOf { .. } | Part::SingleOut(_) | Part::LengthsOf { .. } | Part::SizeOf { .. } | Part::WrittenOf | Part::NullOnly => return None,
    })
}

/// What the method returns, in the trait's terms, before an extension's `Option`.
fn return_parts(method: &Method) -> (Option<String>, Vec<String>) {
    let base = match &method.ret {
        Return::Unit | Return::Success => None,
        Return::Value(s) => Some(s.rust.to_string()),
        Return::Handle(kind) => Some(format!("Option<Self::{kind}>")),
        Return::Text => Some("Option<Vec<u8>>".to_string()),
    };
    let outs = method
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::SingleOut(s) => Some(s.rust.to_string()),
            Part::WrittenOf => Some("i32".to_string()),
            _ => None,
        })
        .collect();
    (base, outs)
}

fn tuple(types: &[String]) -> String {
    if types.len() == 1 { types[0].clone() } else { format!("({})", types.join(", ")) }
}

fn return_type(method: &Method) -> Option<String> {
    let (base, outs) = return_parts(method);
    let inner = match (&method.ret, base, outs.is_empty()) {
        (Return::Success, _, true) => Some("bool".to_string()),
        (Return::Success, _, false) => Some(format!("Option<{}>", tuple(&outs))),
        (_, None, true) => None,
        (_, None, false) => Some(tuple(&outs)),
        (_, Some(base), true) => Some(base),
        (_, Some(base), false) => Some(tuple(&[vec![base], outs].concat())),
    };
    if method.extensions.is_empty() { inner } else { Some(format!("Option<{}>", inner.unwrap_or_else(|| "()".to_string()))) }
}

fn signature(set: &Set, method: &Method, with_self_types: bool) -> String {
    let mut params = vec!["&self".to_string()];
    for (part, param) in method.parts.iter().zip(&method.command.params) {
        if let Some(ty) = trait_type(part, param) {
            let ty = if with_self_types { ty } else { ty.replace("Self::", "") };
            params.push(format!("{}: {ty}", param_name(param)));
        }
    }
    let ret = return_type(method).map(|r| format!(" -> {r}")).unwrap_or_default();
    format!("fn {}({}){ret}", method_name(set, &method.command.name), params.join(", "))
}

fn doc(set: &Set, method: &Method) -> String {
    let mut notes = Vec::new();
    if !method.extensions.is_empty() {
        notes.push(format!("None where the context names none of {}", method.extensions.join(", ")));
    }
    for part in &method.parts {
        match part {
            Part::GuardedOut { refused, .. } if !refused.is_empty() => notes.push(format!("refuses {}", refused.join(", "))),
            Part::PixelUnpack { .. } | Part::PixelPack { .. } | Part::CompressedUnpack => {
                notes.push("bytes only with no pixel buffer bound and long enough, an offset only with one".to_string())
            }
            Part::ArrayBufferOffset => notes.push("refused with no array buffer bound".to_string()),
            Part::ElementBufferOffset => notes.push("refused with no element array buffer bound".to_string()),
            _ => {}
        }
    }
    if method.draw {
        notes.push("refused while an enabled vertex attribute has no buffer".to_string());
    }
    let notes = if notes.is_empty() { String::new() } else { format!("; {}", notes.join("; ")) };
    format!("    /// Calls [`{}`]({}){notes}.\n", method.command.name, source_of(set))
}

const PIXEL_TYPES: &str = "/// Pixels a call reads: the caller's bytes, or an offset into the pixel unpack buffer bound.
#[derive(Clone, Copy, Debug)]
pub enum PixelUnpack<'a> {
    Bytes(&'a [u8]),
    Offset(u64),
}

/// Pixels a call writes: the caller's bytes, or an offset into the pixel pack buffer bound.
#[derive(Debug)]
pub enum PixelPack<'a> {
    Bytes(&'a mut [u8]),
    Offset(u64),
}

/// Compressed pixels a call reads: the caller's bytes, or a size at an offset into the pixel unpack buffer bound.
#[derive(Clone, Copy, Debug)]
pub enum CompressedPixelUnpack<'a> {
    Bytes(&'a [u8]),
    Offset { offset: u64, size: i32 },
}
";

fn write_interface(set: &Set, classified: &Classified) -> String {
    let mut text = header(set);
    let _ = writeln!(text, "//! {}\n", set.summary);
    let _ = writeln!(text, "pub mod constants;\n");
    if set.api == "gles2" {
        text.push_str(PIXEL_TYPES);
        text.push('\n');
    }
    let kinds: BTreeSet<&str> = classified
        .methods
        .iter()
        .flat_map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    Part::Handle { kind, .. } | Part::HandlesOut { kind, .. } => Some(*kind),
                    _ => None,
                })
                .chain(match m.ret {
                    Return::Handle(kind) => Some(kind),
                    _ => None,
                })
        })
        .collect();
    if set.api == "egl" {
        let _ = writeln!(text, "/// EGL's library: each entry point this set carries as a safe method, each handle a value of the implementation's own type.");
    } else {
        let _ = writeln!(text, "/// An OpenGL ES context: each command this set carries as a safe method on the context.");
    }
    // The pending glue error each implementation keeps is reported through get_error, as the registry's own.
    let _ = writeln!(text, "#[allow(reviewable::explicit_shared_error, reason = \"the API reports its errors through get_error, as its registry declares\")]");
    let _ = writeln!(text, "pub trait {}: Sized {{", set.top);
    for kind in &kinds {
        let _ = writeln!(text, "    /// A handle of the implementation's own, made only by its methods.\n    type {kind}: Clone + PartialEq + core::fmt::Debug;\n");
    }
    if set.api == "egl" {
        let _ = writeln!(
            text,
            "    /// Loads the EGL library named libEGL in the folder, or the system's when none is named; None when it cannot be loaded or cannot give every client API's entry point by name.\n    fn load(folder: Option<&std::path::Path>) -> Option<Self>;\n"
        );
    }
    for method in &classified.methods {
        text.push_str(&doc(set, method));
        let arguments = 1 + method.parts.iter().zip(&method.command.params).filter(|(part, param)| trait_type(part, param).is_some()).count();
        if arguments > MOST_ARGUMENTS {
            let _ = writeln!(text, "    #[expect(clippy::too_many_arguments, reason = \"the command's parameters, as the registry declares them\")]");
        }
        let _ = writeln!(text, "    {};\n", signature(set, method, true));
    }
    for composed in set.composed {
        let _ = writeln!(text, "    /// {}.", composed.reason);
        let _ = writeln!(text, "    fn {}(&self, target: u32, offset: i64, data: &mut [u8]) -> bool;\n", composed.name);
    }
    if text.ends_with("\n\n") {
        text.pop();
    }
    text.push_str("}\n");
    if set.api == "gles2" {
        text.push_str(
            "\n/// An OpenGL ES context reached through EGL: made over an EGL context and the surfaces it draws to and reads\n/// from, and made current on the calling thread before each call when it is not.\npub trait OnEgl<E: wcontainers_egl::Egl>: OpenGlEs {\n    fn new(egl: &E, display: &E::Display, context: &E::Context, draw: Option<&E::Surface>, read: Option<&E::Surface>) -> Option<Self>;\n}\n",
        );
    }
    text
}

// ----- The direct crate: the C entry points and the checks before them. -----

fn c_type(scalar: &Scalar) -> &'static str {
    scalar.c
}

/// The C type of a parameter in the entry point's signature.
fn c_param_type(part: &Part, param: &Param) -> String {
    let pointee = |is_const: bool, ty: &str| format!("*{} {ty}", if is_const { "const" } else { "mut" });
    match part {
        Part::Value(s) => c_type(s).to_string(),
        Part::Handle { .. } | Part::NullOnly => "*mut c_void".to_string(),
        Part::CountOf { .. } | Part::SizeOf { .. } => super::classify_scalar(&param.ptype).map(|s| s.c.to_string()).unwrap_or_default(),
        Part::SliceIn { elem, .. } | Part::BoundedIn { elem, .. } => pointee(true, elem.c),
        Part::SliceOut { elem, .. } | Part::GuardedOut { elem, .. } | Part::SingleOut(elem) => pointee(false, elem.c),
        Part::WrittenOf => "*mut i32".to_string(),
        Part::TextIn => "*const u8".to_string(),
        Part::TextsIn { .. } => "*const *const u8".to_string(),
        Part::LengthsOf { .. } => "*const i32".to_string(),
        Part::BytesIn | Part::PixelUnpack { .. } | Part::CompressedUnpack | Part::ArrayBufferOffset | Part::ElementBufferOffset | Part::OptionalBytes { .. } => {
            "*const c_void".to_string()
        }
        Part::PixelPack { .. } => "*mut c_void".to_string(),
        Part::AttributeList => "*const i32".to_string(),
        Part::HandlesOut { .. } => "*mut *mut c_void".to_string(),
    }
}

fn c_return_type(ret: &Return, command: &super::registry::Command) -> Option<String> {
    match ret {
        Return::Unit => None,
        Return::Value(s) => Some(s.c.to_string()),
        Return::Success => Some("u32".to_string()),
        Return::Handle(_) => Some("*mut c_void".to_string()),
        Return::Text => {
            let _ = command;
            Some("*const u8".to_string())
        }
    }
}

fn entry_type(method: &Method) -> String {
    let params: Vec<String> = method.parts.iter().zip(&method.command.params).map(|(part, param)| c_param_type(part, param)).collect();
    let ret = c_return_type(&method.ret, &method.command).map(|r| format!(" -> {r}")).unwrap_or_default();
    format!("unsafe extern \"system\" fn({}){ret}", params.join(", "))
}

/// The glue's statements for one call: checks and copies before, the C arguments, and what comes back.
struct Body {
    before: Vec<String>,
    arguments: Vec<String>,
    after: Vec<String>,
    outs: Vec<String>,
    facts: Vec<String>,
}

/// Clippy's limit of a function's arguments, past which a command's signature is kept as the registry declares it.
const MOST_ARGUMENTS: usize = 7;

/// The value a refused call returns: every return type of the traits has one.
const REFUSED: &str = "return Default::default()";

fn gl_body(set: &Set, method: &Method) -> Body {
    let command = &method.command;
    let mut body = Body { before: Vec::new(), arguments: Vec::new(), after: Vec::new(), outs: Vec::new(), facts: Vec::new() };
    let refuse = |error: &str| format!("{{ self.refuse(constants::{error}); {REFUSED}; }}");
    for (index, (part, param)) in method.parts.iter().zip(&command.params).enumerate() {
        let name = param_name(param);
        let argument = match part {
            Part::Value(s) => match (s.rust, s.c) {
                ("bool", c) => format!("{c}::from({name})"),
                ("i64", "isize") => {
                    body.before.push(format!("let Ok({name}) = isize::try_from({name}) else {};", refuse("INVALID_VALUE")));
                    name.clone()
                }
                _ => name.clone(),
            },
            Part::Handle { optional: false, .. } => {
                body.before.push(format!("let Some({name}_address) = self.fence_address({name}) else {};", refuse("INVALID_VALUE")));
                body.facts.push(format!("{name} is a fence this context made and has not deleted"));
                if command.name == "glDeleteSync" {
                    body.after.push(format!("self.forget_fence({name});"));
                }
                format!("{name}_address")
            }
            Part::CountOf { slice, multiplier } => {
                let slice_name = param_name(&command.params[*slice]);
                let ty = super::classify_scalar(&param.ptype).map(|s| s.c).unwrap_or("i32");
                body.before.push(format!(
                    "let Some({name}) = runtime::counted::<{ty}>({slice_name}.len(), {multiplier}) else {};",
                    refuse("INVALID_VALUE")
                ));
                body.facts.push(format!("{name} is {slice_name}'s length{}", if *multiplier > 1 { format!(" over {multiplier}") } else { String::new() }));
                name.clone()
            }
            Part::SliceIn { .. } | Part::BytesIn => format!("{name}.as_ptr().cast()"),
            Part::SliceOut { .. } => format!("{name}.as_mut_ptr()"),
            Part::SingleOut(s) => {
                body.before.push(format!("let mut {name}: {} = Default::default();", s.c));
                body.outs.push(if s.rust == "bool" { format!("{name} != 0") } else { name.clone() });
                body.facts.push(format!("{name} is one {} of ours", s.c));
                format!("&raw mut {name}")
            }
            Part::TextIn => {
                body.facts.push(format!("{name} is zero-ended"));
                format!("{name}.as_ptr().cast()")
            }
            Part::TextsIn { lengths: Some(_), .. } => {
                body.before.push(format!("let {name}_pointers: Vec<*const u8> = {name}.iter().map(|text| text.as_ptr()).collect();"));
                body.before.push(format!(
                    "let Some({name}_lengths) = {name}.iter().map(|text| i32::try_from(text.len()).ok()).collect::<Option<Vec<i32>>>() else {};",
                    refuse("INVALID_VALUE")
                ));
                body.facts.push(format!("each of {name}_pointers points at as many bytes as {name}_lengths says"));
                format!("{name}_pointers.as_ptr()")
            }
            Part::TextsIn { lengths: None, .. } => {
                body.before.push(format!("let {name}_pointers: Vec<*const u8> = {name}.iter().map(|text| text.as_ptr().cast::<u8>()).collect();"));
                body.facts.push(format!("each of {name}_pointers is zero-ended"));
                format!("{name}_pointers.as_ptr()")
            }
            Part::LengthsOf { texts } => format!("{}_lengths.as_ptr()", param_name(&command.params[*texts])),
            Part::GuardedOut { elem, by, guard, refused } => {
                let by_name = param_name(&command.params[*by]);
                if !refused.is_empty() {
                    let list: Vec<String> = refused.iter().map(|r| format!("constants::{}", value_name(set, r))).collect();
                    body.before.push(format!("if [{}].contains(&{by_name}) {}", list.join(", "), refuse("INVALID_ENUM")));
                }
                body.before.push(format!("let mut {name}_room = [{}::default(); {guard}];", elem.c));
                body.after.push(format!(
                    "for (to, from) in {name}.iter_mut().zip({name}_room) {{ *to = {}; }}",
                    if elem.rust == "bool" { "from != 0" } else { "from" }
                ));
                body.facts.push(format!("{name}_room holds {guard} values, more than any {by_name} writes"));
                format!("{name}_room.as_mut_ptr()")
            }
            Part::BoundedIn { elem, most } => {
                body.before.push(format!("let mut {name}_room = [{}::default(); {most}];", elem.c));
                body.before.push(format!("for (to, from) in {name}_room.iter_mut().zip({name}) {{ *to = *from; }}"));
                body.facts.push(format!("{name}_room holds {most} values, the most the command reads"));
                format!("{name}_room.as_ptr()")
            }
            Part::PixelUnpack { width, height, depth, format, type_ } => {
                let depth = depth.map(|d| format!("Some({})", param_name(&command.params[d]))).unwrap_or_else(|| "None".to_string());
                body.before.push(format!(
                    "let Some({name}) = self.unpack_pointer({name}, {}, {}, {depth}, {}, {}) else {{ {REFUSED}; }};",
                    param_name(&command.params[*width]),
                    param_name(&command.params[*height]),
                    param_name(&command.params[*format]),
                    param_name(&command.params[*type_]),
                ));
                body.facts.push(format!("{name} is the caller's bytes, as many as the unpack state reads, or an offset into the bound unpack buffer"));
                name.clone()
            }
            Part::PixelPack { width, height, format, type_ } => {
                body.before.push(format!(
                    "let Some({name}) = self.pack_pointer({name}, {}, {}, {}, {}) else {{ {REFUSED}; }};",
                    param_name(&command.params[*width]),
                    param_name(&command.params[*height]),
                    param_name(&command.params[*format]),
                    param_name(&command.params[*type_]),
                ));
                body.facts.push(format!("{name} is the caller's bytes, as many as the pack state writes, or an offset into the bound pack buffer"));
                name.clone()
            }
            Part::CompressedUnpack => {
                body.before.push(format!("let Some(({name}, {name}_size)) = self.compressed_pointer({name}) else {{ {REFUSED}; }};"));
                body.facts.push(format!("{name} is the caller's bytes of {name}_size, or an offset into the bound unpack buffer"));
                name.clone()
            }
            Part::SizeOf { data } => format!("{}_size", param_name(&command.params[*data])),
            Part::ArrayBufferOffset | Part::ElementBufferOffset => {
                let binding = if matches!(part, Part::ArrayBufferOffset) { "ARRAY_BUFFER_BINDING" } else { "ELEMENT_ARRAY_BUFFER_BINDING" };
                body.before.push(format!("let Some({name}) = self.offset_into(constants::{binding}, {name}) else {{ {REFUSED}; }};"));
                body.facts.push(format!("{name} is an offset into the bound buffer, never read as an address"));
                name.clone()
            }
            Part::OptionalBytes { size } => {
                let size_name = param_name(&command.params[*size]);
                body.before.push(format!(
                    "if {name}.is_some_and(|bytes| usize::try_from({size_name}).is_ok_and(|size| bytes.len() < size)) {}",
                    refuse("INVALID_VALUE")
                ));
                body.facts.push(format!("{name}, when there, holds at least {size_name} bytes"));
                format!("{name}.map_or(core::ptr::null(), |bytes| bytes.as_ptr().cast())")
            }
            Part::Handle { optional: true, .. } | Part::AttributeList | Part::HandlesOut { .. } | Part::WrittenOf | Part::NullOnly => {
                unreachable_form(index)
            }
        };
        body.arguments.push(argument);
    }
    body
}

/// An EGL form in an OpenGL ES command: classify.rs gives none, and the generator stops before writing one.
fn unreachable_form(index: usize) -> String {
    format!("compile_error!(\"an EGL form at parameter {index}\")")
}

fn egl_body(method: &Method) -> Body {
    let command = &method.command;
    let mut body = Body { before: Vec::new(), arguments: Vec::new(), after: Vec::new(), outs: Vec::new(), facts: Vec::new() };
    let refuse = |error: &str| format!("{{ runtime::refuse(constants::{error}); {REFUSED}; }}");
    let display = command.params.iter().position(|p| p.ptype == "EGLDisplay").map(|at| param_name(&command.params[at]));
    for (part, param) in method.parts.iter().zip(&command.params) {
        let name = param_name(param);
        let argument = match part {
            Part::Value(s) => match s.rust {
                "bool" => format!("{}::from({name})", s.c),
                _ => name.clone(),
            },
            Part::Handle { kind, optional } => {
                let check = match *kind {
                    "Display" => format!("self.display_address(handle)"),
                    "Config" => format!("self.config_address(handle, {})", display.clone().unwrap_or_default()),
                    _ => format!("self.held_address(&handle.held, {})", display.clone().unwrap_or_default()),
                };
                let error = match *kind {
                    "Display" => "BAD_DISPLAY",
                    "Config" => "BAD_CONFIG",
                    "Context" => "BAD_CONTEXT",
                    _ => "BAD_SURFACE",
                };
                if *optional {
                    body.before.push(format!(
                        "let {name} = match {name} {{ None => core::ptr::null_mut(), Some(handle) => match {check} {{ Some(address) => address, None => {} }} }};",
                        refuse(error)
                    ));
                } else {
                    body.before.push(format!("let handle = {name};"));
                    body.before.push(format!("let Some({name}) = {check} else {};", refuse(error)));
                    // Only the handle the call destroys ends: the display it is passed with lives on.
                    if command.name.starts_with("eglDestroy") && !matches!(*kind, "Display" | "Config") {
                        body.after.push("if result != 0 { handle.held.end(); }".to_string());
                    }
                }
                body.facts.push(format!("{name} is a {kind} of this library{}", if *optional { " or none" } else { "" }));
                name.clone()
            }
            Part::CountOf { slice, .. } => {
                let slice_name = param_name(&command.params[*slice]);
                body.before.push(format!("let Some({name}) = runtime::counted::<i32>({slice_name}.len(), 1) else {};", refuse("BAD_PARAMETER")));
                body.facts.push(format!("{name} is {slice_name}'s length"));
                name.clone()
            }
            Part::SingleOut(s) => {
                body.before.push(format!("let mut {name}: {} = Default::default();", s.c));
                body.outs.push(name.clone());
                body.facts.push(format!("{name} is one {} of ours", s.c));
                format!("&raw mut {name}")
            }
            Part::WrittenOf => {
                body.before.push(format!("let mut {name}: i32 = 0;"));
                body.outs.push(name.clone());
                format!("&raw mut {name}")
            }
            Part::AttributeList => {
                body.before.push(format!("let Some({name}) = runtime::attribute_list({name}, FORCED_{}) else {};", names::snake_case(&command.name).to_ascii_uppercase(), refuse("BAD_ATTRIBUTE")));
                body.facts.push(format!("{name} ends at a key EGL_NONE"));
                format!("{name}.as_ptr()")
            }
            Part::HandlesOut { kind, count, written } => {
                let count_name = param_name(&command.params[*count]);
                let written_name = param_name(&command.params[*written]);
                body.before.push(format!("let mut {name}_room = vec![core::ptr::null_mut::<c_void>(); {name}.len()];"));
                body.after.push(format!(
                    "let made = usize::try_from({written_name}).unwrap_or(0).min({name}.len());\n        for (slot, (at, address)) in {name}.iter_mut().zip({name}_room.iter().enumerate()) {{ *slot = (result != 0 && at < made && !address.is_null()).then(|| {kind}::new(*address, {}, self)); }}",
                    display.clone().unwrap_or_default()
                ));
                body.facts.push(format!("{name}_room holds {count_name} handles"));
                format!("{name}_room.as_mut_ptr()")
            }
            Part::NullOnly => {
                body.facts.push(format!("{name} is EGL_DEFAULT_DISPLAY"));
                "core::ptr::null_mut()".to_string()
            }
            _ => unreachable_form(0),
        };
        body.arguments.push(argument);
    }
    body
}

fn result_text(method: &Method, body: &Body, egl: bool) -> String {
    let display = method.command.params.iter().position(|p| p.ptype == "EGLDisplay").map(|at| param_name(&method.command.params[at]));
    let base = match &method.ret {
        Return::Unit | Return::Success => None,
        Return::Value(s) if s.rust == "bool" => Some("result != 0".to_string()),
        Return::Value(_) => Some("result".to_string()),
        Return::Text => Some("runtime::copied_text(result)".to_string()),
        Return::Handle(kind) if egl => Some(match *kind {
            "Display" => "(!result.is_null()).then(|| Display::new(result, self))".to_string(),
            _ => format!("(!result.is_null()).then(|| {kind}::new(result, {}, self))", display.clone().unwrap_or_default()),
        }),
        Return::Handle(_) => Some("self.keep_fence(result)".to_string()),
    };
    let inner = match (&method.ret, base, body.outs.is_empty()) {
        (Return::Success, _, true) => "result != 0".to_string(),
        (Return::Success, _, false) => format!("(result != 0).then_some({})", tuple(&body.outs)),
        (_, None, true) => "()".to_string(),
        (_, None, false) => tuple(&body.outs),
        (_, Some(base), true) => base,
        (_, Some(base), false) => tuple(&[vec![base], body.outs.clone()].concat()),
    };
    if method.extensions.is_empty() { inner } else { format!("Some({inner})") }
}

fn reason(set: &Set, method: &Method, body: &Body) -> String {
    let what = if body.facts.is_empty() { "plain values only".to_string() } else { body.facts.join("; ") };
    format!("{}: {} takes the C types the field declares; {what}", if set.api == "egl" { "egl.xml" } else { "gl.xml" }, method.command.name)
}

fn method_text(set: &Set, method: &Method) -> String {
    let egl = set.api == "egl";
    let body = if egl { egl_body(method) } else { gl_body(set, method) };
    let field = method_name(set, &method.command.name);
    let mut text = String::new();
    let _ = writeln!(text, "    {} {{", signature(set, method, false).replacen("fn ", "fn ", 1).replace("&Fence", "&Fence"));
    if egl {
        let gate = if method.command.name.starts_with("eglDestroy") { "writing" } else { "reading" };
        let _ = writeln!(text, "        let _gate = self.{gate}();");
        if method.command.name == "eglGetError" {
            let _ = writeln!(text, "        if let Some(error) = runtime::take_refusal() {{ return error; }}");
        }
        let _ = writeln!(text, "        let Some(entry) = self.entries().{field} else {{ runtime::refuse(constants::BAD_ACCESS); {REFUSED}; }};");
    } else {
        if method.command.name == "glGetError" {
            let _ = writeln!(text, "        if let Some(error) = self.take_refusal() {{ return error; }}");
        }
        let _ = writeln!(text, "        let Some(entries) = self.enter() else {{ {REFUSED}; }};");
        if !method.extensions.is_empty() {
            let names: Vec<String> = method.extensions.iter().map(|e| format!("c\"{e}\"")).collect();
            let _ = writeln!(text, "        if !self.has_extension(&[{}]) {{ return None; }}", names.join(", "));
            let _ = writeln!(text, "        let entry = entries.{field}?;");
        } else {
            let _ = writeln!(text, "        let Some(entry) = entries.{field} else {{ self.refuse(constants::INVALID_OPERATION); {REFUSED}; }};");
        }
        if method.draw {
            let _ = writeln!(text, "        if !self.draw_ready() {{ self.refuse(constants::INVALID_OPERATION); {REFUSED}; }}");
        }
    }
    for line in &body.before {
        let _ = writeln!(text, "        {line}");
    }
    let _ = writeln!(text, "        #[expect(unsafe_code, reason = \"{}\")]", reason(set, method, &body).replace('"', "'"));
    let _ = writeln!(text, "        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.");
    let result_binding = if matches!(method.ret, Return::Unit) { "" } else { "let result = " };
    let _ = writeln!(text, "        {result_binding}unsafe {{ entry({}) }};", body.arguments.join(", "));
    for line in &body.after {
        let _ = writeln!(text, "        {line}");
    }
    let result = result_text(method, &body, egl);
    if result != "()" {
        let _ = writeln!(text, "        {result}");
    }
    text.push_str("    }\n");
    text
}

fn entries_text(set: &Set, classified: &Classified, registry: &Registry) -> Result<String, String> {
    let mut fields = String::new();
    let mut loads = String::new();
    let mut all: Vec<(String, String, String)> = classified
        .methods
        .iter()
        .map(|m| (m.command.name.clone(), method_name(set, &m.command.name), entry_type(m)))
        .collect();
    // The commands the glue calls itself, typed by their own classification (no check of theirs is needed: the glue
    // passes only its own memory).
    for (inside, _) in set.inside {
        let command = registry.commands.get(*inside).ok_or_else(|| format!("{inside}: not in the registry"))?;
        let ty = inside_type(command)?;
        all.push((command.name.clone(), method_name(set, inside), ty));
    }
    for (c_name, field, ty) in &all {
        let _ = writeln!(fields, "    pub {field}: Option<{ty}>,");
        let _ = writeln!(
            loads,
            "            {field}: {{\n                let address = load(c\"{c_name}\");\n                #[expect(unsafe_code, reason = \"{}: the address the library gives for {c_name} is of the C signature the registry declares, or null\")]\n                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.\n                unsafe {{ core::mem::transmute::<*const c_void, Option<{ty}>>(address) }}\n            }},",
            if set.api == "egl" { "egl.xml" } else { "gl.xml" }
        );
    }
    let mut text = String::new();
    let _ = writeln!(text, "/// The library's entry points, each null when the library gives none.\npub(crate) struct Entries {{\n{fields}}}\n");
    let _ = writeln!(
        text,
        "impl Entries {{\n    /// Asks the library for every entry point by name.\n    pub(crate) fn load(load: impl Fn(&core::ffi::CStr) -> *const c_void) -> Entries {{\n        Entries {{\n{loads}        }}\n    }}\n}}"
    );
    Ok(text)
}

/// The C type of a command the glue calls itself.
fn inside_type(command: &super::registry::Command) -> Result<String, String> {
    let param = |p: &Param| -> Result<String, String> {
        if p.pointers == 0 {
            return super::classify_scalar(&p.ptype).map(|s| s.c.to_string()).or_else(|| (p.ptype == "EGLDisplay").then(|| "*mut c_void".to_string())).ok_or_else(|| format!("{}: {}", command.name, p.ptype));
        }
        Ok(if p.is_const { "*const u8".to_string() } else { "*mut c_void".to_string() })
    };
    let params: Result<Vec<String>, String> = command.params.iter().map(param).collect();
    let ret = match (command.ret.pointers, command.ret.ptype.as_str()) {
        (0, "void") => String::new(),
        (1, "void") | (0, "EGLContext") | (0, "__eglMustCastToProperFunctionPointerType") => " -> *mut c_void".to_string(),
        (0, ptype) => format!(" -> {}", super::classify_scalar(ptype).map(|s| s.c).ok_or_else(|| format!("{}: returns {ptype}", command.name))?),
        _ => return Err(format!("{}: return", command.name)),
    };
    Ok(format!("unsafe extern \"system\" fn({}){ret}", params?.join(", ")))
}

fn forced_text(set: &Set) -> String {
    let mut text = String::new();
    for command in set.commands.iter().filter(|c| set.forms.iter().any(|(fc, _, form)| fc == *c && *form == super::sets::Form::AttributeList)) {
        let pairs: Vec<String> = set
            .forced
            .iter()
            .filter(|f| f.command == *command)
            .map(|f| format!("(constants::{}, constants::{})", value_name(set, f.attribute), value_name(set, f.value)))
            .collect();
        let reasons: Vec<&str> = set.forced.iter().filter(|f| f.command == *command).map(|f| f.reason).collect();
        if !reasons.is_empty() {
            let _ = writeln!(text, "/// Forced into {command}'s attribute list: {}.", reasons.join("; "));
        } else {
            let _ = writeln!(text, "/// Forced into {command}'s attribute list: nothing.");
        }
        let _ = writeln!(text, "const FORCED_{}: &[(i32, i32)] = &[{}];", names::snake_case(command).to_ascii_uppercase(), pairs.join(", "));
    }
    text
}

fn write_calls(set: &Set, classified: &Classified, registry: &Registry) -> Result<String, String> {
    let mut text = header(set);
    let _ = writeln!(text, "//! Every command of the set as a safe method: the checks of its parameters' kinds, then one call of its entry point.\n");
    let interface = format!("wcontainers_{}", set.folder.replace('-', "_"));
    let _ = writeln!(text, "use core::ffi::c_void;\n");
    let _ = writeln!(text, "use {interface}::constants;");
    if set.api == "egl" {
        let _ = writeln!(text, "\nuse crate::{{Config, Context, Display, Library, Surface, runtime}};\n");
        text.push_str(&forced_text(set));
        text.push('\n');
    } else {
        let _ = writeln!(text, "use {interface}::{{CompressedPixelUnpack, PixelPack, PixelUnpack}};\n\nuse crate::{{Fence, OpenGlEsContext, runtime}};\n");
    }
    text.push_str(&entries_text(set, classified, registry)?);
    text.push('\n');
    let (implementor, kinds) = if set.api == "egl" {
        ("Library", "    type Config = Config;\n    type Context = Context;\n    type Display = Display;\n    type Surface = Surface;\n\n    fn load(folder: Option<&std::path::Path>) -> Option<Self> {\n        Library::open(folder)\n    }\n")
    } else {
        ("OpenGlEsContext", "    type Fence = Fence;\n")
    };
    let _ = writeln!(text, "impl {interface}::{} for {implementor} {{\n{kinds}", set.top);
    for method in &classified.methods {
        text.push_str(&method_text(set, method));
        text.push('\n');
    }
    for composed in set.composed {
        let _ = writeln!(text, "    fn {}(&self, target: u32, offset: i64, data: &mut [u8]) -> bool {{\n        self.{}_inside(target, offset, data)\n    }}\n", composed.name, composed.name);
    }
    if text.ends_with("\n\n") {
        text.pop();
    }
    text.push_str("}\n");
    Ok(text)
}

// ----- The conformance suite: every method called, generic over the traits. -----

fn sample(scalar: &Scalar, seed: &str) -> String {
    match scalar.rust {
        "bool" => format!("{seed}.is_multiple_of(2)"),
        "f32" => format!("pick(&FLOATS, {seed})"),
        "f64" => format!("f64::from(pick(&FLOATS, {seed}))"),
        "i64" => format!("pick(&SIZES, {seed})"),
        "u64" => format!("pick(&SIZES, {seed}).unsigned_abs()"),
        "u32" => format!("pick(&WORDS, {seed})"),
        "i32" => format!("pick(&NUMBERS, {seed})"),
        other => format!("pick(&NUMBERS, {seed}) as {other}"),
    }
}

fn suite_call(set: &Set, method: &Method) -> String {
    let mut prepared = Vec::new();
    let mut arguments = Vec::new();
    for (index, (part, param)) in method.parts.iter().zip(&method.command.params).enumerate() {
        let name = param_name(param);
        let seed = format!("seed.wrapping_add({index})");
        let argument = match part {
            Part::Value(s) => sample(s, &seed),
            Part::Handle { .. } => "fence".to_string(),
            Part::SliceIn { elem, .. } | Part::BoundedIn { elem, .. } => {
                prepared.push(format!("let {name} = vec![{}; length({seed})];", zero(elem)));
                format!("&{name}")
            }
            Part::SliceOut { elem, .. } | Part::GuardedOut { elem, .. } => {
                prepared.push(format!("let mut {name} = vec![{}; length({seed})];", zero(elem)));
                format!("&mut {name}")
            }
            Part::TextIn => "c\"name\"".to_string(),
            Part::TextsIn { lengths: Some(_), .. } => "&[b\"void main() {}\".as_slice()]".to_string(),
            Part::TextsIn { lengths: None, .. } => "&[c\"name\"]".to_string(),
            Part::BytesIn => {
                prepared.push(format!("let {name} = vec![0_u8; length({seed})];"));
                format!("&{name}")
            }
            Part::PixelUnpack { .. } => {
                prepared.push(format!("let {name} = vec![0_u8; length({seed})];"));
                format!("if {seed}.is_multiple_of(3) {{ PixelUnpack::Offset(0) }} else {{ PixelUnpack::Bytes(&{name}) }}")
            }
            Part::PixelPack { .. } => {
                prepared.push(format!("let mut {name} = vec![0_u8; length({seed})];"));
                format!("if {seed}.is_multiple_of(3) {{ PixelPack::Offset(0) }} else {{ PixelPack::Bytes(&mut {name}) }}")
            }
            Part::CompressedUnpack => {
                prepared.push(format!("let {name} = vec![0_u8; length({seed})];"));
                format!("if {seed}.is_multiple_of(3) {{ CompressedPixelUnpack::Offset {{ offset: 0, size: 16 }} }} else {{ CompressedPixelUnpack::Bytes(&{name}) }}")
            }
            Part::ArrayBufferOffset | Part::ElementBufferOffset => format!("pick(&SIZES, {seed}).unsigned_abs()"),
            Part::OptionalBytes { .. } => {
                prepared.push(format!("let {name} = vec![0_u8; length({seed})];"));
                format!("({seed}.is_multiple_of(2)).then_some({name}.as_slice())")
            }
            Part::CountOf { .. } | Part::SingleOut(_) | Part::LengthsOf { .. } | Part::SizeOf { .. } | Part::WrittenOf | Part::NullOnly => continue,
            Part::AttributeList | Part::HandlesOut { .. } => "compile_error!(\"EGL form\")".to_string(),
        };
        arguments.push(argument);
    }
    let bind = if return_type(method).is_some() { "let _ = " } else { "" };
    let call = format!("{bind}gl.{}({});", method_name(set, &method.command.name), arguments.join(", "));
    if prepared.is_empty() { format!("            {call}\n") } else { format!("            {{\n                {}\n                {call}\n            }}\n", prepared.join("\n                ")) }
}

fn zero(elem: &Scalar) -> &'static str {
    match elem.rust {
        "bool" => "false",
        "f32" => "0.0_f32",
        "f64" => "0.0_f64",
        "i64" => "0_i64",
        "u64" => "0_u64",
        "u32" => "0_u32",
        "u8" => "0_u8",
        _ => "0_i32",
    }
}

fn write_suite(set: &Set, classified: &Classified, template: &str) -> String {
    let mut calls = String::new();
    let skipped = ["glClientWaitSync", "glFinish"];
    let mut called = 0;
    for (index, method) in classified.methods.iter().enumerate() {
        if skipped.contains(&method.command.name.as_str()) {
            continue;
        }
        let _ = write!(calls, "        {index} => ");
        let text = suite_call(set, method);
        let _ = writeln!(calls, "{{\n{text}        }}");
        called += 1;
    }
    // Each extension's command, called with the context naming none of its extensions, must answer None.
    let mut gated = String::new();
    for method in classified.methods.iter().filter(|m| !m.extensions.is_empty()) {
        let names: Vec<String> = method.extensions.iter().map(|e| format!("\"{e}\"")).collect();
        let call = suite_call(set, method).replace("let _ = gl.", "let answered = gl.").replace(");\n", ").is_some();\n");
        let call = call.trim_end().trim_start_matches(' ').to_string();
        let call = call.strip_prefix('{').and_then(|c| c.strip_suffix('}')).map_or(call.clone(), |inner| inner.trim().to_string());
        let _ = writeln!(
            gated,
            "    {{\n        {}\n        assert!(named(&[{}]) || !answered, \"{} answered with none of its extensions named\");\n    }}",
            call.replace("\n                ", "\n        "),
            names.join(", "),
            method_name(set, &method.command.name)
        );
    }
    let count = classified.methods.len();
    let mut text = header(set);
    text.push_str(template);
    let _ = writeln!(
        text,
        "\n/// Calls each extension's command; one answering while the context names none of its extensions fails.\nfn extension_calls<G: OpenGlEs>(gl: &G, named: &dyn Fn(&[&str]) -> bool) {{\n    let seed = 0_usize;\n{gated}}}"
    );
    let _ = writeln!(
        text,
        "\n/// The number of the trait's methods `call_one` calls ({called} of {count}: glClientWaitSync and glFinish are called by the\n/// scenarios, never with a picked timeout, which could wait for ever).\npub const METHODS: usize = {count};\n\n/// Calls the trait's method numbered `which`, with arguments picked by `seed`.\nfn call_one<G: OpenGlEs>(gl: &G, fence: &G::Fence, which: usize, seed: usize) {{\n    match which {{\n{calls}        _ => {{}}\n    }}\n}}"
    );
    text
}

pub fn write(registry: &Registry, sets: &[&Set], classified: &[Classified], out: &std::path::Path, templates: &Templates) -> Result<String, String> {
    let mut report = String::new();
    for (set, classified) in sets.iter().zip(classified) {
        let folder = out.join("crates").join(set.folder);
        let direct = folder.join("implementation").join("direct");
        let conformance = folder.join("conformance");
        for path in [&folder, &direct, &conformance] {
            std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        let write = |path: std::path::PathBuf, text: &str| std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()));
        write(folder.join("interface.rs"), &write_interface(set, classified))?;
        write(folder.join("constants.rs"), &write_constants(registry, set, &mut report)?)?;
        let package = format!("wcontainers-{}", set.folder);
        let mut manifest = format!("[package]\nname = \"{package}\"\ndescription = \"{}\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\npublish = false\n\n[lib]\npath = \"interface.rs\"\n", set.summary);
        if set.api == "gles2" {
            manifest.push_str("\n[dependencies]\nwcontainers-egl = { path = \"../egl\" }\n");
        }
        manifest.push_str("\n[lints]\nworkspace = true\n");
        write(folder.join("Cargo.toml"), &manifest)?;
        // The direct crate.
        let (runtime, direct_manifest) = if set.api == "egl" { (templates.egl_direct, templates.egl_manifest) } else { (templates.gl_direct, templates.gl_manifest) };
        write(direct.join("implementation.rs"), &format!("{}{runtime}", header(set)))?;
        write(direct.join("calls.rs"), &write_calls(set, classified, registry)?)?;
        write(direct.join("Cargo.toml"), direct_manifest)?;
        if set.api == "gles2" {
            let fakes = out.join("fakes").join(set.folder);
            std::fs::create_dir_all(&fakes).map_err(|e| e.to_string())?;
            write(fakes.join("fake_egl.c"), &write_fake(set, classified, templates.fake))?;
            let tests = direct.join("tests");
            std::fs::create_dir_all(&tests).map_err(|e| e.to_string())?;
            write(tests.join("conformance.rs"), &format!("{}{}", header(set), templates.gl_direct_test))?;
            write(tests.join("kept.rs"), &format!("{}{}", header(set), templates.gl_direct_kept))?;
            // The runs asked for by name: programs of their own, each failing without its input.
            let examples = direct.join("examples");
            std::fs::create_dir_all(&examples).map_err(|e| e.to_string())?;
            write(examples.join("fuzz.rs"), &format!("{}{}", header(set), templates.gl_direct_fuzz))?;
            write(examples.join("measure.rs"), &format!("{}{}", header(set), templates.gl_direct_measure))?;
            write(conformance.join("suite.rs"), &write_suite(set, classified, templates.gl_suite))?;
            write(conformance.join("Cargo.toml"), templates.gl_suite_manifest)?;
        } else {
            std::fs::remove_dir(&conformance).map_err(|e| e.to_string())?;
        }
        let marks: usize = [direct.join("implementation.rs"), direct.join("calls.rs")]
            .iter()
            .map(|p| std::fs::read_to_string(p).map(|t| t.matches("expect(unsafe_code").count()).unwrap_or(0))
            .sum();
        let _ = writeln!(report, "CRATE {}: methods {} (+{} composed), inside {}, marks {marks}", set.folder, classified.methods.len(), set.composed.len(), set.inside.len());
        for composed in set.composed {
            let _ = writeln!(report, "  COMPOSED {} of {}: {}", composed.name, composed.calls.join(", "), composed.reason);
        }
        for (kind, count) in &classified.counts {
            let _ = writeln!(report, "  {kind}: {count}");
        }
        for method in classified.methods.iter().filter(|m| !m.extensions.is_empty()) {
            let _ = writeln!(report, "  EXTENSION {}: {}", method.command.name, method.extensions.join(" "));
        }
        for (command, reason) in set.left_out.iter().chain(set.inside) {
            let _ = writeln!(report, "  NOT OFFERED {command}: {reason}");
        }
        for forced in set.forced {
            let _ = writeln!(report, "  FORCED {} {}={}: {} (test {})", forced.command, forced.attribute, forced.value, forced.reason, forced.test);
        }
    }
    Ok(report)
}

pub struct Templates {
    pub egl_direct: &'static str,
    pub egl_manifest: &'static str,
    pub gl_direct: &'static str,
    pub gl_manifest: &'static str,
    pub gl_direct_test: &'static str,
    pub gl_direct_kept: &'static str,
    pub gl_direct_fuzz: &'static str,
    pub gl_direct_measure: &'static str,
    pub gl_suite: &'static str,
    pub gl_suite_manifest: &'static str,
    pub fake: &'static str,
}

// ----- The stand-in library the fuzzing runs on: every command reads and writes the most the registry lets it. -----

/// The OpenGL ES commands the fake's template writes by hand: those whose state the glue asks about.
const FAKE_BY_HAND: &[&str] = &[
    "glBindBuffer", "glDeleteBuffers", "glGenBuffers", "glPixelStorei", "glGetIntegerv", "glGetVertexAttribiv", "glVertexAttribPointer",
    "glVertexAttribIPointer", "glEnableVertexAttribArray", "glDisableVertexAttribArray", "glDrawArrays", "glDrawArraysInstanced",
    "glDrawElementsInstanced", "glGetString", "glGetStringi", "glMapBufferRange", "glUnmapBuffer", "glFenceSync", "glGetError",
];

/// C's spelling of a type the entry points are declared with.
fn c_of(rust: &str) -> String {
    let scalar = |t: &str| -> &'static str {
        match t {
            "u32" => "uint32_t",
            "i32" => "int32_t",
            "u8" => "uint8_t",
            "i8" => "int8_t",
            "i16" => "int16_t",
            "u16" => "uint16_t",
            "f32" => "float",
            "f64" => "double",
            "i64" => "int64_t",
            "u64" => "uint64_t",
            "isize" => "intptr_t",
            _ => "void",
        }
    };
    match rust {
        "*mut c_void" => "void *".to_string(),
        "*const c_void" => "const void *".to_string(),
        "*const *const u8" => "const uint8_t *const *".to_string(),
        "*mut *mut c_void" => "void **".to_string(),
        other => match (other.strip_prefix("*const "), other.strip_prefix("*mut ")) {
            (Some(inner), _) => format!("const {} *", scalar(inner)),
            (_, Some(inner)) => format!("{} *", scalar(inner)),
            _ => scalar(other).to_string(),
        },
    }
}

/// The count parameter of the slice at `slice`, and the values per counted element.
fn count_of(method: &Method, slice: usize) -> Option<(usize, usize)> {
    method.parts.iter().enumerate().find_map(|(at, part)| match part {
        Part::CountOf { slice: of, multiplier } if *of == slice => Some((at, *multiplier)),
        Part::SizeOf { data } if *data == slice => Some((at, 1)),
        _ => None,
    })
}

fn fake_method(method: &Method) -> String {
    let name = &method.command.name;
    let types: Vec<String> = method.parts.iter().zip(&method.command.params).map(|(part, param)| c_of(&c_param_type(part, param))).collect();
    let params: Vec<String> = types.iter().enumerate().map(|(at, ty)| format!("{ty} p{at}")).collect();
    let ret = c_return_type(&method.ret, &method.command).map_or("void".to_string(), |r| c_of(&r));
    let mut body = String::new();
    for (at, part) in method.parts.iter().enumerate() {
        let size = |elem: &Scalar| format!("sizeof({})", c_of(elem.c));
        let counted = |bytes_each: String| {
            count_of(method, at).map(|(count, by)| format!("if (p{count} > 0) fake_{{}}(\"{name}\", p{at}, (size_t)p{count} * {by} * {bytes_each});"))
        };
        let line = match part {
            Part::SliceIn { elem } => counted(size(elem)).map(|l| l.replace("fake_{}", "fake_read")),
            Part::BytesIn => counted("1".to_string()).map(|l| l.replace("fake_{}", "fake_read")),
            Part::SliceOut { elem } => counted(size(elem)).map(|l| l.replace("fake_{}", "fake_write")),
            Part::SingleOut(elem) => Some(format!("fake_write(\"{name}\", p{at}, {});", size(elem))),
            Part::TextIn => Some(format!("fake_read_text(\"{name}\", p{at});")),
            Part::TextsIn { lengths } => count_of(method, at).map(|(count, _)| {
                let each = match lengths {
                    Some(l) => format!(
                        "fake_read(\"{name}\", &p{l}[k], sizeof *p{l}); if (p{l}[k] < 0) fake_read_text(\"{name}\", p{at}[k]); else fake_read(\"{name}\", p{at}[k], (size_t)p{l}[k]);"
                    ),
                    None => format!("fake_read_text(\"{name}\", p{at}[k]);"),
                };
                format!("for (int32_t k = 0; k < p{count}; k++) {{ fake_read(\"{name}\", &p{at}[k], sizeof *p{at}); {each} }}")
            }),
            Part::GuardedOut { elem, guard, .. } => Some(format!("fake_write(\"{name}\", p{at}, {guard} * {});", size(elem))),
            Part::BoundedIn { elem, most } => Some(format!("fake_read(\"{name}\", p{at}, {most} * {});", size(elem))),
            Part::PixelUnpack { width, height, depth, format, type_ } => Some(format!(
                "if (!fake_unpack_bound()) fake_read(\"{name}\", p{at}, fake_transfer(p{width}, p{height}, {}, p{format}, p{type_}, 0));",
                depth.map_or("0".to_string(), |d| format!("p{d}"))
            )),
            Part::PixelPack { width, height, format, type_ } => {
                Some(format!("if (!fake_pack_bound()) fake_write(\"{name}\", p{at}, fake_transfer(p{width}, p{height}, 0, p{format}, p{type_}, 1));"))
            }
            Part::CompressedUnpack => count_of(method, at).map(|(size, _)| format!("if (!fake_unpack_bound() && p{size} > 0) fake_read(\"{name}\", p{at}, (size_t)p{size});")),
            Part::OptionalBytes { size } => Some(format!("if (p{at} && p{size} > 0) fake_read(\"{name}\", p{at}, (size_t)p{size});")),
            _ => None,
        };
        if let Some(line) = line {
            let _ = writeln!(body, "    {line}");
        }
    }
    let _ = writeln!(body, "    (void)fake_value();");
    for at in 0..types.len() {
        let _ = writeln!(body, "    (void)p{at};");
    }
    if ret != "void" {
        let value = if ret.contains('*') { "fake_handle()".to_string() } else { format!("({ret})fake_value()") };
        let _ = writeln!(body, "    return {value};");
    }
    format!("static {ret} fake_{name}({}) {{\n{body}}}\n", if params.is_empty() { "void".to_string() } else { params.join(", ") })
}

/// The fake's OpenGL ES part: each command not written by hand, and the table eglGetProcAddress answers from.
fn write_fake(set: &Set, classified: &Classified, template: &str) -> String {
    let mut text = format!(
        "/* Written by tools/api-crate-generator from Khronos's gl.xml and egl.xml, pinned in src/khronos/write.rs, and its template\n * templates/khronos/fake-egl.c. Do not edit. */\n{template}\n/* ----- Every other OpenGL ES command of the set: the most its parameters reach, read or written. ----- */\n\n"
    );
    let mut table = Vec::new();
    for method in &classified.methods {
        let name = &method.command.name;
        if !FAKE_BY_HAND.contains(&name.as_str()) {
            text.push_str(&fake_method(method));
            text.push('\n');
        }
        table.push(name.clone());
    }
    for (inside, _) in set.inside {
        table.push((*inside).to_string());
    }
    table.sort();
    table.dedup();
    let entries: Vec<String> = table.iter().map(|name| format!("        {{\"{name}\", (void *)fake_{name}}},")).collect();
    let _ = writeln!(
        text,
        "void *fake_gl_lookup(const char *name) {{\n    static const struct {{\n        const char *name;\n        void *address;\n    }} table[] = {{\n{}\n    }};\n    for (size_t at = 0; at < sizeof table / sizeof table[0]; at++)\n        if (strcmp(table[at].name, name) == 0) return table[at].address;\n    return NULL;\n}}",
        entries.join("\n")
    );
    text
}
