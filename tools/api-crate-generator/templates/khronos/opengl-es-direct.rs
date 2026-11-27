//! OpenGL ES through the entry points EGL gives: a context made current on the calling thread before each call when
//! it is not, every command a safe method (calls.rs, generated), and the checks no registry attribute expresses:
//! pixels sized by the pixel store state, offsets only into a bound buffer, draws only with every enabled vertex
//! attribute reading a buffer, fences as values of the glue's own type.
//!
//! What the marks rely on: an OpenGL ES command reads and writes only the memory its registry entry and the
//! specification name (OpenGL ES 3.2); object names are numbers the context checks; a buffer mapped for reading is
//! neither changed nor moved by the GL until it is unmapped (§6.3); the context reads every buffer robustly (the EGL
//! glue makes every context with robust buffer access).
#![cfg(not(target_arch = "wasm32"))]

#[path = "calls.rs"]
mod calls;

use core::cell::{Cell, OnceCell, RefCell};
use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::BTreeMap;

use wcontainers_egl_direct::{Binding, Context, Display, Library, Surface};
use wcontainers_opengl_es::{CompressedPixelUnpack, OnEgl, PixelPack, PixelUnpack, constants};

/// The room the glue's own `Get` calls write into (the bindings and pixel store values it asks are one integer each).
const ROOM: usize = 64;

/// The vertex attributes checked before a draw, at most (OpenGL ES 3.2 asks for 16; no implementation gives 64).
const MOST_ATTRIBUTES: u32 = 64;

/// Each context's number, which its fences carry.
static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);

mod runtime {
    use core::ffi::CStr;

    /// The count C is told for a slice: its length over `by`, refused when that is not exact or does not fit.
    pub(crate) fn counted<T: TryFrom<usize>>(length: usize, by: usize) -> Option<T> {
        if by == 0 || !length.is_multiple_of(by) {
            return None;
        }
        T::try_from(length / by).ok()
    }

    /// Copies a text the implementation keeps (glGetString's, glGetStringi's), or None for null.
    pub(crate) fn copied_text(text: *const u8) -> Option<Vec<u8>> {
        if text.is_null() {
            return None;
        }
        #[expect(unsafe_code, reason = "gl.xml: glGetString and glGetStringi return a zero-ended text the implementation keeps, or null")]
        // SAFETY: not null, so the implementation's static zero-ended text (OpenGL ES 3.2 §20.2).
        let text = unsafe { CStr::from_ptr(text.cast()) };
        Some(text.to_bytes().to_vec())
    }
}

/// A fence sync object this context made, named by the glue: never the GL's address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fence {
    number: u64,
    context: u64,
}

/// An OpenGL ES context over an EGL context. Not `Send`: OpenGL ES calls are made on the thread the context is current
/// on, and the glue makes it current on the calling thread.
pub struct OpenGlEsContext {
    binding: Binding,
    entries: calls::Entries,
    /// The first error of the glue's refusals since the last `get_error`.
    refused: Cell<u32>,
    /// How many calls the glue itself refused since the context was made.
    refusals: Cell<u64>,
    extensions: OnceCell<Vec<Vec<u8>>>,
    fences: RefCell<BTreeMap<u64, usize>>,
    next_fence: Cell<u64>,
    number: u64,
    _thread: PhantomData<*const ()>,
}

impl OnEgl<Library> for OpenGlEsContext {
    fn new(egl: &Library, display: &Display, context: &Context, draw: Option<&Surface>, read: Option<&Surface>) -> Option<Self> {
        let binding = egl.binding(display, context, draw, read)?;
        let entries = calls::Entries::load(|name| binding.entry_address(name));
        Some(OpenGlEsContext {
            binding,
            entries,
            refused: Cell::new(constants::NO_ERROR),
            refusals: Cell::new(0),
            extensions: OnceCell::new(),
            fences: RefCell::new(BTreeMap::new()),
            next_fence: Cell::new(1),
            number: NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed),
            _thread: PhantomData,
        })
    }
}

/// The pixel store values a transfer reads (OpenGL ES 3.2 §8.4.4.1, §16.1.2).
struct Store {
    alignment: i32,
    row_length: i32,
    skip_rows: i32,
    skip_pixels: i32,
    image_height: i32,
    skip_images: i32,
}

/// The bytes of one pixel of a format and a type (OpenGL ES 3.2 tables 8.2, 8.3, 16.x), or None for a pair the glue
/// does not know: then the call is refused, never guessed.
fn pixel_bytes(format: u32, type_: u32) -> Option<usize> {
    let packed = match type_ {
        constants::UNSIGNED_SHORT_5_6_5 | constants::UNSIGNED_SHORT_4_4_4_4 | constants::UNSIGNED_SHORT_5_5_5_1 => Some(2),
        constants::UNSIGNED_INT_2_10_10_10_REV
        | constants::UNSIGNED_INT_10F_11F_11F_REV
        | constants::UNSIGNED_INT_5_9_9_9_REV
        | constants::UNSIGNED_INT_24_8 => Some(4),
        constants::FLOAT_32_UNSIGNED_INT_24_8_REV => Some(8),
        _ => None,
    };
    if packed.is_some() {
        return packed;
    }
    let component = match type_ {
        constants::UNSIGNED_BYTE | constants::BYTE => 1,
        constants::UNSIGNED_SHORT | constants::SHORT | constants::HALF_FLOAT | constants::HALF_FLOAT_OES => 2,
        constants::UNSIGNED_INT | constants::INT | constants::FLOAT => 4,
        _ => return None,
    };
    let components = match format {
        constants::RED | constants::RED_INTEGER | constants::ALPHA | constants::LUMINANCE | constants::DEPTH_COMPONENT | constants::STENCIL_INDEX => 1,
        constants::RG | constants::RG_INTEGER | constants::LUMINANCE_ALPHA => 2,
        constants::RGB | constants::RGB_INTEGER => 3,
        constants::RGBA | constants::RGBA_INTEGER | constants::BGRA_EXT => 4,
        _ => return None,
    };
    Some(component * components)
}

/// The bytes a transfer of the size reaches from its start: the end of its last row of its last image, every row
/// `alignment`-aligned and `row_length` pixels long, after the skipped images, rows and pixels.
fn transfer_bytes(width: i32, height: i32, depth: Option<i32>, pixel: usize, store: &Store) -> Option<usize> {
    let count = |value: i32| usize::try_from(value).ok();
    let (width, height) = (count(width)?, count(height)?);
    let images = match depth {
        Some(depth) => count(depth)?,
        None => 1,
    };
    if width == 0 || height == 0 || images == 0 {
        return Some(0);
    }
    let row_pixels = if store.row_length > 0 { count(store.row_length)? } else { width };
    let alignment = count(store.alignment)?.max(1);
    let stride = row_pixels.checked_mul(pixel)?.checked_next_multiple_of(alignment)?;
    let (rows_per_image, skip_images) = match depth {
        Some(_) => (if store.image_height > 0 { count(store.image_height)? } else { height }, count(store.skip_images)?),
        None => (height, 0),
    };
    let last_image = skip_images.checked_add(images - 1)?;
    let last_row = last_image.checked_mul(rows_per_image)?.checked_add(count(store.skip_rows)?)?.checked_add(height - 1)?;
    let last_row_end = count(store.skip_pixels)?.checked_add(width)?.checked_mul(pixel)?;
    last_row.checked_mul(stride)?.checked_add(last_row_end)
}

impl OpenGlEsContext {
    /// Makes the context current when it is not and returns its entry points; None (and CONTEXT_LOST recorded) when
    /// it cannot be.
    fn enter(&self) -> Option<&calls::Entries> {
        if self.binding.enter() {
            Some(&self.entries)
        } else {
            self.refuse(constants::CONTEXT_LOST);
            None
        }
    }

    /// How many calls the glue itself refused, before reaching the library, since the context was made: a test tells
    /// the glue's refusal from the library's own error by it.
    pub fn refusals(&self) -> u64 {
        self.refusals.get()
    }

    /// Records a refusal; as OpenGL ES records errors, the first is kept until `get_error`.
    fn refuse(&self, error: u32) {
        self.refusals.set(self.refusals.get().saturating_add(1));
        if self.refused.get() == constants::NO_ERROR {
            self.refused.set(error);
        }
    }

    fn take_refusal(&self) -> Option<u32> {
        Some(self.refused.replace(constants::NO_ERROR)).filter(|&error| error != constants::NO_ERROR)
    }

    /// One integer of the context's state, for the glue's own checks; 0 when it cannot be read.
    fn integer(&self, name: u32) -> i32 {
        let Some(get) = self.entries.get_integerv else { return 0 };
        let mut room = [0_i32; ROOM];
        #[expect(unsafe_code, reason = "gl.xml: glGetIntegerv writes the value of its pname; the glue asks only bindings, pixel store values and counts, one integer each, into 64 of ours")]
        // SAFETY: `room` is 64 integers of ours, more than any value the glue asks.
        unsafe { get(name, room.as_mut_ptr()) };
        room[0]
    }

    fn store(&self, names: [u32; 6]) -> Store {
        let [alignment, row_length, skip_rows, skip_pixels, image_height, skip_images] = names.map(|name| if name == 0 { 0 } else { self.integer(name) });
        Store { alignment, row_length, skip_rows, skip_pixels, image_height, skip_images }
    }

    /// The pointer a pixel-reading command is passed: the caller's bytes when no unpack buffer is bound and they hold
    /// all the call reads, else an offset when one is bound; None (and the error recorded) otherwise.
    fn unpack_pointer(&self, pixels: PixelUnpack<'_>, width: i32, height: i32, depth: Option<i32>, format: u32, type_: u32) -> Option<*const c_void> {
        let bound = self.integer(constants::PIXEL_UNPACK_BUFFER_BINDING) != 0;
        match pixels {
            PixelUnpack::Bytes(bytes) => {
                if bound {
                    self.refuse(constants::INVALID_OPERATION);
                    return None;
                }
                let store = self.store([
                    constants::UNPACK_ALIGNMENT,
                    constants::UNPACK_ROW_LENGTH,
                    constants::UNPACK_SKIP_ROWS,
                    constants::UNPACK_SKIP_PIXELS,
                    constants::UNPACK_IMAGE_HEIGHT,
                    constants::UNPACK_SKIP_IMAGES,
                ]);
                self.checked_bytes(bytes.len(), width, height, depth, format, type_, &store).then(|| bytes.as_ptr().cast())
            }
            PixelUnpack::Offset(offset) => self.offset_if(bound, offset),
        }
    }

    /// The pointer a pixel-writing command is passed, as `unpack_pointer` with the pack state and buffer.
    fn pack_pointer(&self, pixels: PixelPack<'_>, width: i32, height: i32, format: u32, type_: u32) -> Option<*mut c_void> {
        let bound = self.integer(constants::PIXEL_PACK_BUFFER_BINDING) != 0;
        match pixels {
            PixelPack::Bytes(bytes) => {
                if bound {
                    self.refuse(constants::INVALID_OPERATION);
                    return None;
                }
                let store = self.store([constants::PACK_ALIGNMENT, constants::PACK_ROW_LENGTH, constants::PACK_SKIP_ROWS, constants::PACK_SKIP_PIXELS, 0, 0]);
                self.checked_bytes(bytes.len(), width, height, None, format, type_, &store).then(|| bytes.as_mut_ptr().cast())
            }
            PixelPack::Offset(offset) => self.offset_if(bound, offset).map(<*const c_void>::cast_mut),
        }
    }

    #[expect(clippy::too_many_arguments, reason = "the transfer's size and the store it is read with")]
    fn checked_bytes(&self, held: usize, width: i32, height: i32, depth: Option<i32>, format: u32, type_: u32, store: &Store) -> bool {
        let Some(pixel) = pixel_bytes(format, type_) else {
            self.refuse(constants::INVALID_ENUM);
            return false;
        };
        match transfer_bytes(width, height, depth, pixel, store) {
            Some(needed) if needed <= held => true,
            _ => {
                self.refuse(constants::INVALID_VALUE);
                false
            }
        }
    }

    /// The pointer and size a compressed transfer is passed, as `unpack_pointer`.
    fn compressed_pointer(&self, pixels: CompressedPixelUnpack<'_>) -> Option<(*const c_void, i32)> {
        let bound = self.integer(constants::PIXEL_UNPACK_BUFFER_BINDING) != 0;
        match pixels {
            CompressedPixelUnpack::Bytes(bytes) => {
                let size = i32::try_from(bytes.len()).ok();
                match (bound, size) {
                    (false, Some(size)) => Some((bytes.as_ptr().cast(), size)),
                    _ => {
                        self.refuse(if bound { constants::INVALID_OPERATION } else { constants::INVALID_VALUE });
                        None
                    }
                }
            }
            CompressedPixelUnpack::Offset { offset, size } => self.offset_if(bound, offset).map(|pointer| (pointer, size)),
        }
    }

    /// An offset passed where C takes a pointer, only when a buffer is bound to read it from (the GL then checks the
    /// range against the buffer and never reads the caller's memory).
    fn offset_if(&self, bound: bool, offset: u64) -> Option<*const c_void> {
        match (bound, usize::try_from(offset)) {
            (true, Ok(offset)) => Some(core::ptr::without_provenance(offset)),
            _ => {
                self.refuse(constants::INVALID_OPERATION);
                None
            }
        }
    }

    /// `offset_if` for the buffer the binding names.
    fn offset_into(&self, binding: u32, offset: u64) -> Option<*const c_void> {
        self.offset_if(self.integer(binding) != 0, offset)
    }

    /// Whether every enabled vertex attribute reads a buffer: one with none would read client memory at its offset.
    fn draw_ready(&self) -> bool {
        let Some(get) = self.entries.get_vertex_attribiv else { return false };
        let attributes = u32::try_from(self.integer(constants::MAX_VERTEX_ATTRIBS)).unwrap_or(0).min(MOST_ATTRIBUTES);
        (0..attributes).all(|index| {
            let mut room = [0_i32; ROOM];
            #[expect(unsafe_code, reason = "gl.xml: glGetVertexAttribiv writes at most 4 integers (len 4) into 64 of ours")]
            // SAFETY: `room` is 64 integers of ours.
            unsafe { get(index, constants::VERTEX_ATTRIB_ARRAY_ENABLED, room.as_mut_ptr()) };
            if room[0] == 0 {
                return true;
            }
            #[expect(unsafe_code, reason = "gl.xml: glGetVertexAttribiv writes at most 4 integers (len 4) into 64 of ours")]
            // SAFETY: `room` is 64 integers of ours.
            unsafe { get(index, constants::VERTEX_ATTRIB_ARRAY_BUFFER_BINDING, room.as_mut_ptr()) };
            room[0] != 0
        })
    }

    /// Whether the context names any of the extensions (read once, by glGetStringi).
    fn has_extension(&self, names: &[&CStr]) -> bool {
        let extensions = self.extensions.get_or_init(|| {
            let count = u32::try_from(self.integer(constants::NUM_EXTENSIONS)).unwrap_or(0);
            let Some(get) = self.entries.get_stringi else { return Vec::new() };
            (0..count)
                .filter_map(|index| {
                    #[expect(unsafe_code, reason = "gl.xml: glGetStringi takes two values and returns a zero-ended text or null")]
                    // SAFETY: no pointer is passed.
                    let text = unsafe { get(constants::EXTENSIONS, index) };
                    runtime::copied_text(text)
                })
                .collect()
        });
        names.iter().any(|name| extensions.iter().any(|extension| extension.as_slice() == name.to_bytes()))
    }

    fn fence_address(&self, fence: &Fence) -> Option<*mut c_void> {
        if fence.context != self.number {
            return None;
        }
        self.fences.borrow().get(&fence.number).map(|&address| core::ptr::with_exposed_provenance_mut(address))
    }

    fn keep_fence(&self, address: *mut c_void) -> Option<Fence> {
        if address.is_null() {
            return None;
        }
        let number = self.next_fence.get();
        self.next_fence.set(number + 1);
        self.fences.borrow_mut().insert(number, address.expose_provenance());
        Some(Fence { number, context: self.number })
    }

    fn forget_fence(&self, fence: &Fence) {
        self.fences.borrow_mut().remove(&fence.number);
    }

    /// map_buffer_range_read: maps the range for reading only, copies it into `data` and unmaps.
    fn map_buffer_range_read_inside(&self, target: u32, offset: i64, data: &mut [u8]) -> bool {
        let Some(entries) = self.enter() else { return false };
        let (Some(map), Some(unmap)) = (entries.map_buffer_range, entries.unmap_buffer) else {
            self.refuse(constants::INVALID_OPERATION);
            return false;
        };
        let (Ok(offset), Ok(length)) = (isize::try_from(offset), isize::try_from(data.len())) else {
            self.refuse(constants::INVALID_VALUE);
            return false;
        };
        if data.is_empty() {
            return true;
        }
        #[expect(unsafe_code, reason = "gl.xml: glMapBufferRange takes values only and returns the mapped range or null")]
        // SAFETY: no pointer is passed.
        let mapped = unsafe { map(target, offset, length, constants::MAP_READ_BIT) };
        if mapped.is_null() {
            return false;
        }
        #[expect(unsafe_code, reason = "OpenGL ES 3.2 §6.3: a successful glMapBufferRange returns length readable bytes, which the GL neither changes nor moves until glUnmapBuffer (no UNSYNCHRONIZED or PERSISTENT bit asked)")]
        // SAFETY: `mapped` is `data.len()` readable bytes until the unmap below; `data` is the caller's, so the two do not overlap.
        unsafe { core::ptr::copy_nonoverlapping(mapped.cast::<u8>().cast_const(), data.as_mut_ptr(), data.len()) };
        #[expect(unsafe_code, reason = "gl.xml: glUnmapBuffer takes a value and ends the mapping; the glue holds no pointer into it after")]
        // SAFETY: no pointer is passed; `mapped` is not used again.
        let kept = unsafe { unmap(target) };
        kept != 0
    }
}
