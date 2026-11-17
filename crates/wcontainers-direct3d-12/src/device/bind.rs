//! Root signatures at replay: each binding a program's stages read is found in the bound root
//! signature (root constants, a root descriptor, a descriptor table's range, or a static sampler),
//! its descriptor is read as the heap holds it when the list is replayed, and the draw's binding
//! sets are made from them (plan 4.5 and 5.2).

use wcontainers_gpu::{
    BindingEntry, BindingResource, BufferRange, MAX_BINDING_SETS, ReflectedBinding, RingKind,
    SlotKind,
};

use super::*;
use crate::list::{ResourceView, RootArgument};
use crate::root::{RootParameterKind, RootSignatureDesc};

/// One binding of one stage's program.
#[derive(Clone, Debug)]
pub(super) struct StageBinding {
    stages: StageSet,
    binding: ReflectedBinding,
    /// Uniform blocks: the size the program declares.
    declared: u64,
}

/// The bindings of a converted stage, with the sizes its uniform blocks declare.
pub(super) fn stage_bindings(c: &Converted, stages: StageSet) -> Vec<StageBinding> {
    let module = &c.modules[0].module;
    c.reflection
        .bindings
        .iter()
        .map(|b| {
            let declared = module
                .global_variables
                .iter()
                .find(|(_, v)| {
                    v.space == naga::AddressSpace::Uniform
                        && v.binding
                            .as_ref()
                            .is_some_and(|r| (r.group, r.binding) == (b.set, b.binding))
                })
                .map_or(0, |(_, v)| {
                    u64::from(module.types[v.ty].inner.size(module.to_ctx()))
                });
            StageBinding {
                stages,
                binding: b.clone(),
                declared,
            }
        })
        .collect()
}

/// Where a binding's resource comes from in a root signature.
#[derive(Clone, Copy, Debug)]
enum Source {
    Constants(usize),
    Descriptor(usize),
    /// A table's parameter and the descriptor's index from the table's start.
    Table(usize, u32),
    Static(usize),
}

/// `D3D12_SHADER_VISIBILITY` covers the stages.
fn visible(visibility: u32, stages: StageSet) -> bool {
    match visibility {
        0 => true,
        1 => stages == StageSet::VERTEX,
        5 => stages == StageSet::FRAGMENT,
        _ => false,
    }
}

/// The root signature's place for binding `b`.
fn locate(root: &RootSignatureDesc, b: &StageBinding) -> Option<Source> {
    let r = &b.binding;
    let range_kind = match r.kind {
        SlotKind::Texture => 0,
        SlotKind::Storage => 1,
        SlotKind::Uniform => 2,
        SlotKind::Sampler => 3,
        SlotKind::Counter => return None,
    };
    for (i, p) in root.parameters.iter().enumerate() {
        if !visible(p.visibility, b.stages) {
            continue;
        }
        match &p.kind {
            RootParameterKind::Constants {
                register, space, ..
            } if range_kind == 2 && (*register, *space) == (r.register, r.space) => {
                return Some(Source::Constants(i));
            }
            RootParameterKind::Descriptor {
                kind,
                register,
                space,
                ..
            } if *kind == [3, 4, 2, u32::MAX][range_kind as usize]
                && (*register, *space) == (r.register, r.space) =>
            {
                return Some(Source::Descriptor(i));
            }
            RootParameterKind::Table(ranges) => {
                let mut next = 0u32;
                for range in ranges {
                    let start = if range.offset == u32::MAX {
                        next
                    } else {
                        range.offset
                    };
                    if range.kind == range_kind
                        && range.space == r.space
                        && r.register >= range.register
                        && (range.count == u32::MAX || r.register - range.register < range.count)
                    {
                        return Some(Source::Table(i, start + (r.register - range.register)));
                    }
                    next = start.saturating_add(range.count);
                }
            }
            _ => {}
        }
    }
    if range_kind == 3 {
        return root
            .samplers
            .iter()
            .position(|s| {
                (s.register(), s.space()) == (r.register, r.space) && visible(s.0[12], b.stages)
            })
            .map(Source::Static);
    }
    None
}

/// Checks at pipeline creation that the root signature holds every binding the stages read, and
/// refuses what the front does not bind yet.
pub(super) fn check(root: &RootSignatureDesc, bindings: &[StageBinding]) -> Result<(), Error> {
    for b in bindings {
        if b.binding.added.is_some() || b.binding.count != 1 {
            return Err(Error::NotImplemented("arrays of resources in a shader"));
        }
        if matches!(b.binding.kind, SlotKind::Storage | SlotKind::Counter) {
            return Err(Error::NotImplemented("unordered access views in a shader"));
        }
        if locate(root, b).is_none() {
            return Err(Error::Invalid(
                "the root signature lacks a binding a shader reads",
            ));
        }
    }
    Ok(())
}

/// The arguments set for one bind point (graphics or compute) of a command list.
#[derive(Clone, Debug, Default)]
pub(super) struct RootState {
    pub root: Option<RootId>,
    pub arguments: Vec<Option<RootArgument>>,
    pub constants: Vec<Vec<u32>>,
}

impl RootState {
    /// `Set*RootSignature`: every argument is unset again.
    pub fn set_root(&mut self, root: Option<RootId>, signature: Option<&RootSignatureDesc>) {
        self.root = root;
        let n = signature.map_or(0, |s| s.parameters.len());
        self.arguments.clear();
        self.arguments.resize(n, None);
        self.constants.clear();
        self.constants.resize(n, Vec::new());
        if let Some(s) = signature {
            for (c, p) in self.constants.iter_mut().zip(&s.parameters) {
                if let RootParameterKind::Constants { count, .. } = p.kind {
                    c.resize(count as usize, 0);
                }
            }
        }
    }
}

/// A resource of a binding set, held while the set is made.
enum Held<B: Backend> {
    Uniform(B::Buffer, u64, u64),
    Texture(B::TextureView),
    Sampler(B::Sampler),
}

impl<B: Backend> Device<B> {
    /// The binding sets of a draw with pipeline `pid` and root arguments `state`. Upload buffers a
    /// binding reads are written to the GPU first.
    pub(super) fn binding_sets(
        &mut self,
        state: &RootState,
        pid: PipelineId,
    ) -> Result<[Option<B::BindingSet>; MAX_BINDING_SETS], Error> {
        let mut sets: [Option<B::BindingSet>; MAX_BINDING_SETS] = Default::default();
        let bindings = std::mem::take(&mut self.pipelines[pid.0 as usize].bindings);
        if bindings.is_empty() {
            return Ok(sets);
        }
        let result = self.resolve(state, &bindings);
        let held = match result {
            Ok(held) => held,
            Err(e) => {
                self.pipelines[pid.0 as usize].bindings = bindings;
                return Err(e);
            }
        };
        for (set, slot) in sets.iter_mut().enumerate() {
            let entries: Vec<BindingEntry<'_, B>> = held
                .iter()
                .map(|(i, h)| (&bindings[*i], h))
                .filter(|(b, _)| b.binding.set as usize == set)
                .map(|(b, h)| BindingEntry {
                    binding: b.binding.binding,
                    stages: b.stages,
                    resource: match h {
                        Held::Uniform(buffer, offset, size) => {
                            BindingResource::UniformBuffer(BufferRange {
                                buffer,
                                offset: *offset,
                                size: *size,
                            })
                        }
                        Held::Texture(view) => BindingResource::Texture(view),
                        Held::Sampler(sampler) => BindingResource::Sampler(sampler),
                    },
                })
                .collect();
            if !entries.is_empty() {
                match self.backend.create_binding_set(&entries) {
                    Ok(s) => *slot = Some(s),
                    Err(e) => {
                        drop(entries);
                        self.pipelines[pid.0 as usize].bindings = bindings;
                        return Err(e.into());
                    }
                }
            }
        }
        self.pipelines[pid.0 as usize].bindings = bindings;
        Ok(sets)
    }

    /// Each binding's resource, by its index in `bindings`.
    fn resolve(
        &mut self,
        state: &RootState,
        bindings: &[StageBinding],
    ) -> Result<Vec<(usize, Held<B>)>, Error> {
        let root_id = state
            .root
            .ok_or(Error::Invalid("drawing with no root signature"))?;
        let root = self.roots[root_id.0 as usize].clone();
        let mut held = Vec::with_capacity(bindings.len());
        for (index, b) in bindings.iter().enumerate() {
            let source = locate(&root, b).ok_or(Error::Invalid(
                "the bound root signature lacks a binding a shader reads",
            ))?;
            let argument = |i: usize| {
                state.arguments[i].ok_or(Error::Invalid("a root argument the draw reads is unset"))
            };
            let h = match (b.binding.kind, source) {
                (SlotKind::Uniform, Source::Constants(i)) => {
                    let mut bytes: Vec<u8> = state.constants[i]
                        .iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect();
                    bytes.resize(bytes.len().max(b.declared as usize).next_multiple_of(16), 0);
                    self.backend.slow_paths().hit(SlowPath::ImmediatesSpilled);
                    let align = self.backend.caps().limits.min_uniform_offset_alignment;
                    let mut offset = [0];
                    let buffer = self.backend.write_buffer_ring(
                        RingKind::Data,
                        &[&bytes],
                        align,
                        &mut offset,
                    )?;
                    Held::Uniform(buffer, offset[0], bytes.len() as u64)
                }
                (SlotKind::Uniform, Source::Descriptor(i)) => match argument(i)? {
                    RootArgument::Descriptor(address) => {
                        self.constant_buffer(address, b.declared)?
                    }
                    RootArgument::Table(_) => {
                        return Err(Error::Invalid("a table set for a root descriptor"));
                    }
                },
                (kind, Source::Table(i, index)) => {
                    let RootArgument::Table(start) = argument(i)? else {
                        return Err(Error::Invalid("a root descriptor set for a table"));
                    };
                    let handle = start + u64::from(index) * crate::address::DESCRIPTOR_SIZE;
                    match (kind, self.descriptor_at(handle)?) {
                        (SlotKind::Uniform, Descriptor::ConstantBuffer { address, .. }) => {
                            self.constant_buffer(address, b.declared)?
                        }
                        (SlotKind::Texture, Descriptor::ShaderResource(v)) => {
                            Held::Texture(self.shader_resource(&v)?)
                        }
                        (SlotKind::Sampler, Descriptor::Sampler(v)) => {
                            Held::Sampler(self.sampler(&v)?)
                        }
                        (_, Descriptor::Empty) => {
                            return Err(Error::NotImplemented("null descriptors"));
                        }
                        _ => return Err(Error::Invalid("a descriptor of another kind")),
                    }
                }
                (SlotKind::Sampler, Source::Static(i)) => {
                    let v = crate::state::static_sampler(&root.samplers[i]);
                    Held::Sampler(self.sampler(&v)?)
                }
                _ => return Err(Error::NotImplemented("this kind of root binding")),
            };
            held.push((index, h));
        }
        Ok(held)
    }

    /// A uniform binding of `size` bytes at a GPU virtual address.
    fn constant_buffer(&mut self, address: u64, size: u64) -> Result<Held<B>, Error> {
        let (id, offset) = self
            .addresses
            .find(address)
            .ok_or(Error::Invalid("a constant buffer view of no buffer"))?;
        let size = size.max(16);
        if offset + size > self.resources[id.0 as usize].desc.width {
            return Err(Error::NotImplemented(
                "a constant buffer view past its buffer's end",
            ));
        }
        self.upload(id);
        Ok(Held::Uniform(self.buffer(id)?.0.clone(), offset, size))
    }

    /// The texture view a shader resource view names.
    fn shader_resource(&mut self, v: &ResourceView) -> Result<B::TextureView, Error> {
        if v.mapping != 0x1688 {
            return Err(Error::NotImplemented(
                "component mappings other than identity",
            ));
        }
        if v.min_lod != 0 {
            return Err(Error::NotImplemented("a minimum level-of-detail clamp"));
        }
        let (texture_desc, mips) = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                ..
            }) => (*desc, desc.mip_levels),
            _ => return Err(Error::NotImplemented("buffers read through views")),
        };
        let (dimension, arrayed) = match v.dimension {
            4 | 6 => (ViewDimension::D2, false),
            5 | 7 => (ViewDimension::D2Array, true),
            8 => (ViewDimension::D3, false),
            9 => (ViewDimension::Cube, false),
            10 => (ViewDimension::CubeArray, true),
            _ => return Err(Error::NotImplemented("one-dimensional textures")),
        };
        let mip_count = if v.count == u32::MAX {
            mips.saturating_sub(v.mip)
        } else {
            v.count
        };
        let layer_count = match dimension {
            ViewDimension::D3 => 1,
            ViewDimension::Cube => 6,
            ViewDimension::CubeArray => v.layers * 6,
            _ if arrayed => v.layers,
            _ => 1,
        };
        let (format, stencil) = match v.format {
            // The stencil of a depth-stencil texture, read as integers.
            47 | 21 => (texture_desc.format, true),
            f => (dxgi(f)?, false),
        };
        self.texture_view(
            v.resource,
            TextureViewDesc {
                format,
                dimension,
                base_mip: v.mip,
                mip_count,
                base_layer: v.first_layer,
                layer_count,
                stencil,
            },
        )
    }

    /// The back end's sampler for a sampler descriptor, made once per description.
    fn sampler(&mut self, v: &crate::list::SamplerView) -> Result<B::Sampler, Error> {
        let desc = crate::state::sampler(v, self.backend.caps())?;
        if let Some(s) = self.samplers.get(&desc) {
            return Ok(s.clone());
        }
        let s = self.backend.create_sampler(&desc)?;
        self.samplers.insert(desc, s.clone());
        Ok(s)
    }

    /// Writes an upload buffer's memory to the GPU, once per `ExecuteCommandLists`.
    pub(super) fn upload(&mut self, id: ResourceId) {
        if self.uploaded.contains(&id) {
            return;
        }
        if let Some(Resource {
            heap: HeapType::Upload,
            storage:
                Storage::Buffer {
                    buffer,
                    host: Some(host),
                },
            ..
        }) = self.resources.get(id.0 as usize)
        {
            self.backend.slow_paths().hit(SlowPath::MappedUploadWhole);
            self.backend
                .write_buffer(buffer, 0, host, WriteMode::NoOverwrite);
            self.uploaded.push(id);
        }
    }
}
