//! Vertex programs and their helper blocks: the draw's bases, where an indirect draw's arguments
//! lie, and vertex inputs fetched in the program (plan 5.7): streams whose strides or offsets
//! WebGPU's vertex fetch refuses (`VertexPulledInShader`) and steps of several instances on a
//! back end without them (`InstanceRateInShader`).

use wcontainers_gpu::{
    AddedBinding, BindingEntry, BindingResource, BufferRange, DRAW_ARGUMENTS_BINDING, HelperSlot,
    RingKind, VertexFormat, helper_binding,
};

use super::*;

/// A vertex program's helper block: its size and what each slot holds.
#[derive(Clone, Debug, Default)]
pub(super) struct Helpers {
    pub size: u32,
    pub slots: Vec<HelperSlot>,
}

impl Helpers {
    pub fn of(c: &Converted) -> Self {
        Helpers {
            size: c.reflection.helper_size,
            slots: c.reflection.helpers.clone(),
        }
    }
}

/// A vertex program made with its pipeline's pixel program.
pub(super) struct VertexProgram<B: Backend> {
    pub program: B::Program,
    pub helpers: Helpers,
    /// Each pulled input's stream binding (set 0): binding and input location.
    pub streams: Vec<(u32, u32)>,
}

/// The inputs a program fetches itself, by location, with their formats; and whether it reads
/// the bases from indirect arguments.
pub(super) type PulledKey = (Vec<(u32, VertexFormat)>, bool);

/// Where a draw's bases come from.
pub(super) enum Bases<'a, B: Backend> {
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
pub(super) struct Fetch<B: Backend> {
    /// The layout of the streams fetched as laid out.
    pub layout: B::VertexLayout,
    /// The inputs fetched in the program: location, format and the words of
    /// `HelperValue::PulledStream`.
    pub inputs: Vec<(u32, VertexFormat, [u32; 4])>,
    /// The bindings of the streams fetched in the program: buffer, offset, size.
    pub ranges: [Option<(B::Buffer, u64, u64)>; MAX_VERTEX_STREAMS],
}

/// The streams a draw fetches in the program, with the reason: strides WebGPU's vertex fetch
/// refuses (not a multiple of 4, or with an element reaching past the stride), or a step of
/// several instances where the back end has none.
pub(super) fn pulled_streams(
    inputs: &[(InputElement, u32)],
    strides: &[u32; MAX_VERTEX_STREAMS],
    rates: bool,
) -> [Option<SlowPath>; MAX_VERTEX_STREAMS] {
    let mut out = [None; MAX_VERTEX_STREAMS];
    for (e, _) in inputs {
        let slot = e.slot as usize;
        let size = vertex_format(e.format).map_or(4, |f| f.1);
        // Never-stepping streams are laid out with stride 0 (`Device::vertex_layout`).
        let stride = match e.per_instance && e.step_rate == 0 && !rates {
            true => 0,
            false => strides[slot],
        };
        if !stride.is_multiple_of(4)
            || stride != 0 && u64::from(e.offset) + u64::from(size) > u64::from(stride)
        {
            out[slot] = Some(SlowPath::VertexPulledInShader);
        } else if e.per_instance && e.step_rate > 1 && !rates && out[slot].is_none() {
            out[slot] = Some(SlowPath::InstanceRateInShader);
        }
    }
    out
}

impl<B: Backend> Device<B> {
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
                .max(4),
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
                let width = self.resources[id.0 as usize].desc.width.next_multiple_of(4);
                let size = (u64::from(rest) + u64::from(v.size))
                    .next_multiple_of(4)
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

    /// A vertex program converted with `key` and made with `fragment`.
    pub(super) fn vertex_program(
        &mut self,
        vs: &[u8],
        key: VariantKey,
        fragment: Option<&ShaderModule>,
    ) -> Result<VertexProgram<B>, Error> {
        let vs = self.convert(vs, ShaderStage::Vertex, key)?;
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
            helpers: Helpers::of(&vs),
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
            ..VariantKey::default()
        };
        let (code, fragment) = (r.vs_code.clone(), r.fragment.clone());
        let made = self.vertex_program(&code, variant, fragment.as_ref());
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
    /// (`DrawBaseFromArguments`), and its pulled inputs' words; for the back end's `stages` that
    /// run it (the vertex stage, or compute for its form before a geometry stage).
    pub(super) fn helper_set(
        &mut self,
        helpers: &Helpers,
        bases: &Bases<'_, B>,
        pulled: &[(u32, VertexFormat, [u32; 4])],
        stages: StageSet,
    ) -> Result<B::BindingSet, Error> {
        let size = helpers.size as usize;
        let mut block = vec![0u8; size];
        let mut arguments = None;
        for slot in &helpers.slots {
            let words = match (slot.value, bases) {
                (HelperValue::DrawBase, Bases::Direct(b)) => {
                    self.backend.slow_paths().hit(SlowPath::DrawBaseInHelpers);
                    [b[0], b[1], 0, 0]
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
                        .slow_paths()
                        .hit(SlowPath::DrawBaseFromArguments);
                    arguments = Some((*args, offset + if *indexed { 20 } else { 16 }));
                    [(offset / 4) as u32, u32::from(*indexed), 0, 0]
                }
                (HelperValue::PulledStream { input }, _) => {
                    pulled
                        .iter()
                        .find(|p| p.0 == input)
                        .ok_or(Error::Invalid("a pulled input the draw does not fetch"))?
                        .2
                }
                _ => {
                    return Err(Error::NotImplemented(
                        "a helper value the front does not write",
                    ));
                }
            };
            let at = slot.offset as usize;
            let end = (at + 16).min(size);
            if at >= end {
                return Err(Error::Invalid("a helper slot past its block"));
            }
            let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
            block[at..end].copy_from_slice(&bytes[..end - at]);
        }
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offset = [0];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &[&block], align, &mut offset)?;
        let mut entries = vec![BindingEntry {
            binding: helper_binding(ShaderStage::Vertex),
            stages,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offset[0],
                size: size as u64,
            }),
        }];
        if let Some((args, size)) = arguments {
            entries.push(BindingEntry {
                binding: DRAW_ARGUMENTS_BINDING,
                stages,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: args,
                        offset: 0,
                        size,
                    },
                    writable: false,
                },
            });
        }
        Ok(self.backend.create_binding_set(&entries)?)
    }
}
