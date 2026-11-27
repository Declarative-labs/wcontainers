//! What one call holds: the keys and client memory its plain data names (`Keys`, listed by flat.rs), and the objects
//! and bytes held for it (`Held`), so the trait's owned structures can lend them. The objects are taken from the
//! registry under its lock and the lock is let go before any code of either side runs; the bytes are copied into our
//! own memory before the implementation sees them.

use std::sync::Arc;

use wcontainers_gallium::Gallium;

/// The kinds of object plain data names by key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// A resource (`struct pipe_resource`).
    Resource,
    /// A stream output target (`struct pipe_stream_output_target`).
    StreamOutputTarget,
    /// A sampler view (`struct pipe_sampler_view`).
    SamplerView,
    /// A context (`struct pipe_context`).
    Context,
}

/// The keys and client memory a call's plain data names.
#[derive(Debug, Default)]
pub struct Keys {
    /// Each object named, by kind and key (zero names none).
    pub objects: Vec<(ObjectKind, usize)>,
    /// Each range of client memory named: address, offset and length (address zero names none).
    pub memory: Vec<(usize, usize, usize)>,
    /// How many bytes of indices the call's draws read from index 0 (draw_vbo sets it before listing).
    pub index_bytes: usize,
}

impl Keys {
    /// Lists an object by its key.
    pub fn object(&mut self, kind: ObjectKind, key: usize) {
        if key != 0 {
            self.objects.push((kind, key));
        }
    }

    /// Lists a range of client memory.
    pub fn memory(&mut self, address: usize, offset: usize, length: usize) {
        if address != 0 {
            self.memory.push((address, offset, length));
        }
    }
}

/// The objects and bytes held for one call.
pub struct Held<G: Gallium> {
    /// Each resource named, by key.
    pub resources: Vec<(usize, Arc<G::Resource>)>,
    /// Each stream output target named, by key.
    pub stream_output_targets: Vec<(usize, Arc<G::StreamOutputTarget>)>,
    /// Each sampler view named, by key.
    pub sampler_views: Vec<(usize, Arc<G::SamplerView>)>,
    /// Each context named, by key.
    pub contexts: Vec<(usize, Arc<G::Context>)>,
    /// Each range of client memory named, copied: address, offset, length and the bytes.
    pub memory: Vec<((usize, usize, usize), Vec<u8>)>,
    /// The bytes of indices the draws read (from `Keys`).
    pub index_bytes: usize,
}

impl<G: Gallium> Default for Held<G> {
    fn default() -> Self {
        Held::new()
    }
}

/// One object of a list by key: `Some(None)` for key zero (none), `None` for a key not held (the call is refused).
fn find<K>(list: &[(usize, Arc<K>)], key: usize) -> Option<Option<&Arc<K>>> {
    if key == 0 {
        return Some(None);
    }
    list.iter().find(|(held, _)| *held == key).map(|(_, object)| Some(object))
}

impl<G: Gallium> Held<G> {
    /// Nothing held (a call whose plain data names no object and no client memory).
    pub fn new() -> Held<G> {
        Held { resources: Vec::new(), stream_output_targets: Vec::new(), sampler_views: Vec::new(), contexts: Vec::new(), memory: Vec::new(), index_bytes: 0 }
    }

    /// A resource by key.
    pub fn resource(&self, key: usize) -> Option<Option<&Arc<G::Resource>>> {
        find(&self.resources, key)
    }

    /// A stream output target by key.
    pub fn stream_output_target(&self, key: usize) -> Option<Option<&Arc<G::StreamOutputTarget>>> {
        find(&self.stream_output_targets, key)
    }

    /// A sampler view by key.
    pub fn sampler_view(&self, key: usize) -> Option<Option<&Arc<G::SamplerView>>> {
        find(&self.sampler_views, key)
    }

    /// A context by key.
    pub fn context(&self, key: usize) -> Option<Option<&G::Context>> {
        find(&self.contexts, key).map(|found| found.map(|context| &**context))
    }

    /// A range of client memory, copied: `Some(None)` for address zero.
    pub fn memory(&self, address: usize, offset: usize, length: usize) -> Option<Option<&[u8]>> {
        if address == 0 {
            return Some(None);
        }
        self.memory.iter().find(|(range, _)| *range == (address, offset, length)).map(|(_, bytes)| Some(bytes.as_slice()))
    }
}

/// A C `bool` as Rust's: 0 or 1, anything else refused.
pub fn flag(value: u8) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// A bit-field's value, refused past its width.
pub fn within<T: Into<u64> + Copy>(value: T, bits: u32) -> Option<T> {
    (value.into() >> bits == 0).then_some(value)
}

/// Each element of an array made by `make`, or None if any is refused.
pub fn each<T, U, const N: usize>(array: &[T; N], make: impl FnMut(&T) -> Option<U>) -> Option<[U; N]> {
    let made: Option<Vec<U>> = array.iter().map(make).collect();
    made?.try_into().ok()
}
