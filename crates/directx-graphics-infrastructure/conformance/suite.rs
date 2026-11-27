//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The conformance suite of directx-graphics-infrastructure, written against the traits alone (src/glue_hostile.rs): an object of every kind
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
struct Objects<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> {
    api: A,
    k51: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>,
    k52: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>,
    k53: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain,
    >,
    k54: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>,
    k55: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl,
    >,
    k56: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>,
    k57:
        Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>,
    k58: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>,
    k59: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>,
    k60: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>,
    k61: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication,
    >,
    k62: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>,
    k63: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>,
    k64: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>,
    k65: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia,
    >,
    k66: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis,
    >,
}

impl<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> Objects<A> {
    fn new(api: A) -> Objects<A> {
        Objects {
            api,
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
            k61: None,
            k62: None,
            k63: None,
            k64: None,
            k65: None,
            k66: None,
        }
    }

    fn kinds_held(&self) -> usize {
        [
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
            self.k61.is_some(),
            self.k62.is_some(),
            self.k63.is_some(),
            self.k64.is_some(),
            self.k65.is_some(),
            self.k66.is_some(),
        ]
        .iter()
        .filter(|held| **held)
        .count()
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new.
fn make_all<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    o: &mut Objects<A>,
) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        // IDXGIAdapter::EnumOutputs
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Adapter::enum_outputs(
                        this,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k51.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDevice::GetAdapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Device::get_adapter(this);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, Default::default(), Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, Default::default(), Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                            this,
                            Default::default(),
                            Default::default(),
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory1::EnumAdapters1
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory1::enum_adapters1(
                        this,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIFactory::EnumAdapters
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory::enum_adapters(
                        this,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIFactory::CreateSoftwareAdapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory::create_software_adapter(
                        this,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k56.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k59.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIOutput5::DuplicateOutput1
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let v1 = (if let Some(x) = o.k51.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k52.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k53.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k54.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k55.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k56.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k57.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k58.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k59.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k60.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Output5::duplicate_output1(
                        this,
                        v1,
                        Default::default(),
                        &[],
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIOutput1::DuplicateOutput
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let v1 = (if let Some(x) = o.k51.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k52.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k53.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k54.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k55.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k56.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k57.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k58.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k59.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k60.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer =
                        directx_graphics_infrastructure::Output1::duplicate_output(this, v1);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k60.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIOutputDuplication::AcquireNextFrame
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                            this,
                            Default::default(),
                        );
                    answer.ok()
                })
            };
            if let Some((p0, p1)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p1);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIResource1::CreateSubresourceSurface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
                        );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k62.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k63.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGISwapChain::GetFullscreenState
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this);
                    answer.ok()
                })
            };
            if let Some((p0, p1)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p1);
            }
        }
        // IDXGISwapChain::GetContainingOutput
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::SwapChain::get_containing_output(this);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // CreateDXGIFactory for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // CreateDXGIFactory for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // CreateDXGIFactory for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // CreateDXGIFactory for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // CreateDXGIFactory for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // CreateDXGIFactory for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // CreateDXGIFactory for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // CreateDXGIFactory for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // CreateDXGIFactory for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // CreateDXGIFactory for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Adapter
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Debug
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for DecodeSwapChain
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for DisplayControl
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Factory
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for FactoryMedia
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for InfoQueue
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for KeyedMutex
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Output
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for OutputDuplication
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Resource
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Surface
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for SwapChain
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for SwapChainMedia
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for GraphicsAnalysis
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        if o.kinds_held() == before {
            break;
        }
    }
    o.kinds_held()
}

/// Calls every method on every object held, and the exports, each answer checked.
fn drive<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    o: &Objects<A>,
) -> Driven {
    let mut driven = Driven::default();
    // IDXGIAdapter4::GetDesc3 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter4::get_desc3(this))
    });
    // IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::Adapter3::register_hardware_content_protection_teardown_status_event(this, Default::default()))
    });
    // IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::Adapter3::unregister_hardware_content_protection_teardown_status(this, Default::default()))
    });
    // IDXGIAdapter3::QueryVideoMemoryInfo on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Adapter3::query_video_memory_info(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIAdapter3::SetVideoMemoryReservation on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Adapter3::set_video_memory_reservation(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::Adapter3::register_video_memory_budget_change_notification_event(this, Default::default()))
    });
    // IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::Adapter3::unregister_video_memory_budget_change_notification(this, Default::default()))
    });
    // IDXGIAdapter2::GetDesc2 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter2::get_desc2(this))
    });
    // IDXGIAdapter1::GetDesc1 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter1::get_desc1(this))
    });
    // IDXGIAdapter::EnumOutputs on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter::enum_outputs(
            this,
            Default::default(),
        ))
    });
    // IDXGIAdapter::GetDesc on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter::get_desc(this))
    });
    // IDXGIAdapter::CheckInterfaceSupport on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Adapter::check_interface_support(
                this,
                &Default::default(),
            ),
        )
    });
    // IDXGIObject::SetPrivateData on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Adapter
    driven.attempt(|| {
        let Some(this) = o.k51.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDebug1::EnableLeakTrackingForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::enable_leak_tracking_for_thread(this))
    });
    // IDXGIDebug1::DisableLeakTrackingForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::disable_leak_tracking_for_thread(this))
    });
    // IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::is_leak_tracking_enabled_for_thread(this))
    });
    // IDXGIDebug::ReportLiveObjects on Debug
    driven.attempt(|| {
        let Some(this) = o.k52.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Debug::report_live_objects(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGIDecodeSwapChain::PresentBuffer on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DecodeSwapChain::present_buffer(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIDecodeSwapChain::SetDestSize on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DecodeSwapChain::set_dest_size(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIDecodeSwapChain::GetSourceRect on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_source_rect(this))
    });
    // IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_target_rect(this))
    });
    // IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_dest_size(this))
    });
    // IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DecodeSwapChain::set_color_space(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIDecodeSwapChain::GetColorSpace on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k53.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::DecodeSwapChain::get_color_space(this))
    });
    // IDXGIDevice4::OfferResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device4::offer_resources1(
            this,
            &[],
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGIDevice4::ReclaimResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device4::reclaim_resources1(this, &[]))
    });
    // IDXGIDevice3::Trim on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Device3::trim(this))
    });
    // IDXGIDevice2::OfferResources on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::offer_resources(
            this,
            &[],
            Default::default(),
        ))
    });
    // IDXGIDevice2::ReclaimResources on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::reclaim_resources(
            this,
            &[],
        ))
    });
    // IDXGIDevice2::EnqueueSetEvent on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::enqueue_set_event(
            this,
            Default::default(),
        ))
    });
    // IDXGIDevice1::SetMaximumFrameLatency on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device1::set_maximum_frame_latency(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIDevice1::GetMaximumFrameLatency on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device1::get_maximum_frame_latency(this),
        )
    });
    // IDXGIDevice::GetAdapter on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_adapter(this))
    });
    // IDXGIDevice::QueryResourceResidency on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device::query_resource_residency(this, &[]),
        )
    });
    // IDXGIDevice::SetGPUThreadPriority on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device::set_gpu_thread_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIDevice::GetGPUThreadPriority on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_gpu_thread_priority(this))
    });
    // IDXGIObject::SetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Device
    driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDisplayControl::IsStereoEnabled on DisplayControl
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::DisplayControl::is_stereo_enabled(this))
    });
    // IDXGIDisplayControl::SetStereoEnabled on DisplayControl
    driven.attempt(|| {
        let Some(this) = o.k55.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::DisplayControl::set_stereo_enabled(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory7::RegisterAdaptersChangedEvent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory7::register_adapters_changed_event(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory7::UnregisterAdaptersChangedEvent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory7::unregister_adapters_changed_event(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory6::EnumAdapterByGpuPreference on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                this,
                Default::default(),
                Default::default(),
                &Default::default(),
            ),
        )
    });
    // IDXGIFactory5::CheckFeatureSupport on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::Factory5::check_feature_support(
                this,
                Default::default(),
                &lying,
            ),
        )
    });
    // IDXGIFactory4::EnumWarpAdapter on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory4::enum_warp_adapter(this, &Default::default()),
        )
    });
    // IDXGIFactory3::GetCreationFlags on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory3::get_creation_flags(this))
    });
    // IDXGIFactory2::IsWindowedStereoEnabled on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory2::is_windowed_stereo_enabled(this))
    });
    // IDXGIFactory2::GetSharedResourceAdapterLuid on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory2::get_shared_resource_adapter_luid(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::RegisterStereoStatusWindow on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory2::register_stereo_status_window(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::RegisterStereoStatusEvent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory2::register_stereo_status_event(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::UnregisterStereoStatus on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::Factory2::unregister_stereo_status(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::RegisterOcclusionStatusWindow on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory2::register_occlusion_status_window(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::RegisterOcclusionStatusEvent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory2::register_occlusion_status_event(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory2::UnregisterOcclusionStatus on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::Factory2::unregister_occlusion_status(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIFactory1::EnumAdapters1 on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory1::enum_adapters1(
            this,
            Default::default(),
        ))
    });
    // IDXGIFactory1::IsCurrent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory1::is_current(this))
    });
    // IDXGIFactory::EnumAdapters on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory::enum_adapters(
            this,
            Default::default(),
        ))
    });
    // IDXGIFactory::MakeWindowAssociation on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory::make_window_association(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIFactory::GetWindowAssociation on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory::get_window_association(this))
    });
    // IDXGIFactory::CreateSoftwareAdapter on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory::create_software_adapter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIObject::SetPrivateData on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Factory
    driven.attempt(|| {
        let Some(this) = o.k56.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIInfoQueue::SetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::set_message_count_limit(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::ClearStoredMessages on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::clear_stored_messages(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::get_message(
            this,
            Default::default(),
            Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::InfoQueue::get_num_stored_messages_allowed_by_retrieval_filters(this, Default::default()))
    });
    // IDXGIInfoQueue::GetNumStoredMessages on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_num_stored_messages(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::InfoQueue::get_num_messages_discarded_by_message_count_limit(this, Default::default()))
    });
    // IDXGIInfoQueue::GetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_message_count_limit(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_num_messages_allowed_by_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_num_messages_denied_by_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::get_storage_filter(
                this,
                Default::default(),
                Some(&lying),
            ),
        )
    });
    // IDXGIInfoQueue::ClearStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::clear_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushEmptyStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_empty_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushDenyAllStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_deny_all_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushCopyOfStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_copy_of_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PopStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::pop_storage_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetStorageFilterStackSize on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_storage_filter_stack_size(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::get_retrieval_filter(
                this,
                Default::default(),
                Some(&lying),
            ),
        )
    });
    // IDXGIInfoQueue::ClearRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::clear_retrieval_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushEmptyRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_empty_retrieval_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushDenyAllRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_deny_all_retrieval_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PushCopyOfRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::push_copy_of_retrieval_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::PopRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::pop_retrieval_filter(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetRetrievalFilterStackSize on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_retrieval_filter_stack_size(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::AddMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::add_message(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            c"",
        ))
    });
    // IDXGIInfoQueue::AddApplicationMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::add_application_message(
                this,
                Default::default(),
                c"",
            ),
        )
    });
    // IDXGIInfoQueue::SetBreakOnCategory on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::set_break_on_category(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::SetBreakOnSeverity on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::set_break_on_severity(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::SetBreakOnID on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::set_break_on_id(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGIInfoQueue::GetBreakOnCategory on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_break_on_category(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetBreakOnSeverity on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_break_on_severity(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetBreakOnID on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::InfoQueue::get_break_on_id(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGIInfoQueue::SetMuteDebugOutput on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::set_mute_debug_output(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIInfoQueue::GetMuteDebugOutput on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k58.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::InfoQueue::get_mute_debug_output(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIKeyedMutex::AcquireSync on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::KeyedMutex::acquire_sync(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGIKeyedMutex::ReleaseSync on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::KeyedMutex::release_sync(
            this,
            Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k59.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIOutput6::GetDesc1 on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output6::get_desc1(this))
    });
    // IDXGIOutput6::CheckHardwareCompositionSupport on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output6::check_hardware_composition_support(this),
        )
    });
    // IDXGIOutput5::DuplicateOutput1 on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k51.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k56.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output5::duplicate_output1(
            this,
            v1,
            Default::default(),
            &[],
        ))
    });
    // IDXGIOutput4::CheckOverlayColorSpaceSupport on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k51.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k56.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output4::check_overlay_color_space_support(
                this,
                Default::default(),
                Default::default(),
                v1,
            ),
        )
    });
    // IDXGIOutput3::CheckOverlaySupport on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k51.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k56.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output3::check_overlay_support(
                this,
                Default::default(),
                v1,
            ),
        )
    });
    // IDXGIOutput2::SupportsOverlays on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Output2::supports_overlays(
            this,
        ))
    });
    // IDXGIOutput1::GetDisplayModeList1 on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output1::get_display_mode_list1(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIOutput1::GetDisplaySurfaceData1 on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output1::get_display_surface_data1(this, v1),
        )
    });
    // IDXGIOutput1::DuplicateOutput on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k51.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k56.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output1::duplicate_output(
            this, v1,
        ))
    });
    // IDXGIOutput::GetDesc on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_desc(this))
    });
    // IDXGIOutput::GetDisplayModeList on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output::get_display_mode_list(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGIOutput::WaitForVBlank on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::wait_for_v_blank(
            this,
        ))
    });
    // IDXGIOutput::TakeOwnership on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k51.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k52.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k53.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k54.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k55.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k56.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k57.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k58.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k59.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k60.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::take_ownership(
            this,
            v1,
            Default::default(),
        ))
    });
    // IDXGIOutput::ReleaseOwnership on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Output::release_ownership(
            this,
        ))
    });
    // IDXGIOutput::GetGammaControlCapabilities on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output::get_gamma_control_capabilities(this),
        )
    });
    // IDXGIOutput::GetGammaControl on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_gamma_control(
            this,
        ))
    });
    // IDXGIOutput::SetDisplaySurface on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::set_display_surface(this, v1))
    });
    // IDXGIOutput::GetDisplaySurfaceData on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let Some(v1) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output::get_display_surface_data(this, v1),
        )
    });
    // IDXGIOutput::GetFrameStatistics on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_frame_statistics(this))
    });
    // IDXGIObject::SetPrivateData on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Output
    driven.attempt(|| {
        let Some(this) = o.k60.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIOutputDuplication::GetDesc on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::OutputDuplication::get_desc(this))
    });
    // IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIOutputDuplication::GetFrameDirtyRects on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::get_frame_dirty_rects(this, &lying),
        )
    });
    // IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::get_frame_move_rects(this, &lying),
        )
    });
    // IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::get_frame_pointer_shape(
                this, &lying,
            ),
        )
    });
    // IDXGIOutputDuplication::MapDesktopSurface on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::map_desktop_surface(this),
        )
    });
    // IDXGIOutputDuplication::UnMapDesktopSurface on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::un_map_desktop_surface(this),
        )
    });
    // IDXGIOutputDuplication::ReleaseFrame on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::OutputDuplication::release_frame(this))
    });
    // IDXGIObject::SetPrivateData on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIResource1::CreateSubresourceSurface on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource1::CreateSharedHandle on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_shared_handle(
                this,
                None,
                Default::default(),
                None,
            ),
        )
    });
    // IDXGIResource::GetSharedHandle on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource::GetEvictionPriority on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIDeviceSubObject::GetDevice on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Resource
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISurface2::GetResource on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface2::get_resource(
            this,
            &Default::default(),
        ))
    });
    // IDXGISurface1::GetDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::get_dc(
            this,
            Default::default(),
        ))
    });
    // IDXGISurface1::ReleaseDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::release_dc(
            this, None,
        ))
    });
    // IDXGISurface::GetDesc on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::get_desc(this))
    });
    // IDXGISurface::Map on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::map(
            this,
            Default::default(),
        ))
    });
    // IDXGISurface::Unmap on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::unmap(this))
    });
    // IDXGIDeviceSubObject::GetDevice on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Surface
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISwapChain4::SetHDRMetaData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain4::set_hdr_meta_data(
                this,
                Default::default(),
                None,
            ),
        )
    });
    // IDXGISwapChain3::GetCurrentBackBufferIndex on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::SwapChain3::get_current_back_buffer_index(this))
    });
    // IDXGISwapChain3::CheckColorSpaceSupport on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain3::check_color_space_support(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGISwapChain3::SetColorSpace1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain3::set_color_space1(this, Default::default()),
        )
    });
    // IDXGISwapChain3::ResizeBuffers1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain3::resize_buffers1(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                &[],
                &[],
            ),
        )
    });
    // IDXGISwapChain2::SetSourceSize on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain2::set_source_size(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // IDXGISwapChain2::GetSourceSize on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain2::get_source_size(this))
    });
    // IDXGISwapChain2::SetMaximumFrameLatency on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain2::set_maximum_frame_latency(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGISwapChain2::GetMaximumFrameLatency on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain2::get_maximum_frame_latency(this),
        )
    });
    // IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::SwapChain2::get_frame_latency_waitable_object(this),
        )
    });
    // IDXGISwapChain2::GetMatrixTransform on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain2::get_matrix_transform(this))
    });
    // IDXGISwapChain1::GetDesc1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_desc1(this))
    });
    // IDXGISwapChain1::GetFullscreenDesc on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_fullscreen_desc(this))
    });
    // IDXGISwapChain1::GetHwnd on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_hwnd(this))
    });
    // IDXGISwapChain1::GetCoreWindow on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain1::get_core_window(this, &Default::default()),
        )
    });
    // IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::SwapChain1::is_temporary_mono_supported(this))
    });
    // IDXGISwapChain1::GetRestrictToOutput on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain1::get_restrict_to_output(this),
        )
    });
    // IDXGISwapChain1::GetBackgroundColor on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_background_color(this))
    });
    // IDXGISwapChain1::SetRotation on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::set_rotation(
            this,
            Default::default(),
        ))
    });
    // IDXGISwapChain1::GetRotation on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_rotation(
            this,
        ))
    });
    // IDXGISwapChain::Present on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::present(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGISwapChain::GetBuffer on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_buffer(
            this,
            Default::default(),
        ))
    });
    // IDXGISwapChain::SetFullscreenState on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain::set_fullscreen_state(
                this,
                Default::default(),
                None,
            ),
        )
    });
    // IDXGISwapChain::GetFullscreenState on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this))
    });
    // IDXGISwapChain::GetDesc on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_desc(this))
    });
    // IDXGISwapChain::ResizeBuffers on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::resize_buffers(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // IDXGISwapChain::GetContainingOutput on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_containing_output(this))
    });
    // IDXGISwapChain::GetFrameStatistics on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_frame_statistics(this))
    });
    // IDXGISwapChain::GetLastPresentCount on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain::get_last_present_count(this),
        )
    });
    // IDXGIDeviceSubObject::GetDevice on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &Default::default(),
                None,
            ),
        )
    });
    // IDXGIObject::GetPrivateData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChainMedia::get_frame_statistics_media(this),
        )
    });
    // IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChainMedia::set_present_duration(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGISwapChainMedia::CheckPresentDurationSupport on SwapChainMedia
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChainMedia::check_present_duration_support(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGraphicsAnalysis::BeginCapture on GraphicsAnalysis
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::GraphicsAnalysis::begin_capture(this))
    });
    // IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::GraphicsAnalysis::end_capture(this))
    });
    // CreateDXGIFactory
    driven.attempt(|| {
        failure_is_negative(
            directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(
                &o.api,
                &Default::default(),
            ),
        )
    });
    // CreateDXGIFactory1
    driven.attempt(|| {
        failure_is_negative(
            directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(
                &o.api,
                &Default::default(),
            ),
        )
    });
    // CreateDXGIFactory2
    driven.attempt(|| {
        failure_is_negative(
            directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(
                &o.api,
                Default::default(),
                &Default::default(),
            ),
        )
    });
    // DXGIDeclareAdapterRemovalSupport
    driven.attempt(|| {
        failure_is_negative(directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_declare_adapter_removal_support(&o.api))
    });
    // DXGIDisableVBlankVirtualization
    driven.attempt(|| {
        failure_is_negative(directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_disable_v_blank_virtualization(&o.api))
    });
    // DXGIGetDebugInterface1
    driven.attempt(|| {
        failure_is_negative(directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, Default::default(), &Default::default()))
    });
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 211;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    api: A,
) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o);
    println!(
        "SUITE directx-graphics-infrastructure: kinds made {kinds}, calls {CALLS}: called {}, skipped {}, panicked {}",
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
