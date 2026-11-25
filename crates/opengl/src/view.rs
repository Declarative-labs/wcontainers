//! Sampler views on the interface's texture views, and the copies and mipmaps between textures.

use std::ffi::c_uint;
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, CopyBufferTexture, Extent, Format, FormatKind, MapIntent,
    Object, Origin, STORAGE_WORD, SlowPath, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, ViewDimension,
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
    /// For an integer cube map, a 2D array of its faces, which programs load from.
    faces: Option<Result<B::TextureView, &'static str>>,
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

    /// The 2D array of an integer cube map's faces, or none for another view.
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

/// A texture of an image's own format that the image is bound through, where the texture it
/// names is of another format of the same texel size (`ReinterpretCopy`): copied into before the
/// work and, for an image the program writes, back after it, through `buffer`.
struct Twin<B: Backend> {
    texture: B::Texture,
    buffer: B::Buffer,
    source: B::Texture,
    mip: u32,
    layer: u32,
    extent: Extent,
    row_pitch: u32,
    writable: bool,
}

/// The views a binding of images made for one draw or dispatch, and the twins they were bound
/// through; given back by [`Images::finish`] after the work.
pub struct Images<B: Backend> {
    pub views: Vec<B::TextureView>,
    twins: Vec<Twin<B>>,
}

impl<B: Backend> Default for Images<B> {
    fn default() -> Self {
        Images {
            views: Vec::new(),
            twins: Vec::new(),
        }
    }
}

impl<B: Backend> Images<B> {
    /// Copies each twin the program wrote back into its texture, then destroys what was made.
    pub fn finish(self, backend: &mut B) {
        for view in self.views {
            backend.destroy(Object::TextureView(view));
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
        let region = |texture, mip, layer| TextureRegion {
            texture,
            mip,
            layer,
            origin: Origin::default(),
            stencil: false,
        };
        let (from, to) = if into {
            (
                region(&self.source, self.mip, self.layer),
                region(&self.texture, 0, 0),
            )
        } else {
            (
                region(&self.texture, 0, 0),
                region(&self.source, self.mip, self.layer),
            )
        };
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
    let (dimension, base_layer, layer_count) = if range.single_layer_view {
        if three_d {
            return refused(format_args!(
                "one slice of a volume texture as a flat image"
            ));
        }
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
    let (viewed, base_mip, base_layer) = if format == desc.format {
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
            depth_or_layers: if three_d {
                size(desc.depth_or_layers)
            } else {
                layer_count
            },
        };
        let row_pitch = extent.width * bytes;
        let twin = backend
            .create_texture(&TextureDesc {
                dimension: desc.dimension,
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

/// The interface's view of a Gallium sampler view, or why there is none; with `cube_faces`, a
/// cube map's as a 2D array of its faces.
fn texture_view<B: Backend>(
    backend: &mut B,
    resource: &Resource<B>,
    view: &pipe_sampler_view,
    cube_faces: bool,
) -> Result<B::TextureView, &'static str> {
    let (texture, desc) = resource
        .texture()
        .ok_or("a texel buffer (the screen reports none)")?;
    let stencil = crate::format::stencil_view(view.format());
    let format = stencil
        .or_else(|| crate::format::format(view.format()))
        .ok_or("a view format the device lacks")?;
    // SAFETY: the view of a texture holds its texture range.
    let range = unsafe { view.u.tex };
    let (first_layer, last_layer) = (range.first_layer(), range.last_layer());
    let (first_level, last_level) = (range.first_level(), range.last_level());
    // SAFETY: a live view.
    let dimension = match unsafe { wc_sampler_view_target(view) } {
        PIPE_TEXTURE_3D => ViewDimension::D3,
        PIPE_TEXTURE_CUBE if cube_faces => ViewDimension::D2Array,
        PIPE_TEXTURE_CUBE => ViewDimension::Cube,
        PIPE_TEXTURE_CUBE_ARRAY => ViewDimension::CubeArray,
        PIPE_TEXTURE_1D_ARRAY | PIPE_TEXTURE_2D_ARRAY => ViewDimension::D2Array,
        _ => ViewDimension::D2,
    };
    let three_d = desc.dimension == TextureDimension::D3;
    backend
        .create_view(
            texture,
            &TextureViewDesc {
                format,
                dimension,
                base_mip: first_level,
                mip_count: last_level - first_level + 1,
                base_layer: if three_d { 0 } else { first_layer },
                // A view of one layer reads only its first layer, whatever range Gallium gives
                // it (st/mesa and the blitter give a cube's or an array's whole range).
                layer_count: if three_d || dimension == ViewDimension::D2 {
                    1
                } else {
                    last_layer - first_layer + 1
                },
                stencil: stencil.is_some(),
            },
        )
        .map_err(|_| "a view the device does not make")
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
    let view = texture_view::<B>(&mut backend, resource, &base, false);
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
    let cube = unsafe { wc_sampler_view_target(&base) } == PIPE_TEXTURE_CUBE;
    let faces = (integer && cube).then(|| texture_view::<B>(&mut backend, resource, &base, true));
    Box::into_raw(Box::new(View::<B> {
        base,
        view,
        swizzle,
        integer,
        components,
        faces,
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
    // SAFETY: the view held one reference on its texture.
    unsafe { resource::release(view.base.texture) };
}

/// One level's region of a texture at a box's corner.
fn corner<'a, B: Backend>(
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
