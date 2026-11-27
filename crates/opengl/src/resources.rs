//! Gallium's resources on the interface's buffers and textures, as the Gallium trait's `Resource`
//! kind, and the maps and writes that move bytes in and out of them (its `Mapping` kind). A
//! resource gives its back end object back when its last handle is dropped.

use std::cell::Cell;
use std::sync::Arc;

use wcontainers_gallium::{Mapped, PipeBox, ResourceTemplate};
use wcontainers_gpu::{
    Gpu, Object, STORAGE_WORD, caps::Caps, caps::FormatUse, format::FORMAT_FACTS, format::Format,
    format::FormatKind, resource::BufferDesc, resource::BufferUsage, resource::CopyBufferTexture,
    resource::Extent, resource::MapIntent, resource::Origin, resource::ReadSource,
    resource::TextureDesc, resource::TextureDimension, resource::TextureRegion,
    resource::TextureUsage, resource::WriteMode,
};

use crate::guest::refuse;
use crate::pipe::*;
use crate::states::{Retired, retire};

/// What a resource holds on the back end.
pub enum Storage<B: Gpu> {
    Buffer(B::Buffer),
    Texture {
        texture: B::Texture,
        desc: TextureDesc,
    },
    /// A multisampled array texture, a 2D resource of this driver a layer: the host's multisampled
    /// textures are single layers.
    Layers(Vec<Arc<Resource<B>>>),
}

/// One resource: the template it was made from, and what holds it on the back end.
pub struct Resource<B: Gpu> {
    pub template: ResourceTemplate,
    pub storage: Storage<B>,
    retired: Retired<B>,
}

impl<B: Gpu> Drop for Resource<B> {
    /// The back end's object goes when the device is next used; a layered texture's layers go with
    /// their last handles.
    fn drop(&mut self) {
        match &self.storage {
            Storage::Buffer(buffer) => retire(&self.retired, Object::Buffer(buffer.clone())),
            Storage::Texture { texture, .. } => {
                retire(&self.retired, Object::Texture(texture.clone()));
            }
            Storage::Layers(_) => {}
        }
    }
}

impl<B: Gpu> Resource<B> {
    pub fn buffer(&self) -> Option<&B::Buffer> {
        match &self.storage {
            Storage::Buffer(buffer) => Some(buffer),
            Storage::Texture { .. } | Storage::Layers(_) => None,
        }
    }

    /// The texture, none for a buffer or a texture held a layer a resource ([`Resource::layer`]).
    pub fn texture(&self) -> Option<(&B::Texture, &TextureDesc)> {
        match &self.storage {
            Storage::Texture { texture, desc } => Some((texture, desc)),
            Storage::Buffer(_) | Storage::Layers(_) => None,
        }
    }

    /// The resources a layer of a texture held a layer a resource, empty for any other.
    pub fn layers(&self) -> &[Arc<Resource<B>>] {
        match &self.storage {
            Storage::Layers(layers) => layers,
            Storage::Buffer(_) | Storage::Texture { .. } => &[],
        }
    }

    /// The resource holding `layer` of this one and the layer's place in it: the layer's own
    /// resource at its layer 0 for a texture held a layer a resource, this one otherwise.
    pub fn layer(&self, layer: u32) -> (&Resource<B>, u32) {
        match self.layers().get(layer as usize) {
            Some(held) => (held, 0),
            None => (self, layer),
        }
    }

    /// The interface's format of the resource's Gallium format.
    pub fn format(&self) -> Option<Format> {
        crate::format::format(self.template.format)
    }
}

/// Every use a Gallium buffer may be put to: OpenGL binds any buffer any way.
/// Storage too, for vertex streams the vertex program fetches itself.
const BUFFER_USAGE: BufferUsage = BufferUsage::VERTEX
    .union(BufferUsage::INDEX)
    .union(BufferUsage::UNIFORM)
    .union(BufferUsage::STORAGE)
    .union(BufferUsage::INDIRECT)
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST)
    .union(BufferUsage::READ_BACK);

/// The interface's texture of a Gallium template, if the interface holds its format and shape.
fn texture_desc(caps: &Caps, template: &ResourceTemplate) -> Option<TextureDesc> {
    let format = crate::format::format(template.format)?;
    let uses = caps.formats.uses[format as usize];
    let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    if uses.contains(FormatUse::SAMPLE) {
        usage |= TextureUsage::SAMPLE;
    }
    if uses.intersects(FormatUse::RENDER | FormatUse::DEPTH_STENCIL) {
        usage |= TextureUsage::RENDER;
    }
    // OpenGL binds any colour texture as an image later (st/mesa names no image binding at
    // creation). Depth formats are never images, whatever their caps say.
    if uses.contains(FormatUse::STORAGE_WRITE)
        && !FORMAT_FACTS[format as usize].depth_stencil
        && template.nr_samples <= 1
    {
        usage |= TextureUsage::STORAGE;
    }
    let target = template.target;
    let (dimension, depth_or_layers) = if target == PIPE_TEXTURE_3D {
        (TextureDimension::D3, u32::from(template.depth0))
    } else {
        (TextureDimension::D2, u32::from(template.array_size))
    };
    Some(TextureDesc {
        dimension,
        format,
        width: template.width0,
        height: template.height0,
        depth_or_layers,
        mip_levels: u32::from(template.last_level) + 1,
        samples: u32::from(template.nr_samples).max(1),
        usage,
        cube: target == PIPE_TEXTURE_CUBE || target == PIPE_TEXTURE_CUBE_ARRAY,
        reinterpret: false,
    })
}

/// `pipe_screen::resource_create`: none where the back end or the interface cannot hold it.
pub fn create<B: Gpu>(
    backend: &mut B,
    caps: &Caps,
    retired: &Retired<B>,
    template: &ResourceTemplate,
) -> Option<Resource<B>> {
    let storage = if template.target == PIPE_BUFFER {
        let usage = template.usage;
        let intent = if usage == PIPE_USAGE_STREAM || usage == PIPE_USAGE_DYNAMIC {
            MapIntent::Often
        } else {
            MapIntent::Rarely
        };
        let desc = BufferDesc {
            size: u64::from(template.width0.max(1)).next_multiple_of(STORAGE_WORD),
            usage: BUFFER_USAGE,
            intent,
        };
        match backend.create_buffer(&desc) {
            Ok(buffer) => Storage::Buffer(buffer),
            Err(error) => {
                eprintln!("{}: no buffer: {error:?}", screen_name());
                return None;
            }
        }
    } else if template.target == PIPE_TEXTURE_2D_ARRAY && template.nr_samples > 1 {
        let layer = ResourceTemplate {
            target: PIPE_TEXTURE_2D,
            array_size: 1,
            ..*template
        };
        let layers = (0..template.array_size)
            .map(|_| create(backend, caps, retired, &layer).map(Arc::new))
            .collect::<Option<Vec<_>>>()?;
        Storage::Layers(layers)
    } else {
        let desc = texture_desc(caps, template)?;
        match backend.create_texture(&desc) {
            Ok(texture) => Storage::Texture { texture, desc },
            Err(error) => {
                eprintln!("{}: no texture: {error:?}", screen_name());
                return None;
            }
        }
    };
    Some(Resource {
        template: *template,
        storage,
        retired: retired.clone(),
    })
}

fn screen_name() -> std::borrow::Cow<'static, str> {
    crate::screen::DRIVER_NAME.to_string_lossy()
}

/// Whether Gallium's map flags `usage` hold `flag` (whose C enum is signed on some hosts).
fn has(usage: u32, flag: pipe_map_flags) -> bool {
    usage & flag as u32 != 0
}

/// How a write treats the bytes already there, from Gallium's map flags.
pub fn write_mode(usage: u32) -> WriteMode {
    if has(usage, PIPE_MAP_DISCARD_WHOLE_RESOURCE) {
        WriteMode::Discard
    } else if has(usage, PIPE_MAP_UNSYNCHRONIZED) {
        WriteMode::NoOverwrite
    } else {
        WriteMode::Ordered
    }
}

/// One map: the resource, the box of one level mapped, and how its next write treats the bytes
/// already there. The bytes the caller sees are the trait's copy ([`Mapped::bytes`]).
pub struct Mapping<B: Gpu> {
    resource: Arc<Resource<B>>,
    level: u32,
    usage: u32,
    area: PipeBox,
    mode: Cell<WriteMode>,
}

impl<B: Gpu> Mapping<B> {
    /// Notes a write made, after which a map that discarded the resource keeps what it wrote.
    fn wrote(&self) {
        if self.mode.get() == WriteMode::Discard {
            self.mode.set(WriteMode::NoOverwrite);
        }
    }

    /// Whether its unmap writes the mapped bytes: mapped for writing, and not flushed by parts.
    pub fn unmap_writes(&self) -> bool {
        has(self.usage, PIPE_MAP_WRITE) && !has(self.usage, PIPE_MAP_FLUSH_EXPLICIT)
    }
}

/// A box's coordinates, which Gallium declares signed, as the interface's.
fn unsigned(value: impl TryInto<u32>) -> u32 {
    match value.try_into() {
        Ok(value) => value,
        Err(_) => refuse(format_args!("a negative coordinate or size of a box")),
    }
}

/// Gallium's block of a format: width and height in texels, and bytes, from the interface's facts
/// of the format of the same bits (Mesa's own table, `util_format_description`, is a helper the
/// Gallium trait does not offer: NEEDED, G4).
fn block(format: u32) -> (u32, u32, u32) {
    match crate::format::format(format) {
        Some(ours) => FORMAT_FACTS[ours as usize].block,
        None => refuse(format_args!(
            "the blocks of pipe_format {format}, which the interface has no format of (G4)"
        )),
    }
}

/// The rows of blocks of a box, and the bytes of one row.
fn box_layout(format: u32, area: &PipeBox) -> (u32, u32) {
    let (width, height, bytes) = block(format);
    (
        unsigned(area.height).div_ceil(height),
        unsigned(area.width).div_ceil(width) * bytes,
    )
}

/// The texels of one map or write: a level's region and its size.
fn region_of<'a, B: Gpu>(
    texture: &'a B::Texture,
    desc: &TextureDesc,
    level: u32,
    area: &PipeBox,
    stencil: bool,
) -> (TextureRegion<'a, B>, Extent) {
    let three_d = desc.dimension == TextureDimension::D3;
    let z = unsigned(area.z);
    (
        TextureRegion {
            texture,
            mip: level,
            layer: if three_d { 0 } else { z },
            origin: Origin {
                x: unsigned(area.x),
                y: unsigned(area.y),
                z: if three_d { z } else { 0 },
            },
            stencil,
        },
        Extent {
            width: unsigned(area.width),
            height: unsigned(area.height),
            depth_or_layers: unsigned(area.depth),
        },
    )
}

/// Bits of depth in the low part of a word of 24-bit depth and 8-bit stencil.
const DEPTH24_MASK: u32 = u32::MAX >> u8::BITS;
/// Where stencil starts in such a word.
const STENCIL_SHIFT: u32 = u32::BITS - u8::BITS;

/// For a format with both depth and stencil, its texel's bytes as Gallium lays it out.
fn depth_stencil_layout(format: Format) -> Option<usize> {
    match format {
        Format::Depth24PlusStencil8 => Some(size_of::<u32>()),
        Format::Depth32FloatStencil8 => Some(size_of::<u64>()),
        _ => None,
    }
}

/// Gallium's texels of a depth and stencil format from the two aspects as the interface reads
/// them (depth in four bytes, stencil in one).
fn join_depth_stencil(format: Format, depth: &[u8], stencil: &[u8], out: &mut [u8]) {
    let texel = depth_stencil_layout(format).unwrap_or(size_of::<u32>());
    for ((out, depth), &stencil) in out
        .chunks_exact_mut(texel)
        .zip(depth.as_chunks::<{ size_of::<u32>() }>().0)
        .zip(stencil)
    {
        let depth = u32::from_le_bytes(*depth);
        if format == Format::Depth24PlusStencil8 {
            let word = depth & DEPTH24_MASK | u32::from(stencil) << STENCIL_SHIFT;
            out.copy_from_slice(&word.to_le_bytes());
        } else {
            let (d, s) = out.split_at_mut(size_of::<u32>());
            d.copy_from_slice(&depth.to_le_bytes());
            s.fill(0);
            s[0] = stencil;
        }
    }
}

/// The two aspects of Gallium's texels of a depth and stencil format, as the interface writes
/// them.
fn split_depth_stencil(format: Format, texels: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let texel = depth_stencil_layout(format).unwrap_or(size_of::<u32>());
    let count = texels.len() / texel;
    let mut depth = Vec::with_capacity(count * size_of::<u32>());
    let mut stencil = Vec::with_capacity(count);
    for t in texels.chunks_exact(texel) {
        let word = u32::from_le_bytes(t[..size_of::<u32>()].try_into().unwrap_or_default());
        if format == Format::Depth24PlusStencil8 {
            depth.extend_from_slice(&(word & DEPTH24_MASK).to_le_bytes());
            stencil.push(word.to_le_bytes()[size_of::<u32>() - 1]);
        } else {
            depth.extend_from_slice(&word.to_le_bytes());
            stencil.push(t[size_of::<u32>()]);
        }
    }
    (depth, stencil)
}

/// Reads the mapped box of a resource back into `out` (`PIPE_MAP_READ`), outside any pass.
fn read_mapped<B: Gpu>(backend: &mut B, mapping: &Mapping<B>, out: &mut [u8]) -> Result<(), ()> {
    let resource = &mapping.resource;
    let area = &mapping.area;
    if let Some(buffer) = resource.buffer() {
        // Read backs move whole words: the words around the range, then the range from them.
        let offset = u64::from(unsigned(area.x));
        let size = out.len() as u64;
        let start = offset - offset % STORAGE_WORD;
        let end = (offset + size).next_multiple_of(STORAGE_WORD);
        let mut words = vec![0u8; usize::try_from(end - start).map_err(|_| ())?];
        backend
            .read_back_now(
                &ReadSource::Buffer {
                    buffer,
                    offset: start,
                    size: end - start,
                },
                &mut words,
            )
            .map_err(|_| ())?;
        let from = usize::try_from(offset - start).map_err(|_| ())?;
        out.copy_from_slice(&words[from..from + out.len()]);
        return Ok(());
    }
    let Some((texture, desc)) = resource.texture() else {
        return Err(());
    };
    let level = mapping.level;
    let mut read = |stencil: bool, out: &mut [u8]| {
        let (region, extent) = region_of::<B>(texture, desc, level, area, stencil);
        backend
            .read_back_now(&ReadSource::Texture { region, extent }, out)
            .map_err(|_| ())
    };
    match depth_stencil_layout(desc.format) {
        Some(texel) => {
            let texels = out.len() / texel;
            let mut depth = vec![0u8; texels * size_of::<u32>()];
            let mut stencil = vec![0u8; texels];
            read(false, &mut depth)?;
            read(true, &mut stencil)?;
            join_depth_stencil(desc.format, &depth, &stencil, out);
            Ok(())
        }
        None => read(
            FORMAT_FACTS[desc.format as usize].kind == FormatKind::Stencil,
            out,
        ),
    }
}

/// `pipe_context::buffer_map` and the transfer helper's `transfer_map` (a single-sampled texture;
/// the helper maps a multisampled one through one): the bytes of one box of a level, read back
/// first where the map reads. The caller has closed its open pass where it reads.
pub fn map<B: Gpu>(
    backend: &mut B,
    resource: &Arc<Resource<B>>,
    level: u32,
    usage: u32,
    area: &PipeBox,
) -> Option<Mapped<Mapping<B>>> {
    let (size, stride, layer_stride) = if resource.buffer().is_some() {
        (usize::try_from(unsigned(area.width)).ok()?, 0, 0)
    } else {
        resource.texture()?;
        let (rows, stride) = box_layout(resource.template.format, area);
        let layer_stride = u64::from(rows) * u64::from(stride);
        let size = layer_stride * u64::from(unsigned(area.depth));
        (usize::try_from(size).ok()?, stride, layer_stride)
    };
    let mapping = Mapping {
        resource: Arc::clone(resource),
        level,
        usage,
        area: *area,
        mode: Cell::new(write_mode(usage)),
    };
    let mut bytes = vec![0u8; size];
    if has(usage, PIPE_MAP_READ) && read_mapped(backend, &mapping, &mut bytes).is_err() {
        return None;
    }
    Some(Mapped {
        mapping,
        bytes,
        stride,
        layer_stride,
    })
}

/// Writes the bytes of `part` (relative to the map) of a map, from `written`, the whole map's
/// bytes as the caller left them (`transfer_flush_region`, and the unmap of the whole box).
pub fn write_mapped<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    mapping: &Mapping<B>,
    written: &[u8],
    part: &PipeBox,
) {
    let resource = &mapping.resource;
    let mode = mapping.mode.get();
    if let Some(buffer) = resource.buffer() {
        let start = unsigned(part.x) as usize;
        let Some(data) = written.get(start..start + unsigned(part.width) as usize) else {
            refuse(format_args!("a flushed range past its map"));
        };
        let at = u64::from(unsigned(mapping.area.x)) + start as u64;
        backend.write_buffer(buffer, at, data, mode);
        mapping.wrote();
        return;
    }
    if resource.texture().is_none() {
        return;
    }
    // The part of the map flushed, written now (the transfer helper copies a multisampled
    // texture's single-sampled map into it before unmapping that).
    let format = resource.template.format;
    let mapped = mapping.area;
    let (width, height, bytes) = block(format);
    let (rows, stride) = box_layout(format, &mapped);
    let start = unsigned(part.z) as usize * rows as usize * stride as usize
        + (unsigned(part.y) / height) as usize * stride as usize
        + (unsigned(part.x) / width * bytes) as usize;
    let Some(data) = written.get(start..) else {
        refuse(format_args!("a flushed box past its map"));
    };
    let at = PipeBox {
        x: mapped.x + part.x,
        y: mapped.y + part.y,
        z: mapped.z + part.z,
        ..*part
    };
    write_texels(
        backend,
        retired,
        resource,
        mapping.level,
        &at,
        data,
        stride,
        rows,
        mode,
    );
    mapping.wrote();
}

/// Writes texels laid out as Gallium lays them into one box of a level. The caller has closed its
/// open pass (a target's clears wait in the pass tracker for its next pass, which must come
/// first).
#[allow(
    clippy::too_many_arguments,
    reason = "one write of Gallium's, unpacked"
)]
pub fn write_texels<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    resource: &Resource<B>,
    level: u32,
    area: &PipeBox,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    mode: WriteMode,
) {
    let Some((texture, desc)) = resource.texture() else {
        return;
    };
    let mut write = |stencil: bool, data: &[u8], row_pitch: u32, mode: WriteMode| {
        let (region, extent) = region_of::<B>(texture, desc, level, area, stencil);
        write_aspect(
            backend,
            retired,
            desc,
            &region,
            extent,
            data,
            row_pitch,
            rows_per_layer,
            mode,
        );
    };
    if let Some(texel) = depth_stencil_layout(desc.format) {
        let (depth, stencil) = split_depth_stencil(desc.format, data);
        let ratio = (texel / size_of::<u32>()) as u32;
        write(false, &depth, row_pitch / ratio, mode);
        let stencil_mode = if mode == WriteMode::Discard {
            WriteMode::NoOverwrite
        } else {
            mode
        };
        write(true, &stencil, row_pitch / texel as u32, stencil_mode);
        return;
    }
    write(
        FORMAT_FACTS[desc.format as usize].kind == FormatKind::Stencil,
        data,
        row_pitch,
        mode,
    );
}

/// Writes one aspect's texels into a region. Part of a level of depth or stencil goes through a
/// buffer and the interface's copy from it, which makes every region of those aspects (WebGPU
/// writes them only whole, and draws such copies).
#[allow(
    clippy::too_many_arguments,
    reason = "one write of Gallium's, unpacked"
)]
fn write_aspect<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    desc: &TextureDesc,
    region: &TextureRegion<'_, B>,
    extent: Extent,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    mode: WriteMode,
) {
    let whole = region.origin.x == 0
        && region.origin.y == 0
        && extent.width == (desc.width >> region.mip).max(1)
        && extent.height == (desc.height >> region.mip).max(1);
    let aspect = region.stencil
        || matches!(
            FORMAT_FACTS[desc.format as usize].kind,
            FormatKind::Depth | FormatKind::DepthStencil | FormatKind::Stencil
        );
    if whole || !aspect {
        backend.write_texture(region, extent, data, row_pitch, rows_per_layer, mode);
        return;
    }
    let mut words = data.to_vec();
    words.resize(data.len().next_multiple_of(STORAGE_WORD as usize), 0);
    let staged = match backend.create_buffer(&BufferDesc {
        size: words.len() as u64,
        usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        intent: MapIntent::Rarely,
    }) {
        Ok(staged) => staged,
        Err(error) => refuse(format_args!(
            "the buffer part of a depth or stencil level is written from: {error:?}"
        )),
    };
    backend.write_buffer(&staged, 0, &words, WriteMode::Discard);
    backend.copy_buffer_to_texture(
        &CopyBufferTexture {
            buffer: &staged,
            offset: 0,
            row_pitch,
            rows_per_layer,
        },
        region,
        extent,
    );
    retire(retired, Object::Buffer(staged));
}

/// `pipe_context::texture_subdata` of a single-sampled texture, from the caller's bytes at its
/// pitches (0: tightly packed). A multisampled one went through Mesa's default, which writes by
/// the transfer helper's maps: that helper is not offered by the Gallium trait (G4), so it is
/// refused by name.
#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
pub fn subdata<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    resource: &Resource<B>,
    level: u32,
    usage: u32,
    area: &PipeBox,
    data: &[u8],
    stride: u32,
    layer_stride: u64,
) {
    if resource.template.nr_samples > 1 {
        refuse(format_args!(
            "texture_subdata of a multisampled texture: Mesa's u_default_texture_subdata (G4)"
        ));
    }
    let (rows, row) = box_layout(resource.template.format, area);
    let stride = if stride == 0 { row } else { stride };
    let rows_per_layer = if layer_stride == 0 {
        rows
    } else {
        u32::try_from(layer_stride / u64::from(stride)).unwrap_or(u32::MAX)
    };
    let layers = unsigned(area.depth) as usize;
    let size = (rows_per_layer as usize * layers.saturating_sub(1) + rows as usize)
        .saturating_sub(1)
        * stride as usize
        + row as usize;
    let Some(data) = data.get(..size) else {
        refuse(format_args!(
            "texture_subdata of {} bytes, fewer than its box at its pitches ({size})",
            data.len()
        ));
    };
    write_texels(
        backend,
        retired,
        resource,
        level,
        area,
        data,
        stride,
        rows_per_layer,
        write_mode(usage),
    );
}

/// The resource a map is of.
impl<B: Gpu> Mapping<B> {
    pub fn resource(&self) -> &Arc<Resource<B>> {
        &self.resource
    }

    /// The whole box mapped, relative to the map.
    pub fn whole(&self) -> PipeBox {
        PipeBox {
            x: 0,
            y: 0,
            z: 0,
            ..self.area
        }
    }
}
