//! Bytes in and out: writes in their three modes, renaming, copies, clears and read backs (plan
//! section 4.6).

use crate::blocks::{copy_blocks, copy_buffer_blocks, read_blocks, write_blocks};
use crate::draw::split_pass;
use crate::edge::copy_bytes;
use crate::fill::{aspect_by_draw, depth_by_draw, stencil_by_draw};
use crate::packed::{copy_buffer_packed, copy_packed, write_packed};
use crate::{HeldReadback, buffer_of, forget_texture, query_set_of, readback_of, texture_of};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use wcontainers_byte_sink::ByteSink;
use wcontainers_volatile_bytes::VolatileBytes;

use wcontainers_gpu::{
    Gpu, error::GpuError, format::FORMAT_FACTS, format::Format, format::FormatKind,
    resource::CopyBufferTexture, resource::Extent, resource::Origin, resource::QueryKind,
    resource::ReadSource, resource::TextureDesc, resource::TextureDimension,
    resource::TextureRegion, resource::TextureUsage, resource::WriteMode, slow::SlowPath,
};

use crate::fill::{AspectRead, DepthSource};
use crate::{
    Buffer, BufferObj, MAP_PENDING, MAP_READY, QuerySet, Readback, ReadbackObj, Stream, Texture,
    TextureObj, WebGpu, convert, packed, row_bytes,
};

/// Where and how much buffer copies, queue writes and clears may move.
const ALIGN: u64 = wgpu::COPY_BUFFER_ALIGNMENT;

/// The bytes of one query result.
const QUERY_BYTES: u64 = wgpu::QUERY_SIZE as u64;

/// Direct3D 12's alignment of a texture's rows placed in a buffer
/// (`D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT`).
pub(crate) const DX12_PLACEMENT_ALIGNMENT: u64 = 512;

/// The most bytes of pattern a non-zero clear uploads.
const PATTERN_BLOCK: u64 = 64 * 1024;

/// GPU storage a renamed buffer or texture gives up.
#[derive(Clone)]
pub(crate) enum Spare {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
}

/// What storage must share to stand in for another.
///
/// A buffer's key finds only buffers, and a texture's only textures.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SpareKey {
    /// Size and usage.
    Buffer(u64, u32),
    /// Size, levels, samples, dimension, format and usage; the view formats follow the format.
    Texture(
        wgpu::Extent3d,
        u32,
        u32,
        wgpu::TextureDimension,
        wgpu::TextureFormat,
        u32,
    ),
}

/// Returns what storage must share to stand in for this buffer.
fn buffer_key(b: &wgpu::Buffer) -> SpareKey {
    SpareKey::Buffer(b.size(), b.usage().bits())
}

/// Returns what storage must share to stand in for this texture.
fn texture_key(t: &wgpu::Texture) -> SpareKey {
    SpareKey::Texture(
        t.size(),
        t.mip_level_count(),
        t.sample_count(),
        t.dimension(),
        t.format(),
        t.usage().bits(),
    )
}

impl Spare {
    fn key(&self) -> SpareKey {
        match self {
            Spare::Buffer(b) => buffer_key(b),
            Spare::Texture(t) => texture_key(t),
        }
    }

    fn destroy(&self) {
        match self {
            Spare::Buffer(b) => b.destroy(),
            Spare::Texture(t) => t.destroy(),
        }
    }
}

/// Storage that renamed buffers or textures gave up, reused once the GPU has finished with it.
/// Each buffer or texture counts the storage its renames made; destroying it destroys as much of
/// the pool's storage of its key, so the pool holds no more than its live objects made.
#[derive(Default)]
pub(crate) struct Pool {
    /// Storage still used by the work of the serial it was retired in.
    retired: Vec<(u64, Spare)>,
    /// Storage free to reuse, by key.
    free: HashMap<SpareKey, Vec<Spare>>,
    /// Storage destroyed objects made and the pool still holds, by key: destroyed as it becomes
    /// free.
    owed: HashMap<SpareKey, u64>,
}

impl Pool {
    /// Frees storage whose work has completed, destroying what is owed; answers how much it
    /// destroyed.
    pub(crate) fn reclaim(&mut self, completed: u64) -> u64 {
        let (free, owed) = (&mut self.free, &mut self.owed);
        let mut dropped = 0;
        self.retired.retain(|(serial, b)| {
            if *serial > completed {
                return true;
            }
            match owed.get_mut(&b.key()) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    b.destroy();
                    dropped += 1;
                }
                _ => free.entry(b.key()).or_default().push(b.clone()),
            }
            false
        });
        dropped
    }

    fn take(&mut self, key: SpareKey) -> Option<Spare> {
        self.free.get_mut(&key).and_then(Vec::pop)
    }

    fn retire(&mut self, serial: u64, old: Spare) {
        self.retired.push((serial, old));
    }

    /// Owes back `spares` of this key, destroying free ones at once; answers how many it
    /// destroyed.
    fn give_back(&mut self, key: SpareKey, spares: u64) -> u64 {
        if spares == 0 {
            return 0;
        }
        let free = self.free.entry(key).or_default();
        let now = (spares as usize).min(free.len());
        for b in free.drain(free.len() - now..) {
            b.destroy();
        }
        *self.owed.entry(key).or_default() += spares - now as u64;
        now as u64
    }
}

/// Bytes of the first uniform arena; each one that fills is replaced by one twice as large.
const FIRST_ARENA: u64 = 1 << 20;

/// Slices of the size asked that an arena made for it holds at least.
const ARENA_SLICES_AT_LEAST: u64 = 4;

/// The uniform arena (plan sections 4.5 and 4.6): one large GPU buffer holding every uniform-only
/// buffer written often, each in a slice of its own. A `Discard` of such a buffer while recorded work
/// uses it moves it to a free slice, so bind groups keep naming the arena and draws give the slice's
/// offset as a dynamic offset.
pub(crate) struct Arena {
    /// The storage number buffers living in it carry.
    pub(crate) id: u64,
    pub(crate) raw: wgpu::Buffer,
    /// The end of the slices handed out so far.
    top: u64,
    /// Free slices by size.
    free: HashMap<u64, Vec<u64>>,
    /// Slices given up in the encoder being recorded (size, offset), free once it is submitted: queue
    /// writes land after work already submitted.
    retired: Vec<(u64, u64)>,
}

impl Arena {
    fn new(device: &wgpu::Device, id: u64, size: u64) -> Self {
        Arena {
            id,
            raw: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: arena_usage(),
                mapped_at_creation: false,
            }),
            top: 0,
            free: HashMap::new(),
            retired: Vec::new(),
        }
    }

    fn take(&mut self, size: u64) -> Option<u64> {
        if let Some(at) = self.free.get_mut(&size).and_then(Vec::pop) {
            return Some(at);
        }
        (self.top + size <= self.raw.size()).then(|| {
            self.top += size;
            self.top - size
        })
    }

    /// Frees the slices given up before a submit.
    pub(crate) fn submitted(&mut self) {
        for (size, at) in self.retired.drain(..) {
            self.free.entry(size).or_default().push(at);
        }
    }
}

/// The usages of the arena's buffer, which are also those of every buffer placed in it.
pub(crate) fn arena_usage() -> wgpu::BufferUsages {
    wgpu::BufferUsages::UNIFORM
        | wgpu::BufferUsages::COPY_DST
        | wgpu::BufferUsages::COPY_SRC
        | wgpu::BufferUsages::STORAGE
}

/// Whether WebGPU cannot write this format's depth from a buffer, so writes and buffer copies
/// into it are drawn (`DepthWrittenByDraw`).
fn filled_by_draw(format: Format) -> bool {
    matches!(
        format,
        Format::Depth24Plus
            | Format::Depth24PlusStencil8
            | Format::Depth32Float
            | Format::Depth32FloatStencil8
    )
}

/// The format a region's texels are in when copied to or from a buffer: the stencil aspect is
/// 8-bit, a combined format's depth aspect that depth format.
fn region_format<B>(texture: &crate::TextureObj, region: &TextureRegion<'_, B>) -> Format
where
    B: wcontainers_gpu::Gpu,
{
    match (region.stencil, texture.held) {
        (true, _) => Format::Stencil8,
        (false, Format::Depth24PlusStencil8) => Format::Depth24Plus,
        (false, Format::Depth32FloatStencil8) => Format::Depth32Float,
        (false, held) => held,
    }
}

fn texel_copy<'a>(
    raw: &'a wgpu::Texture,
    texture: &crate::TextureObj,
    mip: u32,
    layer: u32,
    origin: wcontainers_gpu::resource::Origin,
    stencil: bool,
) -> wgpu::TexelCopyTextureInfo<'a> {
    wgpu::TexelCopyTextureInfo {
        texture: raw,
        mip_level: mip,
        origin: wgpu::Origin3d {
            x: origin.x,
            y: origin.y,
            z: if texture.desc.dimension == TextureDimension::D3 {
                origin.z
            } else {
                layer
            },
        },
        aspect: convert::aspect(stencil, texture.desc.format),
    }
}

/// The top bit of a pair's spare slot: the pair replaces the result instead of adding to it (a
/// query not begun in its own slot).
const REPLACE: u32 = 1 << (u32::BITS - 1);

/// Adds 64-bit counts of spare query slots into resolved results, pair by pair, or replaces a
/// result where the slot has `REPLACE`.
fn query_sum_wgsl() -> String {
    format!(
        "
@group(0) @binding(0) var<storage, read_write> sums: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read> spare: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> pairs: array<vec2<u32>>;
@compute @workgroup_size(1) fn add() {{
    for (var k = 0u; k < arrayLength(&pairs); k++) {{
        let p = pairs[k];
        let a = select(sums[p.x], vec2(0u), p.y >= {REPLACE}u);
        let b = spare[p.y & {}u];
        let low = a.x + b.x;
        sums[p.x] = vec2(low, a.y + b.y + select(0u, 1u, low < a.x));
    }}
}}
",
        !REPLACE
    )
}

/// The staging ring's first size; it is made twice as large when a submit's staged bytes do not
/// fit.
const RING_SIZE: u64 = 1 << 20;

/// Regions of the staging ring start at multiples of this, the largest offset alignment a copy or
/// storage binding of them needs.
const RING_ALIGN: u64 = 256;

/// The staging ring: one buffer staged bytes are queued into and copied from in command order,
/// taken again from its start after each submit (queued writes land after submitted work).
#[derive(Debug)]
pub(crate) struct Ring {
    buffer: wgpu::Buffer,
    used: u64,
}

impl Ring {
    /// Starts again from the beginning, the submit having taken every region.
    pub(crate) fn submitted(&mut self) {
        self.used = 0;
    }

    /// A region of `size` bytes of the ring in `slot` and where it starts, making a ring twice
    /// as large (or as large as the region) with `usage` when it has no room, counted in `made`.
    fn region(
        slot: &mut Option<Ring>,
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
        made: &mut u64,
    ) -> (wgpu::Buffer, u64) {
        let ring = match slot.take() {
            Some(r) if r.used + size <= r.buffer.size() => r,
            old => {
                let least = old.map_or(RING_SIZE, |r| r.buffer.size() * 2);
                *made += 1;
                Ring {
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: None,
                        size: least.max(size.next_multiple_of(RING_ALIGN)),
                        usage,
                        mapped_at_creation: false,
                    }),
                    used: 0,
                }
            }
        };
        let base = ring.used;
        let buffer = ring.buffer.clone();
        *slot = Some(Ring {
            used: (base + size).next_multiple_of(RING_ALIGN),
            ..ring
        });
        (buffer, base)
    }
}

/// A page's part of resolved query results: (first result, byte offset in the query ring's
/// buffer, results).
type ResolvedPart = (u32, u64, u32);

/// The staging ring's usages: copied from, bound as read-only storage, and resolved spare query
/// slots go into it.
const STAGING_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::COPY_SRC
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::STORAGE)
    .union(wgpu::BufferUsages::QUERY_RESOLVE);

/// The query ring's usages: resolved into, summed in place, copied from.
const QUERY_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::QUERY_RESOLVE
    .union(wgpu::BufferUsages::COPY_SRC)
    .union(wgpu::BufferUsages::STORAGE);

/// Spare slots resolved per page start at multiples of this, as WebGPU aligns resolves to 256
/// bytes.
const PAGE_ALIGN: u32 = 32;

/// WebGPU's size of a copy of `e` texels of `format`: whole blocks, as a level smaller than a block
/// still holds one.
fn extent3d(e: Extent, format: Format) -> wgpu::Extent3d {
    let (bw, bh, _) = FORMAT_FACTS[format as usize].block;
    wgpu::Extent3d {
        width: e.width.next_multiple_of(bw),
        height: e.height.next_multiple_of(bh),
        depth_or_array_layers: e.depth_or_layers,
    }
}

/// A copy source holding `data` from byte `at` of a region of the staging ring, padded to a
/// multiple of four bytes, and where the region starts; also readable by the edge program.
/// The bytes are queued, so they land before the commands recorded until the next submit; the
/// browser stages queued bytes in a ring of its own, natively wgpu copies them once more.
fn staging_at(gpu: &mut WebGpu, at: u64, data: &[u8]) -> (wgpu::Buffer, u64) {
    let size = (at + data.len() as u64).next_multiple_of(ALIGN).max(ALIGN);
    let (buffer, base) = staging_region(gpu, size);
    let mut padded = vec![0; size as usize];
    padded[at as usize..at as usize + data.len()].copy_from_slice(data);
    gpu.queue.write_buffer(&buffer, base, &padded);
    (buffer, base)
}

/// A region of the staging ring no bytes are queued into, for commands to write.
fn staging_region(gpu: &mut WebGpu, size: u64) -> (wgpu::Buffer, u64) {
    Ring::region(
        &mut gpu.ring,
        &gpu.device,
        size,
        STAGING_USAGE,
        &mut gpu.counters.staging_rings,
    )
}

fn staging(gpu: &mut WebGpu, data: &[u8]) -> (wgpu::Buffer, u64) {
    staging_at(gpu, 0, data)
}

/// Whether the encoder being recorded names this buffer, so a write from the queue would land
/// before commands recorded ahead of it.
fn pending(gpu: &WebGpu, used_in: u64) -> bool {
    used_in == gpu.serial && gpu.encoder.is_some()
}

/// Gives a buffer fresh storage; the old storage is kept for the work already recorded with it.
fn rename(gpu: &mut WebGpu, b: &BufferObj) {
    let old = b.raw.borrow().clone();
    reclaim_spares(gpu);
    let fresh = match gpu.pool.take(buffer_key(&old)) {
        Some(Spare::Buffer(spare)) => {
            gpu.counters.spares_reused += 1;
            spare
        }
        _ => {
            b.spares.set(b.spares.get() + 1);
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: old.size(),
                usage: b.usage,
                mapped_at_creation: false,
            })
        }
    };
    b.raw.replace(fresh);
    gpu.pool.retire(gpu.serial, Spare::Buffer(old));
    let storage = gpu.id();
    moved(gpu, b, Some(storage), 0);
    gpu.counters.renames += 1;
}

/// Gives a texture fresh storage for a `Discard` write, as `rename` does for buffers; views and
/// bind groups naming the old storage are made again at their next use.
fn rename_texture(gpu: &mut WebGpu, t: &TextureObj) {
    let old = t.raw();
    reclaim_spares(gpu);
    let fresh = match gpu.texture_pool.take(texture_key(&old)) {
        Some(Spare::Texture(spare)) => {
            gpu.counters.spares_reused += 1;
            spare
        }
        _ => {
            t.spares.set(t.spares.get() + 1);
            let twin = convert::srgb_twin(old.format());
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: old.size(),
                mip_level_count: old.mip_level_count(),
                sample_count: old.sample_count(),
                dimension: old.dimension(),
                format: old.format(),
                usage: old.usage(),
                view_formats: twin.as_slice(),
            })
        }
    };
    t.raw.replace(fresh);
    t.generation.set(t.generation.get() + 1);
    gpu.texture_pool.retire(gpu.serial, Spare::Texture(old));
    gpu.bind_epoch += 1;
    gpu.counters.texture_renames += 1;
}

/// Frees renamed storage the GPU has finished with.
pub(crate) fn reclaim_spares(gpu: &mut WebGpu) {
    let completed = gpu.completed.load(Ordering::Acquire);
    gpu.counters.spares_dropped +=
        gpu.pool.reclaim(completed) + gpu.texture_pool.reclaim(completed);
}

/// Destroys as much pooled storage as a destroyed buffer's renames made.
pub(crate) fn give_back_spares(gpu: &mut WebGpu, b: &BufferObj) {
    let key = buffer_key(&b.raw.borrow());
    gpu.counters.spares_dropped += gpu.pool.give_back(key, b.spares.get());
}

/// Destroys as much pooled storage as a destroyed texture's renames made.
pub(crate) fn give_back_texture_spares(gpu: &mut WebGpu, t: &TextureObj) {
    let key = texture_key(&t.raw());
    gpu.counters.spares_dropped += gpu.texture_pool.give_back(key, t.spares.get());
}

/// Records that a buffer's bytes now start at `base` of new storage or of the same.
fn moved(gpu: &mut WebGpu, b: &BufferObj, storage: Option<u64>, base: u64) {
    if let Some(s) = storage {
        b.storage.set(s);
    }
    b.base.set(base);
    b.generation.set(b.generation.get() + 1);
    b.lead_in.set(0);
    if b.in_sets.get() && (storage.is_some() || b.static_bound.get()) {
        gpu.bind_epoch += 1;
    }
}

/// A free slice of the uniform arena for a buffer of `size` bytes: the arena's buffer, the
/// slice's offset and the arena's storage number. A full arena is replaced by a larger one;
/// buffers still in the old one keep it alive until they move.
pub(crate) fn arena_slice(gpu: &mut WebGpu, size: u64) -> (wgpu::Buffer, u64, u64) {
    let slice = slice_size(gpu, size);
    if let Some(a) = &mut gpu.arena
        && let Some(at) = a.take(slice)
    {
        return (a.raw.clone(), at, a.id);
    }
    let grown = gpu
        .arena
        .as_ref()
        .map_or(FIRST_ARENA, |a| a.raw.size() * 2)
        .max(slice * ARENA_SLICES_AT_LEAST)
        .min(gpu.caps.limits.max_buffer_size)
        .max(slice);
    let id = gpu.id();
    let mut a = Arena::new(&gpu.device, id, grown);
    let at = a
        .take(slice)
        .expect("a new arena holds the slice it was made for");
    let raw = a.raw.clone();
    gpu.arena = Some(a);
    (raw, at, id)
}

fn slice_size(gpu: &WebGpu, size: u64) -> u64 {
    let align = u64::from(gpu.caps.limits.min_uniform_offset_alignment);
    size.next_multiple_of(align).max(align)
}

/// Gives up an arena buffer's slice; it is reused after the next submit.
pub(crate) fn release_slice(gpu: &mut WebGpu, b: &BufferObj) {
    let slice = slice_size(gpu, b.desc.size);
    if let Some(a) = &mut gpu.arena
        && a.id == b.storage.get()
    {
        a.retired.push((slice, b.base.get()));
    }
}

/// Moves an arena buffer to a free slice, leaving the old one to the work recorded with it.
fn move_in_arena(gpu: &mut WebGpu, b: &BufferObj) {
    release_slice(gpu, b);
    let (raw, base, storage) = arena_slice(gpu, b.desc.size);
    let new_storage = (storage != b.storage.get()).then_some(storage);
    if new_storage.is_some() {
        b.raw.replace(raw);
    }
    moved(gpu, b, new_storage, base);
    gpu.counters.slices += 1;
}

pub(crate) fn write_buffer_impl(
    gpu: &mut WebGpu,
    buffer: &Buffer,
    offset: u64,
    data: &[u8],
    mode: WriteMode,
) {
    let Some(b) = buffer_of(gpu, buffer) else {
        return gpu.fail(GpuError::GONE);
    };
    if data.is_empty() {
        return;
    }
    b.wrote();
    if offset + data.len() as u64 > b.desc.size {
        gpu.fail("write_buffer past the buffer's end");
        return;
    }
    let recorded = pending(gpu, b.used_in.get());
    match mode {
        WriteMode::Ordered if recorded => {
            staged_write(gpu, &b, offset, data);
            return;
        }
        WriteMode::Discard if recorded && b.in_arena => move_in_arena(gpu, &b),
        WriteMode::Discard if recorded => rename(gpu, &b),
        _ => {}
    }
    write_now(gpu, &b, offset, data);
}

/// As [`write_buffer_impl`], for bytes another party holds: a write the queue takes now goes
/// from those bytes into the queue's staging memory in one copy; any other is copied out first.
pub(crate) fn write_buffer_from_impl(
    gpu: &mut WebGpu,
    buffer: &Buffer,
    offset: u64,
    data: &impl VolatileBytes,
    mode: WriteMode,
) {
    let Some(b) = buffer_of(gpu, buffer) else {
        return gpu.fail(GpuError::GONE);
    };
    let len = data.len();
    let at = b.base.get() + offset;
    let ordered = matches!(mode, WriteMode::Ordered | WriteMode::Discard);
    let now = len > 0
        && offset
            .checked_add(len)
            .is_some_and(|end| end <= b.desc.size)
        && !(ordered && pending(gpu, b.used_in.get()))
        && at.is_multiple_of(ALIGN)
        && len.is_multiple_of(ALIGN)
        && !gpu.lead_pending(&b);
    if now {
        b.wrote();
        let raw = b.raw.borrow();
        let mut sink = QueueWrite {
            queue: &gpu.queue,
            buffer: &raw,
            at,
        };
        let Ok(_) = data.read_to(&mut sink);
        return;
    }
    let mut bytes = vec![0; usize::try_from(len).unwrap_or(usize::MAX)];
    let read = data.read(&mut bytes);
    bytes.truncate(read);
    write_buffer_impl(gpu, buffer, offset, &bytes, mode);
}

/// Writes ahead of every command recorded so far: from the queue, or, for bytes not in whole words
/// and for any write behind such a one, by a copy in the lead encoder, since queue writes land
/// before it.
fn write_now(gpu: &mut WebGpu, b: &BufferObj, offset: u64, data: &[u8]) {
    let at = b.base.get() + offset;
    let words = at.is_multiple_of(ALIGN) && (data.len() as u64).is_multiple_of(ALIGN);
    if words && !gpu.lead_pending(b) {
        gpu.queue.write_buffer(&b.raw.borrow(), at, data);
        return;
    }
    let (staging, base) = staging_at(gpu, at % ALIGN, data);
    gpu.counters.staged_writes += 1;
    b.lead_in.set(gpu.serial);
    let dst = b.raw.borrow().clone();
    copy_bytes(
        gpu,
        Stream::Lead,
        &staging,
        base + at % ALIGN,
        &dst,
        at,
        data.len() as u64,
    );
}

/// Writes through a staging buffer and a copy recorded in command order, splitting an open pass.
fn staged_write(gpu: &mut WebGpu, b: &BufferObj, offset: u64, data: &[u8]) {
    let at = b.base.get() + offset;
    // The staged bytes sit in the same place of their word as their destination.
    let (staging, base) = staging_at(gpu, at % ALIGN, data);
    gpu.counters.staged_writes += 1;
    b.used_in.set(gpu.serial);
    let dst = b.raw.borrow().clone();
    let size = data.len() as u64;
    split_pass(gpu, SlowPath::WriteSplitPass, |w| {
        copy_bytes(w, Stream::Main, &staging, base + at % ALIGN, &dst, at, size)
    });
}

pub(crate) fn write_texture_impl(
    gpu: &mut WebGpu,
    region: &TextureRegion<'_, WebGpu>,
    extent: Extent,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    mode: WriteMode,
) {
    let Some(t) = texture_of(gpu, region.texture) else {
        return gpu.fail(GpuError::GONE);
    };
    if t.blocks.is_some() {
        write_blocks(gpu, region, extent, data, row_pitch, rows_per_layer, mode);
    } else if t.held != t.desc.format {
        write_packed(gpu, region, extent, data, row_pitch, rows_per_layer, mode);
    } else {
        write_texels(gpu, region, extent, data, row_pitch, rows_per_layer, mode);
    }
}

/// Writes texels in the format the texture holds.
pub(crate) fn write_texels(
    gpu: &mut WebGpu,
    region: &TextureRegion<'_, WebGpu>,
    extent: Extent,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    mode: WriteMode,
) {
    let Some(t) = texture_of(gpu, region.texture) else {
        return gpu.fail(GpuError::GONE);
    };
    if !region.stencil && filled_by_draw(t.desc.format) {
        let Some((staged, sf, how)) = depth_staging(gpu, t.desc.format, extent) else {
            gpu.fail("the texture a depth write is drawn from could not be made");
            return;
        };
        let at = TextureRegion {
            texture: &staged,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        // A new texture: written straight from the queue.
        write_texels(gpu, &at, extent, data, row_pitch, rows_per_layer, mode);
        let Some(staged) = forget_texture(gpu, &staged) else {
            return gpu.fail(GpuError::GONE);
        };
        let (tex, mip, layer, origin) = (
            region.texture.clone(),
            region.mip,
            region.layer,
            region.origin,
        );
        let draw = move |w: &mut WebGpu, stream| {
            let dst = TextureRegion {
                texture: &tex,
                mip,
                layer,
                origin,
                stencil: false,
            };
            depth_copy(
                w,
                (&staged.raw(), sf, how),
                (0, 0, [0, 0]),
                &dst,
                extent,
                stream,
            );
        };
        if pending(gpu, t.used_in.get()) && mode != WriteMode::NoOverwrite {
            split_pass(gpu, SlowPath::WriteSplitPass, move |w| {
                draw(w, Stream::Main)
            });
        } else {
            draw(gpu, Stream::Lead);
        }
        return;
    }
    // A discard of a texture recorded work uses takes fresh storage, unless the open pass draws
    // into it, which must keep drawing into the storage it opened with.
    let attached = gpu
        .pass
        .as_ref()
        .is_some_and(|p| p.attached.iter().any(|a| Rc::ptr_eq(&a.0, &t)));
    let renamed = mode == WriteMode::Discard && pending(gpu, t.used_in.get()) && !attached;
    if renamed {
        rename_texture(gpu, &t);
    }
    let raw = t.raw();
    let copy = texel_copy(
        &raw,
        &t,
        region.mip,
        region.layer,
        region.origin,
        region.stencil,
    );
    if renamed || !pending(gpu, t.used_in.get()) || mode == WriteMode::NoOverwrite {
        let format = region_format(&t, region);
        let (row, rows) = row_bytes(format, extent.width, extent.height);
        // wgpu's Direct3D 12 loses every layer after the first when layers are further apart
        // than their rows: such layers are packed together first.
        let packed;
        let (data, rows_per_layer) = if extent.depth_or_layers > 1
            && u64::from(rows_per_layer) > rows
        {
            gpu.record_slow_path(SlowPath::RowsRepacked);
            let layer = u64::from(row_pitch) * u64::from(rows_per_layer);
            let tight = u64::from(row_pitch) * (rows - 1) + row;
            let mut out = Vec::new();
            for l in 0..u64::from(extent.depth_or_layers) {
                let Some(src) = data.get((l * layer) as usize..(l * layer + tight) as usize) else {
                    gpu.fail("write_texture with fewer bytes than its extent");
                    return;
                };
                out.extend_from_slice(src);
                out.resize((u64::from(row_pitch) * rows * (l + 1)) as usize, 0);
            }
            packed = out;
            (&packed[..], rows as u32)
        } else {
            (data, rows_per_layer)
        };
        gpu.queue.write_texture(
            copy,
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_pitch),
                rows_per_image: Some(rows_per_layer),
            },
            extent3d(extent, format),
        );
        return;
    }
    // Recorded commands use the texture: the write goes through a staging copy in command order.
    let format = region_format(&t, region);
    let (row, rows) = row_bytes(format, extent.width, extent.height);
    let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
    let layers = u64::from(extent.depth_or_layers);
    let mut packed = vec![0u8; (pitch * rows * layers) as usize];
    for l in 0..layers {
        for r in 0..rows {
            let from = (l * u64::from(rows_per_layer) + r) * u64::from(row_pitch);
            let to = (l * rows + r) * pitch;
            let Some(src) = data.get(from as usize..(from + row) as usize) else {
                gpu.fail("write_texture with fewer bytes than its extent");
                return;
            };
            packed[to as usize..(to + row) as usize].copy_from_slice(src);
        }
    }
    let (staging, base) = staging(gpu, &packed);
    gpu.counters.staged_writes += 1;
    let tex = t.clone();
    let (mip, layer, origin, stencil) = (region.mip, region.layer, region.origin, region.stencil);
    split_pass(gpu, SlowPath::WriteSplitPass, move |w| {
        w.encoder().copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: base,
                    bytes_per_row: Some(pitch as u32),
                    rows_per_image: Some(rows as u32),
                },
            },
            texel_copy(&tex.raw(), &tex, mip, layer, origin, stencil),
            extent3d(extent, format),
        );
    });
}

pub(crate) fn copy_buffer_impl(
    gpu: &mut WebGpu,
    src: &Buffer,
    src_offset: u64,
    dst: &Buffer,
    dst_offset: u64,
    size: u64,
) {
    let (Some(src), Some(dst)) = (buffer_of(gpu, src), buffer_of(gpu, dst)) else {
        return gpu.fail(GpuError::GONE);
    };
    if gpu.pass.is_some() {
        gpu.fail("copy_buffer inside a pass");
        return;
    }
    dst.wrote();
    if src_offset + size > src.desc.size || dst_offset + size > dst.desc.size {
        gpu.fail("copy_buffer past a buffer's end");
        return;
    }
    let serial = gpu.serial;
    src.used_in.set(serial);
    dst.used_in.set(serial);
    let (s, d) = (src.raw.borrow().clone(), dst.raw.borrow().clone());
    let (src_offset, dst_offset) = (src.base.get() + src_offset, dst.base.get() + dst_offset);
    // One GPU buffer: the same buffer, or two living in the uniform arena.
    if src.storage.get() == dst.storage.get() {
        gpu.record_slow_path(SlowPath::CopyWithinBuffer);
        // The temporary holds the bytes in the same place of their word as the source.
        let at = src_offset % ALIGN;
        let temp = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (at + size).next_multiple_of(ALIGN),
            usage: wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        copy_bytes(gpu, Stream::Main, &s, src_offset, &temp, at, size);
        copy_bytes(gpu, Stream::Main, &temp, at, &d, dst_offset, size);
    } else {
        copy_bytes(gpu, Stream::Main, &s, src_offset, &d, dst_offset, size);
    }
}

pub(crate) fn copy_texture_impl(
    gpu: &mut WebGpu,
    src: &TextureRegion<'_, WebGpu>,
    dst: &TextureRegion<'_, WebGpu>,
    extent: Extent,
) {
    let (Some(s), Some(d)) = (texture_of(gpu, src.texture), texture_of(gpu, dst.texture)) else {
        return gpu.fail(GpuError::GONE);
    };
    if gpu.pass.is_some() {
        gpu.fail("copy_texture inside a pass");
        return;
    }
    if s.blocks.is_some() || d.blocks.is_some() {
        copy_blocks(gpu, src, dst, extent);
        return;
    }
    let (s, d) = (&*s, &*d);
    let converted = |t: &crate::TextureObj| packed::held(t.desc.format).is_some();
    if s.desc.format != d.desc.format && (converted(s) || converted(d)) {
        copy_packed(gpu, src, dst, extent);
        return;
    }
    s.used_in.set(gpu.serial);
    d.used_in.set(gpu.serial);
    let (sf, df) = (s.held, d.held);
    let covers = |t: &crate::TextureObj, r: &TextureRegion<'_, WebGpu>| {
        let (w, h) = (
            (t.desc.width >> r.mip).max(1),
            (t.desc.height >> r.mip).max(1),
        );
        r.origin.x == 0 && r.origin.y == 0 && extent.width == w && extent.height == h
    };
    let whole = covers(d, dst) && covers(s, src);
    // WebGPU refuses one subresource as both source and destination.
    let within = src.texture.0 == dst.texture.0
        && src.mip == dst.mip
        && (s.desc.dimension == TextureDimension::D3
            || (src.layer < dst.layer + extent.depth_or_layers
                && dst.layer < src.layer + extent.depth_or_layers));
    // Depth and stencil go through a temporary texture of their format, each half by the
    // draws below.
    if within && FORMAT_FACTS[df as usize].depth_stencil {
        gpu.record_slow_path(SlowPath::CopyWithinTexture);
        let Ok(temp) = gpu.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: d.desc.format,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        }) else {
            gpu.fail("the texture a depth or stencil copy within one subresource goes through could not be made");
            return;
        };
        let staged = |stencil| TextureRegion {
            texture: &temp,
            mip: 0,
            layer: 0,
            origin: Default::default(),
            stencil,
        };
        copy_texture_impl(gpu, src, &staged(dst.stencil), extent);
        copy_texture_impl(gpu, &staged(src.stencil), dst, extent);
        forget_texture(gpu, &temp);
        return;
    }
    // A combined format is copied with both aspects.
    let combined = FORMAT_FACTS[sf as usize].kind == FormatKind::DepthStencil
        || FORMAT_FACTS[df as usize].kind == FormatKind::DepthStencil;
    if combined && sf != df {
        gpu.fail("a copy between a combined depth-stencil format and another format");
        return;
    }
    // WebGPU copies depth and stencil only between equal formats and only whole; the rest
    // are draws writing each aspect.
    if FORMAT_FACTS[df as usize].depth_stencil && (sf != df || !whole) {
        let at = (src.mip, src.layer, [src.origin.x, src.origin.y]);
        if FORMAT_FACTS[df as usize].kind != FormatKind::Stencil {
            if let Some(how) = DepthSource::of(sf, df) {
                depth_copy(gpu, (&s.raw(), sf, how), at, dst, extent, Stream::Main);
            } else if df == Format::Depth16Unorm
                && matches!(sf, Format::R16Float | Format::R16Snorm | Format::R16Sint)
            {
                // The rest of the 16-bit group is drawn from its bits, copied first into
                // `r16uint` through a buffer.
                let Ok(bits) = gpu.create_texture(&TextureDesc {
                    dimension: TextureDimension::D2,
                    format: Format::R16Uint,
                    width: extent.width,
                    height: extent.height,
                    depth_or_layers: extent.depth_or_layers,
                    mip_levels: 1,
                    samples: 1,
                    usage: TextureUsage::COPY_DST | TextureUsage::SAMPLE,
                    cube: false,
                    reinterpret: false,
                }) else {
                    gpu.fail("the texture a depth copy is drawn from could not be made");
                    return;
                };
                let staged = TextureRegion {
                    texture: &bits,
                    mip: 0,
                    layer: 0,
                    origin: Default::default(),
                    stencil: false,
                };
                copy_texture_impl(gpu, src, &staged, extent);
                let how = DepthSource::Unorm16;
                let Some(bits) = forget_texture(gpu, &bits) else {
                    return gpu.fail(GpuError::GONE);
                };
                let raw = bits.raw();
                depth_copy(
                    gpu,
                    (&raw, Format::R16Uint, how),
                    (0, 0, [0, 0]),
                    dst,
                    extent,
                    Stream::Main,
                );
            } else {
                gpu.fail("a copy into depth from a format outside its typeless group");
                return;
            }
        }
        if FORMAT_FACTS[df as usize].kind != FormatKind::Depth {
            if sf != df {
                gpu.fail("a copy into stencil from another format");
                return;
            }
            stencil_copy(
                gpu,
                (&s.raw(), wgpu::TextureAspect::StencilOnly),
                at,
                dst,
                extent,
            );
        }
        return;
    }
    let (s_raw, d_raw) = (s.raw(), d.raw());
    let mut from = texel_copy(&s_raw, s, src.mip, src.layer, src.origin, src.stencil);
    let mut to = texel_copy(&d_raw, d, dst.mip, dst.layer, dst.origin, dst.stencil);
    if combined {
        from.aspect = wgpu::TextureAspect::All;
        to.aspect = wgpu::TextureAspect::All;
    }
    if FORMAT_FACTS[sf as usize].linear == FORMAT_FACTS[df as usize].linear && !within {
        gpu.encoder
            .get_or_insert_with(|| {
                gpu.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
            })
            .copy_texture_to_texture(from, to, extent3d(extent, sf));
    } else if FORMAT_FACTS[sf as usize].block.2 == FORMAT_FACTS[df as usize].block.2
        && !FORMAT_FACTS[df as usize].depth_stencil
    {
        // Regions of one subresource, typeless twins, or a block-compressed format and an
        // uncompressed one whose texels are its blocks: the destination's extent covers the
        // same blocks.
        gpu.record_slow_path(if within {
            SlowPath::CopyWithinTexture
        } else {
            SlowPath::CopyThroughBuffer
        });
        // Depth WebGPU cannot copy out (part of a level, 24-bit) is drawn into a colour
        // texture of its bytes first (`DepthReadByDraw`).
        let drawn = match FORMAT_FACTS[sf as usize].depth_stencil {
            true => match aspect_as_colour(gpu, src, extent) {
                Ok(drawn) => drawn,
                Err(()) => return,
            },
            false => None,
        };
        let drawn = drawn.and_then(|t| forget_texture(gpu, &t));
        let drawn_raw = drawn.as_ref().map(|t| t.raw());
        if let (Some(t), Some(raw)) = (&drawn, &drawn_raw) {
            from = texel_copy(raw, t, 0, 0, Default::default(), false);
        }
        let ((sbw, sbh, _), (dbw, dbh, _)) = (
            FORMAT_FACTS[sf as usize].block,
            FORMAT_FACTS[df as usize].block,
        );
        let into = Extent {
            width: extent.width.div_ceil(sbw) * dbw,
            height: extent.height.div_ceil(sbh) * dbh,
            depth_or_layers: extent.depth_or_layers,
        };
        let (row, rows) = row_bytes(sf, extent.width, extent.height);
        let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let temp = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: pitch * rows * u64::from(extent.depth_or_layers),
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(pitch as u32),
            rows_per_image: Some(rows as u32),
        };
        let e = gpu.encoder.get_or_insert_with(|| {
            gpu.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        });
        let info = wgpu::TexelCopyBufferInfo {
            buffer: &temp,
            layout,
        };
        e.copy_texture_to_buffer(from, info, extent3d(extent, sf));
        e.copy_buffer_to_texture(info, to, extent3d(into, df));
    } else {
        gpu.fail("a copy between formats of different block sizes");
    }
}

/// Writes the depth of `dst` from `src` by draws, after checking what the draw needs.
fn depth_copy(
    gpu: &mut WebGpu,
    src: (&wgpu::Texture, Format, DepthSource),
    src_at: (u32, u32, [u32; 2]),
    dst: &TextureRegion<'_, WebGpu>,
    extent: Extent,
    stream: Stream,
) {
    let Some(d) = texture_of(gpu, dst.texture) else {
        return gpu.fail(GpuError::GONE);
    };
    if src.0.sample_count() > 1 || d.desc.samples > 1 {
        gpu.fail("multisampled depth copies by a draw are not built yet");
        return;
    }
    if !src.0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
        || !d
            .raw()
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        gpu.fail("a depth copy by a draw needs the source sampled and the destination rendered");
        return;
    }
    depth_by_draw(
        gpu,
        stream,
        src,
        src_at,
        (&d.raw(), d.desc.format),
        (dst.mip, dst.layer, [dst.origin.x, dst.origin.y]),
        [extent.width, extent.height],
        extent.depth_or_layers,
    );
}

/// Writes the stencil of `dst` from the `u32` texels of `src` in its aspect by draws, after
/// checking what the draws need.
fn stencil_copy(
    gpu: &mut WebGpu,
    src: (&wgpu::Texture, wgpu::TextureAspect),
    src_at: (u32, u32, [u32; 2]),
    dst: &TextureRegion<'_, WebGpu>,
    extent: Extent,
) {
    let Some(d) = texture_of(gpu, dst.texture) else {
        return gpu.fail(GpuError::GONE);
    };
    if src.0.sample_count() > 1 || d.desc.samples > 1 {
        gpu.fail("multisampled stencil copies by a draw are not built yet");
        return;
    }
    if !src.0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
        || !d
            .raw()
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        gpu.fail("a stencil copy by a draw needs the source sampled and the destination rendered");
        return;
    }
    stencil_by_draw(
        gpu,
        Stream::Main,
        src,
        src_at,
        &d.raw(),
        (dst.mip, dst.layer, [dst.origin.x, dst.origin.y]),
        [extent.width, extent.height],
        extent.depth_or_layers,
    );
}

/// The texels of a depth or stencil region WebGPU cannot copy out (24-bit depth, or part of a
/// level), drawn into a colour texture of the extent's size whose texels are the bytes a copy
/// of that aspect holds (`DepthReadByDraw`). `Ok(None)` where WebGPU copies the region itself.
fn aspect_as_colour(
    gpu: &mut WebGpu,
    region: &TextureRegion<'_, WebGpu>,
    extent: Extent,
) -> Result<Option<Texture>, ()> {
    let Some(t) = texture_of(gpu, region.texture) else {
        gpu.fail(GpuError::GONE);
        return Err(());
    };
    let format = t.desc.format;
    let stencil = region.stencil || FORMAT_FACTS[format as usize].kind == FormatKind::Stencil;
    let Some((read, colour)) = AspectRead::of(format, stencil) else {
        return Ok(None);
    };
    let whole = region.origin.x == 0
        && region.origin.y == 0
        && extent.width == (t.desc.width >> region.mip).max(1)
        && extent.height == (t.desc.height >> region.mip).max(1);
    if whole && read != AspectRead::Unorm24 {
        return Ok(None);
    }
    if t.desc.samples > 1
        || !t
            .raw()
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
    {
        gpu.fail("a depth or stencil read by a draw needs a single-sampled bound source");
        return Err(());
    }
    let Ok(drawn) = gpu.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format: colour,
        width: extent.width,
        height: extent.height,
        depth_or_layers: extent.depth_or_layers,
        mip_levels: 1,
        samples: 1,
        usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
        cube: false,
        reinterpret: false,
    }) else {
        gpu.fail("the texture a depth or stencil read is drawn into could not be made");
        return Err(());
    };
    t.used_in.set(gpu.serial);
    let raw = t.raw();
    aspect_by_draw(
        gpu,
        (&raw, read),
        (region.mip, region.layer, [region.origin.x, region.origin.y]),
        &texture_of(gpu, &drawn).ok_or(())?.raw(),
        [extent.width, extent.height],
        extent.depth_or_layers,
    );
    Ok(Some(drawn))
}

/// A single-sampled texture holding a buffer's texels for a depth draw into `format`, which
/// WebGPU cannot fill from a buffer: `r32float` for 32-bit float depth, `r32uint` for 24-bit.
fn depth_staging(
    gpu: &mut WebGpu,
    format: Format,
    extent: Extent,
) -> Option<(Texture, Format, DepthSource)> {
    let (staged, how) = match format {
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            (Format::R32Uint, DepthSource::Unorm24)
        }
        _ => (Format::R32Float, DepthSource::Float),
    };
    let t = gpu
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: staged,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_DST | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        })
        .ok()?;
    Some((t, staged, how))
}

/// A copy between a buffer and a texture region, either way.
pub(crate) fn copy_buffer_texture(
    gpu: &mut WebGpu,
    buffer: &CopyBufferTexture<'_, WebGpu>,
    region: &TextureRegion<'_, WebGpu>,
    extent: Extent,
    to_texture: bool,
) {
    let (Some(t), Some(b)) = (
        texture_of(gpu, region.texture),
        buffer_of(gpu, buffer.buffer),
    ) else {
        return gpu.fail(GpuError::GONE);
    };
    if gpu.pass.is_some() {
        gpu.fail("buffer and texture copies inside a pass");
        return;
    }
    if !to_texture {
        b.wrote();
    }
    if t.blocks.is_some() {
        copy_buffer_blocks(gpu, buffer, region, extent, to_texture);
        return;
    }
    if packed::held(t.desc.format).is_some() {
        copy_buffer_packed(gpu, buffer, region, extent, to_texture);
        return;
    }
    let at_origin = |texture| TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let stencil =
        region.stencil || FORMAT_FACTS[t.desc.format as usize].kind == FormatKind::Stencil;
    let whole = region.origin.x == 0
        && region.origin.y == 0
        && extent.width == (t.desc.width >> region.mip).max(1)
        && extent.height == (t.desc.height >> region.mip).max(1);
    if !to_texture {
        match aspect_as_colour(gpu, region, extent) {
            Err(()) => return,
            Ok(Some(drawn)) => {
                copy_buffer_texture(gpu, buffer, &at_origin(&drawn), extent, false);
                forget_texture(gpu, &drawn);
                return;
            }
            Ok(None) => {}
        }
    } else if stencil && !whole {
        let Ok(staged) = gpu.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::R8Uint,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_DST | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        }) else {
            gpu.fail("the texture a stencil copy is drawn from could not be made");
            return;
        };
        copy_buffer_texture(gpu, buffer, &at_origin(&staged), extent, true);
        t.used_in.set(gpu.serial);
        let Some(staged) = forget_texture(gpu, &staged) else {
            return gpu.fail(GpuError::GONE);
        };
        let raw = staged.raw();
        stencil_copy(
            gpu,
            (&raw, wgpu::TextureAspect::All),
            (0, 0, [0, 0]),
            region,
            extent,
        );
        return;
    } else if t.desc.format == Format::Depth16Unorm && !whole {
        gpu.fail("part of a 16-bit depth level written from a buffer");
        return;
    }
    if to_texture && !region.stencil && filled_by_draw(t.desc.format) {
        let Some((staged, sf, how)) = depth_staging(gpu, t.desc.format, extent) else {
            gpu.fail("the texture a depth copy is drawn from could not be made");
            return;
        };
        copy_buffer_texture(gpu, buffer, &at_origin(&staged), extent, true);
        t.used_in.set(gpu.serial);
        let Some(staged) = forget_texture(gpu, &staged) else {
            return gpu.fail(GpuError::GONE);
        };
        depth_copy(
            gpu,
            (&staged.raw(), sf, how),
            (0, 0, [0, 0]),
            region,
            extent,
            Stream::Main,
        );
        return;
    }
    t.used_in.set(gpu.serial);
    b.used_in.set(gpu.serial);
    let raw = b.raw.borrow().clone();
    let offset = b.base.get() + buffer.offset;
    let tex = t.clone();
    let format = region_format(&t, region);
    let (mip, layer, origin, stencil) = (region.mip, region.layer, region.origin, region.stencil);
    let copy_layers = |w: &mut WebGpu,
                       staged: &wgpu::Buffer,
                       offset: u64,
                       pitch: u64,
                       rows: u64,
                       layers: Range<u32>| {
        let info = wgpu::TexelCopyBufferInfo {
            buffer: staged,
            layout: wgpu::TexelCopyBufferLayout {
                offset,
                bytes_per_row: Some(pitch as u32),
                rows_per_image: Some(rows as u32),
            },
        };
        let tex_raw = tex.raw();
        let shifted = Origin {
            z: origin.z + layers.start,
            ..origin
        };
        let to = texel_copy(&tex_raw, &tex, mip, layer + layers.start, shifted, stencil);
        let part = Extent {
            depth_or_layers: layers.len() as u32,
            ..extent
        };
        if to_texture {
            w.encoder()
                .copy_buffer_to_texture(info, to, extent3d(part, format));
        } else {
            w.encoder()
                .copy_texture_to_buffer(to, info, extent3d(part, format));
        }
    };
    let copy = |w: &mut WebGpu, staged: &wgpu::Buffer, offset: u64, pitch: u64, rows: u64| {
        copy_layers(w, staged, offset, pitch, rows, 0..extent.depth_or_layers);
    };
    let pitch = u64::from(buffer.row_pitch);
    let (row, rows) = row_bytes(region_format(&t, region), extent.width, extent.height);
    let gaps = (rows > 1 && pitch > row)
        || (extent.depth_or_layers > 1 && u64::from(buffer.rows_per_layer) > rows);
    let gaps_kept =
        to_texture || !gpu.padded_reads || !gaps || offset.is_multiple_of(DX12_PLACEMENT_ALIGNMENT);
    if pitch.is_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
        && offset.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
        && gaps_kept
    {
        let rows_per_layer = u64::from(buffer.rows_per_layer);
        // wgpu's Direct3D 12 reads the layers of one copy from the buffer at the extent's
        // height, so layers further apart are written one copy each.
        if to_texture && extent.depth_or_layers > 1 && rows_per_layer > rows {
            for l in 0..extent.depth_or_layers {
                let at = offset + u64::from(l) * rows_per_layer * pitch;
                copy_layers(gpu, &raw, at, pitch, rows, l..l + 1);
            }
        } else {
            copy(gpu, &raw, offset, pitch, rows_per_layer);
        }
        return;
    }
    // The rows go through a buffer at a pitch and place WebGPU takes, one buffer copy per row.
    gpu.record_slow_path(SlowPath::RowsRepacked);
    let api_pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
    let layers = u64::from(extent.depth_or_layers);
    let temp = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: api_pitch * rows * layers,
        usage: wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let theirs = |l: u64, r: u64| offset + (l * u64::from(buffer.rows_per_layer) + r) * pitch;
    let ours = |l: u64, r: u64| (l * rows + r) * api_pitch;
    if !to_texture {
        copy(gpu, &temp, 0, api_pitch, rows);
    }
    for l in 0..layers {
        for r in 0..rows {
            if to_texture {
                copy_bytes(
                    gpu,
                    Stream::Main,
                    &raw,
                    theirs(l, r),
                    &temp,
                    ours(l, r),
                    row,
                );
            } else {
                copy_bytes(
                    gpu,
                    Stream::Main,
                    &temp,
                    ours(l, r),
                    &raw,
                    theirs(l, r),
                    row,
                );
            }
        }
    }
    if to_texture {
        copy(gpu, &temp, 0, api_pitch, rows);
    }
}

pub(crate) fn clear_buffer_impl(
    gpu: &mut WebGpu,
    buffer: &Buffer,
    offset: u64,
    size: u64,
    pattern: &[u8],
) {
    let Some(b) = buffer_of(gpu, buffer) else {
        return gpu.fail(GpuError::GONE);
    };
    if gpu.pass.is_some() {
        gpu.fail("clear_buffer inside a pass");
        return;
    }
    b.wrote();
    if pattern.is_empty() || !size.is_multiple_of(pattern.len() as u64) {
        gpu.fail("clear_buffer with a range that is not a whole number of patterns");
        return;
    }
    if offset + size > b.desc.size {
        gpu.fail("clear_buffer past the buffer's end");
        return;
    }
    b.used_in.set(gpu.serial);
    let raw = b.raw.borrow().clone();
    let offset = b.base.get() + offset;
    let zero = pattern.iter().all(|b| *b == 0);
    if zero && offset.is_multiple_of(ALIGN) && size.is_multiple_of(ALIGN) {
        gpu.encoder().clear_buffer(&raw, offset, Some(size));
        return;
    }
    if !zero {
        gpu.record_slow_path(SlowPath::PatternClearCopied);
    }
    // One block of whole patterns, in the same place of its word as the range, copied over the
    // range; its size is a multiple of the pattern and of 4, so every copy starts on a pattern and
    // on the same place of a word.
    let len = pattern.len() as u64;
    let unit = len << (2 - len.trailing_zeros().min(2));
    let block_size = size.min(PATTERN_BLOCK.div_ceil(unit) * unit);
    let block: Vec<u8> = pattern
        .iter()
        .copied()
        .cycle()
        .take(block_size as usize)
        .collect();
    let at = offset % ALIGN;
    let (staging, base) = staging_at(gpu, at, &block);
    let mut done = 0;
    while done < size {
        let n = block_size.min(size - done);
        copy_bytes(
            gpu,
            Stream::Main,
            &staging,
            base + at,
            &raw,
            offset + done,
            n,
        );
        done += n;
    }
}

fn map_target(gpu: &WebGpu, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size.max(ALIGN),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn readback(
    gpu: &mut WebGpu,
    buffer: wgpu::Buffer,
    packed: Option<Format>,
    skip: u64,
    row: u64,
    pitch: u64,
    rows: u64,
) -> Readback {
    let rb = HeldReadback(Rc::new(ReadbackObj {
        buffer: Some(buffer),
        host: Vec::new(),
        packed,
        state: Arc::new(AtomicU8::new(MAP_PENDING)),
        serial: gpu.serial,
        skip,
        row,
        pitch,
        rows,
    }));
    gpu.to_map.push(rb.clone());
    let number = gpu.id();
    gpu.objects.readbacks.insert(number, rb);
    Readback(number)
}

/// A read back the host answers at once.
pub(crate) fn host_readback(gpu: &mut WebGpu, bytes: Vec<u8>) -> Readback {
    let len = bytes.len() as u64;
    let rb = HeldReadback(Rc::new(ReadbackObj {
        buffer: None,
        host: bytes,
        packed: None,
        state: Arc::new(AtomicU8::new(MAP_READY)),
        serial: gpu.serial,
        skip: 0,
        row: len,
        pitch: len,
        rows: 1,
    }));
    let number = gpu.id();
    gpu.objects.readbacks.insert(number, rb);
    Readback(number)
}

pub(crate) fn read_back_impl(
    gpu: &mut WebGpu,
    src: &ReadSource<'_, WebGpu>,
) -> Result<Readback, GpuError> {
    if gpu.pass.is_some() {
        return Err(GpuError::Invalid("read backs are recorded outside a pass"));
    }
    match src {
        ReadSource::Buffer {
            buffer,
            offset,
            size,
        } => {
            let start = offset & !(ALIGN - 1);
            let end = (offset + size).next_multiple_of(ALIGN);
            let buffer = buffer_of(gpu, buffer).ok_or(GpuError::Invalid(GpuError::GONE))?;
            let target = map_target(gpu, end - start);
            buffer.used_in.set(gpu.serial);
            let raw = buffer.raw.borrow().clone();
            gpu.encoder().copy_buffer_to_buffer(
                &raw,
                buffer.base.get() + start,
                &target,
                0,
                end - start,
            );
            Ok(readback(gpu, target, None, offset - start, *size, *size, 1))
        }
        ReadSource::Texture { region, extent } => {
            let texture =
                texture_of(gpu, region.texture).ok_or(GpuError::Invalid(GpuError::GONE))?;
            if texture.blocks.is_some() {
                return read_blocks(gpu, region, *extent).ok_or(GpuError::Invalid(
                    "a read back outside the texture's blocks",
                ));
            }
            let drawn = match aspect_as_colour(gpu, region, *extent) {
                Err(()) => return Err(GpuError::DeviceLost),
                Ok(drawn) => drawn,
            };
            let at;
            let region = match &drawn {
                Some(texture) => {
                    at = TextureRegion {
                        texture,
                        mip: 0,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    };
                    &at
                }
                None => region,
            };
            let t = match &drawn {
                Some(drawn) => {
                    forget_texture(gpu, drawn).ok_or(GpuError::Invalid(GpuError::GONE))?
                }
                None => texture,
            };
            let t = &*t;
            t.used_in.set(gpu.serial);
            let packed = packed::held(t.desc.format).map(|_| t.desc.format);
            if let Some(f) = packed {
                if !region
                    .origin
                    .x
                    .is_multiple_of(FORMAT_FACTS[f as usize].block.0)
                {
                    return Err(GpuError::Invalid(
                        "a read back of a two-pixel format starting inside a pair",
                    ));
                }
                gpu.record_slow_path(SlowPath::FormatConverted);
            }
            let (row, rows) = row_bytes(region_format(t, region), extent.width, extent.height);
            let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
            let layers = u64::from(extent.depth_or_layers);
            let target = map_target(gpu, pitch * rows * layers);
            let raw = t.raw();
            let tex = texel_copy(
                &raw,
                t,
                region.mip,
                region.layer,
                region.origin,
                region.stencil,
            );
            gpu.encoder().copy_texture_to_buffer(
                tex,
                wgpu::TexelCopyBufferInfo {
                    buffer: &target,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(pitch as u32),
                        rows_per_image: Some(rows as u32),
                    },
                },
                extent3d(*extent, region_format(t, region)),
            );
            let rb = readback(gpu, target, packed, 0, row, pitch, rows * layers);
            Ok(rb)
        }
    }
}

/// Resolves queries `first..first + count` into a region of the query ring, each own page's
/// part at a multiple of WebGPU's 256-byte resolve alignment; answers the ring's buffer and
/// the parts.
fn resolved(
    gpu: &mut WebGpu,
    set: &QuerySet,
    first: u32,
    count: u32,
) -> Result<(wgpu::Buffer, Vec<ResolvedPart>), GpuError> {
    let set = query_set_of(gpu, set).ok_or(GpuError::Invalid(GpuError::GONE))?;
    if gpu.pass.is_some() {
        return Err(GpuError::Invalid("query results are taken outside a pass"));
    }
    if set.kind == QueryKind::Occlusion
        || first.checked_add(count).is_none_or(|end| end > set.count)
    {
        return Err(GpuError::Invalid("queries outside the set"));
    }
    let obj = &*set;
    let max = wgpu::QUERY_SET_MAX_QUERIES;
    // Each own page's part of the range, as (first result, first place in the region,
    // results).
    let mut parts = Vec::new();
    let (mut i, mut place) = (first, 0);
    while i < first + count {
        let end = (i / max + 1).saturating_mul(max).min(first + count);
        parts.push((i - first, place, end - i));
        place = (place + end - i).next_multiple_of(PAGE_ALIGN);
        i = end;
    }
    let places = parts.last().map_or(1, |&(_, at, n)| at + n);
    let at_of = |i: u32| {
        let &(r, at, _) = &parts[(i / max - first / max) as usize];
        at + (i - first - r)
    };
    // Queries carried across pass splits: (place, spare slot) pairs to add. The spare slots
    // resolve after the last own page's queries, then page by page.
    let last = obj.own_pages - 1;
    let own_in_last = obj.count - last as u32 * max;
    let spares_last = obj.page(last).count() - own_in_last;
    let base = |page: usize, slot: u32| {
        if page == last {
            slot - own_in_last
        } else {
            spares_last.next_multiple_of(PAGE_ALIGN) + (page - obj.own_pages) as u32 * max + slot
        }
    };
    let pairs: Vec<u32> = obj
        .segments
        .borrow()
        .iter()
        .filter(|(i, _)| (first..first + count).contains(*i))
        .flat_map(|(&i, s)| {
            s.slots
                .iter()
                .enumerate()
                .flat_map(move |(k, &(page, slot))| {
                    let replace = if k == 0 && !s.own { REPLACE } else { 0 };
                    [at_of(i), base(page, slot) | replace]
                })
        })
        .collect();
    let size = u64::from(places) * QUERY_BYTES;
    let (sums, at) = Ring::region(
        &mut gpu.query_ring,
        &gpu.device,
        size,
        QUERY_USAGE,
        &mut gpu.counters.query_rings,
    );
    for &(r, place, n) in &parts {
        let (page, slot) = (((first + r) / max) as usize, (first + r) % max);
        let raw = obj.page(page);
        gpu.encoder().resolve_query_set(
            &raw,
            slot..slot + n,
            &sums,
            at + u64::from(place) * QUERY_BYTES,
        );
    }
    if !pairs.is_empty() {
        let spare_count = spares_last.next_multiple_of(PAGE_ALIGN)
            + (obj.pages.borrow().len() - obj.own_pages) as u32 * max;
        let (spare, spare_at) = staging_region(gpu, u64::from(spare_count) * QUERY_BYTES);
        let e = gpu.encoder();
        if spares_last > 0 {
            e.resolve_query_set(
                &obj.page(last),
                own_in_last..own_in_last + spares_last,
                &spare,
                spare_at,
            );
        }
        for page in obj.own_pages..obj.pages.borrow().len() {
            let k = base(page, 0);
            e.resolve_query_set(
                &obj.page(page),
                0..max,
                &spare,
                spare_at + u64::from(k) * QUERY_BYTES,
            );
        }
        add_segments(
            gpu,
            (&sums, at, size),
            (&spare, spare_at, u64::from(spare_count) * QUERY_BYTES),
            &pairs,
        );
    }
    let parts = parts
        .into_iter()
        .map(|(r, place, n)| (r, at + u64::from(place) * QUERY_BYTES, n))
        .collect();
    Ok((sums, parts))
}

/// Adds the counts of the resolved spare slots `pairs` names (place in `sums`, slot in
/// `spare`) into the resolved results; regions as (buffer, offset, size).
fn add_segments(
    gpu: &mut WebGpu,
    sums: (&wgpu::Buffer, u64, u64),
    spare: (&wgpu::Buffer, u64, u64),
    pairs: &[u32],
) {
    let bytes: Vec<u8> = pairs.iter().flat_map(|w| w.to_le_bytes()).collect();
    let (list, list_at) = staging(gpu, &bytes);
    let (pipeline, layout) = gpu.query_sum.get_or_insert_with(|| {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(query_sum_wgsl().into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: None,
                compilation_options: Default::default(),
                cache: None,
            });
        let layout = pipeline.get_bind_group_layout(0);
        (pipeline, layout)
    });
    let (pipeline, layout) = (pipeline.clone(), layout.clone());
    fn region((buffer, offset, size): (&wgpu::Buffer, u64, u64)) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset,
            size: NonZeroU64::new(size),
        })
    }
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: region(sums),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: region(spare),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: region((&list, list_at, bytes.len() as u64)),
            },
        ],
    });
    let mut cp = gpu
        .encoder()
        .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
    cp.set_pipeline(&pipeline);
    cp.set_bind_group(0, &group, &[]);
    cp.dispatch_workgroups(1, 1, 1);
}

pub(crate) fn read_queries_impl(
    gpu: &mut WebGpu,
    set: &QuerySet,
    first: u32,
    count: u32,
) -> Result<Readback, GpuError> {
    let (sums, parts) = resolved(gpu, set, first, count)?;
    let size = u64::from(count) * QUERY_BYTES;
    let target = map_target(gpu, size);
    let e = gpu.encoder();
    for (r, at, n) in parts {
        e.copy_buffer_to_buffer(
            &sums,
            at,
            &target,
            u64::from(r) * QUERY_BYTES,
            u64::from(n) * QUERY_BYTES,
        );
    }
    Ok(readback(gpu, target, None, 0, size, size, 1))
}

pub(crate) fn resolve_queries_impl(
    gpu: &mut WebGpu,
    set: &QuerySet,
    first: u32,
    count: u32,
    dst: &Buffer,
    dst_offset: u64,
) -> Result<(), GpuError> {
    let dst = buffer_of(gpu, dst).ok_or(GpuError::Invalid(GpuError::GONE))?;
    if !dst_offset.is_multiple_of(QUERY_BYTES) {
        return Err(GpuError::Invalid(
            "query results go to multiples of 8 bytes",
        ));
    }
    if dst_offset
        .checked_add(u64::from(count) * QUERY_BYTES)
        .is_none_or(|end| end > dst.desc.size)
    {
        return Err(GpuError::Invalid("query results past the buffer's end"));
    }
    let (sums, parts) = resolved(gpu, set, first, count)?;
    dst.wrote();
    dst.used_in.set(gpu.serial);
    let raw = dst.raw.borrow().clone();
    let to = dst.base.get() + dst_offset;
    let e = gpu.encoder();
    for (r, at, n) in parts {
        e.copy_buffer_to_buffer(
            &sums,
            at,
            &raw,
            to + u64::from(r) * QUERY_BYTES,
            u64::from(n) * QUERY_BYTES,
        );
    }
    Ok(())
}

/// As [`finish_read_impl`], into bytes another party holds: a read back of one run of bytes
/// goes from its mapping into them in one copy; any other is finished into a copy first.
pub(crate) fn finish_read_into_impl(
    gpu: &mut WebGpu,
    readback: Readback,
    dst: &impl VolatileBytes,
) {
    let Some(held) = readback_of(gpu, &readback) else {
        return gpu.fail(GpuError::GONE);
    };
    let o = &*held;
    let whole = o.state.load(Ordering::Acquire) == MAP_READY
        && o.packed.is_none()
        && (o.rows <= 1 || o.pitch == o.row)
        && dst.len() >= o.row * o.rows;
    if let (true, Some(buffer)) = (whole, &o.buffer) {
        let from = o.skip as usize;
        let to = from + (o.row * o.rows) as usize;
        {
            let Ok(data) = buffer.slice(..).get_mapped_range() else {
                gpu.fail("a ready read back could not be mapped");
                return;
            };
            dst.write(&data[from..to]);
        }
        buffer.unmap();
        buffer.destroy();
        gpu.objects.readbacks.remove(&readback.0);
        return;
    }
    let mut bytes = vec![0; usize::try_from(dst.len()).unwrap_or(usize::MAX)];
    finish_read_impl(gpu, readback, &mut bytes);
    dst.write(&bytes);
}

pub(crate) fn finish_read_impl(gpu: &mut WebGpu, readback: Readback, dst: &mut [u8]) {
    let Some(held) = gpu.objects.readbacks.remove(&readback.0) else {
        return gpu.fail(GpuError::GONE);
    };
    let o = &*held.0;
    if o.state.load(Ordering::Acquire) != MAP_READY {
        gpu.fail("finish_read of a read back that is not ready or failed");
        return;
    }
    // A converted texture's rows arrive in its held format and are packed into its blocks.
    let out_row = o.packed.map_or(o.row, |f| {
        packed::packed_bytes(f, o.row / u64::from(packed::held_bytes(f)))
    });
    if (dst.len() as u64) < out_row * o.rows {
        gpu.fail(&format!(
            "finish_read into {} bytes, fewer than the read back's {}",
            dst.len(),
            out_row * o.rows
        ));
        return;
    }
    let Some(buffer) = &o.buffer else {
        dst[..o.host.len()].copy_from_slice(&o.host);
        return;
    };
    {
        let Ok(data) = buffer.slice(..).get_mapped_range() else {
            gpu.fail("a ready read back could not be mapped");
            return;
        };
        let mut texels = Vec::new();
        for r in 0..o.rows {
            let from = (o.skip + r * o.pitch) as usize;
            let to = (r * out_row) as usize;
            let row = &data[from..from + o.row as usize];
            match o.packed {
                Some(format) => {
                    texels.clear();
                    packed::pack(format, row, &mut texels);
                    dst[to..to + texels.len()].copy_from_slice(&texels);
                }
                None => dst[to..to + row.len()].copy_from_slice(row),
            }
        }
    }
    buffer.unmap();
    buffer.destroy();
}

#[cfg(all(test, windows))]
mod tests {
    use windows::Win32::Graphics::Direct3D12::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT;

    #[test]
    fn placement_alignment_is_direct3d_12s() {
        assert_eq!(
            super::DX12_PLACEMENT_ALIGNMENT,
            u64::from(D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT)
        );
    }
}

/// One write of the queue at a place of a buffer, which takes the bytes it is given in one copy.
struct QueueWrite<'a> {
    queue: &'a wgpu::Queue,
    buffer: &'a wgpu::Buffer,
    at: u64,
}

impl ByteSink for QueueWrite<'_> {
    type Error = core::convert::Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        self.queue.write_buffer(self.buffer, self.at, bytes);
        Ok(bytes.len())
    }
}
