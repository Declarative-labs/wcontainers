//! Bytes in and out: writes in their three modes, renaming, copies, clears and read backs (plan
//! section 4.6).

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use wcontainers_gpu::{
    CopyBufferTexture, Extent, Format, GpuError, QueryKind, ReadSource, SlowPath, TextureDimension,
    TextureRegion, WriteMode,
};

use crate::{
    Buffer, BufferObj, MAP_PENDING, MAP_READY, QuerySet, Readback, ReadbackObj, Stream, WebGpu,
    convert, row_bytes,
};

/// The most bytes of pattern a non-zero clear uploads.
const PATTERN_BLOCK: u64 = 64 * 1024;

/// Storage that renamed buffers gave up, reused once the GPU has finished with it.
#[derive(Default)]
pub struct BufferPool {
    /// Storage still used by the work of the serial it was retired in.
    retired: Vec<(u64, wgpu::Buffer)>,
    /// Storage free to reuse, by size and usage.
    free: HashMap<(u64, u32), Vec<wgpu::Buffer>>,
}

impl BufferPool {
    fn take(
        &mut self,
        size: u64,
        usage: wgpu::BufferUsages,
        completed: u64,
    ) -> Option<wgpu::Buffer> {
        let free = &mut self.free;
        self.retired.retain(|(serial, b)| {
            if *serial > completed {
                return true;
            }
            free.entry((b.size(), b.usage().bits()))
                .or_default()
                .push(b.clone());
            false
        });
        free.get_mut(&(size, usage.bits())).and_then(Vec::pop)
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

/// The format a region's texels are in: the stencil aspect is 8-bit, depth its own.
fn region_format<B>(texture: &crate::TextureObj, region: &TextureRegion<'_, B>) -> Format
where
    B: wcontainers_gpu::Backend,
{
    if region.stencil {
        Format::Stencil8
    } else {
        texture.desc.format
    }
}

fn texel_copy<'a>(
    texture: &'a crate::TextureObj,
    mip: u32,
    layer: u32,
    origin: wcontainers_gpu::Origin,
    stencil: bool,
) -> wgpu::TexelCopyTextureInfo<'a> {
    wgpu::TexelCopyTextureInfo {
        texture: &texture.raw,
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
        let size = b.raw.borrow().size();
        let completed = self.completed.load(Ordering::Acquire);
        let fresh = self.pool.take(size, b.usage, completed).unwrap_or_else(|| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: b.usage,
                mapped_at_creation: false,
            })
        });
        let old = b.raw.replace(fresh);
        self.pool.retired.push((self.serial, old));
        let storage = self.id();
        self.moved(b, Some(storage), 0);
        self.counters.renames += 1;
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
        self.split_pass(|w| w.copy_bytes(Stream::Main, &staging, at % 4, &dst, at, size));
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
        let t = &region.texture.0;
        let copy = texel_copy(t, region.mip, region.layer, region.origin, region.stencil);
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
        self.split_pass(move |w| {
            w.encoder().copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(pitch as u32),
                        rows_per_image: Some(rows as u32),
                    },
                },
                texel_copy(&tex.0, mip, layer, origin, stencil),
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
        let (s, d) = (&src.texture.0, &dst.texture.0);
        s.used_in.set(self.serial);
        d.used_in.set(self.serial);
        let (sf, df) = (region_format(s, src), region_format(d, dst));
        let from = texel_copy(s, src.mip, src.layer, src.origin, src.stencil);
        let to = texel_copy(d, dst.mip, dst.layer, dst.origin, dst.stencil);
        if sf.linear() == df.linear() {
            self.encoder
                .get_or_insert_with(|| {
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
                })
                .copy_texture_to_texture(from, to, extent3d(extent));
        } else if sf.block() == df.block() && !df.is_depth_stencil() {
            self.slow.hit(SlowPath::CopyThroughBuffer);
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
            e.copy_buffer_to_texture(info, to, extent3d(extent));
        } else {
            self.fail("copies into depth by a draw (DepthWrittenByDraw) are not built yet");
        }
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
        let t = &region.texture.0;
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
            let to = texel_copy(&tex.0, mip, layer, origin, stencil);
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
            buffer,
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
                let t = &region.texture.0;
                t.used_in.set(self.serial);
                let (row, rows) = row_bytes(region_format(t, region), extent.width, extent.height);
                let pitch = row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
                let layers = u64::from(extent.depth_or_layers);
                let target = self.map_target(pitch * rows * layers);
                let tex = texel_copy(t, region.mip, region.layer, region.origin, region.stencil);
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
            self.fail("finish_read into fewer bytes than the read back holds");
            return;
        }
        {
            let Ok(data) = o.buffer.slice(..).get_mapped_range() else {
                self.fail("a ready read back could not be mapped");
                return;
            };
            for r in 0..o.rows {
                let from = (o.skip + r * o.pitch) as usize;
                let to = (r * o.row) as usize;
                dst[to..to + o.row as usize].copy_from_slice(&data[from..from + o.row as usize]);
            }
        }
        o.buffer.unmap();
        o.buffer.destroy();
    }
}
