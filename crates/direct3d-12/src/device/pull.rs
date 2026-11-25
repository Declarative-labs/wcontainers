//! Vertex programs and their helper blocks: the draw's bases, where an indirect draw's arguments
//! lie, and vertex inputs fetched in the program (plan 5.7): streams whose strides or offsets
//! WebGPU's vertex fetch refuses (`VertexPulledInShader`) and steps of several instances on a
//! back end without them (`InstanceRateInShader`).

use wcontainers_gpu::{
    AddedBinding, BindingEntry, BindingResource, BufferRange, CLAMPED_SAMPLER_BINDING, Caps,
    DRAW_ARGUMENTS_BINDING, HelperSlot, RingKind, VertexFormat, helper_binding,
};

use wcontainers_directx_graphics_infrastructure::stage_registers::{
    TOO_MANY_VALUES, input_moves, output_moves, passed_locations, relocate,
};

use super::*;

/// A vertex program's helper block: its size and what each slot holds.
#[derive(Clone, Debug, Default)]
pub(super) struct Helpers {
    pub size: u32,
    pub slots: Vec<HelperSlot>,
    /// The (texture, sampler) registers the program samples with, in the order of its merged
    /// samplers (`CLAMPED_SAMPLER_BINDING`), where it applies a minimum level-of-detail clamp.
    pub pairs: Vec<(u32, u32)>,
    /// The bindings it adds for multisampled arrays' layers after the first: set, binding,
    /// texture slot and layer (`AddedBinding::TextureLayer`).
    pub layers: Vec<(u32, u32, u32, u32)>,
}

impl Helpers {
    pub fn of(c: &Converted) -> Self {
        let clamped = c
            .reflection
            .helpers
            .iter()
            .any(|h| matches!(h.value, HelperValue::MinLod { .. }));
        Helpers {
            size: c.reflection.helper_size,
            slots: c.reflection.helpers.clone(),
            pairs: match clamped {
                true => c.reflection.sampled_pairs.clone(),
                false => Vec::new(),
            },
            layers: c
                .reflection
                .bindings
                .iter()
                .filter_map(|b| match b.added {
                    Some(AddedBinding::TextureLayer(slot, layer)) => {
                        Some((b.set, b.binding, slot, layer))
                    }
                    _ => None,
                })
                .collect(),
        }
    }
}

/// A vertex program made with its pipeline's pixel program.
pub(super) struct VertexProgram<B: Gpu> {
    pub program: B::Program,
    pub helpers: Helpers,
    /// Each pulled input's stream binding (set 0): binding and input location.
    pub streams: Vec<(u32, u32)>,
}

/// Where the values a pipeline's last vertex stage and pixel program pass each other move, where
/// the back end has fewer inter-stage locations than Direct3D's 32 registers (`stage_registers`):
/// the outputs of the vertex program, the domain program or the stage helper's pass-through that
/// is drawn, and the pixel program's inputs.
#[derive(Clone, Debug, Default)]
pub(super) struct StageMoves {
    pub outputs: Vec<(u32, u32)>,
    pub inputs: Vec<(u32, u32)>,
}

/// `c` with its values between the vertex and pixel stages moved by `moves`: a vertex program's
/// outputs, a pixel program's inputs.
pub(super) fn moved(mut c: Converted, moves: &[(u32, u32)]) -> Result<Converted, Error> {
    let values = match c.modules[0].stage {
        ShaderStage::Vertex => &mut c.reflection.outputs,
        _ => &mut c.reflection.inputs,
    };
    relocate(&mut c.modules[0], values, moves).map_err(Error::NotImplemented)?;
    Ok(c)
}

/// `module`, a vertex stage the stage helper makes, with its outputs moved by `moves`.
pub(super) fn moved_module(
    mut module: ShaderModule,
    moves: &[(u32, u32)],
) -> Result<ShaderModule, Error> {
    relocate(&mut module, &mut [], moves).map_err(Error::NotImplemented)?;
    Ok(module)
}

/// The inputs a program fetches itself, by location, with their formats; and whether it reads
/// the bases from indirect arguments.
pub(super) type PulledKey = (Vec<(u32, VertexFormat)>, bool);

/// Where a draw's bases come from.
pub(super) enum Bases<'a, B: Gpu> {
    /// Base vertex (or first vertex) and first instance.
    Direct([u32; 2]),
    /// An indirect draw's arguments.
    Arguments {
        args: &'a B::Buffer,
        offset: u64,
        indexed: bool,
    },
}

/// How a draw fetches its vertex inputs.
pub(super) struct Fetch<B: Gpu> {
    /// The layout of the streams fetched as laid out.
    pub layout: B::VertexLayout,
    /// The inputs fetched in the program: location, format and the words of
    /// `HelperValue::PulledStream`.
    pub inputs: Vec<(u32, VertexFormat, [u32; 4])>,
    /// The bindings of the streams fetched in the program: buffer, offset, size.
    pub ranges: [Option<(B::Buffer, u64, u64)>; MAX_VERTEX_STREAMS],
}

/// The streams a draw fetches in the program, with the reason: Direct3D's slots and input
/// registers past the back end's vertex buffers and attribute locations, strides WebGPU's vertex
/// fetch refuses (not a multiple of 4, or with an element reaching past the stride), or a step of
/// several instances where the back end has none.
pub(super) fn pulled_streams(
    inputs: &[(InputElement, u32)],
    strides: &[u32; MAX_VERTEX_STREAMS],
    caps: &Caps,
) -> [Option<SlowPath>; MAX_VERTEX_STREAMS] {
    let rates = caps.features.contains(Features::INSTANCE_STEP_RATE);
    let (streams, attributes) = (caps.limits.vertex_streams, caps.limits.vertex_attributes);
    let mut out = [None; MAX_VERTEX_STREAMS];
    for (e, location) in inputs {
        let slot = e.slot as usize;
        let size = vertex_format(e.format).map_or(4, |f| f.1);
        // Never-stepping streams are laid out with stride 0 (`Device::vertex_layout`).
        let stride = match e.per_instance && e.step_rate == 0 && !rates {
            true => 0,
            false => strides[slot],
        };
        if e.slot >= streams
            || *location >= attributes
            || !stride.is_multiple_of(4)
            || stride != 0 && u64::from(e.offset) + u64::from(size) > u64::from(stride)
        {
            out[slot] = Some(SlowPath::VertexPulledInShader);
        } else if e.per_instance && e.step_rate > 1 && !rates && out[slot].is_none() {
            out[slot] = Some(SlowPath::InstanceRateInShader);
        }
    }
    out
}

impl<B: Gpu> Device<B> {
    /// Where the values between `last`, the vertex stage a pipeline draws, and pixel program `ps`
    /// move (`StageMoves`): every value the two pass, those the front links beside theirs too.
    pub(super) fn stage_moves(
        &mut self,
        last: &ShaderModule,
        ps: Option<&ShaderModule>,
    ) -> Result<StageMoves, Error> {
        let slots = self.backend.caps().limits.inter_stage_slots;
        let read = ps.map_or_else(Vec::new, passed_locations);
        let written = passed_locations(last);
        let (Some(inputs), Some(outputs)) = (
            input_moves(&read, slots),
            output_moves(&written, &read, slots),
        ) else {
            self.backend.record_slow_path(SlowPath::VaryingsBeyondLimit);
            return Err(Error::NotImplemented(TOO_MANY_VALUES));
        };
        Ok(StageMoves { outputs, inputs })
    }

    /// How a draw fetches `inputs` with the bound `views`: the streams `pulled` names in the
    /// program, the rest as laid out.
    pub(super) fn fetch(
        &mut self,
        inputs: &[(InputElement, u32)],
        views: &[VertexBufferView; MAX_VERTEX_STREAMS],
        pulled: &[Option<SlowPath>; MAX_VERTEX_STREAMS],
        first_instance: Option<u32>,
    ) -> Result<Fetch<B>, Error> {
        let mut strides = [0; MAX_VERTEX_STREAMS];
        for (s, v) in strides.iter_mut().zip(views) {
            *s = v.stride;
        }
        let mut ranges = [const { None }; MAX_VERTEX_STREAMS];
        if pulled.iter().all(Option::is_none) {
            return Ok(Fetch {
                layout: self.vertex_layout(inputs, &strides)?,
                inputs: Vec::new(),
                ranges,
            });
        }
        let laid_out: Vec<_> = inputs
            .iter()
            .filter(|(e, _)| pulled[e.slot as usize].is_none())
            .cloned()
            .collect();
        let layout = self.vertex_layout(&laid_out, &strides)?;
        let align = u64::from(
            self.backend
                .caps()
                .limits
                .min_storage_offset_alignment
                .max(STORAGE_WORD as u32),
        );
        let mut fetched = Vec::new();
        for (e, location) in inputs
            .iter()
            .filter(|(e, _)| pulled[e.slot as usize].is_some())
        {
            let slot = e.slot as usize;
            let v = &views[slot];
            let (id, offset) = self
                .addresses
                .find(v.address)
                .filter(|_| v.size > 0)
                .ok_or(Error::NotImplemented(
                    "a vertex input fetched in the program from no vertex buffer",
                ))?;
            // Bound from the offset rounded down to the storage alignment; the program adds the
            // rest. Its bounds are the view's end rounded up to a word, within the buffer.
            let start = offset - offset % align;
            let rest = (offset - start) as u32;
            if ranges[slot].is_none() {
                let width = self.resources[id.0 as usize]
                    .desc
                    .width
                    .next_multiple_of(STORAGE_WORD);
                let size = (u64::from(rest) + u64::from(v.size))
                    .next_multiple_of(STORAGE_WORD)
                    .min(width - start);
                ranges[slot] = Some((self.buffer(id)?.0.clone(), start, size));
            }
            let at = rest + e.offset;
            let first = || {
                first_instance.ok_or(Error::NotImplemented(
                    "an indirect draw of a per-instance stream fetched in the program that steps \
                     other than once an instance",
                ))
            };
            let too_far = Error::NotImplemented("a per-instance element beyond 4 GiB");
            let words = match (e.per_instance, e.step_rate) {
                (false, _) => [at, v.stride, 0, 0],
                (true, 0) => {
                    let element = first()?.checked_mul(v.stride).ok_or(too_far)?;
                    [element.checked_add(at).ok_or(too_far)?, 0, 1, 0]
                }
                (true, 1) => [at, v.stride, 1, 0],
                (true, n) => [at, v.stride, n, first()?.checked_mul(n - 1).ok_or(too_far)?],
            };
            fetched.push((*location, vertex_format(e.format)?.0, words));
        }
        Ok(Fetch {
            layout,
            inputs: fetched,
            ranges,
        })
    }

    /// A vertex program converted with `key`, its outputs moved by `moves`, and made with
    /// `fragment`.
    pub(super) fn vertex_program(
        &mut self,
        vs: &[u8],
        key: VariantKey,
        fragment: Option<&ShaderModule>,
        moves: &[(u32, u32)],
    ) -> Result<VertexProgram<B>, Error> {
        let vs = moved(self.convert(vs, ShaderStage::Vertex, key)?, moves)?;
        self.program_of(&vs, fragment)
    }

    /// The vertex program of converted stage `vs` made with `fragment`.
    pub(super) fn program_of(
        &mut self,
        vs: &Converted,
        fragment: Option<&ShaderModule>,
    ) -> Result<VertexProgram<B>, Error> {
        let vertex = self.backend.create_shader(&vs.modules[0])?;
        let fragment = match fragment.map(|f| self.backend.create_shader(f)).transpose() {
            Ok(f) => f,
            Err(e) => {
                self.backend.destroy(Object::Shader(vertex));
                return Err(e.into());
            }
        };
        let program = self.backend.create_program(&Program::Render {
            vertex: &vertex,
            fragment: fragment.as_ref(),
        });
        self.backend.destroy(Object::Shader(vertex));
        if let Some(f) = fragment {
            self.backend.destroy(Object::Shader(f));
        }
        let streams = vs
            .reflection
            .bindings
            .iter()
            .filter_map(|b| match b.added {
                Some(AddedBinding::PulledStream(location)) => Some((b.binding, location)),
                _ => None,
            })
            .collect();
        Ok(VertexProgram {
            program: program?,
            helpers: Helpers::of(vs),
            streams,
        })
    }

    /// The index among the pipeline's pulled programs of the one for `key`, made at its first
    /// draw.
    pub(super) fn pulled_program(
        &mut self,
        pid: PipelineId,
        key: PulledKey,
    ) -> Result<usize, Error> {
        let refused = Error::NotImplemented("a vertex shader fetching its inputs did not convert");
        let r = self.render_mut(pid);
        if let Some(i) = r.pulled.iter().position(|(k, _)| *k == key) {
            return r.pulled[i].1.as_ref().map(|_| i).ok_or(refused);
        }
        let variant = VariantKey {
            output_interpolation: r.interpolation.clone(),
            draw_base: match key.1 {
                true => DrawBaseSource::Arguments,
                false => DrawBaseSource::Helpers,
            },
            pulled: key.0.clone(),
            ..r.typed_key.clone()
        };
        let (code, fragment) = (r.vs_code.clone(), r.fragment.clone());
        let moves = r.moves.outputs.clone();
        let made = self.vertex_program(&code, variant, fragment.as_ref(), &moves);
        let r = self.render_mut(pid);
        let failed = made.as_ref().err().cloned();
        r.pulled.push((key, made.ok()));
        match failed {
            None => Ok(r.pulled.len() - 1),
            Some(e) => Err(e),
        }
    }

    /// The binding set of a vertex program's helper block holding what it reads: the draw's
    /// bases (`DrawBaseInHelpers`), where its indirect arguments lie, with them bound
    /// (`DrawBaseFromArguments`), its pulled inputs' words and what its bindings give; for the
    /// back end's `stages` that run it (the vertex stage, or compute for its form before a
    /// geometry stage). `pixel` adds the pixel stage's block, `extra` other bindings of the set
    /// (the pixel counter, views a program testing early reads).
    pub(super) fn helper_set(
        &mut self,
        helpers: &Helpers,
        bases: &Bases<'_, B>,
        pulled: &[(u32, VertexFormat, [u32; 4])],
        stages: StageSet,
        pixel: Option<&Helpers>,
        extra: Vec<BindingEntry<'_, B>>,
    ) -> Result<B::BindingSet, Error> {
        let mut blocks = Vec::with_capacity(2);
        let mut arguments = None;
        if helpers.size != 0 {
            let mut block = vec![0u8; helpers.size as usize];
            for slot in &helpers.slots {
                let words = match (slot.value, bases) {
                    (HelperValue::DrawBase, Bases::Direct(b)) => {
                        self.backend.record_slow_path(SlowPath::DrawBaseInHelpers);
                        words(&[b[0], b[1]])
                    }
                    (
                        HelperValue::DrawArguments,
                        Bases::Arguments {
                            args,
                            offset,
                            indexed,
                        },
                    ) => {
                        self.backend
                            .record_slow_path(SlowPath::DrawBaseFromArguments);
                        arguments =
                            Some(((*args).clone(), offset + if *indexed { 20 } else { 16 }));
                        words(&[(offset / 4) as u32, u32::from(*indexed)])
                    }
                    (HelperValue::PulledStream { input }, _) => words(
                        &pulled
                            .iter()
                            .find(|p| p.0 == input)
                            .ok_or(Error::Invalid("a pulled input the draw does not fetch"))?
                            .2,
                    ),
                    (value, _) => self.bound_helper(bind::VERTEX, value)?,
                };
                put(&mut block, slot.offset, slot.value, words)?;
            }
            blocks.push((helper_binding(ShaderStage::Vertex), stages, block));
        }
        let mut merged = Vec::new();
        if let Some(p) = pixel.filter(|p| p.size != 0) {
            let block = self.stage_block(p, bind::PIXEL)?;
            blocks.push((
                helper_binding(ShaderStage::Fragment),
                StageSet::FRAGMENT,
                block,
            ));
            merged = self.merged_samplers(p, bind::PIXEL, StageSet::FRAGMENT)?;
        }
        let reads: Vec<_> = arguments
            .iter()
            .map(|(buffer, size)| {
                (
                    DRAW_ARGUMENTS_BINDING,
                    stages,
                    BufferRange {
                        buffer,
                        offset: 0,
                        size: *size,
                    },
                )
            })
            .collect();
        self.helper_blocks(&blocks, &reads, &merged, extra)
    }

    /// A clamped program's merged samplers (binding, the back end's stages, sampler): each pair
    /// whose texture is under a clamp samples through its sampler with the minimum level raised
    /// to the clamp.
    pub(super) fn merged_samplers(
        &mut self,
        helpers: &Helpers,
        visibility: u32,
        stages: StageSet,
    ) -> Result<Vec<(u32, StageSet, B::Sampler)>, Error> {
        let mut merged = Vec::new();
        for (n, &(texture, sampler)) in helpers.pairs.iter().enumerate() {
            let Some(&(.., clamp)) = self
                .bound_clamps
                .iter()
                .find(|c| c.0 == visibility && c.1 == texture)
            else {
                continue;
            };
            let v = self
                .bound_samplers
                .iter()
                .find(|s| s.0 == visibility && s.1 == sampler)
                .ok_or(Error::Invalid(
                    "a clamped texture sampled through no sampler",
                ))?
                .2;
            let s = self.clamped_sampler(&v, clamp)?;
            merged.push((CLAMPED_SAMPLER_BINDING + n as u32, stages, s));
        }
        Ok(merged)
    }

    /// The helper block of a pixel or compute stage (API stage `visibility`), from what its
    /// bindings give. The layer and viewport a pixel shader reads are zero: a draw not split per
    /// layer or viewport is one run of layer 0 through viewport 0.
    pub(super) fn stage_block(&self, helpers: &Helpers, visibility: u32) -> Result<Vec<u8>, Error> {
        self.run_block(helpers, visibility, [0, 0])
    }

    /// As `stage_block`, for the run `run` (layer, viewport) of a draw split per layer or
    /// viewport after a geometry stage choosing them.
    pub(super) fn run_block(
        &self,
        helpers: &Helpers,
        visibility: u32,
        run: [u32; 2],
    ) -> Result<Vec<u8>, Error> {
        let mut block = vec![0u8; helpers.size as usize];
        for slot in &helpers.slots {
            let words = match slot.value {
                HelperValue::Layer => words(&[run[0]]),
                HelperValue::Viewport => words(&[run[1]]),
                value => self.bound_helper(visibility, value)?,
            };
            put(&mut block, slot.offset, slot.value, words)?;
        }
        Ok(block)
    }

    /// The words the last `binding_sets` found for `value` of the API stage `visibility`.
    fn bound_helper(&self, visibility: u32, value: HelperValue) -> Result<Words, Error> {
        let unwritten = Error::NotImplemented("a helper value the front does not write");
        if let Some(h) = self
            .bound_helpers
            .iter()
            .find(|h| h.0 == visibility && h.1 == value)
        {
            return Ok(h.2);
        }
        // A bound sampler's values, which programs read where they compute with its state
        // (Direct3D's level-of-detail query, a sample's clamp) though nothing of it is applied
        // in the program.
        let (HelperValue::LodBias { sampler }
        | HelperValue::BorderColor { sampler }
        | HelperValue::SamplerState { sampler }) = value
        else {
            return Err(unwritten);
        };
        let v = self
            .bound_samplers
            .iter()
            .find(|s| s.0 == visibility && s.1 == sampler)
            .ok_or(unwritten)?
            .2;
        Ok(match value {
            HelperValue::LodBias { .. } => words(&[v.lod_bias]),
            HelperValue::BorderColor { .. } => words(&v.border),
            _ => {
                let desc = crate::state::sampler(&v, self.backend.caps())?;
                crate::state::sampler_state(&v, &desc)
            }
        })
    }

    /// A helper set of `blocks` (binding, the back end's stages, bytes), written into the data
    /// ring, with read-only storage buffers `reads` (binding, the back end's stages, range) bound,
    /// such as an indirect draw's arguments, and `extra` added as they are.
    pub(super) fn helper_blocks(
        &mut self,
        blocks: &[(u32, StageSet, Vec<u8>)],
        reads: &[(u32, StageSet, BufferRange<&B::Buffer>)],
        samplers: &[(u32, StageSet, B::Sampler)],
        extra: Vec<BindingEntry<'_, B>>,
    ) -> Result<B::BindingSet, Error> {
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offsets = vec![0; blocks.len()];
        let bytes: Vec<&[u8]> = blocks.iter().map(|b| b.2.as_slice()).collect();
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &bytes, align, &mut offsets)?;
        let mut entries: Vec<_> = blocks
            .iter()
            .zip(&offsets)
            .map(|((binding, stages, block), offset)| BindingEntry {
                binding: *binding,
                stages: *stages,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset: *offset,
                    size: block.len() as u64,
                }),
            })
            .collect();
        entries.extend(samplers.iter().map(|(binding, stages, s)| BindingEntry {
            binding: *binding,
            stages: *stages,
            resource: BindingResource::Sampler(s),
        }));
        entries.extend(reads.iter().map(|(binding, stages, range)| BindingEntry {
            binding: *binding,
            stages: *stages,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: range.buffer,
                    offset: range.offset,
                    size: range.size,
                },
                writable: false,
            },
        }));
        entries.extend(extra);
        Ok(self.backend.create_binding_set(&entries)?)
    }
}

/// A helper value's words: at most nine (`HelperValue::SamplerState`), zeros past its own.
pub(super) type Words = [u32; 9];

/// `w` as helper words.
pub(super) fn words(w: &[u32]) -> Words {
    let mut out = [0; 9];
    out[..w.len()].copy_from_slice(w);
    out
}

/// Writes helper value `value`'s words at `offset` of `block`.
fn put(block: &mut [u8], offset: u32, value: HelperValue, words: Words) -> Result<(), Error> {
    let n = match value {
        HelperValue::LodBias { .. }
        | HelperValue::TextureComponents { .. }
        | HelperValue::TextureLevels { .. }
        | HelperValue::TextureSamples { .. }
        | HelperValue::Layer
        | HelperValue::Viewport => 1,
        HelperValue::TexelBufferView { .. } | HelperValue::MinLod { .. } => 2,
        HelperValue::DrawBase | HelperValue::DrawArguments => 3,
        HelperValue::TexelLayout { .. } => 5,
        HelperValue::SamplerState { .. } => 9,
        _ => 4,
    };
    let at = offset as usize;
    let n = n.min(block.len().saturating_sub(at) / 4);
    if n == 0 {
        return Err(Error::Invalid("a helper slot past its block"));
    }
    for (k, w) in words[..n].iter().enumerate() {
        block[at + 4 * k..at + 4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    Ok(())
}
