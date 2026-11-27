//! The framebuffer st/mesa sets and its clears, on the Gallium trait: each surface kept as the
//! resource it names (a counted handle) with its format, level and layers, and the pass tracker's
//! targets made from them. A target in another format than its texture's, which the host views
//! only in its own format, draws into a twin of its format ([`Twin`]).

use std::sync::Arc;

use wcontainers_gallium::{FramebufferState, Surface};
use wcontainers_gpu::{
    Gpu, MAX_COLOR_TARGETS, Object, STORAGE_WORD, caps::Features, draw::EmptyPass,
    format::FORMAT_FACTS, format::Format, passes::ColorTarget, passes::PassTargets,
    passes::PassTracker, passes::Target, resource::BufferDesc, resource::BufferUsage,
    resource::CopyBufferTexture, resource::Extent, resource::MapIntent, resource::Origin,
    resource::TextureDesc, resource::TextureDimension, resource::TextureRegion,
    resource::TextureUsage, resource::TextureViewDesc, resource::ViewDimension, slow::SlowPath,
};

use crate::guest::refuse;
use crate::resources::Resource;

/// One surface of the framebuffer: the resource it names and what of it is drawn into.
pub struct Attached<B: Gpu> {
    pub resource: Arc<Resource<B>>,
    /// Gallium's format the surface is seen in.
    pub format: u32,
    pub level: u32,
    pub first_layer: u32,
    pub last_layer: u32,
}

impl<B: Gpu> Attached<B> {
    /// The surface of a texture, none for one naming no resource.
    pub fn of(surface: &Surface<'_, Resource<B>>) -> Option<Self> {
        surface.texture.map(|resource| Attached {
            resource: Arc::clone(resource),
            format: surface.format.0,
            level: surface.level,
            first_layer: surface.first_layer,
            last_layer: surface.last_layer,
        })
    }
}

/// What the framebuffer holds: its surfaces, its size where it has none, and the views and twins
/// the pass tracker draws into.
pub struct Framebuffer<B: Gpu> {
    pub colors: Vec<Option<Attached<B>>>,
    pub depth: Option<Attached<B>>,
    pub width: u32,
    pub height: u32,
    /// Its layers where it has no surface.
    pub layers: u32,
    /// Its samples where it has no surface.
    pub samples: u32,
    pub views: Vec<B::TextureView>,
    /// The samples of each pixel its targets have.
    pub target_samples: u32,
    /// The twins its targets in another format than their textures' draw into, copied into before
    /// each draw and back after it and after each clear.
    pub twins: Vec<Twin<B>>,
}

impl<B: Gpu> Default for Framebuffer<B> {
    fn default() -> Self {
        Framebuffer {
            colors: Vec::new(),
            depth: None,
            width: 0,
            height: 0,
            layers: 0,
            samples: 0,
            views: Vec::new(),
            target_samples: 1,
            twins: Vec::new(),
        }
    }
}

impl<B: Gpu> Framebuffer<B> {
    /// The framebuffer st/mesa set, its surfaces held, with no views yet.
    pub fn of(state: &FramebufferState<'_, Resource<B>>) -> Self {
        let colors = usize::from(state.nr_cbufs).min(MAX_COLOR_TARGETS);
        Framebuffer {
            colors: state.cbufs[..colors].iter().map(Attached::of).collect(),
            depth: Attached::of(&state.zsbuf),
            width: state.width,
            height: state.height,
            layers: u32::from(state.layers),
            samples: u32::from(state.samples),
            ..Framebuffer::default()
        }
    }

    /// Its surfaces, colour ones first.
    fn surfaces(&self) -> impl Iterator<Item = &Attached<B>> {
        self.colors.iter().flatten().chain(self.depth.as_ref())
    }

    /// Whether one of its targets is of `resource`.
    pub fn draws_into(&self, resource: &Arc<Resource<B>>) -> bool {
        self.surfaces().any(|s| Arc::ptr_eq(&s.resource, resource))
    }

    /// The layers a draw naming layers and a clear go through one at a time: the fewest its
    /// targets have (OpenGL leaves a layer past any target's undefined), or its own count without
    /// targets; one where it is not layered.
    pub fn layer_count(&self) -> u32 {
        self.surfaces()
            .map(|s| s.last_layer.saturating_sub(s.first_layer) + 1)
            .min()
            .unwrap_or(self.layers)
            .max(1)
    }

    /// Copies each target's twin into its texture, after the work drawing into them (outside a
    /// pass: the open one is closed).
    pub fn copy_back_twins(
        &self,
        tracker: &mut PassTracker<B>,
        backend: &mut B,
    ) -> Result<(), String> {
        if self.twins.is_empty() {
            return Ok(());
        }
        tracker
            .close(backend)
            .map_err(|e| format!("closing the pass for a target's twin: {e:?}"))?;
        for twin in &self.twins {
            twin.copy(backend, false);
        }
        Ok(())
    }

    /// Copies each target's texture into its twin, before work drawing into them (outside a pass:
    /// the open one is closed).
    pub fn copy_into_twins(
        &self,
        tracker: &mut PassTracker<B>,
        backend: &mut B,
    ) -> Result<(), String> {
        if self.twins.is_empty() {
            return Ok(());
        }
        tracker
            .close(backend)
            .map_err(|e| format!("closing the pass for a target's twin: {e:?}"))?;
        for twin in &self.twins {
            twin.copy(backend, true);
        }
        Ok(())
    }

    /// The pass tracker's targets of layer `run` of each surface (its first layer and on), the
    /// views made for them and their samples; targets in another format than their textures'
    /// draw into twins, added to `twins`.
    pub fn layer_targets(
        &self,
        backend: &mut B,
        run: u32,
        twins: &mut Vec<Twin<B>>,
    ) -> (PassTargets<B>, Vec<B::TextureView>, u32) {
        let mut targets = PassTargets {
            colors: [const { None }; MAX_COLOR_TARGETS],
            depth: None,
            empty: None,
        };
        let mut views = Vec::new();
        let mut samples = self.samples.max(1);
        for (slot, surface) in targets.colors.iter_mut().zip(&self.colors) {
            let Some(made) = surface
                .as_ref()
                .and_then(|s| target(backend, s, run, twins))
            else {
                continue;
            };
            views.push(made.view.clone());
            samples = made.samples;
            *slot = Some(ColorTarget {
                target: made,
                resolve: None,
            });
        }
        if let Some(made) = self
            .depth
            .as_ref()
            .and_then(|s| target(backend, s, run, twins))
        {
            views.push(made.view.clone());
            samples = made.samples;
            targets.depth = Some(made);
        }
        if views.is_empty() && self.width > 0 && self.height > 0 {
            targets.empty = Some(EmptyPass {
                width: self.width,
                height: self.height,
                samples,
            });
        }
        (targets, views, samples)
    }

    /// Gives the views and twins it made to `release`.
    pub fn release(&mut self, mut release: impl FnMut(Object<B>)) {
        for view in std::mem::take(&mut self.views) {
            release(Object::TextureView(view));
        }
        for twin in std::mem::take(&mut self.twins) {
            twin.release(&mut release);
        }
    }
}

/// Whether a colour target in `format` of a texture `desc` describes is drawn into through a twin:
/// the host views a texture only in its own format or its sRGB twin unless it reinterprets.
fn through_twin<B: Gpu>(backend: &B, format: Format, desc: &TextureDesc) -> bool {
    let reinterprets = desc.reinterpret
        && backend
            .caps()
            .features
            .contains(Features::FORMAT_REINTERPRET);
    let colour = !FORMAT_FACTS[format as usize].depth_stencil
        && !FORMAT_FACTS[desc.format as usize].depth_stencil;
    colour
        && FORMAT_FACTS[format as usize].linear != FORMAT_FACTS[desc.format as usize].linear
        && !reinterprets
}

/// A view to draw into of layer `layer` of one surface's texture, with the format it is seen in;
/// none for a surface of no texture or of a format the device lacks.
pub fn surface_view<B: Gpu>(
    backend: &mut B,
    surface: &Attached<B>,
    layer: u32,
) -> Option<(B::TextureView, Format)> {
    let (resource, layer) = surface.resource.layer(layer);
    let (texture, desc) = resource.texture()?;
    let format = crate::format::format(surface.format)?;
    if through_twin(backend, format, desc) {
        refuse(format_args!(
            "a target in {format:?} of a texture in {:?}, which the host views only in its own \
             format, cleared alone (not built)",
            desc.format
        ));
    }
    let three_d = desc.dimension == TextureDimension::D3;
    let view = backend
        .create_view(
            texture,
            &TextureViewDesc {
                format,
                dimension: if three_d {
                    ViewDimension::D3
                } else {
                    ViewDimension::D2
                },
                base_mip: surface.level,
                mip_count: 1,
                base_layer: if three_d { 0 } else { layer },
                layer_count: 1,
                stencil: false,
            },
        )
        .ok()?;
    Some((view, format))
}

/// The pass tracker's target of layer `run` of one surface (its first layer and on), with the view
/// made for it; a target in another format than its texture's draws into a twin, added to
/// `twins`.
fn target<B: Gpu>(
    backend: &mut B,
    surface: &Attached<B>,
    run: u32,
    twins: &mut Vec<Twin<B>>,
) -> Option<Target<B>> {
    let first = surface.first_layer + run;
    let (resource, layer) = surface.resource.layer(first);
    let (texture, desc) = resource.texture()?;
    let size = |n: u32| (n >> surface.level).max(1);
    let format = crate::format::format(surface.format)?;
    if through_twin(backend, format, desc) {
        let twin = Twin::of_target(backend, texture, desc, format, surface.level, layer)
            .unwrap_or_else(|why| refuse(format_args!("{why} (not built)")));
        let view = backend
            .create_view(
                &twin.texture,
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
            .ok()?;
        let target = Target {
            view,
            texture: twin.texture.clone(),
            format,
            mip: 0,
            layer: 0,
            depth_slice: 0,
            width: size(desc.width),
            height: size(desc.height),
            samples: 1,
        };
        twins.push(twin);
        return Some(target);
    }
    let (view, format) = surface_view(backend, surface, first)?;
    let three_d = desc.dimension == TextureDimension::D3;
    Some(Target {
        view,
        texture: texture.clone(),
        format,
        mip: surface.level,
        layer: if three_d { 0 } else { layer },
        depth_slice: if three_d { layer } else { 0 },
        width: size(desc.width),
        height: size(desc.height),
        samples: desc.samples,
    })
}

/// A texture of a target's own format that the target draws into, where the texture it names is
/// of another format of the same texel size (`ReinterpretCopy`): copied into before the work and
/// back after it, through `buffer`.
pub struct Twin<B: Gpu> {
    texture: B::Texture,
    buffer: B::Buffer,
    source: B::Texture,
    mip: u32,
    layer: u32,
    /// The depth slice of a volume texture's.
    slice: u32,
    extent: Extent,
    row_pitch: u32,
}

impl<B: Gpu> Twin<B> {
    /// A twin in `format` of one level and layer (a volume texture's depth slice) of `source`,
    /// whose texture `desc` describes; why not where the formats' texels differ in size or the
    /// texture is multisampled (whose texels the host does not copy through buffers).
    fn of_target(
        backend: &mut B,
        source: &B::Texture,
        desc: &TextureDesc,
        format: Format,
        mip: u32,
        layer: u32,
    ) -> Result<Self, String> {
        let (width, _, bytes) = FORMAT_FACTS[format as usize].block;
        let (their_width, _, theirs) = FORMAT_FACTS[desc.format as usize].block;
        if bytes != theirs || width != 1 || their_width != 1 {
            return Err(format!(
                "a target in {format:?} of a texture in {:?}, of another texel size",
                desc.format
            ));
        }
        if desc.samples > 1 {
            return Err(format!(
                "a multisampled target in {format:?} of a texture in {:?}",
                desc.format
            ));
        }
        backend.record_slow_path(SlowPath::ReinterpretCopy);
        let side = |n: u32| (n >> mip).max(1);
        let extent = Extent {
            width: side(desc.width),
            height: side(desc.height),
            depth_or_layers: 1,
        };
        let row_pitch = extent.width * bytes;
        let texture = backend
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: extent.width,
                height: extent.height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .map_err(|e| format!("a target's twin: {e:?}"))?;
        let buffer = backend
            .create_buffer(&BufferDesc {
                size: (u64::from(row_pitch) * u64::from(extent.height))
                    .next_multiple_of(STORAGE_WORD),
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .map_err(|e| format!("a target's copy: {e:?}"))?;
        let three_d = desc.dimension == TextureDimension::D3;
        Ok(Twin {
            texture,
            buffer,
            source: source.clone(),
            mip,
            layer: if three_d { 0 } else { layer },
            slice: if three_d { layer } else { 0 },
            extent,
            row_pitch,
        })
    }

    /// Gives what the twin made to `release`.
    fn release(self, release: &mut impl FnMut(Object<B>)) {
        release(Object::Texture(self.texture));
        release(Object::Buffer(self.buffer));
    }

    /// Moves the texels between the twin and its source (outside a pass): into the twin, or back
    /// from it.
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
