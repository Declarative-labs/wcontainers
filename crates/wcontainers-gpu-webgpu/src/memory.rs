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
    Buffer, BufferObj, MAP_PENDING, MAP_READY, QuerySet, Readback, ReadbackObj, WebGpu, convert,
    row_bytes,
};

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
    /// A copy source holding `data`, padded to a multiple of four bytes.
    fn staging(&self, data: &[u8]) -> wgpu::Buffer {
        let b = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (data.len() as u64).next_multiple_of(4).max(4),
            usage: wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        b.slice(..)
            .get_mapped_range_mut()
            .expect("a buffer mapped at creation")
            .slice(..data.len())
            .copy_from_slice(data);
        b.unmap();
        b
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
        b.generation.set(b.generation.get() + 1);
        self.counters.renames += 1;
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
        if !offset.is_multiple_of(4) || !data.len().is_multiple_of(4) {
            self.fail("buffer writes of unaligned offsets or lengths (UnalignedBufferEdge) are not built yet");
            return;
        }
        if offset + data.len() as u64 > buffer.0.desc.size {
            self.fail("write_buffer past the buffer's end");
            return;
        }
        let b = &buffer.0;
        match mode {
            _ if !self.pending(b.used_in.get()) => {}
            WriteMode::NoOverwrite => {}
            WriteMode::Discard => self.rename(b),
            WriteMode::Ordered => {
                let staging = self.staging(data);
                self.counters.staged_writes += 1;
                let dst = b.raw.borrow().clone();
                let size = data.len() as u64;
                self.split_pass(|e| e.copy_buffer_to_buffer(&staging, 0, &dst, offset, size));
                return;
            }
        }
        self.queue.write_buffer(&b.raw.borrow(), offset, data);
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
        self.split_pass(move |e| {
            e.copy_buffer_to_texture(
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
        if [src_offset, dst_offset, size]
            .iter()
            .any(|v| !v.is_multiple_of(4))
        {
            self.fail("buffer copies of unaligned ranges (UnalignedBufferEdge) are not built yet");
            return;
        }
        let serial = self.serial;
        src.0.used_in.set(serial);
        dst.0.used_in.set(serial);
        let (s, d) = (src.0.raw.borrow().clone(), dst.0.raw.borrow().clone());
        if Rc::ptr_eq(&src.0, &dst.0) {
            self.slow.hit(SlowPath::CopyWithinBuffer);
            let temp = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let e = self.encoder();
            e.copy_buffer_to_buffer(&s, src_offset, &temp, 0, size);
            e.copy_buffer_to_buffer(&temp, 0, &d, dst_offset, size);
        } else {
            self.encoder()
                .copy_buffer_to_buffer(&s, src_offset, &d, dst_offset, size);
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
        if !buffer
            .row_pitch
            .is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        {
            self.fail("buffer rows at pitches other than multiples of 256 are not built yet");
            return;
        }
        let t = &region.texture.0;
        t.used_in.set(self.serial);
        buffer.buffer.0.used_in.set(self.serial);
        let raw = buffer.buffer.0.raw.borrow().clone();
        let info = wgpu::TexelCopyBufferInfo {
            buffer: &raw,
            layout: wgpu::TexelCopyBufferLayout {
                offset: buffer.offset,
                bytes_per_row: Some(buffer.row_pitch),
                rows_per_image: Some(buffer.rows_per_layer),
            },
        };
        let tex = texel_copy(t, region.mip, region.layer, region.origin, region.stencil);
        let e = self.encoder.get_or_insert_with(|| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        });
        if to_texture {
            e.copy_buffer_to_texture(info, tex, extent3d(extent));
        } else {
            e.copy_texture_to_buffer(tex, info, extent3d(extent));
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
        if !offset.is_multiple_of(4) || !size.is_multiple_of(4) {
            self.fail("buffer clears of unaligned ranges (UnalignedBufferEdge) are not built yet");
            return;
        }
        if pattern.is_empty() || !size.is_multiple_of(pattern.len() as u64) {
            self.fail("clear_buffer with a range that is not a whole number of patterns");
            return;
        }
        buffer.0.used_in.set(self.serial);
        let raw = buffer.0.raw.borrow().clone();
        if pattern.iter().all(|b| *b == 0) {
            self.encoder().clear_buffer(&raw, offset, Some(size));
        } else {
            let filled: Vec<u8> = pattern
                .iter()
                .copied()
                .cycle()
                .take(size as usize)
                .collect();
            let staging = self.staging(&filled);
            self.encoder()
                .copy_buffer_to_buffer(&staging, 0, &raw, offset, size);
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
                let raw = buffer.0.raw.borrow().clone();
                self.encoder()
                    .copy_buffer_to_buffer(&raw, start, &target, 0, end - start);
                Ok(self.readback(target, offset - start, *size, *size, 1))
            }
            ReadSource::Texture { region, extent } => {
                let t = &region.texture.0;
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
        self.encoder()
            .copy_buffer_to_buffer(&resolve, 0, &raw, dst_offset, u64::from(count) * 8);
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
