//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! A recording implementation of direct3d-common's traits, for tests only: every call is logged by its place, with the values
//! passed by value; an object asked for is made; a structure, a bound object or a list asked for is not (E_NOTIMPL).

#![allow(
    unused_variables,
    unused_imports,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::new_without_default,
    reason = "generated: every method has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_component_object_model::constants::{E_NOINTERFACE, E_NOTIMPL};

/// Every call any object of this crate received, oldest first: the place, and the values passed by value.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The next object's identity.
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Objects alive now.
static LIVE: AtomicI64 = AtomicI64::new(0);

/// Logs one call.
fn record(call: String) {
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(call);
}

/// Returns every call logged so far whose text starts with `place` (a method's `Interface::Method`, or an export).
pub fn calls(place: &str) -> Vec<String> {
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|call| call.starts_with(place) && call[place.len()..].starts_with('('))
        .cloned()
        .collect()
}

/// Returns how many calls were logged so far.
pub fn logged() -> usize {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// Returns how many objects of this crate are alive.
pub fn live() -> i64 {
    LIVE.load(Ordering::SeqCst)
}

/// A known answer's bytes are each one of `KNOWN_SPAN` values from 1: never zero, and never with the top bit set, so no
/// float read from them is NaN or infinite and every float compares equal to itself.
const KNOWN_SPAN: usize = 126;
/// What a known byte moves by from one byte to the next, so that neighboring fields differ.
const KNOWN_STRIDE: usize = 29;
/// The most bytes the recorder writes into the caller's bytes.
const KNOWN_MOST: usize = 1 << 16;

/// The known byte at `at` of an answer of salt `salt`.
fn known_byte(salt: usize, at: usize) -> u8 {
    u8::try_from(1 + (salt + at * KNOWN_STRIDE) % KNOWN_SPAN).unwrap_or(1)
}

/// The known bytes a structure of the recorder's answers is decoded from.
#[allow(dead_code, reason = "a recorder answering no structure decodes none")]
fn known_bytes<const N: usize>(salt: usize) -> [u8; N] {
    core::array::from_fn(|at| known_byte(salt, at))
}

/// Writes known bytes over all of the caller's bytes (the first `KNOWN_MOST` of them), as the system writes what it
/// is told of; the conformance suite compares what reaches the caller through both glues with the same call made
/// directly.
#[allow(dead_code, reason = "a recorder writing no caller's bytes writes none")]
fn known_fill<Bytes: wcontainers_volatile_bytes::VolatileBytes>(bytes: &Bytes, salt: usize) {
    let length = usize::try_from(bytes.len())
        .unwrap_or(KNOWN_MOST)
        .min(KNOWN_MOST);
    let known: Vec<u8> = (0..length).map(|at| known_byte(salt, at)).collect();
    let _ = bytes.write(&known);
}

/// One object alive, counted until its last clone drops.
#[derive(Debug)]
struct Live(u64);

impl Live {
    fn new() -> Arc<Live> {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Arc::new(Live(NEXT.fetch_add(1, Ordering::SeqCst)))
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A recorded object of the kind `Blob`.
pub struct RecordedBlob<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedBlob<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedBlob<A> {
        RecordedBlob {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedBlob<A> {
    fn clone(&self) -> RecordedBlob<A> {
        RecordedBlob {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedBlob<A> {
    fn eq(&self, other: &RecordedBlob<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedBlob<A> {}

impl<A> Hash for RecordedBlob<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedBlob<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedBlob")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DestructionNotifier`.
pub struct RecordedDestructionNotifier<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDestructionNotifier<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDestructionNotifier<A> {
        RecordedDestructionNotifier {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDestructionNotifier<A> {
    fn clone(&self) -> RecordedDestructionNotifier<A> {
        RecordedDestructionNotifier {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDestructionNotifier<A> {
    fn eq(&self, other: &RecordedDestructionNotifier<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDestructionNotifier<A> {}

impl<A> Hash for RecordedDestructionNotifier<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDestructionNotifier<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDestructionNotifier")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this recorder's.
pub trait Recorded:
    direct3d_common::Direct3DCommon<
        Blob = RecordedBlob<Self>,
        DestructionNotifier = RecordedDestructionNotifier<Self>,
    >
{
}

impl<A: Recorded> direct3d_common::Blob for RecordedBlob<A> {
    fn get_buffer_pointer(&self) -> u8 {
        record("ID3DBlob::GetBufferPointer()".to_string());
        Default::default()
    }
    fn get_buffer_size(&self) -> usize {
        record("ID3DBlob::GetBufferSize()".to_string());
        28_usize
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDestructionNotifier<A> {
    fn register_destruction_callback(
        &self,
        callback_fn: usize,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &callback_fn;
        let _ = &data;
        record("ID3DDestructionNotifier::RegisterDestructionCallback()".to_string());
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(
        &self,
        callback_id: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &callback_id;
        record(format!(
            "ID3DDestructionNotifier::UnregisterDestructionCallback(callback_id={:?})",
            callback_id
        ));
        Ok(())
    }
}

/// The recorder's library: `Direct3DCommon` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Recorder;

impl direct3d_common::Direct3DCommon for Recorder {
    type Blob = RecordedBlob<Self>;
    type DestructionNotifier = RecordedDestructionNotifier<Self>;
}

impl Recorded for Recorder {}
