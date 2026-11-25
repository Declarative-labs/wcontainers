//! Storage textures read and written through a buffer of their texels
//! (`VariantKey::texels_through_buffer`), where the back end has no atomics on storage textures
//! (`AtomicsThroughBuffer`), cannot store into the view's format (`StorageFormatThroughBuffer`), or
//! cannot load and store it in one program (`StorageReadWriteThroughBuffer`):
//! the view's texels are copied into a buffer before each draw or
//! dispatch, outside a pass, and back after it; the program decodes and encodes the view's format.

use super::*;
use crate::desc::ViewDim;
use wcontainers_gpu::{BufferUsage, CopyBufferTexture, TextureDesc as GpuTexture, TextureRegion};

const COMPARED_FORMAT_UNCLAIMED: &str =
    "a comparison through a view of a format Direct3D does not compare";
const COMPARED_VIEW_INVALID: &str = "a comparison through a multisampled or 3D view";

/// A storage view of a stage read through a buffer, and the copies around its uses.
pub(crate) struct Through<B: Backend> {
    register: u32,
    texture: Rc<Texture<B>>,
    mip: u32,
    /// First layer, or first depth slice of a 3D view.
    first: u32,
    extent: Extent,
    three_d: bool,
    buffer: Kept<B, B::Buffer>,
    row_pitch: u32,
    /// `HelperValue::TexelLayout`: row pitch in texels, rows per layer, width, height, layers.
    layout: [u32; 5],
    why: SlowPath,
}

impl<B: Backend> Through<B> {
    pub(super) fn register(&self) -> u32 {
        self.register
    }

    /// The binding of its buffer.
    pub(super) fn range(&self) -> BufferRange<&B::Buffer> {
        BufferRange {
            buffer: &self.buffer.handle,
            offset: 0,
            size: u64::from(self.row_pitch)
                * u64::from(self.extent.height)
                * u64::from(self.extent.depth_or_layers),
        }
    }
}

impl<B: Backend> Device<B> {
    /// Why `stage`'s shader reads its storage view at `register` through a buffer, where it does.
    pub(super) fn reads_through(&self, stage: usize, register: u32) -> Option<SlowPath> {
        let s = self.stage_shader(stage).as_ref()?;
        let t = s
            .typed
            .iter()
            .find(|t| t.storage && !t.buffer && t.register == register)?;
        t.through.or_else(|| self.format_through(stage, t))
    }

    /// The storage texture slots of `s` read through a buffer for the format of the view bound
    /// at `stage` ([`Self::format_through`]).
    pub(super) fn stored_through_slots(&self, stage: usize, s: &Shader<B>) -> u64 {
        s.typed
            .iter()
            .filter(|t| t.storage && !t.buffer && t.through.is_none() && t.register < u64::BITS)
            .filter(|t| self.format_through(stage, t).is_some())
            .fold(0, |m, t| m | 1 << t.register)
    }

    /// Why the storage view bound at `t`'s slot of `stage` is read through a buffer of its texels:
    /// the back end cannot store into its format (`StorageFormatThroughBuffer`), or the program
    /// loads from it and the back end does not load and store the format in one program
    /// (`StorageReadWriteThroughBuffer`).
    fn format_through(&self, stage: usize, t: &crate::shader::TypedView) -> Option<SlowPath> {
        let Some(ShaderView::Texture(v)) = self.ctx.uavs[stage]
            .get(t.register as usize)
            .and_then(Option::as_deref)
        else {
            return None;
        };
        if self.stored_through(v.format) || v.decoded.is_some() {
            Some(SlowPath::StorageFormatThroughBuffer)
        } else if t.loaded && self.read_written_through(v.format) {
            Some(SlowPath::StorageReadWriteThroughBuffer)
        } else {
            None
        }
    }

    /// A buffer holding the texels of storage view `v`, bound at `register` of a stage.
    pub(super) fn through_view(
        &mut self,
        register: u32,
        v: &View<B>,
        why: SlowPath,
    ) -> Result<Through<B>> {
        let (bw, bh, bytes) = v.format.block();
        if (bw, bh) != (1, 1) || v.texture.desc.sample_count > 1 {
            return Err(Error::InvalidArg(
                "a storage view of a block-compressed or multisampled texture",
            ));
        }
        let mip = v.desc.first_mip;
        let three_d = v.desc.dimension == ViewDim::Texture3D;
        let e = v.texture.mip_extent(mip);
        let (width, height) = (e.width, e.height);
        let layers = v.desc.layer_count.max(1);
        let row_pitch = (width * bytes).next_multiple_of(256);
        let size = u64::from(row_pitch) * u64::from(height) * u64::from(layers);
        let buffer = self.backend.create_buffer(&wcontainers_gpu::BufferDesc {
            size,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: wcontainers_gpu::MapIntent::Rarely,
        });
        let buffer = self.gpu(buffer)?;
        let id = self.id();
        Ok(Through {
            register,
            texture: v.texture.clone(),
            mip,
            first: v.desc.first_layer,
            extent: Extent {
                width,
                height,
                depth_or_layers: layers,
            },
            three_d,
            buffer: Kept::new(buffer, id, Object::Buffer, &self.graveyard),
            row_pitch,
            layout: [row_pitch / bytes, height, width, height, layers],
            why,
        })
    }

    /// The texel layout of the view `stage` reads through a buffer at `register`.
    pub(super) fn through_layout(&self, stage: usize, register: u32) -> [u32; 5] {
        self.ctx.through[stage]
            .iter()
            .find(|t| t.register == register)
            .map_or([0; 5], |t| t.layout)
    }

    /// Before a draw or dispatch reading views of `stage` through buffers: their texels copied in,
    /// outside a pass. Answers whether there were any.
    pub(super) fn through_in(&mut self, stage: usize) -> bool {
        if self.ctx.through[stage].is_empty() {
            return false;
        }
        self.end_pass();
        for t in &self.ctx.through[stage] {
            self.backend.slow_paths().hit(t.why);
            let texture = t.texture.gpu.handle.clone();
            self.backend.copy_texture_to_buffer(
                &region(t, &texture),
                &CopyBufferTexture {
                    buffer: &t.buffer.handle,
                    offset: 0,
                    row_pitch: t.row_pitch,
                    rows_per_layer: t.extent.height,
                },
                t.extent,
            );
        }
        true
    }

    /// After it: the texels copied back, outside a pass.
    pub(super) fn through_out(&mut self, stage: usize) {
        self.end_pass();
        for t in &self.ctx.through[stage] {
            let texture = t.texture.gpu.handle.clone();
            self.backend.copy_buffer_to_texture(
                &CopyBufferTexture {
                    buffer: &t.buffer.handle,
                    offset: 0,
                    row_pitch: t.row_pitch,
                    rows_per_layer: t.extent.height,
                },
                &region(t, &texture),
                t.extent,
            );
        }
    }
}

fn region<'a, B: Backend>(t: &Through<B>, texture: &'a B::Texture) -> TextureRegion<'a, B> {
    TextureRegion {
        texture,
        mip: t.mip,
        layer: if t.three_d { 0 } else { t.first },
        origin: Origin {
            x: 0,
            y: 0,
            z: if t.three_d { t.first } else { 0 },
        },
        stencil: false,
    }
}

impl<B: Backend> Device<B> {
    /// Copies the levels and layers view `v` sees between its texture and its twin: into the twin
    /// before a use, out of it after a write.
    pub(super) fn copy_twin(&mut self, v: &View<B>, into: bool) {
        if let Some((buffer, offset)) = &v.buffer {
            let extent = v.texture.mip_extent(0);
            let (_, _, bytes) = v.format.block();
            let handle = buffer.handle();
            let copy = CopyBufferTexture {
                buffer: &handle,
                offset: *offset,
                row_pitch: extent.width * bytes,
                rows_per_layer: 1,
            };
            let texture = TextureRegion {
                texture: &v.texture.gpu.handle,
                mip: 0,
                layer: 0,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            };
            if into {
                self.backend.copy_buffer_to_texture(&copy, &texture, extent);
            } else {
                self.backend.copy_texture_to_buffer(&texture, &copy, extent);
            }
            return;
        }
        let Some(twin) = &v.twin else {
            return;
        };
        let t = &v.texture;
        if v.window {
            let mut extent = t.mip_extent(v.desc.first_mip);
            extent.depth_or_layers = v.desc.layer_count;
            let at = |texture, mip, z| TextureRegion {
                texture,
                mip,
                layer: 0,
                origin: Origin { x: 0, y: 0, z },
                stencil: false,
            };
            let stored = at(&t.gpu.handle, v.desc.first_mip, v.desc.first_layer);
            let other = at(&twin.handle, 0, 0);
            if into {
                self.backend.copy_texture(&stored, &other, extent);
            } else {
                self.backend.copy_texture(&other, &stored, extent);
            }
            return;
        }
        let three_d = t.desc.dimension == crate::desc::TexDim::D3;
        for mip in v.desc.first_mip..v.desc.first_mip + v.desc.mip_count {
            let mut extent = t.mip_extent(mip);
            let layer = if three_d {
                0
            } else {
                extent.depth_or_layers = v.desc.layer_count;
                v.desc.first_layer
            };
            let at = |texture| TextureRegion {
                texture,
                mip,
                layer,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            };
            let (stored, other) = (at(&t.gpu.handle), at(&twin.handle));
            if into {
                self.backend.copy_texture(&stored, &other, extent);
            } else {
                self.backend.copy_texture(&other, &stored, extent);
            }
        }
    }

    /// The handle of the depth texture view `v` is compared through (`View::compared`), made at
    /// the first comparison: Direct3D compares `R32_FLOAT` and `R16_UNORM` views, which WebGPU
    /// compares only as depth.
    pub(super) fn compared_view(&mut self, v: &View<B>) -> Result<B::TextureView> {
        if let Some(c) = &*v.compared.borrow() {
            return Ok(c.view.handle.clone());
        }
        let depth = match v.values() {
            Format::R32Float => Format::Depth32Float,
            Format::R16Unorm => Format::Depth16Unorm,
            _ => {
                return not_implemented(COMPARED_FORMAT_UNCLAIMED);
            }
        };
        let dimension = match v.desc.dimension {
            ViewDim::Texture1D | ViewDim::Texture2D => ViewDimension::D2,
            ViewDim::Texture1DArray | ViewDim::Texture2DArray => ViewDimension::D2Array,
            ViewDim::TextureCube => ViewDimension::Cube,
            ViewDim::TextureCubeArray => ViewDimension::CubeArray,
            _ => {
                return Err(Error::InvalidArg(COMPARED_VIEW_INVALID));
            }
        };
        let texture = self.backend.create_texture(&GpuTexture {
            format: depth,
            usage: TextureUsage::SAMPLE | TextureUsage::RENDER | TextureUsage::COPY_DST,
            reinterpret: false,
            ..v.texture.gpu_desc
        });
        let texture = self.gpu(texture)?;
        let view = self.backend.create_view(
            &texture,
            &TextureViewDesc {
                format: depth,
                dimension,
                base_mip: v.desc.first_mip,
                mip_count: v.desc.mip_count,
                base_layer: v.desc.first_layer,
                layer_count: v.desc.layer_count,
                stencil: false,
            },
        );
        let view = self.gpu(view)?;
        let id = self.id();
        let handle = view.clone();
        *v.compared.borrow_mut() = Some(ComparedDepth {
            texture: Kept::new(texture, id, Object::Texture, &self.graveyard),
            view: Kept::new(view, id, Object::TextureView, &self.graveyard),
        });
        Ok(handle)
    }

    /// Copies the levels and layers view `v` sees into the depth texture it is compared through.
    fn copy_compared(&mut self, v: &View<B>) {
        let compared = v.compared.borrow();
        let Some(ComparedDepth { texture: depth, .. }) = &*compared else {
            return;
        };
        let stored = v.twin.as_ref().unwrap_or(&v.texture.gpu);
        for mip in v.desc.first_mip..v.desc.first_mip + v.desc.mip_count {
            let mut extent = v.texture.mip_extent(mip);
            extent.depth_or_layers = v.desc.layer_count;
            let at = |texture| TextureRegion {
                texture,
                mip,
                layer: v.desc.first_layer,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            };
            self.backend
                .copy_texture(&at(&stored.handle), &at(&depth.handle), extent);
        }
    }

    /// The views of `stages` and the storage views of `writer` made on twins.
    fn twinned(&self, stages: &[usize], writer: usize) -> Vec<(Rc<ShaderView<B>>, bool)> {
        let mut views = Vec::new();
        for &stage in stages {
            for v in self.ctx.resources[stage].iter().flatten() {
                views.push((v.clone(), false));
            }
        }
        for v in self.ctx.uavs[writer].iter().flatten() {
            views.push((v.clone(), true));
        }
        views.retain(|(v, _)| matches!(&**v, ShaderView::Texture(t) if t.twin.is_some()));
        views
    }

    /// Before a draw or dispatch reading views of `stages` (and storage views of `writer`) made
    /// on twins: the twins copied into, outside a pass. Answers whether any storage view needs
    /// copying out after.
    pub(super) fn twins_in(&mut self, stages: &[usize], writer: usize) -> bool {
        if !stages.iter().chain([&writer]).any(|&s| self.ctx.twins[s]) {
            return false;
        }
        let views = self.twinned(stages, writer);
        let compared: Vec<_> = stages
            .iter()
            .flat_map(|&s| self.ctx.compared[s].iter().cloned())
            .collect();
        if views.is_empty() && compared.is_empty() {
            return false;
        }
        self.end_pass();
        self.backend.slow_paths().hit(SlowPath::ReinterpretCopy);
        for (v, _) in &views {
            if let ShaderView::Texture(t) = &**v {
                self.copy_twin(t, true);
            }
        }
        for v in &compared {
            if let ShaderView::Texture(t) = &**v {
                self.copy_compared(t);
            }
        }
        views.iter().any(|(_, w)| *w)
    }

    /// After it: the storage views of `writer` made on twins copied out, outside a pass.
    pub(super) fn twins_out(&mut self, writer: usize) {
        self.end_pass();
        for (v, _) in self.twinned(&[], writer) {
            if let ShaderView::Texture(t) = &*v {
                self.copy_twin(t, false);
            }
        }
    }
}
