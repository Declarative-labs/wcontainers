//! Sampler views on the interface's texture views, and the copies and mipmaps between textures.

use std::ffi::c_uint;
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, Extent, Object, Origin, TextureDimension, TextureRegion, TextureViewDesc,
    ViewDimension,
};

use naga::VectorSize;
use wcontainers_nir::mesa::Swizzle;

use crate::context::Context;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::screen::DRIVER_NAME;

/// One sampler view: Gallium's struct first, then the interface's view, or why there is none.
#[repr(C)]
pub struct View<B: Backend> {
    base: pipe_sampler_view,
    view: Result<B::TextureView, &'static str>,
    /// How the programs that sample it rearrange its results.
    pub swizzle: [Swizzle; VectorSize::MAX],
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

/// The interface's view of a Gallium sampler view, or why there is none.
fn texture_view<B: Backend>(
    backend: &mut B,
    resource: &Resource<B>,
    view: &pipe_sampler_view,
) -> Result<B::TextureView, &'static str> {
    let (texture, desc) = resource
        .texture()
        .ok_or("a texel buffer (the screen reports none)")?;
    let format = crate::format::format(view.format()).ok_or("a view format the device lacks")?;
    // SAFETY: the view of a texture holds its texture range.
    let range = unsafe { view.u.tex };
    let (first_layer, last_layer) = (range.first_layer(), range.last_layer());
    let (first_level, last_level) = (range.first_level(), range.last_level());
    let dimension = match view.target() {
        PIPE_TEXTURE_3D => ViewDimension::D3,
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
                stencil: false,
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
    let view = texture_view::<B>(&mut *context.screen().backend(), resource, &base);
    let swizzle = [
        base.swizzle_r(),
        base.swizzle_g(),
        base.swizzle_b(),
        base.swizzle_a(),
    ]
    .map(|s| swizzle(s as pipe_swizzle));
    Box::into_raw(Box::new(View::<B> {
        base,
        view,
        swizzle,
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
    if let Ok(made) = view.view {
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
        _ => crate::state::refuse("a copy between a buffer and a texture"),
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
