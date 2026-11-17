//! Builds the C++ converters into the crate with the `cc` crate, and names their sources' contents in
//! `WCS_CONVERTER_SOURCES` for the content key. The file lists come from their meson and CMake files.

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
    let mut build = base();
    build
        .std("c++17")
        .include(root)
        .define("DXBC_SPV_ENABLE_SM5", None);
    for file in DXBC_SPIRV {
        build.file(root.join(file));
    }
    build.file("cpp/dxbc.cpp").compile("dxbc_spirv");

    // dxil-spirv (C++14, built as C++17 as its CMake does where it reads dxbc-spirv), with its own glslang SPIR-V builder, bitcode reader and SPIRV-Headers. Its
    // bitcode module also reads dxbc-spirv's IR, from our one copy of dxbc-spirv.
    let dxil = Path::new("dxil-spirv");
    assert!(
        dxil.join("third_party/spirv-headers/include").exists(),
        "dxil-spirv is missing: run `git submodule update --init --recursive crates/wcontainers-shaders/dxil-spirv`"
    );
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
    build.file("cpp/dxil.cpp").compile("dxil_spirv");
    // Linked after both, which call it.
    base().file("cpp/assertion.cpp").compile("wcs_assertion");

    let sources = [
        root.join("dxbc"),
        root.join("ir"),
        root.join("util"),
        dxil.to_path_buf(),
        dxil.join("bc"),
        dxil.join("debug"),
        dxil.join("util"),
        dxil.join("opcodes"),
        dxil.join("third_party/glslang-spirv"),
        dxil.join("third_party/bc-decoder"),
        dxil.join("third_party/spirv-headers/include/spirv/unified1"),
        PathBuf::from("cpp"),
    ];
    println!(
        "cargo:rustc-env=WCS_CONVERTER_SOURCES={}",
        contents_hash(&sources)
    );
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=dxbc-spirv");
    println!("cargo:rerun-if-changed=dxil-spirv");
}

/// BLAKE3 over the C++ sources under `dirs` (those directories' own files, and the whole tree of
/// `opcodes`), by path and contents with carriage returns left out, so every checkout of the same
/// sources names the same converter.
fn contents_hash(dirs: &[PathBuf]) -> String {
    let mut files = Vec::new();
    for dir in dirs {
        let recursive = dir.ends_with("opcodes");
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if recursive {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|e| {
                    ["c", "cpp", "h", "hpp", "inc"].contains(&&*e.to_string_lossy())
                }) || path.parent() == Some(Path::new("cpp/assert"))
                {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    let mut hasher = blake3::Hasher::new();
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
    hasher.finalize().to_hex().to_string()
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
