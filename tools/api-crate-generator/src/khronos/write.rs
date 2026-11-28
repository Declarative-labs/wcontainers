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
    // Any other cast or expression (`EGL_CAST(EGLDisplay,0)`, `((EGLint)-1)`) is no number: passed over below.
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
    let kinds = kinds_of(classified);
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

fn gl_body(set: &Set, method: &Method) -> Result<Body, String> {
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
            // EGL's forms: classify.rs gives an OpenGL ES command none.
            Part::Handle { optional: true, .. } | Part::AttributeList | Part::HandlesOut { .. } | Part::WrittenOf | Part::NullOnly => {
                return Err(format!("{}: an EGL form at parameter {index}", command.name));
            }
        };
        body.arguments.push(argument);
    }
    Ok(body)
}

fn egl_body(method: &Method) -> Result<Body, String> {
    let command = &method.command;
    let mut body = Body { before: Vec::new(), arguments: Vec::new(), after: Vec::new(), outs: Vec::new(), facts: Vec::new() };
    let refuse = |error: &str| format!("{{ runtime::refuse(constants::{error}); {REFUSED}; }}");
    let display = command.params.iter().position(|p| p.ptype == "EGLDisplay").map(|at| param_name(&command.params[at]));
    for (part, param) in method.parts.iter().zip(&command.params) {
        let name = param_name(param);
        let argument = match part {
            // EGL's plain values are its integers, passed as they are.
            Part::Value(_) => name.clone(),
            Part::Handle { kind, optional } => {
                let check = |handle: &str| match *kind {
                    "Display" => format!("self.display_address({handle})"),
                    "Config" => format!("self.config_address({handle}, {})", display.clone().unwrap_or_default()),
                    _ => format!("self.held_address(&{handle}.held, {})", display.clone().unwrap_or_default()),
                };
                let error = match *kind {
                    "Display" => "BAD_DISPLAY",
                    "Config" => "BAD_CONFIG",
                    "Context" => "BAD_CONTEXT",
                    _ => "BAD_SURFACE",
                };
                if *optional {
                    body.before.push(format!(
                        "let {name} = match {name} {{ None => core::ptr::null_mut(), Some(handle) => match {} {{ Some(address) => address, None => {} }} }};",
                        check("handle"),
                        refuse(error)
                    ));
                } else {
                    body.before.push(format!("let {name}_handle = {name};"));
                    body.before.push(format!("let Some({name}) = {} else {};", check(&format!("{name}_handle")), refuse(error)));
                    // Only the handle the call destroys ends: the display it is passed with lives on.
                    if command.name.starts_with("eglDestroy") && !matches!(*kind, "Display" | "Config") {
                        body.after.push(format!("if result != 0 {{ {name}_handle.held.end(); }}"));
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
            // OpenGL ES's forms: classify.rs gives an EGL command none.
            other => return Err(format!("{}: {other:?} has no EGL glue", command.name)),
        };
        body.arguments.push(argument);
    }
    Ok(body)
}

fn result_text(method: &Method, body: &Body, egl: bool) -> String {
    let display = method.command.params.iter().position(|p| p.ptype == "EGLDisplay").map(|at| param_name(&method.command.params[at]));
    let base = match &method.ret {
        Return::Unit | Return::Success => None,
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

fn method_text(set: &Set, method: &Method) -> Result<String, String> {
    let egl = set.api == "egl";
    let body = if egl { egl_body(method)? } else { gl_body(set, method)? };
    let field = method_name(set, &method.command.name);
    let mut text = String::new();
    let _ = writeln!(text, "    {} {{", signature(set, method, false));
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
    Ok(text)
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
            return super::classify_scalar(&p.ptype).map(|s| s.c.to_string()).ok_or_else(|| format!("{}: {}", command.name, p.ptype));
        }
        Ok(if p.is_const { "*const u8".to_string() } else { "*mut c_void".to_string() })
    };
    let params: Result<Vec<String>, String> = command.params.iter().map(param).collect();
    let ret = match (command.ret.pointers, command.ret.ptype.as_str()) {
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
        text.push_str(&method_text(set, method)?);
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
            // A wait of no time: a picked timeout could wait for ever on a fence whose commands are never flushed.
            Part::Value(_) if method.command.name == "glClientWaitSync" && param.name == "timeout" => "0".to_string(),
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
        "i64" => "0_i64",
        "u64" => "0_u64",
        "u32" => "0_u32",
        "u8" => "0_u8",
        _ => "0_i32",
    }
}

/// The handle kinds a set's methods take or make: the trait's associated types.
fn kinds_of(classified: &Classified) -> BTreeSet<&'static str> {
    classified
        .methods
        .iter()
        .flat_map(|m| {
            m.parts
                .iter()
                // (A kind a method makes is one some method takes too.)
                .filter_map(|p| match p {
                    Part::Handle { kind, .. } | Part::HandlesOut { kind, .. } => Some(*kind),
                    _ => None,
                })
        })
        .collect()
}

/// The suite's wrapper `name` of the set's trait: every call that reaches a method of the wrapped implementation is
/// counted by the method's place in the trait, so the fuzzing counts calls that reached each method, not the numbers
/// it picked. The composed methods are forwarded and not counted (the fuzzing does not call them).
fn write_counted(set: &Set, classified: &Classified, name: &str) -> String {
    let inner = if set.api == "egl" { "E" } else { "G" };
    let mut text = String::new();
    let _ = writeln!(text, "\n/// An implementation of `{}` whose every call that reaches one of the wrapped implementation's methods is counted,\n/// by the method's place in the trait.", set.top);
    let _ = writeln!(text, "pub struct {name}<'a, {inner}> {{\n    inner: &'a {inner},\n    calls: Vec<std::cell::Cell<u64>>,\n}}\n");
    let _ = writeln!(
        text,
        "impl<'a, {inner}> {name}<'a, {inner}> {{\n    /// `inner` with no call counted.\n    pub fn new(inner: &'a {inner}) -> Self {{\n        {name} {{ inner, calls: (0..{}).map(|_| std::cell::Cell::new(0)).collect() }}\n    }}\n\n    fn reached(&self, at: usize) {{\n        if let Some(count) = self.calls.get(at) {{\n            count.set(count.get() + 1);\n        }}\n    }}\n\n    /// The calls that reached each method, by its place in the trait.\n    pub fn calls(&self) -> Vec<u64> {{\n        self.calls.iter().map(std::cell::Cell::get).collect()\n    }}\n}}\n",
        classified.methods.len()
    );
    let _ = writeln!(text, "impl<{inner}: {}> {} for {name}<'_, {inner}> {{", set.top, set.top);
    for kind in kinds_of(classified) {
        let _ = writeln!(text, "    type {kind} = {inner}::{kind};");
    }
    if set.api == "egl" {
        let _ = writeln!(text, "\n    /// A wrapper is made only of a library already loaded.\n    fn load(_folder: Option<&std::path::Path>) -> Option<Self> {{\n        None\n    }}");
    }
    for (at, method) in classified.methods.iter().enumerate() {
        let arguments: Vec<String> = method.parts.iter().zip(&method.command.params).filter(|(part, param)| trait_type(part, param).is_some()).map(|(_, param)| param_name(param)).collect();
        // Clippy does not count a trait implementation's parameters: no expectation here.
        text.push('\n');
        let _ = writeln!(text, "    {} {{\n        self.reached({at});\n        self.inner.{}({})\n    }}", signature(set, method, true), method_name(set, &method.command.name), arguments.join(", "));
    }
    for composed in set.composed {
        let _ = writeln!(text, "\n    fn {0}(&self, target: u32, offset: i64, data: &mut [u8]) -> bool {{\n        self.inner.{0}(target, offset, data)\n    }}", composed.name);
    }
    text.push_str("}\n");
    text
}

fn write_suite(set: &Set, classified: &Classified, egl: Option<(&Set, &Classified)>, template: &str) -> String {
    let mut calls = String::new();
    for (index, method) in classified.methods.iter().enumerate() {
        let _ = write!(calls, "        {index} => ");
        let text = suite_call(set, method);
        let _ = writeln!(calls, "{{\n{text}        }}");
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
    text.push_str(&write_counted(set, classified, "CountedGl"));
    if let Some((egl_set, egl_classified)) = egl {
        text.push_str(&write_counted(egl_set, egl_classified, "CountedEgl"));
        let _ = writeln!(text, "\n/// The EGL methods of the trait (the counting wrapper's places).\npub const EGL_TRAIT_METHODS: usize = {};", egl_classified.methods.len());
    }
    let _ = writeln!(text, "\n/// The places of the methods `call_one` never calls: none (glClientWaitSync waits no time).\npub const NOT_FUZZED: [usize; 0] = [];");
    let _ = writeln!(
        text,
        "\n/// Calls each extension's command; one answering while the context names none of its extensions fails.\nfn extension_calls<G: OpenGlEs>(gl: &G, named: &dyn Fn(&[&str]) -> bool) {{\n    let seed = 0_usize;\n{gated}}}"
    );
    let _ = writeln!(
        text,
        "\n/// The number of the trait's methods, each of which `call_one` calls (glClientWaitSync with a timeout of 0: a picked one\n/// could wait for ever).\npub const METHODS: usize = {count};\n\n/// Calls the trait's method numbered `which`, with arguments picked by `seed`.\nfn call_one<G: OpenGlEs>(gl: &G, fence: &G::Fence, which: usize, seed: usize) {{\n    match which {{\n{calls}        _ => {{}}\n    }}\n}}"
    );
    text
}

// ----- The arguments test (tests/arguments.rs) and the calling glues' plant lists. -----

/// A value of its own for argument `at` of method `index` (each of a method's distinct, none zero, all below the room
/// every byte range is given), in Rust and as the stand-in logs it.
fn own_value(scalar: &Scalar, index: usize, at: usize) -> (String, String) {
    let number = 1001 + 100 * (index % 60) + at;
    match scalar.rust {
        "bool" => ("true".to_string(), "1".to_string()),
        "f32" | "f64" => (format!("{number}.5"), format!("{number}.5")),
        // The set's plain values are 32 and 64 bits wide (no byte or short is a value parameter of its commands).
        _ => (number.to_string(), number.to_string()),
    }
}

/// Element `element` of the values argument `at` of method `index` is given: one of its own (a flag both ways, no
/// number zero), in Rust and as the stand-in logs it (a float as its bits).
fn element_value(scalar: &Scalar, index: usize, at: usize, element: usize) -> Result<(String, String), String> {
    let number = 1 + 37 * index + 7 * at + element;
    Ok(match scalar.rust {
        "bool" => {
            let (text, shown) = if element.is_multiple_of(2) { ("true", "1") } else { ("false", "0") };
            (text.to_string(), shown.to_string())
        }
        "u8" => {
            let byte = number % 251 + 1;
            (format!("{byte}_u8"), byte.to_string())
        }
        "f32" => {
            let value = f32::from(u16::try_from(number).map_err(|_| format!("element {number} past a float's exact range"))?) + 0.5;
            (format!("{value}_f32"), format!("0x{:x}", value.to_bits()))
        }
        "i32" | "u32" | "i64" | "u64" => (format!("{number}_{}", scalar.rust), number.to_string()),
        other => return Err(format!("no element value of {other}")),
    })
}

/// The values given for argument `at` of method `index`: `given` of their own, then zeros up to `logged_count` (the
/// values the stand-in reads), as a Rust vector and as the stand-in logs them.
fn given_values(elem: &Scalar, index: usize, at: usize, given: usize, logged_count: usize) -> Result<(String, String), String> {
    let mut rust = Vec::new();
    let mut log = Vec::new();
    for element in 0..logged_count {
        let (text, shown) = element_value(elem, index, at, element)?;
        if element < given {
            rust.push(text);
            log.push(shown);
        } else {
            log.push(zero_logged(elem).to_string());
        }
    }
    Ok((format!("vec![{}]", rust.join(", ")), format!("[{}]", log.join(","))))
}

/// The stand-in's zero of an element as it logs it.
fn zero_logged(elem: &Scalar) -> &'static str {
    if elem.rust == "f32" { "0x0" } else { "0" }
}

/// The values a pixel transfer's parameters take so that the transfer is one the glue sizes: 2 by 2 (by 1) of RGBA
/// bytes, within the room given.
fn pixel_value(method: &Method, at: usize) -> Option<(String, String)> {
    let value = |text: &str, logged: &str| Some((text.to_string(), logged.to_string()));
    for part in &method.parts {
        let (width, height, depth, format, type_) = match part {
            Part::PixelUnpack { width, height, depth, format, type_ } => (*width, *height, *depth, *format, *type_),
            Part::PixelPack { width, height, format, type_ } => (*width, *height, None, *format, *type_),
            _ => continue,
        };
        if at == width || at == height {
            return value("2", "2");
        }
        if Some(at) == depth {
            return value("1", "1");
        }
        if at == format {
            return value("constants::RGBA", "6408");
        }
        if at == type_ {
            return value("constants::UNSIGNED_BYTE", "5121");
        }
    }
    None
}

/// The length of the slice at `at`: twice its count's multiplier, so that its count is 2.
fn slice_length(method: &Method, at: usize) -> usize {
    method
        .parts
        .iter()
        .find_map(|part| match part {
            Part::CountOf { slice, multiplier } if *slice == at => Some(2 * *multiplier),
            _ => None,
        })
        .unwrap_or(2)
}

/// The most values a guarded out parameter is given here: more than one, so that every value copied back is checked.
const GUARDED_GIVEN: usize = 4;

/// The line the stand-in must log for a call, as a Rust expression: its words, and the parts known only when the test
/// runs (a byte range's hash).
struct Logged {
    text: String,
    runtime: Vec<String>,
}

impl Logged {
    fn word(&mut self, word: &str) {
        self.text.push(' ');
        self.text.push_str(&word.replace('{', "{{").replace('}', "}}"));
    }

    fn at_runtime(&mut self, expression: String) {
        self.text.push_str(" {}");
        self.runtime.push(expression);
    }

    fn expression(&self) -> String {
        if self.runtime.is_empty() {
            format!("{:?}", self.text.replace("{{", "{").replace("}}", "}"))
        } else {
            format!("&format!({:?}, {})", self.text, self.runtime.join(", "))
        }
    }
}

/// What the method gives back, checked against what the stand-in logged it wrote and returned: the statements after
/// the call, binding `answer`.
fn given_back_checks(method: &Method, outs_checked: &[String]) -> String {
    let command = &method.command.name;
    let (base, _) = return_parts(method);
    let singles: Vec<usize> = method.parts.iter().enumerate().filter(|(_, part)| matches!(part, Part::SingleOut(_))).map(|(at, _)| at).collect();
    let mut names = Vec::new();
    let mut checks = Vec::new();
    if base.is_some() {
        names.push("returned".to_string());
        checks.push(match &method.ret {
            Return::Text => format!("calls.returned_text({command:?}, returned.as_deref());"),
            Return::Handle(_) => format!("calls.returned_handle({command:?}, returned.is_some());"),
            _ => format!("calls.returned({command:?}, &returned);"),
        });
    }
    for at in &singles {
        names.push(format!("out_{at}"));
        checks.push(format!("calls.wrote({command:?}, {at}, &[out_{at}]);"));
    }
    checks.extend(outs_checked.iter().cloned());
    let bind = match names.len() {
        0 => String::new(),
        1 => format!("let {} = answer;", names[0]),
        _ => format!("let ({}) = answer;", names.join(", ")),
    };
    let inner = [bind].into_iter().chain(checks).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n        ");
    if method.extensions.is_empty() {
        inner
    } else {
        let failed = format!("calls.failed(\"{command} answered None with its extensions named\");");
        if names.is_empty() {
            format!("if answer.is_none() {{ {failed} }}\n        {inner}")
        } else {
            format!("if let Some(answer) = answer {{\n        {inner}\n        }} else {{\n            {failed}\n        }}")
        }
    }
}

/// One method's call with arguments of its own, the line the stand-in must log for it checked, then everything it
/// gives back checked against what the stand-in wrote and returned.
fn argument_call(set: &Set, method: &Method, index: usize) -> Result<String, String> {
    let command = &method.command.name;
    let mut prepared = Vec::new();
    let mut arguments = Vec::new();
    let mut outs_checked = Vec::new();
    let mut logged = Logged { text: command.clone(), runtime: Vec::new() };
    let texts = [format!("first text of {index}"), format!("the second text of {index}")];
    for (at, (part, param)) in method.parts.iter().zip(&method.command.params).enumerate() {
        let name = param_name(param);
        let values = |elem: &Scalar, given: usize, logged_count: usize| given_values(elem, index, at, given, logged_count);
        let argument = match part {
            Part::Value(scalar) => {
                let (text, log) = pixel_value(method, at).unwrap_or_else(|| own_value(scalar, index, at));
                logged.word(&log);
                Some(text)
            }
            Part::Handle { .. } => {
                prepared.push(format!("let {name} = fence(gl);"));
                logged.word(FENCE_MADE);
                Some(format!("&{name}"))
            }
            Part::CountOf { slice, multiplier } => {
                logged.word(&(slice_length(method, *slice) / multiplier).to_string());
                None
            }
            Part::SliceIn { elem } => {
                let length = slice_length(method, at);
                let (given, log) = values(elem, length, length)?;
                prepared.push(format!("let {name} = {given};"));
                logged.word(&log);
                Some(format!("&{name}"))
            }
            Part::BoundedIn { elem, most } => {
                let (given, log) = values(elem, 2.min(*most), *most)?;
                prepared.push(format!("let {name} = {given};"));
                logged.word(&log);
                Some(format!("&{name}"))
            }
            Part::SliceOut { elem } => {
                prepared.push(format!("let mut {name} = vec![{}; {}];", zero(elem), slice_length(method, at)));
                logged.word("p");
                outs_checked.push(format!("calls.wrote({command:?}, {at}, &{name});"));
                Some(format!("&mut {name}"))
            }
            Part::GuardedOut { elem, guard, .. } => {
                prepared.push(format!("let mut {name} = vec![{}; {}];", zero(elem), GUARDED_GIVEN.min(*guard)));
                logged.word("p");
                outs_checked.push(format!("calls.wrote({command:?}, {at}, &{name});"));
                Some(format!("&mut {name}"))
            }
            Part::TextIn => {
                logged.word(&format!("'{}'", texts[0]));
                Some(format!("c{:?}", texts[0]))
            }
            Part::TextsIn { lengths: Some(_) } => {
                logged.word(&format!("['{}','{}']", texts[0], texts[1]));
                Some(format!("&[b{:?}.as_slice(), b{:?}.as_slice()]", texts[0], texts[1]))
            }
            Part::TextsIn { lengths: None } => {
                logged.word(&format!("['{}','{}']", texts[0], texts[1]));
                Some(format!("&[c{:?}, c{:?}]", texts[0], texts[1]))
            }
            Part::LengthsOf { .. } => {
                logged.word(&format!("[{},{}]", texts[0].len(), texts[1].len()));
                None
            }
            Part::BytesIn => {
                let (given, _) = values(&U8, slice_length(method, at), slice_length(method, at))?;
                prepared.push(format!("let {name}: Vec<u8> = {given};"));
                logged.at_runtime(format!("bytes(&{name})"));
                Some(format!("&{name}"))
            }
            Part::PixelUnpack { .. } => {
                prepared.push(format!("let {name} = patterned(ROOM, {});", index * 16 + at));
                logged.at_runtime(format!("bytes(&{name}[..PIXELS])"));
                Some(format!("PixelUnpack::Bytes(&{name})"))
            }
            Part::PixelPack { .. } => {
                prepared.push(format!("let mut {name} = vec![0_u8; ROOM];"));
                logged.word("p");
                outs_checked.push(format!("calls.wrote_bytes({command:?}, {at}, &{name});"));
                Some(format!("PixelPack::Bytes(&mut {name})"))
            }
            Part::CompressedUnpack => {
                prepared.push(format!("let {name} = patterned(COMPRESSED, {});", index * 16 + at));
                logged.at_runtime(format!("bytes(&{name})"));
                Some(format!("CompressedPixelUnpack::Bytes(&{name})"))
            }
            Part::SizeOf { .. } => {
                logged.word(&COMPRESSED.to_string());
                None
            }
            Part::ArrayBufferOffset | Part::ElementBufferOffset => {
                logged.word("16");
                Some("16".to_string())
            }
            Part::OptionalBytes { size } => {
                let size_param = &method.command.params[*size];
                let scalar = super::classify_scalar(&size_param.ptype).ok_or_else(|| format!("{command}: the size is no number"))?;
                let (size_value, _) = own_value(&scalar, index, *size);
                prepared.push(format!("let {name} = patterned(ROOM, {});", index * 16 + at));
                logged.at_runtime(format!("bytes(&{name}[..{size_value}])"));
                Some(format!("Some({name}.as_slice())"))
            }
            Part::SingleOut(_) => {
                logged.word("p");
                None
            }
            Part::AttributeList | Part::HandlesOut { .. } | Part::WrittenOf | Part::NullOnly => return Err(format!("{command}: an EGL form at parameter {at}")),
        };
        arguments.extend(argument);
    }
    let call = format!("gl.{}({})", method_name(set, command), arguments.join(", "));
    let mut text = String::from("    {\n");
    for line in &prepared {
        let _ = writeln!(text, "        {line}");
    }
    // glGetError answers the glue's own refusal first, if one is pending: called twice, the second reaches the library.
    if command == "glGetError" {
        let _ = writeln!(text, "        let _ = {call};");
    }
    let has_answer = return_type(method).is_some();
    let _ = writeln!(text, "        {}{call};", if has_answer { "let answer = " } else { "" });
    let _ = writeln!(text, "        calls.expect({command:?}, {});", logged.expression());
    if command == "glDeleteSync" {
        // The fence is no longer the glue's: a second delete is refused before the library.
        let _ = writeln!(text, "        {call};\n        calls.not_reached({command:?});");
    }
    let checks = given_back_checks(method, &outs_checked);
    if !checks.is_empty() {
        let _ = writeln!(text, "        {checks}");
    }
    text.push_str("    }\n");
    Ok(text)
}

/// The byte scalar, for byte ranges given as values.
const U8: Scalar = Scalar { rust: "u8", c: "u8" };
/// The bytes a compressed transfer is given (tests/arguments.rs's COMPRESSED).
const COMPRESSED: usize = 8;
/// A fence argument in a line the stand-in must log: replaced by the test with the handle the last glFenceSync made.
const FENCE_MADE: &str = "FENCE";

/// tests/arguments.rs's `every_method`: each method called once with arguments of its own, its line and what it gives
/// back checked.
fn write_arguments(set: &Set, classified: &Classified) -> Result<String, String> {
    let mut text = String::from(
        "\n/// Each method called once with arguments of its own: the line the library logged for it checked, and what it gave\n/// back.\nfn every_method(gl: &OpenGlEsContext, calls: &mut Calls) {\n",
    );
    for (index, method) in classified.methods.iter().enumerate() {
        text.push_str(&argument_call(set, method, index)?);
    }
    text.push_str("}\n");
    Ok(text)
}

/// The head of a calling glue's plant list.
fn plants_head(api: &str, file: &str) -> String {
    format!(
        "# {api}'s calling glue, its plant list (written by tools/api-crate-generator, src/khronos/write.rs `write_plants`):\n# for each method's call of its entry point ({file}), one plant per argument replaced by zero or a null address, one\n# per pointer argument pointing at other memory of the same type (what it points at changed), one per statement after\n# the call (each copy back dropped, a copy of several values made a copy of one) and one per value given back (the\n# whole answer its default, each of a tuple's parts its default, a library's refusal read as success, a negative\n# number made zero). Every plant must make the opengl-es direct crate's tests/arguments.rs fail: every argument\n# reaches the stand-in library as the method was given it, and everything the library wrote and returned reaches\n# the caller. Run from the repository's root, inside its `nix develop`, with the plant runner kept with the evidence\n# (outside the repository): run-plants.py <this list>\ntest: cargo test -q --release -p wcontainers-opengl-es-direct --test arguments\n"
    )
}

/// What points at other memory of the type a pointer argument of `c_type` points at, as the plants' content change:
/// none for a plain value.
fn other_memory(part: &Part, c_type: &str, argument: &str) -> Option<String> {
    Some(match (part, c_type) {
        // A handle passed on: the library's next.
        (Part::Handle { .. }, _) => format!("{argument}.wrapping_byte_add(1)"),
        (Part::NullOnly, _) => "core::ptr::without_provenance_mut(1)".to_string(),
        (Part::AttributeList, _) => "[constants::NONE; 2].as_ptr()".to_string(),
        (Part::HandlesOut { .. }, _) => "[core::ptr::null_mut::<c_void>(); 4096].as_mut_ptr()".to_string(),
        (_, "*const c_void") => "[0x5A_u8; 65536].as_ptr().cast()".to_string(),
        (_, "*mut c_void") => "[0_u8; 65536].as_mut_ptr().cast()".to_string(),
        (_, "*const u8") => "c\"other\".as_ptr().cast()".to_string(),
        // Texts as long as any given here, zero-ended.
        (_, "*const *const u8") => "[c\"................................................................\".as_ptr().cast::<u8>(); 64].as_ptr()".to_string(),
        (_, ty) => match (ty.strip_prefix("*const "), ty.strip_prefix("*mut ")) {
            (Some(elem), _) => format!("[{elem}::default(); 4096].as_ptr()"),
            (_, Some(elem)) => format!("[{elem}::default(); 4096].as_mut_ptr()"),
            _ => return None,
        },
    })
}

/// The changes of a statement after the call: dropped, and a copy of several values made a copy of one.
fn after_changes(line: &str) -> Vec<(&'static str, String)> {
    let mut changes = vec![("dropped", String::new())];
    if let Some(at) = line.find(".zip(") {
        let close = line[at..].find(") ").map(|end| at + end + 1);
        if let Some(close) = close {
            changes.push(("copying one value", format!("{}.take(1){}", &line[..close], &line[close..])));
        }
    }
    changes
}

/// The changes of the value given back: its default, each part of what it carries its default, a refusal read as
/// success, a negative number made zero.
fn answer_changes(line: &str, signed: bool) -> Vec<(String, String)> {
    let indent = &line[..line.len() - line.trim_start().len()];
    let answer = line.trim();
    let mut changes = vec![("its default".to_string(), format!("{indent}Default::default()"))];
    // What the answer carries: a `then_some`'s value, a `Some`'s, or the answer itself.
    let (prefix, payload) = match (answer.find(".then_some("), answer.strip_prefix("Some(")) {
        (Some(at), _) => answer.split_at(at + ".then_some(".len()),
        (None, Some(_)) => answer.split_at("Some(".len()),
        (None, None) => ("", answer),
    };
    let payload = if prefix.is_empty() { payload } else { payload.strip_suffix(')').unwrap_or(payload) };
    let close = if prefix.is_empty() { "" } else { ")" };
    let parts = match payload.strip_prefix('(').and_then(|inner| inner.strip_suffix(')')) {
        Some(inner) if !payload.contains("then") && !payload.contains("is_null") => split_arguments(inner),
        _ => Vec::new(),
    };
    for place in 0..parts.len() {
        let mut changed = parts.clone();
        changed[place] = "Default::default()".to_string();
        changes.push((format!("part {place} its default"), format!("{indent}{prefix}({}){close}", changed.join(", "))));
    }
    if parts.is_empty() && !prefix.is_empty() && payload != "()" {
        changes.push(("what it carries its default".to_string(), format!("{indent}{prefix}Default::default(){close}")));
    }
    for (refused, success) in [("result != 0", "result == 0"), ("(!result.is_null())", "(result.is_null())")] {
        if answer.contains(refused) {
            changes.push(("a refusal read as success".to_string(), format!("{indent}{}", answer.replace(refused, success))));
        }
    }
    if signed && answer == "result" {
        changes.push(("a negative number made zero".to_string(), format!("{indent}result.max(0)")));
    }
    changes
}

/// One plant: its name, and the text before and after (from the method's opening line, so that it occurs once).
fn plant(out: &mut String, file: &str, name: &str, lines: &[&str], start: usize, at: usize, new_line: &str) {
    let before: String = lines[start..at].iter().map(|line| format!("{line}\n")).collect();
    let new = if new_line.is_empty() { before.trim_end_matches('\n').to_string() } else { format!("{before}{new_line}") };
    let _ = writeln!(out, "\nplant: {name}\nfile: {file}\nold:\n{before}{}\nnew:\n{new}\nend", lines[at]);
}

/// A calling glue's plant list, from calls.rs as written: for each method's call of its entry point, its arguments,
/// the statements after it and the value it gives back, each changed (`plants_head`).
fn write_plants(set: &Set, classified: &Classified, calls: &str) -> Result<String, String> {
    let file = format!("crates/{}/implementation/direct/calls.rs", set.folder);
    let mut out = plants_head(if set.api == "egl" { "EGL" } else { "OpenGL ES" }, "calls.rs");
    let lines: Vec<&str> = calls.lines().collect();
    for method in &classified.methods {
        let command = &method.command.name;
        let field = method_name(set, command);
        let opening = format!("    {} {{", signature(set, method, false));
        let start = lines.iter().position(|line| *line == opening).ok_or_else(|| format!("calls.rs: no method {field}"))?;
        let end = (start..lines.len()).find(|at| lines[*at] == "    }").ok_or_else(|| format!("calls.rs: {field} has no end"))?;
        let at = (start..end).find(|at| lines[*at].contains("unsafe { entry(")).ok_or_else(|| format!("calls.rs: {field} calls no entry"))?;
        let line = lines[at];
        let open = line.find("entry(").map(|found| found + "entry(".len()).ok_or("calls.rs: no call")?;
        let close = line.rfind(") }").ok_or("calls.rs: no call's end")?;
        let arguments = split_arguments(&line[open..close]);
        if arguments.len() != method.parts.len() {
            return Err(format!("calls.rs: {field} passes {} arguments for {} parameters", arguments.len(), method.parts.len()));
        }
        for (place, ((argument, part), param)) in arguments.iter().zip(&method.parts).zip(&method.command.params).enumerate() {
            let ty = c_param_type(part, param);
            let wrong = if ty.starts_with("*const") {
                "core::ptr::null()"
            } else if ty.starts_with("*mut") {
                "core::ptr::null_mut()"
            } else if ty == "f32" || ty == "f64" {
                "0.0"
            } else {
                "0"
            };
            // (An argument that is already the wrong value, EGL_DEFAULT_DISPLAY's null, has only its other change.)
            let mut changes = if argument == wrong { Vec::new() } else { vec![(format!("as {wrong}"), wrong.to_string())] };
            if let Some(other) = other_memory(part, &ty, argument) {
                changes.push(("pointing at other memory".to_string(), other));
            }
            for (what, replacement) in changes {
                let mut changed = arguments.clone();
                changed[place] = replacement;
                let new_line = format!("{}{}{}", &line[..open], changed.join(", "), &line[close..]);
                plant(&mut out, &file, &format!("calls.rs {command} argument {place} ({argument}) {what}"), &lines, start, at, &new_line);
            }
        }
        let signed = matches!(&method.ret, Return::Value(scalar) if scalar.rust.starts_with('i'));
        for after in at + 1..end {
            let text = lines[after];
            let statement = text.trim_end().ends_with(';') || text.trim_end().ends_with('}');
            if !statement {
                for (what, new_line) in answer_changes(text, signed) {
                    plant(&mut out, &file, &format!("calls.rs {command} answer {what}"), &lines, start, after, &new_line);
                }
            } else if !text.trim_start().starts_with("let ") {
                // (A binding is used by what follows: dropped, nothing would compile.)
                for (what, new_line) in after_changes(text) {
                    plant(&mut out, &file, &format!("calls.rs {command} after the call, line {} {what}", after - at), &lines, start, after, &new_line);
                }
            }
        }
    }
    Ok(out)
}

/// A call's arguments split at its top-level commas.
fn split_arguments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current).trim().to_string());
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

pub fn write(registry: &Registry, sets: &[&Set], classified: &[Classified], egl: Option<(&Set, &Classified)>, out: &std::path::Path, templates: &Templates) -> Result<String, String> {
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
        let calls = write_calls(set, classified, registry)?;
        write(direct.join("calls.rs"), &calls)?;
        // The calling glue's plant list goes beside the crates written, never into a crate: only source is kept in the
        // repository.
        let plants = out.join("plants");
        std::fs::create_dir_all(&plants).map_err(|e| format!("{}: {e}", plants.display()))?;
        let listed = write_plants(set, classified, &calls)?;
        write(plants.join(format!("{}-calls.txt", set.folder)), &listed)?;
        report.push_str(&crate::plants::reported(&format!("{}-calls", set.folder), &listed));
        write(direct.join("Cargo.toml"), direct_manifest)?;
        if set.api == "gles2" {
            // The stand-in library fuzz-khronos.sh fuzzes the glue over (it reads and writes the most each command may).
            let fake = direct.join("fake");
            std::fs::create_dir_all(&fake).map_err(|e| e.to_string())?;
            write(fake.join("fake_egl.c"), &write_fake(set, classified, templates.fake)?)?;
            let tests = direct.join("tests");
            std::fs::create_dir_all(&tests).map_err(|e| e.to_string())?;
            write(tests.join("conformance.rs"), &format!("{}{}", header(set), templates.gl_direct_test))?;
            write(tests.join("kept.rs"), &format!("{}{}", header(set), templates.gl_direct_kept))?;
            write(tests.join("arguments.rs"), &format!("{}{}", header(set), templates.gl_direct_arguments.replace("// METHODS\n", &write_arguments(set, classified)?)))?;
            write(conformance.join("suite.rs"), &write_suite(set, classified, egl, templates.gl_suite))?;
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
    pub gl_suite: &'static str,
    pub gl_suite_manifest: &'static str,
    pub gl_direct_arguments: &'static str,
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
            "f32" => "float",
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

/// The stand-in's letter for the kind of value a pointer reaches (template's `fake_values`).
fn log_kind(elem: &Scalar) -> Result<char, String> {
    Ok(match elem.c {
        "i32" => 'i',
        "u32" => 'u',
        "f32" => 'f',
        "i64" | "isize" => 'I',
        "u64" => 'U',
        "u8" => 'b',
        other => return Err(format!("the stand-in logs no values of {other}")),
    })
}

/// One generated command of the stand-in: the call's line logged (each argument as tests/arguments.rs expects it,
/// what each pointer reaches), the most its parameters reach read or written, each value written logged, a value
/// returned (of its own in a logged run) and logged.
fn fake_method(method: &Method) -> Result<String, String> {
    let name = &method.command.name;
    let types: Vec<String> = method.parts.iter().zip(&method.command.params).map(|(part, param)| c_of(&c_param_type(part, param))).collect();
    let params: Vec<String> = types.iter().enumerate().map(|(at, ty)| format!("{ty} p{at}")).collect();
    let ret = c_return_type(&method.ret, &method.command).map_or("void".to_string(), |r| c_of(&r));
    let mut body = String::new();
    let counted = |at: usize| count_of(method, at).map_or("0".to_string(), |(count, by)| format!("(p{count} > 0 ? (size_t)p{count} * {by} : 0)"));
    let transfer = |width: &usize, height: &usize, depth: &Option<usize>, format: &usize, type_: &usize, pack: u8| {
        format!("fake_transfer(p{width}, p{height}, {}, p{format}, p{type_}, {pack})", depth.map_or("0".to_string(), |d| format!("p{d}")))
    };
    // The call's line in the log.
    let mut format = name.clone();
    let mut logged_values = Vec::new();
    // (The commands taking an offset into a bound buffer are the template's own, which log it as a number.)
    for (at, (part, ty)) in method.parts.iter().zip(&types).enumerate() {
        let (spec, value) = match part {
            Part::Handle { .. } => ("%s", format!("fake_handle_name(p{at})")),
            Part::SliceIn { elem } => ("%s", format!("fake_values(p{at}, {}, '{}')", counted(at), log_kind(elem)?)),
            Part::BoundedIn { elem, most } => ("%s", format!("fake_values(p{at}, {most}, '{}')", log_kind(elem)?)),
            Part::TextIn => ("%s", format!("fake_text(p{at})")),
            Part::TextsIn { lengths } => {
                let count = count_of(method, at).map_or("0".to_string(), |(count, _)| format!("p{count}"));
                ("%s", format!("fake_texts(p{at}, {count}, {})", lengths.map_or("NULL".to_string(), |l| format!("p{l}"))))
            }
            Part::LengthsOf { texts } => {
                let count = count_of(method, *texts).map_or("0".to_string(), |(count, _)| format!("(p{count} > 0 ? (size_t)p{count} : 0)"));
                ("%s", format!("fake_values(p{at}, {count}, 'i')"))
            }
            Part::BytesIn => ("%s", format!("fake_bytes(p{at}, {})", counted(at))),
            Part::PixelUnpack { width, height, depth, format, type_ } => ("%s", format!("fake_unpack_bound() ? \"offset\" : fake_bytes(p{at}, {})", transfer(width, height, depth, format, type_, 0))),
            Part::CompressedUnpack => ("%s", format!("fake_unpack_bound() ? \"offset\" : fake_bytes(p{at}, {})", counted(at))),
            Part::OptionalBytes { size } => ("%s", format!("fake_bytes(p{at}, p{size} > 0 ? (size_t)p{size} : 0)")),
            _ if ty.contains('*') => ("%s", format!("FAKE_ADDRESS(p{at})")),
            _ if ty == "float" => ("%.9g", format!("(double)p{at}")),
            _ if ty == "uint64_t" => ("%llu", format!("(unsigned long long)p{at}")),
            _ => ("%lld", format!("(long long)p{at}")),
        };
        format.push(' ');
        format.push_str(spec);
        logged_values.push(value);
    }
    let _ = writeln!(body, "    fake_log(\"{format}\"{});", logged_values.iter().map(|value| format!(", {value}")).collect::<String>());
    let mut wrote = Vec::new();
    for (at, part) in method.parts.iter().enumerate() {
        let size = |elem: &Scalar| format!("sizeof({})", c_of(elem.c));
        let counted_bytes = |bytes_each: String| {
            count_of(method, at).map(|(count, by)| format!("if (p{count} > 0) fake_{{}}(\"{name}\", p{at}, (size_t)p{count} * {by} * {bytes_each});"))
        };
        let line = match part {
            Part::SliceIn { elem } => counted_bytes(size(elem)).map(|l| l.replace("fake_{}", "fake_read")),
            Part::BytesIn => counted_bytes("1".to_string()).map(|l| l.replace("fake_{}", "fake_read")),
            Part::SliceOut { elem } => {
                wrote.push(format!("fake_log(\"  wrote {at} %s\", fake_values(p{at}, {}, '{}'));", counted(at), log_kind(elem)?));
                counted_bytes(size(elem)).map(|l| l.replace("fake_{}", "fake_write"))
            }
            Part::SingleOut(elem) => {
                wrote.push(format!("fake_log(\"  wrote {at} %s\", fake_values(p{at}, 1, '{}'));", log_kind(elem)?));
                Some(format!("fake_write(\"{name}\", p{at}, {});", size(elem)))
            }
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
            Part::GuardedOut { elem, guard, .. } => {
                wrote.push(format!("fake_log(\"  wrote {at} %s\", fake_values(p{at}, {guard}, '{}'));", log_kind(elem)?));
                Some(format!("fake_write(\"{name}\", p{at}, {guard} * {});", size(elem)))
            }
            Part::BoundedIn { elem, most } => Some(format!("fake_read(\"{name}\", p{at}, {most} * {});", size(elem))),
            Part::PixelUnpack { width, height, depth, format, type_ } => {
                Some(format!("if (!fake_unpack_bound()) fake_read(\"{name}\", p{at}, {});", transfer(width, height, depth, format, type_, 0)))
            }
            Part::PixelPack { width, height, format, type_ } => {
                let bytes = transfer(width, height, &None, format, type_, 1);
                wrote.push(format!("if (!fake_pack_bound()) fake_log(\"  wrote {at} %s\", fake_bytes(p{at}, {bytes}));"));
                Some(format!("if (!fake_pack_bound()) fake_write(\"{name}\", p{at}, {bytes});"))
            }
            Part::CompressedUnpack => count_of(method, at).map(|(size, _)| format!("if (!fake_unpack_bound() && p{size} > 0) fake_read(\"{name}\", p{at}, (size_t)p{size});")),
            Part::OptionalBytes { size } => Some(format!("if (p{at} && p{size} > 0) fake_read(\"{name}\", p{at}, (size_t)p{size});")),
            _ => None,
        };
        if let Some(line) = line {
            let _ = writeln!(body, "    {line}");
        }
    }
    for line in &wrote {
        let _ = writeln!(body, "    {line}");
    }
    let _ = writeln!(body, "    (void)fake_value();");
    for at in 0..types.len() {
        let _ = writeln!(body, "    (void)p{at};");
    }
    if ret != "void" {
        // (Every command returning an address is the template's own.)
        let _ = writeln!(body, "    {ret} value = fake.logging ? ({ret})fake_returned() : ({ret})fake_value();\n    fake_log(\"  returned %lld\", (long long)value);\n    return value;");
    }
    Ok(format!("static {ret} fake_{name}({}) {{\n{body}}}\n", if params.is_empty() { "void".to_string() } else { params.join(", ") }))
}

/// The fake's OpenGL ES part: each command not written by hand, and the table eglGetProcAddress answers from.
fn write_fake(set: &Set, classified: &Classified, template: &str) -> Result<String, String> {
    // Every extension the set names and each command names, so that each extension's command is reached.
    let all: BTreeSet<&str> = set.extensions.iter().copied().chain(classified.methods.iter().flat_map(|method| method.extensions.iter().map(String::as_str))).collect();
    let names: Vec<String> = all.iter().map(|extension| format!("\"{extension}\"")).collect();
    let template = template.replace(
        "/* EXTENSIONS */\n",
        &format!("/* Every extension the set names. */\n#define EXTENSION_COUNT {}\nstatic const char *const extensions[EXTENSION_COUNT] = {{{}}};\n", names.len(), names.join(", ")),
    );
    let mut text = format!(
        "/* Written by tools/api-crate-generator from Khronos's gl.xml and egl.xml, pinned in src/khronos/write.rs, and its template\n * templates/khronos/fake-egl.c. Do not edit. */\n{template}\n/* ----- Every other OpenGL ES command of the set: the most its parameters reach, read or written. ----- */\n\n"
    );
    let mut table = Vec::new();
    for method in &classified.methods {
        let name = &method.command.name;
        if !FAKE_BY_HAND.contains(&name.as_str()) {
            text.push_str(&fake_method(method)?);
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
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::super::classify::{Method, Part, Return};
    use super::super::registry::{Command, Param};
    use super::super::sets::{EGL, OPENGL_ES};
    use super::{count_of, slice_length, value_text};

    fn method(parts: Vec<Part>) -> Method {
        let param = |name: &str| Param { name: name.to_string(), ptype: "GLsizei".to_string(), pointers: 0, is_const: false, inner_const: false, len: None };
        let params = (0..parts.len()).map(|at| param(&format!("p{at}"))).collect();
        Method { command: Command { name: "glTest".to_string(), ret: param("return"), params }, parts, ret: Return::Unit, extensions: Vec::new(), draw: false }
    }

    /// A slice's count is the parameter that names that slice, among several counts.
    #[test]
    fn each_slice_its_own_count() {
        let elem = super::super::classify_scalar("GLint").expect("a scalar");
        let counted = method(vec![Part::CountOf { slice: 2, multiplier: 1 }, Part::CountOf { slice: 3, multiplier: 4 }, Part::SliceIn { elem }, Part::SliceIn { elem }]);
        assert_eq!(count_of(&counted, 2), Some((0, 1)));
        assert_eq!(count_of(&counted, 3), Some((1, 4)));
        assert_eq!((slice_length(&counted, 2), slice_length(&counted, 3)), (2, 8));
        let sized = method(vec![Part::SizeOf { data: 1 }, Part::CompressedUnpack, Part::SizeOf { data: 3 }, Part::CompressedUnpack]);
        assert_eq!(count_of(&sized, 1), Some((0, 1)));
        assert_eq!(count_of(&sized, 3), Some((2, 1)));
    }

    /// A value keeps the width its number needs: EGL's past i32 a u32, GL's `ull` a u64; a cast to EGLint is its
    /// number, any other cast or expression none.
    #[test]
    fn values_keep_their_width() {
        assert_eq!(value_text(&EGL, "0x7fffffff", None), Some(("i32", "0x7fffffff".to_string())));
        assert_eq!(value_text(&EGL, "0x80000000", None), Some(("u32", "0x80000000".to_string())));
        assert_eq!(value_text(&EGL, "EGL_CAST(EGLint,-1)", None), Some(("i32", "-1".to_string())));
        assert_eq!(value_text(&EGL, "EGL_CAST(EGLDisplay,0)", None), None);
        assert_eq!(value_text(&EGL, "((EGLint)-1)", None), None);
        assert_eq!(value_text(&OPENGL_ES, "0xFFFFFFFFFFFFFFFF", Some("ull")), Some(("u64", "0xffffffffffffffff".to_string())));
        assert_eq!(value_text(&OPENGL_ES, "0x8000", None), Some(("u32", "0x8000".to_string())));
    }
}
