//! Builds the C++ converters with the `cc` crate and links them into the crate, natively with the host's compiler and
//! in the crate's own WebAssembly build with wasi-sdk's. Natively with `WASI_SDK_PATH` set, also builds them as a
//! module of their own, which the `contained` implementation runs under wasmtime, and checks that its committed
//! `converters.wasm` and `converters.blake3` are this build of these sources. Gives the hash of the sources to the
//! crates that depend on this one (`DEP_WCS_LINKED_SOURCES`), and checks that the interface's slots and export names
//! are the boundary's. The file lists come from the converters' meson and CMake files.

use std::path::{Path, PathBuf};

use wcontainers_shader_converters::{
    ALLOCATE, EXPORT_NAMES, FREE, SLOT_COUNT, SLOT_FIRST, SLOT_FIRST_COUNT, SLOT_REASON,
    SLOT_REASON_LENGTH, SLOT_SECOND, SLOT_SECOND_COUNT,
};

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

/// The directory of this crate's own C++ boundary.
const BOUNDARY: &str = "cpp";

/// The boundary's header, which declares its slots.
const BOUNDARY_HEADER: &str = "cpp/boundary.h";

/// The boundary's sources, which export its functions.
const BOUNDARY_SOURCES: [&str; 2] = ["cpp/dxbc.cpp", "cpp/dxil.cpp"];

/// The enum of the boundary's slots, and each of its values as the interface names it.
const SLOT_ENUM: &str = "enum class Slot";
const SLOTS: [(&str, usize); 7] = [
    ("eFirst", SLOT_FIRST),
    ("eFirstCount", SLOT_FIRST_COUNT),
    ("eSecond", SLOT_SECOND),
    ("eSecondCount", SLOT_SECOND_COUNT),
    ("eReason", SLOT_REASON),
    ("eReasonLength", SLOT_REASON_LENGTH),
    ("eCount", SLOT_COUNT),
];

/// The folder of the implementation that runs the module, beside this one.
const CONTAINED: &str = "../contained";

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
        "{0} is missing: run `git submodule update --init --recursive crates/shader-converters/implementation/linked/{0}`",
        dxil.display()
    );
    check_boundary();
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
    let sdk = std::env::var("WASI_SDK_PATH").ok();
    let wasi = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi");
    let route = if wasi { Route::Wasi } else { Route::Native };
    let linked = base(route, sdk.as_deref(), &build_dir);
    build(
        root,
        dxil,
        &linked,
        &build_dir,
        &objects_hash(&sources, route, sdk.as_deref()),
        true,
    );
    if let (Route::Native, Some(sdk)) = (route, sdk.as_deref()) {
        let base = base(Route::Module, Some(sdk), &build_dir);
        let hash = objects_hash(&sources, Route::Module, Some(sdk));
        let libraries = build(root, dxil, &base, &build_dir, &hash, false);
        check_module(&base, &libraries, &build_dir, &sources);
    }

    println!("cargo:sources={sources}");
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");
    for watched in [BOUNDARY, PATCHES, "dxbc-spirv", "dxil-spirv", CONTAINED] {
        println!("cargo:rerun-if-changed={watched}");
    }
}

/// Returns the hash of everything that makes a route's objects: the sources, the route and the SDK.
fn objects_hash(sources: &str, route: Route, sdk: Option<&str>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(sources.as_bytes());
    hasher.update(format!("{route:?}").as_bytes());
    hasher.update(sdk.unwrap_or_default().as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Compiles both converters and the assertion handler with `base`, as libraries of the crate when `linked`; returns
/// each library's objects.
fn build(
    root: &Path,
    dxil: &Path,
    base: &cc::Build,
    build_dir: &Path,
    objects_hash: &str,
    linked: bool,
) -> Vec<Vec<PathBuf>> {
    let prefix = if linked { "" } else { MODULE_OBJECTS };
    let mut libraries = Vec::new();
    let mut build = base.clone();
    build
        .std("c++17")
        .include(root)
        .define("DXBC_SPV_ENABLE_SM5", None);
    add_sources(&mut build, build_dir, root, DXBC_SPIRV);
    build.file("cpp/dxbc.cpp");
    libraries.push(compile(
        &build,
        &format!("{prefix}dxbc_spirv"),
        objects_hash,
        linked,
    ));

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
    add_sources(&mut build, build_dir, dxil, DXIL_SPIRV);
    build.file("cpp/dxil.cpp");
    libraries.push(compile(
        &build,
        &format!("{prefix}dxil_spirv"),
        objects_hash,
        linked,
    ));
    // Linked after both, which call it.
    let mut build = base.clone();
    build.file("cpp/assertion.cpp");
    libraries.push(compile(
        &build,
        &format!("{prefix}wcs_assertion"),
        objects_hash,
        linked,
    ));
    libraries
}

/// Links the module from its objects and checks that the contained implementation's committed module and hash are
/// this build of these sources.
fn check_module(base: &cc::Build, libraries: &[Vec<PathBuf>], build_dir: &Path, sources: &str) {
    let out = build_dir.join(MODULE);
    let mut link = base.get_compiler().to_command();
    link.args(MODULE_LINK)
        .arg(&out)
        .args(libraries.iter().flatten());
    let status = link.status().expect("the SDK's clang++ runs");
    assert!(status.success(), "linking {MODULE} failed: {link:?}");
    let committed = Path::new(CONTAINED);
    let recorded = std::fs::read_to_string(committed.join(MODULE_SOURCES)).unwrap_or_default();
    let same = std::fs::read(committed.join(MODULE)).ok() == Some(std::fs::read(&out).unwrap());
    std::fs::write(build_dir.join(MODULE_SOURCES), sources).unwrap();
    assert!(
        same && recorded.trim() == sources,
        "the committed {MODULE} is not this build of its sources: copy {} and {} into {CONTAINED}",
        out.display(),
        build_dir.join(MODULE_SOURCES).display()
    );
}

/// Checks that the interface's slots are the boundary header's, in its order, and that every function the interface
/// names is exported by the boundary's sources.
fn check_boundary() {
    let header = std::fs::read_to_string(BOUNDARY_HEADER).unwrap();
    let start = header
        .find(SLOT_ENUM)
        .unwrap_or_else(|| panic!("{BOUNDARY_HEADER}: no {SLOT_ENUM}"));
    let body = &header[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("};").unwrap()];
    let declared: Vec<&str> = body
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    let named: Vec<&str> = SLOTS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        declared, named,
        "{BOUNDARY_HEADER}'s slots are not the interface's"
    );
    for (place, (name, value)) in SLOTS.iter().enumerate() {
        assert_eq!(
            place, *value,
            "the interface places slot {name} at {value}, the boundary at {place}"
        );
    }
    let exported: String = BOUNDARY_SOURCES
        .iter()
        .map(|file| std::fs::read_to_string(file).unwrap())
        .collect();
    for name in EXPORT_NAMES.iter().chain(&[FREE, ALLOCATE]) {
        assert!(
            exported.contains(&format!("WCS_EXPORT({name})")),
            "the boundary exports no {name}"
        );
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
    /// Linked into the native crate, built by the host's C++ compiler with exceptions on.
    Native,
    /// Linked into the crate's own WebAssembly build.
    Wasi,
    /// Built as the converters' own module, which the contained implementation runs.
    Module,
}

/// The prefix of the names under which the module's objects are kept, apart from the linked ones.
const MODULE_OBJECTS: &str = "module-";

/// The converters' C++ setup for `route`, their assertions as `cpp/assert` defines them. Natively
/// linked: the host's compiler and C++ library. Otherwise wasi-sdk's clang, libc and libc++, C++
/// exceptions off (a failure inside a converter ends its module instance).
fn base(route: Route, sdk: Option<&str>, build_dir: &Path) -> cc::Build {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include("cpp/assert")
        .include(BOUNDARY)
        .warnings(false);
    if route == Route::Native {
        return build;
    }
    let sdk =
        sdk.expect("WASI_SDK_PATH must name the wasi-sdk that builds the C++ for WebAssembly");
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
    build
}
