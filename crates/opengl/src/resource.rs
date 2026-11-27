//! Gallium's resources on the interface's buffers and textures, their references, and the maps and
//! writes that move bytes in and out of them.

use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{Gpu, format::Format, resource::TextureDesc};

use crate::format;
use crate::gallium::*;

/// What a resource holds on the back end.
pub enum Storage<B: Gpu> {
    Buffer(B::Buffer),
    Texture {
        texture: B::Texture,
        desc: TextureDesc,
    },
    /// A multisampled array texture, a 2D resource of this driver a layer, each holding one
    /// reference: the host's multisampled textures are single layers.
    Layers(Vec<*mut pipe_resource>),
}

/// One resource: Gallium's struct first, so that a `pipe_resource` pointer is one to it.
#[repr(C)]
pub struct Resource<B: Gpu> {
    pub base: pipe_resource,
    pub storage: Storage<B>,
}

impl<B: Gpu> Resource<B> {
    /// # Safety
    /// `resource` is one this driver made, alive for `'a`.
    pub unsafe fn of<'a>(resource: *mut pipe_resource) -> &'a Resource<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*resource.cast::<Resource<B>>() }
    }

    pub fn buffer(&self) -> Option<&B::Buffer> {
        match &self.storage {
            Storage::Buffer(buffer) => Some(buffer),
            Storage::Texture { .. } | Storage::Layers(_) => None,
        }
    }

    /// The texture, none for a buffer or a texture held a layer a resource ([`Resource::layer`]).
    pub fn texture(&self) -> Option<(&B::Texture, &TextureDesc)> {
        match &self.storage {
            Storage::Texture { texture, desc } => Some((texture, desc)),
            Storage::Buffer(_) | Storage::Layers(_) => None,
        }
    }

    /// The resources a layer of a texture held a layer a resource, empty for any other.
    pub fn layers(&self) -> &[*mut pipe_resource] {
        match &self.storage {
            Storage::Layers(layers) => layers,
            Storage::Buffer(_) | Storage::Texture { .. } => &[],
        }
    }

    /// The resource holding `layer` of this one and the layer's place in it: the layer's own
    /// resource at its layer 0 for a texture held a layer a resource, this one otherwise.
    pub fn layer(&self, layer: u32) -> (&Resource<B>, u32) {
        match self.layers().get(layer as usize) {
            // SAFETY: the resource holds a reference on each of its layers.
            Some(&held) => (unsafe { Resource::of(held) }, 0),
            None => (self, layer),
        }
    }

    /// The interface's format of the resource's Gallium format.
    pub fn format(&self) -> Option<Format> {
        format::format(self.base.format())
    }
}

/// The count of a Gallium reference.
///
/// # Safety
/// `reference` is a live `pipe_reference`.
unsafe fn count<'a>(reference: *mut pipe_reference) -> &'a AtomicI32 {
    // SAFETY: the caller's; Mesa changes the count only atomically.
    unsafe { AtomicI32::from_ptr(&raw mut (*reference).count) }
}

/// Points `dst` at `src`, taking a reference on `src` and dropping the one `dst` held; a resource
/// whose last reference goes is destroyed (`pipe_resource_reference`).
///
/// # Safety
/// Both are null or live resources of this driver.
pub unsafe fn reference(dst: &mut *mut pipe_resource, src: *mut pipe_resource) {
    if *dst == src {
        return;
    }
    if !src.is_null() {
        // SAFETY: the caller's.
        unsafe { count(&raw mut (*src).__bindgen_anon_1.reference) }
            .fetch_add(1, Ordering::Relaxed);
    }
    let old = std::mem::replace(dst, src);
    // SAFETY: the caller's.
    unsafe { release(old) };
}

/// Drops one reference on `resource`, destroying it with the last.
///
/// # Safety
/// `resource` is null or a live resource holding the reference dropped.
pub unsafe fn release(resource: *mut pipe_resource) {
    if resource.is_null() {
        return;
    }
    // SAFETY: the caller's.
    let last = unsafe { count(&raw mut (*resource).__bindgen_anon_1.reference) }
        .fetch_sub(1, Ordering::AcqRel)
        == 1;
    if last {
        // SAFETY: the resource holds its screen, which outlives it.
        let screen = unsafe { (*resource).screen };
        if let Some(destroy) = unsafe { (*screen).resource_destroy } {
            // SAFETY: the last reference went; Mesa's own destroy takes the resource.
            unsafe { destroy(screen, resource) };
        }
    }
}

/// `pipe_context::resource_release`: the frontend gives its reference back.
pub unsafe extern "C" fn resource_release(pipe: *mut pipe_context, resource: *mut pipe_resource) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes a resource it holds a reference on.
        unsafe { release(resource) };
    })
}

/// The bytes of one block of a format (of one element, for a vertex format).
pub fn block_bytes(format: pipe_format) -> u32 {
    block(format).2
}

/// Gallium's block of a format: width and height in texels, and bytes.
fn block(format: pipe_format) -> (u32, u32, u32) {
    // SAFETY: Mesa's table holds every format.
    let block = unsafe { (*util_format_description(format)).block };
    (block.width, block.height, block.bits / u8::BITS)
}
