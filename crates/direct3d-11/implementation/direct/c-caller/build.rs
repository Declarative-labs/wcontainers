//! Builds the generated hostile C caller. With STAND_INS_ASAN set, the C is built with AddressSanitizer too (clang, as
//! the harness's stand-ins are), to link with the Rust toolchain's AddressSanitizer runtime.

fn main() {
    let mut build = cc::Build::new();
    build
        .file("caller.c")
        .flag("-std=c11")
        .flag("-D_POSIX_C_SOURCE=200809L")
        .flag("-w")
        .opt_level(1);
    if std::env::var_os("STAND_INS_ASAN").is_some() {
        build
            .flag("-fsanitize=address")
            .flag("-fno-omit-frame-pointer")
            .flag("-fno-sanitize=undefined");
    }
    build.compile("hostile_c_caller");
    println!("cargo:rerun-if-changed=caller.c");
    println!("cargo:rerun-if-env-changed=STAND_INS_ASAN");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rustc-link-lib=pthread");
}
