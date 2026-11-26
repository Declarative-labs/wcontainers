//! Builds the stand-ins' C: the fake system Direct3D, the game, the attacks of both checks, and the fuzzing drivers.
//! With STAND_INS_ASAN set, the C is built with AddressSanitizer too, by `zig cc` (clang), to link with the Rust
//! toolchain's AddressSanitizer runtime.

fn main() {
    let mut build = cc::Build::new();
    build
        .file("c/fake_d3d11.c")
        .file("c/game.c")
        .file("c/attack.c")
        .file("c/check2.c")
        .file("c/fuzz.c")
        .flag("-std=c11")
        .flag("-D_POSIX_C_SOURCE=200809L")
        .opt_level(2);
    if std::env::var_os("STAND_INS_ASAN").is_some() {
        build.flag("-fsanitize=address").flag("-fno-omit-frame-pointer").flag("-fno-sanitize=undefined");
    }
    build.compile("stand_ins");
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-env-changed=STAND_INS_ASAN");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rustc-link-lib=pthread");
}
