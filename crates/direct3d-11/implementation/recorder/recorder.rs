//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! A recording implementation of direct3d-11's traits, for tests only: every call is logged by its place, with the values
//! passed by value; an object asked for is made; a structure, a bound object or a list asked for is not (E_NOTIMPL).

#![allow(
    unused_variables,
    unused_imports,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::new_without_default,
    reason = "generated: every method has the same shape"
)]

#[path = "../direct/data.rs"]
mod data_direct3d_11;
#[path = "../../../directx-graphics-infrastructure/implementation/direct/data.rs"]
mod data_directx_graphics_infrastructure;

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_11 as direct3d_11;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_direct3d_common_recorder as direct3d_common_recorder;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;
use wcontainers_directx_graphics_infrastructure_recorder as directx_graphics_infrastructure_recorder;

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

/// The bytes every Map lends, and the pitches it answers.
const MAPPED_LENGTH: usize = 65536;
const MAPPED_ROW_PITCH: u32 = 4096;
const MAPPED_DEPTH_PITCH: u32 = 65536;

/// Bytes the recorder lends for a Map: shared by every clone, zero at first.
#[derive(Clone, Debug)]
pub struct RecordedBytes(Arc<Mutex<Vec<u8>>>);

impl RecordedBytes {
    /// New zero bytes of the length every Map lends.
    pub fn new() -> RecordedBytes {
        RecordedBytes(Arc::new(Mutex::new(vec![0; MAPPED_LENGTH])))
    }
}

impl Default for RecordedBytes {
    fn default() -> RecordedBytes {
        RecordedBytes::new()
    }
}

impl wcontainers_volatile_bytes::VolatileBytes for RecordedBytes {
    fn len(&self) -> u64 {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        let copy = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        let mut copy = vec![u8::MIN; self.0.lock().unwrap_or_else(PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

/// A recorded object of the kind `AuthenticatedChannel`.
pub struct RecordedAuthenticatedChannel<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedAuthenticatedChannel<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedAuthenticatedChannel<A> {
        RecordedAuthenticatedChannel {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedAuthenticatedChannel<A> {
    fn clone(&self) -> RecordedAuthenticatedChannel<A> {
        RecordedAuthenticatedChannel {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedAuthenticatedChannel<A> {
    fn eq(&self, other: &RecordedAuthenticatedChannel<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedAuthenticatedChannel<A> {}

impl<A> Hash for RecordedAuthenticatedChannel<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedAuthenticatedChannel<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedAuthenticatedChannel")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `BlendState`.
pub struct RecordedBlendState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedBlendState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedBlendState<A> {
        RecordedBlendState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedBlendState<A> {
    fn clone(&self) -> RecordedBlendState<A> {
        RecordedBlendState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedBlendState<A> {
    fn eq(&self, other: &RecordedBlendState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedBlendState<A> {}

impl<A> Hash for RecordedBlendState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedBlendState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedBlendState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Buffer`.
pub struct RecordedBuffer<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedBuffer<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedBuffer<A> {
        RecordedBuffer {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedBuffer<A> {
    fn clone(&self) -> RecordedBuffer<A> {
        RecordedBuffer {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedBuffer<A> {
    fn eq(&self, other: &RecordedBuffer<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedBuffer<A> {}

impl<A> Hash for RecordedBuffer<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedBuffer<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedBuffer")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ClassInstance`.
pub struct RecordedClassInstance<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedClassInstance<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedClassInstance<A> {
        RecordedClassInstance {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedClassInstance<A> {
    fn clone(&self) -> RecordedClassInstance<A> {
        RecordedClassInstance {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedClassInstance<A> {
    fn eq(&self, other: &RecordedClassInstance<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedClassInstance<A> {}

impl<A> Hash for RecordedClassInstance<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedClassInstance<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedClassInstance")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ClassLinkage`.
pub struct RecordedClassLinkage<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedClassLinkage<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedClassLinkage<A> {
        RecordedClassLinkage {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedClassLinkage<A> {
    fn clone(&self) -> RecordedClassLinkage<A> {
        RecordedClassLinkage {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedClassLinkage<A> {
    fn eq(&self, other: &RecordedClassLinkage<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedClassLinkage<A> {}

impl<A> Hash for RecordedClassLinkage<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedClassLinkage<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedClassLinkage")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `CommandList`.
pub struct RecordedCommandList<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedCommandList<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedCommandList<A> {
        RecordedCommandList {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedCommandList<A> {
    fn clone(&self) -> RecordedCommandList<A> {
        RecordedCommandList {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedCommandList<A> {
    fn eq(&self, other: &RecordedCommandList<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedCommandList<A> {}

impl<A> Hash for RecordedCommandList<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedCommandList<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedCommandList")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ComputeShader`.
pub struct RecordedComputeShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedComputeShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedComputeShader<A> {
        RecordedComputeShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedComputeShader<A> {
    fn clone(&self) -> RecordedComputeShader<A> {
        RecordedComputeShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedComputeShader<A> {
    fn eq(&self, other: &RecordedComputeShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedComputeShader<A> {}

impl<A> Hash for RecordedComputeShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedComputeShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedComputeShader")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Counter`.
pub struct RecordedCounter<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedCounter<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedCounter<A> {
        RecordedCounter {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedCounter<A> {
    fn clone(&self) -> RecordedCounter<A> {
        RecordedCounter {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedCounter<A> {
    fn eq(&self, other: &RecordedCounter<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedCounter<A> {}

impl<A> Hash for RecordedCounter<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedCounter<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedCounter")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `CryptoSession`.
pub struct RecordedCryptoSession<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedCryptoSession<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedCryptoSession<A> {
        RecordedCryptoSession {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedCryptoSession<A> {
    fn clone(&self) -> RecordedCryptoSession<A> {
        RecordedCryptoSession {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedCryptoSession<A> {
    fn eq(&self, other: &RecordedCryptoSession<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedCryptoSession<A> {}

impl<A> Hash for RecordedCryptoSession<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedCryptoSession<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedCryptoSession")
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

/// A recorded object of the kind `DepthStencilState`.
pub struct RecordedDepthStencilState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDepthStencilState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDepthStencilState<A> {
        RecordedDepthStencilState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDepthStencilState<A> {
    fn clone(&self) -> RecordedDepthStencilState<A> {
        RecordedDepthStencilState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDepthStencilState<A> {
    fn eq(&self, other: &RecordedDepthStencilState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDepthStencilState<A> {}

impl<A> Hash for RecordedDepthStencilState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDepthStencilState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDepthStencilState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DepthStencilView`.
pub struct RecordedDepthStencilView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDepthStencilView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDepthStencilView<A> {
        RecordedDepthStencilView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDepthStencilView<A> {
    fn clone(&self) -> RecordedDepthStencilView<A> {
        RecordedDepthStencilView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDepthStencilView<A> {
    fn eq(&self, other: &RecordedDepthStencilView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDepthStencilView<A> {}

impl<A> Hash for RecordedDepthStencilView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDepthStencilView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDepthStencilView")
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

/// A recorded object of the kind `DeviceContext`.
pub struct RecordedDeviceContext<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDeviceContext<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDeviceContext<A> {
        RecordedDeviceContext {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDeviceContext<A> {
    fn clone(&self) -> RecordedDeviceContext<A> {
        RecordedDeviceContext {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDeviceContext<A> {
    fn eq(&self, other: &RecordedDeviceContext<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDeviceContext<A> {}

impl<A> Hash for RecordedDeviceContext<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDeviceContext<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDeviceContext")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DomainShader`.
pub struct RecordedDomainShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDomainShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDomainShader<A> {
        RecordedDomainShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDomainShader<A> {
    fn clone(&self) -> RecordedDomainShader<A> {
        RecordedDomainShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDomainShader<A> {
    fn eq(&self, other: &RecordedDomainShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDomainShader<A> {}

impl<A> Hash for RecordedDomainShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDomainShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDomainShader")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Fence`.
pub struct RecordedFence<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedFence<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedFence<A> {
        RecordedFence {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedFence<A> {
    fn clone(&self) -> RecordedFence<A> {
        RecordedFence {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedFence<A> {
    fn eq(&self, other: &RecordedFence<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedFence<A> {}

impl<A> Hash for RecordedFence<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedFence<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedFence")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `GeometryShader`.
pub struct RecordedGeometryShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedGeometryShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedGeometryShader<A> {
        RecordedGeometryShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedGeometryShader<A> {
    fn clone(&self) -> RecordedGeometryShader<A> {
        RecordedGeometryShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedGeometryShader<A> {
    fn eq(&self, other: &RecordedGeometryShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedGeometryShader<A> {}

impl<A> Hash for RecordedGeometryShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedGeometryShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedGeometryShader")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `HullShader`.
pub struct RecordedHullShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedHullShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedHullShader<A> {
        RecordedHullShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedHullShader<A> {
    fn clone(&self) -> RecordedHullShader<A> {
        RecordedHullShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedHullShader<A> {
    fn eq(&self, other: &RecordedHullShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedHullShader<A> {}

impl<A> Hash for RecordedHullShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedHullShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedHullShader")
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

/// A recorded object of the kind `InputLayout`.
pub struct RecordedInputLayout<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedInputLayout<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedInputLayout<A> {
        RecordedInputLayout {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedInputLayout<A> {
    fn clone(&self) -> RecordedInputLayout<A> {
        RecordedInputLayout {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedInputLayout<A> {
    fn eq(&self, other: &RecordedInputLayout<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedInputLayout<A> {}

impl<A> Hash for RecordedInputLayout<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedInputLayout<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedInputLayout")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Multithread`.
pub struct RecordedMultithread<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedMultithread<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedMultithread<A> {
        RecordedMultithread {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedMultithread<A> {
    fn clone(&self) -> RecordedMultithread<A> {
        RecordedMultithread {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedMultithread<A> {
    fn eq(&self, other: &RecordedMultithread<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedMultithread<A> {}

impl<A> Hash for RecordedMultithread<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedMultithread<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedMultithread")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `PixelShader`.
pub struct RecordedPixelShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedPixelShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedPixelShader<A> {
        RecordedPixelShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedPixelShader<A> {
    fn clone(&self) -> RecordedPixelShader<A> {
        RecordedPixelShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedPixelShader<A> {
    fn eq(&self, other: &RecordedPixelShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedPixelShader<A> {}

impl<A> Hash for RecordedPixelShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedPixelShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedPixelShader")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Predicate`.
pub struct RecordedPredicate<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedPredicate<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedPredicate<A> {
        RecordedPredicate {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedPredicate<A> {
    fn clone(&self) -> RecordedPredicate<A> {
        RecordedPredicate {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedPredicate<A> {
    fn eq(&self, other: &RecordedPredicate<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedPredicate<A> {}

impl<A> Hash for RecordedPredicate<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedPredicate<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedPredicate")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Query`.
pub struct RecordedQuery<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedQuery<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedQuery<A> {
        RecordedQuery {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedQuery<A> {
    fn clone(&self) -> RecordedQuery<A> {
        RecordedQuery {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedQuery<A> {
    fn eq(&self, other: &RecordedQuery<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedQuery<A> {}

impl<A> Hash for RecordedQuery<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedQuery<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedQuery")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `RasterizerState`.
pub struct RecordedRasterizerState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedRasterizerState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedRasterizerState<A> {
        RecordedRasterizerState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedRasterizerState<A> {
    fn clone(&self) -> RecordedRasterizerState<A> {
        RecordedRasterizerState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedRasterizerState<A> {
    fn eq(&self, other: &RecordedRasterizerState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedRasterizerState<A> {}

impl<A> Hash for RecordedRasterizerState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedRasterizerState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedRasterizerState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `RefDefaultTrackingOptions`.
pub struct RecordedRefDefaultTrackingOptions<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedRefDefaultTrackingOptions<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedRefDefaultTrackingOptions<A> {
        RecordedRefDefaultTrackingOptions {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedRefDefaultTrackingOptions<A> {
    fn clone(&self) -> RecordedRefDefaultTrackingOptions<A> {
        RecordedRefDefaultTrackingOptions {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedRefDefaultTrackingOptions<A> {
    fn eq(&self, other: &RecordedRefDefaultTrackingOptions<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedRefDefaultTrackingOptions<A> {}

impl<A> Hash for RecordedRefDefaultTrackingOptions<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedRefDefaultTrackingOptions<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedRefDefaultTrackingOptions")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `RefTrackingOptions`.
pub struct RecordedRefTrackingOptions<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedRefTrackingOptions<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedRefTrackingOptions<A> {
        RecordedRefTrackingOptions {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedRefTrackingOptions<A> {
    fn clone(&self) -> RecordedRefTrackingOptions<A> {
        RecordedRefTrackingOptions {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedRefTrackingOptions<A> {
    fn eq(&self, other: &RecordedRefTrackingOptions<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedRefTrackingOptions<A> {}

impl<A> Hash for RecordedRefTrackingOptions<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedRefTrackingOptions<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedRefTrackingOptions")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `RenderTargetView`.
pub struct RecordedRenderTargetView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedRenderTargetView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedRenderTargetView<A> {
        RecordedRenderTargetView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedRenderTargetView<A> {
    fn clone(&self) -> RecordedRenderTargetView<A> {
        RecordedRenderTargetView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedRenderTargetView<A> {
    fn eq(&self, other: &RecordedRenderTargetView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedRenderTargetView<A> {}

impl<A> Hash for RecordedRenderTargetView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedRenderTargetView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedRenderTargetView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SamplerState`.
pub struct RecordedSamplerState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedSamplerState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedSamplerState<A> {
        RecordedSamplerState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedSamplerState<A> {
    fn clone(&self) -> RecordedSamplerState<A> {
        RecordedSamplerState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedSamplerState<A> {
    fn eq(&self, other: &RecordedSamplerState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedSamplerState<A> {}

impl<A> Hash for RecordedSamplerState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedSamplerState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedSamplerState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ShaderResourceView`.
pub struct RecordedShaderResourceView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedShaderResourceView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedShaderResourceView<A> {
        RecordedShaderResourceView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedShaderResourceView<A> {
    fn clone(&self) -> RecordedShaderResourceView<A> {
        RecordedShaderResourceView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedShaderResourceView<A> {
    fn eq(&self, other: &RecordedShaderResourceView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedShaderResourceView<A> {}

impl<A> Hash for RecordedShaderResourceView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedShaderResourceView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedShaderResourceView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ShaderTrace`.
pub struct RecordedShaderTrace<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedShaderTrace<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedShaderTrace<A> {
        RecordedShaderTrace {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedShaderTrace<A> {
    fn clone(&self) -> RecordedShaderTrace<A> {
        RecordedShaderTrace {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedShaderTrace<A> {
    fn eq(&self, other: &RecordedShaderTrace<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedShaderTrace<A> {}

impl<A> Hash for RecordedShaderTrace<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedShaderTrace<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedShaderTrace")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `ShaderTraceFactory`.
pub struct RecordedShaderTraceFactory<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedShaderTraceFactory<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedShaderTraceFactory<A> {
        RecordedShaderTraceFactory {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedShaderTraceFactory<A> {
    fn clone(&self) -> RecordedShaderTraceFactory<A> {
        RecordedShaderTraceFactory {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedShaderTraceFactory<A> {
    fn eq(&self, other: &RecordedShaderTraceFactory<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedShaderTraceFactory<A> {}

impl<A> Hash for RecordedShaderTraceFactory<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedShaderTraceFactory<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedShaderTraceFactory")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SwitchToRef`.
pub struct RecordedSwitchToRef<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedSwitchToRef<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedSwitchToRef<A> {
        RecordedSwitchToRef {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedSwitchToRef<A> {
    fn clone(&self) -> RecordedSwitchToRef<A> {
        RecordedSwitchToRef {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedSwitchToRef<A> {
    fn eq(&self, other: &RecordedSwitchToRef<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedSwitchToRef<A> {}

impl<A> Hash for RecordedSwitchToRef<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedSwitchToRef<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedSwitchToRef")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Texture1D`.
pub struct RecordedTexture1D<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedTexture1D<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedTexture1D<A> {
        RecordedTexture1D {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedTexture1D<A> {
    fn clone(&self) -> RecordedTexture1D<A> {
        RecordedTexture1D {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedTexture1D<A> {
    fn eq(&self, other: &RecordedTexture1D<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedTexture1D<A> {}

impl<A> Hash for RecordedTexture1D<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedTexture1D<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedTexture1D")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Texture2D`.
pub struct RecordedTexture2D<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedTexture2D<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedTexture2D<A> {
        RecordedTexture2D {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedTexture2D<A> {
    fn clone(&self) -> RecordedTexture2D<A> {
        RecordedTexture2D {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedTexture2D<A> {
    fn eq(&self, other: &RecordedTexture2D<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedTexture2D<A> {}

impl<A> Hash for RecordedTexture2D<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedTexture2D<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedTexture2D")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Texture3D`.
pub struct RecordedTexture3D<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedTexture3D<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedTexture3D<A> {
        RecordedTexture3D {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedTexture3D<A> {
    fn clone(&self) -> RecordedTexture3D<A> {
        RecordedTexture3D {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedTexture3D<A> {
    fn eq(&self, other: &RecordedTexture3D<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedTexture3D<A> {}

impl<A> Hash for RecordedTexture3D<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedTexture3D<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedTexture3D")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `TracingDevice`.
pub struct RecordedTracingDevice<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedTracingDevice<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedTracingDevice<A> {
        RecordedTracingDevice {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedTracingDevice<A> {
    fn clone(&self) -> RecordedTracingDevice<A> {
        RecordedTracingDevice {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedTracingDevice<A> {
    fn eq(&self, other: &RecordedTracingDevice<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedTracingDevice<A> {}

impl<A> Hash for RecordedTracingDevice<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedTracingDevice<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedTracingDevice")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `UnorderedAccessView`.
pub struct RecordedUnorderedAccessView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedUnorderedAccessView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedUnorderedAccessView<A> {
        RecordedUnorderedAccessView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedUnorderedAccessView<A> {
    fn clone(&self) -> RecordedUnorderedAccessView<A> {
        RecordedUnorderedAccessView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedUnorderedAccessView<A> {
    fn eq(&self, other: &RecordedUnorderedAccessView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedUnorderedAccessView<A> {}

impl<A> Hash for RecordedUnorderedAccessView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedUnorderedAccessView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedUnorderedAccessView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VertexShader`.
pub struct RecordedVertexShader<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVertexShader<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVertexShader<A> {
        RecordedVertexShader {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVertexShader<A> {
    fn clone(&self) -> RecordedVertexShader<A> {
        RecordedVertexShader {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVertexShader<A> {
    fn eq(&self, other: &RecordedVertexShader<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVertexShader<A> {}

impl<A> Hash for RecordedVertexShader<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVertexShader<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVertexShader")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoContext`.
pub struct RecordedVideoContext<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoContext<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoContext<A> {
        RecordedVideoContext {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoContext<A> {
    fn clone(&self) -> RecordedVideoContext<A> {
        RecordedVideoContext {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoContext<A> {
    fn eq(&self, other: &RecordedVideoContext<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoContext<A> {}

impl<A> Hash for RecordedVideoContext<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoContext<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoContext")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoDecoder`.
pub struct RecordedVideoDecoder<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoDecoder<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoDecoder<A> {
        RecordedVideoDecoder {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoDecoder<A> {
    fn clone(&self) -> RecordedVideoDecoder<A> {
        RecordedVideoDecoder {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoDecoder<A> {
    fn eq(&self, other: &RecordedVideoDecoder<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoDecoder<A> {}

impl<A> Hash for RecordedVideoDecoder<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoDecoder<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoDecoder")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoDecoderOutputView`.
pub struct RecordedVideoDecoderOutputView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoDecoderOutputView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoDecoderOutputView<A> {
        RecordedVideoDecoderOutputView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoDecoderOutputView<A> {
    fn clone(&self) -> RecordedVideoDecoderOutputView<A> {
        RecordedVideoDecoderOutputView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoDecoderOutputView<A> {
    fn eq(&self, other: &RecordedVideoDecoderOutputView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoDecoderOutputView<A> {}

impl<A> Hash for RecordedVideoDecoderOutputView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoDecoderOutputView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoDecoderOutputView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoDevice`.
pub struct RecordedVideoDevice<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoDevice<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoDevice<A> {
        RecordedVideoDevice {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoDevice<A> {
    fn clone(&self) -> RecordedVideoDevice<A> {
        RecordedVideoDevice {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoDevice<A> {
    fn eq(&self, other: &RecordedVideoDevice<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoDevice<A> {}

impl<A> Hash for RecordedVideoDevice<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoDevice<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoDevice")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoProcessor`.
pub struct RecordedVideoProcessor<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoProcessor<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoProcessor<A> {
        RecordedVideoProcessor {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoProcessor<A> {
    fn clone(&self) -> RecordedVideoProcessor<A> {
        RecordedVideoProcessor {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoProcessor<A> {
    fn eq(&self, other: &RecordedVideoProcessor<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoProcessor<A> {}

impl<A> Hash for RecordedVideoProcessor<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoProcessor<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoProcessor")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoProcessorEnumerator`.
pub struct RecordedVideoProcessorEnumerator<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoProcessorEnumerator<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoProcessorEnumerator<A> {
        RecordedVideoProcessorEnumerator {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoProcessorEnumerator<A> {
    fn clone(&self) -> RecordedVideoProcessorEnumerator<A> {
        RecordedVideoProcessorEnumerator {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoProcessorEnumerator<A> {
    fn eq(&self, other: &RecordedVideoProcessorEnumerator<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoProcessorEnumerator<A> {}

impl<A> Hash for RecordedVideoProcessorEnumerator<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoProcessorEnumerator<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoProcessorEnumerator")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoProcessorInputView`.
pub struct RecordedVideoProcessorInputView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoProcessorInputView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoProcessorInputView<A> {
        RecordedVideoProcessorInputView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoProcessorInputView<A> {
    fn clone(&self) -> RecordedVideoProcessorInputView<A> {
        RecordedVideoProcessorInputView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoProcessorInputView<A> {
    fn eq(&self, other: &RecordedVideoProcessorInputView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoProcessorInputView<A> {}

impl<A> Hash for RecordedVideoProcessorInputView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoProcessorInputView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoProcessorInputView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `VideoProcessorOutputView`.
pub struct RecordedVideoProcessorOutputView<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedVideoProcessorOutputView<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedVideoProcessorOutputView<A> {
        RecordedVideoProcessorOutputView {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedVideoProcessorOutputView<A> {
    fn clone(&self) -> RecordedVideoProcessorOutputView<A> {
        RecordedVideoProcessorOutputView {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedVideoProcessorOutputView<A> {
    fn eq(&self, other: &RecordedVideoProcessorOutputView<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedVideoProcessorOutputView<A> {}

impl<A> Hash for RecordedVideoProcessorOutputView<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedVideoProcessorOutputView<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedVideoProcessorOutputView")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DeviceContextState`.
pub struct RecordedDeviceContextState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDeviceContextState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDeviceContextState<A> {
        RecordedDeviceContextState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDeviceContextState<A> {
    fn clone(&self) -> RecordedDeviceContextState<A> {
        RecordedDeviceContextState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDeviceContextState<A> {
    fn eq(&self, other: &RecordedDeviceContextState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDeviceContextState<A> {}

impl<A> Hash for RecordedDeviceContextState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDeviceContextState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDeviceContextState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `UserDefinedAnnotation`.
pub struct RecordedUserDefinedAnnotation<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedUserDefinedAnnotation<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedUserDefinedAnnotation<A> {
        RecordedUserDefinedAnnotation {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedUserDefinedAnnotation<A> {
    fn clone(&self) -> RecordedUserDefinedAnnotation<A> {
        RecordedUserDefinedAnnotation {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedUserDefinedAnnotation<A> {
    fn eq(&self, other: &RecordedUserDefinedAnnotation<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedUserDefinedAnnotation<A> {}

impl<A> Hash for RecordedUserDefinedAnnotation<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedUserDefinedAnnotation<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedUserDefinedAnnotation")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this recorder's.
pub trait Recorded:
    direct3d_11::Direct3D11<
        AuthenticatedChannel = RecordedAuthenticatedChannel<Self>,
        BlendState = RecordedBlendState<Self>,
        Buffer = RecordedBuffer<Self>,
        ClassInstance = RecordedClassInstance<Self>,
        ClassLinkage = RecordedClassLinkage<Self>,
        CommandList = RecordedCommandList<Self>,
        ComputeShader = RecordedComputeShader<Self>,
        Counter = RecordedCounter<Self>,
        CryptoSession = RecordedCryptoSession<Self>,
        Debug = RecordedDebug<Self>,
        DepthStencilState = RecordedDepthStencilState<Self>,
        DepthStencilView = RecordedDepthStencilView<Self>,
        Device = RecordedDevice<Self>,
        DeviceContext = RecordedDeviceContext<Self>,
        DomainShader = RecordedDomainShader<Self>,
        Fence = RecordedFence<Self>,
        GeometryShader = RecordedGeometryShader<Self>,
        HullShader = RecordedHullShader<Self>,
        InfoQueue = RecordedInfoQueue<Self>,
        InputLayout = RecordedInputLayout<Self>,
        Multithread = RecordedMultithread<Self>,
        PixelShader = RecordedPixelShader<Self>,
        Predicate = RecordedPredicate<Self>,
        Query = RecordedQuery<Self>,
        RasterizerState = RecordedRasterizerState<Self>,
        RefDefaultTrackingOptions = RecordedRefDefaultTrackingOptions<Self>,
        RefTrackingOptions = RecordedRefTrackingOptions<Self>,
        RenderTargetView = RecordedRenderTargetView<Self>,
        SamplerState = RecordedSamplerState<Self>,
        ShaderResourceView = RecordedShaderResourceView<Self>,
        ShaderTrace = RecordedShaderTrace<Self>,
        ShaderTraceFactory = RecordedShaderTraceFactory<Self>,
        SwitchToRef = RecordedSwitchToRef<Self>,
        Texture1D = RecordedTexture1D<Self>,
        Texture2D = RecordedTexture2D<Self>,
        Texture3D = RecordedTexture3D<Self>,
        TracingDevice = RecordedTracingDevice<Self>,
        UnorderedAccessView = RecordedUnorderedAccessView<Self>,
        VertexShader = RecordedVertexShader<Self>,
        VideoContext = RecordedVideoContext<Self>,
        VideoDecoder = RecordedVideoDecoder<Self>,
        VideoDecoderOutputView = RecordedVideoDecoderOutputView<Self>,
        VideoDevice = RecordedVideoDevice<Self>,
        VideoProcessor = RecordedVideoProcessor<Self>,
        VideoProcessorEnumerator = RecordedVideoProcessorEnumerator<Self>,
        VideoProcessorInputView = RecordedVideoProcessorInputView<Self>,
        VideoProcessorOutputView = RecordedVideoProcessorOutputView<Self>,
        DeviceContextState = RecordedDeviceContextState<Self>,
        UserDefinedAnnotation = RecordedUserDefinedAnnotation<Self>,
        Mapped = RecordedBytes,
    > + direct3d_common_recorder::Recorded
    + directx_graphics_infrastructure_recorder::Recorded
{
}

impl<A: Recorded> direct3d_11::AuthenticatedChannel for RecordedAuthenticatedChannel<A> {
    fn get_certificate_size(&self) -> Result<u32, component_object_model::structures::HResult> {
        record("ID3D11AuthenticatedChannel::GetCertificateSize()".to_string());
        Ok(Default::default())
    }
    fn get_certificate(
        &self,
        certificate: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &certificate;
        record("ID3D11AuthenticatedChannel::GetCertificate()".to_string());
        Ok(())
    }
    fn get_channel_handle(&self) -> component_object_model::structures::Handle {
        record("ID3D11AuthenticatedChannel::GetChannelHandle()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedAuthenticatedChannel<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedAuthenticatedChannel<A> {
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

impl<A: Recorded> direct3d_11::BlendState1 for RecordedBlendState<A> {
    fn get_desc1(&self) -> direct3d_11::structures::BlendDesc1 {
        record("ID3D11BlendState1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc1(
            &[u8::MIN; 328],
        )
    }
}

impl<A: Recorded> direct3d_11::BlendState for RecordedBlendState<A> {
    fn get_desc(&self) -> direct3d_11::structures::BlendDesc {
        record("ID3D11BlendState::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_blend_desc(
            &[u8::MIN; 264],
        )
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedBlendState<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedBlendState<A> {
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

impl<A: Recorded> direct3d_11::Buffer for RecordedBuffer<A> {
    fn get_desc(&self) -> direct3d_11::structures::BufferDesc {
        record("ID3D11Buffer::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_buffer_desc(
            &[u8::MIN; 24],
        )
    }
}

impl<A: Recorded> direct3d_11::Resource for RecordedBuffer<A> {
    fn get_type(&self) -> direct3d_11::enumerations::ResourceDimension {
        record("ID3D11Resource::GetType()".to_string());
        Default::default()
    }
    fn set_eviction_priority(&self, eviction_priority: u32) {
        let _ = &eviction_priority;
        record(format!(
            "ID3D11Resource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
    }
    fn get_eviction_priority(&self) -> u32 {
        record("ID3D11Resource::GetEvictionPriority()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedBuffer<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedBuffer<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedBuffer<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource for RecordedBuffer<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        Ok(Default::default())
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource1 for RecordedBuffer<A> {
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
        Ok(directx_graphics_infrastructure_recorder::RecordedSurface::new())
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
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedBuffer<A> {
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

impl<A: Recorded> direct3d_11::ClassInstance for RecordedClassInstance<A> {
    fn get_class_linkage(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::ClassLinkage> {
        record("ID3D11ClassInstance::GetClassLinkage()".to_string());
        Some(RecordedClassLinkage::new())
    }
    fn get_desc(&self) -> direct3d_11::structures::ClassInstanceDesc {
        record("ID3D11ClassInstance::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_class_instance_desc(
            &[u8::MIN; 32],
        )
    }
    fn get_instance_name(&self, instance_name: Option<&std::ffi::CStr>) {
        let _ = &instance_name;
        record("ID3D11ClassInstance::GetInstanceName()".to_string());
    }
    fn get_type_name(&self, type_name: Option<&std::ffi::CStr>) {
        let _ = &type_name;
        record("ID3D11ClassInstance::GetTypeName()".to_string());
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedClassInstance<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedClassInstance<A> {
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

impl<A: Recorded> direct3d_11::ClassLinkage for RecordedClassLinkage<A> {
    fn get_class_instance(
        &self,
        class_instance_name: &std::ffi::CStr,
        instance_index: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ClassInstance,
        component_object_model::structures::HResult,
    > {
        let _ = &class_instance_name;
        let _ = &instance_index;
        record(format!(
            "ID3D11ClassLinkage::GetClassInstance(instance_index={:?})",
            instance_index
        ));
        Ok(RecordedClassInstance::new())
    }
    fn create_class_instance(
        &self,
        class_type_name: &std::ffi::CStr,
        constant_buffer_offset: u32,
        constant_vector_offset: u32,
        texture_offset: u32,
        sampler_offset: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ClassInstance,
        component_object_model::structures::HResult,
    > {
        let _ = &class_type_name;
        let _ = &constant_buffer_offset;
        let _ = &constant_vector_offset;
        let _ = &texture_offset;
        let _ = &sampler_offset;
        record(format!(
            "ID3D11ClassLinkage::CreateClassInstance(constant_buffer_offset={:?}, constant_vector_offset={:?}, texture_offset={:?}, sampler_offset={:?})",
            constant_buffer_offset, constant_vector_offset, texture_offset, sampler_offset
        ));
        Ok(RecordedClassInstance::new())
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedClassLinkage<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedClassLinkage<A> {
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

impl<A: Recorded> direct3d_11::CommandList for RecordedCommandList<A> {
    fn get_context_flags(&self) -> u32 {
        record("ID3D11CommandList::GetContextFlags()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedCommandList<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedCommandList<A> {
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

impl<A: Recorded> direct3d_11::ComputeShader for RecordedComputeShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedComputeShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedComputeShader<A> {
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

impl<A: Recorded> direct3d_11::Counter for RecordedCounter<A> {
    fn get_desc(&self) -> direct3d_11::structures::CounterDesc {
        record("ID3D11Counter::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_counter_desc(
            &[u8::MIN; 8],
        )
    }
}

impl<A: Recorded> direct3d_11::Asynchronous for RecordedCounter<A> {
    fn get_data_size(&self) -> u32 {
        record("ID3D11Asynchronous::GetDataSize()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedCounter<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedCounter<A> {
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

impl<A: Recorded> direct3d_11::CryptoSession for RecordedCryptoSession<A> {
    fn get_crypto_type(&self) -> component_object_model::structures::Guid {
        record("ID3D11CryptoSession::GetCryptoType()".to_string());
        Default::default()
    }
    fn get_decoder_profile(&self) -> component_object_model::structures::Guid {
        record("ID3D11CryptoSession::GetDecoderProfile()".to_string());
        Default::default()
    }
    fn get_certificate_size(&self) -> Result<u32, component_object_model::structures::HResult> {
        record("ID3D11CryptoSession::GetCertificateSize()".to_string());
        Ok(Default::default())
    }
    fn get_certificate(
        &self,
        certificate: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &certificate;
        record("ID3D11CryptoSession::GetCertificate()".to_string());
        Ok(())
    }
    fn get_crypto_session_handle(&self) -> component_object_model::structures::Handle {
        record("ID3D11CryptoSession::GetCryptoSessionHandle()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedCryptoSession<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedCryptoSession<A> {
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

impl<A: Recorded> direct3d_11::Debug for RecordedDebug<A> {
    type Api = A;

    fn set_feature_mask(
        &self,
        mask: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &mask;
        record(format!("ID3D11Debug::SetFeatureMask(mask={:?})", mask));
        Ok(())
    }
    fn get_feature_mask(&self) -> u32 {
        record("ID3D11Debug::GetFeatureMask()".to_string());
        Default::default()
    }
    fn set_present_per_render_op_delay(
        &self,
        milliseconds: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &milliseconds;
        record(format!(
            "ID3D11Debug::SetPresentPerRenderOpDelay(milliseconds={:?})",
            milliseconds
        ));
        Ok(())
    }
    fn get_present_per_render_op_delay(&self) -> u32 {
        record("ID3D11Debug::GetPresentPerRenderOpDelay()".to_string());
        Default::default()
    }
    fn set_swap_chain(
        &self,
        swap_chain: Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &swap_chain;
        record("ID3D11Debug::SetSwapChain()".to_string());
        Ok(())
    }
    fn get_swap_chain(&self) -> Result<Option<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>, component_object_model::structures::HResult>{
        record("ID3D11Debug::GetSwapChain()".to_string());
        Ok(Some(
            directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
        ))
    }
    fn validate_context(
        &self,
        context: &<Self::Api as direct3d_11::Direct3D11>::DeviceContext,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &context;
        record("ID3D11Debug::ValidateContext()".to_string());
        Ok(())
    }
    fn report_live_device_objects(
        &self,
        flags: direct3d_11::enumerations::RldoFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &flags;
        record(format!(
            "ID3D11Debug::ReportLiveDeviceObjects(flags={:?})",
            flags
        ));
        Ok(())
    }
    fn validate_context_for_dispatch(
        &self,
        context: &<Self::Api as direct3d_11::Direct3D11>::DeviceContext,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &context;
        record("ID3D11Debug::ValidateContextForDispatch()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDebug<A> {
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

impl<A: Recorded> direct3d_11::DepthStencilState for RecordedDepthStencilState<A> {
    fn get_desc(&self) -> direct3d_11::structures::DepthStencilDesc {
        record("ID3D11DepthStencilState::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_desc(
            &[u8::MIN; 52],
        )
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedDepthStencilState<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDepthStencilState<A> {
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

impl<A: Recorded> direct3d_11::DepthStencilView for RecordedDepthStencilView<A> {
    fn get_desc(&self) -> direct3d_11::structures::DepthStencilViewDesc {
        record("ID3D11DepthStencilView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_depth_stencil_view_desc(
            &[u8::MIN; 24],
        )
    }
}

impl<A: Recorded> direct3d_11::View for RecordedDepthStencilView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedDepthStencilView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDepthStencilView<A> {
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

impl<A: Recorded> direct3d_11::Device5 for RecordedDevice<A> {
    fn open_shared_fence(
        &self,
        h_fence: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<direct3d_11::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &h_fence;
        let _ = &returned_interface;
        record(format!(
            "ID3D11Device5::OpenSharedFence(h_fence={:?})",
            h_fence
        ));
        Ok(
            if [
                direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::AuthenticatedChannel(RecordedAuthenticatedChannel::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BLEND_STATE,
                direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::BlendState(RecordedBlendState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BUFFER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Buffer(RecordedBuffer::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassInstance(RecordedClassInstance::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassLinkage(RecordedClassLinkage::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CommandList(RecordedCommandList::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ComputeShader(RecordedComputeShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_COUNTER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Counter(RecordedCounter::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CryptoSession(RecordedCryptoSession::new())
            } else if [direct3d_11::constants::IID_ID3D11_DEBUG].contains(returned_interface) {
                direct3d_11::UnknownObject::Debug(RecordedDebug::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilState(RecordedDepthStencilState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilView(RecordedDepthStencilView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE,
                direct3d_11::constants::IID_ID3D11_DEVICE1,
                direct3d_11::constants::IID_ID3D11_DEVICE2,
                direct3d_11::constants::IID_ID3D11_DEVICE3,
                direct3d_11::constants::IID_ID3D11_DEVICE4,
                direct3d_11::constants::IID_ID3D11_DEVICE5,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT3,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContext(RecordedDeviceContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DomainShader(RecordedDomainShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_FENCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::GeometryShader(RecordedGeometryShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_HULL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::HullShader(RecordedHullShader::new())
            } else if [direct3d_11::constants::IID_ID3D11_INFO_QUEUE].contains(returned_interface) {
                direct3d_11::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::InputLayout(RecordedInputLayout::new())
            } else if [direct3d_11::constants::IID_ID3D11_MULTITHREAD].contains(returned_interface)
            {
                direct3d_11::UnknownObject::Multithread(RecordedMultithread::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::PixelShader(RecordedPixelShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PREDICATE,
                direct3d_11::constants::IID_ID3D11_QUERY,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Predicate(RecordedPredicate::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_QUERY,
                direct3d_11::constants::IID_ID3D11_QUERY1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Query(RecordedQuery::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE1,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RasterizerState(RecordedRasterizerState::new())
            } else if [direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefDefaultTrackingOptions(
                    RecordedRefDefaultTrackingOptions::new(),
                )
            } else if [direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefTrackingOptions(RecordedRefTrackingOptions::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RenderTargetView(RecordedRenderTargetView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SamplerState(RecordedSamplerState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderResourceView(RecordedShaderResourceView::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE].contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTrace(RecordedShaderTrace::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTraceFactory(RecordedShaderTraceFactory::new())
            } else if [direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SwitchToRef(RecordedSwitchToRef::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture1D(RecordedTexture1D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture2D(RecordedTexture2D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture3D(RecordedTexture3D::new())
            } else if [direct3d_11::constants::IID_ID3D11_TRACING_DEVICE]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::TracingDevice(RecordedTracingDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UnorderedAccessView(RecordedUnorderedAccessView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VertexShader(RecordedVertexShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoContext(RecordedVideoContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoder(RecordedVideoDecoder::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoderOutputView(
                    RecordedVideoDecoderOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE1,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDevice(RecordedVideoDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessor(RecordedVideoProcessor::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorEnumerator(
                    RecordedVideoProcessorEnumerator::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorInputView(
                    RecordedVideoProcessorInputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorOutputView(
                    RecordedVideoProcessorOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContextState(RecordedDeviceContextState::new())
            } else if [direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UserDefinedAnnotation(
                    RecordedUserDefinedAnnotation::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_fence(
        &self,
        initial_value: u64,
        flags: direct3d_11::enumerations::FenceFlag,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<direct3d_11::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &initial_value;
        let _ = &flags;
        let _ = &returned_interface;
        record(format!(
            "ID3D11Device5::CreateFence(initial_value={:?}, flags={:?})",
            initial_value, flags
        ));
        Ok(
            if [
                direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::AuthenticatedChannel(RecordedAuthenticatedChannel::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BLEND_STATE,
                direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::BlendState(RecordedBlendState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BUFFER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Buffer(RecordedBuffer::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassInstance(RecordedClassInstance::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassLinkage(RecordedClassLinkage::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CommandList(RecordedCommandList::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ComputeShader(RecordedComputeShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_COUNTER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Counter(RecordedCounter::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CryptoSession(RecordedCryptoSession::new())
            } else if [direct3d_11::constants::IID_ID3D11_DEBUG].contains(returned_interface) {
                direct3d_11::UnknownObject::Debug(RecordedDebug::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilState(RecordedDepthStencilState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilView(RecordedDepthStencilView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE,
                direct3d_11::constants::IID_ID3D11_DEVICE1,
                direct3d_11::constants::IID_ID3D11_DEVICE2,
                direct3d_11::constants::IID_ID3D11_DEVICE3,
                direct3d_11::constants::IID_ID3D11_DEVICE4,
                direct3d_11::constants::IID_ID3D11_DEVICE5,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT3,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContext(RecordedDeviceContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DomainShader(RecordedDomainShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_FENCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::GeometryShader(RecordedGeometryShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_HULL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::HullShader(RecordedHullShader::new())
            } else if [direct3d_11::constants::IID_ID3D11_INFO_QUEUE].contains(returned_interface) {
                direct3d_11::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::InputLayout(RecordedInputLayout::new())
            } else if [direct3d_11::constants::IID_ID3D11_MULTITHREAD].contains(returned_interface)
            {
                direct3d_11::UnknownObject::Multithread(RecordedMultithread::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::PixelShader(RecordedPixelShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PREDICATE,
                direct3d_11::constants::IID_ID3D11_QUERY,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Predicate(RecordedPredicate::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_QUERY,
                direct3d_11::constants::IID_ID3D11_QUERY1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Query(RecordedQuery::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE1,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RasterizerState(RecordedRasterizerState::new())
            } else if [direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefDefaultTrackingOptions(
                    RecordedRefDefaultTrackingOptions::new(),
                )
            } else if [direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefTrackingOptions(RecordedRefTrackingOptions::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RenderTargetView(RecordedRenderTargetView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SamplerState(RecordedSamplerState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderResourceView(RecordedShaderResourceView::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE].contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTrace(RecordedShaderTrace::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTraceFactory(RecordedShaderTraceFactory::new())
            } else if [direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SwitchToRef(RecordedSwitchToRef::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture1D(RecordedTexture1D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture2D(RecordedTexture2D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture3D(RecordedTexture3D::new())
            } else if [direct3d_11::constants::IID_ID3D11_TRACING_DEVICE]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::TracingDevice(RecordedTracingDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UnorderedAccessView(RecordedUnorderedAccessView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VertexShader(RecordedVertexShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoContext(RecordedVideoContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoder(RecordedVideoDecoder::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoderOutputView(
                    RecordedVideoDecoderOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE1,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDevice(RecordedVideoDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessor(RecordedVideoProcessor::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorEnumerator(
                    RecordedVideoProcessorEnumerator::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorInputView(
                    RecordedVideoProcessorInputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorOutputView(
                    RecordedVideoProcessorOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContextState(RecordedDeviceContextState::new())
            } else if [direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UserDefinedAnnotation(
                    RecordedUserDefinedAnnotation::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> direct3d_11::Device4 for RecordedDevice<A> {
    fn register_device_removed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "ID3D11Device4::RegisterDeviceRemovedEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_device_removed(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "ID3D11Device4::UnregisterDeviceRemoved(dw_cookie={:?})",
            dw_cookie
        ));
    }
}

impl<A: Recorded> direct3d_11::Device3 for RecordedDevice<A> {
    fn create_texture_2d1(
        &self,
        desc1: &direct3d_11::structures::Texture2DDesc1,
        initial_data: Option<&direct3d_11::structures::SubresourceData>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Texture2D,
        component_object_model::structures::HResult,
    > {
        let _ = &desc1;
        let _ = &initial_data;
        record("ID3D11Device3::CreateTexture2D1()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_texture_3d1(
        &self,
        desc1: &direct3d_11::structures::Texture3DDesc1,
        initial_data: Option<&direct3d_11::structures::SubresourceData>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Texture3D,
        component_object_model::structures::HResult,
    > {
        let _ = &desc1;
        let _ = &initial_data;
        record("ID3D11Device3::CreateTexture3D1()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_rasterizer_state2(
        &self,
        rasterizer_desc: &direct3d_11::structures::RasterizerDesc2,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::RasterizerState,
        component_object_model::structures::HResult,
    > {
        let _ = &rasterizer_desc;
        record("ID3D11Device3::CreateRasterizerState2()".to_string());
        Ok(RecordedRasterizerState::new())
    }
    fn create_shader_resource_view1(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc1: Option<&direct3d_11::structures::ShaderResourceViewDesc1>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc1;
        record("ID3D11Device3::CreateShaderResourceView1()".to_string());
        Ok(RecordedShaderResourceView::new())
    }
    fn create_unordered_access_view1(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc1: Option<&direct3d_11::structures::UnorderedAccessViewDesc1>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc1;
        record("ID3D11Device3::CreateUnorderedAccessView1()".to_string());
        Ok(RecordedUnorderedAccessView::new())
    }
    fn create_render_target_view1(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc1: Option<&direct3d_11::structures::RenderTargetViewDesc1>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::RenderTargetView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc1;
        record("ID3D11Device3::CreateRenderTargetView1()".to_string());
        Ok(RecordedRenderTargetView::new())
    }
    fn create_query1(
        &self,
        query_desc1: &direct3d_11::structures::QueryDesc1,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Query,
        component_object_model::structures::HResult,
    > {
        let _ = &query_desc1;
        record("ID3D11Device3::CreateQuery1()".to_string());
        Ok(RecordedQuery::new())
    }
    fn get_immediate_context3(
        &self,
    ) -> Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContext> {
        record("ID3D11Device3::GetImmediateContext3()".to_string());
        Some(RecordedDeviceContext::new())
    }
    fn create_deferred_context3(
        &self,
        context_flags: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DeviceContext,
        component_object_model::structures::HResult,
    > {
        let _ = &context_flags;
        record(format!(
            "ID3D11Device3::CreateDeferredContext3(context_flags={:?})",
            context_flags
        ));
        Ok(RecordedDeviceContext::new())
    }
    fn write_to_subresource(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&direct3d_11::structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &dst_box;
        let _ = &src_data;
        let _ = &src_row_pitch;
        let _ = &src_depth_pitch;
        record("ID3D11Device3::WriteToSubresource()".to_string());
    }
    fn read_from_subresource(
        &self,
        dst_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        dst_row_pitch: u32,
        dst_depth_pitch: u32,
        src_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&direct3d_11::structures::Box>,
    ) {
        let _ = &dst_data;
        let _ = &dst_row_pitch;
        let _ = &dst_depth_pitch;
        let _ = &src_resource;
        let _ = &src_subresource;
        let _ = &src_box;
        record("ID3D11Device3::ReadFromSubresource()".to_string());
    }
}

impl<A: Recorded> direct3d_11::Device2 for RecordedDevice<A> {
    fn get_immediate_context2(
        &self,
    ) -> Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContext> {
        record("ID3D11Device2::GetImmediateContext2()".to_string());
        Some(RecordedDeviceContext::new())
    }
    fn create_deferred_context2(
        &self,
        context_flags: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DeviceContext,
        component_object_model::structures::HResult,
    > {
        let _ = &context_flags;
        record(format!(
            "ID3D11Device2::CreateDeferredContext2(context_flags={:?})",
            context_flags
        ));
        Ok(RecordedDeviceContext::new())
    }
    fn get_resource_tiling(
        &self,
        tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        first_subresource_tiling_to_get: u32,
    ) -> (
        u32,
        direct3d_11::structures::PackedMipDesc,
        direct3d_11::structures::TileShape,
        Vec<direct3d_11::structures::SubresourceTiling>,
    ) {
        let _ = &tiled_resource;
        let _ = &first_subresource_tiling_to_get;
        record("ID3D11Device2::GetResourceTiling()".to_string());
        (
            Default::default(),
            data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_packed_mip_desc(
                &[u8::MIN; 12],
            ),
            data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_tile_shape(
                &[u8::MIN; 12],
            ),
            Vec::new(),
        )
    }
    fn check_multisample_quality_levels1(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        sample_count: u32,
        flags: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        let _ = &sample_count;
        let _ = &flags;
        record(format!(
            "ID3D11Device2::CheckMultisampleQualityLevels1(format={:?}, sample_count={:?}, flags={:?})",
            format, sample_count, flags
        ));
        Ok(Default::default())
    }
}

impl<A: Recorded> direct3d_11::Device1 for RecordedDevice<A> {
    fn get_immediate_context1(
        &self,
    ) -> Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContext> {
        record("ID3D11Device1::GetImmediateContext1()".to_string());
        Some(RecordedDeviceContext::new())
    }
    fn create_deferred_context1(
        &self,
        context_flags: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DeviceContext,
        component_object_model::structures::HResult,
    > {
        let _ = &context_flags;
        record(format!(
            "ID3D11Device1::CreateDeferredContext1(context_flags={:?})",
            context_flags
        ));
        Ok(RecordedDeviceContext::new())
    }
    fn create_blend_state1(
        &self,
        blend_state_desc: &direct3d_11::structures::BlendDesc1,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::BlendState,
        component_object_model::structures::HResult,
    > {
        let _ = &blend_state_desc;
        record("ID3D11Device1::CreateBlendState1()".to_string());
        Ok(RecordedBlendState::new())
    }
    fn create_rasterizer_state1(
        &self,
        rasterizer_desc: &direct3d_11::structures::RasterizerDesc1,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::RasterizerState,
        component_object_model::structures::HResult,
    > {
        let _ = &rasterizer_desc;
        record("ID3D11Device1::CreateRasterizerState1()".to_string());
        Ok(RecordedRasterizerState::new())
    }
    fn create_device_context_state(
        &self,
        flags: u32,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
        emulated_interface: &component_object_model::structures::Guid,
    ) -> Result<
        (
            direct3d_common::enumerations::FeatureLevel,
            Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContextState>,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &flags;
        let _ = &feature_levels;
        let _ = &sdk_version;
        let _ = &emulated_interface;
        record(format!(
            "ID3D11Device1::CreateDeviceContextState(flags={:?}, sdk_version={:?})",
            flags, sdk_version
        ));
        Ok((Default::default(), Some(RecordedDeviceContextState::new())))
    }
    fn open_shared_resource1(
        &self,
        h_resource: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<direct3d_11::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &h_resource;
        let _ = &returned_interface;
        record(format!(
            "ID3D11Device1::OpenSharedResource1(h_resource={:?})",
            h_resource
        ));
        Ok(
            if [
                direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::AuthenticatedChannel(RecordedAuthenticatedChannel::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BLEND_STATE,
                direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::BlendState(RecordedBlendState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BUFFER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Buffer(RecordedBuffer::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassInstance(RecordedClassInstance::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassLinkage(RecordedClassLinkage::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CommandList(RecordedCommandList::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ComputeShader(RecordedComputeShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_COUNTER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Counter(RecordedCounter::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CryptoSession(RecordedCryptoSession::new())
            } else if [direct3d_11::constants::IID_ID3D11_DEBUG].contains(returned_interface) {
                direct3d_11::UnknownObject::Debug(RecordedDebug::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilState(RecordedDepthStencilState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilView(RecordedDepthStencilView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE,
                direct3d_11::constants::IID_ID3D11_DEVICE1,
                direct3d_11::constants::IID_ID3D11_DEVICE2,
                direct3d_11::constants::IID_ID3D11_DEVICE3,
                direct3d_11::constants::IID_ID3D11_DEVICE4,
                direct3d_11::constants::IID_ID3D11_DEVICE5,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT3,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContext(RecordedDeviceContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DomainShader(RecordedDomainShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_FENCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::GeometryShader(RecordedGeometryShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_HULL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::HullShader(RecordedHullShader::new())
            } else if [direct3d_11::constants::IID_ID3D11_INFO_QUEUE].contains(returned_interface) {
                direct3d_11::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::InputLayout(RecordedInputLayout::new())
            } else if [direct3d_11::constants::IID_ID3D11_MULTITHREAD].contains(returned_interface)
            {
                direct3d_11::UnknownObject::Multithread(RecordedMultithread::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::PixelShader(RecordedPixelShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PREDICATE,
                direct3d_11::constants::IID_ID3D11_QUERY,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Predicate(RecordedPredicate::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_QUERY,
                direct3d_11::constants::IID_ID3D11_QUERY1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Query(RecordedQuery::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE1,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RasterizerState(RecordedRasterizerState::new())
            } else if [direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefDefaultTrackingOptions(
                    RecordedRefDefaultTrackingOptions::new(),
                )
            } else if [direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefTrackingOptions(RecordedRefTrackingOptions::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RenderTargetView(RecordedRenderTargetView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SamplerState(RecordedSamplerState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderResourceView(RecordedShaderResourceView::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE].contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTrace(RecordedShaderTrace::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTraceFactory(RecordedShaderTraceFactory::new())
            } else if [direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SwitchToRef(RecordedSwitchToRef::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture1D(RecordedTexture1D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture2D(RecordedTexture2D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture3D(RecordedTexture3D::new())
            } else if [direct3d_11::constants::IID_ID3D11_TRACING_DEVICE]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::TracingDevice(RecordedTracingDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UnorderedAccessView(RecordedUnorderedAccessView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VertexShader(RecordedVertexShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoContext(RecordedVideoContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoder(RecordedVideoDecoder::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoderOutputView(
                    RecordedVideoDecoderOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE1,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDevice(RecordedVideoDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessor(RecordedVideoProcessor::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorEnumerator(
                    RecordedVideoProcessorEnumerator::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorInputView(
                    RecordedVideoProcessorInputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorOutputView(
                    RecordedVideoProcessorOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContextState(RecordedDeviceContextState::new())
            } else if [direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UserDefinedAnnotation(
                    RecordedUserDefinedAnnotation::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn open_shared_resource_by_name(
        &self,
        lp_name: &[u16],
        dw_desired_access: u32,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<direct3d_11::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &lp_name;
        let _ = &dw_desired_access;
        let _ = &returned_interface;
        record(format!(
            "ID3D11Device1::OpenSharedResourceByName(dw_desired_access={:?})",
            dw_desired_access
        ));
        Ok(
            if [
                direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::AuthenticatedChannel(RecordedAuthenticatedChannel::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BLEND_STATE,
                direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::BlendState(RecordedBlendState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BUFFER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Buffer(RecordedBuffer::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassInstance(RecordedClassInstance::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassLinkage(RecordedClassLinkage::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CommandList(RecordedCommandList::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ComputeShader(RecordedComputeShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_COUNTER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Counter(RecordedCounter::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CryptoSession(RecordedCryptoSession::new())
            } else if [direct3d_11::constants::IID_ID3D11_DEBUG].contains(returned_interface) {
                direct3d_11::UnknownObject::Debug(RecordedDebug::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilState(RecordedDepthStencilState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilView(RecordedDepthStencilView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE,
                direct3d_11::constants::IID_ID3D11_DEVICE1,
                direct3d_11::constants::IID_ID3D11_DEVICE2,
                direct3d_11::constants::IID_ID3D11_DEVICE3,
                direct3d_11::constants::IID_ID3D11_DEVICE4,
                direct3d_11::constants::IID_ID3D11_DEVICE5,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT3,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContext(RecordedDeviceContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DomainShader(RecordedDomainShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_FENCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::GeometryShader(RecordedGeometryShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_HULL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::HullShader(RecordedHullShader::new())
            } else if [direct3d_11::constants::IID_ID3D11_INFO_QUEUE].contains(returned_interface) {
                direct3d_11::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::InputLayout(RecordedInputLayout::new())
            } else if [direct3d_11::constants::IID_ID3D11_MULTITHREAD].contains(returned_interface)
            {
                direct3d_11::UnknownObject::Multithread(RecordedMultithread::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::PixelShader(RecordedPixelShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PREDICATE,
                direct3d_11::constants::IID_ID3D11_QUERY,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Predicate(RecordedPredicate::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_QUERY,
                direct3d_11::constants::IID_ID3D11_QUERY1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Query(RecordedQuery::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE1,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RasterizerState(RecordedRasterizerState::new())
            } else if [direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefDefaultTrackingOptions(
                    RecordedRefDefaultTrackingOptions::new(),
                )
            } else if [direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefTrackingOptions(RecordedRefTrackingOptions::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RenderTargetView(RecordedRenderTargetView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SamplerState(RecordedSamplerState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderResourceView(RecordedShaderResourceView::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE].contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTrace(RecordedShaderTrace::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTraceFactory(RecordedShaderTraceFactory::new())
            } else if [direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SwitchToRef(RecordedSwitchToRef::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture1D(RecordedTexture1D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture2D(RecordedTexture2D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture3D(RecordedTexture3D::new())
            } else if [direct3d_11::constants::IID_ID3D11_TRACING_DEVICE]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::TracingDevice(RecordedTracingDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UnorderedAccessView(RecordedUnorderedAccessView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VertexShader(RecordedVertexShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoContext(RecordedVideoContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoder(RecordedVideoDecoder::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoderOutputView(
                    RecordedVideoDecoderOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE1,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDevice(RecordedVideoDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessor(RecordedVideoProcessor::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorEnumerator(
                    RecordedVideoProcessorEnumerator::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorInputView(
                    RecordedVideoProcessorInputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorOutputView(
                    RecordedVideoProcessorOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContextState(RecordedDeviceContextState::new())
            } else if [direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UserDefinedAnnotation(
                    RecordedUserDefinedAnnotation::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> direct3d_11::Device for RecordedDevice<A> {
    type Api = A;

    fn create_buffer(
        &self,
        desc: &direct3d_11::structures::BufferDesc,
        initial_data: Option<&direct3d_11::structures::SubresourceData>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Buffer,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &initial_data;
        record("ID3D11Device::CreateBuffer()".to_string());
        Ok(RecordedBuffer::new())
    }
    fn create_texture_1d(
        &self,
        desc: &direct3d_11::structures::Texture1DDesc,
        initial_data: &[direct3d_11::structures::SubresourceData],
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Texture1D,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &initial_data;
        record("ID3D11Device::CreateTexture1D()".to_string());
        Ok(RecordedTexture1D::new())
    }
    fn create_texture_2d(
        &self,
        desc: &direct3d_11::structures::Texture2DDesc,
        initial_data: &[direct3d_11::structures::SubresourceData],
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Texture2D,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &initial_data;
        record("ID3D11Device::CreateTexture2D()".to_string());
        Ok(RecordedTexture2D::new())
    }
    fn create_texture_3d(
        &self,
        desc: &direct3d_11::structures::Texture3DDesc,
        initial_data: &[direct3d_11::structures::SubresourceData],
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Texture3D,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &initial_data;
        record("ID3D11Device::CreateTexture3D()".to_string());
        Ok(RecordedTexture3D::new())
    }
    fn create_shader_resource_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc: Option<&direct3d_11::structures::ShaderResourceViewDesc>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc;
        record("ID3D11Device::CreateShaderResourceView()".to_string());
        Ok(RecordedShaderResourceView::new())
    }
    fn create_unordered_access_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc: Option<&direct3d_11::structures::UnorderedAccessViewDesc>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc;
        record("ID3D11Device::CreateUnorderedAccessView()".to_string());
        Ok(RecordedUnorderedAccessView::new())
    }
    fn create_render_target_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc: Option<&direct3d_11::structures::RenderTargetViewDesc>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::RenderTargetView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc;
        record("ID3D11Device::CreateRenderTargetView()".to_string());
        Ok(RecordedRenderTargetView::new())
    }
    fn create_depth_stencil_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc: Option<&direct3d_11::structures::DepthStencilViewDesc>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DepthStencilView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc;
        record("ID3D11Device::CreateDepthStencilView()".to_string());
        Ok(RecordedDepthStencilView::new())
    }
    fn create_input_layout(
        &self,
        input_element_descs: &[direct3d_11::structures::InputElementDesc],
        shader_bytecode_with_input_signature: &[u8],
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::InputLayout,
        component_object_model::structures::HResult,
    > {
        let _ = &input_element_descs;
        let _ = &shader_bytecode_with_input_signature;
        record("ID3D11Device::CreateInputLayout()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_vertex_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VertexShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreateVertexShader()".to_string());
        Ok(RecordedVertexShader::new())
    }
    fn create_geometry_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::GeometryShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreateGeometryShader()".to_string());
        Ok(RecordedGeometryShader::new())
    }
    fn create_geometry_shader_with_stream_output(
        &self,
        shader_bytecode: &[u8],
        so_declaration: &[direct3d_11::structures::SoDeclarationEntry],
        buffer_strides: &[u32],
        rasterized_stream: u32,
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::GeometryShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &so_declaration;
        let _ = &buffer_strides;
        let _ = &rasterized_stream;
        let _ = &class_linkage;
        record("ID3D11Device::CreateGeometryShaderWithStreamOutput()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_pixel_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::PixelShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreatePixelShader()".to_string());
        Ok(RecordedPixelShader::new())
    }
    fn create_hull_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::HullShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreateHullShader()".to_string());
        Ok(RecordedHullShader::new())
    }
    fn create_domain_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DomainShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreateDomainShader()".to_string());
        Ok(RecordedDomainShader::new())
    }
    fn create_compute_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as direct3d_11::Direct3D11>::ClassLinkage>,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ComputeShader,
        component_object_model::structures::HResult,
    > {
        let _ = &shader_bytecode;
        let _ = &class_linkage;
        record("ID3D11Device::CreateComputeShader()".to_string());
        Ok(RecordedComputeShader::new())
    }
    fn create_class_linkage(
        &self,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ClassLinkage,
        component_object_model::structures::HResult,
    > {
        record("ID3D11Device::CreateClassLinkage()".to_string());
        Ok(RecordedClassLinkage::new())
    }
    fn create_blend_state(
        &self,
        blend_state_desc: &direct3d_11::structures::BlendDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::BlendState,
        component_object_model::structures::HResult,
    > {
        let _ = &blend_state_desc;
        record("ID3D11Device::CreateBlendState()".to_string());
        Ok(RecordedBlendState::new())
    }
    fn create_depth_stencil_state(
        &self,
        depth_stencil_desc: &direct3d_11::structures::DepthStencilDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DepthStencilState,
        component_object_model::structures::HResult,
    > {
        let _ = &depth_stencil_desc;
        record("ID3D11Device::CreateDepthStencilState()".to_string());
        Ok(RecordedDepthStencilState::new())
    }
    fn create_rasterizer_state(
        &self,
        rasterizer_desc: &direct3d_11::structures::RasterizerDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::RasterizerState,
        component_object_model::structures::HResult,
    > {
        let _ = &rasterizer_desc;
        record("ID3D11Device::CreateRasterizerState()".to_string());
        Ok(RecordedRasterizerState::new())
    }
    fn create_sampler_state(
        &self,
        sampler_desc: &direct3d_11::structures::SamplerDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::SamplerState,
        component_object_model::structures::HResult,
    > {
        let _ = &sampler_desc;
        record("ID3D11Device::CreateSamplerState()".to_string());
        Ok(RecordedSamplerState::new())
    }
    fn create_query(
        &self,
        query_desc: &direct3d_11::structures::QueryDesc,
    ) -> Result<direct3d_11::QueryObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &query_desc;
        record("ID3D11Device::CreateQuery()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_predicate(
        &self,
        predicate_desc: &direct3d_11::structures::QueryDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Predicate,
        component_object_model::structures::HResult,
    > {
        let _ = &predicate_desc;
        record("ID3D11Device::CreatePredicate()".to_string());
        Ok(RecordedPredicate::new())
    }
    fn create_counter(
        &self,
        counter_desc: &direct3d_11::structures::CounterDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::Counter,
        component_object_model::structures::HResult,
    > {
        let _ = &counter_desc;
        record("ID3D11Device::CreateCounter()".to_string());
        Ok(RecordedCounter::new())
    }
    fn create_deferred_context(
        &self,
        context_flags: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::DeviceContext,
        component_object_model::structures::HResult,
    > {
        let _ = &context_flags;
        record(format!(
            "ID3D11Device::CreateDeferredContext(context_flags={:?})",
            context_flags
        ));
        Ok(RecordedDeviceContext::new())
    }
    fn open_shared_resource(
        &self,
        h_resource: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<direct3d_11::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &h_resource;
        let _ = &returned_interface;
        record(format!(
            "ID3D11Device::OpenSharedResource(h_resource={:?})",
            h_resource
        ));
        Ok(
            if [
                direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::AuthenticatedChannel(RecordedAuthenticatedChannel::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BLEND_STATE,
                direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::BlendState(RecordedBlendState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_BUFFER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Buffer(RecordedBuffer::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassInstance(RecordedClassInstance::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ClassLinkage(RecordedClassLinkage::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CommandList(RecordedCommandList::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ComputeShader(RecordedComputeShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_COUNTER,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Counter(RecordedCounter::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::CryptoSession(RecordedCryptoSession::new())
            } else if [direct3d_11::constants::IID_ID3D11_DEBUG].contains(returned_interface) {
                direct3d_11::UnknownObject::Debug(RecordedDebug::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilState(RecordedDepthStencilState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DepthStencilView(RecordedDepthStencilView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE,
                direct3d_11::constants::IID_ID3D11_DEVICE1,
                direct3d_11::constants::IID_ID3D11_DEVICE2,
                direct3d_11::constants::IID_ID3D11_DEVICE3,
                direct3d_11::constants::IID_ID3D11_DEVICE4,
                direct3d_11::constants::IID_ID3D11_DEVICE5,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT3,
                direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContext(RecordedDeviceContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DomainShader(RecordedDomainShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_FENCE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::GeometryShader(RecordedGeometryShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_HULL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::HullShader(RecordedHullShader::new())
            } else if [direct3d_11::constants::IID_ID3D11_INFO_QUEUE].contains(returned_interface) {
                direct3d_11::UnknownObject::InfoQueue(RecordedInfoQueue::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::InputLayout(RecordedInputLayout::new())
            } else if [direct3d_11::constants::IID_ID3D11_MULTITHREAD].contains(returned_interface)
            {
                direct3d_11::UnknownObject::Multithread(RecordedMultithread::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::PixelShader(RecordedPixelShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_PREDICATE,
                direct3d_11::constants::IID_ID3D11_QUERY,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Predicate(RecordedPredicate::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_ASYNCHRONOUS,
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_QUERY,
                direct3d_11::constants::IID_ID3D11_QUERY1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Query(RecordedQuery::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE1,
                direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RasterizerState(RecordedRasterizerState::new())
            } else if [direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefDefaultTrackingOptions(
                    RecordedRefDefaultTrackingOptions::new(),
                )
            } else if [direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RefTrackingOptions(RecordedRefTrackingOptions::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW,
                direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::RenderTargetView(RecordedRenderTargetView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SamplerState(RecordedSamplerState::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW,
                direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderResourceView(RecordedShaderResourceView::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE].contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTrace(RecordedShaderTrace::new())
            } else if [direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::ShaderTraceFactory(RecordedShaderTraceFactory::new())
            } else if [direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::SwitchToRef(RecordedSwitchToRef::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture1D(RecordedTexture1D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture2D(RecordedTexture2D::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_RESOURCE,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D,
                direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::Texture3D(RecordedTexture3D::new())
            } else if [direct3d_11::constants::IID_ID3D11_TRACING_DEVICE]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::TracingDevice(RecordedTracingDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW,
                direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UnorderedAccessView(RecordedUnorderedAccessView::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VertexShader(RecordedVertexShader::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT1,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT2,
                direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoContext(RecordedVideoContext::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoder(RecordedVideoDecoder::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDecoderOutputView(
                    RecordedVideoDecoderOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE1,
                direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoDevice(RecordedVideoDevice::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessor(RecordedVideoProcessor::new())
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorEnumerator(
                    RecordedVideoProcessorEnumerator::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorInputView(
                    RecordedVideoProcessorInputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                direct3d_11::constants::IID_ID3D11_VIEW,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::VideoProcessorOutputView(
                    RecordedVideoProcessorOutputView::new(),
                )
            } else if [
                direct3d_11::constants::IID_ID3D11_DEVICE_CHILD,
                direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
            ]
            .contains(returned_interface)
            {
                direct3d_11::UnknownObject::DeviceContextState(RecordedDeviceContextState::new())
            } else if [direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION]
                .contains(returned_interface)
            {
                direct3d_11::UnknownObject::UserDefinedAnnotation(
                    RecordedUserDefinedAnnotation::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn check_format_support(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        record(format!(
            "ID3D11Device::CheckFormatSupport(format={:?})",
            format
        ));
        Ok(Default::default())
    }
    fn check_multisample_quality_levels(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        sample_count: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        let _ = &sample_count;
        record(format!(
            "ID3D11Device::CheckMultisampleQualityLevels(format={:?}, sample_count={:?})",
            format, sample_count
        ));
        Ok(Default::default())
    }
    fn check_counter_info(&self) -> direct3d_11::structures::CounterInfo {
        record("ID3D11Device::CheckCounterInfo()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_counter_info(
            &[u8::MIN; 12],
        )
    }
    fn check_counter(
        &self,
        desc: &direct3d_11::structures::CounterDesc,
        sz_name: Option<&std::ffi::CStr>,
        sz_units: Option<&std::ffi::CStr>,
        sz_description: Option<&std::ffi::CStr>,
    ) -> Result<
        (direct3d_11::enumerations::CounterType, u32),
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &sz_name;
        let _ = &sz_units;
        let _ = &sz_description;
        record("ID3D11Device::CheckCounter()".to_string());
        Err(E_NOTIMPL)
    }
    fn check_feature_support(
        &self,
        feature: direct3d_11::enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &feature;
        let _ = &feature_support_data;
        record(format!(
            "ID3D11Device::CheckFeatureSupport(feature={:?})",
            feature
        ));
        Ok(())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11Device::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11Device::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11Device::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_feature_level(&self) -> direct3d_common::enumerations::FeatureLevel {
        record("ID3D11Device::GetFeatureLevel()".to_string());
        Default::default()
    }
    fn get_creation_flags(&self) -> u32 {
        record("ID3D11Device::GetCreationFlags()".to_string());
        Default::default()
    }
    fn get_device_removed_reason(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D11Device::GetDeviceRemovedReason()".to_string());
        Ok(())
    }
    fn get_immediate_context(
        &self,
    ) -> Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContext> {
        record("ID3D11Device::GetImmediateContext()".to_string());
        Some(RecordedDeviceContext::new())
    }
    fn set_exception_mode(
        &self,
        raise_flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &raise_flags;
        record(format!(
            "ID3D11Device::SetExceptionMode(raise_flags={:?})",
            raise_flags
        ));
        Ok(())
    }
    fn get_exception_mode(&self) -> u32 {
        record("ID3D11Device::GetExceptionMode()".to_string());
        Default::default()
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
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
        Ok(directx_graphics_infrastructure_recorder::RecordedAdapter::new())
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
        Ok((0..num_surfaces.min(4))
            .map(|_| Some(directx_graphics_infrastructure_recorder::RecordedSurface::new()))
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
        Ok(())
    }
    fn get_gpu_thread_priority(&self) -> Result<i32, component_object_model::structures::HResult> {
        record("IDXGIDevice::GetGPUThreadPriority()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGIDevice1::GetMaximumFrameLatency()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn reclaim_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<bool>, component_object_model::structures::HResult> {
        let _ = &resources;
        record("IDXGIDevice2::ReclaimResources()".to_string());
        Ok((0..2_u32).map(|_| Default::default()).collect())
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
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Device3 for RecordedDevice<A> {
    fn trim(&self) {
        record("IDXGIDevice3::Trim()".to_string());
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
        Ok((0..2_u32).map(|_| Default::default()).collect())
    }
}

impl<A: Recorded> direct3d_11::Multithread for RecordedDevice<A> {
    fn enter(&self) {
        record("ID3D11Multithread::Enter()".to_string());
    }
    fn leave(&self) {
        record("ID3D11Multithread::Leave()".to_string());
    }
    fn set_multithread_protected(&self, b_mt_protect: bool) -> bool {
        let _ = &b_mt_protect;
        record(format!(
            "ID3D11Multithread::SetMultithreadProtected(b_mt_protect={:?})",
            b_mt_protect
        ));
        Default::default()
    }
    fn get_multithread_protected(&self) -> bool {
        record("ID3D11Multithread::GetMultithreadProtected()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDevice<A> {
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

impl<A: Recorded> direct3d_11::DeviceContext4 for RecordedDeviceContext<A> {
    fn signal(
        &self,
        fence: &<Self::Api as direct3d_11::Direct3D11>::Fence,
        value: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fence;
        let _ = &value;
        record(format!("ID3D11DeviceContext4::Signal(value={:?})", value));
        Ok(())
    }
    fn wait(
        &self,
        fence: &<Self::Api as direct3d_11::Direct3D11>::Fence,
        value: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fence;
        let _ = &value;
        record(format!("ID3D11DeviceContext4::Wait(value={:?})", value));
        Ok(())
    }
}

impl<A: Recorded> direct3d_11::DeviceContext3 for RecordedDeviceContext<A> {
    fn flush1(
        &self,
        context_type: direct3d_11::enumerations::ContextType,
        h_event: component_object_model::structures::Handle,
    ) {
        let _ = &context_type;
        let _ = &h_event;
        record(format!(
            "ID3D11DeviceContext3::Flush1(context_type={:?}, h_event={:?})",
            context_type, h_event
        ));
    }
    fn set_hardware_protection_state(&self, hw_protection_enable: bool) {
        let _ = &hw_protection_enable;
        record(format!(
            "ID3D11DeviceContext3::SetHardwareProtectionState(hw_protection_enable={:?})",
            hw_protection_enable
        ));
    }
    fn get_hardware_protection_state(&self) -> bool {
        record("ID3D11DeviceContext3::GetHardwareProtectionState()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceContext2 for RecordedDeviceContext<A> {
    fn update_tile_mappings(
        &self,
        tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        tiled_resource_region_start_coordinates: &[direct3d_11::structures::TiledResourceCoordinate],
        tiled_resource_region_sizes: &[direct3d_11::structures::TileRegionSize],
        tile_pool: Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>,
        range_flags: &[u32],
        tile_pool_start_offsets: &[u32],
        range_tile_counts: &[u32],
        flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &tiled_resource;
        let _ = &tiled_resource_region_start_coordinates;
        let _ = &tiled_resource_region_sizes;
        let _ = &tile_pool;
        let _ = &range_flags;
        let _ = &tile_pool_start_offsets;
        let _ = &range_tile_counts;
        let _ = &flags;
        record(format!(
            "ID3D11DeviceContext2::UpdateTileMappings(flags={:?})",
            flags
        ));
        Ok(())
    }
    fn copy_tile_mappings(
        &self,
        dest_tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dest_region_start_coordinate: &direct3d_11::structures::TiledResourceCoordinate,
        source_tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        source_region_start_coordinate: &direct3d_11::structures::TiledResourceCoordinate,
        tile_region_size: &direct3d_11::structures::TileRegionSize,
        flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dest_tiled_resource;
        let _ = &dest_region_start_coordinate;
        let _ = &source_tiled_resource;
        let _ = &source_region_start_coordinate;
        let _ = &tile_region_size;
        let _ = &flags;
        record(format!(
            "ID3D11DeviceContext2::CopyTileMappings(flags={:?})",
            flags
        ));
        Ok(())
    }
    fn copy_tiles(
        &self,
        tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        tile_region_start_coordinate: &direct3d_11::structures::TiledResourceCoordinate,
        tile_region_size: &direct3d_11::structures::TileRegionSize,
        buffer: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        buffer_start_offset_in_bytes: u64,
        flags: u32,
    ) {
        let _ = &tiled_resource;
        let _ = &tile_region_start_coordinate;
        let _ = &tile_region_size;
        let _ = &buffer;
        let _ = &buffer_start_offset_in_bytes;
        let _ = &flags;
        record(format!(
            "ID3D11DeviceContext2::CopyTiles(buffer_start_offset_in_bytes={:?}, flags={:?})",
            buffer_start_offset_in_bytes, flags
        ));
    }
    fn update_tiles(
        &self,
        dest_tiled_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dest_tile_region_start_coordinate: &direct3d_11::structures::TiledResourceCoordinate,
        dest_tile_region_size: &direct3d_11::structures::TileRegionSize,
        source_tile_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        flags: u32,
    ) {
        let _ = &dest_tiled_resource;
        let _ = &dest_tile_region_start_coordinate;
        let _ = &dest_tile_region_size;
        let _ = &source_tile_data;
        let _ = &flags;
        record("ID3D11DeviceContext2::UpdateTiles()".to_string());
    }
    fn resize_tile_pool(
        &self,
        tile_pool: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        new_size_in_bytes: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &tile_pool;
        let _ = &new_size_in_bytes;
        record(format!(
            "ID3D11DeviceContext2::ResizeTilePool(new_size_in_bytes={:?})",
            new_size_in_bytes
        ));
        Ok(())
    }
    fn tiled_resource_barrier(
        &self,
        tiled_resource_or_view_access_before_barrier: Option<
            direct3d_11::DeviceChildRef<'_, Self::Api>,
        >,
        tiled_resource_or_view_access_after_barrier: Option<
            direct3d_11::DeviceChildRef<'_, Self::Api>,
        >,
    ) {
        let _ = &tiled_resource_or_view_access_before_barrier;
        let _ = &tiled_resource_or_view_access_after_barrier;
        record("ID3D11DeviceContext2::TiledResourceBarrier()".to_string());
    }
    fn is_annotation_enabled(&self) -> bool {
        record("ID3D11DeviceContext2::IsAnnotationEnabled()".to_string());
        Default::default()
    }
    fn set_marker_int(&self, label: &[u16], data: i32) {
        let _ = &label;
        let _ = &data;
        record(format!(
            "ID3D11DeviceContext2::SetMarkerInt(data={:?})",
            data
        ));
    }
    fn begin_event_int(&self, label: &[u16], data: i32) {
        let _ = &label;
        let _ = &data;
        record(format!(
            "ID3D11DeviceContext2::BeginEventInt(data={:?})",
            data
        ));
    }
    fn end_event(&self) {
        record("ID3D11DeviceContext2::EndEvent()".to_string());
    }
}

impl<A: Recorded> direct3d_11::DeviceContext1 for RecordedDeviceContext<A> {
    fn copy_subresource_region1(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&direct3d_11::structures::Box>,
        copy_flags: u32,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &dst_x;
        let _ = &dst_y;
        let _ = &dst_z;
        let _ = &src_resource;
        let _ = &src_subresource;
        let _ = &src_box;
        let _ = &copy_flags;
        record(format!(
            "ID3D11DeviceContext1::CopySubresourceRegion1(dst_subresource={:?}, dst_x={:?}, dst_y={:?}, dst_z={:?}, src_subresource={:?}, copy_flags={:?})",
            dst_subresource, dst_x, dst_y, dst_z, src_subresource, copy_flags
        ));
    }
    fn update_subresource1(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&direct3d_11::structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
        copy_flags: u32,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &dst_box;
        let _ = &src_data;
        let _ = &src_row_pitch;
        let _ = &src_depth_pitch;
        let _ = &copy_flags;
        record("ID3D11DeviceContext1::UpdateSubresource1()".to_string());
    }
    fn discard_resource(&self, resource: direct3d_11::ResourceRef<'_, Self::Api>) {
        let _ = &resource;
        record("ID3D11DeviceContext1::DiscardResource()".to_string());
    }
    fn discard_view(&self, resource_view: direct3d_11::ViewRef<'_, Self::Api>) {
        let _ = &resource_view;
        record("ID3D11DeviceContext1::DiscardView()".to_string());
    }
    fn vs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::VSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn hs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::HSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn ds_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::DSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn gs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::GSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn ps_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::PSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn cs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        let _ = &first_constant;
        let _ = &num_constants;
        record(format!(
            "ID3D11DeviceContext1::CSSetConstantBuffers1(start_slot={:?})",
            start_slot
        ));
    }
    fn vs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::VSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn hs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::HSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn ds_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::DSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn gs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::GSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn ps_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::PSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn cs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext1::CSGetConstantBuffers1(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn swap_device_context_state(
        &self,
        state: &<Self::Api as direct3d_11::Direct3D11>::DeviceContextState,
    ) -> Option<<Self::Api as direct3d_11::Direct3D11>::DeviceContextState> {
        let _ = &state;
        record("ID3D11DeviceContext1::SwapDeviceContextState()".to_string());
        Some(RecordedDeviceContextState::new())
    }
    fn clear_view(
        &self,
        view: direct3d_11::ViewRef<'_, Self::Api>,
        color: &[f32; direct3d_11::constants::DEVICE_CONTEXT1_CLEAR_VIEW_COLOR_LENGTH],
        rect: &[component_object_model::structures::Rect],
    ) {
        let _ = &view;
        let _ = &color;
        let _ = &rect;
        record("ID3D11DeviceContext1::ClearView()".to_string());
    }
    fn discard_view1(
        &self,
        resource_view: direct3d_11::ViewRef<'_, Self::Api>,
        rects: &[component_object_model::structures::Rect],
    ) {
        let _ = &resource_view;
        let _ = &rects;
        record("ID3D11DeviceContext1::DiscardView1()".to_string());
    }
}

impl<A: Recorded> direct3d_11::DeviceContext for RecordedDeviceContext<A> {
    fn vs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::VSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn ps_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::PSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn ps_set_shader(
        &self,
        pixel_shader: Option<&<Self::Api as direct3d_11::Direct3D11>::PixelShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &pixel_shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::PSSetShader()".to_string());
    }
    fn ps_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::PSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn vs_set_shader(
        &self,
        vertex_shader: Option<&<Self::Api as direct3d_11::Direct3D11>::VertexShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &vertex_shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::VSSetShader()".to_string());
    }
    fn draw_indexed(&self, index_count: u32, start_index_location: u32, base_vertex_location: i32) {
        let _ = &index_count;
        let _ = &start_index_location;
        let _ = &base_vertex_location;
        record(format!(
            "ID3D11DeviceContext::DrawIndexed(index_count={:?}, start_index_location={:?}, base_vertex_location={:?})",
            index_count, start_index_location, base_vertex_location
        ));
    }
    fn draw(&self, vertex_count: u32, start_vertex_location: u32) {
        let _ = &vertex_count;
        let _ = &start_vertex_location;
        record(format!(
            "ID3D11DeviceContext::Draw(vertex_count={:?}, start_vertex_location={:?})",
            vertex_count, start_vertex_location
        ));
    }
    fn map(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        subresource: u32,
        map_type: direct3d_11::enumerations::Map,
        map_flags: u32,
    ) -> Result<
        direct3d_11::structures::MappedSubresource<<Self::Api as direct3d_11::Direct3D11>::Mapped>,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &subresource;
        let _ = &map_type;
        let _ = &map_flags;
        record(format!(
            "ID3D11DeviceContext::Map(subresource={:?}, map_type={:?}, map_flags={:?})",
            subresource, map_type, map_flags
        ));
        Ok(direct3d_11::structures::MappedSubresource {
            data: RecordedBytes::new(),
            row_pitch: MAPPED_ROW_PITCH,
            depth_pitch: MAPPED_DEPTH_PITCH,
        })
    }
    fn unmap(&self, resource: direct3d_11::ResourceRef<'_, Self::Api>, subresource: u32) {
        let _ = &resource;
        let _ = &subresource;
        record(format!(
            "ID3D11DeviceContext::Unmap(subresource={:?})",
            subresource
        ));
    }
    fn ps_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::PSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn ia_set_input_layout(
        &self,
        input_layout: Option<&<Self::Api as direct3d_11::Direct3D11>::InputLayout>,
    ) {
        let _ = &input_layout;
        record("ID3D11DeviceContext::IASetInputLayout()".to_string());
    }
    fn ia_set_vertex_buffers(
        &self,
        start_slot: u32,
        vertex_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        strides: &[u32],
        offsets: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &vertex_buffers;
        let _ = &strides;
        let _ = &offsets;
        record(format!(
            "ID3D11DeviceContext::IASetVertexBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn ia_set_index_buffer(
        &self,
        index_buffer: Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>,
        format: directx_graphics_infrastructure::enumerations::Format,
        offset: u32,
    ) {
        let _ = &index_buffer;
        let _ = &format;
        let _ = &offset;
        record(format!(
            "ID3D11DeviceContext::IASetIndexBuffer(format={:?}, offset={:?})",
            format, offset
        ));
    }
    fn draw_indexed_instanced(
        &self,
        index_count_per_instance: u32,
        instance_count: u32,
        start_index_location: u32,
        base_vertex_location: i32,
        start_instance_location: u32,
    ) {
        let _ = &index_count_per_instance;
        let _ = &instance_count;
        let _ = &start_index_location;
        let _ = &base_vertex_location;
        let _ = &start_instance_location;
        record(format!(
            "ID3D11DeviceContext::DrawIndexedInstanced(index_count_per_instance={:?}, instance_count={:?}, start_index_location={:?}, base_vertex_location={:?}, start_instance_location={:?})",
            index_count_per_instance,
            instance_count,
            start_index_location,
            base_vertex_location,
            start_instance_location
        ));
    }
    fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    ) {
        let _ = &vertex_count_per_instance;
        let _ = &instance_count;
        let _ = &start_vertex_location;
        let _ = &start_instance_location;
        record(format!(
            "ID3D11DeviceContext::DrawInstanced(vertex_count_per_instance={:?}, instance_count={:?}, start_vertex_location={:?}, start_instance_location={:?})",
            vertex_count_per_instance,
            instance_count,
            start_vertex_location,
            start_instance_location
        ));
    }
    fn gs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::GSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn gs_set_shader(
        &self,
        shader: Option<&<Self::Api as direct3d_11::Direct3D11>::GeometryShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::GSSetShader()".to_string());
    }
    fn ia_set_primitive_topology(
        &self,
        topology: direct3d_common::enumerations::PrimitiveTopology,
    ) {
        let _ = &topology;
        record(format!(
            "ID3D11DeviceContext::IASetPrimitiveTopology(topology={:?})",
            topology
        ));
    }
    fn vs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::VSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn vs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::VSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn begin(&self, async_: direct3d_11::AsynchronousRef<'_, Self::Api>) {
        let _ = &async_;
        record("ID3D11DeviceContext::Begin()".to_string());
    }
    fn end(&self, async_: direct3d_11::AsynchronousRef<'_, Self::Api>) {
        let _ = &async_;
        record("ID3D11DeviceContext::End()".to_string());
    }
    fn get_data(
        &self,
        async_: direct3d_11::AsynchronousRef<'_, Self::Api>,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
        get_data_flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &async_;
        let _ = &data;
        let _ = &get_data_flags;
        record(format!(
            "ID3D11DeviceContext::GetData(get_data_flags={:?})",
            get_data_flags
        ));
        Ok(())
    }
    fn set_predication(
        &self,
        predicate: Option<&<Self::Api as direct3d_11::Direct3D11>::Predicate>,
        predicate_value: bool,
    ) {
        let _ = &predicate;
        let _ = &predicate_value;
        record(format!(
            "ID3D11DeviceContext::SetPredication(predicate_value={:?})",
            predicate_value
        ));
    }
    fn gs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::GSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn gs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::GSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn om_set_render_targets(
        &self,
        render_target_views: &[Option<&<Self::Api as direct3d_11::Direct3D11>::RenderTargetView>],
        depth_stencil_view: Option<&<Self::Api as direct3d_11::Direct3D11>::DepthStencilView>,
    ) {
        let _ = &render_target_views;
        let _ = &depth_stencil_view;
        record("ID3D11DeviceContext::OMSetRenderTargets()".to_string());
    }
    fn om_set_render_targets_and_unordered_access_views(
        &self,
        render_target_views: Option<
            &[Option<&<Self::Api as direct3d_11::Direct3D11>::RenderTargetView>],
        >,
        depth_stencil_view: Option<&<Self::Api as direct3d_11::Direct3D11>::DepthStencilView>,
        uav_start_slot: u32,
        unordered_access_views: Option<
            &[Option<&<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView>],
        >,
        uav_initial_counts: Option<&[u32]>,
    ) {
        let _ = &render_target_views;
        let _ = &depth_stencil_view;
        let _ = &uav_start_slot;
        let _ = &unordered_access_views;
        let _ = &uav_initial_counts;
        record(format!(
            "ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews(uav_start_slot={:?})",
            uav_start_slot
        ));
    }
    fn om_set_blend_state(
        &self,
        blend_state: Option<&<Self::Api as direct3d_11::Direct3D11>::BlendState>,
        blend_factor: Option<
            &[f32; direct3d_11::constants::DEVICE_CONTEXT_OM_SET_BLEND_STATE_BLEND_FACTOR_LENGTH],
        >,
        sample_mask: u32,
    ) {
        let _ = &blend_state;
        let _ = &blend_factor;
        let _ = &sample_mask;
        record(format!(
            "ID3D11DeviceContext::OMSetBlendState(sample_mask={:?})",
            sample_mask
        ));
    }
    fn om_set_depth_stencil_state(
        &self,
        depth_stencil_state: Option<&<Self::Api as direct3d_11::Direct3D11>::DepthStencilState>,
        stencil_ref: u32,
    ) {
        let _ = &depth_stencil_state;
        let _ = &stencil_ref;
        record(format!(
            "ID3D11DeviceContext::OMSetDepthStencilState(stencil_ref={:?})",
            stencil_ref
        ));
    }
    fn so_set_targets(
        &self,
        so_targets: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
        offsets: &[u32],
    ) {
        let _ = &so_targets;
        let _ = &offsets;
        record("ID3D11DeviceContext::SOSetTargets()".to_string());
    }
    fn draw_auto(&self) {
        record("ID3D11DeviceContext::DrawAuto()".to_string());
    }
    fn draw_indexed_instanced_indirect(
        &self,
        buffer_for_args: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    ) {
        let _ = &buffer_for_args;
        let _ = &aligned_byte_offset_for_args;
        record(format!(
            "ID3D11DeviceContext::DrawIndexedInstancedIndirect(aligned_byte_offset_for_args={:?})",
            aligned_byte_offset_for_args
        ));
    }
    fn draw_instanced_indirect(
        &self,
        buffer_for_args: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    ) {
        let _ = &buffer_for_args;
        let _ = &aligned_byte_offset_for_args;
        record(format!(
            "ID3D11DeviceContext::DrawInstancedIndirect(aligned_byte_offset_for_args={:?})",
            aligned_byte_offset_for_args
        ));
    }
    fn dispatch(
        &self,
        thread_group_count_x: u32,
        thread_group_count_y: u32,
        thread_group_count_z: u32,
    ) {
        let _ = &thread_group_count_x;
        let _ = &thread_group_count_y;
        let _ = &thread_group_count_z;
        record(format!(
            "ID3D11DeviceContext::Dispatch(thread_group_count_x={:?}, thread_group_count_y={:?}, thread_group_count_z={:?})",
            thread_group_count_x, thread_group_count_y, thread_group_count_z
        ));
    }
    fn dispatch_indirect(
        &self,
        buffer_for_args: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    ) {
        let _ = &buffer_for_args;
        let _ = &aligned_byte_offset_for_args;
        record(format!(
            "ID3D11DeviceContext::DispatchIndirect(aligned_byte_offset_for_args={:?})",
            aligned_byte_offset_for_args
        ));
    }
    fn rs_set_state(
        &self,
        rasterizer_state: Option<&<Self::Api as direct3d_11::Direct3D11>::RasterizerState>,
    ) {
        let _ = &rasterizer_state;
        record("ID3D11DeviceContext::RSSetState()".to_string());
    }
    fn rs_set_viewports(&self, viewports: &[direct3d_11::structures::Viewport]) {
        let _ = &viewports;
        record("ID3D11DeviceContext::RSSetViewports()".to_string());
    }
    fn rs_set_scissor_rects(&self, rects: &[component_object_model::structures::Rect]) {
        let _ = &rects;
        record("ID3D11DeviceContext::RSSetScissorRects()".to_string());
    }
    fn copy_subresource_region(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&direct3d_11::structures::Box>,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &dst_x;
        let _ = &dst_y;
        let _ = &dst_z;
        let _ = &src_resource;
        let _ = &src_subresource;
        let _ = &src_box;
        record(format!(
            "ID3D11DeviceContext::CopySubresourceRegion(dst_subresource={:?}, dst_x={:?}, dst_y={:?}, dst_z={:?}, src_subresource={:?})",
            dst_subresource, dst_x, dst_y, dst_z, src_subresource
        ));
    }
    fn copy_resource(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        src_resource: direct3d_11::ResourceRef<'_, Self::Api>,
    ) {
        let _ = &dst_resource;
        let _ = &src_resource;
        record("ID3D11DeviceContext::CopyResource()".to_string());
    }
    fn update_subresource(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&direct3d_11::structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &dst_box;
        let _ = &src_data;
        let _ = &src_row_pitch;
        let _ = &src_depth_pitch;
        record("ID3D11DeviceContext::UpdateSubresource()".to_string());
    }
    fn copy_structure_count(
        &self,
        dst_buffer: &<Self::Api as direct3d_11::Direct3D11>::Buffer,
        dst_aligned_byte_offset: u32,
        src_view: &<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
    ) {
        let _ = &dst_buffer;
        let _ = &dst_aligned_byte_offset;
        let _ = &src_view;
        record(format!(
            "ID3D11DeviceContext::CopyStructureCount(dst_aligned_byte_offset={:?})",
            dst_aligned_byte_offset
        ));
    }
    fn clear_render_target_view(
        &self,
        render_target_view: &<Self::Api as direct3d_11::Direct3D11>::RenderTargetView,
        color_rgba: &[f32; direct3d_11::constants::DEVICE_CONTEXT_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
    ) {
        let _ = &render_target_view;
        let _ = &color_rgba;
        record("ID3D11DeviceContext::ClearRenderTargetView()".to_string());
    }
    fn clear_unordered_access_view_uint(
        &self,
        unordered_access_view: &<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
        values: &[u32; direct3d_11::constants::DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_UINT_VALUES_LENGTH],
    ) {
        let _ = &unordered_access_view;
        let _ = &values;
        record("ID3D11DeviceContext::ClearUnorderedAccessViewUint()".to_string());
    }
    fn clear_unordered_access_view_float(
        &self,
        unordered_access_view: &<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
        values: &[f32; direct3d_11::constants::DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_FLOAT_VALUES_LENGTH],
    ) {
        let _ = &unordered_access_view;
        let _ = &values;
        record("ID3D11DeviceContext::ClearUnorderedAccessViewFloat()".to_string());
    }
    fn clear_depth_stencil_view(
        &self,
        depth_stencil_view: &<Self::Api as direct3d_11::Direct3D11>::DepthStencilView,
        clear_flags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let _ = &depth_stencil_view;
        let _ = &clear_flags;
        let _ = &depth;
        let _ = &stencil;
        record(format!(
            "ID3D11DeviceContext::ClearDepthStencilView(clear_flags={:?}, depth={:?}, stencil={:?})",
            clear_flags, depth, stencil
        ));
    }
    fn generate_mips(
        &self,
        shader_resource_view: &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
    ) {
        let _ = &shader_resource_view;
        record("ID3D11DeviceContext::GenerateMips()".to_string());
    }
    fn set_resource_min_lod(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        min_lod: f32,
    ) {
        let _ = &resource;
        let _ = &min_lod;
        record(format!(
            "ID3D11DeviceContext::SetResourceMinLOD(min_lod={:?})",
            min_lod
        ));
    }
    fn get_resource_min_lod(&self, resource: direct3d_11::ResourceRef<'_, Self::Api>) -> f32 {
        let _ = &resource;
        record("ID3D11DeviceContext::GetResourceMinLOD()".to_string());
        Default::default()
    }
    fn resolve_subresource(
        &self,
        dst_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        src_resource: direct3d_11::ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) {
        let _ = &dst_resource;
        let _ = &dst_subresource;
        let _ = &src_resource;
        let _ = &src_subresource;
        let _ = &format;
        record(format!(
            "ID3D11DeviceContext::ResolveSubresource(dst_subresource={:?}, src_subresource={:?}, format={:?})",
            dst_subresource, src_subresource, format
        ));
    }
    fn execute_command_list(
        &self,
        command_list: &<Self::Api as direct3d_11::Direct3D11>::CommandList,
        restore_context_state: bool,
    ) {
        let _ = &command_list;
        let _ = &restore_context_state;
        record(format!(
            "ID3D11DeviceContext::ExecuteCommandList(restore_context_state={:?})",
            restore_context_state
        ));
    }
    fn hs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::HSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn hs_set_shader(
        &self,
        hull_shader: Option<&<Self::Api as direct3d_11::Direct3D11>::HullShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &hull_shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::HSSetShader()".to_string());
    }
    fn hs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::HSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn hs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::HSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn ds_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::DSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn ds_set_shader(
        &self,
        domain_shader: Option<&<Self::Api as direct3d_11::Direct3D11>::DomainShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &domain_shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::DSSetShader()".to_string());
    }
    fn ds_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::DSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn ds_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::DSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn cs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView,
        >],
    ) {
        let _ = &start_slot;
        let _ = &shader_resource_views;
        record(format!(
            "ID3D11DeviceContext::CSSetShaderResources(start_slot={:?})",
            start_slot
        ));
    }
    fn cs_set_unordered_access_views(
        &self,
        start_slot: u32,
        unordered_access_views: &[Option<
            &<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView,
        >],
        uav_initial_counts: &[u32],
    ) {
        let _ = &start_slot;
        let _ = &unordered_access_views;
        let _ = &uav_initial_counts;
        record(format!(
            "ID3D11DeviceContext::CSSetUnorderedAccessViews(start_slot={:?})",
            start_slot
        ));
    }
    fn cs_set_shader(
        &self,
        compute_shader: Option<&<Self::Api as direct3d_11::Direct3D11>::ComputeShader>,
        class_instances: &[Option<&<Self::Api as direct3d_11::Direct3D11>::ClassInstance>],
    ) {
        let _ = &compute_shader;
        let _ = &class_instances;
        record("ID3D11DeviceContext::CSSetShader()".to_string());
    }
    fn cs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::SamplerState>],
    ) {
        let _ = &start_slot;
        let _ = &samplers;
        record(format!(
            "ID3D11DeviceContext::CSSetSamplers(start_slot={:?})",
            start_slot
        ));
    }
    fn cs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) {
        let _ = &start_slot;
        let _ = &constant_buffers;
        record(format!(
            "ID3D11DeviceContext::CSSetConstantBuffers(start_slot={:?})",
            start_slot
        ));
    }
    fn vs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::VSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn ps_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::PSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn ps_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::PixelShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::PSGetShader()".to_string());
        (None, Vec::new())
    }
    fn ps_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::PSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn vs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::VertexShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::VSGetShader()".to_string());
        (None, Vec::new())
    }
    fn ps_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::PSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn ia_get_input_layout(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::InputLayout> {
        record("ID3D11DeviceContext::IAGetInputLayout()".to_string());
        Some(RecordedInputLayout::new())
    }
    fn ia_get_vertex_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    ) {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::IAGetVertexBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (
            (0..num_buffers.min(4))
                .map(|_| Some(RecordedBuffer::new()))
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
            (0..num_buffers.min(4))
                .map(|_| Default::default())
                .collect(),
        )
    }
    fn ia_get_index_buffer(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>,
        directx_graphics_infrastructure::enumerations::Format,
        u32,
    ) {
        record("ID3D11DeviceContext::IAGetIndexBuffer()".to_string());
        (
            Some(RecordedBuffer::new()),
            Default::default(),
            Default::default(),
        )
    }
    fn gs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::GSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn gs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::GeometryShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::GSGetShader()".to_string());
        (None, Vec::new())
    }
    fn ia_get_primitive_topology(&self) -> direct3d_common::enumerations::PrimitiveTopology {
        record("ID3D11DeviceContext::IAGetPrimitiveTopology()".to_string());
        Default::default()
    }
    fn vs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::VSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn vs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::VSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn get_predication(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::Predicate>,
        bool,
    ) {
        record("ID3D11DeviceContext::GetPredication()".to_string());
        (Some(RecordedPredicate::new()), Default::default())
    }
    fn gs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::GSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn gs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::GSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn om_get_render_targets(
        &self,
        num_views: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::RenderTargetView>>,
        Option<<Self::Api as direct3d_11::Direct3D11>::DepthStencilView>,
    ) {
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::OMGetRenderTargets(num_views={:?})",
            num_views
        ));
        (
            (0..num_views.min(4))
                .map(|_| Some(RecordedRenderTargetView::new()))
                .collect(),
            Some(RecordedDepthStencilView::new()),
        )
    }
    fn om_get_render_targets_and_unordered_access_views(
        &self,
        num_rt_vs: u32,
        uav_start_slot: u32,
        num_ua_vs: u32,
    ) -> (
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::RenderTargetView>>,
        Option<<Self::Api as direct3d_11::Direct3D11>::DepthStencilView>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView>>,
    ) {
        let _ = &num_rt_vs;
        let _ = &uav_start_slot;
        let _ = &num_ua_vs;
        record(format!(
            "ID3D11DeviceContext::OMGetRenderTargetsAndUnorderedAccessViews(num_rt_vs={:?}, uav_start_slot={:?}, num_ua_vs={:?})",
            num_rt_vs, uav_start_slot, num_ua_vs
        ));
        (
            (0..num_rt_vs.min(4))
                .map(|_| Some(RecordedRenderTargetView::new()))
                .collect(),
            Some(RecordedDepthStencilView::new()),
            (0..num_ua_vs.min(4))
                .map(|_| Some(RecordedUnorderedAccessView::new()))
                .collect(),
        )
    }
    fn om_get_blend_state(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::BlendState>,
        [f32; direct3d_11::constants::DEVICE_CONTEXT_OM_GET_BLEND_STATE_BLEND_FACTOR_LENGTH],
        u32,
    ) {
        record("ID3D11DeviceContext::OMGetBlendState()".to_string());
        (
            Some(RecordedBlendState::new()),
            core::array::from_fn(|_| Default::default()),
            Default::default(),
        )
    }
    fn om_get_depth_stencil_state(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::DepthStencilState>,
        u32,
    ) {
        record("ID3D11DeviceContext::OMGetDepthStencilState()".to_string());
        (Some(RecordedDepthStencilState::new()), Default::default())
    }
    fn so_get_targets(
        &self,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::SOGetTargets(num_buffers={:?})",
            num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn rs_get_state(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::RasterizerState> {
        record("ID3D11DeviceContext::RSGetState()".to_string());
        Some(RecordedRasterizerState::new())
    }
    fn rs_get_viewports(&self) -> Vec<direct3d_11::structures::Viewport> {
        record("ID3D11DeviceContext::RSGetViewports()".to_string());
        (0..2_u32)
            .map(|_| {
                data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_viewport(
                    &[u8::MIN; 24],
                )
            })
            .collect()
    }
    fn rs_get_scissor_rects(&self) -> Vec<component_object_model::structures::Rect> {
        record("ID3D11DeviceContext::RSGetScissorRects()".to_string());
        (0..2_u32)
            .map(|_| {
                data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(
                    &[u8::MIN; 16],
                )
            })
            .collect()
    }
    fn hs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::HSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn hs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::HullShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::HSGetShader()".to_string());
        (None, Vec::new())
    }
    fn hs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::HSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn hs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::HSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn ds_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::DSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn ds_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::DomainShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::DSGetShader()".to_string());
        (None, Vec::new())
    }
    fn ds_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::DSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn ds_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::DSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn cs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ShaderResourceView>> {
        let _ = &start_slot;
        let _ = &num_views;
        record(format!(
            "ID3D11DeviceContext::CSGetShaderResources(start_slot={:?}, num_views={:?})",
            start_slot, num_views
        ));
        (0..num_views.min(4))
            .map(|_| Some(RecordedShaderResourceView::new()))
            .collect()
    }
    fn cs_get_unordered_access_views(
        &self,
        start_slot: u32,
        num_ua_vs: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::UnorderedAccessView>> {
        let _ = &start_slot;
        let _ = &num_ua_vs;
        record(format!(
            "ID3D11DeviceContext::CSGetUnorderedAccessViews(start_slot={:?}, num_ua_vs={:?})",
            start_slot, num_ua_vs
        ));
        (0..num_ua_vs.min(4))
            .map(|_| Some(RecordedUnorderedAccessView::new()))
            .collect()
    }
    fn cs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as direct3d_11::Direct3D11>::ComputeShader>,
        Vec<Option<<Self::Api as direct3d_11::Direct3D11>::ClassInstance>>,
    ) {
        record("ID3D11DeviceContext::CSGetShader()".to_string());
        (None, Vec::new())
    }
    fn cs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::SamplerState>> {
        let _ = &start_slot;
        let _ = &num_samplers;
        record(format!(
            "ID3D11DeviceContext::CSGetSamplers(start_slot={:?}, num_samplers={:?})",
            start_slot, num_samplers
        ));
        (0..num_samplers.min(4))
            .map(|_| Some(RecordedSamplerState::new()))
            .collect()
    }
    fn cs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as direct3d_11::Direct3D11>::Buffer>> {
        let _ = &start_slot;
        let _ = &num_buffers;
        record(format!(
            "ID3D11DeviceContext::CSGetConstantBuffers(start_slot={:?}, num_buffers={:?})",
            start_slot, num_buffers
        ));
        (0..num_buffers.min(4))
            .map(|_| Some(RecordedBuffer::new()))
            .collect()
    }
    fn clear_state(&self) {
        record("ID3D11DeviceContext::ClearState()".to_string());
    }
    fn flush(&self) {
        record("ID3D11DeviceContext::Flush()".to_string());
    }
    fn get_type(&self) -> direct3d_11::enumerations::DeviceContextType {
        record("ID3D11DeviceContext::GetType()".to_string());
        Default::default()
    }
    fn get_context_flags(&self) -> u32 {
        record("ID3D11DeviceContext::GetContextFlags()".to_string());
        Default::default()
    }
    fn finish_command_list(
        &self,
        restore_deferred_context_state: bool,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::CommandList,
        component_object_model::structures::HResult,
    > {
        let _ = &restore_deferred_context_state;
        record(format!(
            "ID3D11DeviceContext::FinishCommandList(restore_deferred_context_state={:?})",
            restore_deferred_context_state
        ));
        Ok(RecordedCommandList::new())
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedDeviceContext<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_11::Multithread for RecordedDeviceContext<A> {
    fn enter(&self) {
        record("ID3D11Multithread::Enter()".to_string());
    }
    fn leave(&self) {
        record("ID3D11Multithread::Leave()".to_string());
    }
    fn set_multithread_protected(&self, b_mt_protect: bool) -> bool {
        let _ = &b_mt_protect;
        record(format!(
            "ID3D11Multithread::SetMultithreadProtected(b_mt_protect={:?})",
            b_mt_protect
        ));
        Default::default()
    }
    fn get_multithread_protected(&self) -> bool {
        record("ID3D11Multithread::GetMultithreadProtected()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::UserDefinedAnnotation for RecordedDeviceContext<A> {
    fn begin_event(&self, name: &[u16]) -> i32 {
        let _ = &name;
        record("ID3DUserDefinedAnnotation::BeginEvent()".to_string());
        Default::default()
    }
    fn end_event(&self) -> i32 {
        record("ID3DUserDefinedAnnotation::EndEvent()".to_string());
        Default::default()
    }
    fn set_marker(&self, name: &[u16]) {
        let _ = &name;
        record("ID3DUserDefinedAnnotation::SetMarker()".to_string());
    }
    fn get_status(&self) -> bool {
        record("ID3DUserDefinedAnnotation::GetStatus()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDeviceContext<A> {
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

impl<A: Recorded> direct3d_11::DomainShader for RecordedDomainShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedDomainShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDomainShader<A> {
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

impl<A: Recorded> direct3d_11::Fence for RecordedFence<A> {
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
        record("ID3D11Fence::CreateSharedHandle()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_completed_value(&self) -> u64 {
        record("ID3D11Fence::GetCompletedValue()".to_string());
        Default::default()
    }
    fn set_event_on_completion(
        &self,
        value: u64,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &value;
        let _ = &h_event;
        record(format!(
            "ID3D11Fence::SetEventOnCompletion(value={:?}, h_event={:?})",
            value, h_event
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedFence<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedFence<A> {
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

impl<A: Recorded> direct3d_11::GeometryShader for RecordedGeometryShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedGeometryShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedGeometryShader<A> {
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

impl<A: Recorded> direct3d_11::HullShader for RecordedHullShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedHullShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedHullShader<A> {
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

impl<A: Recorded> direct3d_11::InfoQueue for RecordedInfoQueue<A> {
    fn set_message_count_limit(
        &self,
        message_count_limit: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &message_count_limit;
        record(format!(
            "ID3D11InfoQueue::SetMessageCountLimit(message_count_limit={:?})",
            message_count_limit
        ));
        Ok(())
    }
    fn clear_stored_messages(&self) {
        record("ID3D11InfoQueue::ClearStoredMessages()".to_string());
    }
    fn get_message(
        &self,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &message_index;
        let _ = &message;
        record("ID3D11InfoQueue::GetMessage()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_num_messages_allowed_by_storage_filter(&self) -> u64 {
        record("ID3D11InfoQueue::GetNumMessagesAllowedByStorageFilter()".to_string());
        Default::default()
    }
    fn get_num_messages_denied_by_storage_filter(&self) -> u64 {
        record("ID3D11InfoQueue::GetNumMessagesDeniedByStorageFilter()".to_string());
        Default::default()
    }
    fn get_num_stored_messages(&self) -> u64 {
        record("ID3D11InfoQueue::GetNumStoredMessages()".to_string());
        Default::default()
    }
    fn get_num_stored_messages_allowed_by_retrieval_filter(&self) -> u64 {
        record("ID3D11InfoQueue::GetNumStoredMessagesAllowedByRetrievalFilter()".to_string());
        Default::default()
    }
    fn get_num_messages_discarded_by_message_count_limit(&self) -> u64 {
        record("ID3D11InfoQueue::GetNumMessagesDiscardedByMessageCountLimit()".to_string());
        Default::default()
    }
    fn get_message_count_limit(&self) -> u64 {
        record("ID3D11InfoQueue::GetMessageCountLimit()".to_string());
        Default::default()
    }
    fn add_storage_filter_entries(
        &self,
        filter: &direct3d_11::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::AddStorageFilterEntries()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_storage_filter(
        &self,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::GetStorageFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn clear_storage_filter(&self) {
        record("ID3D11InfoQueue::ClearStorageFilter()".to_string());
    }
    fn push_empty_storage_filter(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D11InfoQueue::PushEmptyStorageFilter()".to_string());
        Ok(())
    }
    fn push_copy_of_storage_filter(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D11InfoQueue::PushCopyOfStorageFilter()".to_string());
        Ok(())
    }
    fn push_storage_filter(
        &self,
        filter: &direct3d_11::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::PushStorageFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn pop_storage_filter(&self) {
        record("ID3D11InfoQueue::PopStorageFilter()".to_string());
    }
    fn get_storage_filter_stack_size(&self) -> u32 {
        record("ID3D11InfoQueue::GetStorageFilterStackSize()".to_string());
        Default::default()
    }
    fn add_retrieval_filter_entries(
        &self,
        filter: &direct3d_11::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::AddRetrievalFilterEntries()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_retrieval_filter(
        &self,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::GetRetrievalFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn clear_retrieval_filter(&self) {
        record("ID3D11InfoQueue::ClearRetrievalFilter()".to_string());
    }
    fn push_empty_retrieval_filter(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D11InfoQueue::PushEmptyRetrievalFilter()".to_string());
        Ok(())
    }
    fn push_copy_of_retrieval_filter(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D11InfoQueue::PushCopyOfRetrievalFilter()".to_string());
        Ok(())
    }
    fn push_retrieval_filter(
        &self,
        filter: &direct3d_11::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &filter;
        record("ID3D11InfoQueue::PushRetrievalFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn pop_retrieval_filter(&self) {
        record("ID3D11InfoQueue::PopRetrievalFilter()".to_string());
    }
    fn get_retrieval_filter_stack_size(&self) -> u32 {
        record("ID3D11InfoQueue::GetRetrievalFilterStackSize()".to_string());
        Default::default()
    }
    fn add_message(
        &self,
        category: direct3d_11::enumerations::MessageCategory,
        severity: direct3d_11::enumerations::MessageSeverity,
        id: direct3d_11::enumerations::MessageId,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &category;
        let _ = &severity;
        let _ = &id;
        let _ = &description;
        record(format!(
            "ID3D11InfoQueue::AddMessage(category={:?}, severity={:?}, id={:?})",
            category, severity, id
        ));
        Ok(())
    }
    fn add_application_message(
        &self,
        severity: direct3d_11::enumerations::MessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &severity;
        let _ = &description;
        record(format!(
            "ID3D11InfoQueue::AddApplicationMessage(severity={:?})",
            severity
        ));
        Ok(())
    }
    fn set_break_on_category(
        &self,
        category: direct3d_11::enumerations::MessageCategory,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &category;
        let _ = &b_enable;
        record(format!(
            "ID3D11InfoQueue::SetBreakOnCategory(category={:?}, b_enable={:?})",
            category, b_enable
        ));
        Ok(())
    }
    fn set_break_on_severity(
        &self,
        severity: direct3d_11::enumerations::MessageSeverity,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &severity;
        let _ = &b_enable;
        record(format!(
            "ID3D11InfoQueue::SetBreakOnSeverity(severity={:?}, b_enable={:?})",
            severity, b_enable
        ));
        Ok(())
    }
    fn set_break_on_id(
        &self,
        id: direct3d_11::enumerations::MessageId,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &id;
        let _ = &b_enable;
        record(format!(
            "ID3D11InfoQueue::SetBreakOnID(id={:?}, b_enable={:?})",
            id, b_enable
        ));
        Ok(())
    }
    fn get_break_on_category(&self, category: direct3d_11::enumerations::MessageCategory) -> bool {
        let _ = &category;
        record(format!(
            "ID3D11InfoQueue::GetBreakOnCategory(category={:?})",
            category
        ));
        Default::default()
    }
    fn get_break_on_severity(&self, severity: direct3d_11::enumerations::MessageSeverity) -> bool {
        let _ = &severity;
        record(format!(
            "ID3D11InfoQueue::GetBreakOnSeverity(severity={:?})",
            severity
        ));
        Default::default()
    }
    fn get_break_on_id(&self, id: direct3d_11::enumerations::MessageId) -> bool {
        let _ = &id;
        record(format!("ID3D11InfoQueue::GetBreakOnID(id={:?})", id));
        Default::default()
    }
    fn set_mute_debug_output(&self, b_mute: bool) {
        let _ = &b_mute;
        record(format!(
            "ID3D11InfoQueue::SetMuteDebugOutput(b_mute={:?})",
            b_mute
        ));
    }
    fn get_mute_debug_output(&self) -> bool {
        record("ID3D11InfoQueue::GetMuteDebugOutput()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedInfoQueue<A> {
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

impl<A: Recorded> direct3d_11::InputLayout for RecordedInputLayout<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedInputLayout<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedInputLayout<A> {
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

impl<A: Recorded> direct3d_11::Multithread for RecordedMultithread<A> {
    fn enter(&self) {
        record("ID3D11Multithread::Enter()".to_string());
    }
    fn leave(&self) {
        record("ID3D11Multithread::Leave()".to_string());
    }
    fn set_multithread_protected(&self, b_mt_protect: bool) -> bool {
        let _ = &b_mt_protect;
        record(format!(
            "ID3D11Multithread::SetMultithreadProtected(b_mt_protect={:?})",
            b_mt_protect
        ));
        Default::default()
    }
    fn get_multithread_protected(&self) -> bool {
        record("ID3D11Multithread::GetMultithreadProtected()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedMultithread<A> {
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

impl<A: Recorded> direct3d_11::PixelShader for RecordedPixelShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedPixelShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedPixelShader<A> {
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

impl<A: Recorded> direct3d_11::Predicate for RecordedPredicate<A> {}

impl<A: Recorded> direct3d_11::Query for RecordedPredicate<A> {
    fn get_desc(&self) -> direct3d_11::structures::QueryDesc {
        record("ID3D11Query::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_query_desc(&[u8::MIN; 8])
    }
}

impl<A: Recorded> direct3d_11::Asynchronous for RecordedPredicate<A> {
    fn get_data_size(&self) -> u32 {
        record("ID3D11Asynchronous::GetDataSize()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedPredicate<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedPredicate<A> {
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

impl<A: Recorded> direct3d_11::Query1 for RecordedQuery<A> {
    fn get_desc1(&self) -> direct3d_11::structures::QueryDesc1 {
        record("ID3D11Query1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_query_desc1(
            &[u8::MIN; 12],
        )
    }
}

impl<A: Recorded> direct3d_11::Query for RecordedQuery<A> {
    fn get_desc(&self) -> direct3d_11::structures::QueryDesc {
        record("ID3D11Query::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_query_desc(&[u8::MIN; 8])
    }
}

impl<A: Recorded> direct3d_11::Asynchronous for RecordedQuery<A> {
    fn get_data_size(&self) -> u32 {
        record("ID3D11Asynchronous::GetDataSize()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedQuery<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedQuery<A> {
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

impl<A: Recorded> direct3d_11::RasterizerState2 for RecordedRasterizerState<A> {
    fn get_desc2(&self) -> direct3d_11::structures::RasterizerDesc2 {
        record("ID3D11RasterizerState2::GetDesc2()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc2(
            &[u8::MIN; 48],
        )
    }
}

impl<A: Recorded> direct3d_11::RasterizerState1 for RecordedRasterizerState<A> {
    fn get_desc1(&self) -> direct3d_11::structures::RasterizerDesc1 {
        record("ID3D11RasterizerState1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc1(
            &[u8::MIN; 44],
        )
    }
}

impl<A: Recorded> direct3d_11::RasterizerState for RecordedRasterizerState<A> {
    fn get_desc(&self) -> direct3d_11::structures::RasterizerDesc {
        record("ID3D11RasterizerState::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_rasterizer_desc(
            &[u8::MIN; 40],
        )
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedRasterizerState<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedRasterizerState<A> {
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

impl<A: Recorded> direct3d_11::RefDefaultTrackingOptions for RecordedRefDefaultTrackingOptions<A> {
    fn set_tracking_options(
        &self,
        resource_type_flags: u32,
        options: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resource_type_flags;
        let _ = &options;
        record(format!(
            "ID3D11RefDefaultTrackingOptions::SetTrackingOptions(resource_type_flags={:?}, options={:?})",
            resource_type_flags, options
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedRefDefaultTrackingOptions<A> {
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

impl<A: Recorded> direct3d_11::RefTrackingOptions for RecordedRefTrackingOptions<A> {
    fn set_tracking_options(
        &self,
        u_options: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &u_options;
        record(format!(
            "ID3D11RefTrackingOptions::SetTrackingOptions(u_options={:?})",
            u_options
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedRefTrackingOptions<A> {
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

impl<A: Recorded> direct3d_11::RenderTargetView1 for RecordedRenderTargetView<A> {
    fn get_desc1(&self) -> direct3d_11::structures::RenderTargetViewDesc1 {
        record("ID3D11RenderTargetView1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc1(
            &[u8::MIN; 24],
        )
    }
}

impl<A: Recorded> direct3d_11::RenderTargetView for RecordedRenderTargetView<A> {
    fn get_desc(&self) -> direct3d_11::structures::RenderTargetViewDesc {
        record("ID3D11RenderTargetView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_render_target_view_desc(
            &[u8::MIN; 20],
        )
    }
}

impl<A: Recorded> direct3d_11::View for RecordedRenderTargetView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedRenderTargetView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedRenderTargetView<A> {
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

impl<A: Recorded> direct3d_11::SamplerState for RecordedSamplerState<A> {
    fn get_desc(&self) -> direct3d_11::structures::SamplerDesc {
        record("ID3D11SamplerState::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_sampler_desc(
            &[u8::MIN; 52],
        )
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedSamplerState<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedSamplerState<A> {
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

impl<A: Recorded> direct3d_11::ShaderResourceView1 for RecordedShaderResourceView<A> {
    fn get_desc1(&self) -> direct3d_11::structures::ShaderResourceViewDesc1 {
        record("ID3D11ShaderResourceView1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc1(&[u8::MIN; 28])
    }
}

impl<A: Recorded> direct3d_11::ShaderResourceView for RecordedShaderResourceView<A> {
    fn get_desc(&self) -> direct3d_11::structures::ShaderResourceViewDesc {
        record("ID3D11ShaderResourceView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_shader_resource_view_desc(
            &[u8::MIN; 24],
        )
    }
}

impl<A: Recorded> direct3d_11::View for RecordedShaderResourceView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedShaderResourceView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedShaderResourceView<A> {
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

impl<A: Recorded> direct3d_11::ShaderTrace for RecordedShaderTrace<A> {
    fn trace_ready(&self) -> Result<u64, component_object_model::structures::HResult> {
        record("ID3D11ShaderTrace::TraceReady()".to_string());
        Ok(Default::default())
    }
    fn reset_trace(&self) {
        record("ID3D11ShaderTrace::ResetTrace()".to_string());
    }
    fn get_trace_stats(
        &self,
    ) -> Result<direct3d_11::structures::TraceStats, component_object_model::structures::HResult>
    {
        record("ID3D11ShaderTrace::GetTraceStats()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn ps_select_stamp(
        &self,
        stamp_index: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &stamp_index;
        record(format!(
            "ID3D11ShaderTrace::PSSelectStamp(stamp_index={:?})",
            stamp_index
        ));
        Ok(())
    }
    fn get_initial_register_contents(
        &self,
        register: &direct3d_11::structures::TraceRegister,
    ) -> Result<direct3d_11::structures::TraceValue, component_object_model::structures::HResult>
    {
        let _ = &register;
        record("ID3D11ShaderTrace::GetInitialRegisterContents()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_step(
        &self,
        step_index: u32,
    ) -> Result<direct3d_11::structures::TraceStep, component_object_model::structures::HResult>
    {
        let _ = &step_index;
        record(format!(
            "ID3D11ShaderTrace::GetStep(step_index={:?})",
            step_index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_written_register(
        &self,
        step_index: u32,
        written_register_index: u32,
    ) -> Result<
        (
            direct3d_11::structures::TraceRegister,
            direct3d_11::structures::TraceValue,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &step_index;
        let _ = &written_register_index;
        record(format!(
            "ID3D11ShaderTrace::GetWrittenRegister(step_index={:?}, written_register_index={:?})",
            step_index, written_register_index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_read_register(
        &self,
        step_index: u32,
        read_register_index: u32,
    ) -> Result<
        (
            direct3d_11::structures::TraceRegister,
            direct3d_11::structures::TraceValue,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &step_index;
        let _ = &read_register_index;
        record(format!(
            "ID3D11ShaderTrace::GetReadRegister(step_index={:?}, read_register_index={:?})",
            step_index, read_register_index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedShaderTrace<A> {
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

impl<A: Recorded> direct3d_11::ShaderTraceFactory for RecordedShaderTraceFactory<A> {
    type Api = A;

    fn create_shader_trace(
        &self,
        shader: direct3d_11::UnknownRef<'_, Self::Api>,
        trace_desc: &direct3d_11::structures::ShaderTraceDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::ShaderTrace,
        component_object_model::structures::HResult,
    > {
        let _ = &shader;
        let _ = &trace_desc;
        record("ID3D11ShaderTraceFactory::CreateShaderTrace()".to_string());
        Ok(RecordedShaderTrace::new())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedShaderTraceFactory<A> {
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

impl<A: Recorded> direct3d_11::SwitchToRef for RecordedSwitchToRef<A> {
    fn set_use_ref(&self, use_ref: bool) -> bool {
        let _ = &use_ref;
        record(format!(
            "ID3D11SwitchToRef::SetUseRef(use_ref={:?})",
            use_ref
        ));
        Default::default()
    }
    fn get_use_ref(&self) -> bool {
        record("ID3D11SwitchToRef::GetUseRef()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedSwitchToRef<A> {
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

impl<A: Recorded> direct3d_11::Texture1D for RecordedTexture1D<A> {
    fn get_desc(&self) -> direct3d_11::structures::Texture1DDesc {
        record("ID3D11Texture1D::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_texture_1d_desc(
            &[u8::MIN; 32],
        )
    }
}

impl<A: Recorded> direct3d_11::Resource for RecordedTexture1D<A> {
    fn get_type(&self) -> direct3d_11::enumerations::ResourceDimension {
        record("ID3D11Resource::GetType()".to_string());
        Default::default()
    }
    fn set_eviction_priority(&self, eviction_priority: u32) {
        let _ = &eviction_priority;
        record(format!(
            "ID3D11Resource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
    }
    fn get_eviction_priority(&self) -> u32 {
        record("ID3D11Resource::GetEvictionPriority()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedTexture1D<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedTexture1D<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedTexture1D<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource for RecordedTexture1D<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        Ok(Default::default())
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource1 for RecordedTexture1D<A> {
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
        Ok(directx_graphics_infrastructure_recorder::RecordedSurface::new())
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
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedTexture1D<A> {
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

impl<A: Recorded> direct3d_11::Texture2D1 for RecordedTexture2D<A> {
    fn get_desc1(&self) -> direct3d_11::structures::Texture2DDesc1 {
        record("ID3D11Texture2D1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc1(
            &[u8::MIN; 48],
        )
    }
}

impl<A: Recorded> direct3d_11::Texture2D for RecordedTexture2D<A> {
    fn get_desc(&self) -> direct3d_11::structures::Texture2DDesc {
        record("ID3D11Texture2D::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_texture_2d_desc(
            &[u8::MIN; 44],
        )
    }
}

impl<A: Recorded> direct3d_11::Resource for RecordedTexture2D<A> {
    fn get_type(&self) -> direct3d_11::enumerations::ResourceDimension {
        record("ID3D11Resource::GetType()".to_string());
        Default::default()
    }
    fn set_eviction_priority(&self, eviction_priority: u32) {
        let _ = &eviction_priority;
        record(format!(
            "ID3D11Resource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
    }
    fn get_eviction_priority(&self) -> u32 {
        record("ID3D11Resource::GetEvictionPriority()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedTexture2D<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedTexture2D<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedTexture2D<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource for RecordedTexture2D<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        Ok(Default::default())
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource1 for RecordedTexture2D<A> {
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
        Ok(directx_graphics_infrastructure_recorder::RecordedSurface::new())
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
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface for RecordedTexture2D<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SurfaceDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISurface::GetDesc()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
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
        Err(E_NOTIMPL)
    }
    fn unmap(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGISurface::Unmap()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface1 for RecordedTexture2D<A> {
    fn get_dc(
        &self,
        discard: bool,
    ) -> Result<component_object_model::structures::Hdc, component_object_model::structures::HResult>
    {
        let _ = &discard;
        record(format!("IDXGISurface1::GetDC(discard={:?})", discard));
        Ok(Default::default())
    }
    fn release_dc(
        &self,
        dirty_rect: Option<&component_object_model::structures::Rect>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dirty_rect;
        record("IDXGISurface1::ReleaseDC()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Surface2 for RecordedTexture2D<A> {
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
            Default::default(),
        ))
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedTexture2D<A> {
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

impl<A: Recorded> direct3d_11::Texture3D1 for RecordedTexture3D<A> {
    fn get_desc1(&self) -> direct3d_11::structures::Texture3DDesc1 {
        record("ID3D11Texture3D1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc1(
            &[u8::MIN; 40],
        )
    }
}

impl<A: Recorded> direct3d_11::Texture3D for RecordedTexture3D<A> {
    fn get_desc(&self) -> direct3d_11::structures::Texture3DDesc {
        record("ID3D11Texture3D::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_texture_3d_desc(
            &[u8::MIN; 36],
        )
    }
}

impl<A: Recorded> direct3d_11::Resource for RecordedTexture3D<A> {
    fn get_type(&self) -> direct3d_11::enumerations::ResourceDimension {
        record("ID3D11Resource::GetType()".to_string());
        Default::default()
    }
    fn set_eviction_priority(&self, eviction_priority: u32) {
        let _ = &eviction_priority;
        record(format!(
            "ID3D11Resource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
    }
    fn get_eviction_priority(&self) -> u32 {
        record("ID3D11Resource::GetEvictionPriority()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedTexture3D<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Object for RecordedTexture3D<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::DeviceSubObject for RecordedTexture3D<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource for RecordedTexture3D<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        Ok(Default::default())
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        Ok(Default::default())
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
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Recorded> directx_graphics_infrastructure::Resource1 for RecordedTexture3D<A> {
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
        Ok(directx_graphics_infrastructure_recorder::RecordedSurface::new())
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
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedTexture3D<A> {
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

impl<A: Recorded> direct3d_11::TracingDevice for RecordedTracingDevice<A> {
    type Api = A;

    fn set_shader_tracking_options_by_type(
        &self,
        resource_type_flags: u32,
        options: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resource_type_flags;
        let _ = &options;
        record(format!(
            "ID3D11TracingDevice::SetShaderTrackingOptionsByType(resource_type_flags={:?}, options={:?})",
            resource_type_flags, options
        ));
        Ok(())
    }
    fn set_shader_tracking_options(
        &self,
        shader: direct3d_11::UnknownRef<'_, Self::Api>,
        options: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &shader;
        let _ = &options;
        record(format!(
            "ID3D11TracingDevice::SetShaderTrackingOptions(options={:?})",
            options
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedTracingDevice<A> {
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

impl<A: Recorded> direct3d_11::UnorderedAccessView1 for RecordedUnorderedAccessView<A> {
    fn get_desc1(&self) -> direct3d_11::structures::UnorderedAccessViewDesc1 {
        record("ID3D11UnorderedAccessView1::GetDesc1()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc1(&[u8::MIN; 24])
    }
}

impl<A: Recorded> direct3d_11::UnorderedAccessView for RecordedUnorderedAccessView<A> {
    fn get_desc(&self) -> direct3d_11::structures::UnorderedAccessViewDesc {
        record("ID3D11UnorderedAccessView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_unordered_access_view_desc(&[u8::MIN; 20])
    }
}

impl<A: Recorded> direct3d_11::View for RecordedUnorderedAccessView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedUnorderedAccessView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedUnorderedAccessView<A> {
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

impl<A: Recorded> direct3d_11::VertexShader for RecordedVertexShader<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVertexShader<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVertexShader<A> {
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

impl<A: Recorded> direct3d_11::VideoContext3 for RecordedVideoContext<A> {
    fn decoder_begin_frame1(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        view: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoderOutputView,
        content_key: Option<&[u8]>,
        histogram_offsets: &[u32],
        histogram_buffers: &[Option<&<Self::Api as direct3d_11::Direct3D11>::Buffer>],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &view;
        let _ = &content_key;
        let _ = &histogram_offsets;
        let _ = &histogram_buffers;
        record("ID3D11VideoContext3::DecoderBeginFrame1()".to_string());
        Ok(())
    }
    fn submit_decoder_buffers2(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        buffer_desc: &[direct3d_11::structures::VideoDecoderBufferDesc2],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &buffer_desc;
        record("ID3D11VideoContext3::SubmitDecoderBuffers2()".to_string());
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_11::VideoContext2 for RecordedVideoContext<A> {
    fn video_processor_set_output_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        hdr_meta_data: Option<&[u8]>,
    ) {
        let _ = &video_processor;
        let _ = &type_;
        let _ = &hdr_meta_data;
        record(format!(
            "ID3D11VideoContext2::VideoProcessorSetOutputHDRMetaData(type_={:?})",
            type_
        ));
    }
    fn video_processor_get_output_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        meta_data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> directx_graphics_infrastructure::enumerations::HdrMetadataType {
        let _ = &video_processor;
        let _ = &meta_data;
        record("ID3D11VideoContext2::VideoProcessorGetOutputHDRMetaData()".to_string());
        Default::default()
    }
    fn video_processor_set_stream_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        hdr_meta_data: Option<&[u8]>,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &type_;
        let _ = &hdr_meta_data;
        record(format!(
            "ID3D11VideoContext2::VideoProcessorSetStreamHDRMetaData(stream_index={:?}, type_={:?})",
            stream_index, type_
        ));
    }
    fn video_processor_get_stream_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        meta_data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> directx_graphics_infrastructure::enumerations::HdrMetadataType {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &meta_data;
        record(format!(
            "ID3D11VideoContext2::VideoProcessorGetStreamHDRMetaData(stream_index={:?})",
            stream_index
        ));
        Default::default()
    }
}

impl<A: Recorded> direct3d_11::VideoContext1 for RecordedVideoContext<A> {
    fn submit_decoder_buffers1(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        buffer_desc: &[direct3d_11::structures::VideoDecoderBufferDesc1],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &buffer_desc;
        record("ID3D11VideoContext1::SubmitDecoderBuffers1()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_data_for_new_hardware_key(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        privat_input_data: &[u8],
    ) -> Result<u64, component_object_model::structures::HResult> {
        let _ = &crypto_session;
        let _ = &privat_input_data;
        record("ID3D11VideoContext1::GetDataForNewHardwareKey()".to_string());
        Ok(Default::default())
    }
    fn check_crypto_session_status(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
    ) -> Result<
        direct3d_11::enumerations::CryptoSessionStatus,
        component_object_model::structures::HResult,
    > {
        let _ = &crypto_session;
        record("ID3D11VideoContext1::CheckCryptoSessionStatus()".to_string());
        Ok(Default::default())
    }
    fn decoder_enable_downsampling(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        output_desc: &direct3d_11::structures::VideoSampleDesc,
        reference_frame_count: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &input_color_space;
        let _ = &output_desc;
        let _ = &reference_frame_count;
        record(format!(
            "ID3D11VideoContext1::DecoderEnableDownsampling(input_color_space={:?}, reference_frame_count={:?})",
            input_color_space, reference_frame_count
        ));
        Ok(())
    }
    fn decoder_update_downsampling(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        output_desc: &direct3d_11::structures::VideoSampleDesc,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &output_desc;
        record("ID3D11VideoContext1::DecoderUpdateDownsampling()".to_string());
        Ok(())
    }
    fn video_processor_set_output_color_space1(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) {
        let _ = &video_processor;
        let _ = &color_space;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorSetOutputColorSpace1(color_space={:?})",
            color_space
        ));
    }
    fn video_processor_set_output_shader_usage(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        shader_usage: bool,
    ) {
        let _ = &video_processor;
        let _ = &shader_usage;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorSetOutputShaderUsage(shader_usage={:?})",
            shader_usage
        ));
    }
    fn video_processor_get_output_color_space1(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> directx_graphics_infrastructure::enumerations::ColorSpaceType {
        let _ = &video_processor;
        record("ID3D11VideoContext1::VideoProcessorGetOutputColorSpace1()".to_string());
        Default::default()
    }
    fn video_processor_get_output_shader_usage(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> bool {
        let _ = &video_processor;
        record("ID3D11VideoContext1::VideoProcessorGetOutputShaderUsage()".to_string());
        Default::default()
    }
    fn video_processor_set_stream_color_space1(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &color_space;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorSetStreamColorSpace1(stream_index={:?}, color_space={:?})",
            stream_index, color_space
        ));
    }
    fn video_processor_set_stream_mirror(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        flip_horizontal: bool,
        flip_vertical: bool,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &flip_horizontal;
        let _ = &flip_vertical;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorSetStreamMirror(stream_index={:?}, enable={:?}, flip_horizontal={:?}, flip_vertical={:?})",
            stream_index, enable, flip_horizontal, flip_vertical
        ));
    }
    fn video_processor_get_stream_color_space1(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> directx_graphics_infrastructure::enumerations::ColorSpaceType {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorGetStreamColorSpace1(stream_index={:?})",
            stream_index
        ));
        Default::default()
    }
    fn video_processor_get_stream_mirror(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, bool, bool) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorGetStreamMirror(stream_index={:?})",
            stream_index
        ));
        (Default::default(), Default::default(), Default::default())
    }
    fn video_processor_get_behavior_hints(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        output_width: u32,
        output_height: u32,
        output_format: directx_graphics_infrastructure::enumerations::Format,
        streams: &[direct3d_11::structures::VideoProcessorStreamBehaviorHint],
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &video_processor;
        let _ = &output_width;
        let _ = &output_height;
        let _ = &output_format;
        let _ = &streams;
        record(format!(
            "ID3D11VideoContext1::VideoProcessorGetBehaviorHints(output_width={:?}, output_height={:?}, output_format={:?})",
            output_width, output_height, output_format
        ));
        Ok(Default::default())
    }
}

impl<A: Recorded> direct3d_11::VideoContext for RecordedVideoContext<A> {
    fn get_decoder_buffer(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        type_: direct3d_11::enumerations::VideoDecoderBufferType,
        buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &type_;
        let _ = &buffer;
        record("ID3D11VideoContext::GetDecoderBuffer()".to_string());
        Err(E_NOTIMPL)
    }
    fn release_decoder_buffer(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        type_: direct3d_11::enumerations::VideoDecoderBufferType,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &type_;
        record(format!(
            "ID3D11VideoContext::ReleaseDecoderBuffer(type_={:?})",
            type_
        ));
        Ok(())
    }
    fn decoder_begin_frame(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        view: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoderOutputView,
        content_key: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &view;
        let _ = &content_key;
        record("ID3D11VideoContext::DecoderBeginFrame()".to_string());
        Ok(())
    }
    fn decoder_end_frame(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        record("ID3D11VideoContext::DecoderEndFrame()".to_string());
        Ok(())
    }
    fn submit_decoder_buffers(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        buffer_desc: &[direct3d_11::structures::VideoDecoderBufferDesc],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &decoder;
        let _ = &buffer_desc;
        record("ID3D11VideoContext::SubmitDecoderBuffers()".to_string());
        Err(E_NOTIMPL)
    }
    fn decoder_extension(
        &self,
        decoder: &<Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        extension_data: &direct3d_11::structures::VideoDecoderExtension<Self::Api>,
    ) -> i32 {
        let _ = &decoder;
        let _ = &extension_data;
        record("ID3D11VideoContext::DecoderExtension()".to_string());
        Default::default()
    }
    fn video_processor_set_output_target_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    ) {
        let _ = &video_processor;
        let _ = &enable;
        let _ = &rect;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetOutputTargetRect(enable={:?})",
            enable
        ));
    }
    fn video_processor_set_output_background_color(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        y_cb_cr: bool,
        color: &direct3d_11::structures::VideoColor,
    ) {
        let _ = &video_processor;
        let _ = &y_cb_cr;
        let _ = &color;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetOutputBackgroundColor(y_cb_cr={:?})",
            y_cb_cr
        ));
    }
    fn video_processor_set_output_color_space(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        color_space: &direct3d_11::structures::VideoProcessorColorSpace,
    ) {
        let _ = &video_processor;
        let _ = &color_space;
        record("ID3D11VideoContext::VideoProcessorSetOutputColorSpace()".to_string());
    }
    fn video_processor_set_output_alpha_fill_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        alpha_fill_mode: direct3d_11::enumerations::VideoProcessorAlphaFillMode,
        stream_index: u32,
    ) {
        let _ = &video_processor;
        let _ = &alpha_fill_mode;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetOutputAlphaFillMode(alpha_fill_mode={:?}, stream_index={:?})",
            alpha_fill_mode, stream_index
        ));
    }
    fn video_processor_set_output_constriction(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        enable: bool,
        size: component_object_model::structures::Size,
    ) {
        let _ = &video_processor;
        let _ = &enable;
        let _ = &size;
        record("ID3D11VideoContext::VideoProcessorSetOutputConstriction()".to_string());
    }
    fn video_processor_set_output_stereo_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        enable: bool,
    ) {
        let _ = &video_processor;
        let _ = &enable;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetOutputStereoMode(enable={:?})",
            enable
        ));
    }
    fn video_processor_set_output_extension(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        extension_guid: &component_object_model::structures::Guid,
        data_size: u32,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32 {
        let _ = &video_processor;
        let _ = &extension_guid;
        let _ = &data_size;
        let _ = &data;
        record("ID3D11VideoContext::VideoProcessorSetOutputExtension()".to_string());
        Default::default()
    }
    fn video_processor_get_output_target_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> (bool, component_object_model::structures::Rect) {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputTargetRect()".to_string());
        (
            Default::default(),
            data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(
                &[u8::MIN; 16],
            ),
        )
    }
    fn video_processor_get_output_background_color(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> (bool, direct3d_11::structures::VideoColor) {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputBackgroundColor()".to_string());
        (
            Default::default(),
            data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_color(
                &[u8::MIN; 16],
            ),
        )
    }
    fn video_processor_get_output_color_space(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> direct3d_11::structures::VideoProcessorColorSpace {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputColorSpace()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_color_space(&[u8::MIN; 4])
    }
    fn video_processor_get_output_alpha_fill_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> (direct3d_11::enumerations::VideoProcessorAlphaFillMode, u32) {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputAlphaFillMode()".to_string());
        (Default::default(), Default::default())
    }
    fn video_processor_get_output_constriction(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> (bool, component_object_model::structures::Size) {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputConstriction()".to_string());
        (
            Default::default(),
            data_direct3d_11::decode_windows_win32_foundation_size(&[u8::MIN; 8]),
        )
    }
    fn video_processor_get_output_stereo_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
    ) -> bool {
        let _ = &video_processor;
        record("ID3D11VideoContext::VideoProcessorGetOutputStereoMode()".to_string());
        Default::default()
    }
    fn video_processor_get_output_extension(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        extension_guid: &component_object_model::structures::Guid,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32 {
        let _ = &video_processor;
        let _ = &extension_guid;
        let _ = &data;
        record("ID3D11VideoContext::VideoProcessorGetOutputExtension()".to_string());
        Default::default()
    }
    fn video_processor_set_stream_frame_format(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        frame_format: direct3d_11::enumerations::VideoFrameFormat,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &frame_format;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamFrameFormat(stream_index={:?}, frame_format={:?})",
            stream_index, frame_format
        ));
    }
    fn video_processor_set_stream_color_space(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        color_space: &direct3d_11::structures::VideoProcessorColorSpace,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &color_space;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamColorSpace(stream_index={:?})",
            stream_index
        ));
    }
    fn video_processor_set_stream_output_rate(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        output_rate: direct3d_11::enumerations::VideoProcessorOutputRate,
        repeat_frame: bool,
        custom_rate: Option<&directx_graphics_infrastructure::structures::Rational>,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &output_rate;
        let _ = &repeat_frame;
        let _ = &custom_rate;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamOutputRate(stream_index={:?}, output_rate={:?}, repeat_frame={:?})",
            stream_index, output_rate, repeat_frame
        ));
    }
    fn video_processor_set_stream_source_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &rect;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamSourceRect(stream_index={:?}, enable={:?})",
            stream_index, enable
        ));
    }
    fn video_processor_set_stream_dest_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &rect;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamDestRect(stream_index={:?}, enable={:?})",
            stream_index, enable
        ));
    }
    fn video_processor_set_stream_alpha(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        alpha: f32,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &alpha;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamAlpha(stream_index={:?}, enable={:?}, alpha={:?})",
            stream_index, enable, alpha
        ));
    }
    fn video_processor_set_stream_palette(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        entries: &[u32],
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &entries;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamPalette(stream_index={:?})",
            stream_index
        ));
    }
    fn video_processor_set_stream_pixel_aspect_ratio(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        source_aspect_ratio: Option<&directx_graphics_infrastructure::structures::Rational>,
        destination_aspect_ratio: Option<&directx_graphics_infrastructure::structures::Rational>,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &source_aspect_ratio;
        let _ = &destination_aspect_ratio;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamPixelAspectRatio(stream_index={:?}, enable={:?})",
            stream_index, enable
        ));
    }
    fn video_processor_set_stream_luma_key(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        lower: f32,
        upper: f32,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &lower;
        let _ = &upper;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamLumaKey(stream_index={:?}, enable={:?}, lower={:?}, upper={:?})",
            stream_index, enable, lower, upper
        ));
    }
    fn video_processor_set_stream_stereo_format(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        format: direct3d_11::enumerations::VideoProcessorStereoFormat,
        left_view_frame0: bool,
        base_view_frame0: bool,
        flip_mode: direct3d_11::enumerations::VideoProcessorStereoFlipMode,
        mono_offset: i32,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &format;
        let _ = &left_view_frame0;
        let _ = &base_view_frame0;
        let _ = &flip_mode;
        let _ = &mono_offset;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamStereoFormat(stream_index={:?}, enable={:?}, format={:?}, left_view_frame0={:?}, base_view_frame0={:?}, flip_mode={:?}, mono_offset={:?})",
            stream_index,
            enable,
            format,
            left_view_frame0,
            base_view_frame0,
            flip_mode,
            mono_offset
        ));
    }
    fn video_processor_set_stream_auto_processing_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamAutoProcessingMode(stream_index={:?}, enable={:?})",
            stream_index, enable
        ));
    }
    fn video_processor_set_stream_filter(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        filter: direct3d_11::enumerations::VideoProcessorFilter,
        enable: bool,
        level: i32,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &filter;
        let _ = &enable;
        let _ = &level;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamFilter(stream_index={:?}, filter={:?}, enable={:?}, level={:?})",
            stream_index, filter, enable, level
        ));
    }
    fn video_processor_set_stream_extension(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        extension_guid: &component_object_model::structures::Guid,
        data_size: u32,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32 {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &extension_guid;
        let _ = &data_size;
        let _ = &data;
        record("ID3D11VideoContext::VideoProcessorSetStreamExtension()".to_string());
        Default::default()
    }
    fn video_processor_get_stream_frame_format(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> direct3d_11::enumerations::VideoFrameFormat {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamFrameFormat(stream_index={:?})",
            stream_index
        ));
        Default::default()
    }
    fn video_processor_get_stream_color_space(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> direct3d_11::structures::VideoProcessorColorSpace {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamColorSpace(stream_index={:?})",
            stream_index
        ));
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_color_space(&[u8::MIN; 4])
    }
    fn video_processor_get_stream_output_rate(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        direct3d_11::enumerations::VideoProcessorOutputRate,
        bool,
        directx_graphics_infrastructure::structures::Rational,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamOutputRate(stream_index={:?})",
            stream_index
        ));
        (Default::default(), Default::default(), data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_rational(&[u8::MIN; 8]))
    }
    fn video_processor_get_stream_source_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, component_object_model::structures::Rect) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamSourceRect(stream_index={:?})",
            stream_index
        ));
        (
            Default::default(),
            data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(
                &[u8::MIN; 16],
            ),
        )
    }
    fn video_processor_get_stream_dest_rect(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, component_object_model::structures::Rect) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamDestRect(stream_index={:?})",
            stream_index
        ));
        (
            Default::default(),
            data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(
                &[u8::MIN; 16],
            ),
        )
    }
    fn video_processor_get_stream_alpha(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, f32) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamAlpha(stream_index={:?})",
            stream_index
        ));
        (Default::default(), Default::default())
    }
    fn video_processor_get_stream_palette(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        count: u32,
    ) -> Vec<u32> {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &count;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamPalette(stream_index={:?}, count={:?})",
            stream_index, count
        ));
        (0..count.min(4)).map(|_| Default::default()).collect()
    }
    fn video_processor_get_stream_pixel_aspect_ratio(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        bool,
        directx_graphics_infrastructure::structures::Rational,
        directx_graphics_infrastructure::structures::Rational,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamPixelAspectRatio(stream_index={:?})",
            stream_index
        ));
        (Default::default(), data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_rational(&[u8::MIN; 8]), data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_rational(&[u8::MIN; 8]))
    }
    fn video_processor_get_stream_luma_key(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, f32, f32) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamLumaKey(stream_index={:?})",
            stream_index
        ));
        (Default::default(), Default::default(), Default::default())
    }
    fn video_processor_get_stream_stereo_format(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        bool,
        direct3d_11::enumerations::VideoProcessorStereoFormat,
        bool,
        bool,
        direct3d_11::enumerations::VideoProcessorStereoFlipMode,
        i32,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamStereoFormat(stream_index={:?})",
            stream_index
        ));
        (
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
    }
    fn video_processor_get_stream_auto_processing_mode(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> bool {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamAutoProcessingMode(stream_index={:?})",
            stream_index
        ));
        Default::default()
    }
    fn video_processor_get_stream_filter(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        filter: direct3d_11::enumerations::VideoProcessorFilter,
    ) -> (bool, i32) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &filter;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamFilter(stream_index={:?}, filter={:?})",
            stream_index, filter
        ));
        (Default::default(), Default::default())
    }
    fn video_processor_get_stream_extension(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        extension_guid: &component_object_model::structures::Guid,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32 {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &extension_guid;
        let _ = &data;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamExtension(stream_index={:?})",
            stream_index
        ));
        Default::default()
    }
    fn video_processor_blt(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        view: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessorOutputView,
        output_frame: u32,
        streams: &[direct3d_11::structures::VideoProcessorStream<Self::Api>],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &video_processor;
        let _ = &view;
        let _ = &output_frame;
        let _ = &streams;
        record("ID3D11VideoContext::VideoProcessorBlt()".to_string());
        Err(E_NOTIMPL)
    }
    fn negotiate_crypto_session_key_exchange(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &crypto_session;
        let _ = &data;
        record("ID3D11VideoContext::NegotiateCryptoSessionKeyExchange()".to_string());
        Ok(())
    }
    fn encryption_blt(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        src_surface: &<Self::Api as direct3d_11::Direct3D11>::Texture2D,
        dst_surface: &<Self::Api as direct3d_11::Direct3D11>::Texture2D,
        iv: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) {
        let _ = &crypto_session;
        let _ = &src_surface;
        let _ = &dst_surface;
        let _ = &iv;
        record("ID3D11VideoContext::EncryptionBlt()".to_string());
    }
    fn decryption_blt(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        src_surface: &<Self::Api as direct3d_11::Direct3D11>::Texture2D,
        dst_surface: &<Self::Api as direct3d_11::Direct3D11>::Texture2D,
        encrypted_block_info: Option<&direct3d_11::structures::EncryptedBlockInfo>,
        content_key: Option<&[u8]>,
        iv: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) {
        let _ = &crypto_session;
        let _ = &src_surface;
        let _ = &dst_surface;
        let _ = &encrypted_block_info;
        let _ = &content_key;
        let _ = &iv;
        record("ID3D11VideoContext::DecryptionBlt()".to_string());
    }
    fn start_session_key_refresh(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        random_number: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) {
        let _ = &crypto_session;
        let _ = &random_number;
        record("ID3D11VideoContext::StartSessionKeyRefresh()".to_string());
    }
    fn finish_session_key_refresh(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
    ) {
        let _ = &crypto_session;
        record("ID3D11VideoContext::FinishSessionKeyRefresh()".to_string());
    }
    fn get_encryption_blt_key(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        readback_key: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &crypto_session;
        let _ = &readback_key;
        record("ID3D11VideoContext::GetEncryptionBltKey()".to_string());
        Ok(())
    }
    fn negotiate_authenticated_channel_key_exchange(
        &self,
        channel: &<Self::Api as direct3d_11::Direct3D11>::AuthenticatedChannel,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &channel;
        let _ = &data;
        record("ID3D11VideoContext::NegotiateAuthenticatedChannelKeyExchange()".to_string());
        Ok(())
    }
    fn query_authenticated_channel(
        &self,
        channel: &<Self::Api as direct3d_11::Direct3D11>::AuthenticatedChannel,
        input: &[u8],
        output: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &channel;
        let _ = &input;
        let _ = &output;
        record("ID3D11VideoContext::QueryAuthenticatedChannel()".to_string());
        Ok(())
    }
    fn configure_authenticated_channel(
        &self,
        channel: &<Self::Api as direct3d_11::Direct3D11>::AuthenticatedChannel,
        input: &[u8],
    ) -> Result<
        direct3d_11::structures::AuthenticatedConfigureOutput,
        component_object_model::structures::HResult,
    > {
        let _ = &channel;
        let _ = &input;
        record("ID3D11VideoContext::ConfigureAuthenticatedChannel()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn video_processor_set_stream_rotation(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rotation: direct3d_11::enumerations::VideoProcessorRotation,
    ) {
        let _ = &video_processor;
        let _ = &stream_index;
        let _ = &enable;
        let _ = &rotation;
        record(format!(
            "ID3D11VideoContext::VideoProcessorSetStreamRotation(stream_index={:?}, enable={:?}, rotation={:?})",
            stream_index, enable, rotation
        ));
    }
    fn video_processor_get_stream_rotation(
        &self,
        video_processor: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, direct3d_11::enumerations::VideoProcessorRotation) {
        let _ = &video_processor;
        let _ = &stream_index;
        record(format!(
            "ID3D11VideoContext::VideoProcessorGetStreamRotation(stream_index={:?})",
            stream_index
        ));
        (Default::default(), Default::default())
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoContext<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoContext<A> {
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

impl<A: Recorded> direct3d_11::VideoDecoder for RecordedVideoDecoder<A> {
    fn get_creation_parameters(
        &self,
    ) -> Result<
        (
            direct3d_11::structures::VideoDecoderDesc,
            direct3d_11::structures::VideoDecoderConfig,
        ),
        component_object_model::structures::HResult,
    > {
        record("ID3D11VideoDecoder::GetCreationParameters()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_driver_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("ID3D11VideoDecoder::GetDriverHandle()".to_string());
        Ok(Default::default())
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoDecoder<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoDecoder<A> {
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

impl<A: Recorded> direct3d_11::VideoDecoderOutputView for RecordedVideoDecoderOutputView<A> {
    fn get_desc(&self) -> direct3d_11::structures::VideoDecoderOutputViewDesc {
        record("ID3D11VideoDecoderOutputView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_decoder_output_view_desc(&[u8::MIN; 24])
    }
}

impl<A: Recorded> direct3d_11::View for RecordedVideoDecoderOutputView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoDecoderOutputView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoDecoderOutputView<A> {
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

impl<A: Recorded> direct3d_11::VideoDevice2 for RecordedVideoDevice<A> {
    fn check_feature_support(
        &self,
        feature: direct3d_11::enumerations::FeatureVideo,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &feature;
        let _ = &feature_support_data;
        record(format!(
            "ID3D11VideoDevice2::CheckFeatureSupport(feature={:?})",
            feature
        ));
        Ok(())
    }
    fn negotiate_crypto_session_key_exchange_mt(
        &self,
        crypto_session: &<Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        flags: direct3d_11::enumerations::CryptoSessionKeyExchangeFlags,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &crypto_session;
        let _ = &flags;
        let _ = &data;
        record(format!(
            "ID3D11VideoDevice2::NegotiateCryptoSessionKeyExchangeMT(flags={:?})",
            flags
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_11::VideoDevice1 for RecordedVideoDevice<A> {
    fn get_crypto_session_private_data_size(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        key_exchange_type: &component_object_model::structures::Guid,
    ) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let _ = &crypto_type;
        let _ = &decoder_profile;
        let _ = &key_exchange_type;
        record("ID3D11VideoDevice1::GetCryptoSessionPrivateDataSize()".to_string());
        Ok((Default::default(), Default::default()))
    }
    fn get_video_decoder_caps(
        &self,
        decoder_profile: &component_object_model::structures::Guid,
        sample_width: u32,
        sample_height: u32,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
        bit_rate: u32,
        crypto_type: Option<&component_object_model::structures::Guid>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &decoder_profile;
        let _ = &sample_width;
        let _ = &sample_height;
        let _ = &frame_rate;
        let _ = &bit_rate;
        let _ = &crypto_type;
        record(format!(
            "ID3D11VideoDevice1::GetVideoDecoderCaps(sample_width={:?}, sample_height={:?}, bit_rate={:?})",
            sample_width, sample_height, bit_rate
        ));
        Ok(Default::default())
    }
    fn check_video_decoder_downsampling(
        &self,
        input_desc: &direct3d_11::structures::VideoDecoderDesc,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        input_config: &direct3d_11::structures::VideoDecoderConfig,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
        output_desc: &direct3d_11::structures::VideoSampleDesc,
    ) -> Result<(bool, bool), component_object_model::structures::HResult> {
        let _ = &input_desc;
        let _ = &input_color_space;
        let _ = &input_config;
        let _ = &frame_rate;
        let _ = &output_desc;
        record(format!(
            "ID3D11VideoDevice1::CheckVideoDecoderDownsampling(input_color_space={:?})",
            input_color_space
        ));
        Ok((Default::default(), Default::default()))
    }
    fn recommend_video_decoder_downsample_parameters(
        &self,
        input_desc: &direct3d_11::structures::VideoDecoderDesc,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        input_config: &direct3d_11::structures::VideoDecoderConfig,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
    ) -> Result<direct3d_11::structures::VideoSampleDesc, component_object_model::structures::HResult>
    {
        let _ = &input_desc;
        let _ = &input_color_space;
        let _ = &input_config;
        let _ = &frame_rate;
        record(format!(
            "ID3D11VideoDevice1::RecommendVideoDecoderDownsampleParameters(input_color_space={:?})",
            input_color_space
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_11::VideoDevice for RecordedVideoDevice<A> {
    type Api = A;

    fn create_video_decoder(
        &self,
        video_desc: &direct3d_11::structures::VideoDecoderDesc,
        config: &direct3d_11::structures::VideoDecoderConfig,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoDecoder,
        component_object_model::structures::HResult,
    > {
        let _ = &video_desc;
        let _ = &config;
        record("ID3D11VideoDevice::CreateVideoDecoder()".to_string());
        Ok(RecordedVideoDecoder::new())
    }
    fn create_video_processor(
        &self,
        enum_: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessorEnumerator,
        rate_conversion_index: u32,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoProcessor,
        component_object_model::structures::HResult,
    > {
        let _ = &enum_;
        let _ = &rate_conversion_index;
        record(format!(
            "ID3D11VideoDevice::CreateVideoProcessor(rate_conversion_index={:?})",
            rate_conversion_index
        ));
        Ok(RecordedVideoProcessor::new())
    }
    fn create_authenticated_channel(
        &self,
        channel_type: direct3d_11::enumerations::AuthenticatedChannelType,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::AuthenticatedChannel,
        component_object_model::structures::HResult,
    > {
        let _ = &channel_type;
        record(format!(
            "ID3D11VideoDevice::CreateAuthenticatedChannel(channel_type={:?})",
            channel_type
        ));
        Ok(RecordedAuthenticatedChannel::new())
    }
    fn create_crypto_session(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        key_exchange_type: &component_object_model::structures::Guid,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::CryptoSession,
        component_object_model::structures::HResult,
    > {
        let _ = &crypto_type;
        let _ = &decoder_profile;
        let _ = &key_exchange_type;
        record("ID3D11VideoDevice::CreateCryptoSession()".to_string());
        Ok(RecordedCryptoSession::new())
    }
    fn create_video_decoder_output_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        desc: &direct3d_11::structures::VideoDecoderOutputViewDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoDecoderOutputView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &desc;
        record("ID3D11VideoDevice::CreateVideoDecoderOutputView()".to_string());
        Ok(RecordedVideoDecoderOutputView::new())
    }
    fn create_video_processor_input_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        enum_: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessorEnumerator,
        desc: &direct3d_11::structures::VideoProcessorInputViewDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoProcessorInputView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &enum_;
        let _ = &desc;
        record("ID3D11VideoDevice::CreateVideoProcessorInputView()".to_string());
        Ok(RecordedVideoProcessorInputView::new())
    }
    fn create_video_processor_output_view(
        &self,
        resource: direct3d_11::ResourceRef<'_, Self::Api>,
        enum_: &<Self::Api as direct3d_11::Direct3D11>::VideoProcessorEnumerator,
        desc: &direct3d_11::structures::VideoProcessorOutputViewDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoProcessorOutputView,
        component_object_model::structures::HResult,
    > {
        let _ = &resource;
        let _ = &enum_;
        let _ = &desc;
        record("ID3D11VideoDevice::CreateVideoProcessorOutputView()".to_string());
        Ok(RecordedVideoProcessorOutputView::new())
    }
    fn create_video_processor_enumerator(
        &self,
        desc: &direct3d_11::structures::VideoProcessorContentDesc,
    ) -> Result<
        <Self::Api as direct3d_11::Direct3D11>::VideoProcessorEnumerator,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        record("ID3D11VideoDevice::CreateVideoProcessorEnumerator()".to_string());
        Ok(RecordedVideoProcessorEnumerator::new())
    }
    fn get_video_decoder_profile_count(&self) -> u32 {
        record("ID3D11VideoDevice::GetVideoDecoderProfileCount()".to_string());
        Default::default()
    }
    fn get_video_decoder_profile(
        &self,
        index: u32,
    ) -> Result<component_object_model::structures::Guid, component_object_model::structures::HResult>
    {
        let _ = &index;
        record(format!(
            "ID3D11VideoDevice::GetVideoDecoderProfile(index={:?})",
            index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn check_video_decoder_format(
        &self,
        decoder_profile: &component_object_model::structures::Guid,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<bool, component_object_model::structures::HResult> {
        let _ = &decoder_profile;
        let _ = &format;
        record(format!(
            "ID3D11VideoDevice::CheckVideoDecoderFormat(format={:?})",
            format
        ));
        Ok(Default::default())
    }
    fn get_video_decoder_config_count(
        &self,
        desc: &direct3d_11::structures::VideoDecoderDesc,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &desc;
        record("ID3D11VideoDevice::GetVideoDecoderConfigCount()".to_string());
        Ok(Default::default())
    }
    fn get_video_decoder_config(
        &self,
        desc: &direct3d_11::structures::VideoDecoderDesc,
        index: u32,
    ) -> Result<
        direct3d_11::structures::VideoDecoderConfig,
        component_object_model::structures::HResult,
    > {
        let _ = &desc;
        let _ = &index;
        record(format!(
            "ID3D11VideoDevice::GetVideoDecoderConfig(index={:?})",
            index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_content_protection_caps(
        &self,
        crypto_type: Option<&component_object_model::structures::Guid>,
        decoder_profile: Option<&component_object_model::structures::Guid>,
    ) -> Result<
        direct3d_11::structures::VideoContentProtectionCaps,
        component_object_model::structures::HResult,
    > {
        let _ = &crypto_type;
        let _ = &decoder_profile;
        record("ID3D11VideoDevice::GetContentProtectionCaps()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn check_crypto_key_exchange(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        index: u32,
    ) -> Result<component_object_model::structures::Guid, component_object_model::structures::HResult>
    {
        let _ = &crypto_type;
        let _ = &decoder_profile;
        let _ = &index;
        record(format!(
            "ID3D11VideoDevice::CheckCryptoKeyExchange(index={:?})",
            index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11VideoDevice::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11VideoDevice::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoDevice<A> {
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

impl<A: Recorded> direct3d_11::VideoProcessor for RecordedVideoProcessor<A> {
    fn get_content_desc(&self) -> direct3d_11::structures::VideoProcessorContentDesc {
        record("ID3D11VideoProcessor::GetContentDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_content_desc(&[u8::MIN; 40])
    }
    fn get_rate_conversion_caps(
        &self,
    ) -> direct3d_11::structures::VideoProcessorRateConversionCaps {
        record("ID3D11VideoProcessor::GetRateConversionCaps()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_rate_conversion_caps(&[u8::MIN; 20])
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoProcessor<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoProcessor<A> {
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

impl<A: Recorded> direct3d_11::VideoProcessorEnumerator1 for RecordedVideoProcessorEnumerator<A> {
    fn check_video_processor_format_conversion(
        &self,
        input_format: directx_graphics_infrastructure::enumerations::Format,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        output_format: directx_graphics_infrastructure::enumerations::Format,
        output_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<bool, component_object_model::structures::HResult> {
        let _ = &input_format;
        let _ = &input_color_space;
        let _ = &output_format;
        let _ = &output_color_space;
        record(format!(
            "ID3D11VideoProcessorEnumerator1::CheckVideoProcessorFormatConversion(input_format={:?}, input_color_space={:?}, output_format={:?}, output_color_space={:?})",
            input_format, input_color_space, output_format, output_color_space
        ));
        Ok(Default::default())
    }
}

impl<A: Recorded> direct3d_11::VideoProcessorEnumerator for RecordedVideoProcessorEnumerator<A> {
    fn get_video_processor_content_desc(
        &self,
    ) -> Result<
        direct3d_11::structures::VideoProcessorContentDesc,
        component_object_model::structures::HResult,
    > {
        record("ID3D11VideoProcessorEnumerator::GetVideoProcessorContentDesc()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn check_video_processor_format(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        record(format!(
            "ID3D11VideoProcessorEnumerator::CheckVideoProcessorFormat(format={:?})",
            format
        ));
        Ok(Default::default())
    }
    fn get_video_processor_caps(
        &self,
    ) -> Result<
        direct3d_11::structures::VideoProcessorCaps,
        component_object_model::structures::HResult,
    > {
        record("ID3D11VideoProcessorEnumerator::GetVideoProcessorCaps()".to_string());
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_video_processor_rate_conversion_caps(
        &self,
        type_index: u32,
    ) -> Result<
        direct3d_11::structures::VideoProcessorRateConversionCaps,
        component_object_model::structures::HResult,
    > {
        let _ = &type_index;
        record(format!(
            "ID3D11VideoProcessorEnumerator::GetVideoProcessorRateConversionCaps(type_index={:?})",
            type_index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_video_processor_custom_rate(
        &self,
        type_index: u32,
        custom_rate_index: u32,
    ) -> Result<
        direct3d_11::structures::VideoProcessorCustomRate,
        component_object_model::structures::HResult,
    > {
        let _ = &type_index;
        let _ = &custom_rate_index;
        record(format!(
            "ID3D11VideoProcessorEnumerator::GetVideoProcessorCustomRate(type_index={:?}, custom_rate_index={:?})",
            type_index, custom_rate_index
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
    fn get_video_processor_filter_range(
        &self,
        filter: direct3d_11::enumerations::VideoProcessorFilter,
    ) -> Result<
        direct3d_11::structures::VideoProcessorFilterRange,
        component_object_model::structures::HResult,
    > {
        let _ = &filter;
        record(format!(
            "ID3D11VideoProcessorEnumerator::GetVideoProcessorFilterRange(filter={:?})",
            filter
        ));
        // A structure or a bound object: the recorder makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoProcessorEnumerator<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoProcessorEnumerator<A> {
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

impl<A: Recorded> direct3d_11::VideoProcessorInputView for RecordedVideoProcessorInputView<A> {
    fn get_desc(&self) -> direct3d_11::structures::VideoProcessorInputViewDesc {
        record("ID3D11VideoProcessorInputView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_input_view_desc(&[u8::MIN; 16])
    }
}

impl<A: Recorded> direct3d_11::View for RecordedVideoProcessorInputView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoProcessorInputView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoProcessorInputView<A> {
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

impl<A: Recorded> direct3d_11::VideoProcessorOutputView for RecordedVideoProcessorOutputView<A> {
    fn get_desc(&self) -> direct3d_11::structures::VideoProcessorOutputViewDesc {
        record("ID3D11VideoProcessorOutputView::GetDesc()".to_string());
        data_direct3d_11::decode_windows_win32_graphics_direct_3d11_d_3d11_video_processor_output_view_desc(&[u8::MIN; 16])
    }
}

impl<A: Recorded> direct3d_11::View for RecordedVideoProcessorOutputView<A> {
    fn get_resource(&self) -> Option<direct3d_11::ResourceObject<Self::Api>> {
        record("ID3D11View::GetResource()".to_string());
        None
    }
}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedVideoProcessorOutputView<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedVideoProcessorOutputView<A> {
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

impl<A: Recorded> direct3d_11::DeviceContextState for RecordedDeviceContextState<A> {}

impl<A: Recorded> direct3d_11::DeviceChild for RecordedDeviceContextState<A> {
    type Api = A;

    fn get_device(&self) -> Option<<Self::Api as direct3d_11::Direct3D11>::Device> {
        record("ID3D11DeviceChild::GetDevice()".to_string());
        Some(RecordedDevice::new())
    }
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::GetPrivateData()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<direct3d_11::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &guid;
        let _ = &data;
        record("ID3D11DeviceChild::SetPrivateDataInterface()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedDeviceContextState<A> {
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

impl<A: Recorded> direct3d_11::UserDefinedAnnotation for RecordedUserDefinedAnnotation<A> {
    fn begin_event(&self, name: &[u16]) -> i32 {
        let _ = &name;
        record("ID3DUserDefinedAnnotation::BeginEvent()".to_string());
        Default::default()
    }
    fn end_event(&self) -> i32 {
        record("ID3DUserDefinedAnnotation::EndEvent()".to_string());
        Default::default()
    }
    fn set_marker(&self, name: &[u16]) {
        let _ = &name;
        record("ID3DUserDefinedAnnotation::SetMarker()".to_string());
    }
    fn get_status(&self) -> bool {
        record("ID3DUserDefinedAnnotation::GetStatus()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_common::DestructionNotifier for RecordedUserDefinedAnnotation<A> {
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

/// The recorder's library: `Direct3D11` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Recorder;

impl direct3d_11::Direct3D11 for Recorder {
    type AuthenticatedChannel = RecordedAuthenticatedChannel<Self>;
    type BlendState = RecordedBlendState<Self>;
    type Buffer = RecordedBuffer<Self>;
    type ClassInstance = RecordedClassInstance<Self>;
    type ClassLinkage = RecordedClassLinkage<Self>;
    type CommandList = RecordedCommandList<Self>;
    type ComputeShader = RecordedComputeShader<Self>;
    type Counter = RecordedCounter<Self>;
    type CryptoSession = RecordedCryptoSession<Self>;
    type Debug = RecordedDebug<Self>;
    type DepthStencilState = RecordedDepthStencilState<Self>;
    type DepthStencilView = RecordedDepthStencilView<Self>;
    type Device = RecordedDevice<Self>;
    type DeviceContext = RecordedDeviceContext<Self>;
    type DomainShader = RecordedDomainShader<Self>;
    type Fence = RecordedFence<Self>;
    type GeometryShader = RecordedGeometryShader<Self>;
    type HullShader = RecordedHullShader<Self>;
    type InfoQueue = RecordedInfoQueue<Self>;
    type InputLayout = RecordedInputLayout<Self>;
    type Multithread = RecordedMultithread<Self>;
    type PixelShader = RecordedPixelShader<Self>;
    type Predicate = RecordedPredicate<Self>;
    type Query = RecordedQuery<Self>;
    type RasterizerState = RecordedRasterizerState<Self>;
    type RefDefaultTrackingOptions = RecordedRefDefaultTrackingOptions<Self>;
    type RefTrackingOptions = RecordedRefTrackingOptions<Self>;
    type RenderTargetView = RecordedRenderTargetView<Self>;
    type SamplerState = RecordedSamplerState<Self>;
    type ShaderResourceView = RecordedShaderResourceView<Self>;
    type ShaderTrace = RecordedShaderTrace<Self>;
    type ShaderTraceFactory = RecordedShaderTraceFactory<Self>;
    type SwitchToRef = RecordedSwitchToRef<Self>;
    type Texture1D = RecordedTexture1D<Self>;
    type Texture2D = RecordedTexture2D<Self>;
    type Texture3D = RecordedTexture3D<Self>;
    type TracingDevice = RecordedTracingDevice<Self>;
    type UnorderedAccessView = RecordedUnorderedAccessView<Self>;
    type VertexShader = RecordedVertexShader<Self>;
    type VideoContext = RecordedVideoContext<Self>;
    type VideoDecoder = RecordedVideoDecoder<Self>;
    type VideoDecoderOutputView = RecordedVideoDecoderOutputView<Self>;
    type VideoDevice = RecordedVideoDevice<Self>;
    type VideoProcessor = RecordedVideoProcessor<Self>;
    type VideoProcessorEnumerator = RecordedVideoProcessorEnumerator<Self>;
    type VideoProcessorInputView = RecordedVideoProcessorInputView<Self>;
    type VideoProcessorOutputView = RecordedVideoProcessorOutputView<Self>;
    type DeviceContextState = RecordedDeviceContextState<Self>;
    type UserDefinedAnnotation = RecordedUserDefinedAnnotation<Self>;
    type Mapped = RecordedBytes;

    fn create_device(
        &self,
        adapter: Option<
            &<Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        >,
        driver_type: direct3d_common::enumerations::DriverType,
        software: component_object_model::structures::Hmodule,
        flags: direct3d_11::enumerations::CreateDeviceFlag,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
    ) -> Result<
        (
            <Self as direct3d_11::Direct3D11>::Device,
            direct3d_common::enumerations::FeatureLevel,
            <Self as direct3d_11::Direct3D11>::DeviceContext,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        let _ = &driver_type;
        let _ = &software;
        let _ = &flags;
        let _ = &feature_levels;
        let _ = &sdk_version;
        record(format!(
            "D3D11CreateDevice(driver_type={:?}, software={:?}, flags={:?}, sdk_version={:?})",
            driver_type, software, flags, sdk_version
        ));
        Ok((
            RecordedDevice::new(),
            feature_levels
                .first()
                .copied()
                .unwrap_or(direct3d_common::enumerations::FeatureLevel(45056)),
            RecordedDeviceContext::new(),
        ))
    }
    fn create_device_and_swap_chain(
        &self,
        adapter: Option<
            &<Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        >,
        driver_type: direct3d_common::enumerations::DriverType,
        software: component_object_model::structures::Hmodule,
        flags: direct3d_11::enumerations::CreateDeviceFlag,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
        swap_chain_desc: Option<&directx_graphics_infrastructure::structures::SwapChainDesc>,
    ) -> Result<
        (
            <Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
            <Self as direct3d_11::Direct3D11>::Device,
            direct3d_common::enumerations::FeatureLevel,
            <Self as direct3d_11::Direct3D11>::DeviceContext,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        let _ = &driver_type;
        let _ = &software;
        let _ = &flags;
        let _ = &feature_levels;
        let _ = &sdk_version;
        let _ = &swap_chain_desc;
        record(format!(
            "D3D11CreateDeviceAndSwapChain(driver_type={:?}, software={:?}, flags={:?}, sdk_version={:?})",
            driver_type, software, flags, sdk_version
        ));
        Ok((
            directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
            RecordedDevice::new(),
            feature_levels
                .first()
                .copied()
                .unwrap_or(direct3d_common::enumerations::FeatureLevel(45056)),
            RecordedDeviceContext::new(),
        ))
    }
}

impl Recorded for Recorder {}

impl direct3d_common::Direct3DCommon for Recorder {
    type Blob = direct3d_common_recorder::RecordedBlob<Self>;
    type DestructionNotifier = direct3d_common_recorder::RecordedDestructionNotifier<Self>;
}

impl direct3d_common_recorder::Recorded for Recorder {}

impl directx_graphics_infrastructure::DirectXGraphicsInfrastructure for Recorder {
    type Adapter = directx_graphics_infrastructure_recorder::RecordedAdapter<Self>;
    type Debug = directx_graphics_infrastructure_recorder::RecordedDebug<Self>;
    type DecodeSwapChain = directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain<Self>;
    type Device = directx_graphics_infrastructure_recorder::RecordedDevice<Self>;
    type DisplayControl = directx_graphics_infrastructure_recorder::RecordedDisplayControl<Self>;
    type Factory = directx_graphics_infrastructure_recorder::RecordedFactory<Self>;
    type FactoryMedia = directx_graphics_infrastructure_recorder::RecordedFactoryMedia<Self>;
    type InfoQueue = directx_graphics_infrastructure_recorder::RecordedInfoQueue<Self>;
    type KeyedMutex = directx_graphics_infrastructure_recorder::RecordedKeyedMutex<Self>;
    type Output = directx_graphics_infrastructure_recorder::RecordedOutput<Self>;
    type OutputDuplication =
        directx_graphics_infrastructure_recorder::RecordedOutputDuplication<Self>;
    type Resource = directx_graphics_infrastructure_recorder::RecordedResource<Self>;
    type Surface = directx_graphics_infrastructure_recorder::RecordedSurface<Self>;
    type SwapChain = directx_graphics_infrastructure_recorder::RecordedSwapChain<Self>;
    type SwapChainMedia = directx_graphics_infrastructure_recorder::RecordedSwapChainMedia<Self>;
    type GraphicsAnalysis =
        directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis<Self>;
    type Buffer = RecordedTexture2D<Self>;
    fn create_dxgi_factory(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("CreateDXGIFactory()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
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
        Ok(())
    }
    fn dxgi_disable_v_blank_virtualization(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("DXGIDisableVBlankVirtualization()".to_string());
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
                directx_graphics_infrastructure::UnknownObject::Adapter(
                    directx_graphics_infrastructure_recorder::RecordedAdapter::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(
                    directx_graphics_infrastructure_recorder::RecordedDebug::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    directx_graphics_infrastructure_recorder::RecordedDecodeSwapChain::new(),
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
                directx_graphics_infrastructure::UnknownObject::Device(
                    directx_graphics_infrastructure_recorder::RecordedDevice::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    directx_graphics_infrastructure_recorder::RecordedDisplayControl::new(),
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
                directx_graphics_infrastructure::UnknownObject::Factory(
                    directx_graphics_infrastructure_recorder::RecordedFactory::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    directx_graphics_infrastructure_recorder::RecordedFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    directx_graphics_infrastructure_recorder::RecordedInfoQueue::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    directx_graphics_infrastructure_recorder::RecordedKeyedMutex::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::Output(
                    directx_graphics_infrastructure_recorder::RecordedOutput::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    directx_graphics_infrastructure_recorder::RecordedOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(
                    directx_graphics_infrastructure_recorder::RecordedResource::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(
                    directx_graphics_infrastructure_recorder::RecordedSurface::new(),
                )
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
                directx_graphics_infrastructure::UnknownObject::SwapChain(
                    directx_graphics_infrastructure_recorder::RecordedSwapChain::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    directx_graphics_infrastructure_recorder::RecordedSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    directx_graphics_infrastructure_recorder::RecordedGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl directx_graphics_infrastructure_recorder::Recorded for Recorder {}
