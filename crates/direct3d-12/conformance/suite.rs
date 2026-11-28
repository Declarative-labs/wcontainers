//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The conformance suite of direct3d-12, written against the traits alone (src/glue_hostile.rs): an object of every kind
//! the implementation makes, every method of every kind and every export called on them, each answer checked as the
//! interface promises.

#![allow(
    unused_variables,
    unused_imports,
    dead_code,
    unreachable_code,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::redundant_closure_call,
    clippy::too_many_lines,
    clippy::unnecessary_lazy_evaluations,
    clippy::option_map_unit_fn,
    clippy::bool_assert_comparison,
    clippy::eq_op,
    clippy::format_in_format_args,
    clippy::to_string_in_format_args,
    reason = "generated: every call has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

/// What one drive did: methods called, skipped for want of an object or an argument, and calls that panicked.
#[derive(Clone, Copy, Debug, Default)]
struct Driven {
    called: usize,
    skipped: usize,
    panicked: usize,
}

impl Driven {
    fn attempt(&mut self, call: impl FnOnce() -> bool) {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
            Ok(true) => self.called += 1,
            Ok(false) => self.skipped += 1,
            Err(_) => {
                self.called += 1;
                self.panicked += 1;
            }
        }
    }
}

/// Runs a maker: its object, if any (a panic is counted by the drive, not here).
fn made<T>(make: impl FnOnce() -> Option<T>) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make))
        .ok()
        .flatten()
}

/// The caller's bytes: a buffer of ours, read and written in full.
#[derive(Debug, Default)]
struct SuiteBytes(std::sync::Mutex<Vec<u8>>);

/// How many bytes the suite lends where a method writes bytes of the caller's.
const LENT_BYTES: usize = 256;

fn bytes() -> SuiteBytes {
    SuiteBytes(std::sync::Mutex::new(vec![0; LENT_BYTES]))
}

impl wcontainers_volatile_bytes::VolatileBytes for SuiteBytes {
    fn len(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len() as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let from = usize::try_from(offset)
            .ok()
            .and_then(|offset| held.get(offset..))
            .unwrap_or_default();
        let count = into.len().min(from.len());
        into[..count].copy_from_slice(&from[..count]);
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let to = usize::try_from(offset)
            .ok()
            .and_then(|offset| held.get_mut(offset..))
            .unwrap_or_default();
        let count = bytes.len().min(to.len());
        to[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        let copy = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        let mut copy = vec![
            u8::MIN;
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        ];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

/// A method's failure is negative, as every HRESULT failure is.
fn failure_is_negative<T>(answer: Result<T, component_object_model::structures::HResult>) -> bool {
    if let Err(failure) = answer {
        assert!(
            failure.0 < 0,
            "a failure answered as {:#x}, which reads as success",
            failure.0
        );
    }
    true
}

/// An object handed out equals its clone, as COM's identity is the object's.
fn equals_its_clone<T: Clone + Eq>(object: T) -> bool {
    assert!(object.clone() == object, "an object differs from its clone");
    true
}

/// An answer the interface promises nothing more of than its type.
fn answered<T>(answer: T) -> bool {
    drop(answer);
    true
}

/// What each method both glues carry to the implementation answered, in the order called: the place and the answer's
/// every field (`Debug`), an object in it named only as one (its identity is no value); and the caller's bytes after a
/// method that writes them. The same drive made on an implementation directly and through both glues must give the same.
#[derive(Debug, Default)]
struct Answers(
    std::sync::Mutex<Vec<String>>,
    Option<Received>,
    std::sync::atomic::AtomicUsize,
);

/// The implementation's own log of the arguments it received: how many so far, and those after the first n.
type Received = (fn() -> usize, fn(usize) -> Vec<String>);

impl Answers {
    fn with_received(received: Received) -> Answers {
        Answers(
            std::sync::Mutex::default(),
            Some(received),
            std::sync::atomic::AtomicUsize::new(0),
        )
    }

    /// Marks where the implementation's log of arguments stands before a compared call.
    fn start(&self) {
        if let Some((logged, _)) = self.1 {
            self.2.store(logged(), std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn push(&self, line: String) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(line);
    }

    /// The caller's bytes after the method.
    fn bytes(&self, place: &str, bytes: &SuiteBytes) {
        let held = bytes
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        self.push(format!("{place} wrote {held:?}"));
    }

    fn len(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Notes a call that noted nothing since `noted` answers (skipped for want of an object, or panicked).
    fn not_called_since(&self, noted: usize, place: &str) {
        if self.len() == noted {
            self.push(format!("{place}: not called, or panicked"));
        }
        // What the implementation received during the call, every argument.
        if let Some((_, since)) = self.1 {
            // The same call asked again (a list's length, then the list) is one call received.
            let mut received = since(self.2.load(std::sync::atomic::Ordering::SeqCst));
            received.dedup();
            self.push(format!("{place}: the implementation received {received:?}"));
        }
    }

    fn lines(self) -> Vec<String> {
        self.0
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One object of each kind the suite made, by kind.
struct Objects<A: direct3d_12::Direct3D12> {
    api: A,
    k0: Option<<A as direct3d_common::Direct3DCommon>::Blob>,
    k1: Option<<A as direct3d_common::Direct3DCommon>::DestructionNotifier>,
    k51: Option<<A as direct3d_12::Direct3D12>::CommandAllocator>,
    k52: Option<<A as direct3d_12::Direct3D12>::CommandQueue>,
    k53: Option<<A as direct3d_12::Direct3D12>::DescriptorHeap>,
    k54: Option<<A as direct3d_12::Direct3D12>::Device>,
    k55: Option<<A as direct3d_12::Direct3D12>::Fence>,
    k56: Option<<A as direct3d_12::Direct3D12>::GraphicsCommandList>,
    k57: Option<<A as direct3d_12::Direct3D12>::PipelineState>,
    k58: Option<<A as direct3d_12::Direct3D12>::QueryHeap>,
    k59: Option<<A as direct3d_12::Direct3D12>::Resource>,
    k60: Option<<A as direct3d_12::Direct3D12>::RootSignature>,
}

impl<A: direct3d_12::Direct3D12> Objects<A> {
    fn new(api: A) -> Objects<A> {
        Objects {
            api,
            k0: None,
            k1: None,
            k51: None,
            k52: None,
            k53: None,
            k54: None,
            k55: None,
            k56: None,
            k57: None,
            k58: None,
            k59: None,
            k60: None,
        }
    }

    fn kinds_held(&self) -> usize {
        [
            self.k0.is_some(),
            self.k1.is_some(),
            self.k51.is_some(),
            self.k52.is_some(),
            self.k53.is_some(),
            self.k54.is_some(),
            self.k55.is_some(),
            self.k56.is_some(),
            self.k57.is_some(),
            self.k58.is_some(),
            self.k59.is_some(),
            self.k60.is_some(),
        ]
        .iter()
        .filter(|held| **held)
        .count()
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new: first by
/// the methods that hand out an object of their own kind, then by those too that hand out an interface by id.
fn make_all<A: direct3d_12::Direct3D12>(o: &mut Objects<A>) -> usize {
    for by_id in [false, true] {
        for _ in 0..6 {
            let before = o.kinds_held();
            // ID3D12Device::CreateCommandQueue for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_DEVICE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_FENCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_RESOURCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandQueue for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_queue(this, &direct3d_12::structures::CommandQueueDesc { type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE, priority: 65_i32, flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT, node_mask: 79_u32 }, &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandAllocator for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_command_allocator(
                            this,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommandList for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let v1 = o.k51.as_ref()?;
                        let answer = direct3d_12::Device::create_command_list(
                            this,
                            27_u32,
                            direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                            v1,
                            o.k57.as_ref(),
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_DEVICE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_FENCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_RESOURCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateDescriptorHeap for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_descriptor_heap(this, &direct3d_12::structures::DescriptorHeapDesc { type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, num_descriptors: 41_u32, flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, node_mask: 55_u32 }, &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateRootSignature for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_root_signature(
                            this,
                            27_u32,
                            &[119_u8],
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_DEVICE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_FENCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_RESOURCE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateCommittedResource for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateFence for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_fence(
                            this,
                            54_u64,
                            direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // ID3D12Device::CreateQueryHeap for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k54.as_ref()?;
                        let answer = direct3d_12::Device::create_query_heap(
                            this,
                            &direct3d_12::structures::QueryHeapDesc {
                                type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                                count: 65_u32,
                                node_mask: 72_u32,
                            },
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            // D3D12CreateDevice for CommandAllocator
            if by_id && (o.k51.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                    && o.k51.is_none()
                {
                    o.k51 = Some(p0);
                }
            }
            // D3D12CreateDevice for CommandQueue
            if by_id && (o.k52.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                    && o.k52.is_none()
                {
                    o.k52 = Some(p0);
                }
            }
            // D3D12CreateDevice for DescriptorHeap
            if by_id && (o.k53.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                    && o.k53.is_none()
                {
                    o.k53 = Some(p0);
                }
            }
            // D3D12CreateDevice for Device
            if by_id && (o.k54.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_DEVICE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                    && o.k54.is_none()
                {
                    o.k54 = Some(p0);
                }
            }
            // D3D12CreateDevice for Fence
            if by_id && (o.k55.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_FENCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                    && o.k55.is_none()
                {
                    o.k55 = Some(p0);
                }
            }
            // D3D12CreateDevice for GraphicsCommandList
            if by_id && (o.k56.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                    && o.k56.is_none()
                {
                    o.k56 = Some(p0);
                }
            }
            // D3D12CreateDevice for PipelineState
            if by_id && (o.k57.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                    && o.k57.is_none()
                {
                    o.k57 = Some(p0);
                }
            }
            // D3D12CreateDevice for QueryHeap
            if by_id && (o.k58.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                    && o.k58.is_none()
                {
                    o.k58 = Some(p0);
                }
            }
            // D3D12CreateDevice for Resource
            if by_id && (o.k59.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_RESOURCE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                    && o.k59.is_none()
                {
                    o.k59 = Some(p0);
                }
            }
            // D3D12CreateDevice for RootSignature
            if by_id && (o.k60.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let v1 = if let Some(x) = o.k51.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandAllocator(x))
                        } else if let Some(x) = o.k52.as_ref() {
                            Some(direct3d_12::UnknownRef::CommandQueue(x))
                        } else if let Some(x) = o.k53.as_ref() {
                            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
                        } else if let Some(x) = o.k54.as_ref() {
                            Some(direct3d_12::UnknownRef::Device(x))
                        } else if let Some(x) = o.k55.as_ref() {
                            Some(direct3d_12::UnknownRef::Fence(x))
                        } else if let Some(x) = o.k56.as_ref() {
                            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
                        } else if let Some(x) = o.k57.as_ref() {
                            Some(direct3d_12::UnknownRef::PipelineState(x))
                        } else if let Some(x) = o.k58.as_ref() {
                            Some(direct3d_12::UnknownRef::QueryHeap(x))
                        } else if let Some(x) = o.k59.as_ref() {
                            Some(direct3d_12::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k60.as_ref() {
                            Some(direct3d_12::UnknownRef::RootSignature(x))
                        } else {
                            None
                        };
                        let answer = direct3d_12::Direct3D12::create_device(
                            &o.api,
                            v1,
                            direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                            &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                        );
                        answer.ok()
                    })
                };
                if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                    && o.k60.is_none()
                {
                    o.k60 = Some(p0);
                }
            }
            if o.kinds_held() == before {
                break;
            }
        }
    }
    o.kinds_held()
}

/// Calls every method on every object held, and the exports, each answer checked, and notes in `answers` what each
/// method both glues carry to the implementation answered.
fn drive<A: direct3d_12::Direct3D12>(o: &Objects<A>, answers: &Answers) -> Driven {
    let mut driven = Driven::default();
    // ID3DBlob::GetBufferPointer on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_pointer(this))
    });
    // ID3DBlob::GetBufferSize on Blob
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_common::Blob::get_buffer_size(this);
            answers.push(format!(
                "ID3DBlob::GetBufferSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3DBlob::GetBufferSize on Blob");
    // ID3DDestructionNotifier::RegisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_common::DestructionNotifier::register_destruction_callback(
                this, 97_usize, &lying,
            ),
        )
    });
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                direct3d_common::DestructionNotifier::unregister_destruction_callback(this, 35_u32);
            answers.push(format!(
                "ID3DDestructionNotifier::UnregisterDestructionCallback -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier",
    );
    // ID3D12CommandAllocator::Reset on CommandAllocator
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::CommandAllocator::reset(this);
            answers.push(format!(
                "ID3D12CommandAllocator::Reset -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12CommandAllocator::Reset on CommandAllocator");
    // ID3D12CommandQueue::ExecuteCommandLists on CommandQueue
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        answered(direct3d_12::CommandQueue::execute_command_lists(
            this,
            &[None],
        ))
    });
    // ID3D12CommandQueue::Signal on CommandQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        let Some(v1) = o.k55.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::CommandQueue::signal(this, v1, 118_u64);
            answers.push(format!(
                "ID3D12CommandQueue::Signal -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12CommandQueue::Signal on CommandQueue");
    // ID3D12DescriptorHeap::GetDesc on DescriptorHeap
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::DescriptorHeap::get_desc(this);
            answers.push(format!(
                "ID3D12DescriptorHeap::GetDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12DescriptorHeap::GetDesc on DescriptorHeap");
    // ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart on DescriptorHeap
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        answered(direct3d_12::DescriptorHeap::get_cpu_descriptor_handle_for_heap_start(this))
    });
    // ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart on DescriptorHeap
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        answered(direct3d_12::DescriptorHeap::get_gpu_descriptor_handle_for_heap_start(this))
    });
    // ID3D12Device::CreateCommandQueue on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_command_queue(
                this,
                &direct3d_12::structures::CommandQueueDesc {
                    type_: direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                    priority: 65_i32,
                    flags: direct3d_12::enumerations::D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT,
                    node_mask: 79_u32,
                },
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateCommandQueue -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateCommandQueue on Device");
    // ID3D12Device::CreateCommandAllocator on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_command_allocator(
                this,
                direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateCommandAllocator -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateCommandAllocator on Device");
    // ID3D12Device::CreateCommandList on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        let Some(v1) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_command_list(
                this,
                27_u32,
                direct3d_12::enumerations::D3D12_COMMAND_LIST_TYPE_BUNDLE,
                v1,
                o.k57.as_ref(),
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateCommandList -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateCommandList on Device");
    // ID3D12Device::CreateDescriptorHeap on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_descriptor_heap(
                this,
                &direct3d_12::structures::DescriptorHeapDesc {
                    type_: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                    num_descriptors: 41_u32,
                    flags: direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                    node_mask: 55_u32,
                },
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateDescriptorHeap -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateDescriptorHeap on Device");
    // ID3D12Device::GetDescriptorHandleIncrementSize on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::Device::get_descriptor_handle_increment_size(
                this,
                direct3d_12::enumerations::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            );
            answers.push(format!(
                "ID3D12Device::GetDescriptorHandleIncrementSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12Device::GetDescriptorHandleIncrementSize on Device",
    );
    // ID3D12Device::CreateRootSignature on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_root_signature(
                this,
                27_u32,
                &[119_u8],
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateRootSignature -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateRootSignature on Device");
    // ID3D12Device::CreateShaderResourceView on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        answered(direct3d_12::Device::create_shader_resource_view(this, o.k59.as_ref(), Some(&direct3d_12::structures::ShaderResourceViewDesc { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, view_dimension: direct3d_12::enumerations::D3D12_SRV_DIMENSION_BUFFER, shader4_component_mapping: 72_u32, anonymous: direct3d_12::structures::ShaderResourceViewDescAnonymous { buffer: direct3d_12::structures::BufferSrv { first_element: 0x565554535251504f_u64, num_elements: 0x5a595857_u32, structure_byte_stride: 0x5e5d5c5b_u32, flags: direct3d_12::enumerations::BufferSrvFlags(0x6261605f_i32) }, texture_1d: direct3d_12::structures::Tex1DSrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, resource_min_lod_clamp: f32::from_bits(0x5a595857) }, texture_1d_array: direct3d_12::structures::Tex1DArraySrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, first_array_slice: 0x5a595857_u32, array_size: 0x5e5d5c5b_u32, resource_min_lod_clamp: f32::from_bits(0x6261605f) }, texture_2d: direct3d_12::structures::Tex2DSrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, plane_slice: 0x5a595857_u32, resource_min_lod_clamp: f32::from_bits(0x5e5d5c5b) }, texture_2d_array: direct3d_12::structures::Tex2DArraySrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, first_array_slice: 0x5a595857_u32, array_size: 0x5e5d5c5b_u32, plane_slice: 0x6261605f_u32, resource_min_lod_clamp: f32::from_bits(0x0) }, texture_2dms: direct3d_12::structures::Tex2DmsSrv { unused_field_nothing_to_define: 0x5251504f_u32 }, texture_2dms_array: direct3d_12::structures::Tex2DmsArraySrv { first_array_slice: 0x5251504f_u32, array_size: 0x56555453_u32 }, texture_3d: direct3d_12::structures::Tex3DSrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, resource_min_lod_clamp: f32::from_bits(0x5a595857) }, texture_cube: direct3d_12::structures::TexcubeSrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, resource_min_lod_clamp: f32::from_bits(0x5a595857) }, texture_cube_array: direct3d_12::structures::TexcubeArraySrv { most_detailed_mip: 0x5251504f_u32, mip_levels: 0x56555453_u32, first_2d_array_face: 0x5a595857_u32, num_cubes: 0x5e5d5c5b_u32, resource_min_lod_clamp: f32::from_bits(0x6261605f) }, raytracing_acceleration_structure: direct3d_12::structures::RaytracingAccelerationStructureSrv { location: 0x565554535251504f_u64 } } }), direct3d_12::structures::CpuDescriptorHandle { ptr: 106_usize }))
    });
    // ID3D12Device::CreateRenderTargetView on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        answered(direct3d_12::Device::create_render_target_view(this, o.k59.as_ref(), Some(&direct3d_12::structures::RenderTargetViewDesc { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, view_dimension: direct3d_12::enumerations::D3D12_RTV_DIMENSION_BUFFER, anonymous: direct3d_12::structures::RenderTargetViewDescAnonymous { buffer: direct3d_12::structures::BufferRtv { first_element: 0x4f4e4d4c4b4a4948_u64, num_elements: 0x53525150_u32 }, texture_1d: direct3d_12::structures::Tex1DRtv { mip_slice: 0x4b4a4948_u32 }, texture_1d_array: direct3d_12::structures::Tex1DArrayRtv { mip_slice: 0x4b4a4948_u32, first_array_slice: 0x4f4e4d4c_u32, array_size: 0x53525150_u32 }, texture_2d: direct3d_12::structures::Tex2DRtv { mip_slice: 0x4b4a4948_u32, plane_slice: 0x4f4e4d4c_u32 }, texture_2d_array: direct3d_12::structures::Tex2DArrayRtv { mip_slice: 0x4b4a4948_u32, first_array_slice: 0x4f4e4d4c_u32, array_size: 0x53525150_u32, plane_slice: 0x0_u32 }, texture_2dms: direct3d_12::structures::Tex2DmsRtv { unused_field_nothing_to_define: 0x4b4a4948_u32 }, texture_2dms_array: direct3d_12::structures::Tex2DmsArrayRtv { first_array_slice: 0x4b4a4948_u32, array_size: 0x4f4e4d4c_u32 }, texture_3d: direct3d_12::structures::Tex3DRtv { mip_slice: 0x4b4a4948_u32, first_w_slice: 0x4f4e4d4c_u32, w_size: 0x53525150_u32 } } }), direct3d_12::structures::CpuDescriptorHandle { ptr: 106_usize }))
    });
    // ID3D12Device::CreateCommittedResource on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        failure_is_negative({ let answer = direct3d_12::Device::create_committed_resource(this, &direct3d_12::structures::HeapProperties { type_: direct3d_12::enumerations::D3D12_HEAP_TYPE_DEFAULT, cpu_page_property: direct3d_12::enumerations::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, memory_pool_preference: direct3d_12::enumerations::D3D12_MEMORY_POOL_L0, creation_node_mask: 81_u32, visible_node_mask: 88_u32 }, direct3d_12::enumerations::D3D12_HEAP_FLAG_SHARED, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 65_u64, width: 72_u64, height: 79_u32, depth_or_array_size: 86_u16, mip_levels: 93_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 114_u32, quality: 121_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, direct3d_12::enumerations::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER, Some(&direct3d_12::structures::ClearValue { format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, anonymous: direct3d_12::structures::ClearValueAnonymous { color: [f32::from_bits(0x9080706), f32::from_bits(0xd0c0b0a), f32::from_bits(0x11100f0e), f32::from_bits(0x15141312)], depth_stencil: direct3d_12::structures::DepthStencilValue { depth: f32::from_bits(0x9080706), stencil: 0xa_u8 } } }), &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR); answers.push(format!("ID3D12Device::CreateCommittedResource -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateCommittedResource on Device");
    // ID3D12Device::CreateFence on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_fence(
                this,
                54_u64,
                direct3d_12::enumerations::D3D12_FENCE_FLAG_SHARED,
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateFence -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateFence on Device");
    // ID3D12Device::GetCopyableFootprints on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        answered({ let answer = direct3d_12::Device::get_copyable_footprints(this, &direct3d_12::structures::ResourceDesc { dimension: direct3d_12::enumerations::D3D12_RESOURCE_DIMENSION_BUFFER, alignment: 31_u64, width: 38_u64, height: 45_u32, depth_or_array_size: 52_u16, mip_levels: 59_u16, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 80_u32, quality: 87_u32 }, layout: direct3d_12::enumerations::D3D12_TEXTURE_LAYOUT_ROW_MAJOR, flags: direct3d_12::enumerations::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET }, 125_u32, 58_u32, 72_u64); answers.push(format!("ID3D12Device::GetCopyableFootprints -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "ID3D12Device::GetCopyableFootprints on Device");
    // ID3D12Device::CreateQueryHeap on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Device::create_query_heap(
                this,
                &direct3d_12::structures::QueryHeapDesc {
                    type_: direct3d_12::enumerations::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
                    count: 65_u32,
                    node_mask: 72_u32,
                },
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "ID3D12Device::CreateQueryHeap -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Device::CreateQueryHeap on Device");
    // ID3D12Fence::GetCompletedValue on Fence
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::Fence::get_completed_value(this);
            answers.push(format!(
                "ID3D12Fence::GetCompletedValue -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Fence::GetCompletedValue on Fence");
    // ID3D12Fence::SetEventOnCompletion on Fence
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::Fence::set_event_on_completion(
                this,
                118_u64,
                component_object_model::structures::Handle(82),
            );
            answers.push(format!(
                "ID3D12Fence::SetEventOnCompletion -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Fence::SetEventOnCompletion on Fence");
    // ID3D12GraphicsCommandList::Close on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::GraphicsCommandList::close(this);
            answers.push(format!(
                "ID3D12GraphicsCommandList::Close -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::Close on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::Reset on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = direct3d_12::GraphicsCommandList::reset(this, v1, o.k57.as_ref());
            answers.push(format!(
                "ID3D12GraphicsCommandList::Reset -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::Reset on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::DrawInstanced on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::draw_instanced(
                this, 77_u32, 17_u32, 95_u32, 32_u32,
            );
            answers.push("ID3D12GraphicsCommandList::DrawInstanced -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::DrawInstanced on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::DrawIndexedInstanced on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::draw_indexed_instanced(
                this, 75_u32, 17_u32, 81_u32, 38_i32, 32_u32,
            );
            answers.push("ID3D12GraphicsCommandList::DrawIndexedInstanced -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::DrawIndexedInstanced on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::Dispatch on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::dispatch(this, 105_u32, 63_u32, 122_u32);
            answers.push("ID3D12GraphicsCommandList::Dispatch -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::Dispatch on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::CopyBufferRegion on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k59.as_ref() else {
            return false;
        };
        let Some(v2) = o.k59.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::copy_buffer_region(
                this, v1, 111_u64, v2, 1_u64, 22_u64,
            );
            answers.push("ID3D12GraphicsCommandList::CopyBufferRegion -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::CopyBufferRegion on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::IASetPrimitiveTopology on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::ia_set_primitive_topology(
                this,
                direct3d_common::enumerations::D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
            );
            answers
                .push("ID3D12GraphicsCommandList::IASetPrimitiveTopology -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::IASetPrimitiveTopology on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::SetPipelineState on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k57.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::set_pipeline_state(this, v1);
            answers.push("ID3D12GraphicsCommandList::SetPipelineState -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::SetPipelineState on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::ResourceBarrier on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::resource_barrier(
            this,
            &[],
        ))
    });
    // ID3D12GraphicsCommandList::SetDescriptorHeaps on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer =
                direct3d_12::GraphicsCommandList::set_descriptor_heaps(this, &[o.k53.as_ref()]);
            answers.push("ID3D12GraphicsCommandList::SetDescriptorHeaps -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::SetDescriptorHeaps on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::SetComputeRootSignature on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer =
                direct3d_12::GraphicsCommandList::set_compute_root_signature(this, o.k60.as_ref());
            answers
                .push("ID3D12GraphicsCommandList::SetComputeRootSignature -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::SetComputeRootSignature on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::SetGraphicsRootSignature on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered({
            let answer =
                direct3d_12::GraphicsCommandList::set_graphics_root_signature(this, o.k60.as_ref());
            answers
                .push("ID3D12GraphicsCommandList::SetGraphicsRootSignature -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::SetGraphicsRootSignature on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::SetGraphicsRootDescriptorTable on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            direct3d_12::GraphicsCommandList::set_graphics_root_descriptor_table(
                this,
                107_u32,
                direct3d_12::structures::GpuDescriptorHandle { ptr: 76_u64 },
            ),
        )
    });
    // ID3D12GraphicsCommandList::SetComputeRootConstantBufferView on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            direct3d_12::GraphicsCommandList::set_compute_root_constant_buffer_view(
                this, 107_u32, 6_u64,
            ),
        )
    });
    // ID3D12GraphicsCommandList::SetGraphicsRootConstantBufferView on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            direct3d_12::GraphicsCommandList::set_graphics_root_constant_buffer_view(
                this, 107_u32, 6_u64,
            ),
        )
    });
    // ID3D12GraphicsCommandList::SetComputeRootUnorderedAccessView on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            direct3d_12::GraphicsCommandList::set_compute_root_unordered_access_view(
                this, 107_u32, 6_u64,
            ),
        )
    });
    // ID3D12GraphicsCommandList::IASetIndexBuffer on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else { return false; };
        answered(direct3d_12::GraphicsCommandList::ia_set_index_buffer(this, Some(&direct3d_12::structures::IndexBufferView { buffer_location: 45_u64, size_in_bytes: 52_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS })))
    });
    // ID3D12GraphicsCommandList::IASetVertexBuffers on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::ia_set_vertex_buffers(
            this,
            41_u32,
            &[direct3d_12::structures::VertexBufferView {
                buffer_location: 11_u64,
                size_in_bytes: 18_u32,
                stride_in_bytes: 25_u32,
            }],
        ))
    });
    // ID3D12GraphicsCommandList::SOSetTargets on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::so_set_targets(
            this,
            41_u32,
            &[direct3d_12::structures::StreamOutputBufferView {
                buffer_location: 11_u64,
                size_in_bytes: 18_u64,
                buffer_filled_size_location: 25_u64,
            }],
        ))
    });
    // ID3D12GraphicsCommandList::OMSetRenderTargets on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::om_set_render_targets(
            this,
            102_u32,
            Some(&direct3d_12::structures::CpuDescriptorHandle { ptr: 8_usize }),
            true,
            Some(&direct3d_12::structures::CpuDescriptorHandle { ptr: 2_usize }),
        ))
    });
    // ID3D12GraphicsCommandList::ClearRenderTargetView on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::clear_render_target_view(
            this,
            direct3d_12::structures::CpuDescriptorHandle { ptr: 14_usize },
            &core::array::from_fn(|_| 75.5_f32),
            &[component_object_model::structures::Rect {
                left: 8_i32,
                top: 15_i32,
                right: 22_i32,
                bottom: 29_i32,
            }],
        ))
    });
    // ID3D12GraphicsCommandList::BeginQuery on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k58.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::begin_query(
                this,
                v1,
                direct3d_12::enumerations::D3D12_QUERY_TYPE_BINARY_OCCLUSION,
                35_u32,
            );
            answers.push("ID3D12GraphicsCommandList::BeginQuery -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::BeginQuery on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::EndQuery on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k58.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::end_query(
                this,
                v1,
                direct3d_12::enumerations::D3D12_QUERY_TYPE_BINARY_OCCLUSION,
                35_u32,
            );
            answers.push("ID3D12GraphicsCommandList::EndQuery -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::EndQuery on GraphicsCommandList",
    );
    // ID3D12GraphicsCommandList::ResolveQueryData on GraphicsCommandList
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k58.as_ref() else {
            return false;
        };
        let Some(v2) = o.k59.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::GraphicsCommandList::resolve_query_data(
                this,
                v1,
                direct3d_12::enumerations::D3D12_QUERY_TYPE_BINARY_OCCLUSION,
                53_u32,
                111_u32,
                v2,
                22_u64,
            );
            answers.push("ID3D12GraphicsCommandList::ResolveQueryData -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3D12GraphicsCommandList::ResolveQueryData on GraphicsCommandList",
    );
    // ID3D12Resource::Map on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_12::Resource::map(
            this,
            64_u32,
            Some(&direct3d_12::structures::Range {
                begin: 120_usize,
                end: 1_usize,
            }),
            &lying,
        ))
    });
    // ID3D12Resource::Unmap on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::Resource::unmap(
                this,
                64_u32,
                Some(&direct3d_12::structures::Range {
                    begin: 89_usize,
                    end: 96_usize,
                }),
            );
            answers.push("ID3D12Resource::Unmap -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Resource::Unmap on Resource");
    // ID3D12Resource::GetDesc on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_12::Resource::get_desc(this);
            answers.push(format!(
                "ID3D12Resource::GetDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3D12Resource::GetDesc on Resource");
    // ID3D12Resource::GetGPUVirtualAddress on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered(direct3d_12::Resource::get_gpu_virtual_address(this))
    });
    // D3D12CreateDevice
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let v1 = if let Some(x) = o.k51.as_ref() {
            Some(direct3d_12::UnknownRef::CommandAllocator(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(direct3d_12::UnknownRef::CommandQueue(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(direct3d_12::UnknownRef::DescriptorHeap(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(direct3d_12::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(direct3d_12::UnknownRef::Fence(x))
        } else if let Some(x) = o.k56.as_ref() {
            Some(direct3d_12::UnknownRef::GraphicsCommandList(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(direct3d_12::UnknownRef::PipelineState(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(direct3d_12::UnknownRef::QueryHeap(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(direct3d_12::UnknownRef::Resource(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(direct3d_12::UnknownRef::RootSignature(x))
        } else {
            None
        };
        failure_is_negative({
            let answer = direct3d_12::Direct3D12::create_device(
                &o.api,
                v1,
                direct3d_common::enumerations::D3D_FEATURE_LEVEL_1_0_GENERIC,
                &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            );
            answers.push(format!(
                "D3D12CreateDevice -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "D3D12CreateDevice");
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 53;

/// The number of answers the suite notes and compares: each answer of a method both glues carry to the
/// implementation, and the caller's bytes after each such method that writes them.
const COMPARED: usize = 34;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: direct3d_12::Direct3D12>(api: A) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o, &Answers::default());
    println!(
        "SUITE direct3d-12: kinds made {kinds}, calls {CALLS}: called {}, skipped {}, panicked {}",
        driven.called, driven.skipped, driven.panicked
    );
    assert_eq!(driven.panicked, 0, "a method panicked");
    assert_eq!(
        driven.called + driven.skipped,
        CALLS,
        "a call was neither made nor skipped"
    );
}

/// Makes an object of every kind and drives every method as `every_method_answers` does, and returns what each
/// method both glues carry to the implementation answered, in the order called.
pub fn answers_of<A: direct3d_12::Direct3D12>(api: A) -> Vec<String> {
    let mut o = Objects::new(api);
    make_all(&mut o);
    let answers = Answers::default();
    drive(&o, &answers);
    answers.lines()
}

/// Drives `api` and `reference` alike and compares every answer noted, field by field, in the order called: `api` is
/// `reference` served and called through both glues, so each answer must be what the implementation gave.
pub fn every_answer_equals_the_reference<A: direct3d_12::Direct3D12, R: direct3d_12::Direct3D12>(
    api: A,
    reference: R,
) {
    let got = answers_of(api);
    let wanted = answers_of(reference);
    println!(
        "ANSWERS direct3d-12: the reference answered {} of {COMPARED}, through both glues {}",
        wanted.len(),
        got.len()
    );
    let differing: Vec<String> = got
        .iter()
        .zip(&wanted)
        .enumerate()
        .filter(|(_, (got, wanted))| got != wanted)
        .map(|(at, (got, wanted))| {
            format!("answer {at}: through both glues {got}, the implementation {wanted}")
        })
        .collect();
    for line in &differing {
        println!("DIFFERS {line}");
    }
    assert_eq!(
        differing,
        Vec::<String>::new(),
        "answers differ through both glues"
    );
    assert_eq!(
        got.len(),
        wanted.len(),
        "through both glues, a different number of answers"
    );
    assert_eq!(
        wanted.len(),
        COMPARED,
        "the reference answered a different number of answers than the suite notes"
    );
}

/// As `answers_of`, with what the implementation received during each compared call after its answer (`logged` and
/// `since`: the implementation's own log of the arguments it received).
pub fn answers_received_of<A: direct3d_12::Direct3D12>(
    api: A,
    logged: fn() -> usize,
    since: fn(usize) -> Vec<String>,
) -> Vec<String> {
    let mut o = Objects::new(api);
    make_all(&mut o);
    let answers = Answers::with_received((logged, since));
    drive(&o, &answers);
    answers.lines()
}

/// As `every_answer_equals_the_reference`, and every argument the implementation received through both glues equals
/// what it received from the same call made directly: nothing the caller passed is dropped, zeroed or mixed up on the
/// way. A method whose call through both glues differs by design (a check of the glue's on an argument the suite
/// passes, an answer the glue gives from its own record, a bit a row clears) must be one of `refused` (src/api.rs
/// `SUITE_DIFFERENT`, each with the reason). A call the calling glue makes of its own for a method (`extra`: the method and
/// the line, src/api.rs `SUITE_EXTRA_CALLS`) is taken out of what the implementation received before the comparison.
pub fn every_answer_and_argument_equals_the_reference<
    A: direct3d_12::Direct3D12,
    R: direct3d_12::Direct3D12,
>(
    api: A,
    reference: R,
    logged: fn() -> usize,
    since: fn(usize) -> Vec<String>,
    refused: &[&str],
    extra: &[(&str, &str)],
) {
    let wanted = answers_received_of(reference, logged, since);
    let got = answers_received_of(api, logged, since);
    let mut differing = Vec::new();
    let mut accepted = Vec::new();
    for (at, (got_line, wanted_line)) in got.iter().zip(&wanted).enumerate() {
        if got_line == wanted_line {
            continue;
        }
        // The method's place: the text before " -> ", " wrote " or " on " (a line of a call not made, or of what the
        // implementation received, names the kind too).
        let place = got_line
            .split(" -> ")
            .next()
            .unwrap_or_default()
            .split(" wrote ")
            .next()
            .unwrap_or_default()
            .split(" on ")
            .next()
            .unwrap_or_default()
            .split(": ")
            .next()
            .unwrap_or_default()
            .to_string();
        let mut without_extra = got_line.clone();
        for (_, line) in extra.iter().filter(|(at, _)| *at == place) {
            for form in [
                format!(", \"{line}\""),
                format!("\"{line}\", "),
                format!("\"{line}\""),
            ] {
                without_extra = without_extra.replace(&form, "");
            }
        }
        if without_extra == *wanted_line {
            continue;
        }
        // Only what the implementation received may differ by design: every answer must be the implementation's.
        let received = got_line.contains(": the implementation received ");
        if received
            && (refused.contains(&place.as_str())
                || refused.iter().any(|entry| {
                    got_line.starts_with(entry) && got_line[entry.len()..].starts_with([' ', ':'])
                }))
        {
            accepted.push(place);
        } else {
            differing.push(format!(
                "{at}: through both glues {got_line}, directly {wanted_line}"
            ));
        }
    }
    accepted.dedup();
    println!(
        "ANSWERS AND ARGUMENTS direct3d-12: {} lines directly, {} through both glues; {} methods differing as src/api.rs SUITE_DIFFERENT states: {accepted:?}",
        wanted.len(),
        got.len(),
        accepted.len()
    );
    for line in &differing {
        println!("DIFFERS {line}");
    }
    assert_eq!(
        differing,
        Vec::<String>::new(),
        "answers or arguments differ through both glues"
    );
    assert_eq!(
        got.len(),
        wanted.len(),
        "through both glues, a different number of answers"
    );
}

/// Runs each suite function above as its own test on the value an implementation gives; given a reference, also
/// compares every answer of the value with the reference's.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn every_method_answers() {
            $crate::every_method_answers($value);
        }
    };
    ($value:expr, $reference:expr) => {
        $crate::tests!($value);

        #[test]
        fn every_answer_equals_the_reference() {
            $crate::every_answer_equals_the_reference($value, $reference);
        }
    };
    ($value:expr, $reference:expr, $logged:expr, $since:expr, $refused:expr, $extra:expr) => {
        // One test, so that no other drive adds to the implementation's log of arguments while it is read.
        #[test]
        fn every_method_answers_and_every_answer_and_argument_equals_the_reference() {
            $crate::every_method_answers($value);
            $crate::every_answer_and_argument_equals_the_reference(
                $value, $reference, $logged, $since, $refused, $extra,
            );
        }
    };
}
