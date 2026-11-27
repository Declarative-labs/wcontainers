//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The conformance suite of direct3d-11, written against the traits alone (src/glue_hostile.rs): an object of every kind
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
use wcontainers_direct3d_11 as direct3d_11;
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
struct Objects<A: direct3d_11::Direct3D11> {
    api: A,
    k0: Option<<A as direct3d_common::Direct3DCommon>::Blob>,
    k1: Option<<A as direct3d_common::Direct3DCommon>::DestructionNotifier>,
    k2: Option<<A as direct3d_11::Direct3D11>::AuthenticatedChannel>,
    k3: Option<<A as direct3d_11::Direct3D11>::BlendState>,
    k4: Option<<A as direct3d_11::Direct3D11>::Buffer>,
    k5: Option<<A as direct3d_11::Direct3D11>::ClassInstance>,
    k6: Option<<A as direct3d_11::Direct3D11>::ClassLinkage>,
    k7: Option<<A as direct3d_11::Direct3D11>::CommandList>,
    k8: Option<<A as direct3d_11::Direct3D11>::ComputeShader>,
    k9: Option<<A as direct3d_11::Direct3D11>::Counter>,
    k10: Option<<A as direct3d_11::Direct3D11>::CryptoSession>,
    k11: Option<<A as direct3d_11::Direct3D11>::Debug>,
    k12: Option<<A as direct3d_11::Direct3D11>::DepthStencilState>,
    k13: Option<<A as direct3d_11::Direct3D11>::DepthStencilView>,
    k14: Option<<A as direct3d_11::Direct3D11>::Device>,
    k15: Option<<A as direct3d_11::Direct3D11>::DeviceContext>,
    k16: Option<<A as direct3d_11::Direct3D11>::DomainShader>,
    k17: Option<<A as direct3d_11::Direct3D11>::Fence>,
    k18: Option<<A as direct3d_11::Direct3D11>::GeometryShader>,
    k19: Option<<A as direct3d_11::Direct3D11>::HullShader>,
    k20: Option<<A as direct3d_11::Direct3D11>::InfoQueue>,
    k21: Option<<A as direct3d_11::Direct3D11>::InputLayout>,
    k22: Option<<A as direct3d_11::Direct3D11>::Multithread>,
    k23: Option<<A as direct3d_11::Direct3D11>::PixelShader>,
    k24: Option<<A as direct3d_11::Direct3D11>::Predicate>,
    k25: Option<<A as direct3d_11::Direct3D11>::Query>,
    k26: Option<<A as direct3d_11::Direct3D11>::RasterizerState>,
    k27: Option<<A as direct3d_11::Direct3D11>::RefDefaultTrackingOptions>,
    k28: Option<<A as direct3d_11::Direct3D11>::RefTrackingOptions>,
    k29: Option<<A as direct3d_11::Direct3D11>::RenderTargetView>,
    k30: Option<<A as direct3d_11::Direct3D11>::SamplerState>,
    k31: Option<<A as direct3d_11::Direct3D11>::ShaderResourceView>,
    k32: Option<<A as direct3d_11::Direct3D11>::ShaderTrace>,
    k33: Option<<A as direct3d_11::Direct3D11>::ShaderTraceFactory>,
    k34: Option<<A as direct3d_11::Direct3D11>::SwitchToRef>,
    k35: Option<<A as direct3d_11::Direct3D11>::Texture1D>,
    k36: Option<<A as direct3d_11::Direct3D11>::Texture2D>,
    k37: Option<<A as direct3d_11::Direct3D11>::Texture3D>,
    k38: Option<<A as direct3d_11::Direct3D11>::TracingDevice>,
    k39: Option<<A as direct3d_11::Direct3D11>::UnorderedAccessView>,
    k40: Option<<A as direct3d_11::Direct3D11>::VertexShader>,
    k41: Option<<A as direct3d_11::Direct3D11>::VideoContext>,
    k42: Option<<A as direct3d_11::Direct3D11>::VideoDecoder>,
    k43: Option<<A as direct3d_11::Direct3D11>::VideoDecoderOutputView>,
    k44: Option<<A as direct3d_11::Direct3D11>::VideoDevice>,
    k45: Option<<A as direct3d_11::Direct3D11>::VideoProcessor>,
    k46: Option<<A as direct3d_11::Direct3D11>::VideoProcessorEnumerator>,
    k47: Option<<A as direct3d_11::Direct3D11>::VideoProcessorInputView>,
    k48: Option<<A as direct3d_11::Direct3D11>::VideoProcessorOutputView>,
    k49: Option<<A as direct3d_11::Direct3D11>::DeviceContextState>,
    k50: Option<<A as direct3d_11::Direct3D11>::UserDefinedAnnotation>,
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

impl<A: direct3d_11::Direct3D11> Objects<A> {
    fn new(api: A) -> Objects<A> {
        Objects {
            api,
            k0: None,
            k1: None,
            k2: None,
            k3: None,
            k4: None,
            k5: None,
            k6: None,
            k7: None,
            k8: None,
            k9: None,
            k10: None,
            k11: None,
            k12: None,
            k13: None,
            k14: None,
            k15: None,
            k16: None,
            k17: None,
            k18: None,
            k19: None,
            k20: None,
            k21: None,
            k22: None,
            k23: None,
            k24: None,
            k25: None,
            k26: None,
            k27: None,
            k28: None,
            k29: None,
            k30: None,
            k31: None,
            k32: None,
            k33: None,
            k34: None,
            k35: None,
            k36: None,
            k37: None,
            k38: None,
            k39: None,
            k40: None,
            k41: None,
            k42: None,
            k43: None,
            k44: None,
            k45: None,
            k46: None,
            k47: None,
            k48: None,
            k49: None,
            k50: None,
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
            self.k0.is_some(),
            self.k1.is_some(),
            self.k2.is_some(),
            self.k3.is_some(),
            self.k4.is_some(),
            self.k5.is_some(),
            self.k6.is_some(),
            self.k7.is_some(),
            self.k8.is_some(),
            self.k9.is_some(),
            self.k10.is_some(),
            self.k11.is_some(),
            self.k12.is_some(),
            self.k13.is_some(),
            self.k14.is_some(),
            self.k15.is_some(),
            self.k16.is_some(),
            self.k17.is_some(),
            self.k18.is_some(),
            self.k19.is_some(),
            self.k20.is_some(),
            self.k21.is_some(),
            self.k22.is_some(),
            self.k23.is_some(),
            self.k24.is_some(),
            self.k25.is_some(),
            self.k26.is_some(),
            self.k27.is_some(),
            self.k28.is_some(),
            self.k29.is_some(),
            self.k30.is_some(),
            self.k31.is_some(),
            self.k32.is_some(),
            self.k33.is_some(),
            self.k34.is_some(),
            self.k35.is_some(),
            self.k36.is_some(),
            self.k37.is_some(),
            self.k38.is_some(),
            self.k39.is_some(),
            self.k40.is_some(),
            self.k41.is_some(),
            self.k42.is_some(),
            self.k43.is_some(),
            self.k44.is_some(),
            self.k45.is_some(),
            self.k46.is_some(),
            self.k47.is_some(),
            self.k48.is_some(),
            self.k49.is_some(),
            self.k50.is_some(),
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

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new.
fn make_all<A: direct3d_11::Direct3D11>(o: &mut Objects<A>) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
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
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k4.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
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
        // ID3D11ClassLinkage::GetClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k6.as_ref()?;
                    let answer = direct3d_11::ClassLinkage::get_class_instance(
                        this,
                        c"",
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11ClassLinkage::CreateClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k6.as_ref()?;
                    let answer = direct3d_11::ClassLinkage::create_class_instance(
                        this,
                        c"",
                        Default::default(),
                        Default::default(),
                        Default::default(),
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for AuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::AuthenticatedChannel(p0)) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for BlendState
        if o.k3.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::BlendState(p0)) = answer
                && o.k3.is_none()
            {
                o.k3 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Buffer
        if o.k4.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BUFFER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Buffer(p0)) = answer
                && o.k4.is_none()
            {
                o.k4 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassInstance(p0)) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassLinkage(p0)) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for CommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CommandList(p0)) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ComputeShader(p0)) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Counter
        if o.k9.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COUNTER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Counter(p0)) = answer
                && o.k9.is_none()
            {
                o.k9 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for CryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CryptoSession(p0)) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Debug
        if o.k11.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEBUG,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Debug(p0)) = answer
                && o.k11.is_none()
            {
                o.k11 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for DepthStencilState
        if o.k12.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilState(p0)) = answer
                && o.k12.is_none()
            {
                o.k12 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for DepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilView(p0)) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Device
        if o.k14.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE5,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Device(p0)) = answer
                && o.k14.is_none()
            {
                o.k14 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for DeviceContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContext(p0)) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for DomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DomainShader(p0)) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Fence
        if o.k17.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Fence(p0)) = answer
                && o.k17.is_none()
            {
                o.k17 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for GeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::GeometryShader(p0)) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for HullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_HULL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::HullShader(p0)) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for InfoQueue
        if o.k20.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InfoQueue(p0)) = answer
                && o.k20.is_none()
            {
                o.k20 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for InputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InputLayout(p0)) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Multithread
        if o.k22.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_MULTITHREAD,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Multithread(p0)) = answer
                && o.k22.is_none()
            {
                o.k22 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for PixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::PixelShader(p0)) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Predicate
        if o.k24.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PREDICATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Predicate(p0)) = answer
                && o.k24.is_none()
            {
                o.k24 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Query
        if o.k25.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_QUERY1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Query(p0)) = answer
                && o.k25.is_none()
            {
                o.k25 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for RasterizerState
        if o.k26.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RasterizerState(p0)) = answer
                && o.k26.is_none()
            {
                o.k26 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for RefDefaultTrackingOptions
        if o.k27.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefDefaultTrackingOptions(p0)) = answer
                && o.k27.is_none()
            {
                o.k27 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for RefTrackingOptions
        if o.k28.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefTrackingOptions(p0)) = answer
                && o.k28.is_none()
            {
                o.k28 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for RenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RenderTargetView(p0)) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for SamplerState
        if o.k30.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SamplerState(p0)) = answer
                && o.k30.is_none()
            {
                o.k30 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderResourceView(p0)) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ShaderTrace
        if o.k32.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTrace(p0)) = answer
                && o.k32.is_none()
            {
                o.k32 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for ShaderTraceFactory
        if o.k33.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTraceFactory(p0)) = answer
                && o.k33.is_none()
            {
                o.k33 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for SwitchToRef
        if o.k34.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SwitchToRef(p0)) = answer
                && o.k34.is_none()
            {
                o.k34 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Texture1D
        if o.k35.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture1D(p0)) = answer
                && o.k35.is_none()
            {
                o.k35 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Texture2D
        if o.k36.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture2D(p0)) = answer
                && o.k36.is_none()
            {
                o.k36 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for Texture3D
        if o.k37.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture3D(p0)) = answer
                && o.k37.is_none()
            {
                o.k37 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for TracingDevice
        if o.k38.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TRACING_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::TracingDevice(p0)) = answer
                && o.k38.is_none()
            {
                o.k38 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for UnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UnorderedAccessView(p0)) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VertexShader(p0)) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoContext
        if o.k41.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoContext(p0)) = answer
                && o.k41.is_none()
            {
                o.k41 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoDecoder
        if o.k42.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoder(p0)) = answer
                && o.k42.is_none()
            {
                o.k42 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoDecoderOutputView
        if o.k43.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoderOutputView(p0)) = answer
                && o.k43.is_none()
            {
                o.k43 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoDevice
        if o.k44.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDevice(p0)) = answer
                && o.k44.is_none()
            {
                o.k44 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessor(p0)) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoProcessorEnumerator
        if o.k46.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorEnumerator(p0)) = answer
                && o.k46.is_none()
            {
                o.k46 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoProcessorInputView
        if o.k47.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorInputView(p0)) = answer
                && o.k47.is_none()
            {
                o.k47 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for VideoProcessorOutputView
        if o.k48.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorOutputView(p0)) = answer
                && o.k48.is_none()
            {
                o.k48 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for DeviceContextState
        if o.k49.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContextState(p0)) = answer
                && o.k49.is_none()
            {
                o.k49 = Some(p0);
            }
        }
        // ID3D11Device5::OpenSharedFence for UserDefinedAnnotation
        if o.k50.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::open_shared_fence(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UserDefinedAnnotation(p0)) = answer
                && o.k50.is_none()
            {
                o.k50 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for AuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::AuthenticatedChannel(p0)) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for BlendState
        if o.k3.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::BlendState(p0)) = answer
                && o.k3.is_none()
            {
                o.k3 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Buffer
        if o.k4.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BUFFER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Buffer(p0)) = answer
                && o.k4.is_none()
            {
                o.k4 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassInstance(p0)) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassLinkage(p0)) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for CommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CommandList(p0)) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ComputeShader(p0)) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Counter
        if o.k9.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COUNTER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Counter(p0)) = answer
                && o.k9.is_none()
            {
                o.k9 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for CryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CryptoSession(p0)) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Debug
        if o.k11.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEBUG,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Debug(p0)) = answer
                && o.k11.is_none()
            {
                o.k11 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for DepthStencilState
        if o.k12.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilState(p0)) = answer
                && o.k12.is_none()
            {
                o.k12 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for DepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilView(p0)) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Device
        if o.k14.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE5,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Device(p0)) = answer
                && o.k14.is_none()
            {
                o.k14 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for DeviceContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContext(p0)) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for DomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DomainShader(p0)) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Fence
        if o.k17.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Fence(p0)) = answer
                && o.k17.is_none()
            {
                o.k17 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for GeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::GeometryShader(p0)) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for HullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_HULL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::HullShader(p0)) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for InfoQueue
        if o.k20.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InfoQueue(p0)) = answer
                && o.k20.is_none()
            {
                o.k20 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for InputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InputLayout(p0)) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Multithread
        if o.k22.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_MULTITHREAD,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Multithread(p0)) = answer
                && o.k22.is_none()
            {
                o.k22 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for PixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::PixelShader(p0)) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Predicate
        if o.k24.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PREDICATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Predicate(p0)) = answer
                && o.k24.is_none()
            {
                o.k24 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Query
        if o.k25.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_QUERY1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Query(p0)) = answer
                && o.k25.is_none()
            {
                o.k25 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for RasterizerState
        if o.k26.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RasterizerState(p0)) = answer
                && o.k26.is_none()
            {
                o.k26 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for RefDefaultTrackingOptions
        if o.k27.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefDefaultTrackingOptions(p0)) = answer
                && o.k27.is_none()
            {
                o.k27 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for RefTrackingOptions
        if o.k28.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefTrackingOptions(p0)) = answer
                && o.k28.is_none()
            {
                o.k28 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for RenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RenderTargetView(p0)) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for SamplerState
        if o.k30.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SamplerState(p0)) = answer
                && o.k30.is_none()
            {
                o.k30 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderResourceView(p0)) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ShaderTrace
        if o.k32.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTrace(p0)) = answer
                && o.k32.is_none()
            {
                o.k32 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for ShaderTraceFactory
        if o.k33.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTraceFactory(p0)) = answer
                && o.k33.is_none()
            {
                o.k33 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for SwitchToRef
        if o.k34.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SwitchToRef(p0)) = answer
                && o.k34.is_none()
            {
                o.k34 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Texture1D
        if o.k35.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture1D(p0)) = answer
                && o.k35.is_none()
            {
                o.k35 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Texture2D
        if o.k36.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture2D(p0)) = answer
                && o.k36.is_none()
            {
                o.k36 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for Texture3D
        if o.k37.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture3D(p0)) = answer
                && o.k37.is_none()
            {
                o.k37 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for TracingDevice
        if o.k38.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TRACING_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::TracingDevice(p0)) = answer
                && o.k38.is_none()
            {
                o.k38 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for UnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UnorderedAccessView(p0)) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VertexShader(p0)) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoContext
        if o.k41.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoContext(p0)) = answer
                && o.k41.is_none()
            {
                o.k41 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoDecoder
        if o.k42.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoder(p0)) = answer
                && o.k42.is_none()
            {
                o.k42 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoDecoderOutputView
        if o.k43.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoderOutputView(p0)) = answer
                && o.k43.is_none()
            {
                o.k43 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoDevice
        if o.k44.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDevice(p0)) = answer
                && o.k44.is_none()
            {
                o.k44 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessor(p0)) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoProcessorEnumerator
        if o.k46.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorEnumerator(p0)) = answer
                && o.k46.is_none()
            {
                o.k46 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoProcessorInputView
        if o.k47.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorInputView(p0)) = answer
                && o.k47.is_none()
            {
                o.k47 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for VideoProcessorOutputView
        if o.k48.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorOutputView(p0)) = answer
                && o.k48.is_none()
            {
                o.k48 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for DeviceContextState
        if o.k49.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContextState(p0)) = answer
                && o.k49.is_none()
            {
                o.k49 = Some(p0);
            }
        }
        // ID3D11Device5::CreateFence for UserDefinedAnnotation
        if o.k50.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device5::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UserDefinedAnnotation(p0)) = answer
                && o.k50.is_none()
            {
                o.k50 = Some(p0);
            }
        }
        // ID3D11Device3::CreateShaderResourceView1
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device3::create_shader_resource_view1(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device3::CreateUnorderedAccessView1
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer =
                        direct3d_11::Device3::create_unordered_access_view1(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device3::CreateRenderTargetView1
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device3::create_render_target_view1(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device3::CreateDeferredContext3
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer =
                        direct3d_11::Device3::create_deferred_context3(this, Default::default());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device2::CreateDeferredContext2
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer =
                        direct3d_11::Device2::create_deferred_context2(this, Default::default());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device1::CreateDeferredContext1
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer =
                        direct3d_11::Device1::create_deferred_context1(this, Default::default());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for AuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::AuthenticatedChannel(p0)) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for BlendState
        if o.k3.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::BlendState(p0)) = answer
                && o.k3.is_none()
            {
                o.k3 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Buffer
        if o.k4.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BUFFER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Buffer(p0)) = answer
                && o.k4.is_none()
            {
                o.k4 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassInstance(p0)) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassLinkage(p0)) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for CommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CommandList(p0)) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ComputeShader(p0)) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Counter
        if o.k9.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COUNTER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Counter(p0)) = answer
                && o.k9.is_none()
            {
                o.k9 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for CryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CryptoSession(p0)) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Debug
        if o.k11.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEBUG,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Debug(p0)) = answer
                && o.k11.is_none()
            {
                o.k11 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for DepthStencilState
        if o.k12.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilState(p0)) = answer
                && o.k12.is_none()
            {
                o.k12 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for DepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilView(p0)) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Device
        if o.k14.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE5,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Device(p0)) = answer
                && o.k14.is_none()
            {
                o.k14 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for DeviceContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContext(p0)) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for DomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DomainShader(p0)) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Fence
        if o.k17.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Fence(p0)) = answer
                && o.k17.is_none()
            {
                o.k17 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for GeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::GeometryShader(p0)) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for HullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_HULL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::HullShader(p0)) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for InfoQueue
        if o.k20.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InfoQueue(p0)) = answer
                && o.k20.is_none()
            {
                o.k20 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for InputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InputLayout(p0)) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Multithread
        if o.k22.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_MULTITHREAD,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Multithread(p0)) = answer
                && o.k22.is_none()
            {
                o.k22 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for PixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::PixelShader(p0)) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Predicate
        if o.k24.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PREDICATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Predicate(p0)) = answer
                && o.k24.is_none()
            {
                o.k24 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Query
        if o.k25.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_QUERY1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Query(p0)) = answer
                && o.k25.is_none()
            {
                o.k25 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for RasterizerState
        if o.k26.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RasterizerState(p0)) = answer
                && o.k26.is_none()
            {
                o.k26 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for RefDefaultTrackingOptions
        if o.k27.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefDefaultTrackingOptions(p0)) = answer
                && o.k27.is_none()
            {
                o.k27 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for RefTrackingOptions
        if o.k28.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefTrackingOptions(p0)) = answer
                && o.k28.is_none()
            {
                o.k28 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for RenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RenderTargetView(p0)) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for SamplerState
        if o.k30.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SamplerState(p0)) = answer
                && o.k30.is_none()
            {
                o.k30 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderResourceView(p0)) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ShaderTrace
        if o.k32.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTrace(p0)) = answer
                && o.k32.is_none()
            {
                o.k32 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for ShaderTraceFactory
        if o.k33.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTraceFactory(p0)) = answer
                && o.k33.is_none()
            {
                o.k33 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for SwitchToRef
        if o.k34.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SwitchToRef(p0)) = answer
                && o.k34.is_none()
            {
                o.k34 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Texture1D
        if o.k35.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture1D(p0)) = answer
                && o.k35.is_none()
            {
                o.k35 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Texture2D
        if o.k36.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture2D(p0)) = answer
                && o.k36.is_none()
            {
                o.k36 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for Texture3D
        if o.k37.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture3D(p0)) = answer
                && o.k37.is_none()
            {
                o.k37 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for TracingDevice
        if o.k38.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TRACING_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::TracingDevice(p0)) = answer
                && o.k38.is_none()
            {
                o.k38 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for UnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UnorderedAccessView(p0)) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VertexShader(p0)) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoContext
        if o.k41.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoContext(p0)) = answer
                && o.k41.is_none()
            {
                o.k41 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoDecoder
        if o.k42.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoder(p0)) = answer
                && o.k42.is_none()
            {
                o.k42 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoDecoderOutputView
        if o.k43.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoderOutputView(p0)) = answer
                && o.k43.is_none()
            {
                o.k43 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoDevice
        if o.k44.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDevice(p0)) = answer
                && o.k44.is_none()
            {
                o.k44 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessor(p0)) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoProcessorEnumerator
        if o.k46.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorEnumerator(p0)) = answer
                && o.k46.is_none()
            {
                o.k46 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoProcessorInputView
        if o.k47.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorInputView(p0)) = answer
                && o.k47.is_none()
            {
                o.k47 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for VideoProcessorOutputView
        if o.k48.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorOutputView(p0)) = answer
                && o.k48.is_none()
            {
                o.k48 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for DeviceContextState
        if o.k49.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContextState(p0)) = answer
                && o.k49.is_none()
            {
                o.k49 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResource1 for UserDefinedAnnotation
        if o.k50.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource1(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UserDefinedAnnotation(p0)) = answer
                && o.k50.is_none()
            {
                o.k50 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for AuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::AuthenticatedChannel(p0)) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for BlendState
        if o.k3.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::BlendState(p0)) = answer
                && o.k3.is_none()
            {
                o.k3 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Buffer
        if o.k4.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BUFFER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Buffer(p0)) = answer
                && o.k4.is_none()
            {
                o.k4 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassInstance(p0)) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassLinkage(p0)) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for CommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CommandList(p0)) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ComputeShader(p0)) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Counter
        if o.k9.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COUNTER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Counter(p0)) = answer
                && o.k9.is_none()
            {
                o.k9 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for CryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CryptoSession(p0)) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Debug
        if o.k11.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEBUG,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Debug(p0)) = answer
                && o.k11.is_none()
            {
                o.k11 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for DepthStencilState
        if o.k12.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilState(p0)) = answer
                && o.k12.is_none()
            {
                o.k12 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for DepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilView(p0)) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Device
        if o.k14.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE5,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Device(p0)) = answer
                && o.k14.is_none()
            {
                o.k14 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for DeviceContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContext(p0)) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for DomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DomainShader(p0)) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Fence
        if o.k17.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Fence(p0)) = answer
                && o.k17.is_none()
            {
                o.k17 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for GeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::GeometryShader(p0)) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for HullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_HULL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::HullShader(p0)) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for InfoQueue
        if o.k20.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InfoQueue(p0)) = answer
                && o.k20.is_none()
            {
                o.k20 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for InputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InputLayout(p0)) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Multithread
        if o.k22.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_MULTITHREAD,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Multithread(p0)) = answer
                && o.k22.is_none()
            {
                o.k22 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for PixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::PixelShader(p0)) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Predicate
        if o.k24.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PREDICATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Predicate(p0)) = answer
                && o.k24.is_none()
            {
                o.k24 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Query
        if o.k25.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_QUERY1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Query(p0)) = answer
                && o.k25.is_none()
            {
                o.k25 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for RasterizerState
        if o.k26.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RasterizerState(p0)) = answer
                && o.k26.is_none()
            {
                o.k26 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for RefDefaultTrackingOptions
        if o.k27.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefDefaultTrackingOptions(p0)) = answer
                && o.k27.is_none()
            {
                o.k27 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for RefTrackingOptions
        if o.k28.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefTrackingOptions(p0)) = answer
                && o.k28.is_none()
            {
                o.k28 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for RenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RenderTargetView(p0)) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for SamplerState
        if o.k30.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SamplerState(p0)) = answer
                && o.k30.is_none()
            {
                o.k30 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderResourceView(p0)) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ShaderTrace
        if o.k32.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTrace(p0)) = answer
                && o.k32.is_none()
            {
                o.k32 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for ShaderTraceFactory
        if o.k33.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTraceFactory(p0)) = answer
                && o.k33.is_none()
            {
                o.k33 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for SwitchToRef
        if o.k34.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SwitchToRef(p0)) = answer
                && o.k34.is_none()
            {
                o.k34 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Texture1D
        if o.k35.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture1D(p0)) = answer
                && o.k35.is_none()
            {
                o.k35 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Texture2D
        if o.k36.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture2D(p0)) = answer
                && o.k36.is_none()
            {
                o.k36 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for Texture3D
        if o.k37.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture3D(p0)) = answer
                && o.k37.is_none()
            {
                o.k37 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for TracingDevice
        if o.k38.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TRACING_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::TracingDevice(p0)) = answer
                && o.k38.is_none()
            {
                o.k38 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for UnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UnorderedAccessView(p0)) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VertexShader(p0)) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoContext
        if o.k41.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoContext(p0)) = answer
                && o.k41.is_none()
            {
                o.k41 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoDecoder
        if o.k42.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoder(p0)) = answer
                && o.k42.is_none()
            {
                o.k42 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoDecoderOutputView
        if o.k43.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoderOutputView(p0)) = answer
                && o.k43.is_none()
            {
                o.k43 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoDevice
        if o.k44.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDevice(p0)) = answer
                && o.k44.is_none()
            {
                o.k44 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessor(p0)) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoProcessorEnumerator
        if o.k46.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorEnumerator(p0)) = answer
                && o.k46.is_none()
            {
                o.k46 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoProcessorInputView
        if o.k47.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorInputView(p0)) = answer
                && o.k47.is_none()
            {
                o.k47 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for VideoProcessorOutputView
        if o.k48.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorOutputView(p0)) = answer
                && o.k48.is_none()
            {
                o.k48 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for DeviceContextState
        if o.k49.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContextState(p0)) = answer
                && o.k49.is_none()
            {
                o.k49 = Some(p0);
            }
        }
        // ID3D11Device1::OpenSharedResourceByName for UserDefinedAnnotation
        if o.k50.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device1::open_shared_resource_by_name(
                        this,
                        &[],
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UserDefinedAnnotation(p0)) = answer
                && o.k50.is_none()
            {
                o.k50 = Some(p0);
            }
        }
        // ID3D11Device::CreateShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device::create_shader_resource_view(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device::CreateUnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device::create_unordered_access_view(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device::CreateRenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device::create_render_target_view(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device::CreateDepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let v1 = (if let Some(x) = o.k4.as_ref() {
                        Some(direct3d_11::ResourceRef::Buffer(x))
                    } else if let Some(x) = o.k35.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture1D(x))
                    } else if let Some(x) = o.k36.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture2D(x))
                    } else if let Some(x) = o.k37.as_ref() {
                        Some(direct3d_11::ResourceRef::Texture3D(x))
                    } else {
                        None
                    })?;
                    let answer = direct3d_11::Device::create_depth_stencil_view(this, v1, None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device::CreateInputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_input_layout(this, &[], &[]);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device::CreateVertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_vertex_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device::CreateGeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_geometry_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device::CreateGeometryShaderWithStreamOutput
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_geometry_shader_with_stream_output(
                        this,
                        &[],
                        &[],
                        &[],
                        Default::default(),
                        None,
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device::CreatePixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_pixel_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device::CreateHullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_hull_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device::CreateDomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_domain_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device::CreateComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_compute_shader(this, &[], None);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device::CreateClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::create_class_linkage(this);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device::CreateDeferredContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer =
                        direct3d_11::Device::create_deferred_context(this, Default::default());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for AuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_AUTHENTICATED_CHANNEL,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::AuthenticatedChannel(p0)) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for BlendState
        if o.k3.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BLEND_STATE1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::BlendState(p0)) = answer
                && o.k3.is_none()
            {
                o.k3 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Buffer
        if o.k4.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_BUFFER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Buffer(p0)) = answer
                && o.k4.is_none()
            {
                o.k4 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ClassInstance
        if o.k5.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_INSTANCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassInstance(p0)) = answer
                && o.k5.is_none()
            {
                o.k5 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ClassLinkage
        if o.k6.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CLASS_LINKAGE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ClassLinkage(p0)) = answer
                && o.k6.is_none()
            {
                o.k6 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for CommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CommandList(p0)) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ComputeShader
        if o.k8.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COMPUTE_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ComputeShader(p0)) = answer
                && o.k8.is_none()
            {
                o.k8 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Counter
        if o.k9.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_COUNTER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Counter(p0)) = answer
                && o.k9.is_none()
            {
                o.k9 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for CryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_CRYPTO_SESSION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::CryptoSession(p0)) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Debug
        if o.k11.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEBUG,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Debug(p0)) = answer
                && o.k11.is_none()
            {
                o.k11 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for DepthStencilState
        if o.k12.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilState(p0)) = answer
                && o.k12.is_none()
            {
                o.k12 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for DepthStencilView
        if o.k13.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEPTH_STENCIL_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DepthStencilView(p0)) = answer
                && o.k13.is_none()
            {
                o.k13 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Device
        if o.k14.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE5,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Device(p0)) = answer
                && o.k14.is_none()
            {
                o.k14 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for DeviceContext
        if o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DEVICE_CONTEXT4,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContext(p0)) = answer
                && o.k15.is_none()
            {
                o.k15 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for DomainShader
        if o.k16.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_DOMAIN_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DomainShader(p0)) = answer
                && o.k16.is_none()
            {
                o.k16 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Fence
        if o.k17.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Fence(p0)) = answer
                && o.k17.is_none()
            {
                o.k17 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for GeometryShader
        if o.k18.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_GEOMETRY_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::GeometryShader(p0)) = answer
                && o.k18.is_none()
            {
                o.k18 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for HullShader
        if o.k19.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_HULL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::HullShader(p0)) = answer
                && o.k19.is_none()
            {
                o.k19 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for InfoQueue
        if o.k20.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InfoQueue(p0)) = answer
                && o.k20.is_none()
            {
                o.k20 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for InputLayout
        if o.k21.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_INPUT_LAYOUT,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::InputLayout(p0)) = answer
                && o.k21.is_none()
            {
                o.k21 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Multithread
        if o.k22.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_MULTITHREAD,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Multithread(p0)) = answer
                && o.k22.is_none()
            {
                o.k22 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for PixelShader
        if o.k23.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PIXEL_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::PixelShader(p0)) = answer
                && o.k23.is_none()
            {
                o.k23 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Predicate
        if o.k24.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_PREDICATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Predicate(p0)) = answer
                && o.k24.is_none()
            {
                o.k24 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Query
        if o.k25.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_QUERY1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Query(p0)) = answer
                && o.k25.is_none()
            {
                o.k25 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for RasterizerState
        if o.k26.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RASTERIZER_STATE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RasterizerState(p0)) = answer
                && o.k26.is_none()
            {
                o.k26 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for RefDefaultTrackingOptions
        if o.k27.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefDefaultTrackingOptions(p0)) = answer
                && o.k27.is_none()
            {
                o.k27 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for RefTrackingOptions
        if o.k28.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_REF_TRACKING_OPTIONS,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RefTrackingOptions(p0)) = answer
                && o.k28.is_none()
            {
                o.k28 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for RenderTargetView
        if o.k29.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_RENDER_TARGET_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::RenderTargetView(p0)) = answer
                && o.k29.is_none()
            {
                o.k29 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for SamplerState
        if o.k30.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SAMPLER_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SamplerState(p0)) = answer
                && o.k30.is_none()
            {
                o.k30 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ShaderResourceView
        if o.k31.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_RESOURCE_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderResourceView(p0)) = answer
                && o.k31.is_none()
            {
                o.k31 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ShaderTrace
        if o.k32.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTrace(p0)) = answer
                && o.k32.is_none()
            {
                o.k32 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for ShaderTraceFactory
        if o.k33.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SHADER_TRACE_FACTORY,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::ShaderTraceFactory(p0)) = answer
                && o.k33.is_none()
            {
                o.k33 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for SwitchToRef
        if o.k34.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_SWITCH_TO_REF,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::SwitchToRef(p0)) = answer
                && o.k34.is_none()
            {
                o.k34 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Texture1D
        if o.k35.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_1D,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture1D(p0)) = answer
                && o.k35.is_none()
            {
                o.k35 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Texture2D
        if o.k36.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_2D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture2D(p0)) = answer
                && o.k36.is_none()
            {
                o.k36 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for Texture3D
        if o.k37.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TEXTURE_3D1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::Texture3D(p0)) = answer
                && o.k37.is_none()
            {
                o.k37 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for TracingDevice
        if o.k38.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_TRACING_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::TracingDevice(p0)) = answer
                && o.k38.is_none()
            {
                o.k38 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for UnorderedAccessView
        if o.k39.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_UNORDERED_ACCESS_VIEW1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UnorderedAccessView(p0)) = answer
                && o.k39.is_none()
            {
                o.k39 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VertexShader
        if o.k40.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VERTEX_SHADER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VertexShader(p0)) = answer
                && o.k40.is_none()
            {
                o.k40 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoContext
        if o.k41.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_CONTEXT3,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoContext(p0)) = answer
                && o.k41.is_none()
            {
                o.k41 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoDecoder
        if o.k42.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoder(p0)) = answer
                && o.k42.is_none()
            {
                o.k42 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoDecoderOutputView
        if o.k43.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDecoderOutputView(p0)) = answer
                && o.k43.is_none()
            {
                o.k43 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoDevice
        if o.k44.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_DEVICE2,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoDevice(p0)) = answer
                && o.k44.is_none()
            {
                o.k44 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessor(p0)) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoProcessorEnumerator
        if o.k46.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorEnumerator(p0)) = answer
                && o.k46.is_none()
            {
                o.k46 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoProcessorInputView
        if o.k47.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorInputView(p0)) = answer
                && o.k47.is_none()
            {
                o.k47 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for VideoProcessorOutputView
        if o.k48.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::VideoProcessorOutputView(p0)) = answer
                && o.k48.is_none()
            {
                o.k48 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for DeviceContextState
        if o.k49.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_DEVICE_CONTEXT_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::DeviceContextState(p0)) = answer
                && o.k49.is_none()
            {
                o.k49 = Some(p0);
            }
        }
        // ID3D11Device::OpenSharedResource for UserDefinedAnnotation
        if o.k50.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = direct3d_11::Device::open_shared_resource(
                        this,
                        Default::default(),
                        &direct3d_11::constants::IID_ID3D_USER_DEFINED_ANNOTATION,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_11::UnknownObject::UserDefinedAnnotation(p0)) = answer
                && o.k50.is_none()
            {
                o.k50 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        // IDXGIDevice::GetAdapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k14.as_ref()?;
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
        // ID3D11DeviceContext::FinishCommandList
        if o.k7.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k15.as_ref()?;
                    let answer =
                        direct3d_11::DeviceContext::finish_command_list(this, Default::default());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k7.is_none()
            {
                o.k7 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
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
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k35.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
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
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k36.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
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
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k37.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
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
        // ID3D11VideoDevice::CreateVideoProcessor
        if o.k45.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k44.as_ref()?;
                    let v1 = o.k46.as_ref()?;
                    let answer = direct3d_11::VideoDevice::create_video_processor(
                        this,
                        v1,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k45.is_none()
            {
                o.k45 = Some(p0);
            }
        }
        // ID3D11VideoDevice::CreateAuthenticatedChannel
        if o.k2.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k44.as_ref()?;
                    let answer = direct3d_11::VideoDevice::create_authenticated_channel(
                        this,
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k2.is_none()
            {
                o.k2 = Some(p0);
            }
        }
        // ID3D11VideoDevice::CreateCryptoSession
        if o.k10.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k44.as_ref()?;
                    let answer = direct3d_11::VideoDevice::create_crypto_session(
                        this,
                        &Default::default(),
                        None,
                        &Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k10.is_none()
            {
                o.k10 = Some(p0);
            }
        }
        // IDXGIAdapter::EnumOutputs
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Adapter::enum_outputs(
                        this,
                        Default::default(),
                    );
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
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
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
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
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
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
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
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
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, Default::default(), Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
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
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, Default::default(), Default::default(), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
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
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
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
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIFactory1::EnumAdapters1
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory1::enum_adapters1(
                        this,
                        Default::default(),
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
        // IDXGIFactory::EnumAdapters
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory::enum_adapters(
                        this,
                        Default::default(),
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
        // IDXGIFactory::CreateSoftwareAdapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory::create_software_adapter(
                        this,
                        Default::default(),
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIOutputDuplication::AcquireNextFrame
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                            this,
                            Default::default(),
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
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this,
                            Default::default(),
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
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for GraphicsAnalysis
        if o.k76.is_none() {
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
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
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
        if o.k61.is_none() {
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
        if o.k62.is_none() {
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
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
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
        if o.k64.is_none() {
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
        if o.k65.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
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
        if o.k67.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
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
        if o.k69.is_none() {
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
        if o.k70.is_none() {
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
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
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
        if o.k72.is_none() {
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
        if o.k73.is_none() {
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
        if o.k74.is_none() {
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
        if o.k75.is_none() {
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
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
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
        // D3D11CreateDevice
        if o.k14.is_none() || o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_11::Direct3D11::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        Default::default(),
                        Default::default(),
                        &[],
                        Default::default(),
                    );
                    answer.ok()
                })
            };
            if let Some((p0, p1, p2)) = answer {
                if o.k14.is_none() {
                    o.k14 = Some(p0);
                }
                if o.k15.is_none() {
                    o.k15 = Some(p2);
                }
            }
        }
        // D3D11CreateDeviceAndSwapChain
        if o.k74.is_none() || o.k14.is_none() || o.k15.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_11::Direct3D11::create_device_and_swap_chain(
                        &o.api,
                        None,
                        Default::default(),
                        Default::default(),
                        Default::default(),
                        &[],
                        Default::default(),
                        None,
                    );
                    answer.ok()
                })
            };
            if let Some((p0, p1, p2, p3)) = answer {
                if o.k74.is_none() {
                    o.k74 = Some(p0);
                }
                if o.k14.is_none() {
                    o.k14 = Some(p1);
                }
                if o.k15.is_none() {
                    o.k15 = Some(p3);
                }
            }
        }
        if o.kinds_held() == before {
            break;
        }
    }
    o.kinds_held()
}

/// Calls every method on every object held, and the exports, each answer checked.
fn drive<A: direct3d_11::Direct3D11>(o: &Objects<A>) -> Driven {
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
    // ID3D11AuthenticatedChannel::GetCertificateSize on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::AuthenticatedChannel::get_certificate_size(
            this,
        ))
    });
    // ID3D11AuthenticatedChannel::GetCertificate on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::AuthenticatedChannel::get_certificate(
            this, &lying,
        ))
    });
    // ID3D11AuthenticatedChannel::GetChannelHandle on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        answered(direct3d_11::AuthenticatedChannel::get_channel_handle(this))
    });
    // ID3D11DeviceChild::GetDevice on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on AuthenticatedChannel
    driven.attempt(|| {
        let Some(this) = o.k2.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11BlendState1::GetDesc1 on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        answered(direct3d_11::BlendState1::get_desc1(this))
    });
    // ID3D11BlendState::GetDesc on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        answered(direct3d_11::BlendState::get_desc(this))
    });
    // ID3D11DeviceChild::GetDevice on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on BlendState
    driven.attempt(|| {
        let Some(this) = o.k3.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Buffer::GetDesc on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::Buffer::get_desc(this))
    });
    // ID3D11Resource::GetType on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_type(this))
    });
    // ID3D11Resource::SetEvictionPriority on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::set_eviction_priority(
            this,
            Default::default(),
        ))
    });
    // ID3D11Resource::GetEvictionPriority on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_eviction_priority(this))
    });
    // ID3D11DeviceChild::GetDevice on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // IDXGIObject::SetPrivateData on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
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
    // IDXGIObject::GetPrivateData on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIResource::GetSharedHandle on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource::GetEvictionPriority on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIResource1::CreateSubresourceSurface on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource1::CreateSharedHandle on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
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
    // ID3DDestructionNotifier::RegisterDestructionCallback on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Buffer
    driven.attempt(|| {
        let Some(this) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11ClassInstance::GetClassLinkage on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        answered(direct3d_11::ClassInstance::get_class_linkage(this))
    });
    // ID3D11ClassInstance::GetDesc on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        answered(direct3d_11::ClassInstance::get_desc(this))
    });
    // ID3D11ClassInstance::GetInstanceName on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        answered(direct3d_11::ClassInstance::get_instance_name(this, None))
    });
    // ID3D11ClassInstance::GetTypeName on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        answered(direct3d_11::ClassInstance::get_type_name(this, None))
    });
    // ID3D11DeviceChild::GetDevice on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ClassInstance
    driven.attempt(|| {
        let Some(this) = o.k5.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11ClassLinkage::GetClassInstance on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ClassLinkage::get_class_instance(
            this,
            c"",
            Default::default(),
        ))
    });
    // ID3D11ClassLinkage::CreateClassInstance on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ClassLinkage::create_class_instance(
            this,
            c"",
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceChild::GetDevice on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ClassLinkage
    driven.attempt(|| {
        let Some(this) = o.k6.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11CommandList::GetContextFlags on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        answered(direct3d_11::CommandList::get_context_flags(this))
    });
    // ID3D11DeviceChild::GetDevice on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on CommandList
    driven.attempt(|| {
        let Some(this) = o.k7.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ComputeShader
    driven.attempt(|| {
        let Some(this) = o.k8.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Counter::GetDesc on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        answered(direct3d_11::Counter::get_desc(this))
    });
    // ID3D11Asynchronous::GetDataSize on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        answered(direct3d_11::Asynchronous::get_data_size(this))
    });
    // ID3D11DeviceChild::GetDevice on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Counter
    driven.attempt(|| {
        let Some(this) = o.k9.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11CryptoSession::GetCryptoType on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        answered(direct3d_11::CryptoSession::get_crypto_type(this))
    });
    // ID3D11CryptoSession::GetDecoderProfile on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        answered(direct3d_11::CryptoSession::get_decoder_profile(this))
    });
    // ID3D11CryptoSession::GetCertificateSize on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::CryptoSession::get_certificate_size(this))
    });
    // ID3D11CryptoSession::GetCertificate on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::CryptoSession::get_certificate(this, &lying))
    });
    // ID3D11CryptoSession::GetCryptoSessionHandle on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        answered(direct3d_11::CryptoSession::get_crypto_session_handle(this))
    });
    // ID3D11DeviceChild::GetDevice on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on CryptoSession
    driven.attempt(|| {
        let Some(this) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Debug::SetFeatureMask on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::set_feature_mask(
            this,
            Default::default(),
        ))
    });
    // ID3D11Debug::GetFeatureMask on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        answered(direct3d_11::Debug::get_feature_mask(this))
    });
    // ID3D11Debug::SetPresentPerRenderOpDelay on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::set_present_per_render_op_delay(
            this,
            Default::default(),
        ))
    });
    // ID3D11Debug::GetPresentPerRenderOpDelay on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        answered(direct3d_11::Debug::get_present_per_render_op_delay(this))
    });
    // ID3D11Debug::SetSwapChain on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::set_swap_chain(this, None))
    });
    // ID3D11Debug::GetSwapChain on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::get_swap_chain(this))
    });
    // ID3D11Debug::ValidateContext on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        let Some(v1) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::validate_context(this, v1))
    });
    // ID3D11Debug::ReportLiveDeviceObjects on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::report_live_device_objects(
            this,
            Default::default(),
        ))
    });
    // ID3D11Debug::ValidateContextForDispatch on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        let Some(v1) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Debug::validate_context_for_dispatch(this, v1))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Debug
    driven.attempt(|| {
        let Some(this) = o.k11.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DepthStencilState::GetDesc on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        answered(direct3d_11::DepthStencilState::get_desc(this))
    });
    // ID3D11DeviceChild::GetDevice on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DepthStencilState
    driven.attempt(|| {
        let Some(this) = o.k12.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DepthStencilView::GetDesc on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        answered(direct3d_11::DepthStencilView::get_desc(this))
    });
    // ID3D11View::GetResource on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DepthStencilView
    driven.attempt(|| {
        let Some(this) = o.k13.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Device5::OpenSharedFence on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device5::open_shared_fence(
            this,
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device5::CreateFence on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device5::create_fence(
            this,
            Default::default(),
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device4::RegisterDeviceRemovedEvent on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device4::register_device_removed_event(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device4::UnregisterDeviceRemoved on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device4::unregister_device_removed(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device3::CreateShaderResourceView1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device3::create_shader_resource_view1(
            this, v1, None,
        ))
    });
    // ID3D11Device3::CreateUnorderedAccessView1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device3::create_unordered_access_view1(
            this, v1, None,
        ))
    });
    // ID3D11Device3::CreateRenderTargetView1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device3::create_render_target_view1(
            this, v1, None,
        ))
    });
    // ID3D11Device3::GetImmediateContext3 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device3::get_immediate_context3(this))
    });
    // ID3D11Device3::CreateDeferredContext3 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device3::create_deferred_context3(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device3::WriteToSubresource on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        let lying = bytes();
        answered(direct3d_11::Device3::write_to_subresource(
            this,
            v1,
            Default::default(),
            None,
            &lying,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11Device3::ReadFromSubresource on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let lying = bytes();
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::Device3::read_from_subresource(
            this,
            &lying,
            Default::default(),
            Default::default(),
            v1,
            Default::default(),
            None,
        ))
    });
    // ID3D11Device2::GetImmediateContext2 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device2::get_immediate_context2(this))
    });
    // ID3D11Device2::CreateDeferredContext2 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device2::create_deferred_context2(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device2::GetResourceTiling on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::Device2::get_resource_tiling(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11Device2::CheckMultisampleQualityLevels1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device2::check_multisample_quality_levels1(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11Device1::GetImmediateContext1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device1::get_immediate_context1(this))
    });
    // ID3D11Device1::CreateDeferredContext1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device1::create_deferred_context1(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device1::CreateDeviceContextState on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device1::create_device_context_state(
            this,
            Default::default(),
            &[],
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device1::OpenSharedResource1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device1::open_shared_resource1(
            this,
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device1::OpenSharedResourceByName on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device1::open_shared_resource_by_name(
            this,
            &[],
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device::CreateShaderResourceView on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_shader_resource_view(
            this, v1, None,
        ))
    });
    // ID3D11Device::CreateUnorderedAccessView on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_unordered_access_view(
            this, v1, None,
        ))
    });
    // ID3D11Device::CreateRenderTargetView on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_render_target_view(
            this, v1, None,
        ))
    });
    // ID3D11Device::CreateDepthStencilView on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_depth_stencil_view(
            this, v1, None,
        ))
    });
    // ID3D11Device::CreateInputLayout on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_input_layout(this, &[], &[]))
    });
    // ID3D11Device::CreateVertexShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_vertex_shader(this, &[], None))
    });
    // ID3D11Device::CreateGeometryShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_geometry_shader(this, &[], None))
    });
    // ID3D11Device::CreateGeometryShaderWithStreamOutput on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::Device::create_geometry_shader_with_stream_output(
                this,
                &[],
                &[],
                &[],
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11Device::CreatePixelShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_pixel_shader(this, &[], None))
    });
    // ID3D11Device::CreateHullShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_hull_shader(this, &[], None))
    });
    // ID3D11Device::CreateDomainShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_domain_shader(this, &[], None))
    });
    // ID3D11Device::CreateComputeShader on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_compute_shader(this, &[], None))
    });
    // ID3D11Device::CreateClassLinkage on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_class_linkage(this))
    });
    // ID3D11Device::CreateDeferredContext on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::create_deferred_context(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device::OpenSharedResource on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::open_shared_resource(
            this,
            Default::default(),
            &Default::default(),
        ))
    });
    // ID3D11Device::CheckFormatSupport on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::check_format_support(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device::CheckMultisampleQualityLevels on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::check_multisample_quality_levels(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11Device::CheckCounterInfo on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device::check_counter_info(this))
    });
    // ID3D11Device::CheckFeatureSupport on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::Device::check_feature_support(
            this,
            Default::default(),
            &lying,
        ))
    });
    // ID3D11Device::GetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::Device::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11Device::SetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11Device::SetPrivateDataInterface on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11Device::GetFeatureLevel on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device::get_feature_level(this))
    });
    // ID3D11Device::GetCreationFlags on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device::get_creation_flags(this))
    });
    // ID3D11Device::GetDeviceRemovedReason on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::get_device_removed_reason(this))
    });
    // ID3D11Device::GetImmediateContext on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device::get_immediate_context(this))
    });
    // ID3D11Device::SetExceptionMode on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Device::set_exception_mode(
            this,
            Default::default(),
        ))
    });
    // ID3D11Device::GetExceptionMode on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Device::get_exception_mode(this))
    });
    // IDXGIObject::SetPrivateData on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDevice::GetAdapter on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_adapter(this))
    });
    // IDXGIDevice::QueryResourceResidency on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device::query_resource_residency(this, &[]),
        )
    });
    // IDXGIDevice::SetGPUThreadPriority on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_gpu_thread_priority(this))
    });
    // IDXGIDevice1::SetMaximumFrameLatency on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device1::get_maximum_frame_latency(this),
        )
    });
    // IDXGIDevice2::OfferResources on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::reclaim_resources(
            this,
            &[],
        ))
    });
    // IDXGIDevice2::EnqueueSetEvent on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::enqueue_set_event(
            this,
            Default::default(),
        ))
    });
    // IDXGIDevice3::Trim on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Device3::trim(this))
    });
    // IDXGIDevice4::OfferResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device4::reclaim_resources1(this, &[]))
    });
    // ID3D11Multithread::Enter on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::enter(this))
    });
    // ID3D11Multithread::Leave on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::leave(this))
    });
    // ID3D11Multithread::SetMultithreadProtected on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::set_multithread_protected(
            this,
            Default::default(),
        ))
    });
    // ID3D11Multithread::GetMultithreadProtected on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::get_multithread_protected(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Device
    driven.attempt(|| {
        let Some(this) = o.k14.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceContext4::Signal on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext4::signal(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext4::Wait on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext4::wait(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext3::Flush1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext3::flush1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext3::SetHardwareProtectionState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext3::set_hardware_protection_state(
            this,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext3::GetHardwareProtectionState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext3::get_hardware_protection_state(
            this,
        ))
    });
    // ID3D11DeviceContext2::UpdateTileMappings on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext2::update_tile_mappings(
            this,
            v1,
            &[],
            &[],
            None,
            &[],
            &[],
            &[],
            Default::default(),
        ))
    });
    // ID3D11DeviceContext2::ResizeTilePool on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k4.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext2::resize_tile_pool(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext2::TiledResourceBarrier on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext2::tiled_resource_barrier(
            this, None, None,
        ))
    });
    // ID3D11DeviceContext2::IsAnnotationEnabled on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext2::is_annotation_enabled(this))
    });
    // ID3D11DeviceContext2::SetMarkerInt on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext2::set_marker_int(
            this,
            &[],
            Default::default(),
        ))
    });
    // ID3D11DeviceContext2::BeginEventInt on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext2::begin_event_int(
            this,
            &[],
            Default::default(),
        ))
    });
    // ID3D11DeviceContext2::EndEvent on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext2::end_event(this))
    });
    // ID3D11DeviceContext1::CopySubresourceRegion1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        let Some(v2) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::copy_subresource_region1(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            v2,
            Default::default(),
            None,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::UpdateSubresource1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::update_subresource1(
            this,
            v1,
            Default::default(),
            None,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::DiscardResource on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::discard_resource(this, v1))
    });
    // ID3D11DeviceContext1::DiscardView on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k13.as_ref() {
            Some(direct3d_11::ViewRef::DepthStencilView(x))
        } else if let Some(x) = o.k29.as_ref() {
            Some(direct3d_11::ViewRef::RenderTargetView(x))
        } else if let Some(x) = o.k31.as_ref() {
            Some(direct3d_11::ViewRef::ShaderResourceView(x))
        } else if let Some(x) = o.k39.as_ref() {
            Some(direct3d_11::ViewRef::UnorderedAccessView(x))
        } else if let Some(x) = o.k43.as_ref() {
            Some(direct3d_11::ViewRef::VideoDecoderOutputView(x))
        } else if let Some(x) = o.k47.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorInputView(x))
        } else if let Some(x) = o.k48.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorOutputView(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::discard_view(this, v1))
    });
    // ID3D11DeviceContext1::VSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::vs_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::HSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::hs_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::DSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::ds_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::GSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::gs_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::PSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::ps_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::CSSetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::cs_set_constant_buffers1(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext1::VSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::vs_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::HSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::hs_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::DSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::ds_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::GSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::gs_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::PSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::ps_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::CSGetConstantBuffers1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::cs_get_constant_buffers1(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext1::SwapDeviceContextState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k49.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::swap_device_context_state(
            this, v1,
        ))
    });
    // ID3D11DeviceContext1::ClearView on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k13.as_ref() {
            Some(direct3d_11::ViewRef::DepthStencilView(x))
        } else if let Some(x) = o.k29.as_ref() {
            Some(direct3d_11::ViewRef::RenderTargetView(x))
        } else if let Some(x) = o.k31.as_ref() {
            Some(direct3d_11::ViewRef::ShaderResourceView(x))
        } else if let Some(x) = o.k39.as_ref() {
            Some(direct3d_11::ViewRef::UnorderedAccessView(x))
        } else if let Some(x) = o.k43.as_ref() {
            Some(direct3d_11::ViewRef::VideoDecoderOutputView(x))
        } else if let Some(x) = o.k47.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorInputView(x))
        } else if let Some(x) = o.k48.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorOutputView(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::clear_view(
            this,
            v1,
            &Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext1::DiscardView1 on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k13.as_ref() {
            Some(direct3d_11::ViewRef::DepthStencilView(x))
        } else if let Some(x) = o.k29.as_ref() {
            Some(direct3d_11::ViewRef::RenderTargetView(x))
        } else if let Some(x) = o.k31.as_ref() {
            Some(direct3d_11::ViewRef::ShaderResourceView(x))
        } else if let Some(x) = o.k39.as_ref() {
            Some(direct3d_11::ViewRef::UnorderedAccessView(x))
        } else if let Some(x) = o.k43.as_ref() {
            Some(direct3d_11::ViewRef::VideoDecoderOutputView(x))
        } else if let Some(x) = o.k47.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorInputView(x))
        } else if let Some(x) = o.k48.as_ref() {
            Some(direct3d_11::ViewRef::VideoProcessorOutputView(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext1::discard_view1(this, v1, &[]))
    });
    // ID3D11DeviceContext::VSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::PSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::PSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::PSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::VSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::DrawIndexed on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_indexed(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::Draw on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::Map on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext::map(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::Unmap on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::unmap(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::PSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::IASetInputLayout on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_set_input_layout(this, None))
    });
    // ID3D11DeviceContext::IASetVertexBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_set_vertex_buffers(
            this,
            Default::default(),
            &[],
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext::IASetIndexBuffer on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_set_index_buffer(
            this,
            None,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DrawIndexedInstanced on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_indexed_instanced(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DrawInstanced on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_instanced(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::GSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::IASetPrimitiveTopology on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_set_primitive_topology(
            this,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::VSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::VSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::Begin on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k9.as_ref() {
            Some(direct3d_11::AsynchronousRef::Counter(x))
        } else if let Some(x) = o.k24.as_ref() {
            Some(direct3d_11::AsynchronousRef::Predicate(x))
        } else if let Some(x) = o.k25.as_ref() {
            Some(direct3d_11::AsynchronousRef::Query(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::begin(this, v1))
    });
    // ID3D11DeviceContext::End on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k9.as_ref() {
            Some(direct3d_11::AsynchronousRef::Counter(x))
        } else if let Some(x) = o.k24.as_ref() {
            Some(direct3d_11::AsynchronousRef::Predicate(x))
        } else if let Some(x) = o.k25.as_ref() {
            Some(direct3d_11::AsynchronousRef::Query(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::end(this, v1))
    });
    // ID3D11DeviceContext::GetData on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k9.as_ref() {
            Some(direct3d_11::AsynchronousRef::Counter(x))
        } else if let Some(x) = o.k24.as_ref() {
            Some(direct3d_11::AsynchronousRef::Predicate(x))
        } else if let Some(x) = o.k25.as_ref() {
            Some(direct3d_11::AsynchronousRef::Query(x))
        } else {
            None
        }) else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceContext::get_data(
            this,
            v1,
            Some(&lying),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::SetPredication on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::set_predication(
            this,
            None,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::GSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::OMSetRenderTargets on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_set_render_targets(
            this,
            &[],
            None,
        ))
    });
    // ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::DeviceContext::om_set_render_targets_and_unordered_access_views(
                this,
                None,
                None,
                Default::default(),
                None,
                None,
            ),
        )
    });
    // ID3D11DeviceContext::OMSetBlendState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_set_blend_state(
            this,
            None,
            None,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::OMSetDepthStencilState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_set_depth_stencil_state(
            this,
            None,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::SOSetTargets on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::so_set_targets(this, &[], &[]))
    });
    // ID3D11DeviceContext::DrawAuto on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_auto(this))
    });
    // ID3D11DeviceContext::DrawIndexedInstancedIndirect on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_indexed_instanced_indirect(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DrawInstancedIndirect on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::draw_instanced_indirect(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::Dispatch on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::dispatch(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DispatchIndirect on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k4.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::dispatch_indirect(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::RSSetState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_set_state(this, None))
    });
    // ID3D11DeviceContext::RSSetViewports on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_set_viewports(this, &[]))
    });
    // ID3D11DeviceContext::RSSetScissorRects on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_set_scissor_rects(this, &[]))
    });
    // ID3D11DeviceContext::CopySubresourceRegion on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        let Some(v2) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::copy_subresource_region(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            v2,
            Default::default(),
            None,
        ))
    });
    // ID3D11DeviceContext::CopyResource on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        let Some(v2) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::copy_resource(this, v1, v2))
    });
    // ID3D11DeviceContext::UpdateSubresource on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::update_subresource(
            this,
            v1,
            Default::default(),
            None,
            &[],
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::CopyStructureCount on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k4.as_ref() else {
            return false;
        };
        let Some(v2) = o.k39.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::copy_structure_count(
            this,
            v1,
            Default::default(),
            v2,
        ))
    });
    // ID3D11DeviceContext::ClearRenderTargetView on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k29.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::clear_render_target_view(
            this,
            v1,
            &Default::default(),
        ))
    });
    // ID3D11DeviceContext::ClearUnorderedAccessViewUint on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k39.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::DeviceContext::clear_unordered_access_view_uint(
                this,
                v1,
                &Default::default(),
            ),
        )
    });
    // ID3D11DeviceContext::ClearUnorderedAccessViewFloat on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k39.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::DeviceContext::clear_unordered_access_view_float(
                this,
                v1,
                &Default::default(),
            ),
        )
    });
    // ID3D11DeviceContext::ClearDepthStencilView on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k13.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::clear_depth_stencil_view(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GenerateMips on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k31.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::generate_mips(this, v1))
    });
    // ID3D11DeviceContext::SetResourceMinLOD on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::set_resource_min_lod(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GetResourceMinLOD on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::get_resource_min_lod(this, v1))
    });
    // ID3D11DeviceContext::ResolveSubresource on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        let Some(v2) = (if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::ResourceRef::Buffer(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::ResourceRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::ResourceRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::ResourceRef::Texture3D(x))
        } else {
            None
        }) else {
            return false;
        };
        answered(direct3d_11::DeviceContext::resolve_subresource(
            this,
            v1,
            Default::default(),
            v2,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::ExecuteCommandList on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let Some(v1) = o.k7.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::execute_command_list(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::HSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::HSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::HSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::HSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::DSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::DSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::DSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::DSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::CSSetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_set_shader_resources(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::CSSetUnorderedAccessViews on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_set_unordered_access_views(
            this,
            Default::default(),
            &[],
            &[],
        ))
    });
    // ID3D11DeviceContext::CSSetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_set_shader(this, None, &[]))
    });
    // ID3D11DeviceContext::CSSetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_set_samplers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::CSSetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_set_constant_buffers(
            this,
            Default::default(),
            &[],
        ))
    });
    // ID3D11DeviceContext::VSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::PSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::PSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_get_shader(this))
    });
    // ID3D11DeviceContext::PSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::VSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_get_shader(this))
    });
    // ID3D11DeviceContext::PSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ps_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::IAGetInputLayout on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_get_input_layout(this))
    });
    // ID3D11DeviceContext::IAGetVertexBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_get_vertex_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::IAGetIndexBuffer on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_get_index_buffer(this))
    });
    // ID3D11DeviceContext::GSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_get_shader(this))
    });
    // ID3D11DeviceContext::IAGetPrimitiveTopology on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ia_get_primitive_topology(this))
    });
    // ID3D11DeviceContext::VSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::VSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::vs_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GetPredication on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::get_predication(this))
    });
    // ID3D11DeviceContext::GSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::GSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::gs_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::OMGetRenderTargets on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_get_render_targets(
            this,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::OMGetRenderTargetsAndUnorderedAccessViews on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::DeviceContext::om_get_render_targets_and_unordered_access_views(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceContext::OMGetBlendState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_get_blend_state(this))
    });
    // ID3D11DeviceContext::OMGetDepthStencilState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::om_get_depth_stencil_state(this))
    });
    // ID3D11DeviceContext::SOGetTargets on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::so_get_targets(
            this,
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::RSGetState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_get_state(this))
    });
    // ID3D11DeviceContext::RSGetViewports on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_get_viewports(this))
    });
    // ID3D11DeviceContext::RSGetScissorRects on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::rs_get_scissor_rects(this))
    });
    // ID3D11DeviceContext::HSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::HSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_get_shader(this))
    });
    // ID3D11DeviceContext::HSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::HSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::hs_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_get_shader(this))
    });
    // ID3D11DeviceContext::DSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::DSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::ds_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::CSGetShaderResources on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_get_shader_resources(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::CSGetUnorderedAccessViews on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_get_unordered_access_views(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::CSGetShader on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_get_shader(this))
    });
    // ID3D11DeviceContext::CSGetSamplers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_get_samplers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::CSGetConstantBuffers on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::cs_get_constant_buffers(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceContext::ClearState on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::clear_state(this))
    });
    // ID3D11DeviceContext::Flush on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::flush(this))
    });
    // ID3D11DeviceContext::GetType on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::get_type(this))
    });
    // ID3D11DeviceContext::GetContextFlags on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceContext::get_context_flags(this))
    });
    // ID3D11DeviceContext::FinishCommandList on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceContext::finish_command_list(
            this,
            Default::default(),
        ))
    });
    // ID3D11DeviceChild::GetDevice on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11Multithread::Enter on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::enter(this))
    });
    // ID3D11Multithread::Leave on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::leave(this))
    });
    // ID3D11Multithread::SetMultithreadProtected on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::set_multithread_protected(
            this,
            Default::default(),
        ))
    });
    // ID3D11Multithread::GetMultithreadProtected on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::get_multithread_protected(this))
    });
    // ID3DUserDefinedAnnotation::BeginEvent on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::begin_event(this, &[]))
    });
    // ID3DUserDefinedAnnotation::EndEvent on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::end_event(this))
    });
    // ID3DUserDefinedAnnotation::SetMarker on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::set_marker(this, &[]))
    });
    // ID3DUserDefinedAnnotation::GetStatus on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::get_status(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DeviceContext
    driven.attempt(|| {
        let Some(this) = o.k15.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DomainShader
    driven.attempt(|| {
        let Some(this) = o.k16.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Fence::CreateSharedHandle on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Fence::create_shared_handle(
            this,
            None,
            Default::default(),
            None,
        ))
    });
    // ID3D11Fence::GetCompletedValue on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        answered(direct3d_11::Fence::get_completed_value(this))
    });
    // ID3D11Fence::SetEventOnCompletion on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::Fence::set_event_on_completion(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11DeviceChild::GetDevice on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Fence
    driven.attempt(|| {
        let Some(this) = o.k17.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on GeometryShader
    driven.attempt(|| {
        let Some(this) = o.k18.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on HullShader
    driven.attempt(|| {
        let Some(this) = o.k19.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11InfoQueue::SetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::set_message_count_limit(
            this,
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::ClearStoredMessages on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::clear_stored_messages(this))
    });
    // ID3D11InfoQueue::GetMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::InfoQueue::get_message(
            this,
            Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11InfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_num_messages_allowed_by_storage_filter(this))
    });
    // ID3D11InfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_num_messages_denied_by_storage_filter(this))
    });
    // ID3D11InfoQueue::GetNumStoredMessages on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_num_stored_messages(this))
    });
    // ID3D11InfoQueue::GetNumStoredMessagesAllowedByRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_num_stored_messages_allowed_by_retrieval_filter(this))
    });
    // ID3D11InfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_num_messages_discarded_by_message_count_limit(this))
    });
    // ID3D11InfoQueue::GetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_message_count_limit(this))
    });
    // ID3D11InfoQueue::GetStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::InfoQueue::get_storage_filter(
            this,
            Some(&lying),
        ))
    });
    // ID3D11InfoQueue::ClearStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::clear_storage_filter(this))
    });
    // ID3D11InfoQueue::PushEmptyStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::push_empty_storage_filter(this))
    });
    // ID3D11InfoQueue::PushCopyOfStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::push_copy_of_storage_filter(this))
    });
    // ID3D11InfoQueue::PopStorageFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::pop_storage_filter(this))
    });
    // ID3D11InfoQueue::GetStorageFilterStackSize on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_storage_filter_stack_size(this))
    });
    // ID3D11InfoQueue::GetRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::InfoQueue::get_retrieval_filter(
            this,
            Some(&lying),
        ))
    });
    // ID3D11InfoQueue::ClearRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::clear_retrieval_filter(this))
    });
    // ID3D11InfoQueue::PushEmptyRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::push_empty_retrieval_filter(this))
    });
    // ID3D11InfoQueue::PushCopyOfRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::push_copy_of_retrieval_filter(this))
    });
    // ID3D11InfoQueue::PopRetrievalFilter on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::pop_retrieval_filter(this))
    });
    // ID3D11InfoQueue::GetRetrievalFilterStackSize on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_retrieval_filter_stack_size(
            this,
        ))
    });
    // ID3D11InfoQueue::AddMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::add_message(
            this,
            Default::default(),
            Default::default(),
            Default::default(),
            c"",
        ))
    });
    // ID3D11InfoQueue::AddApplicationMessage on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::add_application_message(
            this,
            Default::default(),
            c"",
        ))
    });
    // ID3D11InfoQueue::SetBreakOnCategory on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::set_break_on_category(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::SetBreakOnSeverity on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::set_break_on_severity(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::SetBreakOnID on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::InfoQueue::set_break_on_id(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::GetBreakOnCategory on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_break_on_category(
            this,
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::GetBreakOnSeverity on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_break_on_severity(
            this,
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::GetBreakOnID on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_break_on_id(
            this,
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::SetMuteDebugOutput on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::set_mute_debug_output(
            this,
            Default::default(),
        ))
    });
    // ID3D11InfoQueue::GetMuteDebugOutput on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        answered(direct3d_11::InfoQueue::get_mute_debug_output(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k20.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on InputLayout
    driven.attempt(|| {
        let Some(this) = o.k21.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Multithread::Enter on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::enter(this))
    });
    // ID3D11Multithread::Leave on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::leave(this))
    });
    // ID3D11Multithread::SetMultithreadProtected on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::set_multithread_protected(
            this,
            Default::default(),
        ))
    });
    // ID3D11Multithread::GetMultithreadProtected on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
            return false;
        };
        answered(direct3d_11::Multithread::get_multithread_protected(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Multithread
    driven.attempt(|| {
        let Some(this) = o.k22.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on PixelShader
    driven.attempt(|| {
        let Some(this) = o.k23.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Query::GetDesc on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        answered(direct3d_11::Query::get_desc(this))
    });
    // ID3D11Asynchronous::GetDataSize on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        answered(direct3d_11::Asynchronous::get_data_size(this))
    });
    // ID3D11DeviceChild::GetDevice on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Predicate
    driven.attempt(|| {
        let Some(this) = o.k24.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Query1::GetDesc1 on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        answered(direct3d_11::Query1::get_desc1(this))
    });
    // ID3D11Query::GetDesc on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        answered(direct3d_11::Query::get_desc(this))
    });
    // ID3D11Asynchronous::GetDataSize on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        answered(direct3d_11::Asynchronous::get_data_size(this))
    });
    // ID3D11DeviceChild::GetDevice on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Query
    driven.attempt(|| {
        let Some(this) = o.k25.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11RasterizerState2::GetDesc2 on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        answered(direct3d_11::RasterizerState2::get_desc2(this))
    });
    // ID3D11RasterizerState1::GetDesc1 on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        answered(direct3d_11::RasterizerState1::get_desc1(this))
    });
    // ID3D11RasterizerState::GetDesc on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        answered(direct3d_11::RasterizerState::get_desc(this))
    });
    // ID3D11DeviceChild::GetDevice on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on RasterizerState
    driven.attempt(|| {
        let Some(this) = o.k26.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11RefDefaultTrackingOptions::SetTrackingOptions on RefDefaultTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k27.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::RefDefaultTrackingOptions::set_tracking_options(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on RefDefaultTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k27.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on RefDefaultTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k27.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11RefTrackingOptions::SetTrackingOptions on RefTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k28.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::RefTrackingOptions::set_tracking_options(
            this,
            Default::default(),
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on RefTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k28.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on RefTrackingOptions
    driven.attempt(|| {
        let Some(this) = o.k28.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11RenderTargetView1::GetDesc1 on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        answered(direct3d_11::RenderTargetView1::get_desc1(this))
    });
    // ID3D11RenderTargetView::GetDesc on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        answered(direct3d_11::RenderTargetView::get_desc(this))
    });
    // ID3D11View::GetResource on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on RenderTargetView
    driven.attempt(|| {
        let Some(this) = o.k29.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11SamplerState::GetDesc on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        answered(direct3d_11::SamplerState::get_desc(this))
    });
    // ID3D11DeviceChild::GetDevice on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on SamplerState
    driven.attempt(|| {
        let Some(this) = o.k30.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11ShaderResourceView1::GetDesc1 on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        answered(direct3d_11::ShaderResourceView1::get_desc1(this))
    });
    // ID3D11ShaderResourceView::GetDesc on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        answered(direct3d_11::ShaderResourceView::get_desc(this))
    });
    // ID3D11View::GetResource on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ShaderResourceView
    driven.attempt(|| {
        let Some(this) = o.k31.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11ShaderTrace::TraceReady on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::trace_ready(this))
    });
    // ID3D11ShaderTrace::ResetTrace on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        answered(direct3d_11::ShaderTrace::reset_trace(this))
    });
    // ID3D11ShaderTrace::GetTraceStats on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::get_trace_stats(this))
    });
    // ID3D11ShaderTrace::PSSelectStamp on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::ps_select_stamp(
            this,
            Default::default(),
        ))
    });
    // ID3D11ShaderTrace::GetStep on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::get_step(this, Default::default()))
    });
    // ID3D11ShaderTrace::GetWrittenRegister on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::get_written_register(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11ShaderTrace::GetReadRegister on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::ShaderTrace::get_read_register(
            this,
            Default::default(),
            Default::default(),
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ShaderTrace
    driven.attempt(|| {
        let Some(this) = o.k32.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on ShaderTraceFactory
    driven.attempt(|| {
        let Some(this) = o.k33.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on ShaderTraceFactory
    driven.attempt(|| {
        let Some(this) = o.k33.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11SwitchToRef::SetUseRef on SwitchToRef
    driven.attempt(|| {
        let Some(this) = o.k34.as_ref() else {
            return false;
        };
        answered(direct3d_11::SwitchToRef::set_use_ref(
            this,
            Default::default(),
        ))
    });
    // ID3D11SwitchToRef::GetUseRef on SwitchToRef
    driven.attempt(|| {
        let Some(this) = o.k34.as_ref() else {
            return false;
        };
        answered(direct3d_11::SwitchToRef::get_use_ref(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on SwitchToRef
    driven.attempt(|| {
        let Some(this) = o.k34.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on SwitchToRef
    driven.attempt(|| {
        let Some(this) = o.k34.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Texture1D::GetDesc on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        answered(direct3d_11::Texture1D::get_desc(this))
    });
    // ID3D11Resource::GetType on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_type(this))
    });
    // ID3D11Resource::SetEvictionPriority on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::set_eviction_priority(
            this,
            Default::default(),
        ))
    });
    // ID3D11Resource::GetEvictionPriority on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_eviction_priority(this))
    });
    // ID3D11DeviceChild::GetDevice on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // IDXGIObject::SetPrivateData on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
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
    // IDXGIObject::GetPrivateData on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIResource::GetSharedHandle on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource::GetEvictionPriority on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIResource1::CreateSubresourceSurface on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource1::CreateSharedHandle on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
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
    // ID3DDestructionNotifier::RegisterDestructionCallback on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Texture1D
    driven.attempt(|| {
        let Some(this) = o.k35.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Texture2D1::GetDesc1 on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::Texture2D1::get_desc1(this))
    });
    // ID3D11Texture2D::GetDesc on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::Texture2D::get_desc(this))
    });
    // ID3D11Resource::GetType on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_type(this))
    });
    // ID3D11Resource::SetEvictionPriority on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::set_eviction_priority(
            this,
            Default::default(),
        ))
    });
    // ID3D11Resource::GetEvictionPriority on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_eviction_priority(this))
    });
    // ID3D11DeviceChild::GetDevice on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // IDXGIObject::SetPrivateData on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
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
    // IDXGIObject::GetPrivateData on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIResource::GetSharedHandle on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource::GetEvictionPriority on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIResource1::CreateSubresourceSurface on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource1::CreateSharedHandle on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
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
    // IDXGISurface::GetDesc on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::get_desc(this))
    });
    // IDXGISurface::Map on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::map(
            this,
            Default::default(),
        ))
    });
    // IDXGISurface::Unmap on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface::unmap(this))
    });
    // IDXGISurface1::GetDC on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::get_dc(
            this,
            Default::default(),
        ))
    });
    // IDXGISurface1::ReleaseDC on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::release_dc(
            this, None,
        ))
    });
    // IDXGISurface2::GetResource on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface2::get_resource(
            this,
            &Default::default(),
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Texture2D
    driven.attempt(|| {
        let Some(this) = o.k36.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11Texture3D1::GetDesc1 on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::Texture3D1::get_desc1(this))
    });
    // ID3D11Texture3D::GetDesc on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::Texture3D::get_desc(this))
    });
    // ID3D11Resource::GetType on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_type(this))
    });
    // ID3D11Resource::SetEvictionPriority on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::set_eviction_priority(
            this,
            Default::default(),
        ))
    });
    // ID3D11Resource::GetEvictionPriority on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::Resource::get_eviction_priority(this))
    });
    // ID3D11DeviceChild::GetDevice on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // IDXGIObject::SetPrivateData on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::set_private_data(
            this,
            &Default::default(),
            &[],
        ))
    });
    // IDXGIObject::SetPrivateDataInterface on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
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
    // IDXGIObject::GetPrivateData on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(directx_graphics_infrastructure::Object::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // IDXGIObject::GetParent on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIResource::GetSharedHandle on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource::GetEvictionPriority on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIResource1::CreateSubresourceSurface on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIResource1::CreateSharedHandle on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
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
    // ID3DDestructionNotifier::RegisterDestructionCallback on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on Texture3D
    driven.attempt(|| {
        let Some(this) = o.k37.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11TracingDevice::SetShaderTrackingOptionsByType on TracingDevice
    driven.attempt(|| {
        let Some(this) = o.k38.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::TracingDevice::set_shader_tracking_options_by_type(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11TracingDevice::SetShaderTrackingOptions on TracingDevice
    driven.attempt(|| {
        let Some(this) = o.k38.as_ref() else {
            return false;
        };
        let Some(v1) = (if let Some(x) = o.k2.as_ref() {
            Some(direct3d_11::UnknownRef::AuthenticatedChannel(x))
        } else if let Some(x) = o.k3.as_ref() {
            Some(direct3d_11::UnknownRef::BlendState(x))
        } else if let Some(x) = o.k4.as_ref() {
            Some(direct3d_11::UnknownRef::Buffer(x))
        } else if let Some(x) = o.k5.as_ref() {
            Some(direct3d_11::UnknownRef::ClassInstance(x))
        } else if let Some(x) = o.k6.as_ref() {
            Some(direct3d_11::UnknownRef::ClassLinkage(x))
        } else if let Some(x) = o.k7.as_ref() {
            Some(direct3d_11::UnknownRef::CommandList(x))
        } else if let Some(x) = o.k8.as_ref() {
            Some(direct3d_11::UnknownRef::ComputeShader(x))
        } else if let Some(x) = o.k9.as_ref() {
            Some(direct3d_11::UnknownRef::Counter(x))
        } else if let Some(x) = o.k10.as_ref() {
            Some(direct3d_11::UnknownRef::CryptoSession(x))
        } else if let Some(x) = o.k11.as_ref() {
            Some(direct3d_11::UnknownRef::Debug(x))
        } else if let Some(x) = o.k12.as_ref() {
            Some(direct3d_11::UnknownRef::DepthStencilState(x))
        } else if let Some(x) = o.k13.as_ref() {
            Some(direct3d_11::UnknownRef::DepthStencilView(x))
        } else if let Some(x) = o.k14.as_ref() {
            Some(direct3d_11::UnknownRef::Device(x))
        } else if let Some(x) = o.k15.as_ref() {
            Some(direct3d_11::UnknownRef::DeviceContext(x))
        } else if let Some(x) = o.k16.as_ref() {
            Some(direct3d_11::UnknownRef::DomainShader(x))
        } else if let Some(x) = o.k17.as_ref() {
            Some(direct3d_11::UnknownRef::Fence(x))
        } else if let Some(x) = o.k18.as_ref() {
            Some(direct3d_11::UnknownRef::GeometryShader(x))
        } else if let Some(x) = o.k19.as_ref() {
            Some(direct3d_11::UnknownRef::HullShader(x))
        } else if let Some(x) = o.k20.as_ref() {
            Some(direct3d_11::UnknownRef::InfoQueue(x))
        } else if let Some(x) = o.k21.as_ref() {
            Some(direct3d_11::UnknownRef::InputLayout(x))
        } else if let Some(x) = o.k22.as_ref() {
            Some(direct3d_11::UnknownRef::Multithread(x))
        } else if let Some(x) = o.k23.as_ref() {
            Some(direct3d_11::UnknownRef::PixelShader(x))
        } else if let Some(x) = o.k24.as_ref() {
            Some(direct3d_11::UnknownRef::Predicate(x))
        } else if let Some(x) = o.k25.as_ref() {
            Some(direct3d_11::UnknownRef::Query(x))
        } else if let Some(x) = o.k26.as_ref() {
            Some(direct3d_11::UnknownRef::RasterizerState(x))
        } else if let Some(x) = o.k27.as_ref() {
            Some(direct3d_11::UnknownRef::RefDefaultTrackingOptions(x))
        } else if let Some(x) = o.k28.as_ref() {
            Some(direct3d_11::UnknownRef::RefTrackingOptions(x))
        } else if let Some(x) = o.k29.as_ref() {
            Some(direct3d_11::UnknownRef::RenderTargetView(x))
        } else if let Some(x) = o.k30.as_ref() {
            Some(direct3d_11::UnknownRef::SamplerState(x))
        } else if let Some(x) = o.k31.as_ref() {
            Some(direct3d_11::UnknownRef::ShaderResourceView(x))
        } else if let Some(x) = o.k32.as_ref() {
            Some(direct3d_11::UnknownRef::ShaderTrace(x))
        } else if let Some(x) = o.k33.as_ref() {
            Some(direct3d_11::UnknownRef::ShaderTraceFactory(x))
        } else if let Some(x) = o.k34.as_ref() {
            Some(direct3d_11::UnknownRef::SwitchToRef(x))
        } else if let Some(x) = o.k35.as_ref() {
            Some(direct3d_11::UnknownRef::Texture1D(x))
        } else if let Some(x) = o.k36.as_ref() {
            Some(direct3d_11::UnknownRef::Texture2D(x))
        } else if let Some(x) = o.k37.as_ref() {
            Some(direct3d_11::UnknownRef::Texture3D(x))
        } else if let Some(x) = o.k38.as_ref() {
            Some(direct3d_11::UnknownRef::TracingDevice(x))
        } else if let Some(x) = o.k39.as_ref() {
            Some(direct3d_11::UnknownRef::UnorderedAccessView(x))
        } else if let Some(x) = o.k40.as_ref() {
            Some(direct3d_11::UnknownRef::VertexShader(x))
        } else if let Some(x) = o.k41.as_ref() {
            Some(direct3d_11::UnknownRef::VideoContext(x))
        } else if let Some(x) = o.k42.as_ref() {
            Some(direct3d_11::UnknownRef::VideoDecoder(x))
        } else if let Some(x) = o.k43.as_ref() {
            Some(direct3d_11::UnknownRef::VideoDecoderOutputView(x))
        } else if let Some(x) = o.k44.as_ref() {
            Some(direct3d_11::UnknownRef::VideoDevice(x))
        } else if let Some(x) = o.k45.as_ref() {
            Some(direct3d_11::UnknownRef::VideoProcessor(x))
        } else if let Some(x) = o.k46.as_ref() {
            Some(direct3d_11::UnknownRef::VideoProcessorEnumerator(x))
        } else if let Some(x) = o.k47.as_ref() {
            Some(direct3d_11::UnknownRef::VideoProcessorInputView(x))
        } else if let Some(x) = o.k48.as_ref() {
            Some(direct3d_11::UnknownRef::VideoProcessorOutputView(x))
        } else if let Some(x) = o.k49.as_ref() {
            Some(direct3d_11::UnknownRef::DeviceContextState(x))
        } else if let Some(x) = o.k50.as_ref() {
            Some(direct3d_11::UnknownRef::UserDefinedAnnotation(x))
        } else {
            None
        }) else {
            return false;
        };
        failure_is_negative(direct3d_11::TracingDevice::set_shader_tracking_options(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on TracingDevice
    driven.attempt(|| {
        let Some(this) = o.k38.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on TracingDevice
    driven.attempt(|| {
        let Some(this) = o.k38.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11UnorderedAccessView1::GetDesc1 on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        answered(direct3d_11::UnorderedAccessView1::get_desc1(this))
    });
    // ID3D11UnorderedAccessView::GetDesc on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        answered(direct3d_11::UnorderedAccessView::get_desc(this))
    });
    // ID3D11View::GetResource on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on UnorderedAccessView
    driven.attempt(|| {
        let Some(this) = o.k39.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VertexShader
    driven.attempt(|| {
        let Some(this) = o.k40.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext3::DecoderBeginFrame1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        let Some(v2) = o.k43.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext3::decoder_begin_frame1(
            this,
            v1,
            v2,
            None,
            &[],
            &[],
        ))
    });
    // ID3D11VideoContext3::SubmitDecoderBuffers2 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext3::submit_decoder_buffers2(
            this,
            v1,
            &[],
        ))
    });
    // ID3D11VideoContext2::VideoProcessorSetOutputHDRMetaData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext2::video_processor_set_output_hdr_meta_data(
                this,
                v1,
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext2::VideoProcessorGetOutputHDRMetaData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext2::video_processor_get_output_hdr_meta_data(
                this,
                v1,
                Some(&lying),
            ),
        )
    });
    // ID3D11VideoContext2::VideoProcessorSetStreamHDRMetaData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext2::video_processor_set_stream_hdr_meta_data(
                this,
                v1,
                Default::default(),
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext2::VideoProcessorGetStreamHDRMetaData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext2::video_processor_get_stream_hdr_meta_data(
                this,
                v1,
                Default::default(),
                Some(&lying),
            ),
        )
    });
    // ID3D11VideoContext1::SubmitDecoderBuffers1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext1::submit_decoder_buffers1(
            this,
            v1,
            &[],
        ))
    });
    // ID3D11VideoContext1::GetDataForNewHardwareKey on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext1::get_data_for_new_hardware_key(
            this,
            v1,
            &[],
        ))
    });
    // ID3D11VideoContext1::CheckCryptoSessionStatus on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext1::check_crypto_session_status(
            this, v1,
        ))
    });
    // ID3D11VideoContext1::VideoProcessorSetOutputColorSpace1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_set_output_color_space1(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorSetOutputShaderUsage on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_set_output_shader_usage(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorGetOutputColorSpace1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext1::video_processor_get_output_color_space1(this, v1))
    });
    // ID3D11VideoContext1::VideoProcessorGetOutputShaderUsage on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext1::video_processor_get_output_shader_usage(this, v1))
    });
    // ID3D11VideoContext1::VideoProcessorSetStreamColorSpace1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_set_stream_color_space1(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorSetStreamMirror on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_set_stream_mirror(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorGetStreamColorSpace1 on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_get_stream_color_space1(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorGetStreamMirror on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext1::video_processor_get_stream_mirror(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext1::VideoProcessorGetBehaviorHints on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoContext1::video_processor_get_behavior_hints(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                &[],
            ),
        )
    });
    // ID3D11VideoContext::GetDecoderBuffer on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::VideoContext::get_decoder_buffer(
            this,
            v1,
            Default::default(),
            &lying,
        ))
    });
    // ID3D11VideoContext::ReleaseDecoderBuffer on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::release_decoder_buffer(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11VideoContext::DecoderBeginFrame on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        let Some(v2) = o.k43.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::decoder_begin_frame(
            this, v1, v2, None,
        ))
    });
    // ID3D11VideoContext::DecoderEndFrame on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::decoder_end_frame(this, v1))
    });
    // ID3D11VideoContext::SubmitDecoderBuffers on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::submit_decoder_buffers(
            this,
            v1,
            &[],
        ))
    });
    // ID3D11VideoContext::VideoProcessorSetOutputTargetRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_output_target_rect(
                this,
                v1,
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetOutputAlphaFillMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_output_alpha_fill_mode(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetOutputStereoMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_output_stereo_mode(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetOutputExtension on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext::video_processor_set_output_extension(
                this,
                v1,
                &Default::default(),
                Default::default(),
                &lying,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetOutputTargetRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_target_rect(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputBackgroundColor on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_background_color(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputColorSpace on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_color_space(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputAlphaFillMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_alpha_fill_mode(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputConstriction on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_constriction(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputStereoMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_output_stereo_mode(this, v1))
    });
    // ID3D11VideoContext::VideoProcessorGetOutputExtension on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext::video_processor_get_output_extension(
                this,
                v1,
                &Default::default(),
                &lying,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamFrameFormat on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_frame_format(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamOutputRate on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_output_rate(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamSourceRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_source_rect(
                this,
                v1,
                Default::default(),
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamDestRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_dest_rect(
                this,
                v1,
                Default::default(),
                Default::default(),
                None,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamAlpha on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_set_stream_alpha(
            this,
            v1,
            Default::default(),
            Default::default(),
            Default::default(),
        ))
    });
    // ID3D11VideoContext::VideoProcessorSetStreamPalette on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_palette(
                this,
                v1,
                Default::default(),
                &[],
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamPixelAspectRatio on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_pixel_aspect_ratio(
                this,
                v1,
                Default::default(),
                Default::default(),
                None,
                None,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamLumaKey on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_luma_key(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamStereoFormat on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_stereo_format(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamAutoProcessingMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_auto_processing_mode(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamFilter on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_filter(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorSetStreamExtension on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_extension(
                this,
                v1,
                Default::default(),
                &Default::default(),
                Default::default(),
                &lying,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamFrameFormat on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_frame_format(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamColorSpace on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_color_space(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamOutputRate on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_output_rate(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamSourceRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_source_rect(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamDestRect on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_dest_rect(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamAlpha on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::video_processor_get_stream_alpha(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11VideoContext::VideoProcessorGetStreamPalette on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_palette(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamPixelAspectRatio on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_pixel_aspect_ratio(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamLumaKey on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_luma_key(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamStereoFormat on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_stereo_format(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamAutoProcessingMode on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_auto_processing_mode(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamFilter on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_filter(
                this,
                v1,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamExtension on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_extension(
                this,
                v1,
                Default::default(),
                &Default::default(),
                &lying,
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorBlt on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        let Some(v2) = o.k48.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::video_processor_blt(
            this,
            v1,
            v2,
            Default::default(),
            &[],
        ))
    });
    // ID3D11VideoContext::NegotiateCryptoSessionKeyExchange on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_11::VideoContext::negotiate_crypto_session_key_exchange(this, v1, &lying),
        )
    });
    // ID3D11VideoContext::EncryptionBlt on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let Some(v2) = o.k36.as_ref() else {
            return false;
        };
        let Some(v3) = o.k36.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(direct3d_11::VideoContext::encryption_blt(
            this,
            v1,
            v2,
            v3,
            Some(&lying),
        ))
    });
    // ID3D11VideoContext::DecryptionBlt on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let Some(v2) = o.k36.as_ref() else {
            return false;
        };
        let Some(v3) = o.k36.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(direct3d_11::VideoContext::decryption_blt(
            this,
            v1,
            v2,
            v3,
            None,
            None,
            Some(&lying),
        ))
    });
    // ID3D11VideoContext::StartSessionKeyRefresh on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        answered(direct3d_11::VideoContext::start_session_key_refresh(
            this, v1, &lying,
        ))
    });
    // ID3D11VideoContext::FinishSessionKeyRefresh on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoContext::finish_session_key_refresh(
            this, v1,
        ))
    });
    // ID3D11VideoContext::GetEncryptionBltKey on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::VideoContext::get_encryption_blt_key(
            this, v1, &lying,
        ))
    });
    // ID3D11VideoContext::NegotiateAuthenticatedChannelKeyExchange on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k2.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_11::VideoContext::negotiate_authenticated_channel_key_exchange(
                this, v1, &lying,
            ),
        )
    });
    // ID3D11VideoContext::QueryAuthenticatedChannel on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k2.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::VideoContext::query_authenticated_channel(
            this,
            v1,
            &[],
            &lying,
        ))
    });
    // ID3D11VideoContext::ConfigureAuthenticatedChannel on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k2.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoContext::configure_authenticated_channel(
            this,
            v1,
            &[],
        ))
    });
    // ID3D11VideoContext::VideoProcessorSetStreamRotation on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_set_stream_rotation(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoContext::VideoProcessorGetStreamRotation on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let Some(v1) = o.k45.as_ref() else {
            return false;
        };
        answered(
            direct3d_11::VideoContext::video_processor_get_stream_rotation(
                this,
                v1,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoContext
    driven.attempt(|| {
        let Some(this) = o.k41.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoDecoder::GetCreationParameters on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDecoder::get_creation_parameters(this))
    });
    // ID3D11VideoDecoder::GetDriverHandle on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDecoder::get_driver_handle(this))
    });
    // ID3D11DeviceChild::GetDevice on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoDecoder
    driven.attempt(|| {
        let Some(this) = o.k42.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoDecoderOutputView::GetDesc on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoDecoderOutputView::get_desc(this))
    });
    // ID3D11View::GetResource on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoDecoderOutputView
    driven.attempt(|| {
        let Some(this) = o.k43.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoDevice2::CheckFeatureSupport on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::VideoDevice2::check_feature_support(
            this,
            Default::default(),
            &lying,
        ))
    });
    // ID3D11VideoDevice2::NegotiateCryptoSessionKeyExchangeMT on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        let Some(v1) = o.k10.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_11::VideoDevice2::negotiate_crypto_session_key_exchange_mt(
                this,
                v1,
                Default::default(),
                &lying,
            ),
        )
    });
    // ID3D11VideoDevice1::GetCryptoSessionPrivateDataSize on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoDevice1::get_crypto_session_private_data_size(
                this,
                &Default::default(),
                None,
                &Default::default(),
            ),
        )
    });
    // ID3D11VideoDevice::CreateVideoProcessor on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        let Some(v1) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::create_video_processor(
            this,
            v1,
            Default::default(),
        ))
    });
    // ID3D11VideoDevice::CreateAuthenticatedChannel on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::create_authenticated_channel(
            this,
            Default::default(),
        ))
    });
    // ID3D11VideoDevice::CreateCryptoSession on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::create_crypto_session(
            this,
            &Default::default(),
            None,
            &Default::default(),
        ))
    });
    // ID3D11VideoDevice::GetVideoDecoderProfileCount on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoDevice::get_video_decoder_profile_count(
            this,
        ))
    });
    // ID3D11VideoDevice::GetVideoDecoderProfile on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::get_video_decoder_profile(
            this,
            Default::default(),
        ))
    });
    // ID3D11VideoDevice::CheckVideoDecoderFormat on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::check_video_decoder_format(
            this,
            &Default::default(),
            Default::default(),
        ))
    });
    // ID3D11VideoDevice::GetContentProtectionCaps on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::get_content_protection_caps(
            this, None, None,
        ))
    });
    // ID3D11VideoDevice::CheckCryptoKeyExchange on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::check_crypto_key_exchange(
            this,
            &Default::default(),
            None,
            Default::default(),
        ))
    });
    // ID3D11VideoDevice::SetPrivateData on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11VideoDevice::SetPrivateDataInterface on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoDevice::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoDevice
    driven.attempt(|| {
        let Some(this) = o.k44.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessor::GetContentDesc on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoProcessor::get_content_desc(this))
    });
    // ID3D11VideoProcessor::GetRateConversionCaps on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoProcessor::get_rate_conversion_caps(this))
    });
    // ID3D11DeviceChild::GetDevice on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoProcessor
    driven.attempt(|| {
        let Some(this) = o.k45.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorEnumerator1::CheckVideoProcessorFormatConversion on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator1::check_video_processor_format_conversion(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorEnumerator::GetVideoProcessorContentDesc on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator::get_video_processor_content_desc(this),
        )
    });
    // ID3D11VideoProcessorEnumerator::CheckVideoProcessorFormat on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator::check_video_processor_format(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorEnumerator::GetVideoProcessorCaps on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::VideoProcessorEnumerator::get_video_processor_caps(this))
    });
    // ID3D11VideoProcessorEnumerator::GetVideoProcessorRateConversionCaps on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator::get_video_processor_rate_conversion_caps(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorEnumerator::GetVideoProcessorCustomRate on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator::get_video_processor_custom_rate(
                this,
                Default::default(),
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorEnumerator::GetVideoProcessorFilterRange on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_11::VideoProcessorEnumerator::get_video_processor_filter_range(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoProcessorEnumerator
    driven.attempt(|| {
        let Some(this) = o.k46.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorInputView::GetDesc on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoProcessorInputView::get_desc(this))
    });
    // ID3D11View::GetResource on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoProcessorInputView
    driven.attempt(|| {
        let Some(this) = o.k47.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11VideoProcessorOutputView::GetDesc on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        answered(direct3d_11::VideoProcessorOutputView::get_desc(this))
    });
    // ID3D11View::GetResource on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        answered(direct3d_11::View::get_resource(this))
    });
    // ID3D11DeviceChild::GetDevice on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on VideoProcessorOutputView
    driven.attempt(|| {
        let Some(this) = o.k48.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3D11DeviceChild::GetDevice on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
            return false;
        };
        answered(direct3d_11::DeviceChild::get_device(this))
    });
    // ID3D11DeviceChild::GetPrivateData on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(direct3d_11::DeviceChild::get_private_data(
            this,
            &Default::default(),
            Some(&lying),
        ))
    });
    // ID3D11DeviceChild::SetPrivateData on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3D11DeviceChild::SetPrivateDataInterface on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
            return false;
        };
        failure_is_negative(direct3d_11::DeviceChild::set_private_data_interface(
            this,
            &Default::default(),
            None,
        ))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DeviceContextState
    driven.attempt(|| {
        let Some(this) = o.k49.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // ID3DUserDefinedAnnotation::BeginEvent on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::begin_event(this, &[]))
    });
    // ID3DUserDefinedAnnotation::EndEvent on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::end_event(this))
    });
    // ID3DUserDefinedAnnotation::SetMarker on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::set_marker(this, &[]))
    });
    // ID3DUserDefinedAnnotation::GetStatus on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
            return false;
        };
        answered(direct3d_11::UserDefinedAnnotation::get_status(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
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
    // ID3DDestructionNotifier::UnregisterDestructionCallback on UserDefinedAnnotation
    driven.attempt(|| {
        let Some(this) = o.k50.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    // IDXGIAdapter4::GetDesc3 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter4::get_desc3(this))
    });
    // IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::Adapter3::register_hardware_content_protection_teardown_status_event(this, Default::default()))
    });
    // IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::Adapter3::unregister_hardware_content_protection_teardown_status(this, Default::default()))
    });
    // IDXGIAdapter3::QueryVideoMemoryInfo on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
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
        let Some(this) = o.k61.as_ref() else {
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
        let Some(this) = o.k61.as_ref() else { return false; };
        failure_is_negative(directx_graphics_infrastructure::Adapter3::register_video_memory_budget_change_notification_event(this, Default::default()))
    });
    // IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::Adapter3::unregister_video_memory_budget_change_notification(this, Default::default()))
    });
    // IDXGIAdapter2::GetDesc2 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter2::get_desc2(this))
    });
    // IDXGIAdapter1::GetDesc1 on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter1::get_desc1(this))
    });
    // IDXGIAdapter::EnumOutputs on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter::enum_outputs(
            this,
            Default::default(),
        ))
    });
    // IDXGIAdapter::GetDesc on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Adapter::get_desc(this))
    });
    // IDXGIAdapter::CheckInterfaceSupport on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
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
        let Some(this) = o.k61.as_ref() else {
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
    // IDXGIObject::GetPrivateData on Adapter
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
    // IDXGIObject::GetParent on Adapter
    driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDebug1::EnableLeakTrackingForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::enable_leak_tracking_for_thread(this))
    });
    // IDXGIDebug1::DisableLeakTrackingForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::disable_leak_tracking_for_thread(this))
    });
    // IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Debug1::is_leak_tracking_enabled_for_thread(this))
    });
    // IDXGIDebug::ReportLiveObjects on Debug
    driven.attempt(|| {
        let Some(this) = o.k62.as_ref() else {
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
        let Some(this) = o.k63.as_ref() else {
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
        let Some(this) = o.k63.as_ref() else {
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
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_source_rect(this))
    });
    // IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_target_rect(this))
    });
    // IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::DecodeSwapChain::get_dest_size(this))
    });
    // IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain
    driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else {
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
        let Some(this) = o.k63.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::DecodeSwapChain::get_color_space(this))
    });
    // IDXGIDevice4::OfferResources1 on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
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
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device4::reclaim_resources1(this, &[]))
    });
    // IDXGIDevice3::Trim on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Device3::trim(this))
    });
    // IDXGIDevice2::OfferResources on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
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
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::reclaim_resources(
            this,
            &[],
        ))
    });
    // IDXGIDevice2::EnqueueSetEvent on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device2::enqueue_set_event(
            this,
            Default::default(),
        ))
    });
    // IDXGIDevice1::SetMaximumFrameLatency on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
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
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device1::get_maximum_frame_latency(this),
        )
    });
    // IDXGIDevice::GetAdapter on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_adapter(this))
    });
    // IDXGIDevice::QueryResourceResidency on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Device::query_resource_residency(this, &[]),
        )
    });
    // IDXGIDevice::SetGPUThreadPriority on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
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
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Device::get_gpu_thread_priority(this))
    });
    // IDXGIObject::SetPrivateData on Device
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
    // IDXGIObject::SetPrivateDataInterface on Device
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
    // IDXGIObject::GetPrivateData on Device
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
    // IDXGIObject::GetParent on Device
    driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIDisplayControl::IsStereoEnabled on DisplayControl
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::DisplayControl::is_stereo_enabled(this))
    });
    // IDXGIDisplayControl::SetStereoEnabled on DisplayControl
    driven.attempt(|| {
        let Some(this) = o.k65.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Factory4::enum_warp_adapter(this, &Default::default()),
        )
    });
    // IDXGIFactory3::GetCreationFlags on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory3::get_creation_flags(this))
    });
    // IDXGIFactory2::IsWindowedStereoEnabled on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory2::is_windowed_stereo_enabled(this))
    });
    // IDXGIFactory2::GetSharedResourceAdapterLuid on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory1::enum_adapters1(
            this,
            Default::default(),
        ))
    });
    // IDXGIFactory1::IsCurrent on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Factory1::is_current(this))
    });
    // IDXGIFactory::EnumAdapters on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory::enum_adapters(
            this,
            Default::default(),
        ))
    });
    // IDXGIFactory::MakeWindowAssociation on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Factory::get_window_association(this))
    });
    // IDXGIFactory::CreateSoftwareAdapter on Factory
    driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
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
        let Some(this) = o.k66.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIInfoQueue::SetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::InfoQueue::get_num_stored_messages_allowed_by_retrieval_filters(this, Default::default()))
    });
    // IDXGIInfoQueue::GetNumStoredMessages on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else { return false; };
        answered(directx_graphics_infrastructure::InfoQueue::get_num_messages_discarded_by_message_count_limit(this, Default::default()))
    });
    // IDXGIInfoQueue::GetMessageCountLimit on InfoQueue
    driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k68.as_ref() else {
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
        let Some(this) = o.k69.as_ref() else {
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
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::KeyedMutex::release_sync(
            this,
            Default::default(),
        ))
    });
    // IDXGIDeviceSubObject::GetDevice on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on KeyedMutex
    driven.attempt(|| {
        let Some(this) = o.k69.as_ref() else {
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
        let Some(this) = o.k69.as_ref() else {
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
        let Some(this) = o.k69.as_ref() else {
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
        let Some(this) = o.k69.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIOutput6::GetDesc1 on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output6::get_desc1(this))
    });
    // IDXGIOutput6::CheckHardwareCompositionSupport on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output6::check_hardware_composition_support(this),
        )
    });
    // IDXGIOutput5::DuplicateOutput1 on Output
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
        failure_is_negative(directx_graphics_infrastructure::Output5::duplicate_output1(
            this,
            v1,
            Default::default(),
            &[],
        ))
    });
    // IDXGIOutput4::CheckOverlayColorSpaceSupport on Output
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
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Output2::supports_overlays(
            this,
        ))
    });
    // IDXGIOutput1::GetDisplayModeList1 on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
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
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output1::get_display_surface_data1(this, v1),
        )
    });
    // IDXGIOutput1::DuplicateOutput on Output
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
        failure_is_negative(directx_graphics_infrastructure::Output1::duplicate_output(
            this, v1,
        ))
    });
    // IDXGIOutput::GetDesc on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_desc(this))
    });
    // IDXGIOutput::GetDisplayModeList on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
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
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::wait_for_v_blank(
            this,
        ))
    });
    // IDXGIOutput::TakeOwnership on Output
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
        failure_is_negative(directx_graphics_infrastructure::Output::take_ownership(
            this,
            v1,
            Default::default(),
        ))
    });
    // IDXGIOutput::ReleaseOwnership on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::Output::release_ownership(
            this,
        ))
    });
    // IDXGIOutput::GetGammaControlCapabilities on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output::get_gamma_control_capabilities(this),
        )
    });
    // IDXGIOutput::GetGammaControl on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_gamma_control(
            this,
        ))
    });
    // IDXGIOutput::SetDisplaySurface on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::set_display_surface(this, v1))
    });
    // IDXGIOutput::GetDisplaySurfaceData on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        let Some(v1) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::Output::get_display_surface_data(this, v1),
        )
    });
    // IDXGIOutput::GetFrameStatistics on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Output::get_frame_statistics(this))
    });
    // IDXGIObject::SetPrivateData on Output
    driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else {
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
        let Some(this) = o.k70.as_ref() else {
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
        let Some(this) = o.k70.as_ref() else {
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
        let Some(this) = o.k70.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIOutputDuplication::GetDesc on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::OutputDuplication::get_desc(this))
    });
    // IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
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
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::get_frame_dirty_rects(this, &lying),
        )
    });
    // IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            directx_graphics_infrastructure::OutputDuplication::get_frame_move_rects(this, &lying),
        )
    });
    // IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
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
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::OutputDuplication::release_frame(this))
    });
    // IDXGIObject::SetPrivateData on OutputDuplication
    driven.attempt(|| {
        let Some(this) = o.k71.as_ref() else {
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
        let Some(this) = o.k71.as_ref() else {
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
        let Some(this) = o.k71.as_ref() else {
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
        let Some(this) = o.k71.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGIResource1::CreateSubresourceSurface on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_shared_handle(this))
    });
    // IDXGIResource::GetUsage on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_usage(this))
    });
    // IDXGIResource::SetEvictionPriority on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Resource::get_eviction_priority(this))
    });
    // IDXGIDeviceSubObject::GetDevice on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on Resource
    driven.attempt(|| {
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
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
        let Some(this) = o.k72.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISurface2::GetResource on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface2::get_resource(
            this,
            &Default::default(),
        ))
    });
    // IDXGISurface1::GetDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::get_dc(
            this,
            Default::default(),
        ))
    });
    // IDXGISurface1::ReleaseDC on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Surface1::release_dc(
            this, None,
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
            Default::default(),
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
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on Surface
    driven.attempt(|| {
        let Some(this) = o.k73.as_ref() else {
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
        let Some(this) = o.k73.as_ref() else {
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
        let Some(this) = o.k73.as_ref() else {
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
        let Some(this) = o.k73.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISwapChain4::SetHDRMetaData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::SwapChain3::get_current_back_buffer_index(this))
    });
    // IDXGISwapChain3::CheckColorSpaceSupport on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain3::set_color_space1(this, Default::default()),
        )
    });
    // IDXGISwapChain3::ResizeBuffers1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain2::get_source_size(this))
    });
    // IDXGISwapChain2::SetMaximumFrameLatency on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain2::get_maximum_frame_latency(this),
        )
    });
    // IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered(
            directx_graphics_infrastructure::SwapChain2::get_frame_latency_waitable_object(this),
        )
    });
    // IDXGISwapChain2::GetMatrixTransform on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain2::get_matrix_transform(this))
    });
    // IDXGISwapChain1::GetDesc1 on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_desc1(this))
    });
    // IDXGISwapChain1::GetFullscreenDesc on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_fullscreen_desc(this))
    });
    // IDXGISwapChain1::GetHwnd on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_hwnd(this))
    });
    // IDXGISwapChain1::GetCoreWindow on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain1::get_core_window(this, &Default::default()),
        )
    });
    // IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::SwapChain1::is_temporary_mono_supported(this))
    });
    // IDXGISwapChain1::GetRestrictToOutput on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain1::get_restrict_to_output(this),
        )
    });
    // IDXGISwapChain1::GetBackgroundColor on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_background_color(this))
    });
    // IDXGISwapChain1::SetRotation on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::set_rotation(
            this,
            Default::default(),
        ))
    });
    // IDXGISwapChain1::GetRotation on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain1::get_rotation(
            this,
        ))
    });
    // IDXGISwapChain::Present on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_buffer(
            this,
            Default::default(),
        ))
    });
    // IDXGISwapChain::SetFullscreenState on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this))
    });
    // IDXGISwapChain::GetDesc on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_desc(this))
    });
    // IDXGISwapChain::ResizeBuffers on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_containing_output(this))
    });
    // IDXGISwapChain::GetFrameStatistics on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::SwapChain::get_frame_statistics(this))
    });
    // IDXGISwapChain::GetLastPresentCount on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChain::get_last_present_count(this),
        )
    });
    // IDXGIDeviceSubObject::GetDevice on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::DeviceSubObject::get_device(this, &Default::default()),
        )
    });
    // IDXGIObject::SetPrivateData on SwapChain
    driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
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
        let Some(this) = o.k74.as_ref() else {
            return false;
        };
        failure_is_negative(directx_graphics_infrastructure::Object::get_parent(
            this,
            &Default::default(),
        ))
    });
    // IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia
    driven.attempt(|| {
        let Some(this) = o.k75.as_ref() else {
            return false;
        };
        failure_is_negative(
            directx_graphics_infrastructure::SwapChainMedia::get_frame_statistics_media(this),
        )
    });
    // IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia
    driven.attempt(|| {
        let Some(this) = o.k75.as_ref() else {
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
        let Some(this) = o.k75.as_ref() else {
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
        let Some(this) = o.k76.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::GraphicsAnalysis::begin_capture(this))
    });
    // IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis
    driven.attempt(|| {
        let Some(this) = o.k76.as_ref() else {
            return false;
        };
        answered(directx_graphics_infrastructure::GraphicsAnalysis::end_capture(this))
    });
    // D3D11CreateDevice
    driven.attempt(|| {
        failure_is_negative(direct3d_11::Direct3D11::create_device(
            &o.api,
            None,
            Default::default(),
            Default::default(),
            Default::default(),
            &[],
            Default::default(),
        ))
    });
    // D3D11CreateDeviceAndSwapChain
    driven.attempt(|| {
        failure_is_negative(direct3d_11::Direct3D11::create_device_and_swap_chain(
            &o.api,
            None,
            Default::default(),
            Default::default(),
            Default::default(),
            &[],
            Default::default(),
            None,
        ))
    });
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 945;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: direct3d_11::Direct3D11>(api: A) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o);
    println!(
        "SUITE direct3d-11: kinds made {kinds}, calls {CALLS}: called {}, skipped {}, panicked {}",
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
