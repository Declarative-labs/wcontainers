//! Null descriptors (views made with no resource, constant buffer views at address zero): a program
//! reads zeros through them and its writes are dropped. Each binds a resource of the front's, of the
//! shape the program reads, holding zeros; those bound writable are zeroed again before the
//! submission that bound them ends.

use super::*;

/// Bytes of the null buffers: the largest constant buffer view.
pub(super) const NULL_BYTES: u64 = 65536;

/// The texture a program's binding reads, as far as a null view must match it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Shape {
    pub dimension: ViewDimension,
    pub format: Format,
    pub samples: u32,
    /// A storage texture.
    pub storage: bool,
}

/// The shape of a sampled or storage texture binding of naga type `ty`; `None` for other types
/// and for shapes no null view is made of (one-dimensional textures, storage formats the interface
/// lacks).
pub(super) fn shape(ty: &naga::TypeInner) -> Option<Shape> {
    let naga::TypeInner::Image {
        dim,
        arrayed,
        class,
    } = *ty
    else {
        return None;
    };
    let dimension = match (dim, arrayed) {
        (naga::ImageDimension::D2, false) => ViewDimension::D2,
        (naga::ImageDimension::D2, true) => ViewDimension::D2Array,
        (naga::ImageDimension::Cube, false) => ViewDimension::Cube,
        (naga::ImageDimension::Cube, true) => ViewDimension::CubeArray,
        (naga::ImageDimension::D3, _) => ViewDimension::D3,
        _ => return None,
    };
    let (format, samples, storage) = match class {
        naga::ImageClass::Sampled { kind, multi } => {
            let format = match kind {
                naga::ScalarKind::Sint => Format::Rgba8Sint,
                naga::ScalarKind::Uint => Format::Rgba8Uint,
                _ => Format::Rgba8Unorm,
            };
            (format, if multi { 4 } else { 1 }, false)
        }
        naga::ImageClass::Depth { multi } => {
            (Format::Depth32Float, if multi { 4 } else { 1 }, false)
        }
        naga::ImageClass::Storage { format, .. } => (storage_format(format)?, 1, true),
        _ => return None,
    };
    Some(Shape {
        dimension,
        format,
        samples,
        storage,
    })
}

fn storage_format(f: naga::StorageFormat) -> Option<Format> {
    use naga::StorageFormat as S;
    Some(match f {
        S::R8Unorm => Format::R8Unorm,
        S::R8Snorm => Format::R8Snorm,
        S::R8Uint => Format::R8Uint,
        S::R8Sint => Format::R8Sint,
        S::R16Uint => Format::R16Uint,
        S::R16Sint => Format::R16Sint,
        S::R16Float => Format::R16Float,
        S::R16Unorm => Format::R16Unorm,
        S::R16Snorm => Format::R16Snorm,
        S::Rg8Unorm => Format::Rg8Unorm,
        S::Rg8Snorm => Format::Rg8Snorm,
        S::Rg8Uint => Format::Rg8Uint,
        S::Rg8Sint => Format::Rg8Sint,
        S::R32Uint => Format::R32Uint,
        S::R32Sint => Format::R32Sint,
        S::R32Float => Format::R32Float,
        S::Rg16Uint => Format::Rg16Uint,
        S::Rg16Sint => Format::Rg16Sint,
        S::Rg16Float => Format::Rg16Float,
        S::Rg16Unorm => Format::Rg16Unorm,
        S::Rg16Snorm => Format::Rg16Snorm,
        S::Rgba8Unorm => Format::Rgba8Unorm,
        S::Rgba8Snorm => Format::Rgba8Snorm,
        S::Rgba8Uint => Format::Rgba8Uint,
        S::Rgba8Sint => Format::Rgba8Sint,
        S::Bgra8Unorm => Format::Bgra8Unorm,
        S::Rgb10a2Uint => Format::Rgb10a2Uint,
        S::Rgb10a2Unorm => Format::Rgb10a2Unorm,
        S::Rg32Uint => Format::Rg32Uint,
        S::Rg32Sint => Format::Rg32Sint,
        S::Rg32Float => Format::Rg32Float,
        S::Rgba16Uint => Format::Rgba16Uint,
        S::Rgba16Sint => Format::Rgba16Sint,
        S::Rgba16Float => Format::Rgba16Float,
        S::Rgba16Unorm => Format::Rgba16Unorm,
        S::Rgba16Snorm => Format::Rgba16Snorm,
        S::Rgba32Uint => Format::Rgba32Uint,
        S::Rgba32Sint => Format::Rgba32Sint,
        S::Rgba32Float => Format::Rgba32Float,
        _ => return None,
    })
}

/// A null texture: its shape, whether it takes writes, and the texture with its one view.
struct NullTexture<B: Gpu> {
    shape: Shape,
    writable: bool,
    texture: B::Texture,
    desc: TextureDesc,
    view: B::TextureView,
}

/// The null resources made so far.
pub(super) struct Nulls<B: Gpu> {
    /// Zeros read as a uniform block or read-only storage, and the buffer writes go to.
    buffers: Option<[B::Buffer; 2]>,
    textures: Vec<NullTexture<B>>,
    /// A writable one was bound since they were last zeroed.
    written: bool,
}

impl<B: Gpu> Default for Nulls<B> {
    fn default() -> Self {
        Nulls {
            buffers: None,
            textures: Vec::new(),
            written: false,
        }
    }
}

impl<B: Gpu> Device<B> {
    /// Makes the null resources a pipeline's bindings may read, while no pass is open (depth and
    /// multisampled textures are zeroed by a clearing pass).
    pub(super) fn prepare_nulls(&mut self, shapes: &[(Shape, bool)]) -> Result<(), Error> {
        if self.nulls.buffers.is_none() {
            let mut make = |usage| {
                self.backend.create_buffer(&BufferDesc {
                    size: NULL_BYTES,
                    usage: usage | BufferUsage::COPY_DST,
                    intent: MapIntent::Rarely,
                })
            };
            let read = make(BufferUsage::UNIFORM | BufferUsage::STORAGE)?;
            let sink = make(BufferUsage::STORAGE)?;
            for b in [&read, &sink] {
                self.backend.clear_buffer(b, 0, NULL_BYTES, &[0]);
            }
            self.nulls.buffers = Some([read, sink]);
        }
        for &(shape, writable) in shapes {
            let writable = writable && shape.storage;
            if self
                .nulls
                .textures
                .iter()
                .any(|t| (t.shape, t.writable) == (shape, writable))
            {
                continue;
            }
            let cube = matches!(
                shape.dimension,
                ViewDimension::Cube | ViewDimension::CubeArray
            );
            let mut usage = TextureUsage::COPY_DST;
            usage |= if shape.storage {
                TextureUsage::STORAGE
            } else {
                TextureUsage::SAMPLE
            };
            let drawn = shape.samples > 1
                || wcontainers_gpu::format::FORMAT_FACTS[shape.format as usize].depth_stencil;
            if drawn {
                usage |= TextureUsage::RENDER;
            }
            let desc = TextureDesc {
                dimension: if shape.dimension == ViewDimension::D3 {
                    TextureDimension::D3
                } else {
                    TextureDimension::D2
                },
                format: shape.format,
                width: 1,
                height: 1,
                depth_or_layers: if cube { 6 } else { 1 },
                mip_levels: 1,
                samples: shape.samples,
                usage,
                cube,
                reinterpret: false,
            };
            let texture = self.backend.create_texture(&desc)?;
            let view = self.backend.create_view(
                &texture,
                &TextureViewDesc {
                    format: shape.format,
                    dimension: shape.dimension,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: desc.depth_or_layers,
                    stencil: false,
                },
            )?;
            let null = NullTexture {
                shape,
                writable,
                texture,
                desc,
                view,
            };
            self.zero_texture(&null)?;
            self.nulls.textures.push(null);
        }
        Ok(())
    }

    fn zero_texture(&mut self, t: &NullTexture<B>) -> Result<(), Error> {
        let layers = t.desc.depth_or_layers;
        if t.shape.samples > 1
            || wcontainers_gpu::format::FORMAT_FACTS[t.shape.format as usize].depth_stencil
        {
            let value =
                if wcontainers_gpu::format::FORMAT_FACTS[t.shape.format as usize].depth_stencil {
                    ClearValue::DepthStencil {
                        depth: Some(0),
                        stencil: None,
                    }
                } else {
                    ClearValue::Color([0; 4])
                };
            self.backend.clear_texture(
                &t.texture,
                &t.desc,
                t.shape.format,
                0..1,
                0..layers,
                value,
            )?;
        } else {
            let bytes = wcontainers_gpu::format::FORMAT_FACTS[t.shape.format as usize]
                .block
                .2;
            let zeros = vec![0; (bytes * layers) as usize];
            self.backend.write_texture(
                &TextureRegion {
                    texture: &t.texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width: 1,
                    height: 1,
                    depth_or_layers: layers,
                },
                &zeros,
                bytes,
                1,
                WriteMode::Ordered,
            );
        }
        Ok(())
    }

    /// The null buffer a binding reads, or the one its writes go to.
    pub(super) fn null_buffer(&mut self, writable: bool) -> Result<B::Buffer, Error> {
        self.nulls.written |= writable;
        self.nulls
            .buffers
            .as_ref()
            .map(|b| b[usize::from(writable)].clone())
            .ok_or(Error::Invalid("null buffers not made"))
    }

    /// The null texture view of `shape`, made at pipeline creation.
    pub(super) fn null_view(
        &mut self,
        shape: Shape,
        writable: bool,
    ) -> Result<B::TextureView, Error> {
        let writable = writable && shape.storage;
        self.nulls.written |= writable;
        self.nulls
            .textures
            .iter()
            .find(|t| (t.shape, t.writable) == (shape, writable))
            .map(|t| t.view.clone())
            .ok_or(Error::Invalid("a null view of a shape not made"))
    }

    /// Zeroes the null resources writes went to, with no pass open.
    pub(super) fn zero_written_nulls(&mut self) -> Result<(), Error> {
        if !std::mem::take(&mut self.nulls.written) {
            return Ok(());
        }
        if let Some([_, sink]) = &self.nulls.buffers {
            let sink = sink.clone();
            self.backend.clear_buffer(&sink, 0, NULL_BYTES, &[0]);
        }
        let textures = std::mem::take(&mut self.nulls.textures);
        let result = textures
            .iter()
            .filter(|t| t.writable)
            .try_for_each(|t| self.zero_texture(t));
        self.nulls.textures = textures;
        result
    }
}
