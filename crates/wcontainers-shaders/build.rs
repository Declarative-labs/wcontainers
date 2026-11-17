//! Builds the C++ converters into the crate with the `cc` crate. The file lists come from their meson
//! and CMake files.

use std::path::Path;

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

fn main() {
    let root = Path::new("dxbc-spirv");
    assert!(
        root.join("meson.build").exists(),
        "dxbc-spirv is missing: run `git submodule update --init` in the repository"
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .flag_if_supported("-fno-exceptions")
        .warnings(false)
        .include(root)
        .define("DXBC_SPV_ENABLE_SM5", None);
    // For WebAssembly the C++ is built with wasi-sdk's clang, libc and libc++ (exceptions off).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi") {
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
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");
    for file in DXBC_SPIRV {
        build.file(root.join(file));
    }
    build.file("cpp/dxbc.cpp").compile("dxbc_spirv");
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=dxbc-spirv");
}
