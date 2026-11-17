//! The occlusion counter (plan 4.12): exact sample counts for the APIs that ask for them, on back ends
//! whose queries answer only "any" ([`SlowPath::OcclusionCountReplay`]). The query's draws run
//! normally and are kept; at the query's end they are drawn again into a counting target cleared to
//! zero, each with its own depth-stencil and raster blocks, blending One plus One and a counting
//! program from the front (the draw's fragment shader writing 1.0 at location 0, its storage writes
//! removed), so each sample holds how many fragments passed every test there. The target's sum is
//! the count.

use crate::MAX_BINDING_SETS;
use crate::backend::Backend;
use crate::draw::{
    AspectOps, ColorAttachment, DepthAttachment, DrawCall, DrawRange, IndexBinding, LoadOp,
    PassDesc, ScissorRect, StoreOp, VertexBinding, Viewport,
};
use crate::error::GpuError;
use crate::format::Format;
use crate::passes::PassTracker;
use crate::resource::{
    Extent, Origin, ReadSource, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    TextureViewDesc, ViewDimension,
};
use crate::slow::SlowPath;
use crate::state::{
    BlendComponent, BlendDesc, BlendFactor, BlendOp, ColorWrites, IndexFormat, RenderTargetBlend,
    Topology,
};
use crate::{FormatUse, MAX_COLOR_TARGETS};
use alloc::vec;
use alloc::vec::Vec;

/// What a kept draw covers.
#[derive(Debug)]
enum Range<B: Backend> {
    Vertices {
        first: u32,
        count: u32,
        first_instance: u32,
        instances: u32,
    },
    Indexed {
        first: u32,
        count: u32,
        base_vertex: i32,
        first_instance: u32,
        instances: u32,
    },
    Indirect(B::Buffer, u64, bool),
}

/// One draw of a counted query, kept to be drawn again with its counting program.
#[derive(Debug)]
struct Kept<B: Backend> {
    program: B::Program,
    alpha_to_coverage: bool,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    vertex_layout: B::VertexLayout,
    bindings: [Option<B::BindingSet>; MAX_BINDING_SETS],
    vertex_buffers: Vec<Option<(B::Buffer, u64)>>,
    index: Option<(B::Buffer, u64, IndexFormat)>,
    topology: Topology,
    restart: bool,
    viewports: Vec<Viewport>,
    scissors: Vec<ScissorRect>,
    stencil_reference: [u8; 2],
    immediates: Vec<u8>,
    range: Range<B>,
}

impl<B: Backend> Clone for Range<B> {
    fn clone(&self) -> Self {
        match self {
            Range::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => Range::Vertices {
                first: *first,
                count: *count,
                first_instance: *first_instance,
                instances: *instances,
            },
            Range::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => Range::Indexed {
                first: *first,
                count: *count,
                base_vertex: *base_vertex,
                first_instance: *first_instance,
                instances: *instances,
            },
            Range::Indirect(args, offset, indexed) => {
                Range::Indirect(args.clone(), *offset, *indexed)
            }
        }
    }
}

impl<B: Backend> Clone for Kept<B> {
    fn clone(&self) -> Self {
        Kept {
            program: self.program.clone(),
            alpha_to_coverage: self.alpha_to_coverage,
            depth_stencil: self.depth_stencil.clone(),
            raster: self.raster.clone(),
            vertex_layout: self.vertex_layout.clone(),
            bindings: self.bindings.clone(),
            vertex_buffers: self.vertex_buffers.clone(),
            index: self.index.clone(),
            topology: self.topology,
            restart: self.restart,
            viewports: self.viewports.clone(),
            scissors: self.scissors.clone(),
            stencil_reference: self.stencil_reference,
            immediates: self.immediates.clone(),
            range: self.range.clone(),
        }
    }
}

/// A pooled texture and its view, by size, format and sample count.
#[derive(Debug)]
struct Pooled<B: Backend> {
    key: (u32, u32, Format, u32),
    texture: B::Texture,
    view: B::TextureView,
}

#[derive(Debug)]
pub struct OcclusionCounter<B: Backend> {
    draws: Vec<Kept<B>>,
    running: bool,
    /// The depth-stencil target as it was at the query's begin, once a draw of the query writes it.
    depth_copy: Option<Pooled<B>>,
    copied: bool,
    targets: Vec<Pooled<B>>,
    /// The One plus One blocks, without and with alpha to coverage.
    blends: [Option<B::Blend>; 2],
}

impl<B: Backend> Default for OcclusionCounter<B> {
    fn default() -> Self {
        OcclusionCounter {
            draws: Vec::new(),
            running: false,
            depth_copy: None,
            copied: false,
            targets: Vec::new(),
            blends: [None, None],
        }
    }
}

impl<B: Backend> OcclusionCounter<B> {
    /// Begins a counted query: the open pass ends, so the count starts from the targets as they are.
    pub fn begin(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        if self.running {
            return Err(GpuError::Invalid("a counted query begun while one runs"));
        }
        passes.close(b)?;
        self.draws.clear();
        self.copied = false;
        self.running = true;
        Ok(())
    }

    /// Keeps a draw of the running query, called before the front draws it. `counting` is the draw's
    /// counting program; `alpha_to_coverage` is its blend block's; `writes_depth_stencil` says whether
    /// its depth-stencil block writes either aspect, in which case the first such draw copies the
    /// depth-stencil target first (the target must have [`TextureUsage::COPY_SRC`]).
    pub fn draw(
        &mut self,
        b: &mut B,
        passes: &mut PassTracker<B>,
        call: &DrawCall<'_, B>,
        counting: &B::Program,
        alpha_to_coverage: bool,
        writes_depth_stencil: bool,
    ) -> Result<(), GpuError> {
        if !self.running {
            return Ok(());
        }
        if writes_depth_stencil && !self.copied {
            self.copy_depth(b, passes)?;
        }
        self.draws.push(Kept {
            program: counting.clone(),
            alpha_to_coverage,
            depth_stencil: call.depth_stencil.clone(),
            raster: call.raster.clone(),
            vertex_layout: call.vertex_layout.clone(),
            bindings: call.bindings.map(|s| s.cloned()),
            vertex_buffers: call
                .vertex_buffers
                .iter()
                .map(|v| v.as_ref().map(|v| (v.buffer.clone(), v.offset)))
                .collect(),
            index: call
                .index
                .as_ref()
                .map(|i| (i.buffer.clone(), i.offset, i.format)),
            topology: call.topology,
            restart: call.restart,
            viewports: call.viewports.to_vec(),
            scissors: call.scissors.to_vec(),
            stencil_reference: call.stencil_reference,
            immediates: call.immediates.to_vec(),
            range: match &call.range {
                DrawRange::Vertices {
                    first,
                    count,
                    first_instance,
                    instances,
                } => Range::Vertices {
                    first: *first,
                    count: *count,
                    first_instance: *first_instance,
                    instances: *instances,
                },
                DrawRange::Indexed {
                    first,
                    count,
                    base_vertex,
                    first_instance,
                    instances,
                } => Range::Indexed {
                    first: *first,
                    count: *count,
                    base_vertex: *base_vertex,
                    first_instance: *first_instance,
                    instances: *instances,
                },
                DrawRange::Indirect { args, offset } => {
                    Range::Indirect((*args).clone(), *offset, false)
                }
                DrawRange::IndexedIndirect { args, offset } => {
                    Range::Indirect((*args).clone(), *offset, true)
                }
            },
        });
        Ok(())
    }

    /// Ends the running query and gives its exact count of samples that passed.
    pub fn end(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<u64, GpuError> {
        if !self.running {
            return Err(GpuError::Invalid(
                "a counted query ended that was not begun",
            ));
        }
        self.running = false;
        passes.close(b)?;
        if self.draws.is_empty() {
            return Ok(0);
        }
        b.slow_paths().hit(SlowPath::OcclusionCountReplay);
        let draws = core::mem::take(&mut self.draws);
        let count = self.count(b, passes, &draws);
        self.draws = draws;
        count
    }

    fn copy_depth(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        passes.close(b)?;
        self.copied = true;
        let Some(d) = passes.targets().and_then(|t| t.depth.clone()) else {
            return Ok(());
        };
        let key = (d.width, d.height, d.format, d.samples);
        if self.depth_copy.as_ref().is_none_or(|c| c.key != key) {
            self.depth_copy = Some(pooled(
                b,
                key,
                TextureUsage::RENDER | TextureUsage::COPY_DST,
            )?);
        }
        let copy = self
            .depth_copy
            .as_ref()
            .ok_or(GpuError::Invalid("no copy"))?;
        let at = |texture, mip, layer| TextureRegion {
            texture,
            mip,
            layer,
            origin: Origin { x: 0, y: 0, z: 0 },
            stencil: false,
        };
        b.copy_texture(
            &at(&d.texture, d.mip, d.layer),
            &at(&copy.texture, 0, 0),
            Extent {
                width: d.width,
                height: d.height,
                depth_or_layers: 1,
            },
        );
        Ok(())
    }

    /// Counts `draws` by drawing them into a counting target and summing it; where a sample reached
    /// the counting format's exact bound, counts the two halves of the draws (or of the one draw's
    /// instances or primitives) apart and adds them.
    fn count(
        &mut self,
        b: &mut B,
        passes: &PassTracker<B>,
        draws: &[Kept<B>],
    ) -> Result<u64, GpuError> {
        let targets = passes
            .targets()
            .ok_or(GpuError::Invalid("a counted draw with no targets"))?;
        let size = targets
            .depth
            .as_ref()
            .or(targets.colors.iter().flatten().map(|c| &c.target).next())
            .map(|t| (t.width, t.height, t.samples))
            .or(targets.empty.map(|e| (e.width, e.height, e.samples)))
            .ok_or(GpuError::Invalid("a counted draw with no targets"))?;
        let float32 = FormatUse::RENDER | FormatUse::BLEND;
        let (format, bound) = if b.caps().formats.get(Format::R32Float).contains(float32) {
            (Format::R32Float, 16_777_216.0)
        } else {
            (Format::R16Float, 2048.0)
        };
        let key = (size.0, size.1, format, size.2);
        let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
        let t = match self.targets.iter().position(|t| t.key == key) {
            Some(i) => i,
            None => {
                self.targets.push(pooled(b, key, usage)?);
                self.targets.len() - 1
            }
        };
        if size.2 > 1
            && !self
                .targets
                .iter()
                .any(|t| t.key == (key.0, key.1, format, 1))
        {
            self.targets
                .push(pooled(b, (key.0, key.1, format, 1), usage)?);
        }
        for (i, slot) in self.blends.iter_mut().enumerate() {
            if slot.is_none() {
                let add = BlendComponent {
                    src: BlendFactor::One,
                    dst: BlendFactor::One,
                    op: BlendOp::Add,
                };
                *slot = Some(b.create_blend(&BlendDesc {
                    targets: [RenderTargetBlend {
                        blend: Some((add, add)),
                        writes: ColorWrites::R,
                    }; MAX_COLOR_TARGETS],
                    same_for_all: true,
                    alpha_to_coverage: i == 1,
                    logic_op: None,
                })?);
            }
        }

        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &self.targets[t].view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear([0; 4]),
            store: StoreOp::Store,
        });
        if let Some(d) = &targets.depth {
            let (has_depth, has_stencil) = crate::backend::aspects(d.format);
            let (view, ops) = match (&self.depth_copy, self.copied) {
                // The copy is the target as it was at the query's begin; the draws may write it.
                (Some(c), true) => (&c.view, (StoreOp::Discard, false)),
                // No draw wrote the target, so it still is as it was.
                _ => (&d.view, (StoreOp::Store, true)),
            };
            pass.depth = Some(DepthAttachment {
                view,
                depth: has_depth.then_some(AspectOps {
                    load: LoadOp::Load,
                    store: ops.0,
                    read_only: ops.1,
                }),
                stencil: has_stencil.then_some(AspectOps {
                    load: LoadOp::Load,
                    store: ops.0,
                    read_only: ops.1,
                }),
            });
        }
        b.begin_pass(&pass);
        for k in draws {
            let streams: Vec<_> = k
                .vertex_buffers
                .iter()
                .map(|v| {
                    v.as_ref().map(|(buffer, offset)| VertexBinding {
                        buffer,
                        offset: *offset,
                    })
                })
                .collect();
            let bindings = [0, 1, 2, 3].map(|i| k.bindings[i].as_ref());
            let blend = self.blends[usize::from(k.alpha_to_coverage)]
                .as_ref()
                .ok_or(GpuError::Invalid("no blend"))?;
            b.draw(&DrawCall {
                program: &k.program,
                blend,
                depth_stencil: &k.depth_stencil,
                raster: &k.raster,
                vertex_layout: &k.vertex_layout,
                bindings,
                vertex_buffers: &streams,
                index: k
                    .index
                    .as_ref()
                    .map(|(buffer, offset, format)| IndexBinding {
                        buffer,
                        offset: *offset,
                        format: *format,
                    }),
                topology: k.topology,
                restart: k.restart,
                viewports: &k.viewports,
                scissors: &k.scissors,
                blend_constant: [0; 4],
                stencil_reference: k.stencil_reference,
                immediates: &k.immediates,
                range: match &k.range {
                    Range::Vertices {
                        first,
                        count,
                        first_instance,
                        instances,
                    } => DrawRange::Vertices {
                        first: *first,
                        count: *count,
                        first_instance: *first_instance,
                        instances: *instances,
                    },
                    Range::Indexed {
                        first,
                        count,
                        base_vertex,
                        first_instance,
                        instances,
                    } => DrawRange::Indexed {
                        first: *first,
                        count: *count,
                        base_vertex: *base_vertex,
                        first_instance: *first_instance,
                        instances: *instances,
                    },
                    Range::Indirect(args, offset, false) => DrawRange::Indirect {
                        args,
                        offset: *offset,
                    },
                    Range::Indirect(args, offset, true) => DrawRange::IndexedIndirect {
                        args,
                        offset: *offset,
                    },
                },
            });
        }
        b.end_pass();

        // Sum the target, resolved first where it has several samples (the average times the count).
        let mut read = &self.targets[t];
        if size.2 > 1 {
            let single = self
                .targets
                .iter()
                .find(|p| p.key == (key.0, key.1, format, 1))
                .ok_or(GpuError::Invalid("no resolve target"))?;
            let at = |texture| TextureRegion {
                texture,
                mip: 0,
                layer: 0,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            };
            b.resolve(&at(&read.texture), &at(&single.texture), format);
            read = single;
        }
        let texel = if format == Format::R32Float { 4 } else { 2 };
        let mut bytes = vec![0u8; (size.0 * size.1) as usize * texel];
        b.read_back_now(
            &ReadSource::Texture {
                region: TextureRegion {
                    texture: &read.texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin { x: 0, y: 0, z: 0 },
                    stencil: false,
                },
                extent: Extent {
                    width: size.0,
                    height: size.1,
                    depth_or_layers: 1,
                },
            },
            &mut bytes,
        )?;
        let mut sum = 0f64;
        let mut full = false;
        for c in bytes.chunks_exact(texel) {
            let v = if texel == 4 {
                f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            } else {
                f64::from(half::f16::from_le_bytes([c[0], c[1]]).to_f32())
            };
            // A resolved value is the average of the samples.
            let v = v * f64::from(size.2);
            full |= v >= bound;
            sum += v;
        }
        if !full {
            return Ok(sum as u64);
        }
        let [a, c] = halves(draws).ok_or(GpuError::Invalid(
            "one primitive passing more fragments at a sample than the counting format holds",
        ))?;
        Ok(self.count(b, passes, &a)? + self.count(b, passes, &c)?)
    }
}

/// The draws split into two halves; one draw into its halves of instances, or of primitives for a
/// list.
fn halves<B: Backend>(draws: &[Kept<B>]) -> Option<[Vec<Kept<B>>; 2]> {
    if draws.len() > 1 {
        let (a, c) = draws.split_at(draws.len() / 2);
        return Some([a.to_vec(), c.to_vec()]);
    }
    let k = draws.first()?;
    let per = match k.topology {
        Topology::PointList => 1,
        Topology::LineList => 2,
        Topology::TriangleList => 3,
        _ => 0,
    };
    let (mut a, mut c) = (k.clone(), k.clone());
    match (&mut a.range, &mut c.range) {
        (
            Range::Vertices { instances: ia, .. },
            Range::Vertices {
                first_instance: fc,
                instances: ic,
                ..
            },
        )
        | (
            Range::Indexed { instances: ia, .. },
            Range::Indexed {
                first_instance: fc,
                instances: ic,
                ..
            },
        ) if *ia > 1 => {
            let half = *ia / 2;
            *ia = half;
            *fc += half;
            *ic -= half;
        }
        (
            Range::Vertices {
                first: fa,
                count: ca,
                ..
            },
            Range::Vertices {
                first: fc,
                count: cc,
                ..
            },
        )
        | (
            Range::Indexed {
                first: fa,
                count: ca,
                ..
            },
            Range::Indexed {
                first: fc,
                count: cc,
                ..
            },
        ) if per > 0 && *ca / per > 1 => {
            let half = *ca / per / 2 * per;
            *ca = half;
            *fc = *fa + half;
            *cc -= half;
        }
        _ => return None,
    }
    Some([vec![a], vec![c]])
}

fn pooled<B: Backend>(
    b: &mut B,
    key: (u32, u32, Format, u32),
    usage: TextureUsage,
) -> Result<Pooled<B>, GpuError> {
    let (width, height, format, samples) = key;
    let texture = b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width,
        height,
        depth_or_layers: 1,
        mip_levels: 1,
        samples,
        usage,
        cube: false,
        reinterpret: false,
    })?;
    let view = b.create_view(
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
    )?;
    Ok(Pooled { key, texture, view })
}
