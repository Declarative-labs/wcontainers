//! A lying implementation of the slice in safe Rust, for the harness: every answer, length, description, object,
//! comparison, hash and drop is random; it panics at random, answers "failures" that read as success, hands out
//! objects of other devices and of the wrong kind of its own (its three view kinds are one Rust type), and lends mapped
//! memory of any length whose `read` and `write` report counts they did not move. A correct C game meets it: the glue
//! must keep its promises whatever the liar does.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::structures::{Guid, HResult, Hmodule};
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

/// Lies told so far.
pub static MISCHIEF: AtomicU64 = AtomicU64::new(0);
static SEED: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
/// The message of the liar's panics, which the fuzzing run does not print.
pub const PANIC: &str = "the liar panics";

/// Seeds every thread's random answers.
pub fn seed(seed: u64) {
    SEED.store(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1, Ordering::Relaxed);
}

thread_local! {
    static STATE: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

fn random() -> u64 {
    STATE.with(|state| {
        let mut x = state.get();
        if x == 0 {
            x = SEED.fetch_add(0x632b_e59b_d9b4_e019, Ordering::Relaxed) | 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        x
    })
}

fn below(n: u64) -> u64 {
    if n == 0 { 0 } else { random() % n }
}

/// True once in `n`, a lie counted.
fn lie(n: u64) -> bool {
    let lies = below(n) == 0;
    if lies {
        MISCHIEF.fetch_add(1, Ordering::Relaxed);
    }
    lies
}

/// Panics once in `n`.
fn maybe_panic(n: u64) {
    if lie(n) {
        std::panic::panic_any(PANIC);
    }
}

/// A random status: a failure, or a "failure" that reads as success.
fn status() -> HResult {
    match below(4) {
        0 => HResult(0),
        1 => HResult(1),
        2 => HResult(-2_147_024_809),
        _ => HResult(random() as i32),
    }
}

fn result<T>(value: impl FnOnce() -> T) -> Result<T, HResult> {
    maybe_panic(64);
    if lie(8) { Err(status()) } else { Ok(value()) }
}

/// Objects the liar handed out, any of which it may hand out again, as any kind.
static POOL: Mutex<Vec<Arc<u64>>> = Mutex::new(Vec::new());

/// The liar's library.
#[derive(Clone, Copy, Default)]
pub struct Liar;

/// One object of the liar's, of kind `K`; its identity is shared with any object of the pool.
pub struct Lie<const K: u8>(Arc<u64>);

impl<const K: u8> Lie<K> {
    fn new() -> Lie<K> {
        if lie(4) {
            let pool = POOL.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(shared) = pool.get(below(pool.len() as u64) as usize) {
                return Lie(Arc::clone(shared));
            }
        }
        let made = Arc::new(random());
        let mut pool = POOL.lock().unwrap_or_else(PoisonError::into_inner);
        if pool.len() < 4096 {
            pool.push(Arc::clone(&made));
        } else {
            let at = below(pool.len() as u64) as usize;
            pool[at] = Arc::clone(&made);
        }
        Lie(made)
    }
}

impl<const K: u8> Clone for Lie<K> {
    fn clone(&self) -> Lie<K> {
        maybe_panic(256);
        Lie(Arc::clone(&self.0))
    }
}

impl<const K: u8> PartialEq for Lie<K> {
    fn eq(&self, other: &Lie<K>) -> bool {
        maybe_panic(256);
        if lie(4) { random().is_multiple_of(2) } else { Arc::ptr_eq(&self.0, &other.0) }
    }
}

impl<const K: u8> Eq for Lie<K> {}

impl<const K: u8> Hash for Lie<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        maybe_panic(256);
        random().hash(state);
    }
}

impl<const K: u8> Drop for Lie<K> {
    fn drop(&mut self) {
        if lie(512) && !std::thread::panicking() {
            std::panic::panic_any(PANIC);
        }
    }
}

const DEVICE: u8 = 0;
const CONTEXT: u8 = 1;
const BUFFER: u8 = 2;
const TEXTURE: u8 = 3;
const VIEW: u8 = 4;
const QUERY: u8 = 5;
const SWAP_CHAIN: u8 = 6;
const OTHER: u8 = 7;

/// Mapped memory of any length, whose counts lie.
pub struct LieBytes(Mutex<Vec<u8>>);

impl VolatileBytes for LieBytes {
    fn len(&self) -> u64 {
        if lie(8) { random() } else { self.0.lock().unwrap_or_else(PoisonError::into_inner).len() as u64 }
    }
    fn read(&self, into: &mut [u8]) -> usize {
        maybe_panic(128);
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        if lie(8) { random() as usize } else { count }
    }
    fn write(&self, bytes: &[u8]) -> usize {
        maybe_panic(128);
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        if lie(8) { random() as usize } else { count }
    }
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        sink.write(&copy)
    }
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![0_u8; self.0.lock().unwrap_or_else(PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?.min(copy.len());
        Ok(self.write(&copy[..count]))
    }
}

fn mapped() -> LieBytes {
    let length = match below(4) {
        0 => 0,
        1 => below(64),
        2 => below(1 << 16),
        _ => below(1 << 20),
    };
    LieBytes(Mutex::new(vec![random() as u8; length as usize]))
}

fn buffer_desc() -> BufferDesc {
    BufferDesc { byte_width: random() as u32, usage: Usage(random() as i32), bind_flags: random() as u32, cpu_access_flags: random() as u32, misc_flags: random() as u32, structure_byte_stride: random() as u32 }
}

fn texture_desc() -> Texture2DDesc {
    Texture2DDesc {
        width: random() as u32,
        height: random() as u32,
        mip_levels: random() as u32,
        array_size: random() as u32,
        format: Format(random() as i32),
        sample_desc: SampleDesc { count: random() as u32, quality: random() as u32 },
        usage: Usage(random() as i32),
        bind_flags: random() as u32,
        cpu_access_flags: random() as u32,
        misc_flags: random() as u32,
    }
}

fn level(levels: &[FeatureLevel]) -> FeatureLevel {
    if lie(4) { FeatureLevel(random() as i32) } else { levels.first().copied().unwrap_or(FeatureLevel(0xb000)) }
}

impl d3d11::Direct3D11 for Liar {
    type Buffer = Lie<BUFFER>;
    type DepthStencilView = Lie<VIEW>;
    type Device = Lie<DEVICE>;
    type DeviceContext = Lie<CONTEXT>;
    type Multithread = Lie<OTHER>;
    type Query = Lie<QUERY>;
    type RenderTargetView = Lie<VIEW>;
    type Texture2D = Lie<TEXTURE>;
    type UnorderedAccessView = Lie<VIEW>;
    type Mapped = LieBytes;

    fn create_device(&self, _adapter: Option<&Lie<OTHER>>, _driver_type: DriverType, _software: Hmodule, _flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], _sdk_version: u32) -> Result<(Lie<DEVICE>, FeatureLevel, Lie<CONTEXT>), HResult> {
        result(|| (Lie::new(), level(feature_levels), Lie::new()))
    }

    fn create_device_and_swap_chain(&self, _adapter: Option<&Lie<OTHER>>, _driver_type: DriverType, _software: Hmodule, _flags: CreateDeviceFlag, feature_levels: &[FeatureLevel], _sdk_version: u32, _swap_chain_desc: Option<&SwapChainDesc>) -> Result<(Lie<SWAP_CHAIN>, Lie<DEVICE>, FeatureLevel, Lie<CONTEXT>), HResult> {
        result(|| (Lie::new(), Lie::new(), level(feature_levels), Lie::new()))
    }
}

impl d3dcommon::Direct3DCommon for Liar {
    type Blob = Lie<OTHER>;
    type DestructionNotifier = Lie<OTHER>;
}

impl dxgi::DirectXGraphicsInfrastructure for Liar {
    type Adapter = Lie<OTHER>;
    type Surface = Lie<TEXTURE>;
    type SwapChain = Lie<SWAP_CHAIN>;
    type Buffer = Lie<TEXTURE>;
}

impl<const K: u8> d3dcommon::Blob for Lie<K> {
    fn get_buffer_pointer(&self) -> u8 {
        random() as u8
    }
    fn get_buffer_size(&self) -> usize {
        random() as usize
    }
}

impl<const K: u8> d3dcommon::DestructionNotifier for Lie<K> {
    fn register_destruction_callback(&self, _callback_fn: usize, _data: &impl VolatileBytes) -> Result<u32, HResult> {
        result(|| random() as u32)
    }
    fn unregister_destruction_callback(&self, _callback_id: u32) -> Result<(), HResult> {
        result(|| ())
    }
}

impl<const K: u8> dxgi::Object for Lie<K> {
    type Api = Liar;
}
impl<const K: u8> dxgi::DeviceSubObject for Lie<K> {}
impl<const K: u8> dxgi::Adapter for Lie<K> {}
impl<const K: u8> dxgi::Surface for Lie<K> {
    fn get_desc(&self) -> Result<SurfaceDesc, HResult> {
        result(|| SurfaceDesc { width: random() as u32, height: random() as u32, format: Format(random() as i32), sample_desc: SampleDesc { count: random() as u32, quality: 0 } })
    }
}
impl<const K: u8> dxgi::SwapChain for Lie<K> {
    fn present(&self, _sync_interval: u32, _flags: Present) -> Result<(), HResult> {
        result(|| ())
    }
    fn get_buffer(&self, _buffer: u32) -> Result<Lie<TEXTURE>, HResult> {
        result(Lie::new)
    }
}

impl<const K: u8> d3d11::DeviceChild for Lie<K> {
    type Api = Liar;
    fn get_device(&self) -> Option<Lie<DEVICE>> {
        maybe_panic(64);
        (!lie(8)).then(Lie::new)
    }
    fn set_private_data_interface(&self, _guid: &Guid, _data: Option<UnknownRef<'_, Liar>>) -> Result<(), HResult> {
        result(|| ())
    }
}
impl<const K: u8> d3d11::Resource for Lie<K> {
    fn get_type(&self) -> ResourceDimension {
        maybe_panic(64);
        ResourceDimension(random() as i32)
    }
}
impl<const K: u8> d3d11::Buffer for Lie<K> {
    fn get_desc(&self) -> BufferDesc {
        maybe_panic(64);
        buffer_desc()
    }
}
impl<const K: u8> d3d11::Texture2D for Lie<K> {
    fn get_desc(&self) -> Texture2DDesc {
        maybe_panic(64);
        texture_desc()
    }
}
impl<const K: u8> d3d11::View for Lie<K> {}
impl<const K: u8> d3d11::RenderTargetView for Lie<K> {}
impl<const K: u8> d3d11::DepthStencilView for Lie<K> {}
impl<const K: u8> d3d11::UnorderedAccessView for Lie<K> {}
impl<const K: u8> d3d11::Asynchronous for Lie<K> {
    fn get_data_size(&self) -> u32 {
        maybe_panic(64);
        random() as u32
    }
}
impl<const K: u8> d3d11::Query for Lie<K> {
    fn get_desc(&self) -> QueryDesc {
        maybe_panic(64);
        QueryDesc { query: QueryKind(random() as i32), misc_flags: random() as u32 }
    }
}
impl<const K: u8> d3d11::Multithread for Lie<K> {
    fn set_multithread_protected(&self, _protect: bool) -> bool {
        random().is_multiple_of(2)
    }
}

impl<const K: u8> d3d11::Device for Lie<K> {
    type Api = Liar;

    fn create_buffer(&self, _desc: &BufferDesc, _initial_data: Option<&SubresourceData>) -> Result<Lie<BUFFER>, HResult> {
        result(Lie::new)
    }
    fn create_texture_2d(&self, _desc: &Texture2DDesc, _initial_data: &[SubresourceData]) -> Result<Lie<TEXTURE>, HResult> {
        result(Lie::new)
    }
    fn create_unordered_access_view(&self, _resource: ResourceRef<'_, Liar>, _desc: Option<&UnorderedAccessViewDesc>) -> Result<Lie<VIEW>, HResult> {
        result(Lie::new)
    }
    fn create_render_target_view(&self, _resource: ResourceRef<'_, Liar>, _desc: Option<&RenderTargetViewDesc>) -> Result<Lie<VIEW>, HResult> {
        result(Lie::new)
    }
    fn create_depth_stencil_view(&self, _resource: ResourceRef<'_, Liar>, _desc: Option<&DepthStencilViewDesc>) -> Result<Lie<VIEW>, HResult> {
        result(Lie::new)
    }
    fn create_query(&self, _query_desc: &QueryDesc) -> Result<Lie<QUERY>, HResult> {
        result(Lie::new)
    }
    fn get_immediate_context(&self) -> Option<Lie<CONTEXT>> {
        maybe_panic(64);
        (!lie(8)).then(Lie::new)
    }
}

impl<const K: u8> d3d11::DeviceContext for Lie<K> {
    fn draw(&self, _vertex_count: u32, _start_vertex_location: u32) {
        maybe_panic(64);
    }
    fn map(&self, _resource: ResourceRef<'_, Liar>, _subresource: u32, _map_type: Map, _map_flags: u32) -> Result<MappedSubresource<LieBytes>, HResult> {
        result(|| MappedSubresource { data: mapped(), row_pitch: if lie(4) { random() as u32 } else { below(4096) as u32 }, depth_pitch: if lie(4) { random() as u32 } else { below(1 << 16) as u32 } })
    }
    fn unmap(&self, _resource: ResourceRef<'_, Liar>, _subresource: u32) {
        maybe_panic(64);
    }
    fn ia_set_vertex_buffers(&self, _start_slot: u32, _vertex_buffers: &[Option<&Lie<BUFFER>>], _strides: &[u32], _offsets: &[u32]) {
        maybe_panic(64);
    }
    fn begin(&self, _async: AsynchronousRef<'_, Liar>) {
        maybe_panic(64);
    }
    fn end(&self, _async: AsynchronousRef<'_, Liar>) {
        maybe_panic(64);
    }
    fn get_data(&self, _async: AsynchronousRef<'_, Liar>, data: Option<&impl VolatileBytes>, _get_data_flags: u32) -> Result<bool, HResult> {
        if let Some(data) = data
            && lie(2)
        {
            let length = if lie(4) { below(1 << 16) } else { data.len().min(1 << 16) };
            let _ = data.write(&vec![random() as u8; length as usize]);
        }
        result(|| lie(2))
    }
    fn om_set_render_targets_and_unordered_access_views(&self, _render_target_views: Option<&[Option<&Lie<VIEW>>]>, _depth_stencil_view: Option<&Lie<VIEW>>, _uav_start_slot: u32, _unordered_access_views: Option<&[Option<&Lie<VIEW>>]>, _uav_initial_counts: Option<&[u32]>) {
        maybe_panic(64);
    }
}
