//! Gallium's values (`PIPE_*`, `MESA_SHADER_*` and the rest `build.rs` allows) and the number types
//! they are declared with, from the pinned Mesa's headers: the constants of the bindings alone, no
//! function, structure or union. The Gallium trait carries each declared value as a `u32`.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::all,
    clippy::pedantic,
    reason = "values made by bindgen, in Mesa's names"
)]

include!(concat!(env!("OUT_DIR"), "/pipe_values.rs"));
