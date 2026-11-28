//! A well-behaved implementation of the slice in safe Rust, for the harness: the spike's front, ported to the
//! generated traits, keeping its resources in memory instead of on a `Gpu`. One Rust type per object kind, so that
//! the serving glue tells the kinds apart; every check the documentation states for the slice is its own, since the
//! generated glue passes ranges on to the implementation.

#![allow(clippy::cast_possible_truncation, reason = "a toy's sizes, all checked small")]

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::constants::{E_INVALIDARG, E_NOTIMPL};
use wcontainers_component_object_model::structures::{Foreign, Guid, HResult, Hmodule};
use wcontainers_direct3d_11 as d3d11;
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map, QueryKind, ResourceDimension, Usage};
use wcontainers_direct3d_11::structures::{
    BufferDesc, DepthStencilViewDesc, MappedSubresource, QueryDesc, RenderTargetViewDesc, SubresourceData, Texture2DDesc,
    UnorderedAccessViewDesc,
};
use wcontainers_direct3d_11::{AsynchronousRef, ResourceRef, UnknownRef};
use wcontainers_direct3d_common as d3dcommon;
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure as dxgi;
use wcontainers_directx_graphics_infrastructure::enumerations::{Format, Present};
use wcontainers_directx_graphics_infrastructure::structures::{SampleDesc, SurfaceDesc, SwapChainDesc};
use wcontainers_volatile_bytes::VolatileBytes;

const SDK_VERSION: u32 = 7;
const LEVEL_9_1: i32 = 0x9100;
const LEVEL_11_0: i32 = 0xb000;
const LEVEL_11_1: i32 = 0xb100;
const USAGE_IMMUTABLE: i32 = 1;
const USAGE_DYNAMIC: i32 = 2;
const USAGE_STAGING: i32 = 3;
const CPU_ACCESS_WRITE: u32 = 0x10000;
const CPU_ACCESS_READ: u32 = 0x20000;
const MAP_READ: i32 = 1;
const MAP_WRITE: i32 = 2;
const MAP_READ_WRITE: i32 = 3;
const MAP_WRITE_DISCARD: i32 = 4;
const MAP_WRITE_NO_OVERWRITE: i32 = 5;
const QUERY_EVENT: i32 = 0;
const QUERY_PIPELINE_STATISTICS: i32 = 4;
const DIMENSION_BUFFER: i32 = 1;
const DIMENSION_TEXTURE2D: i32 = 3;
/// d3d11.h: a texture's side, slices, and the largest resource.
const MOST_SIDE: u32 = 16384;
const MOST_SLICES: u32 = 2048;
const MOST_BYTES: u64 = 2048 << 20;
/// The rows the front lends from Map are this far apart at least, wider than a row so that a pitch is tested.
const ROW_ALIGNMENT: usize = 64;
/// D3D11_QUERY_DATA_PIPELINE_STATISTICS: eleven counts.
const STATISTICS: usize = 11;
const EVENT_DATA_SIZE: u32 = 4;
const BACK_BUFFERS: u32 = 16;
const DXGI_ERROR_INVALID_CALL: HResult = HResult(0x887A_0001_u32 as i32);
const VERTEX_SLOTS: usize = 32;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The front's library.
#[derive(Clone, Copy, Default)]
pub struct Front;

/// What a device and its immediate context share.
pub struct DeviceState {
    vertices: AtomicU64,
    bound: Mutex<Vec<Option<FrontBuffer>>>,
    mapped: Mutex<Vec<(usize, u32, Arc<Mutex<Vec<u8>>>, Target)>>,
    private: Private,
}

/// Where the bytes of a mapping go back at Unmap.
#[derive(Clone)]
enum Target {
    Buffer(Arc<Mutex<Vec<u8>>>),
    Texture(Arc<Mutex<Vec<Vec<u8>>>>, usize, Rows),
}

#[derive(Clone, Copy)]
struct Rows {
    count: usize,
    bytes: usize,
    pitch: usize,
}

/// Private data an object keeps, by key: our objects and others, held while kept.
#[derive(Default)]
struct Private(Mutex<Vec<(Guid, Kept)>>);

enum Kept {
    Foreign(#[expect(dead_code, reason = "held only to keep the object")] Foreign),
    Ours(#[expect(dead_code, reason = "held only to keep the object")] Box<dyn Send + Sync>),
}

impl Private {
    fn replace(&self, guid: &Guid, data: Option<UnknownRef<'_, Front>>) {
        let kept = data.map(|data| match data {
            UnknownRef::Unknown(foreign) => Kept::Foreign(foreign.clone()),
            UnknownRef::Buffer(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::Texture2D(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::Device(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::DeviceContext(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::Query(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::RenderTargetView(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::DepthStencilView(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::UnorderedAccessView(object) => Kept::Ours(Box::new(object.clone())),
            UnknownRef::Multithread(object) => Kept::Ours(Box::new(object.clone())),
        });
        let replaced = {
            let mut store = lock(&self.0);
            let replaced = store.iter().position(|(key, _)| key == guid).map(|at| store.swap_remove(at));
            if let Some(kept) = kept {
                store.push((*guid, kept));
            }
            replaced
        };
        drop(replaced);
    }
}

macro_rules! object {
    ($($name:ident($state:ty)),* $(,)?) => {$(
        pub struct $name(Arc<$state>);

        impl Clone for $name {
            fn clone(&self) -> $name {
                $name(Arc::clone(&self.0))
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                Arc::ptr_eq(&self.0, &other.0)
            }
        }

        impl Eq for $name {}

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                Arc::as_ptr(&self.0).hash(state);
            }
        }
    )*};
}

pub struct BufferState {
    device: FrontDevice,
    desc: BufferDesc,
    bytes: Arc<Mutex<Vec<u8>>>,
    private: Private,
}

pub struct TextureState {
    device: FrontDevice,
    desc: Texture2DDesc,
    subresources: Arc<Mutex<Vec<Vec<u8>>>>,
    private: Private,
}

pub struct ViewState {
    device: FrontDevice,
}

pub struct QueryState {
    device: FrontDevice,
    desc: QueryDesc,
    begun_at: Mutex<Option<u64>>,
    result: Mutex<Option<u64>>,
}

pub struct SwapChainState {
    buffers: Vec<FrontTexture>,
}

/// A part the front has no use for (blobs, adapters, notifiers): every call answers E_NOTIMPL.
pub struct Nothing;

object!(
    FrontDevice(DeviceState),
    FrontContext(DeviceState),
    FrontBuffer(BufferState),
    FrontTexture(TextureState),
    FrontRenderTarget(ViewState),
    FrontDepthStencil(ViewState),
    FrontUnorderedAccess(ViewState),
    FrontQuery(QueryState),
    FrontSwapChain(SwapChainState),
    FrontNothing(Nothing),
);

/// The front's mapped bytes: memory of its own, which it reads back at Unmap.
pub struct FrontMapped(Arc<Mutex<Vec<u8>>>);

impl VolatileBytes for FrontMapped {
    fn len(&self) -> u64 {
        lock(&self.0).len() as u64
    }
    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let held = lock(&self.0);
        let from = usize::try_from(offset).ok().and_then(|offset| held.get(offset..)).unwrap_or_default();
        let count = into.len().min(from.len());
        into[..count].copy_from_slice(&from[..count]);
        count
    }
    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let mut held = lock(&self.0);
        let to = usize::try_from(offset).ok().and_then(|offset| held.get_mut(offset..)).unwrap_or_default();
        let count = bytes.len().min(to.len());
        to[..count].copy_from_slice(&bytes[..count]);
        count
    }
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = lock(&self.0).clone();
        sink.write(&copy)
    }
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![0_u8; lock(&self.0).len()];
        let count = source.read(&mut copy)?.min(copy.len());
        Ok(self.write(&copy[..count]))
    }
}

/// Bytes per block and the block's side, for the formats the front knows.
fn block(format: Format) -> Option<(usize, u32)> {
    match format.0 {
        2 => Some((16, 1)),
        28 | 45 | 87 => Some((4, 1)),
        71 => Some((8, 4)),
        _ => None,
    }
}

fn mip_levels(desc: &Texture2DDesc) -> u32 {
    if desc.mip_levels != 0 { desc.mip_levels } else { u32::BITS - desc.width.max(desc.height).max(1).leading_zeros() }
}

fn rows(desc: &Texture2DDesc, subresource: u32, alignment: usize) -> Option<Rows> {
    let mips = mip_levels(desc);
    if mips == 0 || mips >= u32::BITS || subresource / mips >= desc.array_size {
        return None;
    }
    let mip = subresource % mips;
    let (width, height) = ((desc.width >> mip).max(1), (desc.height >> mip).max(1));
    let (bytes, side) = block(desc.format)?;
    let row = width.div_ceil(side) as usize * bytes;
    let pitch = if alignment == 0 { row } else { row.next_multiple_of(alignment) };
    Some(Rows { count: height.div_ceil(side) as usize, bytes: row, pitch })
}

fn checked_texture(desc: &Texture2DDesc) -> Option<usize> {
    let side = 1..=MOST_SIDE;
    let fits = side.contains(&desc.width) && side.contains(&desc.height) && (1..=MOST_SLICES).contains(&desc.array_size);
    let mips = mip_levels(desc);
    let full = u32::BITS - desc.width.max(desc.height).leading_zeros();
    if !fits || mips > full || block(desc.format).is_none() || !(1..=32).contains(&desc.sample_desc.count) {
        return None;
    }
    let count = mips.checked_mul(desc.array_size)?;
    let mut total = 0_u64;
    for subresource in 0..count {
        let rows = rows(desc, subresource, 0)?;
        total = total.checked_add((rows.count * rows.bytes) as u64)?;
    }
    (total <= MOST_BYTES).then_some(count as usize)
}

fn mappable(usage: Usage, access: u32, map: Map) -> bool {
    let reads = map.0 == MAP_READ || map.0 == MAP_READ_WRITE;
    let writes = map.0 != MAP_READ;
    match usage.0 {
        USAGE_DYNAMIC => map.0 == MAP_WRITE_DISCARD || map.0 == MAP_WRITE_NO_OVERWRITE,
        USAGE_STAGING => {
            matches!(map.0, MAP_READ | MAP_WRITE | MAP_READ_WRITE)
                && (!reads || access & CPU_ACCESS_READ != 0)
                && (!writes || access & CPU_ACCESS_WRITE != 0)
        }
        _ => false,
    }
}

impl FrontDevice {
    fn new() -> FrontDevice {
        FrontDevice(Arc::new(DeviceState {
            vertices: AtomicU64::new(0),
            bound: Mutex::new(vec![None; VERTEX_SLOTS]),
            mapped: Mutex::default(),
            private: Private::default(),
        }))
    }

    fn context(&self) -> FrontContext {
        FrontContext(Arc::clone(&self.0))
    }

    fn owns(&self, resource: &ResourceRef<'_, Front>) -> bool {
        match resource {
            ResourceRef::Buffer(buffer) => buffer.0.device == *self,
            ResourceRef::Texture2D(texture) => texture.0.device == *self,
        }
    }

    fn view(&self, resource: ResourceRef<'_, Front>) -> Result<Arc<ViewState>, HResult> {
        if self.owns(&resource) { Ok(Arc::new(ViewState { device: self.clone() })) } else { Err(E_INVALIDARG) }
    }
}

fn create(feature_levels: &[FeatureLevel], sdk_version: u32) -> Result<(FrontDevice, FeatureLevel, FrontContext), HResult> {
    if sdk_version != SDK_VERSION {
        return Err(E_INVALIDARG);
    }
    let level = match feature_levels {
        [] => FeatureLevel(LEVEL_11_0),
        levels => *levels.iter().find(|level| (LEVEL_9_1..=LEVEL_11_1).contains(&level.0)).ok_or(E_INVALIDARG)?,
    };
    let device = FrontDevice::new();
    let context = device.context();
    Ok((device, level, context))
}

impl d3d11::Direct3D11 for Front {
    type Buffer = FrontBuffer;
    type DepthStencilView = FrontDepthStencil;
    type Device = FrontDevice;
    type DeviceContext = FrontContext;
    type Multithread = FrontContext;
    type Query = FrontQuery;
    type RenderTargetView = FrontRenderTarget;
    type Texture2D = FrontTexture;
    type UnorderedAccessView = FrontUnorderedAccess;
    type Mapped = FrontMapped;

    fn create_device(&self, _adapter: Option<&FrontNothing>, _driver_type: DriverType, _software: Hmodule, _flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32) -> Result<(FrontDevice, FeatureLevel, FrontContext), HResult> {
        create(feature_levels, sdk_version)
    }

    fn create_device_and_swap_chain(&self, _adapter: Option<&FrontNothing>, _driver_type: DriverType, _software: Hmodule, _flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], sdk_version: u32, swap_chain_desc: Option<&SwapChainDesc>) -> Result<(FrontSwapChain, FrontDevice, FeatureLevel, FrontContext), HResult> {
        let desc = swap_chain_desc.ok_or(E_INVALIDARG)?;
        let (device, level, context) = create(feature_levels, sdk_version)?;
        let mode = desc.buffer_desc;
        if !(1..=BACK_BUFFERS).contains(&desc.buffer_count) || mode.width == 0 || mode.height == 0 {
            return Err(E_INVALIDARG);
        }
        let texture = Texture2DDesc {
            width: mode.width,
            height: mode.height,
            mip_levels: 1,
            array_size: 1,
            format: mode.format,
            sample_desc: desc.sample_desc,
            usage: Usage(0),
            bind_flags: 0x28,
            cpu_access_flags: 0,
            misc_flags: 0,
        };
        let buffers = (0..desc.buffer_count).map(|_| d3d11::Device::create_texture_2d(&device, &texture, &[])).collect::<Result<_, _>>()?;
        Ok((FrontSwapChain(Arc::new(SwapChainState { buffers })), device, level, context))
    }
}

impl d3dcommon::Direct3DCommon for Front {
    type Blob = FrontNothing;
    type DestructionNotifier = FrontNothing;
}

impl dxgi::DirectXGraphicsInfrastructure for Front {
    type Adapter = FrontNothing;
    type Surface = FrontTexture;
    type SwapChain = FrontSwapChain;
    type Buffer = FrontTexture;
}

impl d3dcommon::Blob for FrontNothing {
    fn get_buffer_pointer(&self) -> u8 {
        0
    }
    fn get_buffer_size(&self) -> usize {
        0
    }
}

impl d3dcommon::DestructionNotifier for FrontNothing {
    fn register_destruction_callback(&self, _callback_fn: usize, _data: &impl VolatileBytes) -> Result<u32, HResult> {
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(&self, _callback_id: u32) -> Result<(), HResult> {
        Err(E_NOTIMPL)
    }
}

impl dxgi::Object for FrontNothing {
    type Api = Front;
}
impl dxgi::DeviceSubObject for FrontNothing {}
impl dxgi::Adapter for FrontNothing {}

impl dxgi::Object for FrontTexture {
    type Api = Front;
}
impl dxgi::DeviceSubObject for FrontTexture {}
impl dxgi::Surface for FrontTexture {
    fn get_desc(&self) -> Result<SurfaceDesc, HResult> {
        let desc = &self.0.desc;
        Ok(SurfaceDesc { width: desc.width, height: desc.height, format: desc.format, sample_desc: desc.sample_desc })
    }
}

impl dxgi::Object for FrontSwapChain {
    type Api = Front;
}
impl dxgi::DeviceSubObject for FrontSwapChain {}
impl dxgi::SwapChain for FrontSwapChain {
    fn present(&self, _sync_interval: u32, _flags: Present) -> Result<(), HResult> {
        Ok(())
    }
    fn get_buffer(&self, buffer: u32) -> Result<FrontTexture, HResult> {
        self.0.buffers.get(buffer as usize).cloned().ok_or(DXGI_ERROR_INVALID_CALL)
    }
}

macro_rules! child {
    ($($name:ident => $device:expr, $private:expr;)*) => {$(
        impl d3d11::DeviceChild for $name {
            type Api = Front;

            fn get_device(&self) -> Option<FrontDevice> {
                let device: fn(&$name) -> FrontDevice = $device;
                Some(device(self))
            }

            fn set_private_data_interface(&self, guid: &Guid, data: Option<UnknownRef<'_, Front>>) -> Result<(), HResult> {
                let private: fn(&$name) -> Option<&Private> = $private;
                private(self).ok_or(E_INVALIDARG)?.replace(guid, data);
                Ok(())
            }
        }
    )*};
}

child! {
    FrontContext => |context| FrontDevice(Arc::clone(&context.0)), |context| Some(&context.0.private);
    FrontBuffer => |buffer| buffer.0.device.clone(), |buffer| Some(&buffer.0.private);
    FrontTexture => |texture| texture.0.device.clone(), |texture| Some(&texture.0.private);
    FrontRenderTarget => |view| view.0.device.clone(), |_| None;
    FrontDepthStencil => |view| view.0.device.clone(), |_| None;
    FrontUnorderedAccess => |view| view.0.device.clone(), |_| None;
    FrontQuery => |query| query.0.device.clone(), |_| None;
}

impl d3d11::Resource for FrontBuffer {
    fn get_type(&self) -> ResourceDimension {
        ResourceDimension(DIMENSION_BUFFER)
    }
}
impl d3d11::Buffer for FrontBuffer {
    fn get_desc(&self) -> BufferDesc {
        self.0.desc
    }
}
impl d3d11::Resource for FrontTexture {
    fn get_type(&self) -> ResourceDimension {
        ResourceDimension(DIMENSION_TEXTURE2D)
    }
}
impl d3d11::Texture2D for FrontTexture {
    fn get_desc(&self) -> Texture2DDesc {
        self.0.desc
    }
}
impl d3d11::View for FrontRenderTarget {}
impl d3d11::RenderTargetView for FrontRenderTarget {}
impl d3d11::View for FrontDepthStencil {}
impl d3d11::DepthStencilView for FrontDepthStencil {}
impl d3d11::View for FrontUnorderedAccess {}
impl d3d11::UnorderedAccessView for FrontUnorderedAccess {}
impl d3d11::Asynchronous for FrontQuery {
    fn get_data_size(&self) -> u32 {
        if self.0.desc.query.0 == QUERY_EVENT { EVENT_DATA_SIZE } else { (STATISTICS * size_of::<u64>()) as u32 }
    }
}
impl d3d11::Query for FrontQuery {
    fn get_desc(&self) -> QueryDesc {
        self.0.desc
    }
}
impl d3d11::Multithread for FrontContext {
    fn set_multithread_protected(&self, _protect: bool) -> bool {
        true
    }
}

impl d3d11::Device for FrontDevice {
    type Api = Front;

    fn create_buffer(&self, desc: &BufferDesc, initial_data: Option<&SubresourceData>) -> Result<FrontBuffer, HResult> {
        if desc.byte_width == 0 || u64::from(desc.byte_width) > MOST_BYTES || (desc.usage.0 == USAGE_IMMUTABLE && initial_data.is_none()) {
            return Err(E_INVALIDARG);
        }
        let mut bytes = vec![0; desc.byte_width as usize];
        if let Some(initial) = initial_data {
            let length = bytes.len().min(initial.sys_mem.len());
            bytes[..length].copy_from_slice(&initial.sys_mem[..length]);
        }
        Ok(FrontBuffer(Arc::new(BufferState { device: self.clone(), desc: *desc, bytes: Arc::new(Mutex::new(bytes)), private: Private::default() })))
    }

    fn create_texture_2d(&self, desc: &Texture2DDesc, initial_data: &[SubresourceData]) -> Result<FrontTexture, HResult> {
        let count = checked_texture(desc).ok_or(E_INVALIDARG)?;
        if (desc.usage.0 == USAGE_IMMUTABLE && initial_data.is_empty()) || !(initial_data.is_empty() || initial_data.len() == count) {
            return Err(E_INVALIDARG);
        }
        let desc = Texture2DDesc { mip_levels: mip_levels(desc), ..*desc };
        let mut subresources = Vec::with_capacity(count);
        for subresource in 0..count {
            let rows = rows(&desc, subresource as u32, 0).ok_or(E_INVALIDARG)?;
            let mut packed = vec![0; rows.count * rows.bytes];
            if let Some(data) = initial_data.get(subresource) {
                let pitch = data.sys_mem_pitch as usize;
                for (at, row) in packed.chunks_mut(rows.bytes.max(1)).enumerate() {
                    let from = at * pitch;
                    let length = row.len().min(data.sys_mem.len().saturating_sub(from));
                    row[..length].copy_from_slice(&data.sys_mem[from..from + length]);
                }
            }
            subresources.push(packed);
        }
        Ok(FrontTexture(Arc::new(TextureState { device: self.clone(), desc, subresources: Arc::new(Mutex::new(subresources)), private: Private::default() })))
    }

    fn create_unordered_access_view(&self, resource: ResourceRef<'_, Front>, _desc: Option<&UnorderedAccessViewDesc>) -> Result<FrontUnorderedAccess, HResult> {
        Ok(FrontUnorderedAccess(self.view(resource)?))
    }

    fn create_render_target_view(&self, resource: ResourceRef<'_, Front>, _desc: Option<&RenderTargetViewDesc>) -> Result<FrontRenderTarget, HResult> {
        Ok(FrontRenderTarget(self.view(resource)?))
    }

    fn create_depth_stencil_view(&self, resource: ResourceRef<'_, Front>, _desc: Option<&DepthStencilViewDesc>) -> Result<FrontDepthStencil, HResult> {
        Ok(FrontDepthStencil(self.view(resource)?))
    }

    fn create_query(&self, desc: &QueryDesc) -> Result<FrontQuery, HResult> {
        if desc.query.0 != QUERY_EVENT && desc.query.0 != QUERY_PIPELINE_STATISTICS {
            return Err(E_INVALIDARG);
        }
        Ok(FrontQuery(Arc::new(QueryState { device: self.clone(), desc: *desc, begun_at: Mutex::default(), result: Mutex::default() })))
    }

    fn get_immediate_context(&self) -> Option<FrontContext> {
        Some(self.context())
    }
}

fn query_bytes(event: bool, vertices: u64) -> Vec<u8> {
    if event {
        return 1_u32.to_ne_bytes().to_vec();
    }
    let mut counts = [0_u64; STATISTICS];
    counts[0] = vertices;
    counts[1] = vertices / 3;
    counts[2] = vertices;
    counts.iter().flat_map(|count| count.to_ne_bytes()).collect()
}

impl d3d11::DeviceContext for FrontContext {
    fn draw(&self, vertex_count: u32, _start_vertex_location: u32) {
        self.0.vertices.fetch_add(u64::from(vertex_count), Ordering::Relaxed);
    }

    fn map(&self, resource: ResourceRef<'_, Front>, subresource: u32, map_type: Map, _map_flags: u32) -> Result<MappedSubresource<FrontMapped>, HResult> {
        let device = FrontDevice(Arc::clone(&self.0));
        let (address, usage, access) = match &resource {
            ResourceRef::Buffer(buffer) => (Arc::as_ptr(&buffer.0).addr(), buffer.0.desc.usage, buffer.0.desc.cpu_access_flags),
            ResourceRef::Texture2D(texture) => (Arc::as_ptr(&texture.0).addr(), texture.0.desc.usage, texture.0.desc.cpu_access_flags),
        };
        if !mappable(usage, access, map_type) || !device.owns(&resource) {
            return Err(E_INVALIDARG);
        }
        let mut mapped = lock(&self.0.mapped);
        if mapped.iter().any(|(other, at, _, _)| *other == address && *at == subresource) {
            return Err(E_INVALIDARG);
        }
        let (bytes, target, row_pitch, depth_pitch) = match resource {
            ResourceRef::Buffer(buffer) => {
                if subresource != 0 {
                    return Err(E_INVALIDARG);
                }
                let contents = if map_type.0 == MAP_WRITE_DISCARD { vec![0; buffer.0.desc.byte_width as usize] } else { lock(&buffer.0.bytes).clone() };
                let width = buffer.0.desc.byte_width;
                (contents, Target::Buffer(Arc::clone(&buffer.0.bytes)), width, width)
            }
            ResourceRef::Texture2D(texture) => {
                let rows = rows(&texture.0.desc, subresource, ROW_ALIGNMENT).ok_or(E_INVALIDARG)?;
                let packed = lock(&texture.0.subresources).get(subresource as usize).cloned().ok_or(E_INVALIDARG)?;
                let mut spread = vec![0; rows.pitch * rows.count];
                for (row, at) in packed.chunks(rows.bytes.max(1)).zip((0..).step_by(rows.pitch.max(1))) {
                    spread[at..at + row.len()].copy_from_slice(row);
                }
                let pitch = u32::try_from(rows.pitch).map_err(|_| E_INVALIDARG)?;
                (spread, Target::Texture(Arc::clone(&texture.0.subresources), subresource as usize, rows), pitch, pitch * rows.count as u32)
            }
        };
        let lent = Arc::new(Mutex::new(bytes));
        mapped.push((address, subresource, Arc::clone(&lent), target));
        Ok(MappedSubresource { data: FrontMapped(lent), row_pitch, depth_pitch })
    }

    fn unmap(&self, resource: ResourceRef<'_, Front>, subresource: u32) {
        let address = match resource {
            ResourceRef::Buffer(buffer) => Arc::as_ptr(&buffer.0).addr(),
            ResourceRef::Texture2D(texture) => Arc::as_ptr(&texture.0).addr(),
        };
        let removed = {
            let mut mapped = lock(&self.0.mapped);
            mapped.iter().position(|(other, at, _, _)| *other == address && *at == subresource).map(|at| mapped.swap_remove(at))
        };
        let Some((_, _, lent, target)) = removed else { return };
        let bytes = lock(&lent).clone();
        match target {
            Target::Buffer(storage) => {
                let mut storage = lock(&storage);
                let length = storage.len().min(bytes.len());
                storage[..length].copy_from_slice(&bytes[..length]);
            }
            Target::Texture(subresources, at, rows) => {
                let mut subresources = lock(&subresources);
                let Some(packed) = subresources.get_mut(at) else { return };
                for (row, from) in packed.chunks_mut(rows.bytes.max(1)).zip((0..).step_by(rows.pitch.max(1))) {
                    let length = row.len();
                    if let Some(source) = bytes.get(from..from + length) {
                        row.copy_from_slice(source);
                    }
                }
            }
        }
    }

    fn ia_set_vertex_buffers(&self, start_slot: u32, vertex_buffers: &[Option<&FrontBuffer>], _strides: &[u32], _offsets: &[u32]) {
        let device = FrontDevice(Arc::clone(&self.0));
        let replaced: Vec<Option<FrontBuffer>> = {
            let mut bound = lock(&self.0.bound);
            vertex_buffers
                .iter()
                .enumerate()
                .filter_map(|(at, buffer)| {
                    let slot = bound.get_mut(start_slot as usize + at)?;
                    Some(core::mem::replace(slot, buffer.filter(|buffer| buffer.0.device == device).cloned()))
                })
                .collect()
        };
        drop(replaced);
    }

    fn begin(&self, asynchronous: AsynchronousRef<'_, Front>) {
        let AsynchronousRef::Query(query) = asynchronous;
        if query.0.desc.query.0 == QUERY_PIPELINE_STATISTICS {
            *lock(&query.0.begun_at) = Some(self.0.vertices.load(Ordering::Relaxed));
            *lock(&query.0.result) = None;
        }
    }

    fn end(&self, asynchronous: AsynchronousRef<'_, Front>) {
        let AsynchronousRef::Query(query) = asynchronous;
        let vertices = self.0.vertices.load(Ordering::Relaxed);
        let result = match query.0.desc.query.0 {
            QUERY_PIPELINE_STATISTICS => lock(&query.0.begun_at).map(|begun_at| vertices.saturating_sub(begun_at)),
            _ => Some(1),
        };
        *lock(&query.0.result) = result;
    }

    fn get_data(&self, asynchronous: AsynchronousRef<'_, Front>, data: Option<&impl VolatileBytes>, _get_data_flags: u32) -> Result<bool, HResult> {
        let AsynchronousRef::Query(query) = asynchronous;
        if data.is_some_and(|data| data.len() != u64::from(d3d11::Asynchronous::get_data_size(query))) {
            return Err(E_INVALIDARG);
        }
        let Some(result) = *lock(&query.0.result) else { return Ok(false) };
        if let Some(data) = data {
            data.write(&query_bytes(query.0.desc.query.0 == QUERY_EVENT, result));
        }
        Ok(true)
    }

    fn om_set_render_targets_and_unordered_access_views(&self, _render_target_views: Option<&[Option<&FrontRenderTarget>]>, _depth_stencil_view: Option<&FrontDepthStencil>, _uav_start_slot: u32, _unordered_access_views: Option<&[Option<&FrontUnorderedAccess>]>, _uav_initial_counts: Option<&[u32]>) {}
}

/// An unused name of the front's: the sample count a swap chain asks for.
pub const SAMPLE: SampleDesc = SampleDesc { count: 1, quality: 0 };

/// Every query kind the front makes.
pub const QUERIES: [QueryKind; 2] = [QueryKind(QUERY_EVENT), QueryKind(QUERY_PIPELINE_STATISTICS)];
