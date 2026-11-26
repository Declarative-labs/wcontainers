//! Pass-through on Linux: a toy implementation of the slice, served by the serving glue as C entry points, and
//! called back through the calling glue as if it were the system's library. Every call crosses the C ABI twice;
//! the test checks the answers, COM's identity rules (a child's device, the immediate context and a swap chain's
//! buffer are the objects first handed out, not new ones), the exception lines (initial data copied at the length the
//! description gives, memory lent by Map written back at Unmap and revoked after it, a bound object, a foreign
//! `IUnknown`, multithread protection, a refused module handle) and that every object is released at the end.

#![allow(unused_variables, reason = "the toy ignores most of what it is given")]

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::constants::{E_INVALIDARG, E_NOTIMPL};
use wcontainers_component_object_model::structures::{Guid, HResult, Hmodule, Hwnd};
use wcontainers_direct3d_11 as d3d11;
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map, QueryKind, ResourceDimension, Usage};
use wcontainers_direct3d_11::structures::{
    BufferDesc, DepthStencilViewDesc, MappedSubresource, QueryDesc, RenderTargetViewDesc, SubresourceData, Texture2DDesc,
    UnorderedAccessViewDesc,
};
use wcontainers_direct3d_11::{
    Asynchronous as _, AsynchronousRef, Buffer as _, Device as _, DeviceChild as _, DeviceContext as _, Direct3D11 as _,
    Query as _, ResourceRef, Texture2D as _, UnknownRef,
};
use wcontainers_direct3d_11_direct::{SystemDirect3D11, exports};
use wcontainers_direct3d_common as d3dcommon;
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure as dxgi;
use wcontainers_directx_graphics_infrastructure::SwapChain as _;
use wcontainers_directx_graphics_infrastructure::enumerations::{Format, ModeScaling, ModeScanlineOrder, Present, SwapEffect};
use wcontainers_directx_graphics_infrastructure::structures::{ModeDesc, Rational, SampleDesc, SurfaceDesc, SwapChainDesc};
use wcontainers_volatile_bytes::VolatileBytes;

/// Toy objects alive now, across the test (one test per process: this file holds one).
static LIVE: AtomicI64 = AtomicI64::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static BEGUN: AtomicU32 = AtomicU32::new(0);
static PROTECTED: AtomicU32 = AtomicU32::new(0);
static CREATED: AtomicU32 = AtomicU32::new(0);
/// The lengths of the initial bytes the toy was given, per resource made with any.
static INITIAL: Mutex<Vec<Vec<usize>>> = Mutex::new(Vec::new());
/// An event query's data, a BOOL: the size the glue derives from the query's kind (lengths.rs `data_size_of_query`).
const QUERY_DATA: [u8; 4] = [1, 2, 3, 4];
/// DXGI_FORMAT_R8G8B8A8_UNORM: four bytes a texel.
const RGBA: Format = Format(28);

/// One toy object alive: counted while it lives.
struct Live;

impl Default for Live {
    fn default() -> Live {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Live
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a toy object holds: one kind's state, all optional.
#[derive(Default)]
struct Data {
    _live: Live,
    query: Option<QueryDesc>,
    buffer: Option<BufferDesc>,
    texture: Option<Texture2DDesc>,
    /// A resource's bytes, which Map lends.
    bytes: Arc<Mutex<Vec<u8>>>,
    context: OnceLock<Toy>,
    buffers: OnceLock<Toy>,
    private: Mutex<Option<&'static str>>,
}

/// One toy object of any kind; equal only to its clones.
struct Toy(Arc<Data>);

impl Toy {
    fn new(data: Data) -> Toy {
        Toy(Arc::new(data))
    }
}

impl Clone for Toy {
    fn clone(&self) -> Toy {
        Toy(Arc::clone(&self.0))
    }
}

impl PartialEq for Toy {
    fn eq(&self, other: &Toy) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Toy {}

impl Hash for Toy {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// A resource's bytes, lent by Map: the toy's own storage, behind its lock.
struct Shared(Arc<Mutex<Vec<u8>>>);

impl VolatileBytes for Shared {
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

#[derive(Clone, Copy)]
struct ToyApi;

fn texture_desc(width: u32, height: u32, mip_levels: u32) -> Texture2DDesc {
    Texture2DDesc {
        width,
        height,
        mip_levels,
        array_size: 1,
        format: RGBA,
        sample_desc: SampleDesc { count: 1, quality: 0 },
        usage: Usage(0),
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
    }
}

fn new_device() -> (Toy, Toy) {
    let device = Toy::new(Data::default());
    let context = Toy::new(Data::default());
    let _ = device.0.context.set(context.clone());
    (device, context)
}

impl d3d11::Direct3D11 for ToyApi {
    type Buffer = Toy;
    type DepthStencilView = Toy;
    type Device = Toy;
    type DeviceContext = Toy;
    type Multithread = Toy;
    type Query = Toy;
    type RenderTargetView = Toy;
    type Texture2D = Toy;
    type UnorderedAccessView = Toy;
    type Mapped = Shared;

    fn create_device(&self, adapter: Option<&Toy>, driver_type: DriverType, software: Hmodule, flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32) -> Result<(Toy, FeatureLevel, Toy), HResult> {
        let level = *feature_levels.first().ok_or(E_INVALIDARG)?;
        CREATED.fetch_add(1, Ordering::SeqCst);
        let (device, context) = new_device();
        Ok((device, level, context))
    }

    fn create_device_and_swap_chain(&self, adapter: Option<&Toy>, driver_type: DriverType, software: Hmodule, flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32, swap_chain_desc: Option<&SwapChainDesc>) -> Result<(Toy, Toy, FeatureLevel, Toy), HResult> {
        let level = *feature_levels.first().ok_or(E_INVALIDARG)?;
        let desc = swap_chain_desc.ok_or(E_INVALIDARG)?;
        let (device, context) = new_device();
        let swap_chain = Toy::new(Data::default());
        let buffer = Toy::new(Data { texture: Some(texture_desc(desc.buffer_desc.width, desc.buffer_desc.height, 1)), ..Data::default() });
        let _ = swap_chain.0.buffers.set(buffer);
        Ok((swap_chain, device, level, context))
    }
}

impl d3dcommon::Direct3DCommon for ToyApi {
    type Blob = Toy;
    type DestructionNotifier = Toy;
}

impl dxgi::DirectXGraphicsInfrastructure for ToyApi {
    type Adapter = Toy;
    type Surface = Toy;
    type SwapChain = Toy;
    type Buffer = Toy;
}

impl d3dcommon::Blob for Toy {
    fn get_buffer_pointer(&self) -> u8 {
        0
    }
    fn get_buffer_size(&self) -> usize {
        0
    }
}

impl d3dcommon::DestructionNotifier for Toy {
    fn register_destruction_callback(&self, callback_fn: usize, data: &impl VolatileBytes) -> Result<u32, HResult> {
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(&self, callback_id: u32) -> Result<(), HResult> {
        Err(E_NOTIMPL)
    }
}

impl dxgi::Object for Toy {
    type Api = ToyApi;
}
impl dxgi::DeviceSubObject for Toy {}
impl dxgi::Adapter for Toy {}
impl dxgi::Surface for Toy {
    fn get_desc(&self) -> Result<SurfaceDesc, HResult> {
        let desc = self.0.texture.ok_or(E_NOTIMPL)?;
        Ok(SurfaceDesc { width: desc.width, height: desc.height, format: desc.format, sample_desc: desc.sample_desc })
    }
}
impl dxgi::SwapChain for Toy {
    fn present(&self, sync_interval: u32, flags: Present) -> Result<(), HResult> {
        Ok(())
    }
    fn get_buffer(&self, buffer: u32) -> Result<Toy, HResult> {
        // A new handle each time: the glue answers the object it first served while it lives.
        self.0.buffers.get().filter(|_| buffer == 0).map(|held| Toy(Arc::clone(&held.0))).ok_or(E_INVALIDARG)
    }
}

impl d3d11::DeviceChild for Toy {
    type Api = ToyApi;
    fn get_device(&self) -> Option<Toy> {
        // The glue answers from what it recorded; a new device here would break COM's identity.
        Some(Toy::new(Data::default()))
    }
    fn set_private_data_interface(&self, guid: &Guid, data: Option<UnknownRef<'_, ToyApi>>) -> Result<(), HResult> {
        let kind = match data {
            None => "none",
            Some(UnknownRef::Query(_)) => "query",
            Some(UnknownRef::Device(_)) => "device",
            Some(UnknownRef::Unknown(_)) => "foreign",
            Some(_) => "other",
        };
        *self.0.private.lock().unwrap_or_else(PoisonError::into_inner) = Some(kind);
        Ok(())
    }
}
impl d3d11::Resource for Toy {
    fn get_type(&self) -> ResourceDimension {
        ResourceDimension(if self.0.texture.is_some() { 3 } else { 1 })
    }
}
impl d3d11::Buffer for Toy {
    fn get_desc(&self) -> BufferDesc {
        self.0.buffer.unwrap_or(BufferDesc { byte_width: 0, usage: Usage(0), bind_flags: 0, cpu_access_flags: 0, misc_flags: 0, structure_byte_stride: 0 })
    }
}
impl d3d11::Texture2D for Toy {
    fn get_desc(&self) -> Texture2DDesc {
        self.0.texture.unwrap_or(texture_desc(0, 0, 0))
    }
}
impl d3d11::View for Toy {}
impl d3d11::RenderTargetView for Toy {}
impl d3d11::DepthStencilView for Toy {}
impl d3d11::UnorderedAccessView for Toy {}
impl d3d11::Asynchronous for Toy {
    fn get_data_size(&self) -> u32 {
        QUERY_DATA.len() as u32
    }
}
impl d3d11::Query for Toy {
    fn get_desc(&self) -> QueryDesc {
        self.0.query.unwrap_or(QueryDesc { query: QueryKind(-1), misc_flags: 0 })
    }
}
impl d3d11::Multithread for Toy {
    fn set_multithread_protected(&self, protect: bool) -> bool {
        PROTECTED.fetch_add(u32::from(protect), Ordering::SeqCst);
        false
    }
}

impl d3d11::Device for Toy {
    type Api = ToyApi;

    fn create_buffer(&self, desc: &BufferDesc, initial_data: Option<&SubresourceData>) -> Result<Toy, HResult> {
        let mut bytes = vec![0_u8; desc.byte_width as usize];
        if let Some(initial) = initial_data {
            let length = bytes.len();
            bytes.copy_from_slice(&initial.sys_mem[..length]);
        }
        INITIAL.lock().unwrap_or_else(PoisonError::into_inner).push(initial_data.map(|initial| initial.sys_mem.len()).into_iter().collect());
        Ok(Toy::new(Data { buffer: Some(*desc), bytes: Arc::new(Mutex::new(bytes)), ..Data::default() }))
    }
    fn create_texture_2d(&self, desc: &Texture2DDesc, initial_data: &[SubresourceData]) -> Result<Toy, HResult> {
        let bytes = vec![0_u8; (desc.width * desc.height * 4) as usize];
        INITIAL.lock().unwrap_or_else(PoisonError::into_inner).push(initial_data.iter().map(|initial| initial.sys_mem.len()).collect());
        Ok(Toy::new(Data { texture: Some(*desc), bytes: Arc::new(Mutex::new(bytes)), ..Data::default() }))
    }
    fn create_unordered_access_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&UnorderedAccessViewDesc>) -> Result<Toy, HResult> {
        Ok(Toy::new(Data::default()))
    }
    fn create_render_target_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&RenderTargetViewDesc>) -> Result<Toy, HResult> {
        Ok(Toy::new(Data::default()))
    }
    fn create_depth_stencil_view(&self, resource: ResourceRef<'_, ToyApi>, desc: Option<&DepthStencilViewDesc>) -> Result<Toy, HResult> {
        Err(E_NOTIMPL)
    }
    fn create_query(&self, query_desc: &QueryDesc) -> Result<Toy, HResult> {
        Ok(Toy::new(Data { query: Some(*query_desc), ..Data::default() }))
    }
    fn get_immediate_context(&self) -> Option<Toy> {
        self.0.context.get().cloned()
    }
}

impl d3d11::DeviceContext for Toy {
    fn draw(&self, vertex_count: u32, start_vertex_location: u32) {
        DRAWS.fetch_add(vertex_count, Ordering::SeqCst);
    }
    fn map(&self, resource: ResourceRef<'_, ToyApi>, subresource: u32, map_type: Map, map_flags: u32) -> Result<MappedSubresource<Shared>, HResult> {
        // One Rust type for every kind: the glue finds any resource as the first kind of that type, so the toy reads
        // its shape from its own state.
        let (ResourceRef::Buffer(object) | ResourceRef::Texture2D(object)) = resource;
        let (bytes, row_pitch) = (Arc::clone(&object.0.bytes), object.0.texture.map_or(0, |desc| desc.width * 4));
        Ok(MappedSubresource { data: Shared(bytes), row_pitch, depth_pitch: 0 })
    }
    fn unmap(&self, resource: ResourceRef<'_, ToyApi>, subresource: u32) {}
    fn ia_set_vertex_buffers(&self, start_slot: u32, vertex_buffers: &[Option<&Toy>], strides: &[u32], offsets: &[u32]) {}
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
    fn om_set_render_targets_and_unordered_access_views(&self, render_target_views: Option<&[Option<&Toy>]>, depth_stencil_view: Option<&Toy>, uav_start_slot: u32, unordered_access_views: Option<&[Option<&Toy>]>, uav_initial_counts: Option<&[u32]>) {}
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
    assert_eq!(PROTECTED.load(Ordering::SeqCst), 1, "the calling glue turned on multithread protection at creation");
    assert!(system.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &[], 7).is_err(), "the toy's failure crossed back");
    let created = CREATED.load(Ordering::SeqCst);
    assert_eq!(system.create_device(None, DriverType(1), Hmodule(0x1000), CreateDeviceFlag(0), &asked, 7).err(), Some(E_INVALIDARG), "a module handle is refused");
    assert_eq!(CREATED.load(Ordering::SeqCst), created, "a refused module handle never reaches the library");

    // COM's identity: the immediate context and a child's device are the objects created, not new ones.
    assert!(device.get_immediate_context() == Some(context.clone()), "GetImmediateContext answers the context created with the device");
    assert!(context.get_device() == Some(device.clone()), "the context's device is the device it was created with");

    let desc = QueryDesc { query: QueryKind(0), misc_flags: 1 };
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

    // Initial data at the length the description gives, and memory lent by Map, both ways.
    let width = 16;
    let buffer_desc = BufferDesc { byte_width: width, usage: Usage(0), bind_flags: 1, cpu_access_flags: 0, misc_flags: 0, structure_byte_stride: 0 };
    let initial: Vec<u8> = (1..=16).collect();
    let short = SubresourceData { sys_mem: vec![0; 15], sys_mem_pitch: 0, sys_mem_slice_pitch: 0 };
    assert_eq!(device.create_buffer(&buffer_desc, Some(&short)).err(), Some(E_INVALIDARG), "initial bytes shorter than the description are refused");
    let buffer = device.create_buffer(&buffer_desc, Some(&SubresourceData { sys_mem: initial.clone(), sys_mem_pitch: 0, sys_mem_slice_pitch: 0 })).expect("a buffer");
    assert_eq!(buffer.get_desc(), buffer_desc);
    // READ_WRITE: the contents are lent (a discarding Map lends cleared memory, which the documentation leaves undefined).
    let mapped = context.map(ResourceRef::Buffer(&buffer), 0, Map(3), 0).expect("a map");
    assert_eq!(mapped.data.len(), u64::from(width), "the length the description gives");
    let mut read = vec![0_u8; 16];
    assert_eq!(mapped.data.read(&mut read), 16);
    assert_eq!(read, initial, "the initial bytes were copied, and Map lent them back");
    assert!(context.map(ResourceRef::Buffer(&buffer), 0, Map(4), 0).is_err(), "a subresource mapped is not mapped again");
    assert_eq!(mapped.data.write(&[9; 16]), 16);
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    assert_eq!(mapped.data.len(), 0, "Unmap revoked the view before the system's Unmap");
    let again = context.map(ResourceRef::Buffer(&buffer), 0, Map(1), 0).expect("a second map");
    let mut read = vec![0_u8; 16];
    let _ = again.data.read(&mut read);
    assert_eq!(read, vec![9; 16], "what the caller wrote reached the implementation at Unmap");
    assert_eq!(again.data.write(&[5; 16]), 16);
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    let read_again = context.map(ResourceRef::Buffer(&buffer), 0, Map(1), 0).expect("a read map again");
    let mut read = vec![0_u8; 16];
    let _ = read_again.data.read(&mut read);
    assert_eq!(read, vec![9; 16], "what was written to memory lent for reading only did not go back");
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    let discarded = context.map(ResourceRef::Buffer(&buffer), 0, Map(4), 0).expect("a discarding map");
    let mut read = vec![1_u8; 16];
    let _ = discarded.data.read(&mut read);
    assert_eq!(read, vec![0; 16], "a discarding Map lends cleared memory, never an earlier lending's bytes");
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    let last = context.map(ResourceRef::Buffer(&buffer), 0, Map(1), 0).expect("a third map");
    let mut read = vec![0_u8; 16];
    let _ = last.data.read(&mut read);
    assert_eq!(read, vec![0; 16], "the discarding Map's cleared bytes went back at its Unmap");
    context.unmap(ResourceRef::Buffer(&buffer), 0);

    // A texture of two mips: one initial record per subresource, each of its own length.
    let texture_desc = texture_desc(4, 4, 2);
    let mips = [
        SubresourceData { sys_mem: vec![7; 64], sys_mem_pitch: 16, sys_mem_slice_pitch: 0 },
        SubresourceData { sys_mem: vec![8; 16], sys_mem_pitch: 8, sys_mem_slice_pitch: 0 },
    ];
    assert_eq!(device.create_texture_2d(&texture_desc, &mips[..1]).err(), Some(E_INVALIDARG), "one record per subresource, or none");
    let texture = device.create_texture_2d(&texture_desc, &mips).expect("a texture");
    assert_eq!(texture.get_desc(), texture_desc);
    let empty = device.create_texture_2d(&texture_desc, &[]).expect("a texture without initial data");
    let given = INITIAL.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(given, vec![vec![16], vec![64, 16], vec![]], "the serving glue copied each subresource at the length its description and pitch give");
    let view = device.create_render_target_view(ResourceRef::Texture2D(&texture), None).expect("a view");
    context.om_set_render_targets_and_unordered_access_views(Some(&[Some(&view)]), None, 0, None, None);
    context.ia_set_vertex_buffers(0, &[Some(&buffer)], &[4], &[0]);
    let texture_mapped = context.map(ResourceRef::Texture2D(&texture), 0, Map(4), 0).expect("a texture map");
    assert_eq!(texture_mapped.row_pitch, 16);
    assert_eq!(texture_mapped.data.len(), 64, "four rows at the pitch");
    context.unmap(ResourceRef::Texture2D(&texture), 0);

    // An IUnknown in: one of ours is found as its kind.
    let private = Guid { data1: 1, data2: 2, data3: 3, data4: [4; 8] };
    query.set_private_data_interface(&private, Some(UnknownRef::Device(&device))).expect("private data");
    query.set_private_data_interface(&private, None).expect("no private data");

    // A bound object: the swap chain's buffer is a Direct3D texture, the same one each time.
    let mode = ModeDesc { width: 8, height: 2, refresh_rate: Rational { numerator: 60, denominator: 1 }, format: RGBA, scanline_ordering: ModeScanlineOrder(0), scaling: ModeScaling(0) };
    let swap_desc = SwapChainDesc { buffer_desc: mode, sample_desc: SampleDesc { count: 1, quality: 0 }, buffer_usage: dxgi::enumerations::Usage(0x20), buffer_count: 1, output_window: Hwnd(0), windowed: true, swap_effect: SwapEffect(0), flags: 0 };
    let (swap_chain, swap_device, _, swap_context) = system.create_device_and_swap_chain(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &asked, 7, Some(&swap_desc)).expect("a swap chain");
    let back = swap_chain.get_buffer(0).expect("a back buffer");
    assert_eq!(d3d11::Texture2D::get_desc(&back).width, 8, "the buffer is the toy's texture, through both glues");
    assert!(swap_chain.get_buffer(0).expect("the back buffer again") == back, "the same object while it lives");
    assert!(swap_chain.get_buffer(1).is_err());
    swap_chain.present(1, Present(0)).expect("a present");

    drop((back, swap_chain, swap_device, swap_context));
    // A view of lent memory holds the context and the resource until it goes.
    drop((mapped, again, read_again, discarded, last, texture_mapped));
    drop((view, empty, texture, buffer, query, context, device));
    assert_eq!(LIVE.load(Ordering::SeqCst), 0, "every toy object is released when the last C reference goes");
}
