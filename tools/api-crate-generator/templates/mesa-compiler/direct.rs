//! Mesa's compiler through our C bridge (crates/nir/bridge): every bridge function a safe method of
//! wcontainers-mesa-compiler's trait, each shader a value of `Shader` that owns one NIR shader and gives it back when
//! dropped, and every structure the bridge fills copied out whole into owned values before the bridge's free.
//!
//! What the marks rely on: the bridge's functions have the types bridge.h and flat.h declare; a function reads and
//! writes only what its parameters name, as far as the count beside each says (or the header's array length); the
//! arrays and texts it fills are its own until the matching free, which takes a zeroed structure as nothing; a shader
//! it returns or fills is a nir_shader it made, given back once by wc_nir_free; Mesa's instruction names are static.
//! The bridge's C is linked by the crate that builds it (crates/nir's build today; see the status for where it moves).

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

/// One NIR shader the bridge made, given back when dropped. Not `Send`: no fact of the bridge's is relied on for a
/// shader moved between threads (NEEDED B1 leaves it to this crate; it may be offered once measured).
#[derive(Debug)]
pub struct Shader {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl Shader {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<Shader> {
        NonNull::new(nir).map(|nir| Shader { nir, _thread: PhantomData })
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_free gives back a shader the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_nir_free(self.nir.as_ptr()) };
    }
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
