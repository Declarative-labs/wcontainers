//! Builds the direct crate's C tests, only under the `c-caller` feature (the suite's job turns it on; every other build
//! compiles no C): the generated hostile C caller (tests/c_caller/caller.c) and, for a whole API, the exactly sized
//! caller and stand-in system (tests/exact/exact.c). The archive is linked into the test programs only. With
//! STAND_INS_ASAN set, the C is built with AddressSanitizer too (clang, as the harness's stand-ins are), to link with
//! the Rust toolchain's AddressSanitizer runtime.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "c-caller")]
    c_tests();
}

#[cfg(feature = "c-caller")]
fn c_tests() {
    let mut build = cc::Build::new();
    build.file("tests/c_caller/caller.c").flag("-std=c11").flag("-D_POSIX_C_SOURCE=200809L").flag("-w").opt_level(1).cargo_metadata(false);
    if std::path::Path::new("tests/exact/exact.c").exists() {
        build.file("tests/exact/exact.c");
        println!("cargo:rerun-if-changed=tests/exact/exact.c");
    }
    if std::env::var_os("STAND_INS_ASAN").is_some() {
        build.flag("-fsanitize=address").flag("-fno-omit-frame-pointer").flag("-fno-sanitize=undefined");
    }
    build.compile("direct_c_tests");
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    println!("cargo:rustc-link-arg-tests={out}/libdirect_c_tests.a");
    println!("cargo:rustc-link-arg-tests=-lpthread");
    println!("cargo:rerun-if-changed=tests/c_caller/caller.c");
    println!("cargo:rerun-if-env-changed=STAND_INS_ASAN");
    println!("cargo:rerun-if-env-changed=CC");
}
