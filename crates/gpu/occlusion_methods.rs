//! Methods of the types [`crate::occlusion`] declares.

use crate::Gpu;
use crate::MAX_BINDING_SETS;
use crate::draw::{
    AspectOps, ColorAttachment, DepthAttachment, DrawCall, DrawRange, IndexBinding, LoadOp,
    PassDesc, StoreOp, VertexBinding,
};
use crate::error::GpuError;
use crate::format::Format;
use crate::occlusion::*;
use crate::passes::{PassTargets, PassTracker};
use crate::resource::{
    BindingEntry, BindingResource, BufferDesc, BufferRange, BufferUsage, Extent, MapIntent, Origin,
    ReadSource, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    ViewDimension, WriteMode,
};
use crate::shader::{Program, ShaderModule, ShaderStage};
use crate::slow::SlowPath;
use crate::state::{
    BlendComponent, BlendDesc, BlendFactor, BlendOp, ColorWrites, RenderTargetBlend, Topology,
};
use crate::{MAX_COLOR_TARGETS, caps::Features, caps::FormatUse};
use crate::{Object, draw::DispatchCall, draw::DispatchGroups, shader::StageSet};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

impl<B: Gpu> Clone for KeptRange<B> {
    fn clone(&self) -> Self {
        match self {
            KeptRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => KeptRange::Vertices {
                first: *first,
                count: *count,
                first_instance: *first_instance,
                instances: *instances,
            },
            KeptRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => KeptRange::Indexed {
                first: *first,
                count: *count,
                base_vertex: *base_vertex,
                first_instance: *first_instance,
                instances: *instances,
            },
            KeptRange::Indirect(args, offset, indexed) => {
                KeptRange::Indirect(args.clone(), *offset, *indexed)
            }
        }
    }
}

impl<B: Gpu> Clone for KeptDraw<B> {
    fn clone(&self) -> Self {
        KeptDraw {
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
            instances_step_by_one: self.instances_step_by_one,
        }
    }
}

/// The summing program: adds every sample of a counting target into a u64 as two words, each
/// workgroup's part first in workgroup memory. Each sample holds an exact integer below 2^24.
impl<B: Gpu> OcclusionCounter<B> {
    fn summing_source(multisampled: bool) -> String {
        let (ty, samples, at) = match multisampled {
            true => (
                "texture_multisampled_2d<f32>",
                "textureNumSamples(counts)",
                "s",
            ),
            false => ("texture_2d<f32>", "1u", "0"),
        };
        format!(
            "@group(0) @binding(0) var counts: {ty};
@group(0) @binding(1) var<storage, read_write> sum: array<atomic<u32>, 2>;
var<workgroup> part: array<atomic<u32>, 2>;
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) i: u32) {{
    if all(id.xy < textureDimensions(counts)) {{
        var n = 0u;
        for (var s = 0u; s < {samples}; s++) {{
            n += u32(textureLoad(counts, id.xy, {at}).x);
        }}
        if n != 0u && atomicAdd(&part[0], n) > 0xffffffffu - n {{
            atomicAdd(&part[1], 1u);
        }}
    }}
    workgroupBarrier();
    if i == 0u {{
        let low = atomicLoad(&part[0]);
        let carry = select(0u, 1u, low != 0u && atomicAdd(&sum[0], low) > 0xffffffffu - low);
        let high = atomicLoad(&part[1]) + carry;
        if high != 0u {{
            atomicAdd(&sum[1], high);
        }}
    }}
}}"
        )
    }
}

impl<B: Gpu> Default for OcclusionCounter<B> {
    fn default() -> Self {
        OcclusionCounter {
            draws: Vec::new(),
            running: false,
            sum: 0,
            segment: None,
            depth_copy: None,
            copied: false,
            saved: None,
            targets: Vec::new(),
            blends: [None, None],
            summing: [None, None],
            sum_buffer: None,
            on_gpu: false,
        }
    }
}

impl<B: Gpu> OcclusionCounter<B> {
    /// Begins a counted query: the open pass ends, so the count starts from the targets as they are.
    pub fn begin(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        if self.running {
            return Err(GpuError::Invalid("a counted query begun while one runs"));
        }
        passes.close(b)?;
        self.draws.clear();
        self.segment = None;
        self.copied = false;
        self.sum = 0;
        self.on_gpu = false;
        self.running = true;
        Ok(())
    }

    /// Keeps a draw of the running query, called before the front draws it. The targets must be the
    /// ones the kept draws were drawn on: [`OcclusionCounter::flush`] comes before any change of them.
    pub fn draw(
        &mut self,
        b: &mut B,
        passes: &mut PassTracker<B>,
        call: &DrawCall<'_, B>,
        counted: &CountedDraw<'_, B>,
    ) -> Result<(), GpuError> {
        if !self.running {
            return Ok(());
        }
        match &self.segment {
            Some((_, generation)) if *generation != passes.generation() => {
                return Err(GpuError::Invalid(
                    "the targets of a counted query changed without a flush",
                ));
            }
            Some(_) => {}
            None => {
                let targets = passes
                    .targets()
                    .ok_or(GpuError::Invalid("a counted draw with no targets"))?
                    .clone();
                self.segment = Some((targets, passes.generation()));
            }
        }
        if counted.writes_depth_stencil && !self.copied {
            self.copy_depth(b, passes)?;
        }
        self.draws.push(KeptDraw {
            program: counted.counting.clone(),
            alpha_to_coverage: counted.alpha_to_coverage,
            instances_step_by_one: counted.instances_step_by_one,
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
                } => KeptRange::Vertices {
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
                } => KeptRange::Indexed {
                    first: *first,
                    count: *count,
                    base_vertex: *base_vertex,
                    first_instance: *first_instance,
                    instances: *instances,
                },
                DrawRange::Indirect { args, offset } => {
                    KeptRange::Indirect((*args).clone(), *offset, false)
                }
                DrawRange::IndexedIndirect { args, offset } => {
                    KeptRange::Indirect((*args).clone(), *offset, true)
                }
            },
        });
        Ok(())
    }

    /// Counts the draws kept so far. While a counted query runs, the front calls it before anything
    /// else it records that changes what those draws read or the targets they drew on: a buffer or
    /// texture write, a copy, a clear, a resolve, or a change of targets.
    pub fn flush(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        let Some((targets, generation)) = self.segment.take() else {
            return Ok(());
        };
        let draws = core::mem::take(&mut self.draws);
        let copied = core::mem::replace(&mut self.copied, false);
        if generation != passes.generation() {
            return Err(GpuError::Invalid(
                "the targets of a counted query changed without a flush",
            ));
        }
        if passes.is_open() {
            passes.close(b)?;
        }
        b.record_slow_path(SlowPath::OcclusionCountReplay);
        let count = self.count(b, &targets, copied, &draws);
        self.draws = draws;
        self.draws.clear();
        self.sum += count?;
        Ok(())
    }

    /// Ends the running query and gives its exact count of samples that passed, reading back what
    /// was summed on the GPU.
    pub fn end(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<u64, GpuError> {
        self.finish(b, passes)?;
        Ok(self.sum + self.read_sum_buffer(b)?)
    }

    /// Ends the running query and writes its exact count of samples that passed into `dst` at
    /// `offset` (a multiple of 4) as a u64, in order with the work recorded around it. Where every
    /// count was summed on the GPU it is copied there and nothing waits; the default writes what
    /// was read back, adding the GPU's part read back too.
    pub fn end_into(
        &mut self,
        b: &mut B,
        passes: &mut PassTracker<B>,
        dst: &B::Buffer,
        offset: u64,
    ) -> Result<(), GpuError> {
        self.finish(b, passes)?;
        match (&self.sum_buffer, self.on_gpu && self.sum == 0) {
            (Some(sum), true) => b.copy_buffer(sum, 0, dst, offset, 8),
            _ => {
                let count = self.sum + self.read_sum_buffer(b)?;
                b.write_buffer(dst, offset, &count.to_le_bytes(), WriteMode::Ordered);
            }
        }
        Ok(())
    }

    fn finish(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        if !self.running {
            return Err(GpuError::Invalid(
                "a counted query ended that was not begun",
            ));
        }
        self.running = false;
        self.flush(b, passes)
    }

    /// What the running query summed on the GPU, read back now; zero where it summed nothing there.
    fn read_sum_buffer(&mut self, b: &mut B) -> Result<u64, GpuError> {
        let Some(sum) = self.sum_buffer.as_ref().filter(|_| self.on_gpu) else {
            return Ok(0);
        };
        let mut bytes = [0u8; 8];
        let src = ReadSource::Buffer {
            buffer: sum,
            offset: 0,
            size: 8,
        };
        b.read_back_now(&src, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    /// Adds the counting target `t` into the sum buffer on the GPU.
    fn sum_on_gpu(&mut self, b: &mut B, t: usize, samples: u32) -> Result<(), GpuError> {
        let multisampled = samples > 1;
        let which = usize::from(multisampled);
        if self.summing[which].is_none() {
            let module = ShaderModule::from_wgsl(
                &Self::summing_source(multisampled),
                "main",
                ShaderStage::Compute,
            )?;
            let shader = b.create_shader(&module)?;
            let program = b.create_program(&Program::Compute(&shader));
            b.destroy(Object::Shader(shader));
            self.summing[which] = Some(program?);
        }
        if self.sum_buffer.is_none() {
            self.sum_buffer = Some(b.create_buffer(&BufferDesc {
                size: 8,
                usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })?);
        }
        let (Some(program), Some(sum)) = (&self.summing[which], &self.sum_buffer) else {
            return Err(GpuError::Invalid("no summing program"));
        };
        if !self.on_gpu {
            b.clear_buffer(sum, 0, 8, &[0]);
            self.on_gpu = true;
        }
        let target = &self.targets[t];
        let set = b.create_binding_set(&[
            BindingEntry {
                binding: 0,
                stages: StageSet::COMPUTE,
                resource: BindingResource::Texture(&target.view),
            },
            BindingEntry {
                binding: 1,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: sum,
                        offset: 0,
                        size: 8,
                    },
                    writable: true,
                },
            },
        ])?;
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = Default::default();
        bindings[0] = Some(&set);
        let r = b.dispatch(&DispatchCall {
            program,
            bindings,
            immediates: &[],
            groups: DispatchGroups::Direct([
                target.key.0.div_ceil(16),
                target.key.1.div_ceil(16),
                1,
            ]),
        });
        b.destroy(Object::BindingSet(set));
        r
    }

    fn copy_depth(&mut self, b: &mut B, passes: &mut PassTracker<B>) -> Result<(), GpuError> {
        passes.close(b)?;
        self.copied = true;
        let Some(d) = passes.targets().and_then(|t| t.depth.clone()) else {
            return Ok(());
        };
        let key = (d.width, d.height, d.format, d.samples);
        if self.depth_copy.as_ref().is_none_or(|c| c.key != key) {
            let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
            self.depth_copy = Some(Self::pooled(b, key, usage)?);
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

    /// Counts `draws`, drawn on `targets` (on the depth copy where `copied`), by drawing them into a
    /// counting target and summing it: on the GPU into the sum buffer where the back end computes
    /// and no pixel's sum over its samples can reach the counting format's exact bound (giving
    /// zero here), otherwise read back. Where a pixel's sum reached the bound, counts the two
    /// halves of the draws (or of the one draw's instances or primitives) one after the other, from
    /// the depth the whole began with, and adds them.
    fn count(
        &mut self,
        b: &mut B,
        targets: &PassTargets<B>,
        copied: bool,
        draws: &[KeptDraw<B>],
    ) -> Result<u64, GpuError> {
        let size = targets
            .depth
            .as_ref()
            .or(targets.colors.iter().flatten().map(|c| &c.target).next())
            .map(|t| (t.width, t.height, t.samples))
            .or(targets.empty.map(|e| (e.width, e.height, e.samples)))
            .ok_or(GpuError::Invalid("a counted draw with no targets"))?;
        let float32 = FormatUse::RENDER | FormatUse::BLEND;
        let (format, bound) = if b.caps().formats.uses[Format::R32Float as usize].contains(float32)
        {
            (Format::R32Float, 16_777_216.0)
        } else {
            (Format::R16Float, 2048.0)
        };
        let key = (size.0, size.1, format, size.2);
        // The summing program needs compute, 8 bytes of workgroup memory, 256 invocations a
        // workgroup and, for multisampled targets, reading them.
        let caps = b.caps();
        let compute = caps.features.contains(Features::COMPUTE)
            && caps.limits.compute_workgroup_storage >= 8
            && caps.limits.compute_invocations >= 256
            && caps.limits.compute_size[0] >= 16
            && caps.limits.compute_size[1] >= 16
            && (size.2 == 1 || caps.features.contains(Features::MULTISAMPLE_READ));
        let on_gpu =
            compute && Self::primitives(draws).saturating_mul(u64::from(size.2)) < bound as u64;
        let usage = match compute {
            true => TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::SAMPLE,
            false => TextureUsage::RENDER | TextureUsage::COPY_SRC,
        };
        let t = match self.targets.iter().position(|t| t.key == key) {
            Some(i) => i,
            None => {
                self.targets.push(Self::pooled(b, key, usage)?);
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
                .push(Self::pooled(b, (key.0, key.1, format, 1), usage)?);
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
        // Where the draws could reach the bound at a pixel and write depth, the copy is kept as it
        // is, to count the halves from.
        let saving = copied
            && targets.depth.is_some()
            && Self::primitives(draws).saturating_mul(u64::from(size.2)) >= bound as u64;
        if saving {
            let copy = self
                .depth_copy
                .as_ref()
                .ok_or(GpuError::Invalid("no depth copy"))?;
            if self.saved.as_ref().is_none_or(|s| s.key != copy.key) {
                let usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
                self.saved = Some(Self::pooled(b, copy.key, usage)?);
            }
            let saved = self
                .saved
                .as_ref()
                .ok_or(GpuError::Invalid("no saved copy"))?;
            Self::whole_copy(b, copy, saved);
        }
        if let Some(d) = &targets.depth {
            let (has_depth, has_stencil) = d.format.aspects();
            let (view, ops) = match (&self.depth_copy, copied) {
                // The copy is the target as it was before the first draw that writes it, and the
                // count writes it as the draws wrote the target.
                (Some(c), true) => (&c.view, (StoreOp::Store, false)),
                // No draw wrote the target, so it still is as they saw it.
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
                    KeptRange::Vertices {
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
                    KeptRange::Indexed {
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
                    KeptRange::Indirect(args, offset, false) => DrawRange::Indirect {
                        args,
                        offset: *offset,
                    },
                    KeptRange::Indirect(args, offset, true) => DrawRange::IndexedIndirect {
                        args,
                        offset: *offset,
                    },
                },
            });
        }
        b.end_pass();
        if on_gpu {
            self.sum_on_gpu(b, t, size.2)?;
            return Ok(0);
        }

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
            // A resolved value is the average of the samples. A pixel is full once the sum over its
            // samples reaches the bound: below it, the average keeps every count exact.
            let v = v * f64::from(size.2);
            full |= v >= bound;
            sum += v;
        }
        if !full {
            return Ok(sum as u64);
        }
        let bases = b.caps().features.contains(Features::BASE_VERTEX_INSTANCE);
        let [a, c] = Self::halves(draws, bases).ok_or(GpuError::Invalid(
            "a draw reaching the counting format's bound at a pixel that cannot be split: a fan, a strip with restart, an indirect draw, or one primitive's instances where the first instance cannot be raised",
        ))?;
        if saving {
            let (copy, saved) = (
                self.depth_copy
                    .as_ref()
                    .ok_or(GpuError::Invalid("no depth copy"))?,
                self.saved
                    .as_ref()
                    .ok_or(GpuError::Invalid("no saved copy"))?,
            );
            Self::whole_copy(b, saved, copy);
        }
        Ok(self.count(b, targets, copied, &a)? + self.count(b, targets, copied, &c)?)
    }
}

impl<B: Gpu> OcclusionCounter<B> {
    /// Copies one pooled texture into another of the same key, whole.
    fn whole_copy(b: &mut B, from: &PooledTexture<B>, to: &PooledTexture<B>) {
        let at = |texture| TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin { x: 0, y: 0, z: 0 },
            stencil: false,
        };
        b.copy_texture(
            &at(&from.texture),
            &at(&to.texture),
            Extent {
                width: from.key.0,
                height: from.key.1,
                depth_or_layers: 1,
            },
        );
    }

    /// How many primitives the draws draw at most, which bounds how many fragments pass at one sample;
    /// an indirect draw's is unknown.
    fn primitives(draws: &[KeptDraw<B>]) -> u64 {
        draws
            .iter()
            .map(|k| {
                let (count, instances) = match k.range {
                    KeptRange::Vertices {
                        count, instances, ..
                    }
                    | KeptRange::Indexed {
                        count, instances, ..
                    } => (u64::from(count), u64::from(instances)),
                    KeptRange::Indirect(..) => return u64::MAX,
                };
                let per = match k.topology {
                    Topology::PointList | Topology::LineLoop => count,
                    Topology::LineList => count / 2,
                    Topology::LineStrip => count.saturating_sub(1),
                    Topology::TriangleList => count / 3,
                    Topology::TriangleStrip | Topology::TriangleFan => count.saturating_sub(2),
                };
                per.saturating_mul(instances)
            })
            .fold(0u64, u64::saturating_add)
    }

    /// The draws split into two halves; one draw into its halves of instances where the first instance
    /// can be raised without changing what any instance reads (`bases` is [`Features::BASE_VERTEX_INSTANCE`]),
    /// otherwise into its halves of primitives, each drawn for every instance.
    fn halves(draws: &[KeptDraw<B>], bases: bool) -> Option<[Vec<KeptDraw<B>>; 2]> {
        if draws.len() > 1 {
            let (a, c) = draws.split_at(draws.len() / 2);
            return Some([a.to_vec(), c.to_vec()]);
        }
        let k = draws.first()?;
        // Vertices per primitive, vertices shared with the next, and whether a split keeps strip
        // triangles' winding (an even primitive).
        let (per, shared, even) = match (k.topology, k.restart) {
            (Topology::PointList, _) => (1, 0, false),
            (Topology::LineList, _) => (2, 0, false),
            (Topology::TriangleList, _) => (3, 0, false),
            (Topology::LineStrip, false) => (1, 1, false),
            (Topology::TriangleStrip, false) => (1, 2, true),
            _ => (0, 0, false),
        };
        // The primitives the first half takes.
        let split = |count: u32| {
            let k = count.saturating_sub(shared) / per.max(1) / 2;
            if even { k - k % 2 } else { k }
        };
        let (mut a, mut c) = (k.clone(), k.clone());
        let by_instance = bases && k.instances_step_by_one;
        match (&mut a.range, &mut c.range) {
            (
                KeptRange::Vertices { instances: ia, .. },
                KeptRange::Vertices {
                    first_instance: fc,
                    instances: ic,
                    ..
                },
            )
            | (
                KeptRange::Indexed { instances: ia, .. },
                KeptRange::Indexed {
                    first_instance: fc,
                    instances: ic,
                    ..
                },
            ) if by_instance && *ia > 1 => {
                let half = *ia / 2;
                *ia = half;
                *fc += half;
                *ic -= half;
            }
            (
                KeptRange::Vertices {
                    first: fa,
                    count: ca,
                    ..
                },
                KeptRange::Vertices {
                    first: fc,
                    count: cc,
                    ..
                },
            )
            | (
                KeptRange::Indexed {
                    first: fa,
                    count: ca,
                    ..
                },
                KeptRange::Indexed {
                    first: fc,
                    count: cc,
                    ..
                },
            ) if per > 0 && split(*ca) > 0 => {
                let k = split(*ca);
                *ca = k * per + shared;
                *fc = *fa + k * per;
                *cc -= k * per;
            }
            _ => return None,
        }
        Some([vec![a], vec![c]])
    }

    fn pooled(
        b: &mut B,
        key: (u32, u32, Format, u32),
        usage: TextureUsage,
    ) -> Result<PooledTexture<B>, GpuError> {
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
        Ok(PooledTexture { key, texture, view })
    }
}
