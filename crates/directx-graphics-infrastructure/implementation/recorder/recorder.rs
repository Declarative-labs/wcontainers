//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! A recording implementation of directx-graphics-infrastructure's traits, for tests only: every call is logged by its place, with the values
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
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

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

/// Every call's arguments as the recorder received them, oldest first: the place and each argument of plain data
/// (`Debug`, every field), an object named only as one.
static ARGUMENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Logs one call.
fn record(call: String) {
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(call);
}

/// Logs one call's arguments.
fn record_arguments(call: String) {
    ARGUMENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(call);
}

/// Returns how many calls' arguments were logged so far.
pub fn arguments_logged() -> usize {
    ARGUMENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .len()
}

/// Returns the arguments of every call logged after the first `from`, oldest first.
pub fn arguments_since(from: usize) -> Vec<String> {
    ARGUMENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(from..)
        .map(<[String]>::to_vec)
        .unwrap_or_default()
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

/// The known bytes the recorder writes into the caller's bytes (a recorder writing none uses none of it).
#[allow(dead_code, reason = "a recorder writing no caller's bytes writes none")]
mod known {
    /// Each known byte is one of `KNOWN_SPAN` values from 1: never zero.
    const KNOWN_SPAN: usize = 126;
    /// What a known byte moves by from one byte to the next, so that neighboring bytes differ.
    const KNOWN_STRIDE: usize = 29;
    /// The most bytes the recorder writes into the caller's bytes.
    const KNOWN_MOST: usize = 1 << 16;

    /// The known byte at `at` of an answer of salt `salt`.
    fn known_byte(salt: usize, at: usize) -> u8 {
        u8::try_from(1 + (salt + at * KNOWN_STRIDE) % KNOWN_SPAN).unwrap_or(1)
    }

    /// Writes known bytes over all of the caller's bytes (the first `KNOWN_MOST` of them), as the system writes what it
    /// is told of; the conformance suite compares what reaches the caller through both glues with the same call made
    /// directly.
    pub(super) fn known_fill<Bytes: wcontainers_volatile_bytes::VolatileBytes>(
        bytes: &Bytes,
        salt: usize,
    ) {
        let length = usize::try_from(bytes.len())
            .unwrap_or(KNOWN_MOST)
            .min(KNOWN_MOST);
        let known: Vec<u8> = (0..length).map(|at| known_byte(salt, at)).collect();
        let _ = bytes.write(&known);
    }
}

use known::known_fill;

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

/// A recorded object of the kind `Adapter`.
pub struct RecordedAdapter<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedAdapter<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedAdapter<A> {
        RecordedAdapter {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedAdapter<A> {
    fn clone(&self) -> RecordedAdapter<A> {
        RecordedAdapter {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedAdapter<A> {
    fn eq(&self, other: &RecordedAdapter<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedAdapter<A> {}

impl<A> Hash for RecordedAdapter<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedAdapter<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedAdapter")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Debug`.
pub struct RecordedDebug<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDebug<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDebug<A> {
        RecordedDebug {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDebug<A> {
    fn clone(&self) -> RecordedDebug<A> {
        RecordedDebug {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDebug<A> {
    fn eq(&self, other: &RecordedDebug<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDebug<A> {}

impl<A> Hash for RecordedDebug<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDebug<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDebug")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DecodeSwapChain`.
pub struct RecordedDecodeSwapChain<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDecodeSwapChain<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDecodeSwapChain<A> {
        RecordedDecodeSwapChain {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDecodeSwapChain<A> {
    fn clone(&self) -> RecordedDecodeSwapChain<A> {
        RecordedDecodeSwapChain {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDecodeSwapChain<A> {
    fn eq(&self, other: &RecordedDecodeSwapChain<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDecodeSwapChain<A> {}

impl<A> Hash for RecordedDecodeSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDecodeSwapChain<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDecodeSwapChain")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Device`.
pub struct RecordedDevice<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDevice<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDevice<A> {
        RecordedDevice {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDevice<A> {
    fn clone(&self) -> RecordedDevice<A> {
        RecordedDevice {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDevice<A> {
    fn eq(&self, other: &RecordedDevice<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDevice<A> {}

impl<A> Hash for RecordedDevice<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDevice<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDevice")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DisplayControl`.
pub struct RecordedDisplayControl<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDisplayControl<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDisplayControl<A> {
        RecordedDisplayControl {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDisplayControl<A> {
    fn clone(&self) -> RecordedDisplayControl<A> {
        RecordedDisplayControl {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDisplayControl<A> {
    fn eq(&self, other: &RecordedDisplayControl<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDisplayControl<A> {}

impl<A> Hash for RecordedDisplayControl<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDisplayControl<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDisplayControl")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Factory`.
pub struct RecordedFactory<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedFactory<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedFactory<A> {
        RecordedFactory {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedFactory<A> {
    fn clone(&self) -> RecordedFactory<A> {
        RecordedFactory {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedFactory<A> {
    fn eq(&self, other: &RecordedFactory<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedFactory<A> {}

impl<A> Hash for RecordedFactory<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedFactory<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedFactory")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `FactoryMedia`.
pub struct RecordedFactoryMedia<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedFactoryMedia<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedFactoryMedia<A> {
        RecordedFactoryMedia {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedFactoryMedia<A> {
    fn clone(&self) -> RecordedFactoryMedia<A> {
        RecordedFactoryMedia {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedFactoryMedia<A> {
    fn eq(&self, other: &RecordedFactoryMedia<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedFactoryMedia<A> {}

impl<A> Hash for RecordedFactoryMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedFactoryMedia<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedFactoryMedia")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `InfoQueue`.
pub struct RecordedInfoQueue<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedInfoQueue<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedInfoQueue<A> {
        RecordedInfoQueue {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedInfoQueue<A> {
    fn clone(&self) -> RecordedInfoQueue<A> {
        RecordedInfoQueue {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedInfoQueue<A> {
    fn eq(&self, other: &RecordedInfoQueue<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedInfoQueue<A> {}

impl<A> Hash for RecordedInfoQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedInfoQueue<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedInfoQueue")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `KeyedMutex`.
pub struct RecordedKeyedMutex<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedKeyedMutex<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedKeyedMutex<A> {
        RecordedKeyedMutex {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedKeyedMutex<A> {
    fn clone(&self) -> RecordedKeyedMutex<A> {
        RecordedKeyedMutex {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedKeyedMutex<A> {
    fn eq(&self, other: &RecordedKeyedMutex<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedKeyedMutex<A> {}

impl<A> Hash for RecordedKeyedMutex<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedKeyedMutex<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedKeyedMutex")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Output`.
pub struct RecordedOutput<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedOutput<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedOutput<A> {
        RecordedOutput {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedOutput<A> {
    fn clone(&self) -> RecordedOutput<A> {
        RecordedOutput {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedOutput<A> {
    fn eq(&self, other: &RecordedOutput<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedOutput<A> {}

impl<A> Hash for RecordedOutput<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedOutput<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedOutput")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `OutputDuplication`.
pub struct RecordedOutputDuplication<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedOutputDuplication<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedOutputDuplication<A> {
        RecordedOutputDuplication {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedOutputDuplication<A> {
    fn clone(&self) -> RecordedOutputDuplication<A> {
        RecordedOutputDuplication {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedOutputDuplication<A> {
    fn eq(&self, other: &RecordedOutputDuplication<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedOutputDuplication<A> {}

impl<A> Hash for RecordedOutputDuplication<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedOutputDuplication<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedOutputDuplication")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Resource`.
pub struct RecordedResource<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedResource<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedResource<A> {
        RecordedResource {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedResource<A> {
    fn clone(&self) -> RecordedResource<A> {
        RecordedResource {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedResource<A> {
    fn eq(&self, other: &RecordedResource<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedResource<A> {}

impl<A> Hash for RecordedResource<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedResource<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedResource")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Surface`.
pub struct RecordedSurface<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedSurface<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedSurface<A> {
        RecordedSurface {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedSurface<A> {
    fn clone(&self) -> RecordedSurface<A> {
        RecordedSurface {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedSurface<A> {
    fn eq(&self, other: &RecordedSurface<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedSurface<A> {}

impl<A> Hash for RecordedSurface<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedSurface<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedSurface")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SwapChain`.
pub struct RecordedSwapChain<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedSwapChain<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedSwapChain<A> {
        RecordedSwapChain {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedSwapChain<A> {
    fn clone(&self) -> RecordedSwapChain<A> {
        RecordedSwapChain {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedSwapChain<A> {
    fn eq(&self, other: &RecordedSwapChain<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedSwapChain<A> {}

impl<A> Hash for RecordedSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedSwapChain<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedSwapChain")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SwapChainMedia`.
pub struct RecordedSwapChainMedia<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedSwapChainMedia<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedSwapChainMedia<A> {
        RecordedSwapChainMedia {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedSwapChainMedia<A> {
    fn clone(&self) -> RecordedSwapChainMedia<A> {
        RecordedSwapChainMedia {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedSwapChainMedia<A> {
    fn eq(&self, other: &RecordedSwapChainMedia<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedSwapChainMedia<A> {}

impl<A> Hash for RecordedSwapChainMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedSwapChainMedia<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedSwapChainMedia")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `GraphicsAnalysis`.
pub struct RecordedGraphicsAnalysis<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedGraphicsAnalysis<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedGraphicsAnalysis<A> {
        RecordedGraphicsAnalysis {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedGraphicsAnalysis<A> {
    fn clone(&self) -> RecordedGraphicsAnalysis<A> {
        RecordedGraphicsAnalysis {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedGraphicsAnalysis<A> {
    fn eq(&self, other: &RecordedGraphicsAnalysis<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedGraphicsAnalysis<A> {}

impl<A> Hash for RecordedGraphicsAnalysis<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedGraphicsAnalysis<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedGraphicsAnalysis")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this recorder's.
pub trait Recorded:
    directx_graphics_infrastructure::DirectXGraphicsInfrastructure<
        Adapter = RecordedAdapter<Self>,
        Debug = RecordedDebug<Self>,
        DecodeSwapChain = RecordedDecodeSwapChain<Self>,
        Device = RecordedDevice<Self>,
        DisplayControl = RecordedDisplayControl<Self>,
        Factory = RecordedFactory<Self>,
        FactoryMedia = RecordedFactoryMedia<Self>,
        InfoQueue = RecordedInfoQueue<Self>,
        KeyedMutex = RecordedKeyedMutex<Self>,
        Output = RecordedOutput<Self>,
        OutputDuplication = RecordedOutputDuplication<Self>,
        Resource = RecordedResource<Self>,
        Surface = RecordedSurface<Self>,
        SwapChain = RecordedSwapChain<Self>,
        SwapChainMedia = RecordedSwapChainMedia<Self>,
        GraphicsAnalysis = RecordedGraphicsAnalysis<Self>,
    >
{
}

impl<A: Recorded> directx_graphics_infrastructure::Adapter4 for RecordedAdapter<A> {
    fn get_desc3(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc3,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter4::GetDesc3()".to_string());
        record_arguments("IDXGIAdapter4::GetDesc3()".to_string());
        Ok(directx_graphics_infrastructure::structures::AdapterDesc3 {
            description: core::array::from_fn(|_| 32_u16),
            vendor_id: 39_u32,
            device_id: 46_u32,
            sub_sys_id: 53_u32,
            revision: 60_u32,
            dedicated_video_memory: 67_usize,
            dedicated_system_memory: 74_usize,
            shared_system_memory: 81_usize,
            adapter_luid: component_object_model::structures::Luid {
                low_part: 95_u32,
                high_part: 102_i32,
            },
            flags: directx_graphics_infrastructure::enumerations::AdapterFlag3(95),
            graphics_preemption_granularity:
                directx_graphics_infrastructure::enumerations::GraphicsPreemptionGranularity(102),
            compute_preemption_granularity:
                directx_graphics_infrastructure::enumerations::ComputePreemptionGranularity(109),
        })
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Adapter3 for RecordedAdapter<A> {
    fn register_hardware_content_protection_teardown_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(80_u32)
    }
    fn unregister_hardware_content_protection_teardown_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus(dw_cookie={:?})",
            dw_cookie
        ));
        record_arguments(format!(
            "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn query_video_memory_info(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
    ) -> Result<
        directx_graphics_infrastructure::structures::QueryVideoMemoryInfo,
        component_object_model::structures::HResult,
    > {
        let _ = &node_index;
        let _ = &memory_segment_group;
        record(format!(
            "IDXGIAdapter3::QueryVideoMemoryInfo(node_index={:?}, memory_segment_group={:?})",
            node_index, memory_segment_group
        ));
        record_arguments(format!(
            "IDXGIAdapter3::QueryVideoMemoryInfo(node_index={:?}, memory_segment_group={:?})",
            node_index, memory_segment_group
        ));
        Ok(
            directx_graphics_infrastructure::structures::QueryVideoMemoryInfo {
                budget: 64_u64,
                current_usage: 71_u64,
                available_for_reservation: 78_u64,
                current_reservation: 85_u64,
            },
        )
    }
    fn set_video_memory_reservation(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
        reservation: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &node_index;
        let _ = &memory_segment_group;
        let _ = &reservation;
        record(format!(
            "IDXGIAdapter3::SetVideoMemoryReservation(node_index={:?}, memory_segment_group={:?}, reservation={:?})",
            node_index, memory_segment_group, reservation
        ));
        record_arguments(format!(
            "IDXGIAdapter3::SetVideoMemoryReservation(node_index={:?}, memory_segment_group={:?}, reservation={:?})",
            node_index, memory_segment_group, reservation
        ));
        Ok(())
    }
    fn register_video_memory_budget_change_notification_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent(h_event={:?})",
            h_event
        ));
        Ok(116_u32)
    }
    fn unregister_video_memory_budget_change_notification(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification(dw_cookie={:?})",
            dw_cookie
        ));
        record_arguments(format!(
            "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification(dw_cookie={:?})",
            dw_cookie
        ));
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Adapter2 for RecordedAdapter<A> {
    fn get_desc2(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc2,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter2::GetDesc2()".to_string());
        record_arguments("IDXGIAdapter2::GetDesc2()".to_string());
        Ok(directx_graphics_infrastructure::structures::AdapterDesc2 {
            description: core::array::from_fn(|_| 28_u16),
            vendor_id: 35_u32,
            device_id: 42_u32,
            sub_sys_id: 49_u32,
            revision: 56_u32,
            dedicated_video_memory: 63_usize,
            dedicated_system_memory: 70_usize,
            shared_system_memory: 77_usize,
            adapter_luid: component_object_model::structures::Luid {
                low_part: 91_u32,
                high_part: 98_i32,
            },
            flags: 91_u32,
            graphics_preemption_granularity:
                directx_graphics_infrastructure::enumerations::GraphicsPreemptionGranularity(98),
            compute_preemption_granularity:
                directx_graphics_infrastructure::enumerations::ComputePreemptionGranularity(105),
        })
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Adapter1 for RecordedAdapter<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter1::GetDesc1()".to_string());
        record_arguments("IDXGIAdapter1::GetDesc1()".to_string());
        Ok(directx_graphics_infrastructure::structures::AdapterDesc1 {
            description: core::array::from_fn(|_| 14_u16),
            vendor_id: 21_u32,
            device_id: 28_u32,
            sub_sys_id: 35_u32,
            revision: 42_u32,
            dedicated_video_memory: 49_usize,
            dedicated_system_memory: 56_usize,
            shared_system_memory: 63_usize,
            adapter_luid: component_object_model::structures::Luid {
                low_part: 77_u32,
                high_part: 84_i32,
            },
            flags: 77_u32,
        })
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Adapter for RecordedAdapter<A> {
    fn enum_outputs(
        &self,
        output: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        let _ = &output;
        record(format!("IDXGIAdapter::EnumOutputs(output={:?})", output));
        record_arguments(format!("IDXGIAdapter::EnumOutputs(output={:?})", output));
        Ok(RecordedOutput::new())
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter::GetDesc()".to_string());
        record_arguments("IDXGIAdapter::GetDesc()".to_string());
        Ok(directx_graphics_infrastructure::structures::AdapterDesc {
            description: core::array::from_fn(|_| 71_u16),
            vendor_id: 78_u32,
            device_id: 85_u32,
            sub_sys_id: 92_u32,
            revision: 99_u32,
            dedicated_video_memory: 106_usize,
            dedicated_system_memory: 113_usize,
            shared_system_memory: 120_usize,
            adapter_luid: component_object_model::structures::Luid {
                low_part: 8_u32,
                high_part: 15_i32,
            },
        })
    }
    fn check_interface_support(
        &self,
        interface_name: &component_object_model::structures::Guid,
    ) -> Result<i64, component_object_model::structures::HResult> {
        let _ = &interface_name;
        record("IDXGIAdapter::CheckInterfaceSupport()".to_string());
        record_arguments(format!(
            "IDXGIAdapter::CheckInterfaceSupport(interface_name={:?})",
            interface_name
        ));
        Ok(13_i64)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedAdapter<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Debug1 for RecordedDebug<A> {
    fn enable_leak_tracking_for_thread(&self) {
        record("IDXGIDebug1::EnableLeakTrackingForThread()".to_string());
        record_arguments("IDXGIDebug1::EnableLeakTrackingForThread()".to_string());
    }
    fn disable_leak_tracking_for_thread(&self) {
        record("IDXGIDebug1::DisableLeakTrackingForThread()".to_string());
        record_arguments("IDXGIDebug1::DisableLeakTrackingForThread()".to_string());
    }
    fn is_leak_tracking_enabled_for_thread(&self) -> bool {
        record("IDXGIDebug1::IsLeakTrackingEnabledForThread()".to_string());
        record_arguments("IDXGIDebug1::IsLeakTrackingEnabledForThread()".to_string());
        true
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Debug for RecordedDebug<A> {
    fn report_live_objects(
        &self,
        apiid: component_object_model::structures::Guid,
        flags: directx_graphics_infrastructure::enumerations::DebugRloFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &apiid;
        let _ = &flags;
        record(format!(
            "IDXGIDebug::ReportLiveObjects(apiid={:?}, flags={:?})",
            apiid, flags
        ));
        record_arguments(format!(
            "IDXGIDebug::ReportLiveObjects(apiid={:?}, flags={:?})",
            apiid, flags
        ));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DecodeSwapChain for RecordedDecodeSwapChain<A> {
    fn present_buffer(
        &self,
        buffer_to_present: u32,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &buffer_to_present;
        let _ = &sync_interval;
        let _ = &flags;
        record(format!(
            "IDXGIDecodeSwapChain::PresentBuffer(buffer_to_present={:?}, sync_interval={:?}, flags={:?})",
            buffer_to_present, sync_interval, flags
        ));
        record_arguments(format!(
            "IDXGIDecodeSwapChain::PresentBuffer(buffer_to_present={:?}, sync_interval={:?}, flags={:?})",
            buffer_to_present, sync_interval, flags
        ));
        Ok(())
    }
    fn set_source_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rect;
        record("IDXGIDecodeSwapChain::SetSourceRect()".to_string());
        record_arguments(format!(
            "IDXGIDecodeSwapChain::SetSourceRect(rect={:?})",
            rect
        ));
        Ok(())
    }
    fn set_target_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rect;
        record("IDXGIDecodeSwapChain::SetTargetRect()".to_string());
        record_arguments(format!(
            "IDXGIDecodeSwapChain::SetTargetRect(rect={:?})",
            rect
        ));
        Ok(())
    }
    fn set_dest_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        record(format!(
            "IDXGIDecodeSwapChain::SetDestSize(width={:?}, height={:?})",
            width, height
        ));
        record_arguments(format!(
            "IDXGIDecodeSwapChain::SetDestSize(width={:?}, height={:?})",
            width, height
        ));
        Ok(())
    }
    fn get_source_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        record("IDXGIDecodeSwapChain::GetSourceRect()".to_string());
        record_arguments("IDXGIDecodeSwapChain::GetSourceRect()".to_string());
        Ok(component_object_model::structures::Rect {
            left: 35_i32,
            top: 42_i32,
            right: 49_i32,
            bottom: 56_i32,
        })
    }
    fn get_target_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        record("IDXGIDecodeSwapChain::GetTargetRect()".to_string());
        record_arguments("IDXGIDecodeSwapChain::GetTargetRect()".to_string());
        Ok(component_object_model::structures::Rect {
            left: 62_i32,
            top: 69_i32,
            right: 76_i32,
            bottom: 83_i32,
        })
    }
    fn get_dest_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        record("IDXGIDecodeSwapChain::GetDestSize()".to_string());
        record_arguments("IDXGIDecodeSwapChain::GetDestSize()".to_string());
        Ok((100_u32, 107_u32))
    }
    fn set_color_space(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGIDecodeSwapChain::SetColorSpace(color_space={:?})",
            color_space
        ));
        record_arguments(format!(
            "IDXGIDecodeSwapChain::SetColorSpace(color_space={:?})",
            color_space
        ));
        Ok(())
    }
    fn get_color_space(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags {
        record("IDXGIDecodeSwapChain::GetColorSpace()".to_string());
        record_arguments("IDXGIDecodeSwapChain::GetColorSpace()".to_string());
        directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags(108)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device4 for RecordedDevice<A> {
    fn offer_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
        flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resources;
        let _ = &priority;
        let _ = &flags;
        record(format!(
            "IDXGIDevice4::OfferResources1(priority={:?}, flags={:?})",
            priority, flags
        ));
        record_arguments(format!(
            "IDXGIDevice4::OfferResources1(resources={:?}, priority={:?}, flags={:?})",
            resources
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>(),
            priority,
            flags
        ));
        Ok(())
    }
    fn reclaim_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::ReclaimResourceResults>,
        component_object_model::structures::HResult,
    > {
        let _ = &resources;
        record("IDXGIDevice4::ReclaimResources1()".to_string());
        record_arguments(format!(
            "IDXGIDevice4::ReclaimResources1(resources={:?})",
            resources
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>()
        ));
        Ok((0..2_u32)
            .map(|_| directx_graphics_infrastructure::enumerations::ReclaimResourceResults(53))
            .collect())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device3 for RecordedDevice<A> {
    fn trim(&self) {
        record("IDXGIDevice3::Trim()".to_string());
        record_arguments("IDXGIDevice3::Trim()".to_string());
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device2 for RecordedDevice<A> {
    fn offer_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resources;
        let _ = &priority;
        record(format!(
            "IDXGIDevice2::OfferResources(priority={:?})",
            priority
        ));
        record_arguments(format!(
            "IDXGIDevice2::OfferResources(resources={:?}, priority={:?})",
            resources
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>(),
            priority
        ));
        Ok(())
    }
    fn reclaim_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<bool>, component_object_model::structures::HResult> {
        let _ = &resources;
        record("IDXGIDevice2::ReclaimResources()".to_string());
        record_arguments(format!(
            "IDXGIDevice2::ReclaimResources(resources={:?})",
            resources
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>()
        ));
        Ok((0..2_u32).map(|_| true).collect())
    }
    fn enqueue_set_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIDevice2::EnqueueSetEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIDevice2::EnqueueSetEvent(h_event={:?})",
            h_event
        ));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device1 for RecordedDevice<A> {
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &max_latency;
        record(format!(
            "IDXGIDevice1::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        record_arguments(format!(
            "IDXGIDevice1::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGIDevice1::GetMaximumFrameLatency()".to_string());
        record_arguments("IDXGIDevice1::GetMaximumFrameLatency()".to_string());
        Ok(38_u32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device for RecordedDevice<A> {
    fn get_adapter(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        record("IDXGIDevice::GetAdapter()".to_string());
        record_arguments("IDXGIDevice::GetAdapter()".to_string());
        Ok(RecordedAdapter::new())
    }
    fn create_surface(&self, desc: &directx_graphics_infrastructure::structures::SurfaceDesc, num_surfaces: u32, usage: directx_graphics_infrastructure::enumerations::Usage, shared_resource: Option<&directx_graphics_infrastructure::structures::SharedResource>) -> Result<Vec<Option<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>>, component_object_model::structures::HResult>{
        let _ = &desc;
        let _ = &num_surfaces;
        let _ = &usage;
        let _ = &shared_resource;
        record(format!(
            "IDXGIDevice::CreateSurface(num_surfaces={:?}, usage={:?})",
            num_surfaces, usage
        ));
        record_arguments(format!(
            "IDXGIDevice::CreateSurface(desc={:?}, num_surfaces={:?}, usage={:?}, shared_resource={:?})",
            desc, num_surfaces, usage, shared_resource
        ));
        Ok((0..num_surfaces)
            .map(|_| Some(RecordedSurface::new()))
            .collect())
    }
    fn query_resource_residency(
        &self,
        resources: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::Residency>,
        component_object_model::structures::HResult,
    > {
        let _ = &resources;
        record("IDXGIDevice::QueryResourceResidency()".to_string());
        record_arguments(format!(
            "IDXGIDevice::QueryResourceResidency(resources={:?})",
            resources
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>()
        ));
        Err(E_NOTIMPL)
    }
    fn set_gpu_thread_priority(
        &self,
        priority: i32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &priority;
        record(format!(
            "IDXGIDevice::SetGPUThreadPriority(priority={:?})",
            priority
        ));
        record_arguments(format!(
            "IDXGIDevice::SetGPUThreadPriority(priority={:?})",
            priority
        ));
        Ok(())
    }
    fn get_gpu_thread_priority(&self) -> Result<i32, component_object_model::structures::HResult> {
        record("IDXGIDevice::GetGPUThreadPriority()".to_string());
        record_arguments("IDXGIDevice::GetGPUThreadPriority()".to_string());
        Ok(34_i32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedDevice<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DisplayControl for RecordedDisplayControl<A> {
    fn is_stereo_enabled(&self) -> bool {
        record("IDXGIDisplayControl::IsStereoEnabled()".to_string());
        record_arguments("IDXGIDisplayControl::IsStereoEnabled()".to_string());
        true
    }
    fn set_stereo_enabled(&self, enabled: bool) {
        let _ = &enabled;
        record(format!(
            "IDXGIDisplayControl::SetStereoEnabled(enabled={:?})",
            enabled
        ));
        record_arguments(format!(
            "IDXGIDisplayControl::SetStereoEnabled(enabled={:?})",
            enabled
        ));
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory7 for RecordedFactory<A> {
    fn register_adapters_changed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory7::RegisterAdaptersChangedEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIFactory7::RegisterAdaptersChangedEvent(h_event={:?})",
            h_event
        ));
        Ok(87_u32)
    }
    fn unregister_adapters_changed_event(
        &self,
        dw_cookie: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory7::UnregisterAdaptersChangedEvent(dw_cookie={:?})",
            dw_cookie
        ));
        record_arguments(format!(
            "IDXGIFactory7::UnregisterAdaptersChangedEvent(dw_cookie={:?})",
            dw_cookie
        ));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory6 for RecordedFactory<A> {
    fn enum_adapter_by_gpu_preference(
        &self,
        adapter: u32,
        gpu_preference: directx_graphics_infrastructure::enumerations::GpuPreference,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        let _ = &gpu_preference;
        let _ = &riid;
        record(format!(
            "IDXGIFactory6::EnumAdapterByGpuPreference(adapter={:?}, gpu_preference={:?})",
            adapter, gpu_preference
        ));
        record_arguments(format!(
            "IDXGIFactory6::EnumAdapterByGpuPreference(adapter={:?}, gpu_preference={:?}, riid={:?})",
            adapter, gpu_preference, riid
        ));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory5 for RecordedFactory<A> {
    fn check_feature_support(
        &self,
        feature: directx_graphics_infrastructure::enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &feature;
        let _ = &feature_support_data;
        record(format!(
            "IDXGIFactory5::CheckFeatureSupport(feature={:?})",
            feature
        ));
        record_arguments(format!(
            "IDXGIFactory5::CheckFeatureSupport(feature={:?}, feature_support_data=an object or the caller's bytes)",
            feature
        ));
        known_fill(feature_support_data, 175431);
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory4 for RecordedFactory<A> {
    fn enum_adapter_by_luid(
        &self,
        adapter_luid: component_object_model::structures::Luid,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter_luid;
        let _ = &riid;
        record("IDXGIFactory4::EnumAdapterByLuid()".to_string());
        record_arguments(format!(
            "IDXGIFactory4::EnumAdapterByLuid(adapter_luid={:?}, riid={:?})",
            adapter_luid, riid
        ));
        Err(E_NOTIMPL)
    }
    fn enum_warp_adapter(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIFactory4::EnumWarpAdapter()".to_string());
        record_arguments(format!("IDXGIFactory4::EnumWarpAdapter(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory3 for RecordedFactory<A> {
    fn get_creation_flags(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::CreateFactoryFlags {
        record("IDXGIFactory3::GetCreationFlags()".to_string());
        record_arguments("IDXGIFactory3::GetCreationFlags()".to_string());
        directx_graphics_infrastructure::enumerations::CreateFactoryFlags(37)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory2 for RecordedFactory<A> {
    fn is_windowed_stereo_enabled(&self) -> bool {
        record("IDXGIFactory2::IsWindowedStereoEnabled()".to_string());
        record_arguments("IDXGIFactory2::IsWindowedStereoEnabled()".to_string());
        true
    }
    fn create_swap_chain_for_hwnd(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_wnd: component_object_model::structures::Hwnd,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        fullscreen_desc: Option<
            &directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        >,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &h_wnd;
        let _ = &desc;
        let _ = &fullscreen_desc;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactory2::CreateSwapChainForHwnd(h_wnd={:?})",
            h_wnd
        ));
        record_arguments(format!(
            "IDXGIFactory2::CreateSwapChainForHwnd(device=an object or the caller's bytes, h_wnd={:?}, desc={:?}, fullscreen_desc={:?}, restrict_to_output={:?})",
            h_wnd,
            desc,
            fullscreen_desc,
            restrict_to_output.is_some()
        ));
        Ok(RecordedSwapChain::new())
    }
    fn create_swap_chain_for_core_window(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        window: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &window;
        let _ = &desc;
        let _ = &restrict_to_output;
        record("IDXGIFactory2::CreateSwapChainForCoreWindow()".to_string());
        record_arguments(format!(
            "IDXGIFactory2::CreateSwapChainForCoreWindow(device=an object or the caller's bytes, window=an object or the caller's bytes, desc={:?}, restrict_to_output={:?})",
            desc,
            restrict_to_output.is_some()
        ));
        Ok(RecordedSwapChain::new())
    }
    fn get_shared_resource_adapter_luid(
        &self,
        h_resource: component_object_model::structures::Handle,
    ) -> Result<component_object_model::structures::Luid, component_object_model::structures::HResult>
    {
        let _ = &h_resource;
        record(format!(
            "IDXGIFactory2::GetSharedResourceAdapterLuid(h_resource={:?})",
            h_resource
        ));
        record_arguments(format!(
            "IDXGIFactory2::GetSharedResourceAdapterLuid(h_resource={:?})",
            h_resource
        ));
        Ok(component_object_model::structures::Luid {
            low_part: 87_u32,
            high_part: 94_i32,
        })
    }
    fn register_stereo_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &w_msg;
        record(format!(
            "IDXGIFactory2::RegisterStereoStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        record_arguments(format!(
            "IDXGIFactory2::RegisterStereoStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        Ok(99_u32)
    }
    fn register_stereo_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory2::RegisterStereoStatusEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIFactory2::RegisterStereoStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(125_u32)
    }
    fn unregister_stereo_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory2::UnregisterStereoStatus(dw_cookie={:?})",
            dw_cookie
        ));
        record_arguments(format!(
            "IDXGIFactory2::UnregisterStereoStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn register_occlusion_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &w_msg;
        record(format!(
            "IDXGIFactory2::RegisterOcclusionStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        record_arguments(format!(
            "IDXGIFactory2::RegisterOcclusionStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        Ok(16_u32)
    }
    fn register_occlusion_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory2::RegisterOcclusionStatusEvent(h_event={:?})",
            h_event
        ));
        record_arguments(format!(
            "IDXGIFactory2::RegisterOcclusionStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(85_u32)
    }
    fn unregister_occlusion_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory2::UnregisterOcclusionStatus(dw_cookie={:?})",
            dw_cookie
        ));
        record_arguments(format!(
            "IDXGIFactory2::UnregisterOcclusionStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn create_swap_chain_for_composition(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &desc;
        let _ = &restrict_to_output;
        record("IDXGIFactory2::CreateSwapChainForComposition()".to_string());
        record_arguments(format!(
            "IDXGIFactory2::CreateSwapChainForComposition(device=an object or the caller's bytes, desc={:?}, restrict_to_output={:?})",
            desc,
            restrict_to_output.is_some()
        ));
        Ok(RecordedSwapChain::new())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory1 for RecordedFactory<A> {
    fn enum_adapters1(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        record(format!(
            "IDXGIFactory1::EnumAdapters1(adapter={:?})",
            adapter
        ));
        record_arguments(format!(
            "IDXGIFactory1::EnumAdapters1(adapter={:?})",
            adapter
        ));
        Ok(RecordedAdapter::new())
    }
    fn is_current(&self) -> bool {
        record("IDXGIFactory1::IsCurrent()".to_string());
        record_arguments("IDXGIFactory1::IsCurrent()".to_string());
        true
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Factory for RecordedFactory<A> {
    fn enum_adapters(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        record(format!("IDXGIFactory::EnumAdapters(adapter={:?})", adapter));
        record_arguments(format!("IDXGIFactory::EnumAdapters(adapter={:?})", adapter));
        Ok(RecordedAdapter::new())
    }
    fn make_window_association(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        flags: directx_graphics_infrastructure::enumerations::MwaFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &flags;
        record(format!(
            "IDXGIFactory::MakeWindowAssociation(window_handle={:?}, flags={:?})",
            window_handle, flags
        ));
        record_arguments(format!(
            "IDXGIFactory::MakeWindowAssociation(window_handle={:?}, flags={:?})",
            window_handle, flags
        ));
        Ok(())
    }
    fn get_window_association(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        record("IDXGIFactory::GetWindowAssociation()".to_string());
        record_arguments("IDXGIFactory::GetWindowAssociation()".to_string());
        Ok(component_object_model::structures::Hwnd(49))
    }
    fn create_swap_chain(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &desc;
        record("IDXGIFactory::CreateSwapChain()".to_string());
        record_arguments(format!(
            "IDXGIFactory::CreateSwapChain(device=an object or the caller's bytes, desc={:?})",
            desc
        ));
        Ok(RecordedSwapChain::new())
    }
    fn create_software_adapter(
        &self,
        module: component_object_model::structures::Hmodule,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &module;
        record(format!(
            "IDXGIFactory::CreateSoftwareAdapter(module={:?})",
            module
        ));
        record_arguments(format!(
            "IDXGIFactory::CreateSoftwareAdapter(module={:?})",
            module
        ));
        Ok(RecordedAdapter::new())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedFactory<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::FactoryMedia for RecordedFactoryMedia<A> {
    type Api = A;

    fn create_swap_chain_for_composition_surface_handle(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_surface: component_object_model::structures::Handle,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &h_surface;
        let _ = &desc;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle(h_surface={:?})",
            h_surface
        ));
        record_arguments(format!(
            "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle(device=an object or the caller's bytes, h_surface={:?}, desc={:?}, restrict_to_output={:?})",
            h_surface,
            desc,
            restrict_to_output.is_some()
        ));
        Ok(RecordedSwapChain::new())
    }
    fn create_decode_swap_chain_for_composition_surface_handle(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, h_surface: component_object_model::structures::Handle, desc: &directx_graphics_infrastructure::structures::DecodeSwapChainDesc, yuv_decode_buffers: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource, restrict_to_output: Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain, component_object_model::structures::HResult>{
        let _ = &device;
        let _ = &h_surface;
        let _ = &desc;
        let _ = &yuv_decode_buffers;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle(h_surface={:?})",
            h_surface
        ));
        record_arguments(format!(
            "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle(device=an object or the caller's bytes, h_surface={:?}, desc={:?}, yuv_decode_buffers=an object or the caller's bytes, restrict_to_output={:?})",
            h_surface,
            desc,
            restrict_to_output.is_some()
        ));
        Ok(RecordedDecodeSwapChain::new())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::InfoQueue for RecordedInfoQueue<A> {
    fn set_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
        message_count_limit: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &message_count_limit;
        record(format!(
            "IDXGIInfoQueue::SetMessageCountLimit(producer={:?}, message_count_limit={:?})",
            producer, message_count_limit
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::SetMessageCountLimit(producer={:?}, message_count_limit={:?})",
            producer, message_count_limit
        ));
        Ok(())
    }
    fn clear_stored_messages(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearStoredMessages(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::ClearStoredMessages(producer={:?})",
            producer
        ));
    }
    fn get_message(
        &self,
        producer: component_object_model::structures::Guid,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &message_index;
        let _ = &message;
        record("IDXGIInfoQueue::GetMessage()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::GetMessage(producer={:?}, message_index={:?}, message=an object or the caller's bytes)",
            producer, message_index
        ));
        Err(E_NOTIMPL)
    }
    fn get_num_stored_messages_allowed_by_retrieval_filters(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters(producer={:?})",
            producer
        ));
        8_u64
    }
    fn get_num_stored_messages(&self, producer: component_object_model::structures::Guid) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumStoredMessages(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetNumStoredMessages(producer={:?})",
            producer
        ));
        54_u64
    }
    fn get_num_messages_discarded_by_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit(producer={:?})",
            producer
        ));
        47_u64
    }
    fn get_message_count_limit(&self, producer: component_object_model::structures::Guid) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetMessageCountLimit(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetMessageCountLimit(producer={:?})",
            producer
        ));
        65_u64
    }
    fn get_num_messages_allowed_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter(producer={:?})",
            producer
        ));
        91_u64
    }
    fn get_num_messages_denied_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter(producer={:?})",
            producer
        ));
        41_u64
    }
    fn add_storage_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::AddStorageFilterEntries()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::AddStorageFilterEntries(producer={:?}, filter={:?})",
            producer, filter
        ));
        Err(E_NOTIMPL)
    }
    fn get_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::GetStorageFilter()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::GetStorageFilter(producer={:?}, filter=an object or the caller's bytes)",
            producer
        ));
        Err(E_NOTIMPL)
    }
    fn clear_storage_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::ClearStorageFilter(producer={:?})",
            producer
        ));
    }
    fn push_empty_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushEmptyStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushEmptyStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_deny_all_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushDenyAllStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushDenyAllStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_copy_of_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushCopyOfStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushCopyOfStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::PushStorageFilter()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::PushStorageFilter(producer={:?}, filter={:?})",
            producer, filter
        ));
        Err(E_NOTIMPL)
    }
    fn pop_storage_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PopStorageFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PopStorageFilter(producer={:?})",
            producer
        ));
    }
    fn get_storage_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetStorageFilterStackSize(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetStorageFilterStackSize(producer={:?})",
            producer
        ));
        13_u32
    }
    fn add_retrieval_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::AddRetrievalFilterEntries()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::AddRetrievalFilterEntries(producer={:?}, filter={:?})",
            producer, filter
        ));
        Err(E_NOTIMPL)
    }
    fn get_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::GetRetrievalFilter()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::GetRetrievalFilter(producer={:?}, filter=an object or the caller's bytes)",
            producer
        ));
        Err(E_NOTIMPL)
    }
    fn clear_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearRetrievalFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::ClearRetrievalFilter(producer={:?})",
            producer
        ));
    }
    fn push_empty_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushEmptyRetrievalFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushEmptyRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_deny_all_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushDenyAllRetrievalFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushDenyAllRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_copy_of_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushCopyOfRetrievalFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PushCopyOfRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::PushRetrievalFilter()".to_string());
        record_arguments(format!(
            "IDXGIInfoQueue::PushRetrievalFilter(producer={:?}, filter={:?})",
            producer, filter
        ));
        Err(E_NOTIMPL)
    }
    fn pop_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PopRetrievalFilter(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::PopRetrievalFilter(producer={:?})",
            producer
        ));
    }
    fn get_retrieval_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetRetrievalFilterStackSize(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetRetrievalFilterStackSize(producer={:?})",
            producer
        ));
        41_u32
    }
    fn add_message(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        id: i32,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &category;
        let _ = &severity;
        let _ = &id;
        let _ = &description;
        record(format!(
            "IDXGIInfoQueue::AddMessage(producer={:?}, category={:?}, severity={:?}, id={:?})",
            producer, category, severity, id
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::AddMessage(producer={:?}, category={:?}, severity={:?}, id={:?}, description={:?})",
            producer, category, severity, id, description
        ));
        Ok(())
    }
    fn add_application_message(
        &self,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &severity;
        let _ = &description;
        record(format!(
            "IDXGIInfoQueue::AddApplicationMessage(severity={:?})",
            severity
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::AddApplicationMessage(severity={:?}, description={:?})",
            severity, description
        ));
        Ok(())
    }
    fn set_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &category;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnCategory(producer={:?}, category={:?}, b_enable={:?})",
            producer, category, b_enable
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::SetBreakOnCategory(producer={:?}, category={:?}, b_enable={:?})",
            producer, category, b_enable
        ));
        Ok(())
    }
    fn set_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &severity;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnSeverity(producer={:?}, severity={:?}, b_enable={:?})",
            producer, severity, b_enable
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::SetBreakOnSeverity(producer={:?}, severity={:?}, b_enable={:?})",
            producer, severity, b_enable
        ));
        Ok(())
    }
    fn set_break_on_id(
        &self,
        producer: component_object_model::structures::Guid,
        id: i32,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &id;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnID(producer={:?}, id={:?}, b_enable={:?})",
            producer, id, b_enable
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::SetBreakOnID(producer={:?}, id={:?}, b_enable={:?})",
            producer, id, b_enable
        ));
        Ok(())
    }
    fn get_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
    ) -> bool {
        let _ = &producer;
        let _ = &category;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnCategory(producer={:?}, category={:?})",
            producer, category
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetBreakOnCategory(producer={:?}, category={:?})",
            producer, category
        ));
        true
    }
    fn get_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
    ) -> bool {
        let _ = &producer;
        let _ = &severity;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnSeverity(producer={:?}, severity={:?})",
            producer, severity
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetBreakOnSeverity(producer={:?}, severity={:?})",
            producer, severity
        ));
        true
    }
    fn get_break_on_id(&self, producer: component_object_model::structures::Guid, id: i32) -> bool {
        let _ = &producer;
        let _ = &id;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnID(producer={:?}, id={:?})",
            producer, id
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetBreakOnID(producer={:?}, id={:?})",
            producer, id
        ));
        true
    }
    fn set_mute_debug_output(
        &self,
        producer: component_object_model::structures::Guid,
        b_mute: bool,
    ) {
        let _ = &producer;
        let _ = &b_mute;
        record(format!(
            "IDXGIInfoQueue::SetMuteDebugOutput(producer={:?}, b_mute={:?})",
            producer, b_mute
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::SetMuteDebugOutput(producer={:?}, b_mute={:?})",
            producer, b_mute
        ));
    }
    fn get_mute_debug_output(&self, producer: component_object_model::structures::Guid) -> bool {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetMuteDebugOutput(producer={:?})",
            producer
        ));
        record_arguments(format!(
            "IDXGIInfoQueue::GetMuteDebugOutput(producer={:?})",
            producer
        ));
        true
    }
}

impl<A: Recorded> directx_graphics_infrastructure::KeyedMutex for RecordedKeyedMutex<A> {
    fn acquire_sync(
        &self,
        key: u64,
        dw_milliseconds: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &key;
        let _ = &dw_milliseconds;
        record(format!(
            "IDXGIKeyedMutex::AcquireSync(key={:?}, dw_milliseconds={:?})",
            key, dw_milliseconds
        ));
        record_arguments(format!(
            "IDXGIKeyedMutex::AcquireSync(key={:?}, dw_milliseconds={:?})",
            key, dw_milliseconds
        ));
        Ok(())
    }
    fn release_sync(&self, key: u64) -> Result<(), component_object_model::structures::HResult> {
        let _ = &key;
        record(format!("IDXGIKeyedMutex::ReleaseSync(key={:?})", key));
        record_arguments(format!("IDXGIKeyedMutex::ReleaseSync(key={:?})", key));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedKeyedMutex<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        record_arguments(format!("IDXGIDeviceSubObject::GetDevice(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedKeyedMutex<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output6 for RecordedOutput<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput6::GetDesc1()".to_string());
        record_arguments("IDXGIOutput6::GetDesc1()".to_string());
        Ok(directx_graphics_infrastructure::structures::OutputDesc1 {
            device_name: core::array::from_fn(|_| 54_u16),
            desktop_coordinates: component_object_model::structures::Rect {
                left: 68_i32,
                top: 75_i32,
                right: 82_i32,
                bottom: 89_i32,
            },
            attached_to_desktop: true,
            rotation: directx_graphics_infrastructure::enumerations::ModeRotation(75),
            monitor: component_object_model::structures::Hmonitor(82),
            bits_per_color: 89_u32,
            color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType(96),
            red_primary: core::array::from_fn(|_| 103.5_f32),
            green_primary: core::array::from_fn(|_| 110.5_f32),
            blue_primary: core::array::from_fn(|_| 117.5_f32),
            white_point: core::array::from_fn(|_| 124.5_f32),
            min_luminance: 5.5_f32,
            max_luminance: 12.5_f32,
            max_full_frame_luminance: 19.5_f32,
        })
    }
    fn check_hardware_composition_support(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGIOutput6::CheckHardwareCompositionSupport()".to_string());
        record_arguments("IDXGIOutput6::CheckHardwareCompositionSupport()".to_string());
        Ok(55_u32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output5 for RecordedOutput<A> {
    fn duplicate_output1(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, flags: u32, supported_formats: &[directx_graphics_infrastructure::enumerations::Format]) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let _ = &device;
        let _ = &flags;
        let _ = &supported_formats;
        record(format!("IDXGIOutput5::DuplicateOutput1(flags={:?})", flags));
        record_arguments(format!(
            "IDXGIOutput5::DuplicateOutput1(device=an object or the caller's bytes, flags={:?}, supported_formats={:?})",
            flags, supported_formats
        ));
        Ok(RecordedOutputDuplication::new())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output4 for RecordedOutput<A> {
    fn check_overlay_color_space_support(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        let _ = &color_space;
        let _ = &concerned_device;
        record(format!(
            "IDXGIOutput4::CheckOverlayColorSpaceSupport(format={:?}, color_space={:?})",
            format, color_space
        ));
        record_arguments(format!(
            "IDXGIOutput4::CheckOverlayColorSpaceSupport(format={:?}, color_space={:?}, concerned_device=an object or the caller's bytes)",
            format, color_space
        ));
        Ok(26_u32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output3 for RecordedOutput<A> {
    fn check_overlay_support(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &enum_format;
        let _ = &concerned_device;
        record(format!(
            "IDXGIOutput3::CheckOverlaySupport(enum_format={:?})",
            enum_format
        ));
        record_arguments(format!(
            "IDXGIOutput3::CheckOverlaySupport(enum_format={:?}, concerned_device=an object or the caller's bytes)",
            enum_format
        ));
        Ok(64_u32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output2 for RecordedOutput<A> {
    fn supports_overlays(&self) -> bool {
        record("IDXGIOutput2::SupportsOverlays()".to_string());
        record_arguments("IDXGIOutput2::SupportsOverlays()".to_string());
        true
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output1 for RecordedOutput<A> {
    fn get_display_mode_list1(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc1>,
        component_object_model::structures::HResult,
    > {
        let _ = &enum_format;
        let _ = &flags;
        record(format!(
            "IDXGIOutput1::GetDisplayModeList1(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        record_arguments(format!(
            "IDXGIOutput1::GetDisplayModeList1(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        Ok((0..2_u32)
            .map(|_| directx_graphics_infrastructure::structures::ModeDesc1 {
                width: 116_u32,
                height: 123_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 11_u32,
                    denominator: 18_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(11),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    18,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(25),
                stereo: true,
            })
            .collect())
    }
    fn find_closest_matching_mode1(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc1,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc1,
        component_object_model::structures::HResult,
    > {
        let _ = &mode_to_match;
        let _ = &concerned_device;
        record("IDXGIOutput1::FindClosestMatchingMode1()".to_string());
        record_arguments(format!(
            "IDXGIOutput1::FindClosestMatchingMode1(mode_to_match={:?}, concerned_device={:?})",
            mode_to_match,
            concerned_device.is_some()
        ));
        Ok(directx_graphics_infrastructure::structures::ModeDesc1 {
            width: 9_u32,
            height: 16_u32,
            refresh_rate: directx_graphics_infrastructure::structures::Rational {
                numerator: 30_u32,
                denominator: 37_u32,
            },
            format: directx_graphics_infrastructure::enumerations::Format(30),
            scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(37),
            scaling: directx_graphics_infrastructure::enumerations::ModeScaling(44),
            stereo: true,
        })
    }
    fn get_display_surface_data1(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &destination;
        record("IDXGIOutput1::GetDisplaySurfaceData1()".to_string());
        record_arguments(
            "IDXGIOutput1::GetDisplaySurfaceData1(destination=an object or the caller's bytes)"
                .to_string(),
        );
        Ok(())
    }
    fn duplicate_output(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let _ = &device;
        record("IDXGIOutput1::DuplicateOutput()".to_string());
        record_arguments(
            "IDXGIOutput1::DuplicateOutput(device=an object or the caller's bytes)".to_string(),
        );
        Ok(RecordedOutputDuplication::new())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Output for RecordedOutput<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetDesc()".to_string());
        record_arguments("IDXGIOutput::GetDesc()".to_string());
        Ok(directx_graphics_infrastructure::structures::OutputDesc {
            device_name: core::array::from_fn(|_| 112_u16),
            desktop_coordinates: component_object_model::structures::Rect {
                left: 126_i32,
                top: 7_i32,
                right: 14_i32,
                bottom: 21_i32,
            },
            attached_to_desktop: true,
            rotation: directx_graphics_infrastructure::enumerations::ModeRotation(7),
            monitor: component_object_model::structures::Hmonitor(14),
        })
    }
    fn get_display_mode_list(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc>,
        component_object_model::structures::HResult,
    > {
        let _ = &enum_format;
        let _ = &flags;
        record(format!(
            "IDXGIOutput::GetDisplayModeList(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        record_arguments(format!(
            "IDXGIOutput::GetDisplayModeList(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        Ok((0..2_u32)
            .map(|_| directx_graphics_infrastructure::structures::ModeDesc {
                width: 112_u32,
                height: 119_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 7_u32,
                    denominator: 14_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(7),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    14,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(21),
            })
            .collect())
    }
    fn find_closest_matching_mode(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc,
        component_object_model::structures::HResult,
    > {
        let _ = &mode_to_match;
        let _ = &concerned_device;
        record("IDXGIOutput::FindClosestMatchingMode()".to_string());
        record_arguments(format!(
            "IDXGIOutput::FindClosestMatchingMode(mode_to_match={:?}, concerned_device={:?})",
            mode_to_match,
            concerned_device.is_some()
        ));
        Ok(directx_graphics_infrastructure::structures::ModeDesc {
            width: 121_u32,
            height: 2_u32,
            refresh_rate: directx_graphics_infrastructure::structures::Rational {
                numerator: 16_u32,
                denominator: 23_u32,
            },
            format: directx_graphics_infrastructure::enumerations::Format(16),
            scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(23),
            scaling: directx_graphics_infrastructure::enumerations::ModeScaling(30),
        })
    }
    fn wait_for_v_blank(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutput::WaitForVBlank()".to_string());
        record_arguments("IDXGIOutput::WaitForVBlank()".to_string());
        Ok(())
    }
    fn take_ownership(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        exclusive: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &device;
        let _ = &exclusive;
        record(format!(
            "IDXGIOutput::TakeOwnership(exclusive={:?})",
            exclusive
        ));
        record_arguments(format!(
            "IDXGIOutput::TakeOwnership(device=an object or the caller's bytes, exclusive={:?})",
            exclusive
        ));
        Ok(())
    }
    fn release_ownership(&self) {
        record("IDXGIOutput::ReleaseOwnership()".to_string());
        record_arguments("IDXGIOutput::ReleaseOwnership()".to_string());
    }
    fn get_gamma_control_capabilities(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControlCapabilities,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetGammaControlCapabilities()".to_string());
        record_arguments("IDXGIOutput::GetGammaControlCapabilities()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::GammaControlCapabilities {
                scale_and_offset_supported: true,
                max_converted_value: 29.5_f32,
                min_converted_value: 36.5_f32,
                num_gamma_control_points: 43_u32,
                control_point_positions: core::array::from_fn(|_| 50.5_f32),
            },
        )
    }
    fn set_gamma_control(
        &self,
        array: &directx_graphics_infrastructure::structures::GammaControl,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &array;
        record("IDXGIOutput::SetGammaControl()".to_string());
        record_arguments(format!("IDXGIOutput::SetGammaControl(array={:?})", array));
        Ok(())
    }
    fn get_gamma_control(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControl,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetGammaControl()".to_string());
        record_arguments("IDXGIOutput::GetGammaControl()".to_string());
        Ok(directx_graphics_infrastructure::structures::GammaControl {
            scale: directx_graphics_infrastructure::structures::Rgb {
                red: 28.5_f32,
                green: 35.5_f32,
                blue: 42.5_f32,
            },
            offset: directx_graphics_infrastructure::structures::Rgb {
                red: 35.5_f32,
                green: 42.5_f32,
                blue: 49.5_f32,
            },
            gamma_curve: core::array::from_fn(|_| {
                directx_graphics_infrastructure::structures::Rgb {
                    red: 42.5_f32,
                    green: 49.5_f32,
                    blue: 56.5_f32,
                }
            }),
        })
    }
    fn set_display_surface(
        &self,
        scanout_surface: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &scanout_surface;
        record("IDXGIOutput::SetDisplaySurface()".to_string());
        record_arguments(
            "IDXGIOutput::SetDisplaySurface(scanout_surface=an object or the caller's bytes)"
                .to_string(),
        );
        Ok(())
    }
    fn get_display_surface_data(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &destination;
        record("IDXGIOutput::GetDisplaySurfaceData()".to_string());
        record_arguments(
            "IDXGIOutput::GetDisplaySurfaceData(destination=an object or the caller's bytes)"
                .to_string(),
        );
        Ok(())
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetFrameStatistics()".to_string());
        record_arguments("IDXGIOutput::GetFrameStatistics()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::FrameStatistics {
                present_count: 47_u32,
                present_refresh_count: 54_u32,
                sync_refresh_count: 61_u32,
                sync_qpc_time: 68_i64,
                sync_gpu_time: 75_i64,
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedOutput<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::OutputDuplication
    for RecordedOutputDuplication<A>
{
    fn get_desc(&self) -> directx_graphics_infrastructure::structures::OutduplDesc {
        record("IDXGIOutputDuplication::GetDesc()".to_string());
        record_arguments("IDXGIOutputDuplication::GetDesc()".to_string());
        directx_graphics_infrastructure::structures::OutduplDesc {
            mode_desc: directx_graphics_infrastructure::structures::ModeDesc {
                width: 109_u32,
                height: 116_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 4_u32,
                    denominator: 11_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(4),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    11,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(18),
            },
            rotation: directx_graphics_infrastructure::enumerations::ModeRotation(109),
            desktop_image_in_system_memory: true,
        }
    }
    fn acquire_next_frame(
        &self,
        timeout_in_milliseconds: u32,
    ) -> Result<
        (
            directx_graphics_infrastructure::structures::OutduplFrameInfo,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &timeout_in_milliseconds;
        record(format!(
            "IDXGIOutputDuplication::AcquireNextFrame(timeout_in_milliseconds={:?})",
            timeout_in_milliseconds
        ));
        record_arguments(format!(
            "IDXGIOutputDuplication::AcquireNextFrame(timeout_in_milliseconds={:?})",
            timeout_in_milliseconds
        ));
        Ok((
            directx_graphics_infrastructure::structures::OutduplFrameInfo {
                last_present_time: 5_i64,
                last_mouse_update_time: 12_i64,
                accumulated_frames: 19_u32,
                rects_coalesced: true,
                protected_content_masked_out: true,
                pointer_position:
                    directx_graphics_infrastructure::structures::OutduplPointerPosition {
                        position: component_object_model::structures::Point {
                            x: 54_i32,
                            y: 61_i32,
                        },
                        visible: true,
                    },
                total_metadata_buffer_size: 47_u32,
                pointer_shape_buffer_size: 54_u32,
            },
            RecordedResource::new(),
        ))
    }
    fn get_frame_dirty_rects(
        &self,
        dirty_rects_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &dirty_rects_buffer;
        record("IDXGIOutputDuplication::GetFrameDirtyRects()".to_string());
        record_arguments("IDXGIOutputDuplication::GetFrameDirtyRects(dirty_rects_buffer=an object or the caller's bytes)".to_string());
        known_fill(dirty_rects_buffer, 677566);
        Ok(65_u32)
    }
    fn get_frame_move_rects(
        &self,
        move_rect_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &move_rect_buffer;
        record("IDXGIOutputDuplication::GetFrameMoveRects()".to_string());
        record_arguments("IDXGIOutputDuplication::GetFrameMoveRects(move_rect_buffer=an object or the caller's bytes)".to_string());
        known_fill(move_rect_buffer, 340890);
        Ok(61_u32)
    }
    fn get_frame_pointer_shape(
        &self,
        pointer_shape_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<
        (
            u32,
            directx_graphics_infrastructure::structures::OutduplPointerShapeInfo,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &pointer_shape_buffer;
        record("IDXGIOutputDuplication::GetFramePointerShape()".to_string());
        record_arguments("IDXGIOutputDuplication::GetFramePointerShape(pointer_shape_buffer=an object or the caller's bytes)".to_string());
        known_fill(pointer_shape_buffer, 322244);
        Ok((
            63_u32,
            directx_graphics_infrastructure::structures::OutduplPointerShapeInfo {
                type_: 77_u32,
                width: 84_u32,
                height: 91_u32,
                pitch: 98_u32,
                hot_spot: component_object_model::structures::Point {
                    x: 112_i32,
                    y: 119_i32,
                },
            },
        ))
    }
    fn map_desktop_surface(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutputDuplication::MapDesktopSurface()".to_string());
        record_arguments("IDXGIOutputDuplication::MapDesktopSurface()".to_string());
        Err(E_NOTIMPL)
    }
    fn un_map_desktop_surface(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutputDuplication::UnMapDesktopSurface()".to_string());
        record_arguments("IDXGIOutputDuplication::UnMapDesktopSurface()".to_string());
        Ok(())
    }
    fn release_frame(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutputDuplication::ReleaseFrame()".to_string());
        record_arguments("IDXGIOutputDuplication::ReleaseFrame()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedOutputDuplication<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource1 for RecordedResource<A> {
    fn create_subresource_surface(
        &self,
        index: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
        component_object_model::structures::HResult,
    > {
        let _ = &index;
        record(format!(
            "IDXGIResource1::CreateSubresourceSurface(index={:?})",
            index
        ));
        record_arguments(format!(
            "IDXGIResource1::CreateSubresourceSurface(index={:?})",
            index
        ));
        Ok(RecordedSurface::new())
    }
    fn create_shared_handle(
        &self,
        attributes: Option<&component_object_model::structures::SecurityAttributes>,
        dw_access: u32,
        lp_name: Option<&[u16]>,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        let _ = &attributes;
        let _ = &dw_access;
        let _ = &lp_name;
        record("IDXGIResource1::CreateSharedHandle()".to_string());
        record_arguments(format!(
            "IDXGIResource1::CreateSharedHandle(attributes={:?}, dw_access={:?}, lp_name={:?})",
            attributes, dw_access, lp_name
        ));
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource for RecordedResource<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        record_arguments("IDXGIResource::GetSharedHandle()".to_string());
        Ok(component_object_model::structures::Handle(54))
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        record_arguments("IDXGIResource::GetUsage()".to_string());
        Ok(directx_graphics_infrastructure::enumerations::Usage(113))
    }
    fn set_eviction_priority(
        &self,
        eviction_priority: directx_graphics_infrastructure::enumerations::ResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &eviction_priority;
        record(format!(
            "IDXGIResource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
        record_arguments(format!(
            "IDXGIResource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        record_arguments("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(directx_graphics_infrastructure::enumerations::ResourcePriority(94))
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedResource<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        record_arguments(format!("IDXGIDeviceSubObject::GetDevice(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedResource<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface2 for RecordedSurface<A> {
    fn get_resource(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        (
            directx_graphics_infrastructure::UnknownObject<Self::Api>,
            u32,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGISurface2::GetResource()".to_string());
        record_arguments(format!("IDXGISurface2::GetResource(riid={:?})", riid));
        Ok((
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
            57_u32,
        ))
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface1 for RecordedSurface<A> {
    fn get_dc(
        &self,
        discard: bool,
    ) -> Result<component_object_model::structures::Hdc, component_object_model::structures::HResult>
    {
        let _ = &discard;
        record(format!("IDXGISurface1::GetDC(discard={:?})", discard));
        record_arguments(format!("IDXGISurface1::GetDC(discard={:?})", discard));
        Ok(component_object_model::structures::Hdc(15))
    }
    fn release_dc(
        &self,
        dirty_rect: Option<&component_object_model::structures::Rect>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dirty_rect;
        record("IDXGISurface1::ReleaseDC()".to_string());
        record_arguments(format!(
            "IDXGISurface1::ReleaseDC(dirty_rect={:?})",
            dirty_rect
        ));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface for RecordedSurface<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SurfaceDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISurface::GetDesc()".to_string());
        record_arguments("IDXGISurface::GetDesc()".to_string());
        Ok(directx_graphics_infrastructure::structures::SurfaceDesc {
            width: 84_u32,
            height: 91_u32,
            format: directx_graphics_infrastructure::enumerations::Format(98),
            sample_desc: directx_graphics_infrastructure::structures::SampleDesc {
                count: 112_u32,
                quality: 119_u32,
            },
        })
    }
    fn map(
        &self,
        map_flags: directx_graphics_infrastructure::enumerations::MapFlags,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        let _ = &map_flags;
        record("IDXGISurface::Map()".to_string());
        record_arguments(format!("IDXGISurface::Map(map_flags={:?})", map_flags));
        Err(E_NOTIMPL)
    }
    fn unmap(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGISurface::Unmap()".to_string());
        record_arguments("IDXGISurface::Unmap()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedSurface<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        record_arguments(format!("IDXGIDeviceSubObject::GetDevice(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedSurface<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChain4 for RecordedSwapChain<A> {
    fn set_hdr_meta_data(
        &self,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        meta_data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &type_;
        let _ = &meta_data;
        record(format!(
            "IDXGISwapChain4::SetHDRMetaData(type_={:?})",
            type_
        ));
        record_arguments(format!(
            "IDXGISwapChain4::SetHDRMetaData(type_={:?}, meta_data={:?})",
            type_, meta_data
        ));
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChain3 for RecordedSwapChain<A> {
    fn get_current_back_buffer_index(&self) -> u32 {
        record("IDXGISwapChain3::GetCurrentBackBufferIndex()".to_string());
        record_arguments("IDXGISwapChain3::GetCurrentBackBufferIndex()".to_string());
        70_u32
    }
    fn check_color_space_support(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGISwapChain3::CheckColorSpaceSupport(color_space={:?})",
            color_space
        ));
        record_arguments(format!(
            "IDXGISwapChain3::CheckColorSpaceSupport(color_space={:?})",
            color_space
        ));
        Ok(76_u32)
    }
    fn set_color_space1(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGISwapChain3::SetColorSpace1(color_space={:?})",
            color_space
        ));
        record_arguments(format!(
            "IDXGISwapChain3::SetColorSpace1(color_space={:?})",
            color_space
        ));
        Ok(())
    }
    fn resize_buffers1(
        &self,
        width: u32,
        height: u32,
        format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
        creation_node_mask: &[u32],
        present_queue: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        let _ = &format;
        let _ = &swap_chain_flags;
        let _ = &creation_node_mask;
        let _ = &present_queue;
        record("IDXGISwapChain3::ResizeBuffers1()".to_string());
        record_arguments(format!(
            "IDXGISwapChain3::ResizeBuffers1(width={:?}, height={:?}, format={:?}, swap_chain_flags={:?}, creation_node_mask={:?}, present_queue={:?})",
            width,
            height,
            format,
            swap_chain_flags,
            creation_node_mask,
            present_queue
                .iter()
                .map(|item| item.is_some())
                .collect::<Vec<_>>()
        ));
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChain2 for RecordedSwapChain<A> {
    fn set_source_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        record(format!(
            "IDXGISwapChain2::SetSourceSize(width={:?}, height={:?})",
            width, height
        ));
        record_arguments(format!(
            "IDXGISwapChain2::SetSourceSize(width={:?}, height={:?})",
            width, height
        ));
        Ok(())
    }
    fn get_source_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        record("IDXGISwapChain2::GetSourceSize()".to_string());
        record_arguments("IDXGISwapChain2::GetSourceSize()".to_string());
        Ok((1_u32, 8_u32))
    }
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &max_latency;
        record(format!(
            "IDXGISwapChain2::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        record_arguments(format!(
            "IDXGISwapChain2::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGISwapChain2::GetMaximumFrameLatency()".to_string());
        record_arguments("IDXGISwapChain2::GetMaximumFrameLatency()".to_string());
        Ok(55_u32)
    }
    fn get_frame_latency_waitable_object(&self) -> component_object_model::structures::Handle {
        record("IDXGISwapChain2::GetFrameLatencyWaitableObject()".to_string());
        record_arguments("IDXGISwapChain2::GetFrameLatencyWaitableObject()".to_string());
        component_object_model::structures::Handle(5)
    }
    fn set_matrix_transform(
        &self,
        matrix: &directx_graphics_infrastructure::structures::Matrix3x2F,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &matrix;
        record("IDXGISwapChain2::SetMatrixTransform()".to_string());
        record_arguments(format!(
            "IDXGISwapChain2::SetMatrixTransform(matrix={:?})",
            matrix
        ));
        Ok(())
    }
    fn get_matrix_transform(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Matrix3x2F,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain2::GetMatrixTransform()".to_string());
        record_arguments("IDXGISwapChain2::GetMatrixTransform()".to_string());
        Ok(directx_graphics_infrastructure::structures::Matrix3x2F {
            _11: 83.5_f32,
            _12: 90.5_f32,
            _21: 97.5_f32,
            _22: 104.5_f32,
            _31: 111.5_f32,
            _32: 118.5_f32,
        })
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChain1 for RecordedSwapChain<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetDesc1()".to_string());
        record_arguments("IDXGISwapChain1::GetDesc1()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::SwapChainDesc1 {
                width: 5_u32,
                height: 12_u32,
                format: directx_graphics_infrastructure::enumerations::Format(19),
                stereo: true,
                sample_desc: directx_graphics_infrastructure::structures::SampleDesc {
                    count: 40_u32,
                    quality: 47_u32,
                },
                buffer_usage: directx_graphics_infrastructure::enumerations::Usage(40),
                buffer_count: 47_u32,
                scaling: directx_graphics_infrastructure::enumerations::Scaling(54),
                swap_effect: directx_graphics_infrastructure::enumerations::SwapEffect(61),
                alpha_mode: directx_graphics_infrastructure::enumerations::AlphaMode(68),
                flags: 75_u32,
            },
        )
    }
    fn get_fullscreen_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetFullscreenDesc()".to_string());
        record_arguments("IDXGISwapChain1::GetFullscreenDesc()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::SwapChainFullscreenDesc {
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 62_u32,
                    denominator: 69_u32,
                },
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    62,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(69),
                windowed: true,
            },
        )
    }
    fn get_hwnd(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        record("IDXGISwapChain1::GetHwnd()".to_string());
        record_arguments("IDXGISwapChain1::GetHwnd()".to_string());
        Ok(component_object_model::structures::Hwnd(47))
    }
    fn get_core_window(
        &self,
        refiid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &refiid;
        record("IDXGISwapChain1::GetCoreWindow()".to_string());
        record_arguments(format!(
            "IDXGISwapChain1::GetCoreWindow(refiid={:?})",
            refiid
        ));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn present1(
        &self,
        sync_interval: u32,
        present_flags: directx_graphics_infrastructure::enumerations::Present,
        present_parameters: &directx_graphics_infrastructure::structures::PresentParameters,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &sync_interval;
        let _ = &present_flags;
        let _ = &present_parameters;
        record("IDXGISwapChain1::Present1()".to_string());
        record_arguments(format!(
            "IDXGISwapChain1::Present1(sync_interval={:?}, present_flags={:?}, present_parameters={:?})",
            sync_interval, present_flags, present_parameters
        ));
        Err(E_NOTIMPL)
    }
    fn is_temporary_mono_supported(&self) -> bool {
        record("IDXGISwapChain1::IsTemporaryMonoSupported()".to_string());
        record_arguments("IDXGISwapChain1::IsTemporaryMonoSupported()".to_string());
        true
    }
    fn get_restrict_to_output(
        &self,
    ) -> Result<
        Option<
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetRestrictToOutput()".to_string());
        record_arguments("IDXGISwapChain1::GetRestrictToOutput()".to_string());
        Ok(Some(RecordedOutput::new()))
    }
    fn set_background_color(
        &self,
        color: &directx_graphics_infrastructure::structures::Rgba,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color;
        record("IDXGISwapChain1::SetBackgroundColor()".to_string());
        record_arguments(format!(
            "IDXGISwapChain1::SetBackgroundColor(color={:?})",
            color
        ));
        Ok(())
    }
    fn get_background_color(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Rgba,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetBackgroundColor()".to_string());
        record_arguments("IDXGISwapChain1::GetBackgroundColor()".to_string());
        Ok(directx_graphics_infrastructure::structures::Rgba {
            r: 46.5_f32,
            g: 53.5_f32,
            b: 60.5_f32,
            a: 67.5_f32,
        })
    }
    fn set_rotation(
        &self,
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rotation;
        record(format!(
            "IDXGISwapChain1::SetRotation(rotation={:?})",
            rotation
        ));
        record_arguments(format!(
            "IDXGISwapChain1::SetRotation(rotation={:?})",
            rotation
        ));
        Ok(())
    }
    fn get_rotation(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ModeRotation,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetRotation()".to_string());
        record_arguments("IDXGISwapChain1::GetRotation()".to_string());
        Ok(directx_graphics_infrastructure::enumerations::ModeRotation(
            119,
        ))
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChain for RecordedSwapChain<A> {
    fn present(
        &self,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &sync_interval;
        let _ = &flags;
        record(format!(
            "IDXGISwapChain::Present(sync_interval={:?}, flags={:?})",
            sync_interval, flags
        ));
        record_arguments(format!(
            "IDXGISwapChain::Present(sync_interval={:?}, flags={:?})",
            sync_interval, flags
        ));
        Ok(())
    }
    fn get_buffer(
        &self,
        buffer: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer,
        component_object_model::structures::HResult,
    > {
        let _ = &buffer;
        record(format!("IDXGISwapChain::GetBuffer(buffer={:?})", buffer));
        record_arguments(format!("IDXGISwapChain::GetBuffer(buffer={:?})", buffer));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn set_fullscreen_state(
        &self,
        fullscreen: bool,
        target: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fullscreen;
        let _ = &target;
        record(format!(
            "IDXGISwapChain::SetFullscreenState(fullscreen={:?})",
            fullscreen
        ));
        record_arguments(format!(
            "IDXGISwapChain::SetFullscreenState(fullscreen={:?}, target={:?})",
            fullscreen,
            target.is_some()
        ));
        Ok(())
    }
    fn get_fullscreen_state(
        &self,
    ) -> Result<
        (
            bool,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        ),
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetFullscreenState()".to_string());
        record_arguments("IDXGISwapChain::GetFullscreenState()".to_string());
        Ok((true, RecordedOutput::new()))
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetDesc()".to_string());
        record_arguments("IDXGISwapChain::GetDesc()".to_string());
        Ok(directx_graphics_infrastructure::structures::SwapChainDesc {
            buffer_desc: directx_graphics_infrastructure::structures::ModeDesc {
                width: 92_u32,
                height: 99_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 113_u32,
                    denominator: 120_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(113),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    120,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(1),
            },
            sample_desc: directx_graphics_infrastructure::structures::SampleDesc {
                count: 99_u32,
                quality: 106_u32,
            },
            buffer_usage: directx_graphics_infrastructure::enumerations::Usage(99),
            buffer_count: 106_u32,
            output_window: component_object_model::structures::Hwnd(113),
            windowed: true,
            swap_effect: directx_graphics_infrastructure::enumerations::SwapEffect(1),
            flags: 8_u32,
        })
    }
    fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &buffer_count;
        let _ = &width;
        let _ = &height;
        let _ = &new_format;
        let _ = &swap_chain_flags;
        record(format!(
            "IDXGISwapChain::ResizeBuffers(buffer_count={:?}, width={:?}, height={:?}, new_format={:?}, swap_chain_flags={:?})",
            buffer_count, width, height, new_format, swap_chain_flags
        ));
        record_arguments(format!(
            "IDXGISwapChain::ResizeBuffers(buffer_count={:?}, width={:?}, height={:?}, new_format={:?}, swap_chain_flags={:?})",
            buffer_count, width, height, new_format, swap_chain_flags
        ));
        Ok(())
    }
    fn resize_target(
        &self,
        new_target_parameters: &directx_graphics_infrastructure::structures::ModeDesc,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &new_target_parameters;
        record("IDXGISwapChain::ResizeTarget()".to_string());
        record_arguments(format!(
            "IDXGISwapChain::ResizeTarget(new_target_parameters={:?})",
            new_target_parameters
        ));
        Ok(())
    }
    fn get_containing_output(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetContainingOutput()".to_string());
        record_arguments("IDXGISwapChain::GetContainingOutput()".to_string());
        Ok(RecordedOutput::new())
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetFrameStatistics()".to_string());
        record_arguments("IDXGISwapChain::GetFrameStatistics()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::FrameStatistics {
                present_count: 112_u32,
                present_refresh_count: 119_u32,
                sync_refresh_count: 126_u32,
                sync_qpc_time: 7_i64,
                sync_gpu_time: 14_i64,
            },
        )
    }
    fn get_last_present_count(&self) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGISwapChain::GetLastPresentCount()".to_string());
        record_arguments("IDXGISwapChain::GetLastPresentCount()".to_string());
        Ok(8_u32)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedSwapChain<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        record_arguments(format!("IDXGIDeviceSubObject::GetDevice(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedSwapChain<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateData(name={:?}, data={:?})",
            name, data
        ));
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        record_arguments(format!(
            "IDXGIObject::SetPrivateDataInterface(name={:?}, unknown={:?})",
            name,
            unknown.is_some()
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        record_arguments(format!(
            "IDXGIObject::GetPrivateData(name={:?}, data=an object or the caller's bytes)",
            name
        ));
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        record_arguments(format!("IDXGIObject::GetParent(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::SwapChainMedia for RecordedSwapChainMedia<A> {
    fn get_frame_statistics_media(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatisticsMedia,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChainMedia::GetFrameStatisticsMedia()".to_string());
        record_arguments("IDXGISwapChainMedia::GetFrameStatisticsMedia()".to_string());
        Ok(
            directx_graphics_infrastructure::structures::FrameStatisticsMedia {
                present_count: 73_u32,
                present_refresh_count: 80_u32,
                sync_refresh_count: 87_u32,
                sync_qpc_time: 94_i64,
                sync_gpu_time: 101_i64,
                composition_mode:
                    directx_graphics_infrastructure::enumerations::FramePresentationMode(108),
                approved_present_duration: 115_u32,
            },
        )
    }
    fn set_present_duration(
        &self,
        duration: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &duration;
        record(format!(
            "IDXGISwapChainMedia::SetPresentDuration(duration={:?})",
            duration
        ));
        record_arguments(format!(
            "IDXGISwapChainMedia::SetPresentDuration(duration={:?})",
            duration
        ));
        Ok(())
    }
    fn check_present_duration_support(
        &self,
        desired_present_duration: u32,
    ) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let _ = &desired_present_duration;
        record(format!(
            "IDXGISwapChainMedia::CheckPresentDurationSupport(desired_present_duration={:?})",
            desired_present_duration
        ));
        record_arguments(format!(
            "IDXGISwapChainMedia::CheckPresentDurationSupport(desired_present_duration={:?})",
            desired_present_duration
        ));
        Ok((59_u32, 66_u32))
    }
}

impl<A: Recorded> directx_graphics_infrastructure::GraphicsAnalysis
    for RecordedGraphicsAnalysis<A>
{
    fn begin_capture(&self) {
        record("IDXGraphicsAnalysis::BeginCapture()".to_string());
        record_arguments("IDXGraphicsAnalysis::BeginCapture()".to_string());
    }
    fn end_capture(&self) {
        record("IDXGraphicsAnalysis::EndCapture()".to_string());
        record_arguments("IDXGraphicsAnalysis::EndCapture()".to_string());
    }
}

/// The recorder's library: `DirectXGraphicsInfrastructure` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Recorder;

impl directx_graphics_infrastructure::DirectXGraphicsInfrastructure for Recorder {
    type Adapter = RecordedAdapter<Self>;
    type Debug = RecordedDebug<Self>;
    type DecodeSwapChain = RecordedDecodeSwapChain<Self>;
    type Device = RecordedDevice<Self>;
    type DisplayControl = RecordedDisplayControl<Self>;
    type Factory = RecordedFactory<Self>;
    type FactoryMedia = RecordedFactoryMedia<Self>;
    type InfoQueue = RecordedInfoQueue<Self>;
    type KeyedMutex = RecordedKeyedMutex<Self>;
    type Output = RecordedOutput<Self>;
    type OutputDuplication = RecordedOutputDuplication<Self>;
    type Resource = RecordedResource<Self>;
    type Surface = RecordedSurface<Self>;
    type SwapChain = RecordedSwapChain<Self>;
    type SwapChainMedia = RecordedSwapChainMedia<Self>;
    type GraphicsAnalysis = RecordedGraphicsAnalysis<Self>;
    type Buffer = RecordedSurface<Self>;

    fn create_dxgi_factory(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("CreateDXGIFactory()".to_string());
        record_arguments(format!("CreateDXGIFactory(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_dxgi_factory1(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("CreateDXGIFactory1()".to_string());
        record_arguments(format!("CreateDXGIFactory1(riid={:?})", riid));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_dxgi_factory2(
        &self,
        flags: directx_graphics_infrastructure::enumerations::CreateFactoryFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &flags;
        let _ = &riid;
        record(format!("CreateDXGIFactory2(flags={:?})", flags));
        record_arguments(format!(
            "CreateDXGIFactory2(flags={:?}, riid={:?})",
            flags, riid
        ));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn dxgi_declare_adapter_removal_support(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("DXGIDeclareAdapterRemovalSupport()".to_string());
        record_arguments("DXGIDeclareAdapterRemovalSupport()".to_string());
        Ok(())
    }
    fn dxgi_disable_v_blank_virtualization(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("DXGIDisableVBlankVirtualization()".to_string());
        record_arguments("DXGIDisableVBlankVirtualization()".to_string());
        Ok(())
    }
    fn dxgi_get_debug_interface1(
        &self,
        flags: u32,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &flags;
        let _ = &riid;
        record(format!("DXGIGetDebugInterface1(flags={:?})", flags));
        record_arguments(format!(
            "DXGIGetDebugInterface1(flags={:?}, riid={:?})",
            flags, riid
        ));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(RecordedAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(RecordedDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    RecordedDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(RecordedDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    RecordedDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(RecordedFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(RecordedKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(RecordedOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(RecordedResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(RecordedSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(RecordedSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl Recorded for Recorder {}
