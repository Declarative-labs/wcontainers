//! Writes the C++ converters' enums that the lowering reads as Rust constants (`ir_codes.rs`), from the headers of the
//! linked implementation of wcontainers-shader-converters, which builds the converters. Names the whole converter's
//! sources (the C++, as that build script hashes it, this crate's Rust and the carried naga) in
//! `WCS_CONVERTER_SOURCES` for the content key.

use std::path::{Path, PathBuf};

/// The folder of the converters' sources.
const CONVERTERS: &str = "../shader-converters/implementation/linked";

/// An enum of a C++ header: the header, the enum's name, the Rust module its constants go in and their
/// type.
type HeaderEnum = (&'static str, &'static str, &'static str, &'static str);

/// dxbc-spirv's enums the lowering reads, by header and C++ name: the module of `ir.rs` each becomes,
/// and the type its constants take where they are compared (the serialised IR's literal operands are
/// u64).
const IR_ENUMS: &[HeaderEnum] = &[
    ("ir/ir.h", "ScalarType", "scalar", "u8"),
    ("ir/ir.h", "OpCode", "code", "u16"),
    ("ir/ir.h", "OpFlag", "flag", "u8"),
    ("ir/ir.h", "BuiltIn", "builtin", "u64"),
    ("ir/ir.h", "ResourceKind", "kind", "u64"),
    ("ir/ir.h", "ShaderStage", "stage", "u64"),
    ("ir/ir.h", "PrimitiveType", "primitive", "u64"),
    ("ir/ir.h", "InterpolationMode", "interp", "u64"),
    ("ir/ir.h", "Construct", "construct", "u64"),
    ("ir/ir.h", "UavFlag", "uav", "u64"),
    ("ir/ir.h", "Scope", "scope", "u64"),
    ("ir/ir.h", "MemoryType", "memory", "u64"),
    ("ir/ir.h", "AtomicOp", "atomic_op", "u64"),
    ("ir/ir.h", "RoundMode", "round", "u64"),
    ("ir/ir.h", "DerivativeMode", "derivative", "u64"),
    ("dxbc/dxbc_signature.h", "SignatureSysval", "sysval", "u32"),
    ("dxbc/dxbc_parser.h", "ShaderType", "program", "u32"),
    ("dxbc/dxbc_types.h", "OpCode", "dxbc_op", "u32"),
    ("dxbc/dxbc_types.h", "UavFlag", "dxbc_uav", "u32"),
    ("dxbc/dxbc_types.h", "ResourceDim", "dxbc_dimension", "u32"),
    ("dxbc/dxbc_types.h", "SampledType", "dxbc_sampled", "u32"),
];

/// The boundary's enums, in the same form, so both sides share one table.
const BOUNDARY_ENUMS: &[HeaderEnum] = &[
    ("boundary.h", "ResourceClass", "class", "u32"),
    ("boundary.h", "TableWord", "table_word", "usize"),
    ("boundary.h", "ViewWord", "view_word", "usize"),
    ("boundary.h", "ViewShape", "view_shape", "u32"),
    ("boundary.h", "ViewValue", "view_value", "u32"),
    ("boundary.h", "ViewRange", "view_range", "u32"),
    ("boundary.h", "Slot", "slot", "usize"),
];

/// Left out of sources before they are hashed, so every checkout reads the same.
const CARRIAGE_RETURN: u8 = b'\r';

/// The variables in which the build scripts of the two implementations of the converters give the hash of their
/// sources; whichever is set is the converters' hash.
const CONVERTER_SOURCES: [&str; 2] = ["DEP_WCS_LINKED_SOURCES", "DEP_WCS_CONTAINED_SOURCES"];

fn main() {
    let converters = Path::new(CONVERTERS);
    let root = converters.join("dxbc-spirv");
    assert!(
        root.join("meson.build").exists(),
        "{} is missing: run `git submodule update --init` in the repository",
        root.display()
    );
    let build_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    ir_codes(
        &[(&root, IR_ENUMS), (&converters.join("cpp"), BOUNDARY_ENUMS)],
        &build_dir.join("ir_codes.rs"),
    );

    // The converter as a whole: the C++ above, this crate's Rust and this script, and the naga carried in the
    // workspace, whose readers and writers make the module.
    let sources = CONVERTER_SOURCES
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .expect("an implementation of the shader converters gives the hash of their sources");
    let naga = Path::new("../../third_party/naga/src");
    let mut converter = blake3::Hasher::new();
    converter.update(sources.as_bytes());
    hash_files(
        &mut converter,
        &[
            (PathBuf::from("src"), true),
            (PathBuf::from("build.rs"), false),
            (naga.into(), true),
        ],
    );
    println!(
        "cargo:rustc-env=WCS_CONVERTER_SOURCES={}",
        converter.finalize().to_hex()
    );
    for watched in [CONVERTERS, "src"] {
        println!("cargo:rerun-if-changed={watched}");
    }
    if naga.exists() {
        println!("cargo:rerun-if-changed={}", naga.display());
    }
}

/// Writes each list's enums, read from the headers under its directory, as Rust constants, so the
/// numbers the lowering compares are the C++ side's own, never retyped: `eSetCsWorkgroupSize` becomes
/// `SET_CS_WORKGROUP_SIZE`.
fn ir_codes(lists: &[(&Path, &[HeaderEnum])], out: &Path) {
    let mut rust = String::new();
    for (root, file, cpp, module, ty) in lists
        .iter()
        .flat_map(|(root, enums)| enums.iter().map(move |e| (root, e.0, e.1, e.2, e.3)))
    {
        let header = root.join(file);
        let text = std::fs::read_to_string(&header).unwrap();
        let start = text
            .find(&format!("enum class {cpp} :"))
            .unwrap_or_else(|| panic!("{}: no enum {cpp}", header.display()));
        let body = &text[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find("};").unwrap()];
        rust += &format!("/// `{cpp}`, from `{file}`.\n#[allow(dead_code)]\npub mod {module} {{\n");
        let mut values: Vec<(String, u64)> = Vec::new();
        for entry in without_comments(body).split(',') {
            let entry = entry.trim();
            let Some(name) = entry.strip_prefix('e') else {
                continue;
            };
            let (name, value) = match name.split_once('=') {
                Some((name, value)) => (name.trim(), value.trim()),
                None => (name, ""),
            };
            let value = match value {
                "" => values.last().map_or(0, |(_, v)| v + 1),
                v if v.starts_with('e') => {
                    values
                        .iter()
                        .find(|(n, _)| *n == v[1..])
                        .unwrap_or_else(|| panic!("{cpp}: {v} is not defined before {name}"))
                        .1
                }
                v => evaluate(v).unwrap_or_else(|| panic!("{cpp}::e{name}: cannot read {v}")),
            };
            values.push((name.to_string(), value));
            rust += &format!("    pub const {}: {ty} = {value};\n", screaming(name));
        }
        rust += "}\n";
    }
    std::fs::write(out, rust).unwrap();
}

/// C++ source without its comments.
fn without_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("/*") {
            rest = r.find("*/").map_or("", |end| &r[end + 2..]);
        } else if let Some(r) = rest.strip_prefix("//") {
            rest = r.find('\n').map_or("", |end| &r[end..]);
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// An enumerator's value: an unsigned literal (`5`, `5u`, `0x10u`), or a shift of one (`(1u << 3)`).
fn evaluate(text: &str) -> Option<u64> {
    let text = text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    if let Some((a, b)) = text.split_once("<<") {
        return Some(evaluate(a)? << evaluate(b)?);
    }
    let digits = text.trim_end_matches(['u', 'U']);
    match digits.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => digits.parse().ok(),
    }
}

/// `SetCsWorkgroupSize` as `SET_CS_WORKGROUP_SIZE`, `Image2DMsArray` as `IMAGE_2D_MS_ARRAY`.
fn screaming(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let lower = |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase());
        let word = match prev {
            None => false,
            Some(p) if c.is_ascii_digit() => p.is_ascii_lowercase(),
            Some(p) if c.is_ascii_uppercase() => {
                p.is_ascii_lowercase() || (p.is_ascii_uppercase() && lower(next))
            }
            _ => false,
        };
        if word {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// Hashes the files under each `(path, recursive)` (a path may be one file) by name and contents,
/// carriage returns left out, so every checkout of the same sources gives the same hash. A
/// directory listed without its subdirectories (a submodule's top) gives only its files with an
/// extension, as its sources all have one: git's files there have none, among them the submodule's
/// `.git` link, which depends on where the checkout keeps git's data.
fn hash_files(hasher: &mut blake3::Hasher, paths: &[(PathBuf, bool)]) {
    for file in list_files(paths) {
        let name = file.to_string_lossy().replace('\\', "/");
        let bytes: Vec<u8> = std::fs::read(&file)
            .unwrap()
            .into_iter()
            .filter(|&b| b != CARRIAGE_RETURN)
            .collect();
        hasher.update(name.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
}

/// The files under each `(path, recursive)`, sorted, as `hash_files` takes them.
fn list_files(paths: &[(PathBuf, bool)]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for (path, recursive) in paths {
        if path.is_file() {
            files.push(path.clone());
            continue;
        }
        let mut stack = vec![path.clone()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if *recursive {
                        stack.push(path);
                    }
                } else if *recursive || path.extension().is_some() {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}
