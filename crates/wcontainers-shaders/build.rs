//! Builds the C++ converters into the crate with the `cc` crate, and names the whole converter's
//! sources (the C++, this crate's Rust and the carried naga) in `WCS_CONVERTER_SOURCES` for the
//! content key. The file lists come from the converters' meson and CMake files.

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
        "dxbc-spirv is missing: run `git submodule update --init` in the repository"
    );
    let dxil = Path::new("dxil-spirv");
    assert!(
        dxil.join("third_party/spirv-headers/include").exists(),
        "dxil-spirv is missing: run `git submodule update --init --recursive crates/wcontainers-shaders/dxil-spirv`"
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
        (PathBuf::from("cpp"), true),
        (PathBuf::from("build.rs"), false),
    ];
    let mut sources = blake3::Hasher::new();
    hash_files(&mut sources, &cpp);
    let sources = sources.finalize().to_hex().to_string();
    let mut objects = blake3::Hasher::new();
    objects.update(sources.as_bytes());
    objects.update(
        std::env::var("WASI_SDK_PATH")
            .unwrap_or_default()
            .as_bytes(),
    );
    let objects = objects.finalize().to_hex().to_string();

    let mut build = base();
    build
        .std("c++17")
        .include(root)
        .define("DXBC_SPV_ENABLE_SM5", None);
    for file in DXBC_SPIRV {
        build.file(root.join(file));
    }
    build.file("cpp/dxbc.cpp");
    compile(&build, "dxbc_spirv", &objects);

    // dxil-spirv (C++14, built as C++17 as its CMake does where it reads dxbc-spirv), with its own
    // glslang SPIR-V builder, bitcode reader and SPIRV-Headers. Its bitcode module also reads
    // dxbc-spirv's IR, from our one copy of dxbc-spirv.
    let mut build = base();
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
    for file in DXIL_SPIRV {
        build.file(dxil.join(file));
    }
    build.file("cpp/dxil.cpp");
    compile(&build, "dxil_spirv", &objects);
    // Linked after both, which call it.
    let mut build = base();
    build.file("cpp/assertion.cpp");
    compile(&build, "wcs_assertion", &objects);

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
    for watched in ["cpp", "dxbc-spirv", "dxil-spirv", "src"] {
        println!("cargo:rerun-if-changed={watched}");
    }
    if naga.exists() {
        println!("cargo:rerun-if-changed={}", naga.display());
    }
}

/// Compiles `build` into the static library `name`, reusing the objects of an earlier run while
/// `hash` (of everything that makes them) is unchanged, so that a change to the Rust sources alone,
/// which reruns this script for the content key, compiles no C++.
fn compile(build: &cc::Build, name: &str, hash: &str) {
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
    let objects = if std::fs::read_to_string(&stamp).ok().as_deref() == Some(hash) {
        list(&dir)
    } else {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, object) in build.compile_intermediates().iter().enumerate() {
            let file = object.file_name().unwrap().to_string_lossy();
            std::fs::copy(object, dir.join(format!("{i:03}-{file}"))).unwrap();
        }
        std::fs::write(&stamp, hash).unwrap();
        list(&dir)
    };
    base().objects(objects).compile(name);
}

/// Hashes the files under each `(path, recursive)` (a path may be one file) by name and contents,
/// carriage returns left out, so every checkout of the same sources gives the same hash.
fn hash_files(hasher: &mut blake3::Hasher, paths: &[(PathBuf, bool)]) {
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
                } else {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    for file in files {
        let name = file.to_string_lossy().replace('\\', "/");
        let bytes: Vec<u8> = std::fs::read(&file)
            .unwrap()
            .into_iter()
            .filter(|&b| b != b'\r')
            .collect();
        hasher.update(name.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
}

/// The common C++ setup: the converters' assertions as `cpp/assert` defines them, C++ exceptions on
/// natively (so the boundary contains them), and for WebAssembly wasi-sdk's clang, libc and libc++
/// (exceptions off).
fn base() -> cc::Build {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include("cpp/assert")
        .include("cpp")
        .warnings(false);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi") {
        build.flag("-fno-exceptions");
        let sdk = std::env::var("WASI_SDK_PATH")
            .expect("WASI_SDK_PATH must name the wasi-sdk that builds the C++ for WebAssembly");
        let lib = format!("{sdk}/share/wasi-sysroot/lib/wasm32-wasip1");
        build
            .compiler(format!("{sdk}/bin/clang++"))
            .flag(format!("--sysroot={sdk}/share/wasi-sysroot"))
            .cpp_link_stdlib(None);
        // Only libc++ is taken from the SDK; libc stays Rust's own, so the two never mix.
        let out = std::env::var("OUT_DIR").unwrap();
        for name in ["libc++.a", "libc++abi.a"] {
            std::fs::copy(format!("{lib}/{name}"), format!("{out}/{name}")).unwrap();
        }
        println!("cargo:rustc-link-search=native={out}");
        println!("cargo:rustc-link-lib=static=c++");
        println!("cargo:rustc-link-lib=static=c++abi");
    }
    build
}
