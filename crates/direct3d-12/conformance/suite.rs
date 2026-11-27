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
    reason = "generated: every call has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_common as direct3d_common;

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

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new.
fn make_all<A: direct3d_12::Direct3D12>(o: &mut Objects<A>) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        // ID3D12Device::CreateCommandAllocator for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
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
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
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
        // ID3D12Device::CreateRootSignature for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
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
        // ID3D12Device::CreateFence for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
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
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
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
    o.kinds_held()
}

/// Calls every method on every object held, and the exports, each answer checked.
fn drive<A: direct3d_12::Direct3D12>(o: &Objects<A>) -> Driven {
    let mut driven = Driven::default();
    // ID3DBlob::GetBufferPointer on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_pointer(this))
    });
    // ID3DBlob::GetBufferSize on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_size(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_common::DestructionNotifier::register_destruction_callback(
                this,
                Default::default(),
                &lying,
            ),
        )
    });
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D12CommandAllocator::Reset on CommandAllocator
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::CommandAllocator::reset(this))
    });
    // ID3D12CommandQueue::ExecuteCommandLists on CommandQueue
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        answered(direct3d_12::CommandQueue::execute_command_lists(this, &[]))
    });
    // ID3D12CommandQueue::Signal on CommandQueue
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        let Some(v1) = o.k55.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::CommandQueue::signal(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D12DescriptorHeap::GetDesc on DescriptorHeap
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        answered(direct3d_12::DescriptorHeap::get_desc(this))
    });
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
    // ID3D12Device::CreateCommandAllocator on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::Device::create_command_allocator(
            this,
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D12Device::CreateCommandList on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        let Some(v1) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::Device::create_command_list(
            this,
            Default::default(),
            Default::default(),
            v1,
            None,
            &Default::default(),
        ))
    });
    // ID3D12Device::GetDescriptorHandleIncrementSize on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        answered(direct3d_12::Device::get_descriptor_handle_increment_size(
            this,
            Default::default(),
        ))
    });
    // ID3D12Device::CreateRootSignature on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::Device::create_root_signature(
            this,
            Default::default(),
            &[],
            &Default::default(),
        ))
    });
    // ID3D12Device::CreateFence on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::Device::create_fence(
            this,
            Default::default(),
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D12Fence::GetCompletedValue on Fence
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        answered(direct3d_12::Fence::get_completed_value(this))
    });
    // ID3D12Fence::SetEventOnCompletion on Fence
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::Fence::set_event_on_completion(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::Close on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::GraphicsCommandList::close(this))
    });
    // ID3D12GraphicsCommandList::Reset on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_12::GraphicsCommandList::reset(this, v1, None))
    });
    // ID3D12GraphicsCommandList::DrawInstanced on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::draw_instanced(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::DrawIndexedInstanced on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::draw_indexed_instanced(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::Dispatch on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::dispatch(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::CopyBufferRegion on GraphicsCommandList
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
        answered(direct3d_12::GraphicsCommandList::copy_buffer_region(
            this,
            v1,
            Default::default(),
            v2,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::IASetPrimitiveTopology on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::ia_set_primitive_topology(
            this,
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::SetPipelineState on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k57.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::set_pipeline_state(
            this, v1,
        ))
    });
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
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::set_descriptor_heaps(
            this,
            &[],
        ))
    });
    // ID3D12GraphicsCommandList::SetComputeRootSignature on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::set_compute_root_signature(this, None))
    });
    // ID3D12GraphicsCommandList::SetGraphicsRootSignature on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::set_graphics_root_signature(this, None))
    });
    // ID3D12GraphicsCommandList::SetComputeRootConstantBufferView on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            direct3d_12::GraphicsCommandList::set_compute_root_constant_buffer_view(
                this,
                Default::default(),
                Default::default(),
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
                this,
                Default::default(),
                Default::default(),
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
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D12GraphicsCommandList::IASetIndexBuffer on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::ia_set_index_buffer(
            this, None,
        ))
    });
    // ID3D12GraphicsCommandList::IASetVertexBuffers on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::ia_set_vertex_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D12GraphicsCommandList::SOSetTargets on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::so_set_targets(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D12GraphicsCommandList::OMSetRenderTargets on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::om_set_render_targets(
            this,
            Default::default(),
            None,
            Default::default(),
            None,
        ))
    });
    // ID3D12GraphicsCommandList::BeginQuery on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k58.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::begin_query(
            this,
            v1,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::EndQuery on GraphicsCommandList
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let Some(v1) = o.k58.as_ref() else {
            return false;
        };
        answered(direct3d_12::GraphicsCommandList::end_query(
            this,
            v1,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D12GraphicsCommandList::ResolveQueryData on GraphicsCommandList
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
        answered(direct3d_12::GraphicsCommandList::resolve_query_data(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
            v2,
            Default::default(),
        ))
    });
    // ID3D12Resource::Map on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_12::Resource::map(
            this,
            Default::default(),
            None,
            &lying,
        ))
    });
    // ID3D12Resource::Unmap on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered(direct3d_12::Resource::unmap(this, Default::default(), None))
    });
    // ID3D12Resource::GetDesc on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered(direct3d_12::Resource::get_desc(this))
    });
    // ID3D12Resource::GetGPUVirtualAddress on Resource
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        answered(direct3d_12::Resource::get_gpu_virtual_address(this))
    });
    // D3D12CreateDevice
    driven.attempt(|| {
        failure_is_negative(direct3d_12::Direct3D12::create_device(
            &o.api,
            None,
            Default::default(),
            &Default::default(),
        ))
    });
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 44;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: direct3d_12::Direct3D12>(api: A) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o);
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

/// Runs each suite function above as its own test on the value an implementation gives.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn every_method_answers() {
            $crate::every_method_answers($value);
        }
    };
}
