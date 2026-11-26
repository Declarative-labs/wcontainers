//! Pass-through on Linux: a toy implementation of the slice, served by the serving glue as C entry points, and

//! called back through the calling glue as if it were the system's library. Every call crosses the C ABI twice;
//! the test checks the answers, COM's identity rules (a child's device and the immediate context are the objects
//! created first, not new ones) and that every object is released when the last reference goes.

#![allow(unused_variables, reason = "the toy ignores most of what it is given")]

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::constants::{E_NOTIMPL, E_INVALIDARG};
use wcontainers_component_object_model::structures::{Guid, HResult, Hmodule};
use wcontainers_direct3d_11 as d3d11;
use wcontainers_direct3d_11::{AsynchronousRef, Device as _, DeviceChild as _, DeviceContext as _, Direct3D11 as _, Query as _, Asynchronous as _, ResourceRef, UnknownRef};
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map, QueryKind, ResourceDimension};
use wcontainers_direct3d_11::structures::{BufferDesc, DepthStencilViewDesc, MappedSubresource, QueryDesc, RenderTargetViewDesc, SubresourceData, Texture2DDesc, UnorderedAccessViewDesc};
use wcontainers_direct3d_11_direct::{SystemDirect3D11, exports};
use wcontainers_direct3d_common as d3dcommon;
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure as dxgi;
use wcontainers_directx_graphics_infrastructure::enumerations::Present;
use wcontainers_directx_graphics_infrastructure::structures::{SurfaceDesc, SwapChainDesc};
use wcontainers_volatile_bytes::VolatileBytes;

/// Toy objects alive now, across the test (one test per process: this file holds one).
static LIVE: AtomicI64 = AtomicI64::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static BEGUN: AtomicU32 = AtomicU32::new(0);
const DEVICE: u8 = 0;
const CONTEXT: u8 = 1;
const QUERY: u8 = 2;
const OTHER: u8 = 3;
const QUERY_DATA: [u8; 4] = [1, 2, 3, 4];

struct Data {
    query: Option<QueryDesc>,
    context: OnceLock<Toy<CONTEXT>>,
}

impl Drop for Data {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One toy object of a kind; equal only to its clones.
struct Toy<const KIND: u8>(Arc<Data>);

impl<const KIND: u8> Toy<KIND> {
    fn new(query: Option<QueryDesc>) -> Toy<KIND> {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Toy(Arc::new(Data { query, context: OnceLock::new() }))
    }
}

impl<const KIND: u8> Clone for Toy<KIND> {
    fn clone(&self) -> Toy<KIND> {
        Toy(Arc::clone(&self.0))
    }
}

impl<const KIND: u8> PartialEq for Toy<KIND> {
    fn eq(&self, other: &Toy<KIND>) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<const KIND: u8> Eq for Toy<KIND> {}

impl<const KIND: u8> Hash for Toy<KIND> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

#[derive(Clone, Copy)]
struct ToyApi;

impl d3d11::Direct3D11 for ToyApi {
    type Buffer = Toy<OTHER>;
    type DepthStencilView = Toy<OTHER>;
    type Device = Toy<DEVICE>;
    type DeviceContext = Toy<CONTEXT>;
    type Query = Toy<QUERY>;
    type RenderTargetView = Toy<OTHER>;
    type Texture2D = Toy<OTHER>;
    type UnorderedAccessView = Toy<OTHER>;
    type Unknown = Toy<OTHER>;

    fn create_device(&self, adapter: Option<&Toy<OTHER>>, driver_type: DriverType, software: Hmodule, flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32) -> Result<(Toy<DEVICE>, FeatureLevel, Toy<CONTEXT>), HResult> {
        let level = *feature_levels.first().ok_or(E_INVALIDARG)?;
        let device = Toy::<DEVICE>::new(None);
        let context = Toy::<CONTEXT>::new(None);
        let _ = device.0.context.set(context.clone());
        Ok((device, level, context))
    }

    fn create_device_and_swap_chain(&self, adapter: Option<&Toy<OTHER>>, driver_type: DriverType, software: Hmodule, flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32, swap_chain_desc: Option<&SwapChainDesc>) -> Result<(Toy<OTHER>, Toy<DEVICE>, FeatureLevel, Toy<CONTEXT>), HResult> {
        Err(E_NOTIMPL)
    }
}

impl d3dcommon::Direct3DCommon for ToyApi {
    type Blob = Toy<OTHER>;
    type DestructionNotifier = Toy<OTHER>;
}

impl dxgi::DirectXGraphicsInfrastructure for ToyApi {
    type Adapter = Toy<OTHER>;
    type Surface = Toy<OTHER>;
    type SwapChain = Toy<OTHER>;
    type Unknown = Toy<OTHER>;
}

impl<const KIND: u8> d3dcommon::Blob for Toy<KIND> {
    fn get_buffer_pointer(&self) -> u8 {
        0
    }
    fn get_buffer_size(&self) -> usize {
        0
    }
}

impl<const KIND: u8> d3dcommon::DestructionNotifier for Toy<KIND> {
    fn register_destruction_callback(&self, callback_fn: usize, data: &impl VolatileBytes) -> Result<u32, HResult> {
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(&self, callback_id: u32) -> Result<(), HResult> {
        Err(E_NOTIMPL)
    }
}

impl<const KIND: u8> dxgi::Object for Toy<KIND> {
    type Api = ToyApi;
}
impl<const KIND: u8> dxgi::DeviceSubObject for Toy<KIND> {}
impl<const KIND: u8> dxgi::Adapter for Toy<KIND> {}
impl<const KIND: u8> dxgi::Surface for Toy<KIND> {
    fn get_desc(&self) -> Result<SurfaceDesc, HResult> {
        Err(E_NOTIMPL)
    }
}
impl<const KIND: u8> dxgi::SwapChain for Toy<KIND> {
    fn present(&self, sync_interval: u32, flags: Present) -> Result<(), HResult> {
        Err(E_NOTIMPL)
    }
    fn get_buffer(&self, buffer: u32, riid: &Guid) -> Result<dxgi::UnknownObject<ToyApi>, HResult> {
        Err(E_NOTIMPL)
    }
}

impl<const KIND: u8> d3d11::DeviceChild for Toy<KIND> {
    type Api = ToyApi;
    fn get_device(&self) -> Option<Toy<DEVICE>> {
        // The glue answers from what it recorded; a new device here would break COM's identity.
        Some(Toy::new(None))
    }
    fn set_private_data_interface(&self, guid: &Guid, data: Option<UnknownRef<'_, ToyApi>>) -> Result<(), HResult> {
        Err(E_NOTIMPL)
    }
}
impl<const KIND: u8> d3d11::Resource for Toy<KIND> {
    fn get_type(&self) -> ResourceDimension {
        ResourceDimension(1)
    }
}
impl<const KIND: u8> d3d11::Buffer for Toy<KIND> {
    fn get_desc(&self) -> BufferDesc {
        BufferDesc { byte_width: 0, usage: d3d11::enumerations::Usage(0), bind_flags: 0, cpu_access_flags: 0, misc_flags: 0, structure_byte_stride: 0 }
    }
}
impl<const KIND: u8> d3d11::Texture2D for Toy<KIND> {
    fn get_desc(&self) -> Texture2DDesc {
        Texture2DDesc {
            width: 0,
            height: 0,
            mip_levels: 0,
            array_size: 0,
            format: dxgi::enumerations::Format(0),
            sample_desc: dxgi::structures::SampleDesc { count: 0, quality: 0 },
            usage: d3d11::enumerations::Usage(0),
            bind_flags: 0,
            cpu_access_flags: 0,
            misc_flags: 0,
        }
    }
}
impl<const KIND: u8> d3d11::View for Toy<KIND> {}
impl<const KIND: u8> d3d11::RenderTargetView for Toy<KIND> {}
impl<const KIND: u8> d3d11::DepthStencilView for Toy<KIND> {}
impl<const KIND: u8> d3d11::UnorderedAccessView for Toy<KIND> {}
impl<const KIND: u8> d3d11::Asynchronous for Toy<KIND> {
    fn get_data_size(&self) -> u32 {
        QUERY_DATA.len() as u32
    }
}
impl<const KIND: u8> d3d11::Query for Toy<KIND> {
    fn get_desc(&self) -> QueryDesc {
        self.0.query.unwrap_or(QueryDesc { query: QueryKind(-1), misc_flags: 0 })
    }
}

impl<const KIND: u8> d3d11::Device for Toy<KIND> {
    type Api = ToyApi;

    fn create_buffer(&self, desc: &BufferDesc, initial_data: Option<&SubresourceData>) -> Result<Toy<OTHER>, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_texture_2d(&self, desc: &Texture2DDesc, initial_data: Option<&SubresourceData>) -> Result<Toy<OTHER>, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_unordered_access_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&UnorderedAccessViewDesc>) -> Result<Toy<OTHER>, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_render_target_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&RenderTargetViewDesc>) -> Result<Toy<OTHER>, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_depth_stencil_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&DepthStencilViewDesc>) -> Result<Toy<OTHER>, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_query(&self, query_desc: &QueryDesc) -> Result<Toy<QUERY>, HResult> {
        Ok(Toy::new(Some(*query_desc)))
    }
    fn get_immediate_context(&self) -> Option<Toy<CONTEXT>> {
        self.0.context.get().cloned()
    }
}

impl<const KIND: u8> d3d11::DeviceContext for Toy<KIND> {
    fn draw(&self, vertex_count: u32, start_vertex_location: u32) {
        DRAWS.fetch_add(vertex_count, Ordering::SeqCst);
    }
    fn map(&self, resource: ResourceRef<'_, ToyApi>, subresource: u32, map_type: Map, map_flags: u32) -> Result<MappedSubresource, HResult> {
        Err(E_NOTIMPL)
    }
    fn unmap(&self, resource: ResourceRef<'_, ToyApi>, subresource: u32) {}
    fn ia_set_vertex_buffers(&self, start_slot: u32, vertex_buffers: &[Option<&Toy<OTHER>>], strides: &[u32], offsets: &[u32]) {}
    fn begin(&self, async_: AsynchronousRef<'_, ToyApi>) {
        BEGUN.fetch_add(1, Ordering::SeqCst);
    }
    fn end(&self, async_: AsynchronousRef<'_, ToyApi>) {}
    fn get_data(&self, async_: AsynchronousRef<'_, ToyApi>, data: Option<&impl VolatileBytes>, get_data_flags: u32) -> Result<(), HResult> {
        if let Some(data) = data {
            data.write(&QUERY_DATA);
        }
        Ok(())
    }
    fn om_set_render_targets_and_unordered_access_views(&self, render_target_views: &[Option<&Toy<OTHER>>], depth_stencil_view: Option<&Toy<OTHER>>, uav_start_slot: u32, unordered_access_views: &[Option<&Toy<OTHER>>], uav_initial_counts: &[u32]) {}
}

/// Bytes of ours behind a lock, as a caller's buffer.
struct Bytes(Mutex<Vec<u8>>);

impl VolatileBytes for Bytes {
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
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        sink.write(&copy)
    }
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![0_u8; self.0.lock().unwrap_or_else(PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

#[test]
fn the_toy_through_both_glues() {
    let library = exports(ToyApi);
    #[expect(unsafe_code, reason = "the library is our own serving glue's exports, which take what the metadata gives")]
    // SAFETY: each entry is the serving glue's export of that name, generated from the same metadata.
    let system = unsafe { SystemDirect3D11::new(library) };
    let asked = [FeatureLevel(0xb000), FeatureLevel(0xa100)];
    let (device, level, context) = system.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &asked, 7).expect("a device");
    assert_eq!(level, FeatureLevel(0xb000), "the feature level crossed both glues");
    assert!(system.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &[], 7).is_err(), "the toy's failure crossed back");

    // COM's identity: the immediate context and a child's device are the objects created, not new ones.
    assert!(device.get_immediate_context() == Some(context.clone()), "GetImmediateContext answers the context created with the device");
    assert!(context.get_device() == Some(device.clone()), "the context's device is the device it was created with");

    let desc = QueryDesc { query: QueryKind(4), misc_flags: 1 };
    let query = device.create_query(&desc).expect("a query");
    assert_eq!(query.get_desc(), desc, "a description crossed both glues both ways");
    assert_eq!(query.get_data_size(), QUERY_DATA.len() as u32);
    assert!(query.get_device() == Some(device.clone()), "a child's device is the device that made it");

    context.draw(3, 0);
    context.draw(4, 0);
    assert_eq!(DRAWS.load(Ordering::SeqCst), 7);
    context.begin(AsynchronousRef::Query(&query));
    context.end(AsynchronousRef::Query(&query));
    assert_eq!(BEGUN.load(Ordering::SeqCst), 1);
    let bytes = Bytes(Mutex::new(vec![0; QUERY_DATA.len()]));
    context.get_data(AsynchronousRef::Query(&query), Some(&bytes), 0).expect("query data");
    assert_eq!(*bytes.0.lock().unwrap_or_else(PoisonError::into_inner), QUERY_DATA.to_vec(), "bytes written through the caller's buffer");

    // A method the glue does not handle yet answers E_NOTIMPL, never a crash.
    let buffer_desc = BufferDesc { byte_width: 16, usage: d3d11::enumerations::Usage(0), bind_flags: 1, cpu_access_flags: 0, misc_flags: 0, structure_byte_stride: 0 };
    assert_eq!(device.create_buffer(&buffer_desc, None).err(), Some(E_NOTIMPL));

    drop((query, context, device));
    assert_eq!(LIVE.load(Ordering::SeqCst), 0, "every toy object is released when the last C reference goes");
}
