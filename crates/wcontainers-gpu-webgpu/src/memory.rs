//! Bytes in and out: writes in their three modes, renaming, copies, clears and read backs (plan
//! section 4.6).

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use wcontainers_gpu::{
    Backend, CopyBufferTexture, Extent, Format, FormatKind, GpuError, Origin, QueryKind,
    ReadSource, SlowPath, TextureDesc, TextureDimension, TextureRegion, TextureUsage, WriteMode,
};

use crate::fill::{AspectRead, DepthSource};
use crate::{
    Buffer, BufferObj, MAP_PENDING, MAP_READY, QuerySet, Readback, ReadbackObj, Stream, Texture,
    TextureObj, WebGpu, convert, row_bytes,
};

/// The most bytes of pattern a non-zero clear uploads.
const PATTERN_BLOCK: u64 = 64 * 1024;

/// GPU storage a renamed buffer or texture gives up.
pub trait Spare: Clone {
    /// What storage must share to stand in for another.
    type Key: std::hash::Hash + Eq + Copy;
    fn key(&self) -> Self::Key;
    fn destroy(&self);
}

impl Spare for wgpu::Buffer {
    type Key = (u64, u32);
    fn key(&self) -> Self::Key {
        (self.size(), self.usage().bits())
    }
    fn destroy(&self) {
        wgpu::Buffer::destroy(self);
    }
}

impl Spare for wgpu::Texture {
    /// Size, levels, samples, dimension, format and usage; the view formats follow the format.
    type Key = ([u32; 5], wgpu::TextureDimension, wgpu::TextureFormat, u32);
    fn key(&self) -> Self::Key {
        let s = self.size();
        (
            [
                s.width,
                s.height,
                s.depth_or_array_layers,
                self.mip_level_count(),
                self.sample_count(),
            ],
            self.dimension(),
            self.format(),
            self.usage().bits(),
        )
    }
    fn destroy(&self) {
        wgpu::Texture::destroy(self);
    }
}

/// Storage that renamed buffers or textures gave up, reused once the GPU has finished with it.
/// Each buffer or texture counts the storage its renames made; destroying it destroys as much of
/// the pool's storage of its key, so the pool holds no more than its live objects made.
pub struct Pool<T: Spare> {
    /// Storage still used by the work of the serial it was retired in.
    retired: Vec<(u64, T)>,
    /// Storage free to reuse, by key.
    free: HashMap<T::Key, Vec<T>>,
    /// Storage destroyed objects made and the pool still holds, by key: destroyed as it becomes
    /// free.
    owed: HashMap<T::Key, u64>,
}

impl<T: Spare> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            retired: Vec::new(),
            free: HashMap::new(),
            owed: HashMap::new(),
        }
    }
}

impl<T: Spare> Pool<T> {
    /// Frees storage whose work has completed, destroying what is owed; answers how much it
    /// destroyed.
    pub fn reclaim(&mut self, completed: u64) -> u64 {
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

    fn take(&mut self, key: T::Key) -> Option<T> {
        self.free.get_mut(&key).and_then(Vec::pop)
    }

    fn retire(&mut self, serial: u64, old: T) {
        self.retired.push((serial, old));
    }

    /// Owes back `spares` of this key, destroying free ones at once; answers how many it
    /// destroyed.
    fn give_back(&mut self, key: T::Key, spares: u64) -> u64 {
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

/// The uniform arena (plan sections 4.5 and 4.6): one large GPU buffer holding every uniform-only
/// buffer written often, each in a slice of its own. A `Discard` of such a buffer while recorded work
/// uses it moves it to a free slice, so bind groups keep naming the arena and draws give the slice's
/// offset as a dynamic offset.
pub struct Arena {
    /// The storage number buffers living in it carry.
    pub id: u64,
    pub raw: wgpu::Buffer,
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
    pub fn submitted(&mut self) {
        for (size, at) in self.retired.drain(..) {
            self.free.entry(size).or_default().push(at);
        }
    }
}

/// The usages of the arena's buffer, which are also those of every buffer placed in it.
pub fn arena_usage() -> wgpu::BufferUsages {
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
    B: wcontainers_gpu::Backend,
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
    origin: wcontainers_gpu::Origin,
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

fn extent3d(e: Extent) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: e.width,
        height: e.height,
        depth_or_array_layers: e.depth_or_layers,
    }
}

impl WebGpu {
    /// A copy source holding `data` from byte `at`, padded to a multiple of four bytes; also
    /// readable by the edge program.
    fn staging_at(&self, at: u64, data: &[u8]) -> wgpu::Buffer {
        let b = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (at + data.len() as u64).next_multiple_of(4).max(4),
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: true,
        });
        b.slice(..)
            .get_mapped_range_mut()
            .expect("a buffer mapped at creation")
            .slice(at as usize..at as usize + data.len())
            .copy_from_slice(data);
        b.unmap();
        b
    }

    fn staging(&self, data: &[u8]) -> wgpu::Buffer {
        self.staging_at(0, data)
    }

    /// Whether the encoder being recorded names this buffer, so a write from the queue would land
    /// before commands recorded ahead of it.
    fn pending(&self, used_in: u64) -> bool {
        used_in == self.serial && self.encoder.is_some()
    }

    /// Gives a buffer fresh storage; the old storage is kept for the work already recorded with it.
    fn rename(&mut self, b: &BufferObj) {
        let old = b.raw.borrow().clone();
        self.reclaim_spares();
        let fresh = match self.pool.take(old.key()) {
            Some(spare) => {
                self.counters.spares_reused += 1;
                spare
            }
            None => {
                b.spares.set(b.spares.get() + 1);
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: old.size(),
                    usage: b.usage,
                    mapped_at_creation: false,
                })
            }
        };
        b.raw.replace(fresh);
        self.pool.retire(self.serial, old);
        let storage = self.id();
        self.moved(b, Some(storage), 0);
        self.counters.renames += 1;
    }

    /// Gives a texture fresh storage for a `Discard` write, as `rename` does for buffers; views and
    /// bind groups naming the old storage are made again at their next use.
    fn rename_texture(&mut self, t: &TextureObj) {
        let old = t.raw();
        self.reclaim_spares();
        let fresh = match self.texture_pool.take(old.key()) {
            Some(spare) => {
                self.counters.spares_reused += 1;
                spare
            }
            None => {
                t.spares.set(t.spares.get() + 1);
                let twin = convert::srgb_twin(old.format());
                self.device.create_texture(&wgpu::TextureDescriptor {
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
        t.used_in.set(0);
        self.texture_pool.retire(self.serial, old);
        self.bind_epoch += 1;
        self.counters.texture_renames += 1;
    }

    /// Frees renamed storage the GPU has finished with.
    pub(crate) fn reclaim_spares(&mut self) {
        let completed = self.completed.load(Ordering::Acquire);
        self.counters.spares_dropped +=
            self.pool.reclaim(completed) + self.texture_pool.reclaim(completed);
    }

    /// Destroys as much pooled storage as a destroyed buffer's renames made.
    pub(crate) fn give_back_spares(&mut self, b: &BufferObj) {
        let key = b.raw.borrow().key();
        self.counters.spares_dropped += self.pool.give_back(key, b.spares.get());
    }

    /// Destroys as much pooled storage as a destroyed texture's renames made.
    pub(crate) fn give_back_texture_spares(&mut self, t: &TextureObj) {
        let key = t.raw().key();
        self.counters.spares_dropped += self.texture_pool.give_back(key, t.spares.get());
    }

    /// Records that a buffer's bytes now start at `base` of new storage or of the same.
    fn moved(&mut self, b: &BufferObj, storage: Option<u64>, base: u64) {
        if let Some(s) = storage {
            b.storage.set(s);
        }
        b.base.set(base);
        b.generation.set(b.generation.get() + 1);
        b.lead_in.set(0);
        if b.in_sets.get() && (storage.is_some() || b.static_bound.get()) {
            self.bind_epoch += 1;
        }
    }

    /// A free slice of the uniform arena for a buffer of `size` bytes: the arena's buffer, the
    /// slice's offset and the arena's storage number. A full arena is replaced by a larger one;
    /// buffers still in the old one keep it alive until they move.
    pub(crate) fn arena_slice(&mut self, size: u64) -> (wgpu::Buffer, u64, u64) {
        let slice = self.slice_size(size);
        if let Some(a) = &mut self.arena
            && let Some(at) = a.take(slice)
        {
            return (a.raw.clone(), at, a.id);
        }
        let grown = self
            .arena
            .as_ref()
            .map_or(FIRST_ARENA, |a| a.raw.size() * 2)
            .max(slice * 4)
            .min(self.caps.limits.max_buffer_size)
            .max(slice);
        let id = self.id();
        let mut a = Arena::new(&self.device, id, grown);
        let at = a
            .take(slice)
            .expect("a new arena holds the slice it was made for");
        let raw = a.raw.clone();
        self.arena = Some(a);
        (raw, at, id)
    }

    fn slice_size(&self, size: u64) -> u64 {
        let align = u64::from(self.caps.limits.min_uniform_offset_alignment);
        size.next_multiple_of(align).max(align)
    }

    /// Gives up an arena buffer's slice; it is reused after the next submit.
    pub(crate) fn release_slice(&mut self, b: &BufferObj) {
        let slice = self.slice_size(b.desc.size);
        if let Some(a) = &mut self.arena
            && a.id == b.storage.get()
        {
            a.retired.push((slice, b.base.get()));
        }
    }

    /// Moves an arena buffer to a free slice, leaving the old one to the work recorded with it.
    fn move_in_arena(&mut self, b: &BufferObj) {
        self.release_slice(b);
        let (raw, base, storage) = self.arena_slice(b.desc.size);
        let new_storage = (storage != b.storage.get()).then_some(storage);
        if new_storage.is_some() {
            b.raw.replace(raw);
        }
        self.moved(b, new_storage, base);
        self.counters.slices += 1;
    }

    pub(crate) fn write_buffer_impl(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        data: &[u8],
        mode: WriteMode,
    ) {
        if data.is_empty() {
            return;
        }
        buffer.0.wrote();
        if offset + data.len() as u64 > buffer.0.desc.size {
            self.fail("write_buffer past the buffer's end");
            return;
        }
        let b = &buffer.0;
        let recorded = self.pending(b.used_in.get());
        match mode {
            WriteMode::Ordered if recorded => {
                self.staged_write(b, offset, data);
                return;
            }
            WriteMode::Discard if recorded && b.in_arena => self.move_in_arena(b),
            WriteMode::Discard if recorded => self.rename(b),
            _ => {}
        }
        self.write_now(b, offset, data);
    }

    /// Writes ahead of every command recorded so far: from the queue, or, for bytes not in whole words
    /// and for any write behind such a one, by a copy in the lead encoder, since queue writes land
    /// before it.
    fn write_now(&mut self, b: &BufferObj, offset: u64, data: &[u8]) {
        let at = b.base.get() + offset;
        let words = at.is_multiple_of(4) && data.len().is_multiple_of(4);
        if words && !self.lead_pending(b) {
            self.queue.write_buffer(&b.raw.borrow(), at, data);
            return;
        }
        let staging = self.staging_at(at % 4, data);
        self.counters.staged_writes += 1;
        b.lead_in.set(self.serial);
        let dst = b.raw.borrow().clone();
        self.copy_bytes(Stream::Lead, &staging, at % 4, &dst, at, data.len() as u64);
    }

    /// Writes through a staging buffer and a copy recorded in command order, splitting an open pass.
    fn staged_write(&mut self, b: &BufferObj, offset: u64, data: &[u8]) {
        let at = b.base.get() + offset;
        // The staged bytes sit in the same place of their word as their destination.
        let staging = self.staging_at(at % 4, data);
        self.counters.staged_writes += 1;
        b.used_in.set(self.serial);
        let dst = b.raw.borrow().clone();
        let size = data.len() as u64;
        self.split_pass(SlowPath::WriteSplitPass, |w| {
            w.copy_bytes(Stream::Main, &staging, at % 4, &dst, at, size)
        });
    }

    pub(crate) fn write_texture_impl(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        if region.texture.0.blocks.is_some() {
            self.write_blocks(region, extent, data, row_pitch, rows_per_layer, mode);
        } else {
            self.write_texels(region, extent, data, row_pitch, rows_per_layer, mode);
        }
    }

    /// Writes texels in the format the texture holds.
    pub(crate) fn write_texels(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        let t = &region.texture.0;
        if !region.stencil && filled_by_draw(t.desc.format) {
            let Some((staged, sf, how)) = self.depth_staging(t.desc.format, extent) else {
                self.fail("the texture a depth write is drawn from could not be made");
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
            self.write_texels(&at, extent, data, row_pitch, rows_per_layer, mode);
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
                w.depth_copy(
                    (&staged.0.raw(), sf, how),
                    (0, 0, [0, 0]),
                    &dst,
                    extent,
                    stream,
                );
            };
            if self.pending(t.used_in.get()) && mode != WriteMode::NoOverwrite {
                self.split_pass(SlowPath::WriteSplitPass, move |w| draw(w, Stream::Main));
            } else {
                draw(self, Stream::Lead);
            }
            return;
        }
        // A discard of a texture recorded work uses takes fresh storage, unless the open pass draws
        // into it, which must keep drawing into the storage it opened with.
        let attached = self.pass.as_ref().is_some_and(|p| {
            p.attached
                .iter()
                .any(|a| Rc::ptr_eq(&a.0, &region.texture.0))
        });
        if mode == WriteMode::Discard && self.pending(t.used_in.get()) && !attached {
            self.rename_texture(t);
        }
        let raw = t.raw();
        let copy = texel_copy(
            &raw,
            t,
            region.mip,
            region.layer,
            region.origin,
            region.stencil,
        );
        if !self.pending(t.used_in.get()) || mode == WriteMode::NoOverwrite {
            self.queue.write_texture(
                copy,
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_pitch),
                    rows_per_image: Some(rows_per_layer),
                },
                extent3d(extent),
            );
            return;
        }
        // Recorded commands use the texture: the write goes through a staging copy in command order.
        let (row, rows) = row_bytes(region_format(t, region), extent.width, extent.height);
        let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let layers = u64::from(extent.depth_or_layers);
        let mut packed = vec![0u8; (pitch * rows * layers) as usize];
        for l in 0..layers {
            for r in 0..rows {
                let from = (l * u64::from(rows_per_layer) + r) * u64::from(row_pitch);
                let to = (l * rows + r) * pitch;
                let Some(src) = data.get(from as usize..(from + row) as usize) else {
                    self.fail("write_texture with fewer bytes than its extent");
                    return;
                };
                packed[to as usize..(to + row) as usize].copy_from_slice(src);
            }
        }
        let staging = self.staging(&packed);
        self.counters.staged_writes += 1;
        let tex = region.texture.clone();
        let (mip, layer, origin, stencil) =
            (region.mip, region.layer, region.origin, region.stencil);
        self.split_pass(SlowPath::WriteSplitPass, move |w| {
            w.encoder().copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(pitch as u32),
                        rows_per_image: Some(rows as u32),
                    },
                },
                texel_copy(&tex.0.raw(), &tex.0, mip, layer, origin, stencil),
                extent3d(extent),
            );
        });
    }

    pub(crate) fn copy_buffer_impl(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        if self.pass.is_some() {
            self.fail("copy_buffer inside a pass");
            return;
        }
        dst.0.wrote();
        if src_offset + size > src.0.desc.size || dst_offset + size > dst.0.desc.size {
            self.fail("copy_buffer past a buffer's end");
            return;
        }
        let serial = self.serial;
        src.0.used_in.set(serial);
        dst.0.used_in.set(serial);
        let (s, d) = (src.0.raw.borrow().clone(), dst.0.raw.borrow().clone());
        let (src_offset, dst_offset) =
            (src.0.base.get() + src_offset, dst.0.base.get() + dst_offset);
        // One GPU buffer: the same buffer, or two living in the uniform arena.
        if src.0.storage.get() == dst.0.storage.get() {
            self.slow.hit(SlowPath::CopyWithinBuffer);
            // The temporary holds the bytes in the same place of their word as the source.
            let at = src_offset % 4;
            let temp = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (at + size).next_multiple_of(4),
                usage: wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            self.copy_bytes(Stream::Main, &s, src_offset, &temp, at, size);
            self.copy_bytes(Stream::Main, &temp, at, &d, dst_offset, size);
        } else {
            self.copy_bytes(Stream::Main, &s, src_offset, &d, dst_offset, size);
        }
    }

    pub(crate) fn copy_texture_impl(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        if self.pass.is_some() {
            self.fail("copy_texture inside a pass");
            return;
        }
        if src.texture.0.blocks.is_some() || dst.texture.0.blocks.is_some() {
            self.copy_blocks(src, dst, extent);
            return;
        }
        let (s, d) = (&src.texture.0, &dst.texture.0);
        s.used_in.set(self.serial);
        d.used_in.set(self.serial);
        let (sf, df) = (s.held, d.held);
        let covers = |t: &crate::TextureObj, r: &TextureRegion<'_, Self>| {
            let (w, h) = (
                (t.desc.width >> r.mip).max(1),
                (t.desc.height >> r.mip).max(1),
            );
            r.origin.x == 0 && r.origin.y == 0 && extent.width == w && extent.height == h
        };
        let whole = covers(d, dst) && covers(s, src);
        // A combined format is copied with both aspects.
        let combined =
            sf.kind() == FormatKind::DepthStencil || df.kind() == FormatKind::DepthStencil;
        if combined && sf != df {
            self.fail("a copy between a combined depth-stencil format and another format");
            return;
        }
        // WebGPU copies depth and stencil only between equal formats and only whole; the rest
        // are draws writing each aspect.
        if df.is_depth_stencil() && (sf != df || !whole) {
            let at = (src.mip, src.layer, [src.origin.x, src.origin.y]);
            if df.kind() != FormatKind::Stencil {
                let Some(how) = DepthSource::of(sf, df) else {
                    self.fail("a copy into depth from a format outside its typeless group");
                    return;
                };
                self.depth_copy((&s.raw(), sf, how), at, dst, extent, Stream::Main);
            }
            if df.kind() != FormatKind::Depth {
                if sf != df {
                    self.fail("a copy into stencil from another format");
                    return;
                }
                self.stencil_copy(
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
        if sf.linear() == df.linear() {
            self.encoder
                .get_or_insert_with(|| {
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
                })
                .copy_texture_to_texture(from, to, extent3d(extent));
        } else if sf.block().2 == df.block().2 && !df.is_depth_stencil() {
            // Typeless twins, or a block-compressed format and an uncompressed one whose texels are
            // its blocks: the destination's extent covers the same blocks.
            self.slow.hit(SlowPath::CopyThroughBuffer);
            let ((sbw, sbh, _), (dbw, dbh, _)) = (sf.block(), df.block());
            let into = Extent {
                width: extent.width.div_ceil(sbw) * dbw,
                height: extent.height.div_ceil(sbh) * dbh,
                depth_or_layers: extent.depth_or_layers,
            };
            let (row, rows) = row_bytes(sf, extent.width, extent.height);
            let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
            let temp = self.device.create_buffer(&wgpu::BufferDescriptor {
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
            let e = self.encoder.get_or_insert_with(|| {
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
            });
            let info = wgpu::TexelCopyBufferInfo {
                buffer: &temp,
                layout,
            };
            e.copy_texture_to_buffer(from, info, extent3d(extent));
            e.copy_buffer_to_texture(info, to, extent3d(into));
        } else {
            self.fail("a copy between formats of different block sizes");
        }
    }

    /// Writes the depth of `dst` from `src` by draws, after checking what the draw needs.
    fn depth_copy(
        &mut self,
        src: (&wgpu::Texture, Format, DepthSource),
        src_at: (u32, u32, [u32; 2]),
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
        stream: Stream,
    ) {
        let d = &dst.texture.0;
        if src.0.sample_count() > 1 || d.desc.samples > 1 {
            self.fail("multisampled depth copies by a draw are not built yet");
            return;
        }
        if !src.0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || !d
                .raw()
                .usage()
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            self.fail(
                "a depth copy by a draw needs the source sampled and the destination rendered",
            );
            return;
        }
        self.depth_by_draw(
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
        &mut self,
        src: (&wgpu::Texture, wgpu::TextureAspect),
        src_at: (u32, u32, [u32; 2]),
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let d = &dst.texture.0;
        if src.0.sample_count() > 1 || d.desc.samples > 1 {
            self.fail("multisampled stencil copies by a draw are not built yet");
            return;
        }
        if !src.0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || !d
                .raw()
                .usage()
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            self.fail(
                "a stencil copy by a draw needs the source sampled and the destination rendered",
            );
            return;
        }
        self.stencil_by_draw(
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
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<Option<Texture>, ()> {
        let t = &region.texture.0;
        let format = t.desc.format;
        let stencil = region.stencil || format.kind() == FormatKind::Stencil;
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
            self.fail("a depth or stencil read by a draw needs a single-sampled bound source");
            return Err(());
        }
        let Ok(drawn) = self.create_texture(&TextureDesc {
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
            self.fail("the texture a depth or stencil read is drawn into could not be made");
            return Err(());
        };
        t.used_in.set(self.serial);
        let raw = t.raw();
        self.aspect_by_draw(
            (&raw, read),
            (region.mip, region.layer, [region.origin.x, region.origin.y]),
            &drawn.0.raw(),
            [extent.width, extent.height],
            extent.depth_or_layers,
        );
        Ok(Some(drawn))
    }

    /// A single-sampled texture holding a buffer's texels for a depth draw into `format`, which
    /// WebGPU cannot fill from a buffer: `r32float` for 32-bit float depth, `r32uint` for 24-bit.
    fn depth_staging(
        &mut self,
        format: Format,
        extent: Extent,
    ) -> Option<(Texture, Format, DepthSource)> {
        let (staged, how) = match format {
            Format::Depth24Plus | Format::Depth24PlusStencil8 => {
                (Format::R32Uint, DepthSource::Unorm24)
            }
            _ => (Format::R32Float, DepthSource::Float),
        };
        let t = self
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
        &mut self,
        buffer: &CopyBufferTexture<'_, Self>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        to_texture: bool,
    ) {
        if self.pass.is_some() {
            self.fail("buffer and texture copies inside a pass");
            return;
        }
        if !to_texture {
            buffer.buffer.0.wrote();
        }
        if region.texture.0.blocks.is_some() {
            self.copy_buffer_blocks(buffer, region, extent, to_texture);
            return;
        }
        let t = &region.texture.0;
        let at_origin = |texture| TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let stencil = region.stencil || t.desc.format.kind() == FormatKind::Stencil;
        let whole = region.origin.x == 0
            && region.origin.y == 0
            && extent.width == (t.desc.width >> region.mip).max(1)
            && extent.height == (t.desc.height >> region.mip).max(1);
        if !to_texture {
            match self.aspect_as_colour(region, extent) {
                Err(()) => return,
                Ok(Some(drawn)) => {
                    self.copy_buffer_texture(buffer, &at_origin(&drawn), extent, false);
                    return;
                }
                Ok(None) => {}
            }
        } else if stencil && !whole {
            let Ok(staged) = self.create_texture(&TextureDesc {
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
                self.fail("the texture a stencil copy is drawn from could not be made");
                return;
            };
            self.copy_buffer_texture(buffer, &at_origin(&staged), extent, true);
            t.used_in.set(self.serial);
            let raw = staged.0.raw();
            self.stencil_copy(
                (&raw, wgpu::TextureAspect::All),
                (0, 0, [0, 0]),
                region,
                extent,
            );
            return;
        } else if t.desc.format == Format::Depth16Unorm && !whole {
            self.fail("part of a 16-bit depth level written from a buffer");
            return;
        }
        if to_texture && !region.stencil && filled_by_draw(t.desc.format) {
            let Some((staged, sf, how)) = self.depth_staging(t.desc.format, extent) else {
                self.fail("the texture a depth copy is drawn from could not be made");
                return;
            };
            self.copy_buffer_texture(buffer, &at_origin(&staged), extent, true);
            t.used_in.set(self.serial);
            self.depth_copy(
                (&staged.0.raw(), sf, how),
                (0, 0, [0, 0]),
                region,
                extent,
                Stream::Main,
            );
            return;
        }
        t.used_in.set(self.serial);
        buffer.buffer.0.used_in.set(self.serial);
        let raw = buffer.buffer.0.raw.borrow().clone();
        let offset = buffer.buffer.0.base.get() + buffer.offset;
        let tex = region.texture.clone();
        let (mip, layer, origin, stencil) =
            (region.mip, region.layer, region.origin, region.stencil);
        let copy = |w: &mut WebGpu, staged: &wgpu::Buffer, offset: u64, pitch: u64, rows: u64| {
            let info = wgpu::TexelCopyBufferInfo {
                buffer: staged,
                layout: wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(pitch as u32),
                    rows_per_image: Some(rows as u32),
                },
            };
            let tex_raw = tex.0.raw();
            let to = texel_copy(&tex_raw, &tex.0, mip, layer, origin, stencil);
            if to_texture {
                w.encoder()
                    .copy_buffer_to_texture(info, to, extent3d(extent));
            } else {
                w.encoder()
                    .copy_texture_to_buffer(to, info, extent3d(extent));
            }
        };
        let pitch = u64::from(buffer.row_pitch);
        if pitch.is_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            && offset.is_multiple_of(4)
        {
            copy(self, &raw, offset, pitch, u64::from(buffer.rows_per_layer));
            return;
        }
        // The rows go through a buffer at a pitch WebGPU takes, one buffer copy per row.
        self.slow.hit(SlowPath::RowsRepacked);
        let (row, rows) = row_bytes(region_format(t, region), extent.width, extent.height);
        let api_pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let layers = u64::from(extent.depth_or_layers);
        let temp = self.device.create_buffer(&wgpu::BufferDescriptor {
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
            copy(self, &temp, 0, api_pitch, rows);
        }
        for l in 0..layers {
            for r in 0..rows {
                if to_texture {
                    self.copy_bytes(Stream::Main, &raw, theirs(l, r), &temp, ours(l, r), row);
                } else {
                    self.copy_bytes(Stream::Main, &temp, ours(l, r), &raw, theirs(l, r), row);
                }
            }
        }
        if to_texture {
            copy(self, &temp, 0, api_pitch, rows);
        }
    }

    pub(crate) fn clear_buffer_impl(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        size: u64,
        pattern: &[u8],
    ) {
        if self.pass.is_some() {
            self.fail("clear_buffer inside a pass");
            return;
        }
        buffer.0.wrote();
        if pattern.is_empty() || !size.is_multiple_of(pattern.len() as u64) {
            self.fail("clear_buffer with a range that is not a whole number of patterns");
            return;
        }
        if offset + size > buffer.0.desc.size {
            self.fail("clear_buffer past the buffer's end");
            return;
        }
        buffer.0.used_in.set(self.serial);
        let raw = buffer.0.raw.borrow().clone();
        let offset = buffer.0.base.get() + offset;
        let zero = pattern.iter().all(|b| *b == 0);
        if zero && offset.is_multiple_of(4) && size.is_multiple_of(4) {
            self.encoder().clear_buffer(&raw, offset, Some(size));
            return;
        }
        if !zero {
            self.slow.hit(SlowPath::PatternClearCopied);
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
        let at = offset % 4;
        let staging = self.staging_at(at, &block);
        let mut done = 0;
        while done < size {
            let n = block_size.min(size - done);
            self.copy_bytes(Stream::Main, &staging, at, &raw, offset + done, n);
            done += n;
        }
    }

    fn map_target(&self, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size.max(4),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn readback(
        &mut self,
        buffer: wgpu::Buffer,
        skip: u64,
        row: u64,
        pitch: u64,
        rows: u64,
    ) -> Readback {
        let rb = Readback(Rc::new(ReadbackObj {
            buffer: Some(buffer),
            host: Vec::new(),
            state: Arc::new(AtomicU8::new(MAP_PENDING)),
            serial: self.serial,
            skip,
            row,
            pitch,
            rows,
        }));
        self.to_map.push(rb.clone());
        rb
    }

    /// A read back the host answers at once.
    pub(crate) fn host_readback(&mut self, bytes: Vec<u8>) -> Readback {
        let len = bytes.len() as u64;
        Readback(Rc::new(ReadbackObj {
            buffer: None,
            host: bytes,
            state: Arc::new(AtomicU8::new(MAP_READY)),
            serial: self.serial,
            skip: 0,
            row: len,
            pitch: len,
            rows: 1,
        }))
    }

    pub(crate) fn read_back_impl(
        &mut self,
        src: &ReadSource<'_, Self>,
    ) -> Result<Readback, GpuError> {
        if self.pass.is_some() {
            return Err(GpuError::Invalid("read backs are recorded outside a pass"));
        }
        match src {
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                let start = offset & !3;
                let end = (offset + size).next_multiple_of(4);
                let target = self.map_target(end - start);
                buffer.0.used_in.set(self.serial);
                let raw = buffer.0.raw.borrow().clone();
                self.encoder().copy_buffer_to_buffer(
                    &raw,
                    buffer.0.base.get() + start,
                    &target,
                    0,
                    end - start,
                );
                Ok(self.readback(target, offset - start, *size, *size, 1))
            }
            ReadSource::Texture { region, extent } => {
                if region.texture.0.blocks.is_some() {
                    return self.read_blocks(region, *extent).ok_or(GpuError::Invalid(
                        "a read back outside the texture's blocks",
                    ));
                }
                let drawn = match self.aspect_as_colour(region, *extent) {
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
                let t = &region.texture.0;
                t.used_in.set(self.serial);
                let (row, rows) = row_bytes(region_format(t, region), extent.width, extent.height);
                let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
                let layers = u64::from(extent.depth_or_layers);
                let target = self.map_target(pitch * rows * layers);
                let raw = t.raw();
                let tex = texel_copy(
                    &raw,
                    t,
                    region.mip,
                    region.layer,
                    region.origin,
                    region.stencil,
                );
                self.encoder().copy_texture_to_buffer(
                    tex,
                    wgpu::TexelCopyBufferInfo {
                        buffer: &target,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(pitch as u32),
                            rows_per_image: Some(rows as u32),
                        },
                    },
                    extent3d(*extent),
                );
                Ok(self.readback(target, 0, row, pitch, rows * layers))
            }
        }
    }

    /// Resolves queries into a buffer of its own, at offset zero as WebGPU's alignment needs.
    fn resolved(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<wgpu::Buffer, GpuError> {
        if self.pass.is_some() {
            return Err(GpuError::Invalid("query results are taken outside a pass"));
        }
        if set.0.kind != QueryKind::OcclusionAny || first + count > set.0.count {
            return Err(GpuError::Invalid("queries outside the set"));
        }
        let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(count.max(1)) * 8,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let raw = set.0.raw.clone();
        self.encoder()
            .resolve_query_set(&raw, first..first + count, &resolve, 0);
        Ok(resolve)
    }

    pub(crate) fn read_queries_impl(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Readback, GpuError> {
        let resolve = self.resolved(set, first, count)?;
        let size = u64::from(count) * 8;
        let target = self.map_target(size);
        self.encoder()
            .copy_buffer_to_buffer(&resolve, 0, &target, 0, size);
        Ok(self.readback(target, 0, size, size, 1))
    }

    pub(crate) fn resolve_queries_impl(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
        dst: &Buffer,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        dst.0.wrote();
        if !dst_offset.is_multiple_of(8) {
            return Err(GpuError::Invalid(
                "query results go to multiples of 8 bytes",
            ));
        }
        let resolve = self.resolved(set, first, count)?;
        dst.0.used_in.set(self.serial);
        let raw = dst.0.raw.borrow().clone();
        let at = dst.0.base.get() + dst_offset;
        self.encoder()
            .copy_buffer_to_buffer(&resolve, 0, &raw, at, u64::from(count) * 8);
        Ok(())
    }

    pub(crate) fn finish_read_impl(&mut self, readback: Readback, dst: &mut [u8]) {
        let o = &readback.0;
        if o.state.load(Ordering::Acquire) != MAP_READY {
            self.fail("finish_read of a read back that is not ready or failed");
            return;
        }
        if (dst.len() as u64) < o.row * o.rows {
            self.fail(&format!(
                "finish_read into {} bytes, fewer than the read back's {}",
                dst.len(),
                o.row * o.rows
            ));
            return;
        }
        let Some(buffer) = &o.buffer else {
            dst[..o.host.len()].copy_from_slice(&o.host);
            return;
        };
        {
            let Ok(data) = buffer.slice(..).get_mapped_range() else {
                self.fail("a ready read back could not be mapped");
                return;
            };
            for r in 0..o.rows {
                let from = (o.skip + r * o.pitch) as usize;
                let to = (r * o.row) as usize;
                dst[to..to + o.row as usize].copy_from_slice(&data[from..from + o.row as usize]);
            }
        }
        buffer.unmap();
        buffer.destroy();
    }
}
