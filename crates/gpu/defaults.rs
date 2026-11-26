//! Methods of the interface's own types that the provided methods of [`crate::Gpu`] call.

use crate::caps::{Features, FormatUse};
use crate::draw::{
    ColorAttachment, DrawCall, DrawRange, LoadOp, PassDesc, ScissorRect, StoreOp, Viewport,
};
use crate::error::GpuError;
use crate::format::{Format, FormatKind};
use crate::helpers::{Builtin, Builtins, Helpers, MipRead, PARAMS_SIZE};
use crate::resource::{
    BindingEntry, BindingResource, BufferRange, Extent, Origin, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, ViewDimension,
};
use crate::shader::StageSet;
use crate::slow::SlowPath;
use crate::state::{IndexFormat, ProvokingVertex, Topology};
use crate::*;
use alloc::vec::Vec;
use core::ops::Range;

impl<B: Gpu> Helpers<B> {
    /// The default of [`Gpu::generate_mips`], for a back end whose override takes only some cases.
    pub fn generate_mips_by_draws(
        b: &mut B,
        texture: &B::Texture,
        desc: &TextureDesc,
        values: Format,
        mips: Range<u32>,
        layers: Range<u32>,
    ) -> Result<(), GpuError> {
        let own = desc.own_view();
        let three_d = own == ViewDimension::D3;
        let layers = if three_d {
            0..1
        } else {
            layers.start..layers.end.min(desc.depth_or_layers)
        };
        let mips = mips.start.max(1)..mips.end.min(desc.mip_levels);
        if mips.is_empty() || layers.is_empty() {
            return Ok(());
        }
        let built = Builtins::new(b)?;
        let decoded = Format::norm_twin(values, desc.format)?;
        // The views read and write the texels as their values: the sRGB twin is a view every back end
        // makes, so filtering happens in linear light.
        let seen = if decoded.is_some() {
            desc.format
        } else {
            values
        };
        let read = match decoded {
            Some((_, false)) => MipRead::Unorm,
            Some((_, true)) => MipRead::Snorm,
            None if b.caps().formats.uses[seen as usize].contains(FormatUse::FILTER) => {
                MipRead::Filtered
            }
            None => MipRead::Unfiltered,
        };
        let cube = matches!(own, ViewDimension::Cube | ViewDimension::CubeArray);
        let read_dim = if decoded.is_some() && cube {
            ViewDimension::D2Array
        } else {
            own
        };
        let copied = read_dim != own && !b.caps().features.contains(Features::TEXTURE_VIEWS);
        let which = Builtin::Mip(read_dim, read);
        let program = Helpers::program(b, which)?;
        for mip in mips {
            let size = |n: u32| (n >> mip).max(1);
            let (width, height) = (size(desc.width), size(desc.height));
            let slices = if three_d {
                size(desc.depth_or_layers)
            } else {
                1
            };
            let viewport = [Viewport::full(width, height)];
            let scissor = [ScissorRect {
                x: 0,
                y: 0,
                width,
                height,
            }];
            let all = if three_d { 1 } else { desc.depth_or_layers };
            let mut source_view = TextureViewDesc {
                format: seen,
                dimension: read_dim,
                base_mip: mip - 1,
                mip_count: 1,
                base_layer: 0,
                layer_count: all,
                stencil: false,
            };
            // A cube read by loads without views: the level above copied into a 2D array first.
            let copy = if copied {
                b.record_slow_path(SlowPath::ReinterpretCopy);
                let above = |n: u32| (n >> (mip - 1)).max(1);
                let extent = Extent {
                    width: above(desc.width),
                    height: above(desc.height),
                    depth_or_layers: all,
                };
                let scratch = b.create_texture(&TextureDesc {
                    dimension: TextureDimension::D2,
                    format: desc.format,
                    width: extent.width,
                    height: extent.height,
                    depth_or_layers: all,
                    mip_levels: 1,
                    samples: 1,
                    usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
                    reinterpret: false,
                    cube: false,
                })?;
                let region = |texture, mip| TextureRegion {
                    texture,
                    mip,
                    layer: 0,
                    origin: Origin { x: 0, y: 0, z: 0 },
                    stencil: false,
                };
                b.copy_texture(&region(texture, mip - 1), &region(&scratch, 0), extent);
                source_view.base_mip = 0;
                Some(scratch)
            } else {
                None
            };
            let src = b.create_view(copy.as_ref().unwrap_or(texture), &source_view)?;
            for layer in layers.clone() {
                let dst = b.create_view(
                    texture,
                    &TextureViewDesc {
                        format: seen,
                        dimension: if three_d {
                            ViewDimension::D3
                        } else {
                            ViewDimension::D2
                        },
                        base_mip: mip,
                        mip_count: 1,
                        base_layer: layer,
                        layer_count: 1,
                        stencil: false,
                    },
                )?;
                for slice in 0..slices {
                    if read != MipRead::Filtered {
                        b.record_slow_path(SlowPath::FilterInShader);
                    }
                    let ring;
                    let mut entries = alloc::vec![BindingEntry {
                        binding: 32,
                        stages: StageSet::FRAGMENT,
                        resource: BindingResource::Texture(&src),
                    }];
                    if which.uses_sampler() {
                        let sampler = match read {
                            MipRead::Filtered => &built.linear,
                            _ => &built.nearest,
                        };
                        entries.push(BindingEntry {
                            binding: 16,
                            stages: StageSet::FRAGMENT,
                            resource: BindingResource::Sampler(sampler),
                        });
                    }
                    if which.uses_params() {
                        let mut params = [0u8; PARAMS_SIZE];
                        let scale = decoded.map_or(0, |(scale, _)| scale);
                        params[0..4].copy_from_slice(&scale.to_le_bytes());
                        let w = (slice as f32 + 0.5) / slices as f32;
                        params[16..20].copy_from_slice(&w.to_bits().to_le_bytes());
                        params[24..28].copy_from_slice(&layer.to_le_bytes());
                        let align = b.caps().limits.min_uniform_offset_alignment;
                        let mut offset = [0];
                        ring =
                            b.write_buffer_ring(RingKind::Data, &[&params], align, &mut offset)?;
                        entries.push(BindingEntry {
                            binding: 0,
                            stages: StageSet::FRAGMENT,
                            resource: BindingResource::UniformBuffer(BufferRange {
                                buffer: &ring,
                                offset: offset[0],
                                size: PARAMS_SIZE as u64,
                            }),
                        });
                    }
                    let set = b.create_binding_set(&entries)?;
                    let mut pass = PassDesc {
                        colors: Default::default(),
                        depth: None,
                        empty: None,
                        occlusion: None,
                    };
                    pass.colors[0] = Some(ColorAttachment {
                        view: &dst,
                        depth_slice: slice,
                        resolve: None,
                        load: LoadOp::DontCare,
                        store: StoreOp::Store,
                    });
                    b.begin_pass(&pass);
                    b.draw(&DrawCall {
                        bindings: [Some(&set), None, None, None],
                        scissors: &scissor,
                        ..DrawCall::builtin(&built, &program, &viewport)
                    });
                    b.end_pass();
                    b.destroy(Object::BindingSet(set));
                }
                b.destroy(Object::TextureView(dst));
            }
            b.destroy(Object::TextureView(src));
            if let Some(copy) = copy {
                b.destroy(Object::Texture(copy));
            }
        }
        Ok(())
    }
}

impl Format {
    /// Which aspects a depth-stencil format has: depth, stencil.
    pub(crate) fn aspects(self) -> (bool, bool) {
        match crate::format::FORMAT_FACTS[self as usize].kind {
            FormatKind::Depth => (true, false),
            FormatKind::Stencil => (false, true),
            FormatKind::DepthStencil => (true, true),
            _ => (false, false),
        }
    }

    /// For a texture a front stores as the integer twin of a normalised format, the largest encoded value
    /// and whether it is signed; None when the texture holds its own format's values.
    pub(crate) fn norm_twin(
        values: Format,
        stored: Format,
    ) -> Result<Option<(u32, bool)>, GpuError> {
        use Format::*;
        Ok(match (values, stored) {
            _ if crate::format::FORMAT_FACTS[values as usize].linear
                == crate::format::FORMAT_FACTS[stored as usize].linear =>
            {
                None
            }
            (R16Unorm | Rg16Unorm | Rgba16Unorm, _)
                if crate::format::FORMAT_FACTS[values as usize].integer_twin == Some(stored) =>
            {
                Some((u16::MAX.into(), false))
            }
            (R16Snorm | Rg16Snorm | Rgba16Snorm, _)
                if crate::format::FORMAT_FACTS[values as usize].integer_twin == Some(stored) =>
            {
                Some((i16::MAX as u32, true))
            }
            (R8Snorm | Rg8Snorm | Rgba8Snorm, _)
                if crate::format::FORMAT_FACTS[values as usize].integer_twin == Some(stored) =>
            {
                Some((i8::MAX as u32, true))
            }
            _ => {
                return Err(GpuError::Invalid(
                    "generate_mips: values are neither the stored format, its srgb twin nor its integer twin",
                ));
            }
        })
    }
}

impl Viewport {
    /// A viewport over `width` by `height` pixels at depth zero to one.
    pub(crate) fn full(width: u32, height: u32) -> Viewport {
        Viewport {
            x: 0,
            y: 0,
            width: (width as f32).to_bits(),
            height: (height as f32).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }
    }
}

impl<'a, B: Gpu> DrawCall<'a, B> {
    /// A draw of the built-in full-screen triangle with `program`, no bindings and no depth test.
    pub(crate) fn builtin(
        b: &'a Builtins<B>,
        program: &'a B::Program,
        viewports: &'a [Viewport],
    ) -> DrawCall<'a, B> {
        DrawCall {
            program,
            blend: &b.blend,
            depth_stencil: &b.no_depth,
            raster: &b.raster,
            vertex_layout: &b.vertex_layout,
            bindings: [None; crate::MAX_BINDING_SETS],
            vertex_buffers: &[],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports,
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 3,
                first_instance: 0,
                instances: 1,
            },
        }
    }
}

impl FanOrLoop {
    /// Appends to `out`, as little-endian u32, the index list that draws a fan as a triangle list or a loop
    /// as a line list, honouring restart in guest indices, and returns the base vertex to draw it with. Each
    /// fan triangle keeps the fan's winding and puts the vertex `provoking` names where the list's own
    /// provoking vertex is: first for [`ProvokingVertex::First`] (vertex i + 1 of the fan), last for
    /// [`ProvokingVertex::Last`] (vertex i + 2).
    pub(crate) fn indices(
        self,
        source: IndexSource<'_>,
        restart: bool,
        provoking: ProvokingVertex,
        out: &mut Vec<u8>,
    ) -> i32 {
        // Generic over the index width, so each width gets its own loop with no call per index.
        fn walk(
            kind: FanOrLoop,
            provoking: ProvokingVertex,
            values: impl Iterator<Item = u32>,
            restart_value: Option<u32>,
            out: &mut Vec<u8>,
        ) {
            let mut push = |v: [u32; 3], n: usize| {
                for i in &v[..n] {
                    out.extend_from_slice(&i.to_le_bytes());
                }
            };
            // The piece's first vertex, the previous one, and how many it has had. A loop closes when its
            // piece ends.
            let (mut first, mut prev, mut n) = (0, 0, 0usize);
            let closes = |n: usize| kind == FanOrLoop::LineLoop && n >= 2;
            for v in values {
                if Some(v) == restart_value {
                    if closes(n) {
                        push([prev, first, 0], 2);
                    }
                    n = 0;
                    continue;
                }
                match (kind, n) {
                    (_, 0) => first = v,
                    (FanOrLoop::TriangleFan, 1) => {}
                    (FanOrLoop::TriangleFan, _) => match provoking {
                        ProvokingVertex::First => push([prev, v, first], 3),
                        ProvokingVertex::Last => push([first, prev, v], 3),
                    },
                    (FanOrLoop::LineLoop, _) => push([prev, v, 0], 2),
                }
                prev = v;
                n += 1;
            }
            if closes(n) {
                push([prev, first, 0], 2);
            }
        }
        let kind = self;
        match source {
            IndexSource::Sequential { first, count } => {
                walk(
                    kind,
                    provoking,
                    first..first.saturating_add(count),
                    None,
                    out,
                );
                0
            }
            IndexSource::Guest {
                indices,
                format,
                base_vertex,
            } => {
                let restart_value =
                    restart.then(|| crate::state::INDEX_FORMATS[format as usize].restart_value);
                match format {
                    IndexFormat::U8 => walk(
                        kind,
                        provoking,
                        indices.iter().map(|&b| u32::from(b)),
                        restart_value,
                        out,
                    ),
                    IndexFormat::U16 => walk(
                        kind,
                        provoking,
                        indices
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u32::from(u16::from_le_bytes(*b))),
                        restart_value,
                        out,
                    ),
                    IndexFormat::U32 => walk(
                        kind,
                        provoking,
                        indices
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|b| u32::from_le_bytes(*b)),
                        restart_value,
                        out,
                    ),
                }
                base_vertex
            }
        }
    }
}

#[cfg(test)]
mod tests;
