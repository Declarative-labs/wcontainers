//! Sampler views on the interface's texture views, and the copies and mipmaps between textures.

use std::ffi::c_uint;
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, CopyBufferTexture, Extent, Features, Format, FormatKind,
    MapIntent, Object, Origin, STORAGE_WORD, SlowPath, TEXEL_BUFFER_TEXTURE_WIDTH, TextureDesc,
    TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, ViewDimension,
};

use naga::VectorSize;
use wcontainers_nir::mesa::Swizzle;

use crate::context::Context;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::screen::DRIVER_NAME;
use crate::state::refused;

/// One sampler view: Gallium's struct first, then the interface's view, or why there is none.
#[repr(C)]
pub struct View<B: Backend> {
    base: pipe_sampler_view,
    view: Result<B::TextureView, &'static str>,
    /// How the programs that sample it rearrange its results.
    pub swizzle: [Swizzle; VectorSize::MAX],
    /// Its texels are integers, which programs sample as their nearest texel's load.
    pub integer: bool,
    /// The components its format has (`Format::components`), as a border colour applied in the
    /// program reads them.
    pub components: u32,
    /// For an integer cube map or cube map array, a 2D array of its faces, which programs load from.
    faces: Option<Result<B::TextureView, &'static str>>,
    /// For a texel buffer, the 2D texture its range is copied into, which `view` views.
    pub texels: Option<TexelCopy<B>>,
    /// For a view in another format than its texture's that the back end cannot view it in, the
    /// texture of the view's format its levels and layers are copied into, which `view` views.
    twin: Option<ViewTwin<B>>,
}

/// A texture in a sampler view's format, of its levels and layers, copied from the view's texture
/// before each work that reads it (`ReinterpretCopy`): the interface views a texture only in its
/// own format or its sRGB twin unless the back end reinterprets.
struct ViewTwin<B: Backend> {
    texture: B::Texture,
    first_level: u32,
    first_layer: u32,
    levels: u32,
    /// The layers of a 2D texture's, or the depth of a volume texture's first level.
    layers: u32,
    three_d: bool,
}

/// A texel buffer view's 2D texture, rows of [`TEXEL_BUFFER_TEXTURE_WIDTH`] texels, which the
/// buffer's range is copied into before each work that reads it (`TexelBufferAsTexture`).
pub struct TexelCopy<B: Backend> {
    texture: B::Texture,
    /// Where the view starts in its buffer, in bytes.
    offset: u64,
    /// The view's elements, which programs read up to.
    pub length: u32,
    texel_bytes: u32,
}

/// The identity swizzle, which needs no change to a program.
pub const IDENTITY: [Swizzle; VectorSize::MAX] = [Swizzle::X, Swizzle::Y, Swizzle::Z, Swizzle::W];

/// The NIR route's swizzle of a Gallium one.
fn swizzle(s: pipe_swizzle) -> Swizzle {
    match s {
        PIPE_SWIZZLE_X => Swizzle::X,
        PIPE_SWIZZLE_Y => Swizzle::Y,
        PIPE_SWIZZLE_Z => Swizzle::Z,
        PIPE_SWIZZLE_W => Swizzle::W,
        PIPE_SWIZZLE_1 => Swizzle::One,
        _ => Swizzle::Zero,
    }
}

impl<B: Backend> View<B> {
    /// # Safety
    /// `view` is one this driver made, alive for `'a`.
    pub unsafe fn of<'a>(view: *mut pipe_sampler_view) -> &'a View<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &*view.cast::<View<B>>() }
    }

    /// The view a program samples, or why the driver could not make it.
    pub fn view(&self) -> Result<&B::TextureView, String> {
        self.view.as_ref().map_err(|why| (*why).to_owned())
    }

    /// Copies what the view reads into the texture it views, where that is a copy: a texel buffer
    /// view's range, or a twin's levels and layers, as they are now; nothing for another view.
    pub fn copy_texels(&self, backend: &mut B) -> Result<(), String> {
        if self.twin.is_none() && self.texels.is_none() {
            return Ok(());
        }
        // SAFETY: the view holds a reference on its texture or buffer.
        let resource = unsafe { Resource::<B>::of(self.base.texture) };
        if let Some(twin) = &self.twin {
            let (source, desc) = resource.texture().ok_or("a twin of a buffer's view")?;
            backend.slow_paths().hit(SlowPath::ReinterpretCopy);
            for level in 0..twin.levels {
                let mip = twin.first_level + level;
                let side = |n: u32| (n >> mip).max(1);
                let depth_or_layers = if twin.three_d {
                    side(desc.depth_or_layers)
                } else {
                    twin.layers
                };
                backend.copy_texture(
                    &corner::<B>(source, twin.three_d, mip, 0, 0, twin.first_layer),
                    &corner::<B>(&twin.texture, twin.three_d, level, 0, 0, 0),
                    Extent {
                        width: side(desc.width),
                        height: side(desc.height),
                        depth_or_layers,
                    },
                );
            }
            return Ok(());
        }
        let Some(copy) = &self.texels else {
            return Ok(());
        };
        let buffer = resource
            .buffer()
            .ok_or("a texel buffer view of a texture")?;
        backend.slow_paths().hit(SlowPath::TexelBufferAsTexture);
        let row_pitch = TEXEL_BUFFER_TEXTURE_WIDTH * copy.texel_bytes;
        let rows = copy.length / TEXEL_BUFFER_TEXTURE_WIDTH;
        let last = copy.length % TEXEL_BUFFER_TEXTURE_WIDTH;
        // The whole rows, then the last row's elements.
        let parts = [(0, rows, TEXEL_BUFFER_TEXTURE_WIDTH), (rows, 1, last)];
        for (row, height, width) in parts.into_iter().filter(|&(_, h, w)| h != 0 && w != 0) {
            backend.copy_buffer_to_texture(
                &CopyBufferTexture {
                    buffer,
                    offset: copy.offset + u64::from(row) * u64::from(row_pitch),
                    row_pitch,
                    rows_per_layer: height,
                },
                &TextureRegion {
                    texture: &copy.texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin { x: 0, y: row, z: 0 },
                    stencil: false,
                },
                Extent {
                    width,
                    height,
                    depth_or_layers: 1,
                },
            );
        }
        Ok(())
    }

    /// The 2D array of an integer cube map's or cube map array's faces, or none for another view.
    pub fn faces(&self) -> Option<Result<&B::TextureView, String>> {
        self.faces
            .as_ref()
            .map(|faces| faces.as_ref().map_err(|why| (*why).to_owned()))
    }
}

/// Points `dst` at `src`, taking a reference on `src` and dropping the one `dst` held; a view
/// whose last reference goes is destroyed by its context (`pipe_sampler_view_reference`).
///
/// # Safety
/// Both are null or live views of this driver.
pub unsafe fn reference(dst: &mut *mut pipe_sampler_view, src: *mut pipe_sampler_view) {
    if *dst == src {
        return;
    }
    if !src.is_null() {
        // SAFETY: the caller's.
        unsafe { AtomicI32::from_ptr(&raw mut (*src).reference.count) }
            .fetch_add(1, Ordering::Relaxed);
    }
    let old = std::mem::replace(dst, src);
    // SAFETY: the caller's.
    unsafe { release(old) };
}

/// Drops one reference on `view`, destroying it with the last.
///
/// # Safety
/// `view` is null or a live view holding the reference dropped.
unsafe fn release(view: *mut pipe_sampler_view) {
    if view.is_null() {
        return;
    }
    // SAFETY: the caller's.
    let last = unsafe { AtomicI32::from_ptr(&raw mut (*view).reference.count) }
        .fetch_sub(1, Ordering::AcqRel)
        == 1;
    if last {
        // SAFETY: a view's context outlives it, as Gallium requires.
        unsafe {
            let context = (*view).context;
            if let Some(destroy) = (*context).sampler_view_destroy {
                destroy(context, view);
            }
        }
    }
}

/// `pipe_context::sampler_view_release`: the frontend gives its reference back.
pub unsafe extern "C" fn sampler_view_release(_: *mut pipe_context, view: *mut pipe_sampler_view) {
    // SAFETY: Mesa passes a view it holds a reference on.
    unsafe { release(view) };
}

/// A texture of an image's own format and shape that the image is bound through, where the
/// texture it names is of another format of the same texel size, or is a volume texture of which
/// the image is one slice (`ReinterpretCopy`): copied into before the work and, for an image the
/// program writes, back after it, through `buffer`.
struct Twin<B: Backend> {
    texture: B::Texture,
    buffer: B::Buffer,
    source: B::Texture,
    mip: u32,
    layer: u32,
    /// The first depth slice of a volume texture's.
    slice: u32,
    extent: Extent,
    row_pitch: u32,
    writable: bool,
}

/// An image of a texture read and written through a buffer of its texels, as
/// `HelperValue::TexelLayout` lays them out: for atomics on it (`AtomicsThroughBuffer`), which the
/// common form has none of that give the old value. Copied into before the work and, for an image
/// the program writes, back after it.
struct Texels<B: Backend> {
    /// The resource the texels are of, which images naming the same texels share.
    resource: *mut pipe_resource,
    buffer: B::Buffer,
    source: B::Texture,
    layout: TexelLayout,
    writable: bool,
}

/// Where an image's texels are in its texture and in the buffer they are read through.
#[derive(Clone, Copy, PartialEq)]
pub struct TexelLayout {
    mip: u32,
    /// The first layer, or the first depth slice of a volume texture.
    first: u32,
    three_d: bool,
    extent: Extent,
    texel_bytes: u32,
}

impl TexelLayout {
    /// `HelperValue::TexelLayout`'s words: row pitch in texels, rows per layer, then the view's
    /// width, height and layers.
    pub fn words(&self) -> [u32; wcontainers_nir::direct::TEXEL_LAYOUT_WORDS] {
        let e = self.extent;
        [e.width, e.height, e.width, e.height, e.depth_or_layers]
    }

    /// The texels in the buffer.
    pub fn texels(&self) -> u32 {
        let e = self.extent;
        e.width * e.height * e.depth_or_layers
    }

    fn bytes(&self) -> u64 {
        (u64::from(self.texels()) * u64::from(self.texel_bytes)).next_multiple_of(STORAGE_WORD)
    }

    fn region<'a, B: Backend>(&self, texture: &'a B::Texture) -> TextureRegion<'a, B> {
        TextureRegion {
            texture,
            mip: self.mip,
            layer: if self.three_d { 0 } else { self.first },
            origin: Origin {
                z: if self.three_d { self.first } else { 0 },
                ..Origin::default()
            },
            stencil: false,
        }
    }

    fn rows<'a, B: Backend>(&self, buffer: &'a B::Buffer) -> CopyBufferTexture<'a, B> {
        CopyBufferTexture {
            buffer,
            offset: 0,
            row_pitch: self.extent.width * self.texel_bytes,
            rows_per_layer: self.extent.height,
        }
    }
}

/// The layout of the texels of an image of a texture read through a buffer: its level's
/// layers it names (one slice of a volume texture where it names one), tightly packed.
pub fn texel_layout<B: Backend>(image: &pipe_image_view) -> Result<TexelLayout, String> {
    // SAFETY: an image's resource is live while it is bound.
    let resource = unsafe { Resource::<B>::of(image.resource) };
    let (_, desc) = resource
        .texture()
        .ok_or("an image of a buffer read as texels")?;
    let format = crate::format::format(image.format).ok_or("an image format the device lacks")?;
    let (width, height, texel_bytes) = format.block();
    let (their_width, their_height, theirs) = desc.format.block();
    if (width, height, their_width, their_height) != (1, 1, 1, 1) || texel_bytes != theirs {
        return Err(format!(
            "an image in {format:?} read as texels of a texture in {:?}",
            desc.format
        ));
    }
    if desc.samples > 1 {
        return refused(format_args!("an atomic on a multisampled image"));
    }
    // SAFETY: the image of a texture holds its texture range.
    let range = unsafe { image.u.tex };
    let mip = range.level();
    let size = |n: u32| (n >> mip).max(1);
    let three_d = desc.dimension == TextureDimension::D3;
    let (first, layers) = if range.single_layer_view {
        (range.first_layer(), 1)
    } else if three_d {
        (0, size(desc.depth_or_layers))
    } else {
        (
            range.first_layer(),
            range.last_layer() - range.first_layer() + 1,
        )
    };
    Ok(TexelLayout {
        mip,
        first,
        three_d,
        extent: Extent {
            width: size(desc.width),
            height: size(desc.height),
            depth_or_layers: layers,
        },
        texel_bytes,
    })
}

/// The buffer an image of a texture is read and written through, its texels copied into it at
/// once (outside a pass), with its size in bytes.
pub fn image_texels<B: Backend>(
    backend: &mut B,
    image: &pipe_image_view,
    images: &mut Images<B>,
) -> Result<(usize, u64), String> {
    let layout = texel_layout::<B>(image)?;
    let writable = u32::from(image.shader_access) & PIPE_IMAGE_ACCESS_WRITE != 0;
    // Images of the same texels are bound to one buffer, as they would be to one texture.
    if let Some(i) = images
        .texels
        .iter()
        .position(|t| t.resource == image.resource && t.layout == layout)
    {
        images.texels[i].writable |= writable;
        return Ok((i, layout.bytes()));
    }
    // SAFETY: an image's resource is live while it is bound.
    let resource = unsafe { Resource::<B>::of(image.resource) };
    let (texture, _) = resource
        .texture()
        .ok_or("an image of a buffer read as texels")?;
    let bytes = layout.bytes();
    let buffer = backend
        .create_buffer(&BufferDesc {
            size: bytes,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .map_err(|e| format!("an image's texels: {e:?}"))?;
    backend.slow_paths().hit(SlowPath::AtomicsThroughBuffer);
    backend.copy_texture_to_buffer(
        &layout.region::<B>(texture),
        &layout.rows::<B>(&buffer),
        layout.extent,
    );
    images.texels.push(Texels {
        resource: image.resource,
        buffer,
        source: texture.clone(),
        layout,
        writable,
    });
    Ok((images.texels.len() - 1, bytes))
}

/// The views a binding of images made for one draw or dispatch, and the twins and buffers of
/// texels they were bound through; given back by [`Images::finish`] after the work.
pub struct Images<B: Backend> {
    pub views: Vec<B::TextureView>,
    twins: Vec<Twin<B>>,
    texels: Vec<Texels<B>>,
    /// Buffers the work reads that were made for it (uniform blocks grown to their declared size).
    pub buffers: Vec<B::Buffer>,
    /// Samplers bound beside texel buffers, which programs load and never sample: Gallium binds
    /// none there, the converted program declares one.
    pub samplers: Vec<B::Sampler>,
}

impl<B: Backend> Default for Images<B> {
    fn default() -> Self {
        Images {
            views: Vec::new(),
            twins: Vec::new(),
            texels: Vec::new(),
            buffers: Vec::new(),
            samplers: Vec::new(),
        }
    }
}

impl<B: Backend> Images<B> {
    /// The buffer of texels `image_texels` made at `index`.
    pub fn texel_buffer(&self, index: usize) -> &B::Buffer {
        &self.texels[index].buffer
    }

    /// Copies each twin and buffer of texels the program wrote back into its texture, then
    /// destroys what was made.
    pub fn finish(self, backend: &mut B) {
        for t in self.texels {
            if t.writable {
                backend.copy_buffer_to_texture(
                    &t.layout.rows::<B>(&t.buffer),
                    &t.layout.region::<B>(&t.source),
                    t.layout.extent,
                );
            }
            backend.destroy(Object::Buffer(t.buffer));
        }
        for buffer in self.buffers {
            backend.destroy(Object::Buffer(buffer));
        }
        for view in self.views {
            backend.destroy(Object::TextureView(view));
        }
        for sampler in self.samplers {
            backend.destroy(Object::Sampler(sampler));
        }
        for twin in self.twins {
            if twin.writable {
                twin.copy(backend, false);
            }
            backend.destroy(Object::Texture(twin.texture));
            backend.destroy(Object::Buffer(twin.buffer));
        }
    }
}

impl<B: Backend> Twin<B> {
    /// Moves the texels between the twin and its source: into the twin, or back from it.
    fn copy(&self, backend: &mut B, into: bool) {
        let region = |texture, mip, layer, z| TextureRegion {
            texture,
            mip,
            layer,
            origin: Origin {
                z,
                ..Origin::default()
            },
            stencil: false,
        };
        let source = region(&self.source, self.mip, self.layer, self.slice);
        let twin = region(&self.texture, 0, 0, 0);
        let (from, to) = if into { (source, twin) } else { (twin, source) };
        let rows = CopyBufferTexture {
            buffer: &self.buffer,
            offset: 0,
            row_pitch: self.row_pitch,
            rows_per_layer: self.extent.height,
        };
        backend.copy_texture_to_buffer(&from, &rows, self.extent);
        backend.copy_buffer_to_texture(&rows, &to, self.extent);
    }
}

/// The interface's view of an image a program binds, kept in `images`: one layer, or the layers
/// it names (a cube's as a 2D array of its faces, which is how the common form holds a storage
/// cube). An image of another format than its texture's, of the same texel size, is bound
/// through a twin of its format. Outside a pass: a twin is copied into at once.
pub fn image_view<B: Backend>(
    backend: &mut B,
    image: &pipe_image_view,
    images: &mut Images<B>,
) -> Result<usize, String> {
    // SAFETY: an image's resource is live while it is bound.
    let resource = unsafe { Resource::<B>::of(image.resource) };
    let (texture, desc) = resource
        .texture()
        .ok_or("an image of a buffer (the screen reports none)")?;
    let format = crate::format::format(image.format).ok_or("an image format the device lacks")?;
    // SAFETY: the image of a texture holds its texture range.
    let range = unsafe { image.u.tex };
    let three_d = desc.dimension == TextureDimension::D3;
    let mip = range.level();
    let size = |n: u32| (n >> mip).max(1);
    // One slice of a volume texture, which the host views only whole, is bound through a flat
    // twin of it.
    let slice = (range.single_layer_view && three_d).then(|| range.first_layer());
    let (dimension, base_layer, layer_count) = if range.single_layer_view {
        (ViewDimension::D2, range.first_layer(), 1)
    } else if three_d {
        (ViewDimension::D3, 0, 1)
    } else {
        let layers = range.last_layer() - range.first_layer() + 1;
        let dimension = match resource.base.target() {
            PIPE_TEXTURE_1D_ARRAY
            | PIPE_TEXTURE_2D_ARRAY
            | PIPE_TEXTURE_CUBE
            | PIPE_TEXTURE_CUBE_ARRAY => ViewDimension::D2Array,
            _ => ViewDimension::D2,
        };
        (dimension, range.first_layer(), layers)
    };
    let (viewed, base_mip, base_layer) = if format == desc.format && slice.is_none() {
        (texture.clone(), mip, base_layer)
    } else {
        let (width, _, bytes) = format.block();
        let (their_width, _, theirs) = desc.format.block();
        if bytes != theirs || width != 1 || their_width != 1 {
            return Err(format!(
                "an image in {format:?} of a texture in {:?}, of another texel size",
                desc.format
            ));
        }
        backend.slow_paths().hit(SlowPath::ReinterpretCopy);
        let extent = Extent {
            width: size(desc.width),
            height: size(desc.height),
            depth_or_layers: if three_d && slice.is_none() {
                size(desc.depth_or_layers)
            } else {
                layer_count
            },
        };
        let row_pitch = extent.width * bytes;
        let twin = backend
            .create_texture(&TextureDesc {
                dimension: if slice.is_some() {
                    TextureDimension::D2
                } else {
                    desc.dimension
                },
                format,
                width: extent.width,
                height: extent.height,
                depth_or_layers: extent.depth_or_layers,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::STORAGE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .map_err(|e| format!("an image's twin: {e:?}"))?;
        let buffer = backend
            .create_buffer(&BufferDesc {
                size: (u64::from(row_pitch)
                    * u64::from(extent.height)
                    * u64::from(extent.depth_or_layers))
                .next_multiple_of(STORAGE_WORD),
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .map_err(|e| format!("an image's copy: {e:?}"))?;
        let twin = Twin {
            texture: twin,
            buffer,
            source: texture.clone(),
            mip,
            layer: if three_d { 0 } else { base_layer },
            slice: slice.unwrap_or(0),
            extent,
            row_pitch,
            writable: u32::from(image.shader_access) & PIPE_IMAGE_ACCESS_WRITE != 0,
        };
        twin.copy(backend, true);
        let viewed = twin.texture.clone();
        images.twins.push(twin);
        (viewed, 0, 0)
    };
    let view = backend
        .create_view(
            &viewed,
            &TextureViewDesc {
                format,
                dimension,
                base_mip,
                mip_count: 1,
                base_layer,
                layer_count,
                stencil: false,
            },
        )
        .map_err(|e| format!("an image's view: {e:?}"))?;
    images.views.push(view);
    Ok(images.views.len() - 1)
}

/// What a Gallium sampler view of a texture sees: its format (a stencil view's, read as
/// unsigned integers), its dimension (with `cube_faces`, a cube map's or cube map array's as a 2D array of its
/// faces), its levels and its layers.
struct Seen {
    format: Format,
    stencil: bool,
    dimension: ViewDimension,
    first_level: u32,
    levels: u32,
    first_layer: u32,
    layers: u32,
}

impl Seen {
    fn of(
        view: &pipe_sampler_view,
        desc: &TextureDesc,
        cube_faces: bool,
    ) -> Result<Self, &'static str> {
        let stencil = crate::format::stencil_view(view.format());
        let format = stencil
            .or_else(|| crate::format::format(view.format()))
            .ok_or("a view format the device lacks")?;
        // SAFETY: the view of a texture holds its texture range.
        let range = unsafe { view.u.tex };
        // SAFETY: a live view.
        let dimension = match unsafe { wc_sampler_view_target(view) } {
            PIPE_TEXTURE_3D => ViewDimension::D3,
            PIPE_TEXTURE_CUBE | PIPE_TEXTURE_CUBE_ARRAY if cube_faces => ViewDimension::D2Array,
            PIPE_TEXTURE_CUBE => ViewDimension::Cube,
            PIPE_TEXTURE_CUBE_ARRAY => ViewDimension::CubeArray,
            PIPE_TEXTURE_1D_ARRAY | PIPE_TEXTURE_2D_ARRAY => ViewDimension::D2Array,
            _ => ViewDimension::D2,
        };
        let three_d = desc.dimension == TextureDimension::D3;
        Ok(Self {
            format,
            stencil: stencil.is_some(),
            dimension,
            first_level: range.first_level(),
            levels: range.last_level() - range.first_level() + 1,
            first_layer: if three_d { 0 } else { range.first_layer() },
            // A view of one layer reads only its first layer, whatever range Gallium gives it
            // (st/mesa and the blitter give a cube's or an array's whole range).
            layers: if three_d || dimension == ViewDimension::D2 {
                1
            } else {
                range.last_layer() - range.first_layer() + 1
            },
        })
    }
}

/// The twin a sampler view of a texture is read through, where the back end cannot view the
/// texture in the view's format; none where it can.
fn view_twin<B: Backend>(
    backend: &mut B,
    resource: &Resource<B>,
    view: &pipe_sampler_view,
) -> Result<Option<ViewTwin<B>>, &'static str> {
    let Some((_, desc)) = resource.texture() else {
        return Ok(None);
    };
    let seen = Seen::of(view, desc, false)?;
    let reinterprets = desc.reinterpret
        && backend
            .caps()
            .features
            .contains(Features::FORMAT_REINTERPRET);
    if seen.stencil || seen.format.linear() == desc.format.linear() || reinterprets {
        return Ok(None);
    }
    if seen.format.block() != desc.format.block()
        || seen.format.is_depth_stencil()
        || desc.format.is_depth_stencil()
    {
        return Err("a view in a format of another block than its texture's");
    }
    if desc.samples > 1 {
        return Err("a multisampled texture viewed in another format (not built)");
    }
    let three_d = desc.dimension == TextureDimension::D3;
    let side = |n: u32| (n >> seen.first_level).max(1);
    let texture = backend
        .create_texture(&TextureDesc {
            dimension: desc.dimension,
            format: seen.format,
            width: side(desc.width),
            height: side(desc.height),
            depth_or_layers: if three_d {
                side(desc.depth_or_layers)
            } else {
                seen.layers
            },
            mip_levels: seen.levels,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: matches!(
                seen.dimension,
                ViewDimension::Cube | ViewDimension::CubeArray
            ),
            reinterpret: false,
        })
        .map_err(|_| "a view's twin the device does not make")?;
    Ok(Some(ViewTwin {
        texture,
        first_level: seen.first_level,
        first_layer: seen.first_layer,
        levels: seen.levels,
        layers: seen.layers,
        three_d,
    }))
}

/// The interface's view of a Gallium sampler view, or why there is none; with `cube_faces`, a
/// cube map's or cube map array's as a 2D array of its faces. A view with a twin views the twin.
fn texture_view<B: Backend>(
    backend: &mut B,
    resource: &Resource<B>,
    view: &pipe_sampler_view,
    twin: Option<&ViewTwin<B>>,
    cube_faces: bool,
) -> Result<B::TextureView, &'static str> {
    let (texture, desc) = resource
        .texture()
        .ok_or("a texel buffer (the screen reports none)")?;
    let seen = Seen::of(view, desc, cube_faces)?;
    let (viewed, base_mip, base_layer) = match twin {
        Some(twin) => (&twin.texture, 0, 0),
        None => (texture, seen.first_level, seen.first_layer),
    };
    backend
        .create_view(
            viewed,
            &TextureViewDesc {
                format: seen.format,
                dimension: seen.dimension,
                base_mip,
                mip_count: seen.levels,
                base_layer,
                layer_count: seen.layers,
                stencil: seen.stencil,
            },
        )
        .map_err(|_| "a view the device does not make")
}

/// A texel buffer view's texture and the interface's view of it, or why there are none.
fn texel_buffer<B: Backend>(
    backend: &mut B,
    view: &pipe_sampler_view,
) -> Result<(TexelCopy<B>, B::TextureView), &'static str> {
    let format =
        crate::format::format(view.format()).ok_or("a texel buffer format the device lacks")?;
    let (_, _, texel_bytes) = format.block();
    // SAFETY: the view of a buffer holds its buffer range.
    let range = unsafe { view.u.buf };
    let length = range.size / texel_bytes;
    let rows = length.div_ceil(TEXEL_BUFFER_TEXTURE_WIDTH).max(1);
    if rows > backend.caps().limits.max_texture_2d {
        return Err("a texel buffer longer than its texture holds");
    }
    let texture = backend
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: TEXEL_BUFFER_TEXTURE_WIDTH,
            height: rows,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .map_err(|_| "a texel buffer's texture the device does not make")?;
    let viewed = backend
        .create_view(
            &texture,
            &TextureViewDesc {
                format,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .map_err(|_| "a texel buffer's view the device does not make")?;
    Ok((
        TexelCopy {
            texture,
            offset: u64::from(range.offset),
            length,
            texel_bytes,
        },
        viewed,
    ))
}

/// `pipe_context::create_sampler_view`.
pub unsafe extern "C" fn create_sampler_view<B: Backend>(
    pipe: *mut pipe_context,
    texture: *mut pipe_resource,
    template: *const pipe_sampler_view,
) -> *mut pipe_sampler_view {
    // SAFETY: Mesa passes this driver's context, a resource of it and a template.
    let (context, resource, template) = unsafe {
        (
            Context::<B>::of(pipe),
            Resource::<B>::of(texture),
            *template,
        )
    };
    let mut base = template;
    base.reference = pipe_reference { count: 1 };
    base.texture = std::ptr::null_mut();
    base.context = pipe;
    // SAFETY: as above; the view takes its own reference.
    unsafe { resource::reference(&mut base.texture, texture) };
    let mut backend = context.screen().backend();
    let (view, texels, twin) = if resource.buffer().is_some() {
        match texel_buffer::<B>(&mut backend, &base) {
            Ok((texels, view)) => (Ok(view), Some(texels), None),
            Err(why) => (Err(why), None, None),
        }
    } else {
        match view_twin::<B>(&mut backend, resource, &base) {
            Ok(twin) => (
                texture_view::<B>(&mut backend, resource, &base, twin.as_ref(), false),
                None,
                twin,
            ),
            Err(why) => (Err(why), None, None),
        }
    };
    let mut swizzles = [0; naga::VectorSize::MAX];
    // SAFETY: a live view, and a place for one swizzle a component.
    unsafe { wc_sampler_view_swizzle(&base, swizzles.as_mut_ptr()) };
    let swizzle = swizzles.map(|s| swizzle(s as pipe_swizzle));
    let stencil = crate::format::stencil_view(base.format());
    let format = stencil.or_else(|| crate::format::format(base.format()));
    // A stencil view's texels are unsigned integers, as a stencil texture's are.
    let integer = stencil.is_some()
        || format.is_some_and(|f| {
            matches!(
                f.kind(),
                FormatKind::Uint | FormatKind::Sint | FormatKind::Stencil
            )
        });
    let components = format.map_or(0, Format::components);
    // SAFETY: a live view.
    let cube = matches!(
        unsafe { wc_sampler_view_target(&base) },
        PIPE_TEXTURE_CUBE | PIPE_TEXTURE_CUBE_ARRAY
    );
    let faces = (integer && cube)
        .then(|| texture_view::<B>(&mut backend, resource, &base, twin.as_ref(), true));
    Box::into_raw(Box::new(View::<B> {
        base,
        view,
        swizzle,
        integer,
        components,
        faces,
        texels,
        twin,
    }))
    .cast()
}

/// `pipe_context::sampler_view_destroy`.
pub unsafe extern "C" fn sampler_view_destroy<B: Backend>(
    pipe: *mut pipe_context,
    view: *mut pipe_sampler_view,
) {
    // SAFETY: Mesa passes this driver's context and a view whose last reference went.
    let (context, view) = unsafe {
        (
            Context::<B>::of(pipe),
            Box::from_raw(view.cast::<View<B>>()),
        )
    };
    for made in [Some(view.view), view.faces]
        .into_iter()
        .flatten()
        .flatten()
    {
        context.screen().retire(Object::TextureView(made));
    }
    for texture in [view.texels.map(|t| t.texture), view.twin.map(|t| t.texture)]
        .into_iter()
        .flatten()
    {
        context.screen().retire(Object::Texture(texture));
    }
    // SAFETY: the view held one reference on its texture.
    unsafe { resource::release(view.base.texture) };
}

/// One level's region of a texture at a box's corner.
pub fn corner<'a, B: Backend>(
    texture: &'a B::Texture,
    three_d: bool,
    level: c_uint,
    x: c_uint,
    y: c_uint,
    z: c_uint,
) -> TextureRegion<'a, B> {
    TextureRegion {
        texture,
        mip: level,
        layer: if three_d { 0 } else { z },
        origin: Origin {
            x,
            y,
            z: if three_d { z } else { 0 },
        },
        stencil: false,
    }
}

/// `pipe_context::resource_copy_region`.
#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
pub unsafe extern "C" fn resource_copy_region<B: Backend>(
    pipe: *mut pipe_context,
    dst: *mut pipe_resource,
    dst_level: c_uint,
    dst_x: c_uint,
    dst_y: c_uint,
    dst_z: c_uint,
    src: *mut pipe_resource,
    src_level: c_uint,
    area: *const pipe_box,
) {
    // SAFETY: Mesa passes this driver's context, two of its resources and a box of the source.
    let (context, dst, src, area) = unsafe {
        (
            Context::<B>::of(pipe),
            Resource::<B>::of(dst),
            Resource::<B>::of(src),
            *area,
        )
    };
    let mut backend = context.screen().backend();
    if let Err(error) = context.tracker.close(&mut backend) {
        eprintln!(
            "{}: closing a pass: {error:?}",
            DRIVER_NAME.to_string_lossy()
        );
    }
    match (src.buffer(), dst.buffer(), src.texture(), dst.texture()) {
        (Some(from), Some(to), _, _) => {
            backend.copy_buffer(from, area.x as u64, to, u64::from(dst_x), area.width as u64)
        }
        (_, _, Some((from, from_desc)), Some((to, to_desc))) => backend.copy_texture(
            &corner::<B>(
                from,
                from_desc.dimension == TextureDimension::D3,
                src_level,
                area.x as u32,
                area.y as u32,
                area.z as u32,
            ),
            &corner::<B>(
                to,
                to_desc.dimension == TextureDimension::D3,
                dst_level,
                dst_x,
                dst_y,
                dst_z,
            ),
            Extent {
                width: area.width as u32,
                height: area.height as u32,
                depth_or_layers: area.depth as u32,
            },
        ),
        _ => crate::state::refuse(format_args!("a copy between a buffer and a texture")),
    }
}

/// `pipe_context::generate_mipmap`: levels after `base` filled from the one above, in `format`;
/// false when the device cannot, and st/mesa then makes them its own way.
pub unsafe extern "C" fn generate_mipmap<B: Backend>(
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    format: pipe_format,
    base: c_uint,
    last: c_uint,
    first_layer: c_uint,
    last_layer: c_uint,
) -> bool {
    // SAFETY: Mesa passes this driver's context and a texture of it.
    let (context, resource) = unsafe { (Context::<B>::of(pipe), Resource::<B>::of(resource)) };
    let (Some((texture, desc)), Some(values)) = (resource.texture(), crate::format::format(format))
    else {
        return false;
    };
    let mut backend = context.screen().backend();
    if context.tracker.close(&mut backend).is_err() {
        return false;
    }
    backend
        .generate_mips(
            texture,
            desc,
            values,
            base + 1..last + 1,
            first_layer..last_layer + 1,
        )
        .is_ok()
}
