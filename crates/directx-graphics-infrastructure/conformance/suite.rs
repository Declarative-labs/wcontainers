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
    clippy::format_in_format_args,
    clippy::to_string_in_format_args,
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
struct Objects<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> {
    api: A,
    k61: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>,
    k62: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>,
    k63: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain,
    >,
    k64: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>,
    k65: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl,
    >,
    k66: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>,
    k67:
        Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>,
    k68: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>,
    k69: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>,
    k70: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>,
    k71: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication,
    >,
    k72: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>,
    k73: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>,
    k74: Option<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>,
    k75: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia,
    >,
    k76: Option<
        <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis,
    >,
}

impl<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> Objects<A> {
    fn new(api: A) -> Objects<A> {
        Objects {
            api,
            k61: None,
            k62: None,
            k63: None,
            k64: None,
            k65: None,
            k66: None,
            k67: None,
            k68: None,
            k69: None,
            k70: None,
            k71: None,
            k72: None,
            k73: None,
            k74: None,
            k75: None,
            k76: None,
        }
    }

    fn kinds_held(&self) -> usize {
        [
            self.k61.is_some(),
            self.k62.is_some(),
            self.k63.is_some(),
            self.k64.is_some(),
            self.k65.is_some(),
            self.k66.is_some(),
            self.k67.is_some(),
            self.k68.is_some(),
            self.k69.is_some(),
            self.k70.is_some(),
            self.k71.is_some(),
            self.k72.is_some(),
            self.k73.is_some(),
            self.k74.is_some(),
            self.k75.is_some(),
            self.k76.is_some(),
        ]
        .iter()
        .filter(|held| **held)
        .count()
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new: first by
/// the methods that hand out an object of their own kind, then by those too that hand out an interface by id.
fn make_all<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    o: &mut Objects<A>,
) -> usize {
    for by_id in [false, true] {
        for _ in 0..6 {
            let before = o.kinds_held();
            // IDXGIFactory2::CreateSwapChainForHwnd
            if o.k74.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_hwnd(this, v1, component_object_model::structures::Hwnd(98), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, Some(&directx_graphics_infrastructure::structures::SwapChainFullscreenDesc { refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 87_u32, denominator: 94_u32 }, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, windowed: true }), o.k70.as_ref());
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory2::CreateSwapChainForCoreWindow
            if o.k74.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let v2 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_core_window(this, v1, v2, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory2::CreateSwapChainForComposition
            if o.k74.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_composition(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory::CreateSwapChain
            if o.k74.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::Factory::create_swap_chain(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc { buffer_desc: directx_graphics_infrastructure::structures::ModeDesc { width: 65_u32, height: 72_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 86_u32, denominator: 93_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 72_u32, quality: 79_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 79_u32, output_window: component_object_model::structures::Hwnd(86), windowed: true, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, flags: 107_u32 });
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory::CreateSoftwareAdapter
            if o.k61.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::Factory::create_software_adapter(
                                this,
                                component_object_model::structures::Hmodule(0),
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
            // IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle
            if o.k74.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k67.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::FactoryMedia::create_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle
            if o.k63.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k67.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let v2 = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::FactoryMedia::create_decode_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::DecodeSwapChainDesc { flags: 58_u32 }, v2, o.k70.as_ref());
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIResource1::CreateSubresourceSurface
            if o.k73.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                                this, 35_u32,
                            );
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIAdapter::EnumOutputs
            if o.k70.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k61.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::Adapter::enum_outputs(this, 22_u32);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIDevice::GetAdapter
            if o.k61.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k64.as_ref()?;
                        let answer = directx_graphics_infrastructure::Device::get_adapter(this);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIFactory1::EnumAdapters1
            if o.k61.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::Factory1::enum_adapters1(this, 9_u32);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIFactory::EnumAdapters
            if o.k61.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::Factory::enum_adapters(this, 9_u32);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIOutput5::DuplicateOutput1
            if o.k71.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                        } else {
                            None
                        })?;
                        let answer = directx_graphics_infrastructure::Output5::duplicate_output1(this, v1, 31_u32, &[directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS]);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIOutput1::DuplicateOutput
            if o.k71.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let v1 = (if let Some(x) = o.k61.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                        } else if let Some(x) = o.k62.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                        } else if let Some(x) = o.k63.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                        } else if let Some(x) = o.k64.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                        } else if let Some(x) = o.k65.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                                x,
                            ))
                        } else if let Some(x) = o.k66.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                        } else if let Some(x) = o.k67.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                        } else if let Some(x) = o.k68.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                        } else if let Some(x) = o.k69.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                        } else if let Some(x) = o.k70.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                        } else if let Some(x) = o.k71.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                        } else if let Some(x) = o.k72.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                        } else if let Some(x) = o.k73.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                        } else if let Some(x) = o.k74.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                        } else if let Some(x) = o.k75.as_ref() {
                            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                                x,
                            ))
                        } else if let Some(x) = o.k76.as_ref() {
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
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIOutputDuplication::AcquireNextFrame
            if o.k72.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                                this, 24_u32,
                            );
                        answer.ok()
                    })
                };
                if let Some((p0, p1)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p1);
                }
            }
            // IDXGISwapChain::GetFullscreenState
            if o.k70.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this);
                        answer.ok()
                    })
                };
                if let Some((p0, p1)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p1);
                }
            }
            // IDXGISwapChain::GetContainingOutput
            if o.k70.is_none() {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer =
                            directx_graphics_infrastructure::SwapChain::get_containing_output(this);
                        answer.ok()
                    })
                };
                if let Some(p0) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
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
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
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
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k61.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
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
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
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
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
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
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
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
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
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
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k61.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
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
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
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
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
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
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
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
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
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
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
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
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k64.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
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
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
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
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
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
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
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
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
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
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k64.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
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
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
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
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
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
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
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
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
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
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIFactory6::EnumAdapterByGpuPreference for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIFactory4::EnumWarpAdapter for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k66.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k69.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k70.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k71.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k72.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k73.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGISwapChain1::GetCoreWindow for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(this, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // IDXGIObject::GetParent for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let this = o.k74.as_ref()?;
                        let answer = directx_graphics_infrastructure::Object::get_parent(
                            this,
                            &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                        );
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // CreateDXGIFactory for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // CreateDXGIFactory for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // CreateDXGIFactory for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // CreateDXGIFactory for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // CreateDXGIFactory for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // CreateDXGIFactory for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // CreateDXGIFactory for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // CreateDXGIFactory for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // CreateDXGIFactory for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // CreateDXGIFactory for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // CreateDXGIFactory for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // CreateDXGIFactory for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // CreateDXGIFactory for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // CreateDXGIFactory for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // CreateDXGIFactory for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // CreateDXGIFactory for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // CreateDXGIFactory1 for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // CreateDXGIFactory2 for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Adapter
            if by_id && (o.k61.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                    && o.k61.is_none()
                {
                    o.k61 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Debug
            if by_id && (o.k62.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                    && o.k62.is_none()
                {
                    o.k62 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for DecodeSwapChain
            if by_id && (o.k63.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                    answer
                    && o.k63.is_none()
                {
                    o.k63 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Device
            if by_id && (o.k64.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                    && o.k64.is_none()
                {
                    o.k64 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for DisplayControl
            if by_id && (o.k65.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) =
                    answer
                    && o.k65.is_none()
                {
                    o.k65 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Factory
            if by_id && (o.k66.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                    && o.k66.is_none()
                {
                    o.k66 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for FactoryMedia
            if by_id && (o.k67.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) =
                    answer
                    && o.k67.is_none()
                {
                    o.k67 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for InfoQueue
            if by_id && (o.k68.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                    && o.k68.is_none()
                {
                    o.k68 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for KeyedMutex
            if by_id && (o.k69.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                    && o.k69.is_none()
                {
                    o.k69 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Output
            if by_id && (o.k70.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                    && o.k70.is_none()
                {
                    o.k70 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for OutputDuplication
            if by_id && (o.k71.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                    answer
                    && o.k71.is_none()
                {
                    o.k71 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Resource
            if by_id && (o.k72.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                    && o.k72.is_none()
                {
                    o.k72 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for Surface
            if by_id && (o.k73.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                    && o.k73.is_none()
                {
                    o.k73 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for SwapChain
            if by_id && (o.k74.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                    && o.k74.is_none()
                {
                    o.k74 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for SwapChainMedia
            if by_id && (o.k75.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) =
                    answer
                    && o.k75.is_none()
                {
                    o.k75 = Some(p0);
                }
            }
            // DXGIGetDebugInterface1 for GraphicsAnalysis
            if by_id && (o.k76.is_none()) {
                let answer = {
                    let o = &*o;
                    made(|| {
                        let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                        answer.ok()
                    })
                };
                if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                    answer
                    && o.k76.is_none()
                {
                    o.k76 = Some(p0);
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
fn drive<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    o: &Objects<A>,
    answers: &Answers,
) -> Driven {
    let mut driven = Driven::default();
    // IDXGIAdapter4::GetDesc3 on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter4::get_desc3(this);
            answers.push(format!(
                "IDXGIAdapter4::GetDesc3 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter4::GetDesc3 on Adapter");
    // IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Adapter3::register_hardware_content_protection_teardown_status_event(this, component_object_model::structures::Handle(82)); answers.push(format!("IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter",
    );
    // IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::Adapter3::unregister_hardware_content_protection_teardown_status(this, 87_u32); answers.push("IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus -> nothing".to_string()); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter",
    );
    // IDXGIAdapter3::QueryVideoMemoryInfo on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter3::query_video_memory_info(
                this,
                72_u32,
                directx_graphics_infrastructure::enumerations::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
            );
            answers.push(format!(
                "IDXGIAdapter3::QueryVideoMemoryInfo -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter3::QueryVideoMemoryInfo on Adapter");
    // IDXGIAdapter3::SetVideoMemoryReservation on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter3::set_video_memory_reservation(
                this,
                72_u32,
                directx_graphics_infrastructure::enumerations::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
                73_u64,
            );
            answers.push(format!(
                "IDXGIAdapter3::SetVideoMemoryReservation -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter3::SetVideoMemoryReservation on Adapter");
    // IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Adapter3::register_video_memory_budget_change_notification_event(this, component_object_model::structures::Handle(82)); answers.push(format!("IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent on Adapter",
    );
    // IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::Adapter3::unregister_video_memory_budget_change_notification(this, 87_u32); answers.push("IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification -> nothing".to_string()); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter",
    );
    // IDXGIAdapter2::GetDesc2 on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter2::get_desc2(this);
            answers.push(format!(
                "IDXGIAdapter2::GetDesc2 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter2::GetDesc2 on Adapter");
    // IDXGIAdapter1::GetDesc1 on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter1::get_desc1(this);
            answers.push(format!(
                "IDXGIAdapter1::GetDesc1 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter1::GetDesc1 on Adapter");
    // IDXGIAdapter::EnumOutputs on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter::enum_outputs(this, 22_u32);
            answers.push(format!(
                "IDXGIAdapter::EnumOutputs -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter::EnumOutputs on Adapter");
    // IDXGIAdapter::GetDesc on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter::get_desc(this);
            answers.push(format!(
                "IDXGIAdapter::GetDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter::GetDesc on Adapter");
    // IDXGIAdapter::CheckInterfaceSupport on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Adapter::check_interface_support(
                this,
                &component_object_model::structures::Guid {
                    data1: 117_u32,
                    data2: 124_u16,
                    data3: 5_u16,
                    data4: core::array::from_fn(|_| 12_u8),
                },
            );
            answers.push(format!(
                "IDXGIAdapter::CheckInterfaceSupport -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIAdapter::CheckInterfaceSupport on Adapter");
    // IDXGIObject::SetPrivateData on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Adapter");
    // IDXGIObject::SetPrivateDataInterface on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Adapter");
    // IDXGIObject::GetPrivateData on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Adapter
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Adapter");
    // IDXGIDebug1::EnableLeakTrackingForThread on Debug
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::Debug1::enable_leak_tracking_for_thread(this);
            answers.push("IDXGIDebug1::EnableLeakTrackingForThread -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDebug1::EnableLeakTrackingForThread on Debug");
    // IDXGIDebug1::DisableLeakTrackingForThread on Debug
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::Debug1::disable_leak_tracking_for_thread(this);
            answers.push("IDXGIDebug1::DisableLeakTrackingForThread -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDebug1::DisableLeakTrackingForThread on Debug");
    // IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::Debug1::is_leak_tracking_enabled_for_thread(this);
            answers.push(format!(
                "IDXGIDebug1::IsLeakTrackingEnabledForThread -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug",
    );
    // IDXGIDebug::ReportLiveObjects on Debug
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Debug::report_live_objects(
                this,
                component_object_model::structures::Guid {
                    data1: 61_u32,
                    data2: 68_u16,
                    data3: 75_u16,
                    data4: core::array::from_fn(|_| 82_u8),
                },
                directx_graphics_infrastructure::enumerations::DXGI_DEBUG_RLO_SUMMARY,
            );
            answers.push(format!(
                "IDXGIDebug::ReportLiveObjects -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDebug::ReportLiveObjects on Debug");
    // IDXGIDecodeSwapChain::PresentBuffer on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::present_buffer(
                this,
                61_u32,
                46_u32,
                directx_graphics_infrastructure::enumerations::DXGI_PRESENT_TEST,
            );
            answers.push(format!(
                "IDXGIDecodeSwapChain::PresentBuffer -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::PresentBuffer on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::SetSourceRect on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::set_source_rect(
                this,
                &component_object_model::structures::Rect {
                    left: 37_i32,
                    top: 44_i32,
                    right: 51_i32,
                    bottom: 58_i32,
                },
            );
            answers.push(format!(
                "IDXGIDecodeSwapChain::SetSourceRect -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::SetSourceRect on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::SetTargetRect on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::set_target_rect(
                this,
                &component_object_model::structures::Rect {
                    left: 37_i32,
                    top: 44_i32,
                    right: 51_i32,
                    bottom: 58_i32,
                },
            );
            answers.push(format!(
                "IDXGIDecodeSwapChain::SetTargetRect -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::SetTargetRect on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::SetDestSize on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::set_dest_size(
                this, 120_u32, 44_u32,
            );
            answers.push(format!(
                "IDXGIDecodeSwapChain::SetDestSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::SetDestSize on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::GetSourceRect on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::get_source_rect(this);
            answers.push(format!(
                "IDXGIDecodeSwapChain::GetSourceRect -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::GetSourceRect on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::get_target_rect(this);
            answers.push(format!(
                "IDXGIDecodeSwapChain::GetTargetRect -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::get_dest_size(this);
            answers.push(format!(
                "IDXGIDecodeSwapChain::GetDestSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::DecodeSwapChain::set_color_space(this, directx_graphics_infrastructure::enumerations::DXGI_MULTIPLANE_OVERLAY_YCBCR_FLAG_NOMINAL_RANGE); answers.push(format!("IDXGIDecodeSwapChain::SetColorSpace -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain",
    );
    // IDXGIDecodeSwapChain::GetColorSpace on DecodeSwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::DecodeSwapChain::get_color_space(this);
            answers.push(format!(
                "IDXGIDecodeSwapChain::GetColorSpace -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDecodeSwapChain::GetColorSpace on DecodeSwapChain",
    );
    // IDXGIDevice4::OfferResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device4::offer_resources1(
            this,
            &[o.k72.as_ref()],
            directx_graphics_infrastructure::enumerations::DXGI_OFFER_RESOURCE_PRIORITY_LOW,
            31_u32,
        ))
    });
    // IDXGIDevice4::ReclaimResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device4::reclaim_resources1(this, &[o.k72.as_ref()]),
        )
    });
    // IDXGIDevice3::Trim on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Device3::trim(this);
            answers.push("IDXGIDevice3::Trim -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice3::Trim on Device");
    // IDXGIDevice2::OfferResources on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::offer_resources(
            this,
            &[o.k72.as_ref()],
            directx_graphics_infrastructure::enumerations::DXGI_OFFER_RESOURCE_PRIORITY_LOW,
        ))
    });
    // IDXGIDevice2::ReclaimResources on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::reclaim_resources(
            this,
            &[o.k72.as_ref()],
        ))
    });
    // IDXGIDevice2::EnqueueSetEvent on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Device2::enqueue_set_event(
                this,
                component_object_model::structures::Handle(82),
            );
            answers.push(format!(
                "IDXGIDevice2::EnqueueSetEvent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice2::EnqueueSetEvent on Device");
    // IDXGIDevice1::SetMaximumFrameLatency on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Device1::set_maximum_frame_latency(this, 63_u32);
            answers.push(format!(
                "IDXGIDevice1::SetMaximumFrameLatency -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice1::SetMaximumFrameLatency on Device");
    // IDXGIDevice1::GetMaximumFrameLatency on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Device1::get_maximum_frame_latency(this);
            answers.push(format!(
                "IDXGIDevice1::GetMaximumFrameLatency -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice1::GetMaximumFrameLatency on Device");
    // IDXGIDevice::GetAdapter on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Device::get_adapter(this);
            answers.push(format!(
                "IDXGIDevice::GetAdapter -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice::GetAdapter on Device");
    // IDXGIDevice::CreateSurface on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Device::create_surface(this, &directx_graphics_infrastructure::structures::SurfaceDesc { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 86_u32, quality: 93_u32 } }, 116_u32, directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, Some(&directx_graphics_infrastructure::structures::SharedResource { handle: component_object_model::structures::Handle(55) })); answers.push(format!("IDXGIDevice::CreateSurface -> {}", match &answer { Ok(v0) => format!("Ok({})", format!("[{}]", v0.iter().map(|v1| match v1 { Some(v2) => format!("Some({})", "an object".to_string()), None => "None".to_string() }).collect::<Vec<String>>().join(", "))), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "IDXGIDevice::CreateSurface on Device");
    // IDXGIDevice::QueryResourceResidency on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device::query_resource_residency(this, &[None]),
        )
    });
    // IDXGIDevice::SetGPUThreadPriority on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Device::set_gpu_thread_priority(this, 14_i32);
            answers.push(format!(
                "IDXGIDevice::SetGPUThreadPriority -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice::SetGPUThreadPriority on Device");
    // IDXGIDevice::GetGPUThreadPriority on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Device::get_gpu_thread_priority(this);
            answers.push(format!(
                "IDXGIDevice::GetGPUThreadPriority -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDevice::GetGPUThreadPriority on Device");
    // IDXGIObject::SetPrivateData on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Device");
    // IDXGIObject::SetPrivateDataInterface on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Device");
    // IDXGIObject::GetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Device
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Device");
    // IDXGIDisplayControl::IsStereoEnabled on DisplayControl
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::DisplayControl::is_stereo_enabled(this);
            answers.push(format!(
                "IDXGIDisplayControl::IsStereoEnabled -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDisplayControl::IsStereoEnabled on DisplayControl",
    );
    // IDXGIDisplayControl::SetStereoEnabled on DisplayControl
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::DisplayControl::set_stereo_enabled(this, true);
            answers.push("IDXGIDisplayControl::SetStereoEnabled -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIDisplayControl::SetStereoEnabled on DisplayControl",
    );
    // IDXGIFactory7::RegisterAdaptersChangedEvent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory7::register_adapters_changed_event(
                this,
                component_object_model::structures::Handle(82),
            );
            answers.push(format!(
                "IDXGIFactory7::RegisterAdaptersChangedEvent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory7::RegisterAdaptersChangedEvent on Factory",
    );
    // IDXGIFactory7::UnregisterAdaptersChangedEvent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Factory7::unregister_adapters_changed_event(
                    this, 87_u32,
                );
            answers.push(format!(
                "IDXGIFactory7::UnregisterAdaptersChangedEvent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory7::UnregisterAdaptersChangedEvent on Factory",
    );
    // IDXGIFactory6::EnumAdapterByGpuPreference on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                this,
                9_u32,
                directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIFactory6::EnumAdapterByGpuPreference -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory6::EnumAdapterByGpuPreference on Factory",
    );
    // IDXGIFactory5::CheckFeatureSupport on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory5::check_feature_support(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FEATURE_PRESENT_ALLOW_TEARING,
                &lying,
            );
            answers.push(format!(
                "IDXGIFactory5::CheckFeatureSupport -> {}",
                format!("{:?}", &answer)
            ));
            answers.bytes("IDXGIFactory5::CheckFeatureSupport", &lying);
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory5::CheckFeatureSupport on Factory");
    // IDXGIFactory4::EnumAdapterByLuid on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory4::enum_adapter_by_luid(
                this,
                component_object_model::structures::Luid {
                    low_part: 5_u32,
                    high_part: 12_i32,
                },
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            ),
        )
    });
    // IDXGIFactory4::EnumWarpAdapter on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIFactory4::EnumWarpAdapter -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory4::EnumWarpAdapter on Factory");
    // IDXGIFactory3::GetCreationFlags on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Factory3::get_creation_flags(this);
            answers.push(format!(
                "IDXGIFactory3::GetCreationFlags -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory3::GetCreationFlags on Factory");
    // IDXGIFactory2::IsWindowedStereoEnabled on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::Factory2::is_windowed_stereo_enabled(this);
            answers.push(format!(
                "IDXGIFactory2::IsWindowedStereoEnabled -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory2::IsWindowedStereoEnabled on Factory");
    // IDXGIFactory2::CreateSwapChainForHwnd on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_hwnd(this, v1, component_object_model::structures::Hwnd(98), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, Some(&directx_graphics_infrastructure::structures::SwapChainFullscreenDesc { refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 87_u32, denominator: 94_u32 }, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, windowed: true }), o.k70.as_ref()); answers.push(format!("IDXGIFactory2::CreateSwapChainForHwnd -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "IDXGIFactory2::CreateSwapChainForHwnd on Factory");
    // IDXGIFactory2::CreateSwapChainForCoreWindow on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let Some(v2) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_core_window(this, v1, v2, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref()); answers.push(format!("IDXGIFactory2::CreateSwapChainForCoreWindow -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::CreateSwapChainForCoreWindow on Factory",
    );
    // IDXGIFactory2::GetSharedResourceAdapterLuid on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Factory2::get_shared_resource_adapter_luid(
                    this,
                    component_object_model::structures::Handle(68),
                );
            answers.push(format!(
                "IDXGIFactory2::GetSharedResourceAdapterLuid -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::GetSharedResourceAdapterLuid on Factory",
    );
    // IDXGIFactory2::RegisterStereoStatusWindow on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory2::register_stereo_status_window(
                this,
                component_object_model::structures::Hwnd(95),
                29_u32,
            );
            answers.push(format!(
                "IDXGIFactory2::RegisterStereoStatusWindow -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::RegisterStereoStatusWindow on Factory",
    );
    // IDXGIFactory2::RegisterStereoStatusEvent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory2::register_stereo_status_event(
                this,
                component_object_model::structures::Handle(82),
            );
            answers.push(format!(
                "IDXGIFactory2::RegisterStereoStatusEvent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory2::RegisterStereoStatusEvent on Factory");
    // IDXGIFactory2::UnregisterStereoStatus on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::Factory2::unregister_stereo_status(this, 87_u32);
            answers.push("IDXGIFactory2::UnregisterStereoStatus -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory2::UnregisterStereoStatus on Factory");
    // IDXGIFactory2::RegisterOcclusionStatusWindow on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Factory2::register_occlusion_status_window(
                    this,
                    component_object_model::structures::Hwnd(95),
                    29_u32,
                );
            answers.push(format!(
                "IDXGIFactory2::RegisterOcclusionStatusWindow -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::RegisterOcclusionStatusWindow on Factory",
    );
    // IDXGIFactory2::RegisterOcclusionStatusEvent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory2::register_occlusion_status_event(
                this,
                component_object_model::structures::Handle(82),
            );
            answers.push(format!(
                "IDXGIFactory2::RegisterOcclusionStatusEvent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::RegisterOcclusionStatusEvent on Factory",
    );
    // IDXGIFactory2::UnregisterOcclusionStatus on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Factory2::unregister_occlusion_status(
                this, 87_u32,
            );
            answers.push("IDXGIFactory2::UnregisterOcclusionStatus -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory2::UnregisterOcclusionStatus on Factory");
    // IDXGIFactory2::CreateSwapChainForComposition on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_composition(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref()); answers.push(format!("IDXGIFactory2::CreateSwapChainForComposition -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactory2::CreateSwapChainForComposition on Factory",
    );
    // IDXGIFactory1::EnumAdapters1 on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory1::enum_adapters1(this, 9_u32);
            answers.push(format!(
                "IDXGIFactory1::EnumAdapters1 -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory1::EnumAdapters1 on Factory");
    // IDXGIFactory1::IsCurrent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Factory1::is_current(this);
            answers.push(format!(
                "IDXGIFactory1::IsCurrent -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory1::IsCurrent on Factory");
    // IDXGIFactory::EnumAdapters on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory::enum_adapters(this, 9_u32);
            answers.push(format!(
                "IDXGIFactory::EnumAdapters -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory::EnumAdapters on Factory");
    // IDXGIFactory::MakeWindowAssociation on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory::make_window_association(
                this,
                component_object_model::structures::Hwnd(95),
                directx_graphics_infrastructure::enumerations::DXGI_MWA_NO_WINDOW_CHANGES,
            );
            answers.push(format!(
                "IDXGIFactory::MakeWindowAssociation -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory::MakeWindowAssociation on Factory");
    // IDXGIFactory::GetWindowAssociation on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Factory::get_window_association(this);
            answers.push(format!(
                "IDXGIFactory::GetWindowAssociation -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIFactory::GetWindowAssociation on Factory");
    // IDXGIFactory::CreateSwapChain on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Factory::create_swap_chain(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc { buffer_desc: directx_graphics_infrastructure::structures::ModeDesc { width: 65_u32, height: 72_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 86_u32, denominator: 93_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 72_u32, quality: 79_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 79_u32, output_window: component_object_model::structures::Hwnd(86), windowed: true, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, flags: 107_u32 }); answers.push(format!("IDXGIFactory::CreateSwapChain -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "IDXGIFactory::CreateSwapChain on Factory");
    // IDXGIFactory::CreateSoftwareAdapter on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory::create_software_adapter(
                this,
                component_object_model::structures::Hmodule(0),
            ),
        )
    });
    // IDXGIObject::SetPrivateData on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Factory");
    // IDXGIObject::SetPrivateDataInterface on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Factory");
    // IDXGIObject::GetPrivateData on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Factory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Factory");
    // IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle on FactoryMedia
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k67.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::FactoryMedia::create_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref()); answers.push(format!("IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle on FactoryMedia",
    );
    // IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle on FactoryMedia
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k67.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let Some(v2) = o.k72.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::FactoryMedia::create_decode_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::DecodeSwapChainDesc { flags: 58_u32 }, v2, o.k70.as_ref()); answers.push(format!("IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle on FactoryMedia",
    );
    // IDXGIInfoQueue::SetMessageCountLimit on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::set_message_count_limit(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                110_u64,
            );
            answers.push(format!(
                "IDXGIInfoQueue::SetMessageCountLimit -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::SetMessageCountLimit on InfoQueue");
    // IDXGIInfoQueue::ClearStoredMessages on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::clear_stored_messages(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push("IDXGIInfoQueue::ClearStoredMessages -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::ClearStoredMessages on InfoQueue");
    // IDXGIInfoQueue::GetMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::get_message(
            this,
            component_object_model::structures::Guid {
                data1: 84_u32,
                data2: 91_u16,
                data3: 98_u16,
                data4: core::array::from_fn(|_| 105_u8),
            },
            115_u64,
            Some(&lying),
        ))
    });
    // IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_num_stored_messages_allowed_by_retrieval_filters(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }); answers.push(format!("IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters on InfoQueue",
    );
    // IDXGIInfoQueue::GetNumStoredMessages on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::get_num_stored_messages(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::GetNumStoredMessages -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetNumStoredMessages on InfoQueue");
    // IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_num_messages_discarded_by_message_count_limit(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }); answers.push(format!("IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue",
    );
    // IDXGIInfoQueue::GetMessageCountLimit on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::get_message_count_limit(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::GetMessageCountLimit -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetMessageCountLimit on InfoQueue");
    // IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_num_messages_allowed_by_storage_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }); answers.push(format!("IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue",
    );
    // IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_num_messages_denied_by_storage_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }); answers.push(format!("IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue",
    );
    // IDXGIInfoQueue::AddStorageFilterEntries on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::add_storage_filter_entries(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, &directx_graphics_infrastructure::structures::InfoQueueFilter { allow_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 9_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 23_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 37_u32, id_list: vec![44_i32] }, deny_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 16_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 30_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 44_u32, id_list: vec![51_i32] } }))
    });
    // IDXGIInfoQueue::GetStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::get_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                Some(&lying),
            ),
        )
    });
    // IDXGIInfoQueue::ClearStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::clear_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push("IDXGIInfoQueue::ClearStorageFilter -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::ClearStorageFilter on InfoQueue");
    // IDXGIInfoQueue::PushEmptyStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_empty_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushEmptyStorageFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::PushEmptyStorageFilter on InfoQueue");
    // IDXGIInfoQueue::PushDenyAllStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_deny_all_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushDenyAllStorageFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::PushDenyAllStorageFilter on InfoQueue",
    );
    // IDXGIInfoQueue::PushCopyOfStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_copy_of_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushCopyOfStorageFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::PushCopyOfStorageFilter on InfoQueue",
    );
    // IDXGIInfoQueue::PushStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::push_storage_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, &directx_graphics_infrastructure::structures::InfoQueueFilter { allow_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 9_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 23_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 37_u32, id_list: vec![44_i32] }, deny_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 16_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 30_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 44_u32, id_list: vec![51_i32] } }))
    });
    // IDXGIInfoQueue::PopStorageFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::pop_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push("IDXGIInfoQueue::PopStorageFilter -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::PopStorageFilter on InfoQueue");
    // IDXGIInfoQueue::GetStorageFilterStackSize on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::get_storage_filter_stack_size(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::GetStorageFilterStackSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetStorageFilterStackSize on InfoQueue",
    );
    // IDXGIInfoQueue::AddRetrievalFilterEntries on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::add_retrieval_filter_entries(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, &directx_graphics_infrastructure::structures::InfoQueueFilter { allow_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 9_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 23_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 37_u32, id_list: vec![44_i32] }, deny_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 16_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 30_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 44_u32, id_list: vec![51_i32] } }))
    });
    // IDXGIInfoQueue::GetRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::InfoQueue::get_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                Some(&lying),
            ),
        )
    });
    // IDXGIInfoQueue::ClearRetrievalFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::clear_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push("IDXGIInfoQueue::ClearRetrievalFilter -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::ClearRetrievalFilter on InfoQueue");
    // IDXGIInfoQueue::PushEmptyRetrievalFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_empty_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushEmptyRetrievalFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::PushEmptyRetrievalFilter on InfoQueue",
    );
    // IDXGIInfoQueue::PushDenyAllRetrievalFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_deny_all_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushDenyAllRetrievalFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::PushDenyAllRetrievalFilter on InfoQueue",
    );
    // IDXGIInfoQueue::PushCopyOfRetrievalFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::push_copy_of_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::PushCopyOfRetrievalFilter -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::PushCopyOfRetrievalFilter on InfoQueue",
    );
    // IDXGIInfoQueue::PushRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::InfoQueue::push_retrieval_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, &directx_graphics_infrastructure::structures::InfoQueueFilter { allow_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 9_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 23_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 37_u32, id_list: vec![44_i32] }, deny_list: directx_graphics_infrastructure::structures::InfoQueueFilterDesc { num_categories: 16_u32, category_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS], num_severities: 30_u32, severity_list: vec![directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR], num_i_ds: 44_u32, id_list: vec![51_i32] } }))
    });
    // IDXGIInfoQueue::PopRetrievalFilter on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::pop_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push("IDXGIInfoQueue::PopRetrievalFilter -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::PopRetrievalFilter on InfoQueue");
    // IDXGIInfoQueue::GetRetrievalFilterStackSize on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::InfoQueue::get_retrieval_filter_stack_size(
                    this,
                    component_object_model::structures::Guid {
                        data1: 84_u32,
                        data2: 91_u16,
                        data3: 98_u16,
                        data4: core::array::from_fn(|_| 105_u8),
                    },
                );
            answers.push(format!(
                "IDXGIInfoQueue::GetRetrievalFilterStackSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIInfoQueue::GetRetrievalFilterStackSize on InfoQueue",
    );
    // IDXGIInfoQueue::AddMessage on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::InfoQueue::add_message(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, 101_i32, c"known"); answers.push(format!("IDXGIInfoQueue::AddMessage -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::AddMessage on InfoQueue");
    // IDXGIInfoQueue::AddApplicationMessage on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::InfoQueue::add_application_message(this, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, c"known"); answers.push(format!("IDXGIInfoQueue::AddApplicationMessage -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::AddApplicationMessage on InfoQueue");
    // IDXGIInfoQueue::SetBreakOnCategory on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::InfoQueue::set_break_on_category(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS, false); answers.push(format!("IDXGIInfoQueue::SetBreakOnCategory -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::SetBreakOnCategory on InfoQueue");
    // IDXGIInfoQueue::SetBreakOnSeverity on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::InfoQueue::set_break_on_severity(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, false); answers.push(format!("IDXGIInfoQueue::SetBreakOnSeverity -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::SetBreakOnSeverity on InfoQueue");
    // IDXGIInfoQueue::SetBreakOnID on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::InfoQueue::set_break_on_id(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                101_i32,
                false,
            );
            answers.push(format!(
                "IDXGIInfoQueue::SetBreakOnID -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::SetBreakOnID on InfoQueue");
    // IDXGIInfoQueue::GetBreakOnCategory on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_break_on_category(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS); answers.push(format!("IDXGIInfoQueue::GetBreakOnCategory -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetBreakOnCategory on InfoQueue");
    // IDXGIInfoQueue::GetBreakOnSeverity on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        answered({ let answer = directx_graphics_infrastructure::InfoQueue::get_break_on_severity(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR); answers.push(format!("IDXGIInfoQueue::GetBreakOnSeverity -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetBreakOnSeverity on InfoQueue");
    // IDXGIInfoQueue::GetBreakOnID on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::get_break_on_id(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                101_i32,
            );
            answers.push(format!(
                "IDXGIInfoQueue::GetBreakOnID -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetBreakOnID on InfoQueue");
    // IDXGIInfoQueue::SetMuteDebugOutput on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::set_mute_debug_output(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                true,
            );
            answers.push("IDXGIInfoQueue::SetMuteDebugOutput -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::SetMuteDebugOutput on InfoQueue");
    // IDXGIInfoQueue::GetMuteDebugOutput on InfoQueue
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::InfoQueue::get_mute_debug_output(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            answers.push(format!(
                "IDXGIInfoQueue::GetMuteDebugOutput -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIInfoQueue::GetMuteDebugOutput on InfoQueue");
    // IDXGIKeyedMutex::AcquireSync on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::KeyedMutex::acquire_sync(this, 35_u64, 45_u32);
            answers.push(format!(
                "IDXGIKeyedMutex::AcquireSync -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIKeyedMutex::AcquireSync on KeyedMutex");
    // IDXGIKeyedMutex::ReleaseSync on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::KeyedMutex::release_sync(this, 35_u64);
            answers.push(format!(
                "IDXGIKeyedMutex::ReleaseSync -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIKeyedMutex::ReleaseSync on KeyedMutex");
    // IDXGIDeviceSubObject::GetDevice on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIDeviceSubObject::GetDevice -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDeviceSubObject::GetDevice on KeyedMutex");
    // IDXGIObject::SetPrivateData on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on KeyedMutex");
    // IDXGIObject::SetPrivateDataInterface on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on KeyedMutex");
    // IDXGIObject::GetPrivateData on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on KeyedMutex
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on KeyedMutex");
    // IDXGIOutput6::GetDesc1 on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output6::get_desc1(this);
            answers.push(format!(
                "IDXGIOutput6::GetDesc1 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput6::GetDesc1 on Output");
    // IDXGIOutput6::CheckHardwareCompositionSupport on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Output6::check_hardware_composition_support(this);
            answers.push(format!(
                "IDXGIOutput6::CheckHardwareCompositionSupport -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutput6::CheckHardwareCompositionSupport on Output",
    );
    // IDXGIOutput5::DuplicateOutput1 on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Output5::duplicate_output1(this, v1, 31_u32, &[directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS]); answers.push(format!("IDXGIOutput5::DuplicateOutput1 -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "IDXGIOutput5::DuplicateOutput1 on Output");
    // IDXGIOutput4::CheckOverlayColorSpaceSupport on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Output4::check_overlay_color_space_support(this, directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709, v1); answers.push(format!("IDXGIOutput4::CheckOverlayColorSpaceSupport -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutput4::CheckOverlayColorSpaceSupport on Output",
    );
    // IDXGIOutput3::CheckOverlaySupport on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k67.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k68.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k69.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k70.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k71.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k72.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k73.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k74.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k75.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k76.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output3::check_overlay_support(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                v1,
            );
            answers.push(format!(
                "IDXGIOutput3::CheckOverlaySupport -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput3::CheckOverlaySupport on Output");
    // IDXGIOutput2::SupportsOverlays on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Output2::supports_overlays(this);
            answers.push(format!(
                "IDXGIOutput2::SupportsOverlays -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput2::SupportsOverlays on Output");
    // IDXGIOutput1::GetDisplayModeList1 on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output1::get_display_mode_list1(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                directx_graphics_infrastructure::enumerations::DXGI_ENUM_MODES_INTERLACED,
            );
            answers.push(format!(
                "IDXGIOutput1::GetDisplayModeList1 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput1::GetDisplayModeList1 on Output");
    // IDXGIOutput1::FindClosestMatchingMode1 on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Output1::find_closest_matching_mode1(this, &directx_graphics_infrastructure::structures::ModeDesc1 { width: 4_u32, height: 11_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 25_u32, denominator: 32_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, stereo: true }, None); answers.push(format!("IDXGIOutput1::FindClosestMatchingMode1 -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIOutput1::FindClosestMatchingMode1 on Output");
    // IDXGIOutput1::GetDisplaySurfaceData1 on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Output1::get_display_surface_data1(this, v1);
            answers.push(format!(
                "IDXGIOutput1::GetDisplaySurfaceData1 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput1::GetDisplaySurfaceData1 on Output");
    // IDXGIOutput1::DuplicateOutput on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k67.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k68.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k69.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k70.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k71.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k72.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k73.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k74.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k75.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k76.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output1::duplicate_output(this, v1);
            answers.push(format!(
                "IDXGIOutput1::DuplicateOutput -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput1::DuplicateOutput on Output");
    // IDXGIOutput::GetDesc on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::get_desc(this);
            answers.push(format!(
                "IDXGIOutput::GetDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetDesc on Output");
    // IDXGIOutput::GetDisplayModeList on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::get_display_mode_list(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                directx_graphics_infrastructure::enumerations::DXGI_ENUM_MODES_INTERLACED,
            );
            answers.push(format!(
                "IDXGIOutput::GetDisplayModeList -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetDisplayModeList on Output");
    // IDXGIOutput::FindClosestMatchingMode on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::Output::find_closest_matching_mode(this, &directx_graphics_infrastructure::structures::ModeDesc { width: 4_u32, height: 11_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 25_u32, denominator: 32_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, None); answers.push(format!("IDXGIOutput::FindClosestMatchingMode -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGIOutput::FindClosestMatchingMode on Output");
    // IDXGIOutput::WaitForVBlank on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::wait_for_v_blank(this);
            answers.push(format!(
                "IDXGIOutput::WaitForVBlank -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::WaitForVBlank on Output");
    // IDXGIOutput::TakeOwnership on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
        } else if let Some(x) = o.k62.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
        } else if let Some(x) = o.k63.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
        } else if let Some(x) = o.k64.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Device(x))
        } else if let Some(x) = o.k65.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ))
        } else if let Some(x) = o.k66.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
        } else if let Some(x) = o.k67.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
        } else if let Some(x) = o.k68.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k69.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
        } else if let Some(x) = o.k70.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Output(x))
        } else if let Some(x) = o.k71.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
        } else if let Some(x) = o.k72.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
        } else if let Some(x) = o.k73.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
        } else if let Some(x) = o.k74.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
        } else if let Some(x) = o.k75.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ))
        } else if let Some(x) = o.k76.as_ref() {
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::take_ownership(this, v1, true);
            answers.push(format!(
                "IDXGIOutput::TakeOwnership -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::TakeOwnership on Output");
    // IDXGIOutput::ReleaseOwnership on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::Output::release_ownership(this);
            answers.push("IDXGIOutput::ReleaseOwnership -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::ReleaseOwnership on Output");
    // IDXGIOutput::GetGammaControlCapabilities on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Output::get_gamma_control_capabilities(this);
            answers.push(format!(
                "IDXGIOutput::GetGammaControlCapabilities -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetGammaControlCapabilities on Output");
    // IDXGIOutput::SetGammaControl on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::set_gamma_control(
                this,
                &directx_graphics_infrastructure::structures::GammaControl {
                    scale: directx_graphics_infrastructure::structures::Rgb {
                        red: 126.5_f32,
                        green: 7.5_f32,
                        blue: 14.5_f32,
                    },
                    offset: directx_graphics_infrastructure::structures::Rgb {
                        red: 7.5_f32,
                        green: 14.5_f32,
                        blue: 21.5_f32,
                    },
                    gamma_curve: core::array::from_fn(|_| {
                        directx_graphics_infrastructure::structures::Rgb {
                            red: 14.5_f32,
                            green: 21.5_f32,
                            blue: 28.5_f32,
                        }
                    }),
                },
            );
            answers.push(format!(
                "IDXGIOutput::SetGammaControl -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::SetGammaControl on Output");
    // IDXGIOutput::GetGammaControl on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::get_gamma_control(this);
            answers.push(format!(
                "IDXGIOutput::GetGammaControl -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetGammaControl on Output");
    // IDXGIOutput::SetDisplaySurface on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::set_display_surface(this, v1);
            answers.push(format!(
                "IDXGIOutput::SetDisplaySurface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::SetDisplaySurface on Output");
    // IDXGIOutput::GetDisplaySurfaceData on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::Output::get_display_surface_data(this, v1);
            answers.push(format!(
                "IDXGIOutput::GetDisplaySurfaceData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetDisplaySurfaceData on Output");
    // IDXGIOutput::GetFrameStatistics on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Output::get_frame_statistics(this);
            answers.push(format!(
                "IDXGIOutput::GetFrameStatistics -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIOutput::GetFrameStatistics on Output");
    // IDXGIObject::SetPrivateData on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Output");
    // IDXGIObject::SetPrivateDataInterface on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Output");
    // IDXGIObject::GetPrivateData on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Output
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Output");
    // IDXGIOutputDuplication::GetDesc on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::OutputDuplication::get_desc(this);
            answers.push(format!(
                "IDXGIOutputDuplication::GetDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::GetDesc on OutputDuplication",
    );
    // IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                this, 24_u32,
            );
            answers.push(format!(
                "IDXGIOutputDuplication::AcquireNextFrame -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", {
                        let (t1_0, t1_1) = v0;
                        format!("({}, {})", format!("{:?}", t1_0), "an object".to_string())
                    }),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication",
    );
    // IDXGIOutputDuplication::GetFrameDirtyRects on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative({
            let answer = directx_graphics_infrastructure::OutputDuplication::get_frame_dirty_rects(
                this, &lying,
            );
            answers.push(format!(
                "IDXGIOutputDuplication::GetFrameDirtyRects -> {}",
                format!("{:?}", &answer)
            ));
            answers.bytes("IDXGIOutputDuplication::GetFrameDirtyRects", &lying);
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::GetFrameDirtyRects on OutputDuplication",
    );
    // IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative({
            let answer = directx_graphics_infrastructure::OutputDuplication::get_frame_move_rects(
                this, &lying,
            );
            answers.push(format!(
                "IDXGIOutputDuplication::GetFrameMoveRects -> {}",
                format!("{:?}", &answer)
            ));
            answers.bytes("IDXGIOutputDuplication::GetFrameMoveRects", &lying);
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication",
    );
    // IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::OutputDuplication::get_frame_pointer_shape(
                    this, &lying,
                );
            answers.push(format!(
                "IDXGIOutputDuplication::GetFramePointerShape -> {}",
                format!("{:?}", &answer)
            ));
            answers.bytes("IDXGIOutputDuplication::GetFramePointerShape", &lying);
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication",
    );
    // IDXGIOutputDuplication::MapDesktopSurface on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::map_desktop_surface(this),
        )
    });
    // IDXGIOutputDuplication::UnMapDesktopSurface on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::un_map_desktop_surface(this),
        )
    });
    // IDXGIOutputDuplication::ReleaseFrame on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::OutputDuplication::release_frame(this);
            answers.push(format!(
                "IDXGIOutputDuplication::ReleaseFrame -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIOutputDuplication::ReleaseFrame on OutputDuplication",
    );
    // IDXGIObject::SetPrivateData on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on OutputDuplication");
    // IDXGIObject::SetPrivateDataInterface on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIObject::SetPrivateDataInterface on OutputDuplication",
    );
    // IDXGIObject::GetPrivateData on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on OutputDuplication
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on OutputDuplication");
    // IDXGIResource1::CreateSubresourceSurface on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this, 35_u32,
            );
            answers.push(format!(
                "IDXGIResource1::CreateSubresourceSurface -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGIResource1::CreateSubresourceSurface on Resource",
    );
    // IDXGIResource1::CreateSharedHandle on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_shared_handle(
                this,
                Some(&component_object_model::structures::SecurityAttributes {
                    n_length: 1_u32,
                    lp_security_descriptor: vec![8_u8],
                    b_inherit_handle: true,
                }),
                93_u32,
                Some(&[95_u16]),
            ),
        )
    });
    // IDXGIResource::GetSharedHandle on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Resource::get_shared_handle(this);
            answers.push(format!(
                "IDXGIResource::GetSharedHandle -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIResource::GetSharedHandle on Resource");
    // IDXGIResource::GetUsage on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Resource::get_usage(this);
            answers.push(format!(
                "IDXGIResource::GetUsage -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIResource::GetUsage on Resource");
    // IDXGIResource::SetEvictionPriority on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_RESOURCE_PRIORITY_MINIMUM,
            );
            answers.push(format!(
                "IDXGIResource::SetEvictionPriority -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIResource::SetEvictionPriority on Resource");
    // IDXGIResource::GetEvictionPriority on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Resource::get_eviction_priority(this);
            answers.push(format!(
                "IDXGIResource::GetEvictionPriority -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIResource::GetEvictionPriority on Resource");
    // IDXGIDeviceSubObject::GetDevice on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIDeviceSubObject::GetDevice -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDeviceSubObject::GetDevice on Resource");
    // IDXGIObject::SetPrivateData on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Resource");
    // IDXGIObject::SetPrivateDataInterface on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Resource");
    // IDXGIObject::GetPrivateData on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Resource
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Resource");
    // IDXGISurface2::GetResource on Surface
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Surface2::get_resource(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGISurface2::GetResource -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", {
                        let (t1_0, t1_1) = v0;
                        format!("({}, {})", "an object".to_string(), format!("{:?}", t1_1))
                    }),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISurface2::GetResource on Surface");
    // IDXGISurface1::GetDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::get_dc(
            this, true,
        ))
    });
    // IDXGISurface1::ReleaseDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::release_dc(
            this,
            Some(&component_object_model::structures::Rect {
                left: 1_i32,
                top: 8_i32,
                right: 15_i32,
                bottom: 22_i32,
            }),
        ))
    });
    // IDXGISurface::GetDesc on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::get_desc(this))
    });
    // IDXGISurface::Map on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::map(
            this,
            directx_graphics_infrastructure::enumerations::DXGI_MAP_READ,
        ))
    });
    // IDXGISurface::Unmap on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::unmap(this))
    });
    // IDXGIDeviceSubObject::GetDevice on Surface
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIDeviceSubObject::GetDevice -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDeviceSubObject::GetDevice on Surface");
    // IDXGIObject::SetPrivateData on Surface
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on Surface");
    // IDXGIObject::SetPrivateDataInterface on Surface
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on Surface");
    // IDXGIObject::GetPrivateData on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Surface
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on Surface");
    // IDXGISwapChain4::SetHDRMetaData on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain4::set_hdr_meta_data(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_HDR_METADATA_TYPE_HDR10,
                Some(&[45_u8]),
            );
            answers.push(format!(
                "IDXGISwapChain4::SetHDRMetaData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain4::SetHDRMetaData on SwapChain");
    // IDXGISwapChain3::GetCurrentBackBufferIndex on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::SwapChain3::get_current_back_buffer_index(this);
            answers.push(format!(
                "IDXGISwapChain3::GetCurrentBackBufferIndex -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain3::GetCurrentBackBufferIndex on SwapChain",
    );
    // IDXGISwapChain3::CheckColorSpaceSupport on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::SwapChain3::check_color_space_support(this, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709); answers.push(format!("IDXGISwapChain3::CheckColorSpaceSupport -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain3::CheckColorSpaceSupport on SwapChain",
    );
    // IDXGISwapChain3::SetColorSpace1 on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::SwapChain3::set_color_space1(this, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709); answers.push(format!("IDXGISwapChain3::SetColorSpace1 -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGISwapChain3::SetColorSpace1 on SwapChain");
    // IDXGISwapChain3::ResizeBuffers1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain3::resize_buffers1(
                this,
                120_u32,
                44_u32,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                5_u32,
                &[117_u32],
                &[None],
            ),
        )
    });
    // IDXGISwapChain2::SetSourceSize on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::SwapChain2::set_source_size(this, 120_u32, 44_u32);
            answers.push(format!(
                "IDXGISwapChain2::SetSourceSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain2::SetSourceSize on SwapChain");
    // IDXGISwapChain2::GetSourceSize on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain2::get_source_size(this);
            answers.push(format!(
                "IDXGISwapChain2::GetSourceSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain2::GetSourceSize on SwapChain");
    // IDXGISwapChain2::SetMaximumFrameLatency on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain2::set_maximum_frame_latency(
                this, 63_u32,
            );
            answers.push(format!(
                "IDXGISwapChain2::SetMaximumFrameLatency -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain2::SetMaximumFrameLatency on SwapChain",
    );
    // IDXGISwapChain2::GetMaximumFrameLatency on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::SwapChain2::get_maximum_frame_latency(this);
            answers.push(format!(
                "IDXGISwapChain2::GetMaximumFrameLatency -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain2::GetMaximumFrameLatency on SwapChain",
    );
    // IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::SwapChain2::get_frame_latency_waitable_object(
                    this,
                );
            answers.push(format!(
                "IDXGISwapChain2::GetFrameLatencyWaitableObject -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain",
    );
    // IDXGISwapChain2::SetMatrixTransform on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain2::set_matrix_transform(
                this,
                &directx_graphics_infrastructure::structures::Matrix3x2F {
                    _11: 3.5_f32,
                    _12: 10.5_f32,
                    _21: 17.5_f32,
                    _22: 24.5_f32,
                    _31: 31.5_f32,
                    _32: 38.5_f32,
                },
            );
            answers.push(format!(
                "IDXGISwapChain2::SetMatrixTransform -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain2::SetMatrixTransform on SwapChain");
    // IDXGISwapChain2::GetMatrixTransform on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain2::get_matrix_transform(this);
            answers.push(format!(
                "IDXGISwapChain2::GetMatrixTransform -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain2::GetMatrixTransform on SwapChain");
    // IDXGISwapChain1::GetDesc1 on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_desc1(this);
            answers.push(format!(
                "IDXGISwapChain1::GetDesc1 -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetDesc1 on SwapChain");
    // IDXGISwapChain1::GetFullscreenDesc on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_fullscreen_desc(this);
            answers.push(format!(
                "IDXGISwapChain1::GetFullscreenDesc -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetFullscreenDesc on SwapChain");
    // IDXGISwapChain1::GetHwnd on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_hwnd(this);
            answers.push(format!(
                "IDXGISwapChain1::GetHwnd -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetHwnd on SwapChain");
    // IDXGISwapChain1::GetCoreWindow on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                this,
                &component_object_model::structures::Guid {
                    data1: 11_u32,
                    data2: 18_u16,
                    data3: 25_u16,
                    data4: core::array::from_fn(|_| 32_u8),
                },
            );
            answers.push(format!(
                "IDXGISwapChain1::GetCoreWindow -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetCoreWindow on SwapChain");
    // IDXGISwapChain1::Present1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::present1(
            this,
            46_u32,
            directx_graphics_infrastructure::enumerations::DXGI_PRESENT_TEST,
            &directx_graphics_infrastructure::structures::PresentParameters {
                dirty_rects_count: 123_u32,
                dirty_rects: vec![component_object_model::structures::Rect {
                    left: 11_i32,
                    top: 18_i32,
                    right: 25_i32,
                    bottom: 32_i32,
                }],
                scroll_rect: vec![component_object_model::structures::Rect {
                    left: 18_i32,
                    top: 25_i32,
                    right: 32_i32,
                    bottom: 39_i32,
                }],
                scroll_offset: vec![component_object_model::structures::Point {
                    x: 25_i32,
                    y: 32_i32,
                }],
            },
        ))
    });
    // IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered({
            let answer =
                directx_graphics_infrastructure::SwapChain1::is_temporary_mono_supported(this);
            answers.push(format!(
                "IDXGISwapChain1::IsTemporaryMonoSupported -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain",
    );
    // IDXGISwapChain1::GetRestrictToOutput on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_restrict_to_output(this);
            answers.push(format!(
                "IDXGISwapChain1::GetRestrictToOutput -> {}",
                match &answer {
                    Ok(v0) => format!(
                        "Ok({})",
                        match v0 {
                            Some(v1) => format!("Some({})", "an object".to_string()),
                            None => "None".to_string(),
                        }
                    ),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetRestrictToOutput on SwapChain");
    // IDXGISwapChain1::SetBackgroundColor on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::set_background_color(
                this,
                &directx_graphics_infrastructure::structures::Rgba {
                    r: 79.5_f32,
                    g: 86.5_f32,
                    b: 93.5_f32,
                    a: 100.5_f32,
                },
            );
            answers.push(format!(
                "IDXGISwapChain1::SetBackgroundColor -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::SetBackgroundColor on SwapChain");
    // IDXGISwapChain1::GetBackgroundColor on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_background_color(this);
            answers.push(format!(
                "IDXGISwapChain1::GetBackgroundColor -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetBackgroundColor on SwapChain");
    // IDXGISwapChain1::SetRotation on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::set_rotation(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_MODE_ROTATION_IDENTITY,
            );
            answers.push(format!(
                "IDXGISwapChain1::SetRotation -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::SetRotation on SwapChain");
    // IDXGISwapChain1::GetRotation on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain1::get_rotation(this);
            answers.push(format!(
                "IDXGISwapChain1::GetRotation -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain1::GetRotation on SwapChain");
    // IDXGISwapChain::Present on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::present(
                this,
                46_u32,
                directx_graphics_infrastructure::enumerations::DXGI_PRESENT_TEST,
            );
            answers.push(format!(
                "IDXGISwapChain::Present -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::Present on SwapChain");
    // IDXGISwapChain::GetBuffer on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::get_buffer(this, 0_u32);
            answers.push(format!(
                "IDXGISwapChain::GetBuffer -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::GetBuffer on SwapChain");
    // IDXGISwapChain::SetFullscreenState on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::set_fullscreen_state(
                this,
                true,
                o.k70.as_ref(),
            );
            answers.push(format!(
                "IDXGISwapChain::SetFullscreenState -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::SetFullscreenState on SwapChain");
    // IDXGISwapChain::GetFullscreenState on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this);
            answers.push(format!(
                "IDXGISwapChain::GetFullscreenState -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", {
                        let (t1_0, t1_1) = v0;
                        format!("({}, {})", format!("{:?}", t1_0), "an object".to_string())
                    }),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::GetFullscreenState on SwapChain");
    // IDXGISwapChain::GetDesc on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_desc(this))
    });
    // IDXGISwapChain::ResizeBuffers on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::resize_buffers(
                this,
                107_u32,
                120_u32,
                44_u32,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                5_u32,
            );
            answers.push(format!(
                "IDXGISwapChain::ResizeBuffers -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::ResizeBuffers on SwapChain");
    // IDXGISwapChain::ResizeTarget on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        failure_is_negative({ let answer = directx_graphics_infrastructure::SwapChain::resize_target(this, &directx_graphics_infrastructure::structures::ModeDesc { width: 118_u32, height: 125_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 13_u32, denominator: 20_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }); answers.push(format!("IDXGISwapChain::ResizeTarget -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "IDXGISwapChain::ResizeTarget on SwapChain");
    // IDXGISwapChain::GetContainingOutput on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::get_containing_output(this);
            answers.push(format!(
                "IDXGISwapChain::GetContainingOutput -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::GetContainingOutput on SwapChain");
    // IDXGISwapChain::GetFrameStatistics on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::get_frame_statistics(this);
            answers.push(format!(
                "IDXGISwapChain::GetFrameStatistics -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::GetFrameStatistics on SwapChain");
    // IDXGISwapChain::GetLastPresentCount on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::SwapChain::get_last_present_count(this);
            answers.push(format!(
                "IDXGISwapChain::GetLastPresentCount -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGISwapChain::GetLastPresentCount on SwapChain");
    // IDXGIDeviceSubObject::GetDevice on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIDeviceSubObject::GetDevice -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIDeviceSubObject::GetDevice on SwapChain");
    // IDXGIObject::SetPrivateData on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateData -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateData on SwapChain");
    // IDXGIObject::SetPrivateDataInterface on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                None,
            );
            answers.push(format!(
                "IDXGIObject::SetPrivateDataInterface -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::SetPrivateDataInterface on SwapChain");
    // IDXGIObject::GetPrivateData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &component_object_model::structures::Guid {
                data1: 5_u32,
                data2: 12_u16,
                data3: 19_u16,
                data4: core::array::from_fn(|_| 26_u8),
            },
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on SwapChain
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            answers.push(format!(
                "IDXGIObject::GetParent -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "IDXGIObject::GetParent on SwapChain");
    // IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k75.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::SwapChainMedia::get_frame_statistics_media(this);
            answers.push(format!(
                "IDXGISwapChainMedia::GetFrameStatisticsMedia -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia",
    );
    // IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k75.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::SwapChainMedia::set_present_duration(this, 59_u32);
            answers.push(format!(
                "IDXGISwapChainMedia::SetPresentDuration -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia",
    );
    // IDXGISwapChainMedia::CheckPresentDurationSupport on SwapChainMedia
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k75.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::SwapChainMedia::check_present_duration_support(
                    this, 101_u32,
                );
            answers.push(format!(
                "IDXGISwapChainMedia::CheckPresentDurationSupport -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGISwapChainMedia::CheckPresentDurationSupport on SwapChainMedia",
    );
    // IDXGraphicsAnalysis::BeginCapture on GraphicsAnalysis
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k76.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::GraphicsAnalysis::begin_capture(this);
            answers.push("IDXGraphicsAnalysis::BeginCapture -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(
        noted,
        "IDXGraphicsAnalysis::BeginCapture on GraphicsAnalysis",
    );
    // IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k76.as_ref() else {
            return false;
        };
        answered({
            let answer = directx_graphics_infrastructure::GraphicsAnalysis::end_capture(this);
            answers.push("IDXGraphicsAnalysis::EndCapture -> nothing".to_string());
            answer
        })
    });
    answers.not_called_since(noted, "IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis");
    // CreateDXGIFactory
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({
            let answer =
                directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(
                    &o.api,
                    &component_object_model::structures::Guid {
                        data1: 3_u32,
                        data2: 10_u16,
                        data3: 17_u16,
                        data4: core::array::from_fn(|_| 24_u8),
                    },
                );
            answers.push(format!(
                "CreateDXGIFactory -> {}",
                match &answer {
                    Ok(v0) => format!("Ok({})", "an object".to_string()),
                    Err(v0) => format!("Err({:#x})", v0.0),
                }
            ));
            answer
        })
    });
    answers.not_called_since(noted, "CreateDXGIFactory");
    // CreateDXGIFactory1
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({ let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) }); answers.push(format!("CreateDXGIFactory1 -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "CreateDXGIFactory1");
    // CreateDXGIFactory2
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({ let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) }); answers.push(format!("CreateDXGIFactory2 -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "CreateDXGIFactory2");
    // DXGIDeclareAdapterRemovalSupport
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({ let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_declare_adapter_removal_support(&o.api); answers.push(format!("DXGIDeclareAdapterRemovalSupport -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "DXGIDeclareAdapterRemovalSupport");
    // DXGIDisableVBlankVirtualization
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({ let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_disable_v_blank_virtualization(&o.api); answers.push(format!("DXGIDisableVBlankVirtualization -> {}", format!("{:?}", &answer))); answer })
    });
    answers.not_called_since(noted, "DXGIDisableVBlankVirtualization");
    // DXGIGetDebugInterface1
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        failure_is_negative({ let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) }); answers.push(format!("DXGIGetDebugInterface1 -> {}", match &answer { Ok(v0) => format!("Ok({})", "an object".to_string()), Err(v0) => format!("Err({:#x})", v0.0) })); answer })
    });
    answers.not_called_since(noted, "DXGIGetDebugInterface1");
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 232;

/// The number of answers the suite notes and compares: each answer of a method both glues carry to the
/// implementation, and the caller's bytes after each such method that writes them.
const COMPARED: usize = 202;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    api: A,
) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o, &Answers::default());
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

/// Makes an object of every kind and drives every method as `every_method_answers` does, and returns what each
/// method both glues carry to the implementation answered, in the order called.
pub fn answers_of<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    api: A,
) -> Vec<String> {
    let mut o = Objects::new(api);
    make_all(&mut o);
    let answers = Answers::default();
    drive(&o, &answers);
    answers.lines()
}

/// Drives `api` and `reference` alike and compares every answer noted, field by field, in the order called: `api` is
/// `reference` served and called through both glues, so each answer must be what the implementation gave.
pub fn every_answer_equals_the_reference<
    A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
    R: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>(
    api: A,
    reference: R,
) {
    let got = answers_of(api);
    let wanted = answers_of(reference);
    println!(
        "ANSWERS directx-graphics-infrastructure: the reference answered {} of {COMPARED}, through both glues {}",
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
pub fn answers_received_of<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
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
    A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
    R: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
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
        "ANSWERS AND ARGUMENTS directx-graphics-infrastructure: {} lines directly, {} through both glues; {} methods differing as src/api.rs SUITE_DIFFERENT states: {accepted:?}",
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
