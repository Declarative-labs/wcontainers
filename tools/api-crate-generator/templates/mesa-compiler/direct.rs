//! Mesa's compiler through our C bridge (crates/mesa-compiler/implementation/direct/bridge): every bridge function a safe method of
//! wcontainers-mesa-compiler's trait, each shader a value of the type of its place in the bridge's typed chain, owning
//! one NIR shader and giving it back when dropped, and every structure the bridge fills copied out whole into owned
//! values before the bridge's free.
//!
//! What the marks rely on: the bridge's functions have the types bridge.h and flat.h declare; a function reads and
//! writes only what its parameters name, as far as the count beside each says (or the header's array length); the
//! arrays and texts it fills are its own until the matching free, which takes a zeroed structure as nothing; a shader
//! it returns or fills is a shader it made, of the type it names, given back once by that type's free; a function
//! taking a shader named `taken` gives it back itself; Mesa's instruction names are static. The bridge's C is built
//! and linked by this crate's build.rs.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use wcontainers_mesa_compiler::*;

/// The compiler: Mesa's type tables taken once for the process.
#[derive(Clone, Copy, Debug)]
pub struct Bridge {
    _private: (),
}

/// Whether the type tables were taken.
static STARTED: std::sync::Once = std::sync::Once::new();

impl Bridge {
    fn started() -> Bridge {
        STARTED.call_once(|| {
            #[expect(unsafe_code, reason = "bridge.h: wc_init takes nothing and takes a reference on Mesa's type tables")]
            // SAFETY: no pointer is passed; called once for the process.
            unsafe { wc_init() };
        });
        Bridge { _private: () }
    }
}

/// A shader type of the glue: made from a shader the bridge made.
trait Owning: Sized {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<Self>;
}

/// Copies the `count` elements the bridge holds at `data`; none when it is null or the count does not fit.
fn array<T: Copy, C: TryInto<usize>>(data: *mut T, count: C) -> Vec<T> {
    let Ok(count) = count.try_into() else { return Vec::new() };
    if data.is_null() || count == 0 {
        return Vec::new();
    }
    #[expect(unsafe_code, reason = "bridge.h and flat.h: each array the bridge fills holds the count beside it, and is the bridge's until its free")]
    // SAFETY: not null; `count` elements of T, the bridge's, unchanged until the free that follows the copy.
    let elements = unsafe { core::slice::from_raw_parts(data, count) };
    elements.to_vec()
}

/// Copies a zero-ended text the bridge owns, or Mesa keeps; None for null.
fn text(data: *mut u8) -> Option<Vec<u8>> {
    if data.is_null() {
        return None;
    }
    #[expect(unsafe_code, reason = "bridge.h and flat.h: every text the bridge fills is zero-ended (its own until its free, or Mesa's static instruction names)")]
    // SAFETY: not null and zero-ended, unchanged until the free that follows the copy.
    let text = unsafe { core::ffi::CStr::from_ptr(data.cast()) };
    Some(text.to_bytes().to_vec())
}
