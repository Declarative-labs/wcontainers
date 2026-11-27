//! Sampler views on the interface's texture views, as the Gallium trait's `SamplerView` kind: made
//! from a resource and the template st/mesa gives (`create_sampler_view`), given back to the back end
//! when the last handle goes.

use std::sync::Arc;

use naga::VectorSize;
use wcontainers_gallium::{SamplerViewTemplate, values};
use wcontainers_gpu::{
    Gpu, Object, caps::Features, format::FORMAT_FACTS, format::Format, format::FormatKind,
    reflect::TEXEL_BUFFER_TEXTURE_WIDTH, resource::CopyBufferTexture, resource::Extent,
    resource::Origin, resource::TextureDesc, resource::TextureDimension, resource::TextureRegion,
    resource::TextureUsage, resource::TextureViewDesc, resource::ViewDimension, slow::SlowPath,
};
use wcontainers_nir::mesa::Swizzle;

use crate::pipe::*;
use crate::resources::Resource;
use crate::states::{Retired, retire};

/// One sampler view: the resource it views and its template, then the interface's view, or why
/// there is none.
pub struct SamplerView<B: Gpu> {
    resource: Arc<Resource<B>>,
    retired: Retired<B>,
    /// The template st/mesa made it from.
    pub template: SamplerViewTemplate,
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
    /// The kind of value programs sample from it.
    pub sampled: naga::ScalarKind,
    /// Its dimension and samples, which a stand-in of another kind takes.
    pub dimension: ViewDimension,
    pub samples: u32,
    /// For a view of a multisampled array, the views of its layers after the first, a texture each
    /// ([`Storage::Layers`](crate::resources::Storage::Layers)), which `view` is the first of.
    layers: Option<Vec<B::TextureView>>,
}

/// A texture in a sampler view's format, of its levels and layers, copied from the view's texture
/// before each work that reads it (`ReinterpretCopy`): the interface views a texture only in its
/// own format or its sRGB twin unless the back end reinterprets.
struct ViewTwin<B: Gpu> {
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
pub struct TexelCopy<B: Gpu> {
    texture: B::Texture,
    /// Where the view starts in its buffer, in bytes.
    offset: u64,
    /// The view's elements, which programs read up to.
    pub length: u32,
    texel_bytes: u32,
}

impl<B: Gpu> Drop for SamplerView<B> {
    /// The views and textures it made go when the device is next used; its resource goes with its
    /// last handle.
    fn drop(&mut self) {
        let views = [self.view.as_ref().ok(), self.faces.as_ref().and_then(|f| f.as_ref().ok())]
            .into_iter()
            .flatten()
            .chain(self.layers.iter().flatten());
        for made in views {
            retire(&self.retired, Object::TextureView(made.clone()));
        }
        let textures = [
            self.texels.as_ref().map(|t| &t.texture),
            self.twin.as_ref().map(|t| &t.texture),
        ];
        for texture in textures.into_iter().flatten() {
            retire(&self.retired, Object::Texture(texture.clone()));
        }
    }
}

/// The NIR route's swizzle of a Gallium one.
fn swizzle(s: values::Swizzle) -> Swizzle {
    match s.0 {
        PIPE_SWIZZLE_X => Swizzle::X,
        PIPE_SWIZZLE_Y => Swizzle::Y,
        PIPE_SWIZZLE_Z => Swizzle::Z,
        PIPE_SWIZZLE_W => Swizzle::W,
        PIPE_SWIZZLE_1 => Swizzle::One,
        _ => Swizzle::Zero,
    }
}

/// One level's region of a texture at a box's corner.
fn corner<B: Gpu>(texture: &B::Texture, three_d: bool, level: u32, z: u32) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: level,
        layer: if three_d { 0 } else { z },
        origin: Origin {
            x: 0,
            y: 0,
            z: if three_d { z } else { 0 },
        },
        stencil: false,
    }
}

impl<B: Gpu> SamplerView<B> {
    /// The resource it views.
    pub fn resource(&self) -> &Arc<Resource<B>> {
        &self.resource
    }

    /// The view a program samples, or why the driver could not make it.
    pub fn view(&self) -> Result<&B::TextureView, String> {
        self.view.as_ref().map_err(|why| (*why).to_owned())
    }

    /// Whether the view reads a copy, which [`SamplerView::copy_texels`] makes (outside a pass).
    pub fn copies(&self) -> bool {
        self.twin.is_some() || self.texels.is_some()
    }

    /// Copies what the view reads into the texture it views, where that is a copy: a texel buffer
    /// view's range, or a twin's levels and layers, as they are now; nothing for another view.
    pub fn copy_texels(&self, backend: &mut B) -> Result<(), String> {
        if let Some(twin) = &self.twin {
            let (source, desc) = self
                .resource
                .texture()
                .ok_or("a twin of a buffer's view")?;
            backend.record_slow_path(SlowPath::ReinterpretCopy);
            for level in 0..twin.levels {
                let mip = twin.first_level + level;
                let side = |n: u32| (n >> mip).max(1);
                let depth_or_layers = if twin.three_d {
                    side(desc.depth_or_layers)
                } else {
                    twin.layers
                };
                backend.copy_texture(
                    &corner::<B>(source, twin.three_d, mip, twin.first_layer),
                    &corner::<B>(&twin.texture, twin.three_d, level, 0),
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
        let buffer = self
            .resource
            .buffer()
            .ok_or("a texel buffer view of a texture")?;
        backend.record_slow_path(SlowPath::TexelBufferAsTexture);
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

    /// The layers a view of a multisampled array sees, which programs read a texture each
    /// (`VariantKey::texture_layers`); none for another view.
    pub fn layer_count(&self) -> Option<u32> {
        self.layers.as_ref().map(|rest| rest.len() as u32 + 1)
    }

    /// The view of layer `n` (from 1) of a view of a multisampled array.
    pub fn layer_view(&self, n: u32) -> Option<&B::TextureView> {
        self.layers.as_ref()?.get(n.checked_sub(1)? as usize)
    }

    /// The 2D array of an integer cube map's or cube map array's faces, or none for another view.
    pub fn faces(&self) -> Option<Result<&B::TextureView, String>> {
        self.faces
            .as_ref()
            .map(|faces| faces.as_ref().map_err(|why| (*why).to_owned()))
    }
}

/// The interface's dimension of a Gallium sampler view's target; with `cube_faces`, a cube map's or
/// cube map array's as a 2D array of its faces.
fn view_dimension(target: values::TextureTarget, cube_faces: bool) -> ViewDimension {
    match target.0 {
        PIPE_TEXTURE_3D => ViewDimension::D3,
        PIPE_TEXTURE_CUBE | PIPE_TEXTURE_CUBE_ARRAY if cube_faces => ViewDimension::D2Array,
        PIPE_TEXTURE_CUBE => ViewDimension::Cube,
        PIPE_TEXTURE_CUBE_ARRAY => ViewDimension::CubeArray,
        PIPE_TEXTURE_1D_ARRAY | PIPE_TEXTURE_2D_ARRAY => ViewDimension::D2Array,
        _ => ViewDimension::D2,
    }
}

/// What a Gallium sampler view of a texture sees: its format (a stencil view's, read as
/// unsigned integers), its dimension (with `cube_faces`, a cube map's or cube map array's as a 2D
/// array of its faces), its levels and its layers.
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
        template: &SamplerViewTemplate,
        desc: &TextureDesc,
        cube_faces: bool,
    ) -> Result<Self, &'static str> {
        let stencil = crate::format::stencil_view(template.format.0);
        let format = stencil
            .or_else(|| crate::format::format(template.format.0))
            .ok_or("a view format the device lacks")?;
        let range = template.u.tex;
        let dimension = view_dimension(template.target, cube_faces);
        let three_d = desc.dimension == TextureDimension::D3;
        let levels = range
            .last_level
            .checked_sub(range.first_level)
            .ok_or("a view whose last level is before its first")?
            + 1;
        let layers = range
            .last_layer
            .checked_sub(range.first_layer)
            .ok_or("a view whose last layer is before its first")?
            + 1;
        Ok(Self {
            format,
            stencil: stencil.is_some(),
            dimension,
            first_level: range.first_level,
            levels,
            first_layer: if three_d { 0 } else { range.first_layer },
            // A view of one layer reads only its first layer, whatever range Gallium gives it
            // (st/mesa and the blitter give a cube's or an array's whole range).
            layers: if three_d || dimension == ViewDimension::D2 {
                1
            } else {
                layers
            },
        })
    }
}

/// The twin a sampler view of a texture is read through, where the back end cannot view the
/// texture in the view's format; none where it can.
fn view_twin<B: Gpu>(
    backend: &mut B,
    resource: &Resource<B>,
    template: &SamplerViewTemplate,
) -> Result<Option<ViewTwin<B>>, &'static str> {
    let Some((_, desc)) = resource.texture() else {
        return Ok(None);
    };
    let seen = Seen::of(template, desc, false)?;
    let reinterprets = desc.reinterpret
        && backend
            .caps()
            .features
            .contains(Features::FORMAT_REINTERPRET);
    if seen.stencil
        || FORMAT_FACTS[seen.format as usize].linear == FORMAT_FACTS[desc.format as usize].linear
        || reinterprets
    {
        return Ok(None);
    }
    if FORMAT_FACTS[seen.format as usize].block != FORMAT_FACTS[desc.format as usize].block
        || FORMAT_FACTS[seen.format as usize].depth_stencil
        || FORMAT_FACTS[desc.format as usize].depth_stencil
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
fn texture_view<B: Gpu>(
    backend: &mut B,
    resource: &Resource<B>,
    template: &SamplerViewTemplate,
    twin: Option<&ViewTwin<B>>,
    cube_faces: bool,
) -> Result<B::TextureView, &'static str> {
    let (texture, desc) = resource
        .texture()
        .ok_or("a texel buffer (the screen reports none)")?;
    let seen = Seen::of(template, desc, cube_faces)?;
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

/// The interface's views of the layers a Gallium sampler view of a multisampled array sees, a
/// texture each, first first, or why there are none.
fn layer_views<B: Gpu>(
    backend: &mut B,
    resource: &Resource<B>,
    template: &SamplerViewTemplate,
) -> Result<Vec<B::TextureView>, &'static str> {
    let (_, desc) = resource
        .layer(0)
        .0
        .texture()
        .ok_or("a multisampled array without layers")?;
    let seen = Seen::of(template, desc, false)?;
    if !seen.stencil
        && FORMAT_FACTS[seen.format as usize].linear != FORMAT_FACTS[desc.format as usize].linear
    {
        return Err("a multisampled texture viewed in another format (not built)");
    }
    let mut views = Vec::new();
    for layer in seen.first_layer..seen.first_layer + seen.layers {
        let made = resource
            .layer(layer)
            .0
            .texture()
            .ok_or("a view of layers past its texture's")
            .and_then(|(texture, _)| {
                backend
                    .create_view(
                        texture,
                        &TextureViewDesc {
                            format: seen.format,
                            dimension: ViewDimension::D2,
                            base_mip: seen.first_level,
                            mip_count: seen.levels,
                            base_layer: 0,
                            layer_count: 1,
                            stencil: seen.stencil,
                        },
                    )
                    .map_err(|_| "a view the device does not make")
            });
        match made {
            Ok(made) => views.push(made),
            Err(why) => {
                for made in views {
                    backend.destroy(Object::TextureView(made));
                }
                return Err(why);
            }
        }
    }
    Ok(views)
}

/// A texel buffer view's texture and the interface's view of it, or why there are none.
fn texel_buffer<B: Gpu>(
    backend: &mut B,
    template: &SamplerViewTemplate,
) -> Result<(TexelCopy<B>, B::TextureView), &'static str> {
    let format = crate::format::format(template.format.0)
        .ok_or("a texel buffer format the device lacks")?;
    let (_, _, texel_bytes) = FORMAT_FACTS[format as usize].block;
    let range = template.u.buf;
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
    let viewed = backend.create_view(
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
    );
    let viewed = match viewed {
        Ok(viewed) => viewed,
        Err(_) => {
            backend.destroy(Object::Texture(texture));
            return Err("a texel buffer's view the device does not make");
        }
    };
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

/// `pipe_context::create_sampler_view`: the view of `resource` the template describes. A view the
/// back end cannot make is still made, holding why, which binding it reports.
pub fn create<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    resource: &Arc<Resource<B>>,
    template: &SamplerViewTemplate,
) -> SamplerView<B> {
    let mut layers = None;
    let (view, texels, twin) = if !resource.layers().is_empty() {
        match layer_views::<B>(backend, resource, template) {
            Ok(mut views) if !views.is_empty() => {
                let first = views.remove(0);
                layers = Some(views);
                (Ok(first), None, None)
            }
            Ok(_) => (Err("a view of no layers"), None, None),
            // Programs are converted for one layer, and binding it says why there is none.
            Err(why) => {
                layers = Some(Vec::new());
                (Err(why), None, None)
            }
        }
    } else if resource.buffer().is_some() {
        match texel_buffer::<B>(backend, template) {
            Ok((texels, view)) => (Ok(view), Some(texels), None),
            Err(why) => (Err(why), None, None),
        }
    } else {
        match view_twin::<B>(backend, resource, template) {
            Ok(twin) => (
                texture_view::<B>(backend, resource, template, twin.as_ref(), false),
                None,
                twin,
            ),
            Err(why) => (Err(why), None, None),
        }
    };
    let opaque = crate::format::opaque_in_alpha(template.format.0);
    let swizzle = [
        template.swizzle_r,
        template.swizzle_g,
        template.swizzle_b,
        template.swizzle_a,
    ]
    .map(|s| match swizzle(s) {
        Swizzle::W if opaque => Swizzle::One,
        s => s,
    });
    let stencil = crate::format::stencil_view(template.format.0);
    let format = stencil.or_else(|| crate::format::format(template.format.0));
    // A stencil view's texels are unsigned integers, as a stencil texture's are.
    let integer = stencil.is_some()
        || format.is_some_and(|f| {
            matches!(
                FORMAT_FACTS[f as usize].kind,
                FormatKind::Uint | FormatKind::Sint | FormatKind::Stencil
            )
        });
    let components = format.map_or(0, |f| FORMAT_FACTS[f as usize].components);
    let sampled = match format.map(|f| FORMAT_FACTS[f as usize].kind) {
        Some(FormatKind::Sint) => naga::ScalarKind::Sint,
        _ if integer => naga::ScalarKind::Uint,
        _ => naga::ScalarKind::Float,
    };
    let cube = matches!(
        template.target.0,
        PIPE_TEXTURE_CUBE | PIPE_TEXTURE_CUBE_ARRAY
    );
    let samples = u32::from(resource.template.nr_samples).max(1);
    let faces = (integer && cube)
        .then(|| texture_view::<B>(backend, resource, template, twin.as_ref(), true));
    SamplerView {
        resource: resource.clone(),
        retired: retired.clone(),
        template: *template,
        view,
        swizzle,
        integer,
        components,
        faces,
        texels,
        twin,
        sampled,
        // A multisampled array's views are of its layers' 2D textures.
        dimension: if layers.is_some() {
            ViewDimension::D2
        } else {
            view_dimension(template.target, false)
        },
        samples,
        layers,
    }
}
