// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Gallium through Mesa's C ABI: the serving glue that takes Mesa's calls into a driver and calls any implementation
//! of wcontainers-gallium's traits. Mesa's structures reach Rust only as the flat copies the glue's C makes (flat.h,
//! flat.c: no bit-field, no union, every object its address as a key), so Rust reads no layout of Mesa's; an object is
//! a key the glue looks up, never a pointer Rust follows.
//!
//! What the marks rely on: flat.c is compiled against the pinned Mesa's headers by this crate's build.rs and its flat
//! structures have the layout their Rust mirrors have (each size checked by `layouts_agree`, each field's offset and
//! size by `fields_agree`); `wc_flat_size` and `wc_flat_layout` read nothing.
//!
//! Built so far: the plain data (flat.rs, held.rs). The calls (the screen's and the context's pointers, the objects
//! Mesa sees, the registry) come next.

pub mod flat;
pub mod held;

#[cfg(mesa)]
#[expect(unsafe_code, reason = "flat.c defines wc_flat_size, wc_flat_layout and wc_value as declared here, each reading nothing")]
unsafe extern "C" {
    fn wc_flat_size(which: u32) -> usize;
    fn wc_flat_layout(which: u32, what: u32) -> usize;
    fn wc_value(which: u32) -> i64;
}

/// Whether every declared value of the trait (values.rs, the generator's reading of the headers) is the value C
/// computes from Mesa's headers; the first that is not, with both. Returns how many were compared.
#[cfg(mesa)]
pub fn values_agree() -> Result<usize, (&'static str, i64, i64)> {
    let values = flat::values();
    for (which, (name, value)) in (0_u32..).zip(&values) {
        #[expect(unsafe_code, reason = "flat.c: wc_value takes a number and returns a constant of Mesa's headers, reading and writing nothing")]
        // SAFETY: a plain call of our own C function with a number.
        let c_value = unsafe { wc_value(which) };
        if c_value != *value {
            return Err((name, c_value, *value));
        }
    }
    Ok(values.len())
}

/// One field of a flat form as the generator wrote it (flat.rs's FIELDS).
#[derive(Clone, Copy, Debug)]
pub struct FieldLayout {
    /// Mesa's structure holding it (`pipe_draw_info.index` for a nested one).
    pub structure: &'static str,
    /// Its name in Mesa's structure and in the flat form.
    pub field: &'static str,
    /// Its offset in the Rust mirror.
    pub flat_offset: usize,
    /// Its size in the Rust mirror.
    pub flat_size: usize,
    /// Its width where Mesa declares it a bit-field, else 0.
    pub bits: u64,
    /// Whether its size in Mesa's structure must be its size in the flat form.
    pub same_size: bool,
    /// Whether, of a `same_size` field, the flat form's size need only be at least Mesa's (an enumeration).
    pub at_least: bool,
}

/// One field as both compilers lay it out: the Rust mirror's offset and size, and C's for the flat form and for Mesa's
/// own structure (None for a bit-field, which has no offset in C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldMeasured {
    /// Its offset in Mesa's structure (from the structure holding it, for a nested one), by C's offsetof.
    pub mesa_offset: Option<usize>,
    /// Its size in Mesa's structure, by C's sizeof.
    pub mesa_size: Option<usize>,
    /// Its offset in the flat form, by C's offsetof.
    pub flat_offset: usize,
    /// Its size in the flat form, by C's sizeof.
    pub flat_size: usize,
}

/// C's bit-field answer and its answer for a field it does not list (flat.c's WC_BIT_FIELD and WC_NO_FIELD).
#[cfg(mesa)]
const BIT_FIELD: usize = usize::MAX;
#[cfg(mesa)]
const NO_FIELD: usize = usize::MAX - 1;

/// Every field of every flat form measured by C: each with its offset and size in the flat form equal to the Rust
/// mirror's, and where its size in Mesa must be its size here, equal; the first field that differs, with what C says.
#[cfg(mesa)]
pub fn fields_agree() -> Result<Vec<(FieldLayout, FieldMeasured)>, (FieldLayout, FieldMeasured)> {
    let mut out = Vec::new();
    for (which, layout) in (0_u32..).zip(flat::FIELDS) {
        let mut answers = [0_usize; 4];
        for (what, answer) in (0_u32..).zip(answers.iter_mut()) {
            #[expect(unsafe_code, reason = "flat.c: wc_flat_layout takes two numbers and returns an offsetof or a sizeof, reading and writing nothing")]
            // SAFETY: a plain call of our own C function with two numbers.
            let measured = unsafe { wc_flat_layout(which, what) };
            *answer = measured;
        }
        let [flat_offset, flat_size, mesa_offset, mesa_size] = answers;
        let measured = FieldMeasured {
            mesa_offset: (mesa_offset != BIT_FIELD).then_some(mesa_offset),
            mesa_size: (mesa_size != BIT_FIELD).then_some(mesa_size),
            flat_offset,
            flat_size,
        };
        let bit_field = layout.bits != 0;
        let agrees = !answers.contains(&NO_FIELD)
            && flat_offset == layout.flat_offset
            && flat_size == layout.flat_size
            && bit_field == measured.mesa_offset.is_none()
            && bit_field == measured.mesa_size.is_none()
            && (!layout.same_size || measured.mesa_size.is_some_and(|size| if layout.at_least { size <= flat_size } else { size == flat_size }))
            && (!bit_field || layout.bits <= u64::try_from(flat_size).unwrap_or(0) * u64::from(u8::BITS));
        if !agrees {
            return Err((*layout, measured));
        }
        out.push((*layout, measured));
    }
    Ok(out)
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
