//! Bindings of the pinned Mesa's Gallium headers (`bridge/gallium.h`), made by bindgen.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unsafe_op_in_unsafe_fn,
    unnecessary_transmutes,
    clippy::all,
    clippy::pedantic,
    reason = "bindings made by bindgen, in Mesa's names"
)]

include!(concat!(env!("OUT_DIR"), "/gallium.rs"));
