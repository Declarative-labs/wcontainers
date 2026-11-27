//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! A recording implementation of direct3d-12's traits, for tests only: every call is logged by its place, with the values
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
mod data_direct3d_12;

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_direct3d_common_recorder as direct3d_common_recorder;

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

/// A recorded object of the kind `CommandAllocator`.
pub struct RecordedCommandAllocator<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedCommandAllocator<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedCommandAllocator<A> {
        RecordedCommandAllocator {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedCommandAllocator<A> {
    fn clone(&self) -> RecordedCommandAllocator<A> {
        RecordedCommandAllocator {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedCommandAllocator<A> {
    fn eq(&self, other: &RecordedCommandAllocator<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedCommandAllocator<A> {}

impl<A> Hash for RecordedCommandAllocator<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedCommandAllocator<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedCommandAllocator")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `CommandQueue`.
pub struct RecordedCommandQueue<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedCommandQueue<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedCommandQueue<A> {
        RecordedCommandQueue {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedCommandQueue<A> {
    fn clone(&self) -> RecordedCommandQueue<A> {
        RecordedCommandQueue {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedCommandQueue<A> {
    fn eq(&self, other: &RecordedCommandQueue<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedCommandQueue<A> {}

impl<A> Hash for RecordedCommandQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedCommandQueue<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedCommandQueue")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DescriptorHeap`.
pub struct RecordedDescriptorHeap<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedDescriptorHeap<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedDescriptorHeap<A> {
        RecordedDescriptorHeap {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedDescriptorHeap<A> {
    fn clone(&self) -> RecordedDescriptorHeap<A> {
        RecordedDescriptorHeap {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedDescriptorHeap<A> {
    fn eq(&self, other: &RecordedDescriptorHeap<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedDescriptorHeap<A> {}

impl<A> Hash for RecordedDescriptorHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedDescriptorHeap<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedDescriptorHeap")
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

/// A recorded object of the kind `GraphicsCommandList`.
pub struct RecordedGraphicsCommandList<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedGraphicsCommandList<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedGraphicsCommandList<A> {
        RecordedGraphicsCommandList {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedGraphicsCommandList<A> {
    fn clone(&self) -> RecordedGraphicsCommandList<A> {
        RecordedGraphicsCommandList {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedGraphicsCommandList<A> {
    fn eq(&self, other: &RecordedGraphicsCommandList<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedGraphicsCommandList<A> {}

impl<A> Hash for RecordedGraphicsCommandList<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedGraphicsCommandList<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedGraphicsCommandList")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `PipelineState`.
pub struct RecordedPipelineState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedPipelineState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedPipelineState<A> {
        RecordedPipelineState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedPipelineState<A> {
    fn clone(&self) -> RecordedPipelineState<A> {
        RecordedPipelineState {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedPipelineState<A> {
    fn eq(&self, other: &RecordedPipelineState<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedPipelineState<A> {}

impl<A> Hash for RecordedPipelineState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedPipelineState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedPipelineState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `QueryHeap`.
pub struct RecordedQueryHeap<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedQueryHeap<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedQueryHeap<A> {
        RecordedQueryHeap {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedQueryHeap<A> {
    fn clone(&self) -> RecordedQueryHeap<A> {
        RecordedQueryHeap {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedQueryHeap<A> {
    fn eq(&self, other: &RecordedQueryHeap<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedQueryHeap<A> {}

impl<A> Hash for RecordedQueryHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedQueryHeap<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedQueryHeap")
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

/// A recorded object of the kind `RootSignature`.
pub struct RecordedRootSignature<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> RecordedRootSignature<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> RecordedRootSignature<A> {
        RecordedRootSignature {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for RecordedRootSignature<A> {
    fn clone(&self) -> RecordedRootSignature<A> {
        RecordedRootSignature {
            live: Arc::clone(&self.live),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for RecordedRootSignature<A> {
    fn eq(&self, other: &RecordedRootSignature<A>) -> bool {
        self.live.0 == other.live.0
    }
}

impl<A> Eq for RecordedRootSignature<A> {}

impl<A> Hash for RecordedRootSignature<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.live.0.hash(state);
    }
}

impl<A> core::fmt::Debug for RecordedRootSignature<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("RecordedRootSignature")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this recorder's.
pub trait Recorded:
    direct3d_12::Direct3D12<
        CommandAllocator = RecordedCommandAllocator<Self>,
        CommandQueue = RecordedCommandQueue<Self>,
        DescriptorHeap = RecordedDescriptorHeap<Self>,
        Device = RecordedDevice<Self>,
        Fence = RecordedFence<Self>,
        GraphicsCommandList = RecordedGraphicsCommandList<Self>,
        PipelineState = RecordedPipelineState<Self>,
        QueryHeap = RecordedQueryHeap<Self>,
        Resource = RecordedResource<Self>,
        RootSignature = RecordedRootSignature<Self>,
    > + direct3d_common_recorder::Recorded
{
}

impl<A: Recorded> direct3d_12::CommandAllocator for RecordedCommandAllocator<A> {
    fn reset(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D12CommandAllocator::Reset()".to_string());
        Ok(())
    }
}

impl<A: Recorded> direct3d_12::Pageable for RecordedCommandAllocator<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedCommandAllocator<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedCommandAllocator<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::CommandQueue for RecordedCommandQueue<A> {
    fn execute_command_lists(
        &self,
        command_lists: &[Option<direct3d_12::CommandListRef<'_, Self::Api>>],
    ) {
        let _ = &command_lists;
        record("ID3D12CommandQueue::ExecuteCommandLists()".to_string());
    }
    fn signal(
        &self,
        fence: &<Self::Api as direct3d_12::Direct3D12>::Fence,
        value: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fence;
        let _ = &value;
        record(format!("ID3D12CommandQueue::Signal(value={:?})", value));
        Ok(())
    }
}

impl<A: Recorded> direct3d_12::Pageable for RecordedCommandQueue<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedCommandQueue<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedCommandQueue<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::DescriptorHeap for RecordedDescriptorHeap<A> {
    fn get_desc(&self) -> direct3d_12::structures::DescriptorHeapDesc {
        record("ID3D12DescriptorHeap::GetDesc()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
            &[u8::MIN; 16],
        )
    }
    fn get_cpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::CpuDescriptorHandle {
        record("ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
            &[u8::MIN; 8],
        )
    }
    fn get_gpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::GpuDescriptorHandle {
        record("ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_gpu_descriptor_handle(
            &[u8::MIN; 8],
        )
    }
}

impl<A: Recorded> direct3d_12::Pageable for RecordedDescriptorHeap<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedDescriptorHeap<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedDescriptorHeap<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::Device for RecordedDevice<A> {
    fn create_command_queue(
        &self,
        desc: &direct3d_12::structures::CommandQueueDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateCommandQueue()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_command_allocator(
        &self,
        type_: direct3d_12::enumerations::CommandListType,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &type_;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateCommandAllocator(type_={:?})",
            type_
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_graphics_pipeline_state(
        &self,
        desc: &direct3d_12::structures::GraphicsPipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateGraphicsPipelineState()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_compute_pipeline_state(
        &self,
        desc: &direct3d_12::structures::ComputePipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateComputePipelineState()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_command_list(
        &self,
        node_mask: u32,
        type_: direct3d_12::enumerations::CommandListType,
        command_allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &node_mask;
        let _ = &type_;
        let _ = &command_allocator;
        let _ = &initial_state;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateCommandList(node_mask={:?}, type_={:?})",
            node_mask, type_
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_descriptor_heap(
        &self,
        descriptor_heap_desc: &direct3d_12::structures::DescriptorHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &descriptor_heap_desc;
        let _ = &riid;
        record("ID3D12Device::CreateDescriptorHeap()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn get_descriptor_handle_increment_size(
        &self,
        descriptor_heap_type: direct3d_12::enumerations::DescriptorHeapType,
    ) -> u32 {
        let _ = &descriptor_heap_type;
        record(format!(
            "ID3D12Device::GetDescriptorHandleIncrementSize(descriptor_heap_type={:?})",
            descriptor_heap_type
        ));
        Default::default()
    }
    fn create_root_signature(
        &self,
        node_mask: u32,
        blob_with_root_signature: &[u8],
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &node_mask;
        let _ = &blob_with_root_signature;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateRootSignature(node_mask={:?})",
            node_mask
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_shader_resource_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::ShaderResourceViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = &resource;
        let _ = &desc;
        let _ = &dest_descriptor;
        record("ID3D12Device::CreateShaderResourceView()".to_string());
    }
    fn create_render_target_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::RenderTargetViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = &resource;
        let _ = &desc;
        let _ = &dest_descriptor;
        record("ID3D12Device::CreateRenderTargetView()".to_string());
    }
    fn create_committed_resource(
        &self,
        heap_properties: &direct3d_12::structures::HeapProperties,
        heap_flags: direct3d_12::enumerations::HeapFlags,
        desc: &direct3d_12::structures::ResourceDesc,
        initial_resource_state: direct3d_12::enumerations::ResourceStates,
        optimized_clear_value: Option<&direct3d_12::structures::ClearValue>,
        riid_resource: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &heap_properties;
        let _ = &heap_flags;
        let _ = &desc;
        let _ = &initial_resource_state;
        let _ = &optimized_clear_value;
        let _ = &riid_resource;
        record(format!(
            "ID3D12Device::CreateCommittedResource(heap_flags={:?}, initial_resource_state={:?})",
            heap_flags, initial_resource_state
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_fence(
        &self,
        initial_value: u64,
        flags: direct3d_12::enumerations::FenceFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &initial_value;
        let _ = &flags;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateFence(initial_value={:?}, flags={:?})",
            initial_value, flags
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn get_copyable_footprints(
        &self,
        resource_desc: &direct3d_12::structures::ResourceDesc,
        first_subresource: u32,
        num_subresources: u32,
        base_offset: u64,
    ) -> (
        Vec<direct3d_12::structures::PlacedSubresourceFootprint>,
        Vec<u32>,
        Vec<u64>,
        u64,
    ) {
        let _ = &resource_desc;
        let _ = &first_subresource;
        let _ = &num_subresources;
        let _ = &base_offset;
        record(format!(
            "ID3D12Device::GetCopyableFootprints(first_subresource={:?}, num_subresources={:?}, base_offset={:?})",
            first_subresource, num_subresources, base_offset
        ));
        ((0..num_subresources.min(4)).map(|_| data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(&[u8::MIN; 32])).collect(), (0..num_subresources.min(4)).map(|_| Default::default()).collect(), (0..num_subresources.min(4)).map(|_| Default::default()).collect(), Default::default())
    }
    fn create_query_heap(
        &self,
        desc: &direct3d_12::structures::QueryHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateQueryHeap()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Recorded> direct3d_12::Object for RecordedDevice<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::Fence for RecordedFence<A> {
    fn get_completed_value(&self) -> u64 {
        record("ID3D12Fence::GetCompletedValue()".to_string());
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
            "ID3D12Fence::SetEventOnCompletion(value={:?}, h_event={:?})",
            value, h_event
        ));
        Ok(())
    }
}

impl<A: Recorded> direct3d_12::Pageable for RecordedFence<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedFence<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedFence<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::GraphicsCommandList for RecordedGraphicsCommandList<A> {
    fn close(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D12GraphicsCommandList::Close()".to_string());
        Ok(())
    }
    fn reset(
        &self,
        allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &allocator;
        let _ = &initial_state;
        record("ID3D12GraphicsCommandList::Reset()".to_string());
        Ok(())
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
            "ID3D12GraphicsCommandList::DrawInstanced(vertex_count_per_instance={:?}, instance_count={:?}, start_vertex_location={:?}, start_instance_location={:?})",
            vertex_count_per_instance,
            instance_count,
            start_vertex_location,
            start_instance_location
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
            "ID3D12GraphicsCommandList::DrawIndexedInstanced(index_count_per_instance={:?}, instance_count={:?}, start_index_location={:?}, base_vertex_location={:?}, start_instance_location={:?})",
            index_count_per_instance,
            instance_count,
            start_index_location,
            base_vertex_location,
            start_instance_location
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
            "ID3D12GraphicsCommandList::Dispatch(thread_group_count_x={:?}, thread_group_count_y={:?}, thread_group_count_z={:?})",
            thread_group_count_x, thread_group_count_y, thread_group_count_z
        ));
    }
    fn copy_buffer_region(
        &self,
        dst_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        dst_offset: u64,
        src_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        src_offset: u64,
        num_bytes: u64,
    ) {
        let _ = &dst_buffer;
        let _ = &dst_offset;
        let _ = &src_buffer;
        let _ = &src_offset;
        let _ = &num_bytes;
        record(format!(
            "ID3D12GraphicsCommandList::CopyBufferRegion(dst_offset={:?}, src_offset={:?}, num_bytes={:?})",
            dst_offset, src_offset, num_bytes
        ));
    }
    fn copy_texture_region(
        &self,
        dst: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        src_box: Option<&direct3d_12::structures::Box>,
    ) {
        let _ = &dst;
        let _ = &dst_x;
        let _ = &dst_y;
        let _ = &dst_z;
        let _ = &src;
        let _ = &src_box;
        record("ID3D12GraphicsCommandList::CopyTextureRegion()".to_string());
    }
    fn ia_set_primitive_topology(
        &self,
        primitive_topology: direct3d_common::enumerations::PrimitiveTopology,
    ) {
        let _ = &primitive_topology;
        record(format!(
            "ID3D12GraphicsCommandList::IASetPrimitiveTopology(primitive_topology={:?})",
            primitive_topology
        ));
    }
    fn set_pipeline_state(
        &self,
        pipeline_state: &<Self::Api as direct3d_12::Direct3D12>::PipelineState,
    ) {
        let _ = &pipeline_state;
        record("ID3D12GraphicsCommandList::SetPipelineState()".to_string());
    }
    fn resource_barrier(&self, barriers: &[direct3d_12::structures::ResourceBarrier<Self::Api>]) {
        let _ = &barriers;
        record("ID3D12GraphicsCommandList::ResourceBarrier()".to_string());
    }
    fn set_descriptor_heaps(
        &self,
        descriptor_heaps: &[Option<&<Self::Api as direct3d_12::Direct3D12>::DescriptorHeap>],
    ) {
        let _ = &descriptor_heaps;
        record("ID3D12GraphicsCommandList::SetDescriptorHeaps()".to_string());
    }
    fn set_compute_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        let _ = &root_signature;
        record("ID3D12GraphicsCommandList::SetComputeRootSignature()".to_string());
    }
    fn set_graphics_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        let _ = &root_signature;
        record("ID3D12GraphicsCommandList::SetGraphicsRootSignature()".to_string());
    }
    fn set_graphics_root_descriptor_table(
        &self,
        root_parameter_index: u32,
        base_descriptor: direct3d_12::structures::GpuDescriptorHandle,
    ) {
        let _ = &root_parameter_index;
        let _ = &base_descriptor;
        record("ID3D12GraphicsCommandList::SetGraphicsRootDescriptorTable()".to_string());
    }
    fn set_compute_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetComputeRootConstantBufferView()".to_string());
    }
    fn set_graphics_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetGraphicsRootConstantBufferView()".to_string());
    }
    fn set_compute_root_unordered_access_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetComputeRootUnorderedAccessView()".to_string());
    }
    fn ia_set_index_buffer(&self, view: Option<&direct3d_12::structures::IndexBufferView>) {
        let _ = &view;
        record("ID3D12GraphicsCommandList::IASetIndexBuffer()".to_string());
    }
    fn ia_set_vertex_buffers(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::VertexBufferView],
    ) {
        let _ = &start_slot;
        let _ = &views;
        record("ID3D12GraphicsCommandList::IASetVertexBuffers()".to_string());
    }
    fn so_set_targets(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::StreamOutputBufferView],
    ) {
        let _ = &start_slot;
        let _ = &views;
        record("ID3D12GraphicsCommandList::SOSetTargets()".to_string());
    }
    fn om_set_render_targets(
        &self,
        num_render_target_descriptors: u32,
        render_target_descriptors: Option<&direct3d_12::structures::CpuDescriptorHandle>,
        r_ts_single_handle_to_descriptor_range: bool,
        depth_stencil_descriptor: Option<&direct3d_12::structures::CpuDescriptorHandle>,
    ) {
        let _ = &num_render_target_descriptors;
        let _ = &render_target_descriptors;
        let _ = &r_ts_single_handle_to_descriptor_range;
        let _ = &depth_stencil_descriptor;
        record("ID3D12GraphicsCommandList::OMSetRenderTargets()".to_string());
    }
    fn clear_render_target_view(
        &self,
        render_target_view: direct3d_12::structures::CpuDescriptorHandle,
        color_rgba: &[f32; direct3d_12::constants::GRAPHICS_COMMAND_LIST_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
        rects: &[component_object_model::structures::Rect],
    ) {
        let _ = &render_target_view;
        let _ = &color_rgba;
        let _ = &rects;
        record("ID3D12GraphicsCommandList::ClearRenderTargetView()".to_string());
    }
    fn begin_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &index;
        record(format!(
            "ID3D12GraphicsCommandList::BeginQuery(type_={:?}, index={:?})",
            type_, index
        ));
    }
    fn end_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &index;
        record(format!(
            "ID3D12GraphicsCommandList::EndQuery(type_={:?}, index={:?})",
            type_, index
        ));
    }
    fn resolve_query_data(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        start_index: u32,
        num_queries: u32,
        destination_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        aligned_destination_buffer_offset: u64,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &start_index;
        let _ = &num_queries;
        let _ = &destination_buffer;
        let _ = &aligned_destination_buffer_offset;
        record(format!(
            "ID3D12GraphicsCommandList::ResolveQueryData(type_={:?}, start_index={:?}, num_queries={:?}, aligned_destination_buffer_offset={:?})",
            type_, start_index, num_queries, aligned_destination_buffer_offset
        ));
    }
}

impl<A: Recorded> direct3d_12::CommandList for RecordedGraphicsCommandList<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedGraphicsCommandList<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedGraphicsCommandList<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::PipelineState for RecordedPipelineState<A> {}

impl<A: Recorded> direct3d_12::Pageable for RecordedPipelineState<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedPipelineState<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedPipelineState<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::QueryHeap for RecordedQueryHeap<A> {}

impl<A: Recorded> direct3d_12::Pageable for RecordedQueryHeap<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedQueryHeap<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedQueryHeap<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::Resource for RecordedResource<A> {
    fn map(
        &self,
        subresource: u32,
        read_range: Option<&direct3d_12::structures::Range>,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &subresource;
        let _ = &read_range;
        let _ = &data;
        record("ID3D12Resource::Map()".to_string());
        Err(E_NOTIMPL)
    }
    fn unmap(&self, subresource: u32, written_range: Option<&direct3d_12::structures::Range>) {
        let _ = &subresource;
        let _ = &written_range;
        record(format!(
            "ID3D12Resource::Unmap(subresource={:?})",
            subresource
        ));
    }
    fn get_desc(&self) -> direct3d_12::structures::ResourceDesc {
        record("ID3D12Resource::GetDesc()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
            &[u8::MIN; 56],
        )
    }
    fn get_gpu_virtual_address(&self) -> u64 {
        record("ID3D12Resource::GetGPUVirtualAddress()".to_string());
        Default::default()
    }
}

impl<A: Recorded> direct3d_12::Pageable for RecordedResource<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedResource<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedResource<A> {
    type Api = A;
}

impl<A: Recorded> direct3d_12::RootSignature for RecordedRootSignature<A> {}

impl<A: Recorded> direct3d_12::DeviceChild for RecordedRootSignature<A> {}

impl<A: Recorded> direct3d_12::Object for RecordedRootSignature<A> {
    type Api = A;
}

/// The recorder's library: `Direct3D12` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Recorder;

impl direct3d_12::Direct3D12 for Recorder {
    type CommandAllocator = RecordedCommandAllocator<Self>;
    type CommandQueue = RecordedCommandQueue<Self>;
    type DescriptorHeap = RecordedDescriptorHeap<Self>;
    type Device = RecordedDevice<Self>;
    type Fence = RecordedFence<Self>;
    type GraphicsCommandList = RecordedGraphicsCommandList<Self>;
    type PipelineState = RecordedPipelineState<Self>;
    type QueryHeap = RecordedQueryHeap<Self>;
    type Resource = RecordedResource<Self>;
    type RootSignature = RecordedRootSignature<Self>;

    fn create_device(
        &self,
        adapter: Option<direct3d_12::UnknownRef<'_, Self>>,
        minimum_feature_level: direct3d_common::enumerations::FeatureLevel,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self>, component_object_model::structures::HResult> {
        let _ = &adapter;
        let _ = &minimum_feature_level;
        let _ = &riid;
        record(format!(
            "D3D12CreateDevice(minimum_feature_level={:?})",
            minimum_feature_level
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(RecordedCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(RecordedCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(RecordedDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(RecordedDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(RecordedFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(RecordedGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(RecordedPipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(RecordedQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(RecordedResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(RecordedRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl Recorded for Recorder {}

impl direct3d_common::Direct3DCommon for Recorder {
    type Blob = direct3d_common_recorder::RecordedBlob<Self>;
    type DestructionNotifier = direct3d_common_recorder::RecordedDestructionNotifier<Self>;
}

impl direct3d_common_recorder::Recorded for Recorder {}
