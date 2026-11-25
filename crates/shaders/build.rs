//! Builds the C++ converters with the `cc` crate: for WebAssembly into the crate, natively into a
//! module of their own that the crate runs under wasmtime (`converters.wasm`, kept in the repository
//! with the hash of the sources it was built from, so that a native build needs no SDK). Names the
//! whole converter's sources (the C++, this crate's Rust and the carried naga) in
//! `WCS_CONVERTER_SOURCES` for the content key. The file lists come from the converters' meson and
//! CMake files.

use std::path::{Path, PathBuf};

const DXBC_SPIRV: &[&str] = &[
    "dxbc/dxbc_api.cpp",
    "dxbc/dxbc_container.cpp",
    "dxbc/dxbc_converter.cpp",
    "dxbc/dxbc_disasm.cpp",
    "dxbc/dxbc_interface.cpp",
    "dxbc/dxbc_io_map.cpp",
    "dxbc/dxbc_parser.cpp",
    "dxbc/dxbc_registers.cpp",
    "dxbc/dxbc_resources.cpp",
    "dxbc/dxbc_signature.cpp",
    "dxbc/dxbc_types.cpp",
    "ir/ir.cpp",
    "ir/ir_builder.cpp",
    "ir/ir_disasm.cpp",
    "ir/ir_divergence.cpp",
    "ir/ir_dominance.cpp",
    "ir/ir_legalize.cpp",
    "ir/ir_serialize.cpp",
    "ir/ir_utils.cpp",
    "ir/passes/ir_pass_arithmetic.cpp",
    "ir/passes/ir_pass_buffer_indexing.cpp",
    "ir/passes/ir_pass_buffer_kind.cpp",
    "ir/passes/ir_pass_cfg_cleanup.cpp",
    "ir/passes/ir_pass_cfg_convert.cpp",
    "ir/passes/ir_pass_conditions.cpp",
    "ir/passes/ir_pass_cse.cpp",
    "ir/passes/ir_pass_derivative.cpp",
    "ir/passes/ir_pass_descriptor_indexing.cpp",
    "ir/passes/ir_pass_eval.cpp",
    "ir/passes/ir_pass_function.cpp",
    "ir/passes/ir_pass_lower_consume.cpp",
    "ir/passes/ir_pass_lower_io.cpp",
    "ir/passes/ir_pass_lower_min16.cpp",
    "ir/passes/ir_pass_propagate_resource_types.cpp",
    "ir/passes/ir_pass_propagate_types.cpp",
    "ir/passes/ir_pass_remove_unused.cpp",
    "ir/passes/ir_pass_scalarize.cpp",
    "ir/passes/ir_pass_scratch.cpp",
    "ir/passes/ir_pass_ssa.cpp",
    "ir/passes/ir_pass_sync.cpp",
    "util/util_float16.cpp",
    "util/util_log.cpp",
    "util/util_md5.cpp",
    "util/util_swizzle.cpp",
];

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

/// The directory of this crate's own C++ boundary.
const BOUNDARY: &str = "cpp";

/// The converters' module as built from the sources whose hash `MODULE_SOURCES` holds.
const MODULE: &str = "converters.wasm";
const MODULE_SOURCES: &str = "converters.blake3";

/// How the module is linked: a reactor (a library whose exports the host calls; the boundary marks
/// its functions with `WCS_EXPORT`), without debug information, its 1 MiB stack (Rust's for
/// WebAssembly, with which every corpus converts in the crate's own WebAssembly build) placed first
/// in its memory so that an overflow traps.
const MODULE_LINK: &[&str] = &[
    "-mexec-model=reactor",
    "-Wl,--strip-debug",
    "-Wl,-z,stack-size=1048576",
    "-Wl,--stack-first",
    "-o",
];

/// How the module is built again after its sources change.
const REBUILD: &str =
    "build the crate natively with wasi-sdk 25 named as for WebAssembly, which says what to copy";

/// The optimisation level the module is built at, whatever the crate's profile, so that every
/// build of the same sources gives the same bytes.
const MODULE_OPTIMISATION: u32 = 2;

/// The flag mapping the build directory's paths (of the patched sources, in assertion messages) to
/// one name, so that every machine's build gives the same bytes.
const PREFIX_MAP: [&str; 2] = ["-ffile-prefix-map=", "=out"];

/// The target the module is built for.
const MODULE_TARGET: &str = "wasm32-wasip1";

/// Changes carried to the converters until upstream has them: `<converter>/<file>.patch` here is a
/// unified diff of `<file>`, one of the converter's compiled sources, which is then compiled from
/// the build directory in place of the submodule's.
const PATCHES: &str = "converter-patches";

/// A carried patch's file extension.
const PATCH_EXTENSION: &str = "patch";

/// Left out of sources before they are hashed or patched, so every checkout reads the same.
const CARRIAGE_RETURN: u8 = b'\r';

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

const DXIL_SPIRV: &[&str] = &[
    "debug/logging.cpp",
    "util/thread_local_allocator.cpp",
    "descriptor_qa.cpp",
    "spirv_module.cpp",
    "spirv_module_instrumentation.cpp",
    "memory_stream.cpp",
    "llvm_bitcode_parser.cpp",
    "dxil_converter.cpp",
    "cfg_structurizer.cpp",
    "cfg_structurizer_new.cpp",
    "node_pool.cpp",
    "node.cpp",
    "dxil_parser.cpp",
    "opcodes/dxil/dxil_common.cpp",
    "opcodes/dxil/dxil_resources.cpp",
    "opcodes/dxil/dxil_compute.cpp",
    "opcodes/dxil/dxil_arithmetic.cpp",
    "opcodes/dxil/dxil_pixel_ops.cpp",
    "opcodes/dxil/dxil_geometry.cpp",
    "opcodes/dxil/dxil_tessellation.cpp",
    "opcodes/dxil/dxil_waveops.cpp",
    "opcodes/dxil/dxil_sampling.cpp",
    "opcodes/dxil/dxil_buffer.cpp",
    "opcodes/dxil/dxil_ray_tracing.cpp",
    "opcodes/dxil/dxil_mesh.cpp",
    "opcodes/dxil/dxil_workgraph.cpp",
    "opcodes/dxil/dxil_ags.cpp",
    "opcodes/dxil/dxil_nvapi.cpp",
    "opcodes/opcodes_llvm_builtins.cpp",
    "opcodes/opcodes_dxil_builtins.cpp",
    "dxil_spirv_c.cpp",
    "third_party/glslang-spirv/SpvBuilder.cpp",
    "third_party/bc-decoder/llvm_decoder.cpp",
    "bc/value.cpp",
    "bc/instruction.cpp",
    "bc/function.cpp",
    "bc/context.cpp",
    "bc/type.cpp",
    "bc/module.cpp",
    "bc/module_dxbc_ir.cpp",
    "bc/metadata.cpp",
    "bc/disassembler.cpp",
];

fn main() {
    let root = Path::new("dxbc-spirv");
    assert!(
        root.join("meson.build").exists(),
        "{} is missing: run `git submodule update --init` in the repository",
        root.display()
    );
    let dxil = Path::new("dxil-spirv");
    assert!(
        dxil.join("third_party/spirv-headers/include").exists(),
        "{0} is missing: run `git submodule update --init --recursive crates/shaders/{0}`",
        dxil.display()
    );
    // The C++ sources (dxil-spirv's top directory alone, its subdirectories are listed) and this
    // script; the objects also depend on the SDK for WebAssembly, which the module does not.
    let cpp = [
        (root.join("dxbc"), true),
        (root.join("ir"), true),
        (root.join("util"), true),
        (dxil.to_path_buf(), false),
        (dxil.join("bc"), true),
        (dxil.join("debug"), true),
        (dxil.join("util"), true),
        (dxil.join("opcodes"), true),
        (dxil.join("third_party/glslang-spirv"), true),
        (dxil.join("third_party/bc-decoder"), true),
        (
            dxil.join("third_party/spirv-headers/include/spirv/unified1"),
            true,
        ),
        (PathBuf::from(BOUNDARY), true),
        (PathBuf::from(PATCHES), true),
        (PathBuf::from("build.rs"), false),
    ];
    let mut sources = blake3::Hasher::new();
    hash_files(&mut sources, &cpp);
    let sources = sources.finalize().to_hex().to_string();
    let build_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let (route, base) = base(&build_dir);
    let mut hasher = blake3::Hasher::new();
    hasher.update(sources.as_bytes());
    hasher.update(format!("{route:?}").as_bytes());
    hasher.update(
        std::env::var("WASI_SDK_PATH")
            .unwrap_or_default()
            .as_bytes(),
    );
    let objects_hash = hasher.finalize().to_hex().to_string();

    let linked = route != Route::Module;
    let built = base.map(|base| {
        let mut libraries = Vec::new();
        let mut build = base.clone();
        build
            .std("c++17")
            .include(root)
            .define("DXBC_SPV_ENABLE_SM5", None);
        add_sources(&mut build, &build_dir, root, DXBC_SPIRV);
        build.file("cpp/dxbc.cpp");
        libraries.push(compile(&build, "dxbc_spirv", &objects_hash, linked));

        // dxil-spirv (C++14, built as C++17 as its CMake does where it reads dxbc-spirv), with its own
        // glslang SPIR-V builder, bitcode reader and SPIRV-Headers. Its bitcode module also reads
        // dxbc-spirv's IR, from our one copy of dxbc-spirv.
        let mut build = base.clone();
        build
            .std("c++17")
            .flag_if_supported("-fno-rtti")
            .include(dxil)
            .include(dxil.join("debug"))
            .include(dxil.join("util"))
            .include(dxil.join("bc"))
            .include(dxil.join("third_party/glslang-spirv"))
            .include(dxil.join("third_party/bc-decoder"))
            .include(dxil.join("third_party/spirv-headers/include/spirv/unified1"))
            .include(root)
            .include(root.join("ir"))
            .define("HAVE_LLVMBC", None)
            .define("AMD_EXTENSIONS", None)
            .define("DXBC_SPV_ENABLE_SM5", None)
            .define("NOMINMAX", None);
        add_sources(&mut build, &build_dir, dxil, DXIL_SPIRV);
        build.file("cpp/dxil.cpp");
        libraries.push(compile(&build, "dxil_spirv", &objects_hash, linked));
        // Linked after both, which call it.
        let mut build = base.clone();
        build.file("cpp/assertion.cpp");
        libraries.push(compile(&build, "wcs_assertion", &objects_hash, linked));
        (base, libraries)
    });
    match built {
        Some(_) if linked => {}
        Some((base, libraries)) => {
            let out = build_dir.join(MODULE);
            let mut link = base.get_compiler().to_command();
            link.args(MODULE_LINK)
                .arg(&out)
                .args(libraries.iter().flatten());
            let status = link.status().expect("the SDK's clang++ runs");
            assert!(status.success(), "linking {MODULE} failed: {link:?}");
            let recorded = std::fs::read_to_string(MODULE_SOURCES).unwrap_or_default();
            let same = std::fs::read(MODULE).ok() == Some(std::fs::read(&out).unwrap());
            std::fs::write(build_dir.join(MODULE_SOURCES), &sources).unwrap();
            assert!(
                same && recorded.trim() == sources,
                "the committed {MODULE} is not this build of its sources: copy {} and {} into the crate",
                out.display(),
                build_dir.join(MODULE_SOURCES).display()
            );
        }
        None => {
            let recorded = std::fs::read_to_string(MODULE_SOURCES).unwrap_or_default();
            assert!(
                recorded.trim() == sources,
                "the converters' sources changed since {} was built: {}",
                MODULE,
                REBUILD
            );
        }
    }

    ir_codes(
        &[(root, IR_ENUMS), (Path::new(BOUNDARY), BOUNDARY_ENUMS)],
        &build_dir.join("ir_codes.rs"),
    );

    // The converter as a whole: the C++ above, this crate's Rust, and the naga carried in the
    // workspace, whose readers and writers make the module.
    let naga = Path::new("../../third_party/naga/src");
    let mut converter = blake3::Hasher::new();
    converter.update(sources.as_bytes());
    hash_files(
        &mut converter,
        &[(PathBuf::from("src"), true), (naga.into(), true)],
    );
    println!(
        "cargo:rustc-env=WCS_CONVERTER_SOURCES={}",
        converter.finalize().to_hex()
    );
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");
    for watched in [
        BOUNDARY,
        PATCHES,
        "dxbc-spirv",
        "dxil-spirv",
        "src",
        MODULE,
        MODULE_SOURCES,
    ] {
        println!("cargo:rerun-if-changed={watched}");
    }
    if naga.exists() {
        println!("cargo:rerun-if-changed={}", naga.display());
    }
}

/// Adds the converter's `files` under `root` to `build`, each with its carried patch applied
/// (`PATCHES`) into `build_dir`; a patch naming no file of the list stops the build.
fn add_sources(build: &mut cc::Build, build_dir: &Path, root: &Path, files: &[&str]) {
    let converter = root.file_name().unwrap();
    let patches = Path::new(PATCHES).join(converter);
    let mut unused = list_files(&[(patches.clone(), true)]);
    let out = build_dir.join(PATCHES).join(converter);
    for file in files {
        let original = root.join(file);
        let patch = patches.join(file).with_added_extension(PATCH_EXTENSION);
        let Some(place) = unused.iter().position(|p| *p == patch) else {
            build.file(original);
            continue;
        };
        unused.swap_remove(place);
        let text = |path: &Path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .replace(char::from(CARRIAGE_RETURN), "")
        };
        let diff = text(&patch);
        let diff = diffy::Patch::from_str(&diff)
            .unwrap_or_else(|e| panic!("{}: not a unified diff: {e}", patch.display()));
        let changed = diffy::apply(&text(&original), &diff)
            .unwrap_or_else(|e| panic!("{} does not apply: {e}", patch.display()));
        let target = out.join(file);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, changed).unwrap();
        // Its quoted includes name files beside the original.
        build.include(original.parent().unwrap());
        build.file(target);
    }
    assert!(
        unused.is_empty(),
        "patches naming no compiled source: {unused:?}"
    );
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

/// Compiles `build`'s sources into objects kept under the name `name`, reusing those of an earlier
/// run while `hash` (of everything that makes them) is unchanged, so that a change to the Rust sources
/// alone, which reruns this script for the content key, compiles no C++. Where the converters are
/// linked into the crate, makes them its static library `name`; returns the objects.
fn compile(build: &cc::Build, name: &str, hash: &str, linked: bool) -> Vec<PathBuf> {
    let dir = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(format!("{name}-objects"));
    let stamp = dir.join("hash");
    let list = |dir: &Path| -> Vec<PathBuf> {
        let mut objects: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != &stamp)
            .collect();
        objects.sort();
        objects
    };
    if std::fs::read_to_string(&stamp).ok().as_deref() != Some(hash) {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, object) in build.compile_intermediates().iter().enumerate() {
            let file = object.file_name().unwrap().to_string_lossy();
            std::fs::copy(object, dir.join(format!("{i:03}-{file}"))).unwrap();
        }
        std::fs::write(&stamp, hash).unwrap();
    }
    let objects = list(&dir);
    if linked {
        let mut library = build.clone();
        library.objects(&objects);
        library.compile(name);
    }
    objects
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

/// Where the converters' C++ goes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Route {
    /// Linked into the native crate, built by the host's C++ compiler with exceptions on (the
    /// default).
    Native,
    /// Linked into the crate's own WebAssembly build.
    Wasi,
    /// Run as their own module under wasmtime (natively, with `contained-converters`).
    Module,
}

/// The variable cargo sets for the module route's feature.
const MODULE_FEATURE: &str = "CARGO_FEATURE_CONTAINED_CONVERTERS";

/// The converters' route, and their C++ setup, their assertions as `cpp/assert` defines them.
/// Natively linked: the host's compiler and C++ library. Otherwise wasi-sdk's clang, libc and
/// libc++, C++ exceptions off (a failure inside a converter ends its module instance). No setup for
/// the module route without the SDK, where the committed module is used.
fn base(build_dir: &Path) -> (Route, Option<cc::Build>) {
    let route = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi") {
        Route::Wasi
    } else if std::env::var_os(MODULE_FEATURE).is_some() {
        Route::Module
    } else {
        Route::Native
    };
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include("cpp/assert")
        .include(BOUNDARY)
        .warnings(false);
    if route == Route::Native {
        return (route, Some(build));
    }
    let sdk = match std::env::var("WASI_SDK_PATH") {
        Ok(sdk) => sdk,
        Err(_) if route == Route::Module => return (route, None),
        Err(_) => {
            panic!("WASI_SDK_PATH must name the wasi-sdk that builds the C++ for WebAssembly")
        }
    };
    build
        .flag("-fno-exceptions")
        .compiler(format!("{sdk}/bin/clang++"))
        .flag(format!("--sysroot={sdk}/share/wasi-sysroot"))
        .cpp_link_stdlib(None);
    if route == Route::Wasi {
        // Only libc++ is taken from the SDK; libc stays Rust's own, so the two never mix.
        let lib = format!("{sdk}/share/wasi-sysroot/lib/wasm32-wasip1");
        let out = std::env::var("OUT_DIR").unwrap();
        for name in ["libc++.a", "libc++abi.a"] {
            std::fs::copy(format!("{lib}/{name}"), format!("{out}/{name}")).unwrap();
        }
        println!("cargo:rustc-link-search=native={out}");
        println!("cargo:rustc-link-lib=static=c++");
        println!("cargo:rustc-link-lib=static=c++abi");
    } else {
        build
            .target(MODULE_TARGET)
            .opt_level(MODULE_OPTIMISATION)
            .debug(false)
            .flag([PREFIX_MAP[0], &build_dir.to_string_lossy(), PREFIX_MAP[1]].concat());
    }
    (route, Some(build))
}
