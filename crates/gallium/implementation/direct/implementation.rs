// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Gallium through Mesa's C ABI: the serving glue that takes Mesa's calls into a driver and calls any implementation
//! of wcontainers-gallium's traits. Mesa's structures reach Rust only as the flat copies the glue's C makes (flat.h,
//! flat.c: no bit-field, no union, every object its address as a key), so Rust reads no layout of Mesa's; an object is
//! a key the glue looks up, never a pointer Rust follows.
//!
//! What the marks rely on: flat.c is compiled against the pinned Mesa's headers by this crate's build.rs and its flat
//! structures have the layout their Rust mirrors have (each size checked by `layouts_agree`); `wc_flat_size` reads
//! nothing.
//!
//! Built so far: the plain data (flat.rs, held.rs). The calls (the screen's and the context's pointers, the objects
//! Mesa sees, the registry) come next.

pub mod flat;
pub mod held;

#[cfg(mesa)]
#[expect(unsafe_code, reason = "flat.c defines wc_flat_size as declared here, reading nothing")]
unsafe extern "C" {
    fn wc_flat_size(which: u32) -> usize;
}

/// Whether every flat structure has the same size in C as its Rust mirror; the first that does not, with both sizes.
#[cfg(mesa)]
pub fn layouts_agree() -> Result<usize, (u32, usize, usize)> {
    for &(which, size) in flat::SIZES {
        #[expect(unsafe_code, reason = "flat.c: wc_flat_size takes a number and returns a sizeof, reading and writing nothing")]
        // SAFETY: a plain call of our own C function with a number.
        let c_size = unsafe { wc_flat_size(which) };
        if c_size != size {
            return Err((which, c_size, size));
        }
    }
    Ok(flat::SIZES.len())
}
