//! Gallium's resources on the interface's buffers and textures, their references, and the maps and
//! writes that move bytes in and out of them.

use std::ffi::{c_uint, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, Extent, Format, FormatKind, FormatUse, MapIntent, Object,
    Origin, ReadSource, STORAGE_WORD, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    WriteMode,
};

use crate::context::Context;
use crate::format;
use crate::gallium::*;
use crate::screen::Screen;

/// What a resource holds on the back end.
pub enum Storage<B: Backend> {
    Buffer(B::Buffer),
    Texture {
        texture: B::Texture,
        desc: TextureDesc,
    },
}

/// One resource: Gallium's struct first, so that a `pipe_resource` pointer is one to it.
#[repr(C)]
pub struct Resource<B: Backend> {
    pub base: pipe_resource,
    pub storage: Storage<B>,
}

impl<B: Backend> Resource<B> {
    /// # Safety
    /// `resource` is one this driver made, alive for `'a`.
    pub unsafe fn of<'a>(resource: *mut pipe_resource) -> &'a Resource<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*resource.cast::<Resource<B>>() }
    }

    pub fn buffer(&self) -> Option<&B::Buffer> {
        match &self.storage {
            Storage::Buffer(buffer) => Some(buffer),
            Storage::Texture { .. } => None,
        }
    }

    pub fn texture(&self) -> Option<(&B::Texture, &TextureDesc)> {
        match &self.storage {
            Storage::Texture { texture, desc } => Some((texture, desc)),
            Storage::Buffer(_) => None,
        }
    }

    /// The interface's format of the resource's Gallium format.
    pub fn format(&self) -> Option<Format> {
        format::format(self.base.format())
    }
}

/// The count of a Gallium reference.
///
/// # Safety
/// `reference` is a live `pipe_reference`.
unsafe fn count<'a>(reference: *mut pipe_reference) -> &'a AtomicI32 {
    // SAFETY: the caller's; Mesa changes the count only atomically.
    unsafe { AtomicI32::from_ptr(&raw mut (*reference).count) }
}

/// Points `dst` at `src`, taking a reference on `src` and dropping the one `dst` held; a resource
/// whose last reference goes is destroyed (`pipe_resource_reference`).
///
/// # Safety
/// Both are null or live resources of this driver.
pub unsafe fn reference(dst: &mut *mut pipe_resource, src: *mut pipe_resource) {
    if *dst == src {
        return;
    }
    if !src.is_null() {
        // SAFETY: the caller's.
        unsafe { count(&raw mut (*src).__bindgen_anon_1.reference) }
            .fetch_add(1, Ordering::Relaxed);
    }
    let old = std::mem::replace(dst, src);
    // SAFETY: the caller's.
    unsafe { release(old) };
}

/// Drops one reference on `resource`, destroying it with the last.
///
/// # Safety
/// `resource` is null or a live resource holding the reference dropped.
pub unsafe fn release(resource: *mut pipe_resource) {
    if resource.is_null() {
        return;
    }
    // SAFETY: the caller's.
    let last = unsafe { count(&raw mut (*resource).__bindgen_anon_1.reference) }
        .fetch_sub(1, Ordering::AcqRel)
        == 1;
    if last {
        // SAFETY: the resource holds its screen, which outlives it.
        let screen = unsafe { (*resource).screen };
        if let Some(destroy) = unsafe { (*screen).resource_destroy } {
            // SAFETY: the last reference went; Mesa's own destroy takes the resource.
            unsafe { destroy(screen, resource) };
        }
    }
}

/// `pipe_context::resource_release`: the frontend gives its reference back.
pub unsafe extern "C" fn resource_release(_: *mut pipe_context, resource: *mut pipe_resource) {
    // SAFETY: Mesa passes a resource it holds a reference on.
    unsafe { release(resource) };
}

/// Every use a Gallium buffer may be put to: OpenGL binds any buffer any way.
const BUFFER_USAGE: BufferUsage = BufferUsage::VERTEX
    .union(BufferUsage::INDEX)
    .union(BufferUsage::UNIFORM)
    .union(BufferUsage::INDIRECT)
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST)
    .union(BufferUsage::READ_BACK);

/// The interface's texture of a Gallium template, if the interface holds its format and shape.
fn texture_desc<B: Backend>(screen: &Screen<B>, template: &pipe_resource) -> Option<TextureDesc> {
    let format = format::format(template.format())?;
    let uses = screen.caps().formats.get(format);
    let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    if uses.contains(FormatUse::SAMPLE) {
        usage |= TextureUsage::SAMPLE;
    }
    if uses.intersects(FormatUse::RENDER | FormatUse::DEPTH_STENCIL) {
        usage |= TextureUsage::RENDER;
    }
    let target = template.target();
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

/// `pipe_screen::resource_create`.
pub unsafe extern "C" fn resource_create<B: Backend>(
    screen: *mut pipe_screen,
    template: *const pipe_resource,
) -> *mut pipe_resource {
    // SAFETY: Mesa passes this driver's screen and a template.
    let (ours, template) = unsafe { (Screen::<B>::of(screen), &*template) };
    let storage: Storage<B> = if template.target() == PIPE_BUFFER {
        let usage = template.usage();
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
        match ours.backend().create_buffer(&desc) {
            Ok(buffer) => Storage::Buffer(buffer),
            Err(error) => {
                eprintln!("{}: no buffer: {error:?}", screen_name());
                return std::ptr::null_mut();
            }
        }
    } else {
        let Some(desc) = texture_desc(ours, template) else {
            return std::ptr::null_mut();
        };
        match ours.backend().create_texture(&desc) {
            Ok(texture) => Storage::Texture { texture, desc },
            Err(error) => {
                eprintln!("{}: no texture: {error:?}", screen_name());
                return std::ptr::null_mut();
            }
        }
    };
    let mut base = *template;
    base.__bindgen_anon_1.reference = pipe_reference { count: 1 };
    base.screen = screen;
    base.next = std::ptr::null_mut();
    Box::into_raw(Box::new(Resource { base, storage })).cast()
}

fn screen_name() -> std::borrow::Cow<'static, str> {
    crate::screen::DRIVER_NAME.to_string_lossy()
}

/// `pipe_screen::resource_destroy`. The back end's object goes when the device is next used.
pub unsafe extern "C" fn resource_destroy<B: Backend>(
    screen: *mut pipe_screen,
    resource: *mut pipe_resource,
) {
    // SAFETY: Mesa passes this driver's screen and a resource it made, destroyed once.
    let (screen, resource) = unsafe {
        (
            Screen::<B>::of(screen),
            Box::from_raw(resource.cast::<Resource<B>>()),
        )
    };
    screen.retire(match resource.storage {
        Storage::Buffer(buffer) => Object::Buffer(buffer),
        Storage::Texture { texture, .. } => Object::Texture(texture),
    });
}

/// Whether Gallium's map flags `usage` hold `flag` (whose C enum is signed on some hosts).
fn has(usage: c_uint, flag: pipe_map_flags) -> bool {
    usage & flag as c_uint != 0
}

/// How a write treats the bytes already there, from Gallium's map flags.
pub fn write_mode(usage: c_uint) -> WriteMode {
    if has(usage, PIPE_MAP_DISCARD_WHOLE_RESOURCE) {
        WriteMode::Discard
    } else if has(usage, PIPE_MAP_UNSYNCHRONIZED) {
        WriteMode::NoOverwrite
    } else {
        WriteMode::Ordered
    }
}

/// One map: Gallium's struct first, then the bytes the caller sees.
#[repr(C)]
struct Transfer {
    base: pipe_transfer,
    /// The mapped bytes, in words so that they are aligned for any texel.
    words: Vec<u64>,
    bytes: usize,
    /// How the next write treats the bytes already there.
    mode: WriteMode,
}

impl Transfer {
    fn bytes(&mut self) -> &mut [u8] {
        let bytes = self.bytes;
        // SAFETY: the words hold at least `bytes` bytes.
        unsafe { std::slice::from_raw_parts_mut(self.words.as_mut_ptr().cast(), bytes) }
    }
}

/// A transfer of `bytes` bytes of `resource`, holding a reference on it.
///
/// # Safety
/// `resource` is a live resource of this driver.
unsafe fn transfer(
    resource: *mut pipe_resource,
    level: c_uint,
    usage: c_uint,
    area: pipe_box,
    bytes: usize,
) -> Box<Transfer> {
    let mut base = pipe_transfer::default();
    // SAFETY: the caller's.
    unsafe { reference(&mut base.resource, resource) };
    base.set_level(level);
    base.set_usage(usage as pipe_map_flags);
    base.box_ = area;
    Box::new(Transfer {
        base,
        words: vec![0; bytes.div_ceil(size_of::<u64>())],
        bytes,
        mode: write_mode(usage),
    })
}

/// Closes the context's open pass, as every read back and copy needs.
fn outside_pass<B: Backend>(context: &mut Context<B>, backend: &mut B) {
    if let Err(error) = context.tracker.close(backend) {
        eprintln!("{}: closing a pass: {error:?}", screen_name());
    }
}

/// `pipe_context::buffer_map`.
pub unsafe extern "C" fn buffer_map<B: Backend>(
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    level: c_uint,
    usage: c_uint,
    area: *const pipe_box,
    out: *mut *mut pipe_transfer,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context, a buffer of it and the range.
    let (context, area) = unsafe { (Context::<B>::of(pipe), *area) };
    let (Ok(offset), Ok(size)) = (u64::try_from(area.x), usize::try_from(area.width)) else {
        return std::ptr::null_mut();
    };
    // SAFETY: as above.
    let mut transfer = unsafe { transfer(resource, level, usage, area, size) };
    if has(usage, PIPE_MAP_READ) {
        // SAFETY: as above.
        let ours = unsafe { Resource::<B>::of(resource) };
        let Some(buffer) = ours.buffer() else {
            return std::ptr::null_mut();
        };
        let mut backend = context.screen().backend();
        outside_pass(context, &mut backend);
        // Read backs move whole words: the words around the range, then the range from them.
        let start = offset - offset % STORAGE_WORD;
        let end = (offset + size as u64).next_multiple_of(STORAGE_WORD);
        let mut words = vec![0u8; (end - start) as usize];
        let read = backend.read_back_now(
            &ReadSource::Buffer {
                buffer,
                offset: start,
                size: end - start,
            },
            &mut words,
        );
        if read.is_err() {
            return std::ptr::null_mut();
        }
        let from = (offset - start) as usize;
        transfer.bytes().copy_from_slice(&words[from..from + size]);
    }
    let transfer = Box::into_raw(transfer);
    // SAFETY: Mesa gives a place for the transfer; the bytes live until the unmap.
    unsafe {
        *out = transfer.cast();
        (*transfer).words.as_mut_ptr().cast()
    }
}

/// Writes `range` (bytes from the map's start) of a buffer map.
fn write_mapped_buffer<B: Backend>(
    context: &mut Context<B>,
    transfer: &mut Transfer,
    range: std::ops::Range<usize>,
) {
    // SAFETY: the transfer holds a reference on its resource.
    let resource = unsafe { Resource::<B>::of(transfer.base.resource) };
    let Some(buffer) = resource.buffer() else {
        return;
    };
    let at = transfer.base.box_.x as u64 + range.start as u64;
    let mode = transfer.mode;
    let data = &transfer.bytes()[range];
    context
        .screen()
        .backend()
        .write_buffer(buffer, at, data, mode);
    if mode == WriteMode::Discard {
        transfer.mode = WriteMode::NoOverwrite;
    }
}

/// `pipe_context::transfer_flush_region`: `area` is relative to the map.
pub unsafe extern "C" fn transfer_flush_region<B: Backend>(
    pipe: *mut pipe_context,
    transfer: *mut pipe_transfer,
    area: *const pipe_box,
) {
    // SAFETY: Mesa passes this driver's context, one of its maps and a part of it.
    let (context, transfer, area) = unsafe {
        (
            Context::<B>::of(pipe),
            &mut *transfer.cast::<Transfer>(),
            *area,
        )
    };
    // SAFETY: as above.
    let target = unsafe { (*transfer.base.resource).target() };
    if target != PIPE_BUFFER {
        return;
    }
    let start = area.x as usize;
    write_mapped_buffer(context, transfer, start..start + area.width as usize);
}

/// `pipe_context::buffer_unmap`: writes what was mapped for writing, unless the caller flushed
/// its parts itself.
pub unsafe extern "C" fn buffer_unmap<B: Backend>(
    pipe: *mut pipe_context,
    transfer: *mut pipe_transfer,
) {
    // SAFETY: Mesa passes this driver's context and one of its maps, unmapped once.
    let (context, mut transfer) = unsafe {
        (
            Context::<B>::of(pipe),
            Box::from_raw(transfer.cast::<Transfer>()),
        )
    };
    let usage = transfer.base.usage() as c_uint;
    if has(usage, PIPE_MAP_WRITE) && !has(usage, PIPE_MAP_FLUSH_EXPLICIT) {
        let bytes = transfer.bytes;
        write_mapped_buffer(context, &mut transfer, 0..bytes);
    }
    // SAFETY: the transfer's reference is dropped once.
    unsafe { release(transfer.base.resource) };
}

/// `pipe_context::buffer_subdata`: written from the caller's bytes, with no copy before.
pub unsafe extern "C" fn buffer_subdata<B: Backend>(
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    usage: c_uint,
    offset: c_uint,
    size: c_uint,
    data: *const c_void,
) {
    // SAFETY: Mesa passes this driver's context, a buffer of it and `size` bytes.
    let (context, resource, data) = unsafe {
        (
            Context::<B>::of(pipe),
            Resource::<B>::of(resource),
            std::slice::from_raw_parts(data.cast::<u8>(), size as usize),
        )
    };
    let Some(buffer) = resource.buffer() else {
        return;
    };
    context
        .screen()
        .backend()
        .write_buffer(buffer, u64::from(offset), data, write_mode(usage));
}

/// Gallium's block of a format: width and height in texels, and bytes.
fn block(format: pipe_format) -> (u32, u32, u32) {
    // SAFETY: Mesa's table holds every format.
    let block = unsafe { (*util_format_description(format)).block };
    (block.width, block.height, block.bits / u8::BITS)
}

/// The rows of blocks of a box, and the bytes of one row.
fn box_layout(format: pipe_format, area: &pipe_box) -> (u32, u32) {
    let (width, height, bytes) = block(format);
    (
        (area.height as u32).div_ceil(height),
        (area.width as u32).div_ceil(width) * bytes,
    )
}

/// The texels of one map or write: a level's region and its size.
fn region_of<'a, B: Backend>(
    texture: &'a B::Texture,
    desc: &TextureDesc,
    level: c_uint,
    area: &pipe_box,
    stencil: bool,
) -> (TextureRegion<'a, B>, Extent) {
    let three_d = desc.dimension == TextureDimension::D3;
    let z = area.z as u32;
    (
        TextureRegion {
            texture,
            mip: level,
            layer: if three_d { 0 } else { z },
            origin: Origin {
                x: area.x as u32,
                y: area.y as u32,
                z: if three_d { z } else { 0 },
            },
            stencil,
        },
        Extent {
            width: area.width as u32,
            height: area.height as u32,
            depth_or_layers: area.depth as u32,
        },
    )
}

/// Bits of depth in the low part of a word of 24-bit depth and 8-bit stencil.
const DEPTH24_MASK: u32 = u32::MAX >> u8::BITS;
/// Where stencil starts in such a word.
const STENCIL_SHIFT: u32 = u32::BITS - u8::BITS;

/// For a format with both depth and stencil, its texel's bytes and where in it each aspect is,
/// as Gallium lays it out.
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
            stencil.push((word >> STENCIL_SHIFT) as u8);
        } else {
            depth.extend_from_slice(&word.to_le_bytes());
            stencil.push(t[size_of::<u32>()]);
        }
    }
    (depth, stencil)
}

/// `pipe_context::texture_map`.
pub unsafe extern "C" fn texture_map<B: Backend>(
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    level: c_uint,
    usage: c_uint,
    area: *const pipe_box,
    out: *mut *mut pipe_transfer,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context, a texture of it and a box of one level.
    let (context, ours, area) =
        unsafe { (Context::<B>::of(pipe), Resource::<B>::of(resource), *area) };
    let Some((texture, desc)) = ours.texture() else {
        return std::ptr::null_mut();
    };
    if desc.samples > 1 {
        eprintln!(
            "{}: refused: a map of a multisampled texture",
            screen_name()
        );
        return std::ptr::null_mut();
    }
    let (rows, stride) = box_layout(ours.base.format(), &area);
    let layer_stride = rows as usize * stride as usize;
    let size = layer_stride * area.depth as usize;
    // SAFETY: as above.
    let mut transfer = unsafe { transfer(resource, level, usage, area, size) };
    transfer.base.stride = stride;
    transfer.base.layer_stride = layer_stride;
    if has(usage, PIPE_MAP_READ) {
        let mut backend = context.screen().backend();
        outside_pass(context, &mut backend);
        let read = |backend: &mut B, stencil: bool, out: &mut [u8]| {
            let (region, extent) = region_of::<B>(texture, desc, level, &area, stencil);
            backend.read_back_now(&ReadSource::Texture { region, extent }, out)
        };
        let done = if depth_stencil_layout(desc.format).is_some() {
            let texels = size / depth_stencil_layout(desc.format).unwrap_or(size);
            let mut depth = vec![0u8; texels * size_of::<u32>()];
            let mut stencil = vec![0u8; texels];
            let both = read(&mut backend, false, &mut depth)
                .and_then(|()| read(&mut backend, true, &mut stencil));
            join_depth_stencil(desc.format, &depth, &stencil, transfer.bytes());
            both
        } else {
            read(
                &mut backend,
                desc.format.kind() == FormatKind::Stencil,
                transfer.bytes(),
            )
        };
        if done.is_err() {
            return std::ptr::null_mut();
        }
    }
    let transfer = Box::into_raw(transfer);
    // SAFETY: Mesa gives a place for the transfer; the bytes live until the unmap.
    unsafe {
        *out = transfer.cast();
        (*transfer).words.as_mut_ptr().cast()
    }
}

/// Writes texels laid out as Gallium lays them into one box of a level.
#[allow(
    clippy::too_many_arguments,
    reason = "one write of Gallium's, unpacked"
)]
fn write_texels<B: Backend>(
    backend: &mut B,
    texture: &B::Texture,
    desc: &TextureDesc,
    level: c_uint,
    area: &pipe_box,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    mode: WriteMode,
) {
    if let Some(texel) = depth_stencil_layout(desc.format) {
        let (depth, stencil) = split_depth_stencil(desc.format, data);
        let ratio = (texel / size_of::<u32>()) as u32;
        let (region, extent) = region_of::<B>(texture, desc, level, area, false);
        backend.write_texture(
            &region,
            extent,
            &depth,
            row_pitch / ratio,
            rows_per_layer,
            mode,
        );
        let stencil_mode = if mode == WriteMode::Discard {
            WriteMode::NoOverwrite
        } else {
            mode
        };
        let (region, extent) = region_of::<B>(texture, desc, level, area, true);
        let stencil_pitch = row_pitch / texel as u32;
        backend.write_texture(
            &region,
            extent,
            &stencil,
            stencil_pitch,
            rows_per_layer,
            stencil_mode,
        );
        return;
    }
    let stencil = desc.format.kind() == FormatKind::Stencil;
    let (region, extent) = region_of::<B>(texture, desc, level, area, stencil);
    backend.write_texture(&region, extent, data, row_pitch, rows_per_layer, mode);
}

/// `pipe_context::texture_unmap`: writes what was mapped for writing.
pub unsafe extern "C" fn texture_unmap<B: Backend>(
    pipe: *mut pipe_context,
    transfer: *mut pipe_transfer,
) {
    // SAFETY: Mesa passes this driver's context and one of its maps, unmapped once.
    let (context, mut transfer) = unsafe {
        (
            Context::<B>::of(pipe),
            Box::from_raw(transfer.cast::<Transfer>()),
        )
    };
    if has(transfer.base.usage() as c_uint, PIPE_MAP_WRITE) {
        // SAFETY: the transfer holds a reference on its resource.
        let resource = unsafe { Resource::<B>::of(transfer.base.resource) };
        if let Some((texture, desc)) = resource.texture() {
            let (rows, stride) = box_layout(resource.base.format(), &transfer.base.box_);
            let (level, area, mode) = (transfer.base.level(), transfer.base.box_, transfer.mode);
            write_texels(
                &mut *context.screen().backend(),
                texture,
                desc,
                level,
                &area,
                transfer.bytes(),
                stride,
                rows,
                mode,
            );
        }
    }
    // SAFETY: the transfer's reference is dropped once.
    unsafe { release(transfer.base.resource) };
}

/// `pipe_context::texture_subdata`: written from the caller's bytes.
#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
pub unsafe extern "C" fn texture_subdata<B: Backend>(
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    level: c_uint,
    usage: c_uint,
    area: *const pipe_box,
    data: *const c_void,
    stride: c_uint,
    layer_stride: usize,
) {
    // SAFETY: Mesa passes this driver's context, a texture of it and a box of one level.
    let (context, ours, area) =
        unsafe { (Context::<B>::of(pipe), Resource::<B>::of(resource), *area) };
    let Some((texture, desc)) = ours.texture() else {
        return;
    };
    let (rows, row) = box_layout(ours.base.format(), &area);
    let stride = if stride == 0 { row } else { stride };
    let rows_per_layer = if layer_stride == 0 {
        rows
    } else {
        (layer_stride / stride as usize) as u32
    };
    let layers = area.depth as usize;
    let size = (rows_per_layer as usize * (layers - 1) + rows as usize - 1) * stride as usize
        + row as usize;
    // SAFETY: Mesa's bytes cover the box at these pitches.
    let data = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
    write_texels(
        &mut *context.screen().backend(),
        texture,
        desc,
        level,
        &area,
        data,
        stride,
        rows_per_layer,
        write_mode(usage),
    );
}
