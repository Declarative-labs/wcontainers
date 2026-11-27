//! Makes Rust bindings of the pinned Mesa's Gallium headers when `WCONTAINERS_MESA` names its Meson
//! build folder; without it the crate builds without the driver.

use std::env;
use std::path::PathBuf;

#[path = "../nir/mesa/meson_build.rs"]
mod meson_build;
use meson_build::{
    CFG, CHECK_CFG, MESA_VARIABLE, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, mesa_defines,
    mesa_flags, source_folder,
};

/// The Gallium headers the driver is written against, and the bindings made of them in `OUT_DIR`
/// (`src/gallium.rs` includes them from there by their file name).
const GALLIUM_HEADER: &str = "bridge/gallium.h";
/// The bit fields bindgen cannot lay out as Visual C++ does, read in C (`bridge/fields.h`).
const FIELDS: &str = "bridge/fields.c";
const FIELDS_HEADER: &str = "bridge/fields.h";
const FIELDS_LIBRARY: &str = "wcontainers_opengl_fields";
const BINDINGS: &str = "gallium.rs";
/// The bindings' constants and the plain number types they are declared with, alone: Gallium's
/// values, which the driver on the Gallium trait names (`src/pipe.rs` includes them). They hold no
/// function, structure or union.
const VALUES: &str = "pipe_values.rs";
/// The lines of the bindings `VALUES` keeps: constants, and type names of C's integer types.
const VALUE_LINE: &str = "pub const ";
const NUMBER_TYPE_LINE: &str = "pub type ";
const C_NUMBER_TYPE: &str = " = ::std::os::raw::c_";
const STATEMENT_END: char = ';';
/// What the bindings hold: Gallium's types and constants, and the functions of Mesa's that the
/// driver calls.
const ALLOWED_TYPES: &str = "pipe_.*|util_format_description|blitter_context|u_transfer_vtbl";
const ALLOWED_FUNCTIONS: &str = "u_init_pipe_screen_caps|u_upload_create_default|u_upload_destroy|\
     util_format_description|util_copy_framebuffer_state|util_blitter_(create|destroy|blit|is_blit_supported|stencil_fallback|clear_depth_stencil|default_dst_texture)|\
     wc_sampler_view_.*|tgsi_to_nir|u_index_(translator|generator)|util_prim_restart_convert_to_direct|\
     u_default_get_sample_position|u_default_texture_subdata|\
     u_transfer_helper_(create|destroy|transfer_map|transfer_unmap|transfer_flush_region)";
const ALLOWED_CONSTANTS: &str = "PIPE_.*|MESA_SHADER_.*|MESA_PRIM_.*|PV_(FIRST|LAST)|PR_DISABLE|U_TRANSLATE_.*|\
     U_TRANSFER_HELPER_.*|MAX_GEOMETRY_(OUTPUT_VERTICES|TOTAL_OUTPUT_COMPONENTS|SHADER_INVOCATIONS)";
/// The Gallium file whose defines (and, for `FIELDS`, C standard) the bindings take from Mesa's
/// compile commands.
const FLAGS_FROM: &str = "u_screen.c";
const CLANG_HEADER_FOLDER: &str = "-I";
/// Mesa's atomics for the Visual C++ compiler call intrinsics libclang does not declare there; they
/// sit in inline functions' bodies, which the bindings never hold.
const UNDECLARED_CALLS_ALLOWED: &str = "-Wno-error=implicit-function-declaration";

/// Folders of the Mesa tree, each under both its source and its build folder (generated headers).
const INCLUDES: &[&str] = &[
    "include",
    "src",
    "src/gallium/include",
    "src/gallium/auxiliary",
    "src/util",
];

fn main() {
    println!("{CHECK_CFG}");
    println!("{RERUN_IF_ENV_CHANGED}{MESA_VARIABLE}");
    let Some(build) = env::var_os(MESA_VARIABLE).map(PathBuf::from) else {
        return;
    };
    for file in [GALLIUM_HEADER, FIELDS, FIELDS_HEADER] {
        println!("{RERUN_IF_CHANGED}{file}");
    }
    let roots = [source_folder(&build), build.clone()];
    let folders: Vec<PathBuf> = INCLUDES
        .iter()
        .flat_map(|folder| roots.iter().map(move |root| root.join(folder)))
        .collect();
    let includes = folders
        .iter()
        .map(|folder| format!("{CLANG_HEADER_FOLDER}{}", folder.display()));
    let defines = mesa_defines(&build, FLAGS_FROM);
    let mut fields = cc::Build::new();
    // Mesa's headers are not written for the compiler's extra warnings.
    fields.file(FIELDS).includes(&folders).warnings(false);
    // Mesa's defines and C standard, as it compiles the same file.
    for flag in mesa_flags(&build, FLAGS_FROM) {
        fields.flag(&flag);
    }
    fields.compile(FIELDS_LIBRARY);
    let bindings = bindgen::Builder::default()
        .header(GALLIUM_HEADER)
        .clang_args(includes.chain(defines))
        .clang_arg(UNDECLARED_CALLS_ALLOWED)
        .allowlist_type(ALLOWED_TYPES)
        .allowlist_function(ALLOWED_FUNCTIONS)
        .allowlist_var(ALLOWED_CONSTANTS)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        .layout_tests(false)
        // Mesa's comments hold C examples, which rustdoc would run as Rust.
        .generate_comments(false)
        .generate()
        .expect("bindgen reads Mesa's Gallium headers");
    let file = build_rs::input::out_dir().join(BINDINGS);
    bindings
        .write_to_file(&file)
        .expect("the bindings are written");
    let text = bindings.to_string();
    let mut values = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let kept = line.starts_with(VALUE_LINE)
            || (line.starts_with(NUMBER_TYPE_LINE) && line.contains(C_NUMBER_TYPE));
        if !kept {
            continue;
        }
        // A long constant goes on over several lines, to its semicolon.
        values.push_str(line);
        let mut last = line;
        while !last.trim_end().ends_with(STATEMENT_END) {
            let Some(more) = lines.next() else { break };
            values.push(' ');
            values.push_str(more.trim());
            last = more;
        }
        values.push('\n');
    }
    std::fs::write(build_rs::input::out_dir().join(VALUES), values)
        .expect("the values are written");
    println!("{CFG}");
}
