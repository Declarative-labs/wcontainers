//! Storage textures read and written through a buffer of their texels
//! (`VariantKey::texels_through_buffer`): for atomics on typed texture views
//! (`AtomicsThroughBuffer`) and for formats the back end cannot store into
//! (`StorageFormatThroughBuffer`). The view's texels are copied into a scratch buffer before the
//! draw or dispatch, outside a pass, and back after it where the view is writable; the program
//! decodes and encodes the view's format.

use super::*;
use crate::list::{ResourceView, UAV_DIMENSION_BUFFER};
use bind::StageBinding;

/// The copies a bound view read through a buffer needs around its draw or dispatch.
pub(super) struct Through<B: Backend> {
    resource: ResourceId,
    mip: u32,
    /// First layer, or first depth slice of a 3D view.
    first: u32,
    extent: Extent,
    three_d: bool,
    buffer: B::Buffer,
    /// Bytes from one row to the next, and rows from one layer to the next.
    row_pitch: u32,
    rows: u32,
    writable: bool,
}

impl<B: Backend> Device<B> {
    /// An unordered access view of a texture bound for `b` as a storage buffer of its texels.
    pub(super) fn texels_through(
        &mut self,
        v: &ResourceView,
        b: &StageBinding,
        writable: bool,
        why: SlowPath,
    ) -> Result<bind::Held<B>, Error> {
        if !matches!(b.visibility, 0 | bind::PIXEL) {
            return Err(Error::NotImplemented(
                "storage textures read through a buffer in a vertex or geometry shader",
            ));
        }
        if v.dimension == UAV_DIMENSION_BUFFER {
            return Err(Error::NotImplemented(bind::OTHER_UAV_DIMENSION));
        }
        let slot = b.binding.slot();
        if slot >= 64 {
            return Err(Error::NotImplemented(
                "a storage texture read through a buffer at a slot past 63",
            ));
        }
        let format = dxgi(v.format)?;
        let (bw, bh, bytes) = format.block();
        if (bw, bh) != (1, 1) {
            return Err(Error::Invalid(
                "a storage view of a block-compressed format",
            ));
        }
        let (_, desc) = self.texture(v.resource)?;
        let desc = *desc;
        if desc.samples > 1 {
            return Err(Error::NotImplemented(
                "multisampled storage textures read through a buffer",
            ));
        }
        let level = |n: u32| (n >> v.mip).max(1);
        let (width, height) = (level(desc.width), level(desc.height));
        let three_d = desc.dimension == TextureDimension::D3;
        let (first, layers) = match v.dimension {
            UAV_DIMENSION_TEXTURE1D | UAV_DIMENSION_TEXTURE2D => (v.first_layer, 1),
            UAV_DIMENSION_TEXTURE1DARRAY | UAV_DIMENSION_TEXTURE2DARRAY => (
                v.first_layer,
                v.layers
                    .min(desc.depth_or_layers.saturating_sub(v.first_layer)),
            ),
            UAV_DIMENSION_TEXTURE3D => {
                let depth = level(desc.depth_or_layers);
                (
                    v.first_layer,
                    v.layers.min(depth.saturating_sub(v.first_layer)),
                )
            }
            _ => return Err(Error::Invalid("a texture view of a buffer")),
        };
        if v.mip >= desc.mip_levels || layers == 0 {
            return Err(Error::Invalid("a storage view past its texture"));
        }
        let row_pitch = (width * bytes).next_multiple_of(256);
        let size = u64::from(row_pitch) * u64::from(height) * u64::from(layers);
        let buffer = self.scratch(
            size,
            BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        )?;
        self.backend.slow_paths().hit(why);
        self.note(v.resource, writable);
        self.bound_formats
            .push((b.visibility, (true, slot), format));
        self.bound_through.push((b.visibility, slot));
        self.bound_helpers.push((
            b.visibility,
            HelperValue::TexelLayout { slot },
            pull::words(&[row_pitch / bytes, height, width, height, layers]),
        ));
        self.through.push(Through {
            resource: v.resource,
            mip: v.mip,
            first,
            extent: Extent {
                width,
                height,
                depth_or_layers: layers,
            },
            three_d,
            buffer: buffer.clone(),
            row_pitch,
            rows: height,
            writable,
        });
        Ok(bind::Held::Storage {
            buffer,
            offset: 0,
            size,
            writable,
        })
    }

    /// Copies the texels of the views the last binding sets read through buffers into them;
    /// outside a pass, after the copies of inherited writes.
    pub(super) fn through_in(&mut self) -> Result<(), Error> {
        for t in &self.through {
            let texture = stored(&self.resources, t.resource)?;
            let region = TextureRegion {
                texture,
                mip: t.mip,
                layer: if t.three_d { 0 } else { t.first },
                origin: Origin {
                    x: 0,
                    y: 0,
                    z: if t.three_d { t.first } else { 0 },
                },
                stencil: false,
            };
            let to = CopyBufferTexture {
                buffer: &t.buffer,
                offset: 0,
                row_pitch: t.row_pitch,
                rows_per_layer: t.rows,
            };
            self.backend.copy_texture_to_buffer(&region, &to, t.extent);
        }
        Ok(())
    }

    /// After the draw or dispatch, outside a pass: the texels of writable views copied back,
    /// and the scratch buffers given to `keep` until the list ends.
    pub(super) fn through_out(&mut self, keep: &mut Vec<B::Buffer>) -> Result<(), Error> {
        for t in std::mem::take(&mut self.through) {
            if t.writable {
                let texture = stored(&self.resources, t.resource)?;
                let from = CopyBufferTexture {
                    buffer: &t.buffer,
                    offset: 0,
                    row_pitch: t.row_pitch,
                    rows_per_layer: t.rows,
                };
                let region = TextureRegion {
                    texture,
                    mip: t.mip,
                    layer: if t.three_d { 0 } else { t.first },
                    origin: Origin {
                        x: 0,
                        y: 0,
                        z: if t.three_d { t.first } else { 0 },
                    },
                    stencil: false,
                };
                self.backend
                    .copy_buffer_to_texture(&from, &region, t.extent);
            }
            keep.push(t.buffer);
        }
        Ok(())
    }
}

/// The back end's texture of resource `id`.
fn stored<B: Backend>(resources: &Slab<Resource<B>>, id: ResourceId) -> Result<&B::Texture, Error> {
    match resources.get(id.0 as usize) {
        Some(Resource {
            storage: Storage::Texture { texture, .. },
            ..
        }) => Ok(texture),
        _ => Err(Error::Invalid("not a texture")),
    }
}
