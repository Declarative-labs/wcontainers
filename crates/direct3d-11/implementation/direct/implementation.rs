//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The library d3d11.dll, and the debug layer's interfaces its objects answer through its C ABI: the serving glue (raw C calls in, any implementation of the traits out) and the
//! calling glue (the C side's objects as an implementation of the traits). Every `unsafe` of the API is here, each marked.

#[path = "abi.rs"]
pub mod abi;
#[path = "call.rs"]
mod call;
#[path = "data.rs"]
mod data;
#[path = "lengths.rs"]
mod lengths;
#[cfg(test)]
#[path = "lengths_reference.rs"]
mod lengths_reference;
#[cfg(test)]
#[path = "lengths_tests.rs"]
mod lengths_tests;
#[path = "serve.rs"]
mod serve;

#[allow(
    unused_imports,
    reason = "what another crate of the set names is public; a module may have none"
)]
pub use abi::*;
#[allow(
    unused_imports,
    reason = "what another crate of the set names is public; a module may have none"
)]
pub use call::*;
#[allow(
    unused_imports,
    reason = "what another crate of the set names is public; a module may have none"
)]
pub use serve::*;

use wcontainers_component_object_model_direct::GlueKey;

#[expect(
    unsafe_code,
    reason = "the glue's key, which only this crate's glue holds"
)]
// SAFETY: kept in this private static and passed only to the COM crate's glue functions.
static KEY: GlueKey = unsafe { GlueKey::new() };
