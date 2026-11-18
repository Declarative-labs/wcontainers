//! Root signatures at replay: each binding a program's stages read is found in the bound root
//! signature (root constants, a root descriptor, a descriptor table's range, or a static sampler),
//! its descriptor is read as the heap holds it when the list is replayed, and the draw's binding
//! sets are made from them (plan 4.5 and 5.2).

use wcontainers_gpu::{
    BindingEntry, BindingResource, BufferRange, MAX_BINDING_SETS, ReflectedBinding, SlotKind,
};

use super::*;
use crate::list::{ResourceView, RootArgument};
use crate::root::{RootParameterKind, RootSignatureDesc};

/// One binding of one stage's program.
#[derive(Clone, Debug)]
pub(super) struct StageBinding {
    /// The back end's stages that run the program.
    stages: StageSet,
    /// The API stage: the `D3D12_SHADER_VISIBILITY` naming it, 0 for compute.
    pub(super) visibility: u32,
    pub(super) binding: ReflectedBinding,
    /// Uniform blocks: the size the program declares.
    declared: u64,
    /// How the program reads the resource.
    form: Form,
    /// Textures: the shape a null view of it takes.
    shape: Option<null::Shape>,
    /// Textures read by comparison, which the back end samples only in depth formats.
    compared: bool,
    /// A typed buffer read through a storage buffer (`HelperValue::TexelBufferView` names it).
    texel: bool,
    /// A storage texture the program reads through a buffer of its texels
    /// (`HelperValue::TexelLayout` names it).
    through: bool,
}

/// The null texture shapes `bindings` may read, and whether each is written.
pub(super) fn null_shapes(bindings: &[StageBinding]) -> Vec<(null::Shape, bool)> {
    bindings
        .iter()
        .filter_map(|b| {
            let writable = matches!(b.form, Form::Image { writable: true });
            b.shape.map(|s| (s, writable))
        })
        .collect()
}

/// The form of a resource binding in the converted program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    /// A uniform block, sampler or sampled texture.
    Plain,
    /// A buffer of the storage address space (raw and structured views); writable or not.
    Buffer { writable: bool },
    /// A storage texture; writable or not.
    Image { writable: bool },
}

/// `D3D12_SHADER_VISIBILITY` of each graphics stage.
pub(super) const VERTEX: u32 = 1;
pub(super) const GEOMETRY: u32 = 4;
pub(super) const PIXEL: u32 = 5;

/// The bindings of a converted stage of the API's `visibility` (0 for compute), run by the back
/// end's `stages`, with the sizes its uniform blocks declare. Bindings the conversion added for
/// pulled vertex streams are left out: the draw binds them.
pub(super) fn stage_bindings(
    c: &Converted,
    visibility: u32,
    stages: StageSet,
) -> Vec<StageBinding> {
    let module = &c.modules[0].module;
    c.reflection
        .bindings
        .iter()
        .filter(|b| {
            !matches!(
                b.added,
                Some(wcontainers_gpu::AddedBinding::PulledStream(_))
            )
        })
        .map(|b| {
            let global = module.global_variables.iter().find(|(_, v)| {
                v.binding
                    .as_ref()
                    .is_some_and(|r| (r.group, r.binding) == (b.set, b.binding))
            });
            let (mut declared, mut form, mut shape) = (0, Form::Plain, None);
            let mut compared = false;
            if let Some((_, v)) = global {
                shape = null::shape(&module.types[v.ty].inner);
                compared = matches!(
                    module.types[v.ty].inner,
                    naga::TypeInner::Image {
                        class: naga::ImageClass::Depth { .. },
                        ..
                    }
                );
                match (v.space, &module.types[v.ty].inner) {
                    (naga::AddressSpace::Uniform, ty) => {
                        declared = u64::from(ty.size(module.to_ctx()));
                    }
                    (naga::AddressSpace::Storage { access }, _) => {
                        form = Form::Buffer {
                            writable: access.contains(naga::StorageAccess::STORE),
                        };
                    }
                    (
                        _,
                        naga::TypeInner::Image {
                            class: naga::ImageClass::Storage { access, .. },
                            ..
                        },
                    ) => {
                        form = Form::Image {
                            writable: access.contains(naga::StorageAccess::STORE),
                        };
                    }
                    _ => {}
                }
            }
            let texel = c.reflection.helpers.iter().any(|h| {
                h.value
                    == HelperValue::TexelBufferView {
                        set: b.set,
                        binding: b.binding,
                    }
            });
            let through = b.kind == SlotKind::Storage
                && c.reflection
                    .helpers
                    .iter()
                    .any(|h| h.value == HelperValue::TexelLayout { slot: b.slot() });
            StageBinding {
                stages,
                visibility,
                binding: b.clone(),
                declared,
                form,
                shape,
                compared,
                texel,
                through,
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

/// The helper value giving a typed buffer binding's place and length.
fn texel_value(b: &StageBinding) -> HelperValue {
    HelperValue::TexelBufferView {
        set: b.binding.set,
        binding: b.binding.binding,
    }
}

/// A root parameter's `D3D12_SHADER_VISIBILITY` covers the binding's stage.
fn visible(visibility: u32, b: &StageBinding) -> bool {
    visibility == 0 || visibility == b.visibility
}

/// The root signature's place for binding `b`.
fn locate(root: &RootSignatureDesc, b: &StageBinding) -> Option<Source> {
    let r = &b.binding;
    let range_kind = match r.kind {
        SlotKind::Texture => 0,
        SlotKind::Storage => 1,
        SlotKind::Uniform => 2,
        SlotKind::Sampler => 3,
        // A counter is found where its view is.
        SlotKind::Counter => 1,
    };
    for (i, p) in root.parameters.iter().enumerate() {
        if !visible(p.visibility, b) {
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
            .position(|s| (s.register(), s.space()) == (r.register, r.space) && visible(s.0[12], b))
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
    pub root: Option<RootSignature>,
    pub arguments: Vec<Option<RootArgument>>,
    pub constants: Vec<Vec<u32>>,
}

impl RootState {
    /// `Set*RootSignature`: every argument is unset again.
    pub fn set_root(&mut self, root: Option<RootSignature>) {
        let n = root.as_ref().map_or(0, |s| s.parameters.len());
        self.arguments.clear();
        self.arguments.resize(n, None);
        self.constants.clear();
        self.constants.resize(n, Vec::new());
        if let Some(s) = &root {
            for (c, p) in self.constants.iter_mut().zip(&s.parameters) {
                if let RootParameterKind::Constants { count, .. } = p.kind {
                    c.resize(count as usize, 0);
                }
            }
        }
        self.root = root;
    }
}

/// A resource of a binding set, held while the set is made.
pub(super) enum Held<B: Backend> {
    Uniform(B::Buffer, u64, u64),
    Texture(B::TextureView),
    Sampler(B::Sampler),
    Storage {
        buffer: B::Buffer,
        offset: u64,
        size: u64,
        writable: bool,
    },
    StorageTexture(B::TextureView, bool),
}

impl<B: Backend> Device<B> {
    /// The binding sets of a draw or dispatch with pipeline `pid` and root arguments `state`:
    /// of the API stages `only` (`D3D12_SHADER_VISIBILITY`s), or of every stage where it is
    /// empty. `extra` are
    /// read-only storage buffers (binding, buffer, offset, size) of the pulled vertex streams,
    /// added to set 0 for the back end's stages `extra_stages`.
    pub(super) fn binding_sets(
        &mut self,
        state: &RootState,
        pid: PipelineId,
        only: &[u32],
        extra: &[(u32, &B::Buffer, u64, u64)],
        extra_stages: StageSet,
    ) -> Result<[Option<B::BindingSet>; MAX_BINDING_SETS], Error> {
        let mut sets: [Option<B::BindingSet>; MAX_BINDING_SETS] = Default::default();
        let mut bindings = std::mem::take(&mut self.pipelines[pid.0 as usize].bindings);
        let mut others = Vec::new();
        if !only.is_empty() {
            (bindings, others) = bindings
                .into_iter()
                .partition(|b| only.contains(&b.visibility));
        }
        let restore = |d: &mut Self, mut bindings: Vec<StageBinding>, others: Vec<StageBinding>| {
            bindings.extend(others);
            d.pipelines[pid.0 as usize].bindings = bindings;
        };
        if bindings.is_empty() && extra.is_empty() {
            restore(self, bindings, others);
            return Ok(sets);
        }
        let result = self.resolve(state, &bindings);
        let held = match result {
            Ok(held) => held,
            Err(e) => {
                restore(self, bindings, others);
                return Err(e);
            }
        };
        for (set, slot) in sets.iter_mut().enumerate() {
            let entries: Vec<BindingEntry<'_, B>> =
                held.iter()
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
                            Held::Storage {
                                buffer,
                                offset,
                                size,
                                writable,
                            } => BindingResource::StorageBuffer {
                                range: BufferRange {
                                    buffer,
                                    offset: *offset,
                                    size: *size,
                                },
                                writable: *writable,
                            },
                            Held::StorageTexture(view, writable) => {
                                BindingResource::StorageTexture {
                                    view,
                                    writable: *writable,
                                }
                            }
                        },
                    })
                    .chain(extra.iter().filter(|_| set == 0).map(
                        |&(binding, buffer, offset, size)| BindingEntry {
                            binding,
                            stages: extra_stages,
                            resource: BindingResource::StorageBuffer {
                                range: BufferRange {
                                    buffer,
                                    offset,
                                    size,
                                },
                                writable: false,
                            },
                        },
                    ))
                    .collect();
            if !entries.is_empty() {
                match self.backend.create_binding_set(&entries) {
                    Ok(s) => *slot = Some(s),
                    Err(e) => {
                        drop(entries);
                        restore(self, bindings, others);
                        return Err(e.into());
                    }
                }
            }
        }
        restore(self, bindings, others);
        Ok(sets)
    }

    /// Whether the root arguments the bindings of pipeline `pid` read are all set. Direct3D 12
    /// leaves a draw or dispatch reading one not set since its root signature was set undefined;
    /// the replay skips it, with a warning.
    pub(super) fn arguments_set(&mut self, state: &RootState, pid: PipelineId) -> bool {
        let Some(root) = &state.root else {
            return true;
        };
        let set = self.pipelines[pid.0 as usize]
            .bindings
            .iter()
            .all(|b| match locate(root, b) {
                Some(Source::Descriptor(i) | Source::Table(i, _)) => state.arguments[i].is_some(),
                _ => true,
            });
        if !set {
            self.messages.push(
                "warning: a draw or dispatch reading a root argument unset since its root \
                 signature was set was skipped"
                    .into(),
            );
        }
        set
    }

    /// Each binding's resource, by its index in `bindings`.
    fn resolve(
        &mut self,
        state: &RootState,
        bindings: &[StageBinding],
    ) -> Result<Vec<(usize, Held<B>)>, Error> {
        let root = state
            .root
            .clone()
            .ok_or(Error::Invalid("drawing with no root signature"))?;
        let mut held = Vec::with_capacity(bindings.len());
        self.bound_formats.clear();
        self.bound_helpers.clear();
        self.bound_sampling.clear();
        self.bound_through.clear();
        self.bound_clamps.clear();
        self.bound_samplers.clear();
        self.bound_unfiltered.clear();
        // Views read through buffers before a binding failed.
        self.through.clear();
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
                        wcontainers_gpu::RingKind::Data,
                        &[&bytes],
                        align,
                        &mut offset,
                    )?;
                    Held::Uniform(buffer, offset[0], bytes.len() as u64)
                }
                (kind, Source::Descriptor(i)) => {
                    let RootArgument::Descriptor(address) = argument(i)? else {
                        return Err(Error::Invalid("a table set for a root descriptor"));
                    };
                    match (kind, b.form) {
                        (SlotKind::Counter, _) => {
                            return Err(Error::Invalid(
                                "a counter of a root unordered access view",
                            ));
                        }
                        (SlotKind::Uniform, _) => self.constant_buffer(address, b.declared)?,
                        (_, Form::Buffer { writable }) => self.root_buffer(address, writable)?,
                        _ => {
                            return Err(Error::Invalid("a root descriptor for a shader's texture"));
                        }
                    }
                }
                (kind, Source::Table(i, index)) => {
                    let RootArgument::Table(start) = argument(i)? else {
                        return Err(Error::Invalid("a root descriptor set for a table"));
                    };
                    let handle = start + u64::from(index) * crate::address::DESCRIPTOR_SIZE;
                    match (kind, self.descriptor_at(handle)?) {
                        (SlotKind::Uniform, Descriptor::ConstantBuffer { address, .. }) => {
                            self.constant_buffer(address, b.declared)?
                        }
                        (SlotKind::Texture, Descriptor::ShaderResource(v)) if b.texel => {
                            self.texel_view(&v, b, false)?
                        }
                        (SlotKind::Texture, Descriptor::ShaderResource(v))
                            if b.form != Form::Plain =>
                        {
                            self.buffer_view(&v, false)?
                        }
                        (SlotKind::Texture, Descriptor::ShaderResource(v)) => {
                            let view = self.shader_resource(&v, b.compared)?;
                            self.note_clamp(&v, b)?;
                            self.note_filtering(&v, b)?;
                            let components = dxgi(v.format)?.components();
                            self.bound_helpers.push((
                                b.visibility,
                                HelperValue::TextureComponents {
                                    texture: b.binding.slot(),
                                },
                                pull::words(&[components]),
                            ));
                            Held::Texture(view)
                        }
                        (SlotKind::Counter, Descriptor::UnorderedAccess(v)) => {
                            self.view_counter(&v)?
                        }
                        (SlotKind::Storage, Descriptor::UnorderedAccess(v)) => match b.form {
                            Form::Buffer { writable } if b.texel => {
                                self.texel_view(&v, b, writable)?
                            }
                            Form::Buffer { writable } if b.through => self.texels_through(
                                &v,
                                b,
                                writable,
                                SlowPath::AtomicsThroughBuffer,
                            )?,
                            Form::Image { writable } if !self.stores(v.format) => self
                                .texels_through(
                                    &v,
                                    b,
                                    writable,
                                    SlowPath::StorageFormatThroughBuffer,
                                )?,
                            Form::Buffer { writable } => self.buffer_view(&v, writable)?,
                            Form::Image { writable } => {
                                self.note(v.resource, writable);
                                let view = self.storage_texture(&v)?;
                                self.bound_formats.push((
                                    b.visibility,
                                    (true, b.binding.slot()),
                                    dxgi(v.format)?,
                                ));
                                Held::StorageTexture(view, writable)
                            }
                            Form::Plain => return Err(Error::Invalid("a storage binding")),
                        },
                        (SlotKind::Sampler, Descriptor::Sampler(v)) => {
                            Held::Sampler(self.sampler(&v, b)?)
                        }
                        (SlotKind::Sampler, Descriptor::Empty) => {
                            return Err(Error::Invalid("an empty sampler descriptor"));
                        }
                        (kind, Descriptor::Empty) => self.null(kind, b)?,
                        _ => return Err(Error::Invalid("a descriptor of another kind")),
                    }
                }
                (SlotKind::Sampler, Source::Static(i)) => {
                    let v = crate::state::static_sampler(&root.samplers[i]);
                    Held::Sampler(self.sampler(&v, b)?)
                }
                _ => return Err(Error::NotImplemented("this kind of root binding")),
            };
            held.push((index, h));
        }
        Ok(held)
    }

    /// The zeros a null descriptor binds for `b`.
    fn null(&mut self, kind: SlotKind, b: &StageBinding) -> Result<Held<B>, Error> {
        Ok(match (kind, b.form) {
            (SlotKind::Uniform, _) => Held::Uniform(
                self.null_buffer(false)?,
                0,
                b.declared.clamp(16, null::NULL_BYTES).next_multiple_of(16),
            ),
            (_, Form::Buffer { writable }) => {
                if b.through {
                    // A null view holds no texel.
                    self.bound_helpers.push((
                        b.visibility,
                        HelperValue::TexelLayout {
                            slot: b.binding.slot(),
                        },
                        pull::words(&[0; 5]),
                    ));
                }
                if b.texel {
                    // A null typed buffer holds no element; its format stays the program's.
                    self.bound_helpers
                        .push((b.visibility, texel_value(b), pull::words(&[0, 0])));
                }
                Held::Storage {
                    buffer: self.null_buffer(writable)?,
                    offset: 0,
                    size: null::NULL_BYTES,
                    writable,
                }
            }
            (_, Form::Image { writable }) => {
                let shape = b
                    .shape
                    .ok_or(Error::NotImplemented("null views of this storage texture"))?;
                // A null view's format is the program's own: its reads are zeros and its
                // writes are dropped in any format.
                self.bound_formats
                    .push((b.visibility, (true, b.binding.slot()), shape.format));
                Held::StorageTexture(self.null_view(shape, writable)?, writable)
            }
            (_, Form::Plain) => {
                let shape = b.shape.ok_or(Error::NotImplemented(
                    "null views of one-dimensional textures",
                ))?;
                // Every component of a null view reads zero.
                self.bound_helpers.push((
                    b.visibility,
                    HelperValue::TextureComponents {
                        texture: b.binding.slot(),
                    },
                    pull::words(&[0b1111]),
                ));
                Held::Texture(self.null_view(shape, false)?)
            }
        })
    }

    /// A uniform binding of `size` bytes at a GPU virtual address.
    fn constant_buffer(&mut self, address: u64, size: u64) -> Result<Held<B>, Error> {
        let (id, offset) = self
            .addresses
            .find(address)
            .ok_or(Error::Invalid("a constant buffer view of no buffer"))?;
        let size = size.max(16);
        let width = self.resources[id.0 as usize].desc.width;
        if offset + size > super::stored_size(width, self.backend.caps().limits.max_buffer_size) {
            return Err(Error::NotImplemented(
                "a constant buffer view past its buffer's end",
            ));
        }
        self.upload(id)?;
        self.note(id, false);
        Ok(Held::Uniform(self.buffer(id)?.0.clone(), offset, size))
    }

    /// The counter of an unordered access view, as a writable storage word; zeros whose writes
    /// are dropped for a view made without one (Direct3D 12 leaves that undefined).
    fn view_counter(&mut self, v: &ResourceView) -> Result<Held<B>, Error> {
        if v.counter() == 0 {
            return Ok(Held::Storage {
                buffer: self.null_buffer(true)?,
                offset: 0,
                size: 4,
                writable: true,
            });
        }
        let (id, offset) = self
            .addresses
            .find(v.counter())
            .ok_or(Error::Invalid("a view counter in no buffer"))?;
        self.storage_range(id, offset, 4, true)
    }

    /// A root shader resource or unordered access view: the buffer from `address` to its end.
    fn root_buffer(&mut self, address: u64, writable: bool) -> Result<Held<B>, Error> {
        let (id, offset) = self
            .addresses
            .find(address)
            .ok_or(Error::Invalid("a root buffer view of no buffer"))?;
        let size = self.resources[id.0 as usize].desc.width - offset;
        self.storage_range(id, offset, size, writable)
    }

    /// A raw or structured buffer view, read or written as a storage buffer.
    fn buffer_view(&mut self, v: &ResourceView, writable: bool) -> Result<Held<B>, Error> {
        let raw = v.flags & 1 != 0;
        let element = match (raw, v.stride) {
            (true, _) => 4,
            (false, 0) => {
                return Err(Error::NotImplemented(
                    "typed buffer views bound as storage buffers",
                ));
            }
            (false, stride) => u64::from(stride),
        };
        if !matches!(
            self.resources.get(v.resource.0 as usize),
            Some(Resource {
                storage: Storage::Buffer { .. },
                ..
            })
        ) {
            return Err(Error::Invalid("a buffer view of a texture"));
        }
        let offset = v.first_element * element;
        let size = u64::from(v.count) * element;
        self.storage_range(v.resource, offset, size, writable)
    }

    /// A typed buffer view read through a storage buffer of its words: bound from its first
    /// element rounded down to the storage alignment, its place in the bound range and its
    /// element count in the program's helper block, its format noted for the program's variant.
    fn texel_view(
        &mut self,
        v: &ResourceView,
        b: &StageBinding,
        writable: bool,
    ) -> Result<Held<B>, Error> {
        if v.flags & 1 != 0 || v.stride != 0 {
            return Err(Error::Invalid("a raw or structured view of a typed buffer"));
        }
        let width = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Buffer { .. },
                desc,
                ..
            }) => desc.width,
            _ => return Err(Error::Invalid("a buffer view of a texture")),
        };
        let format = dxgi(v.format)?;
        let element = u64::from(format.block().2);
        let offset = v.first_element * element;
        let size = u64::from(v.count) * element;
        if offset + size > width {
            return Err(Error::Invalid("a buffer view past its buffer's end"));
        }
        let align = u64::from(self.backend.caps().limits.min_storage_offset_alignment);
        let start = offset - offset % align;
        let rest = offset - start;
        let bound = (rest + size).max(4).min(width.next_multiple_of(4) - start);
        self.backend
            .slow_paths()
            .hit(SlowPath::TexelBufferAsStorage);
        self.bound_formats.push((
            b.visibility,
            (b.binding.kind != SlotKind::Texture, b.binding.slot()),
            format,
        ));
        self.bound_helpers.push((
            b.visibility,
            texel_value(b),
            pull::words(&[rest as u32, v.count]),
        ));
        self.storage_range(v.resource, start, bound, writable)
    }

    /// A storage binding of bytes `offset..offset + size` of buffer `id`.
    fn storage_range(
        &mut self,
        id: ResourceId,
        offset: u64,
        size: u64,
        writable: bool,
    ) -> Result<Held<B>, Error> {
        let align = u64::from(self.backend.caps().limits.min_storage_offset_alignment);
        if !offset.is_multiple_of(align) {
            // The program would add the rest from its helper constants (`StorageOffset`), a
            // variant the converter does not build yet.
            return Err(Error::NotImplemented(
                "a buffer view at an offset the back end cannot bind",
            ));
        }
        if offset + size > self.resources[id.0 as usize].desc.width.next_multiple_of(4) {
            return Err(Error::Invalid("a buffer view past its buffer's end"));
        }
        if writable && self.resources[id.0 as usize].heap == HeapType::Shared {
            return Err(Error::NotImplemented(
                "unordered access buffers in a custom heap the CPU sees",
            ));
        }
        self.upload(id)?;
        self.note(id, writable);
        Ok(Held::Storage {
            buffer: self.buffer(id)?.0.clone(),
            offset,
            size: size.next_multiple_of(4),
            writable,
        })
    }

    /// Notes a shader resource view's minimum level-of-detail clamp (`ResourceMinLODClamp`, in
    /// the resource's levels) for the program's variant, with its `HelperValue::MinLod`: the
    /// clamp in the view's levels and the view's level count.
    fn note_clamp(&mut self, v: &ResourceView, b: &StageBinding) -> Result<(), Error> {
        let clamp = f32::from_bits(v.min_lod) - v.mip as f32;
        if clamp <= 0.0 {
            return Ok(());
        }
        let texture = b.binding.slot();
        if !matches!(b.visibility, 0 | PIXEL) || texture >= 128 {
            return Err(Error::NotImplemented(
                "a minimum level-of-detail clamp in a vertex or geometry shader or past \
                 texture slot 127",
            ));
        }
        let levels = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                ..
            }) if v.count == u32::MAX => desc.mip_levels.saturating_sub(v.mip),
            _ => v.count,
        };
        self.bound_clamps.push((b.visibility, texture, clamp));
        self.bound_helpers.push((
            b.visibility,
            HelperValue::MinLod { texture },
            pull::words(&[clamp.to_bits(), levels]),
        ));
        Ok(())
    }

    /// Notes a texture the back end cannot filter in the view's format (a depth texture read as
    /// its depth, or a float format without filtering), which the program then filters where a
    /// filtering sampler samples it (`formats.rs`).
    fn note_filtering(&mut self, v: &ResourceView, b: &StageBinding) -> Result<(), Error> {
        if b.compared || !matches!(b.visibility, 0 | PIXEL) {
            return Ok(());
        }
        let stored = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                ..
            }) => desc.format,
            _ => return Ok(()),
        };
        let format = dxgi(v.format)?;
        let filters = |f: Format| {
            self.backend
                .caps()
                .formats
                .get(f)
                .contains(FormatUse::FILTER)
        };
        // A view reading a depth texture's depth reads a texture of the depth format.
        let read = if stored.is_depth_stencil() {
            stored
        } else {
            format
        };
        if !filters(read) {
            self.bound_unfiltered.push((b.visibility, b.binding.slot()));
        }
        Ok(())
    }

    /// The back end's sampler for `v` with its minimum level raised to `clamp`, for a merged
    /// sampler of a clamped program.
    pub(super) fn clamped_sampler(
        &mut self,
        v: &crate::list::SamplerView,
        clamp: f32,
    ) -> Result<B::Sampler, Error> {
        let mut desc = crate::state::sampler(v, self.backend.caps())?;
        desc.lod_range[0] = f32::from_bits(desc.lod_range[0]).max(clamp).to_bits();
        if let Some(s) = self.samplers.get(&desc) {
            return Ok(s.clone());
        }
        let s = self.backend.create_sampler(&desc)?;
        self.samplers.insert(desc, s.clone());
        Ok(s)
    }

    /// The back end stores into textures of DXGI format `value`.
    fn stores(&self, value: u32) -> bool {
        dxgi(value).is_ok_and(|f| {
            self.backend
                .caps()
                .formats
                .get(f)
                .contains(FormatUse::STORAGE_WRITE)
        })
    }

    /// The texture view an unordered access view of a texture names.
    fn storage_texture(&mut self, v: &ResourceView) -> Result<B::TextureView, Error> {
        let dimension = match v.dimension {
            4 => ViewDimension::D2,
            5 => ViewDimension::D2Array,
            8 => ViewDimension::D3,
            1 => {
                return Err(Error::NotImplemented(
                    "typed buffer views bound as storage textures",
                ));
            }
            _ => return Err(Error::NotImplemented("one-dimensional storage textures")),
        };
        // Array sizes past the texture's layers (such as -1) take the layers left, as for
        // shader resource views.
        let layers = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                ..
            }) => desc.depth_or_layers,
            _ => return Err(Error::Invalid("a texture view of no texture")),
        };
        let layer_count = match dimension {
            ViewDimension::D2Array => v.layers.min(layers.saturating_sub(v.first_layer)),
            _ => 1,
        };
        if dimension == ViewDimension::D3 && v.first_layer != 0 {
            return Err(Error::NotImplemented(
                "storage views of part of a 3D texture",
            ));
        }
        self.texture_view(
            v.resource,
            TextureViewDesc {
                format: dxgi(v.format)?,
                dimension,
                base_mip: v.mip,
                mip_count: 1,
                base_layer: v.first_layer,
                layer_count,
                stencil: false,
            },
        )
    }

    /// The texture view a shader resource view names; `compared` for a program reading it by
    /// comparison.
    fn shader_resource(
        &mut self,
        v: &ResourceView,
        compared: bool,
    ) -> Result<B::TextureView, Error> {
        if v.mapping != 0x1688 {
            return Err(Error::NotImplemented(
                "component mappings other than identity",
            ));
        }
        let (texture_desc, mips) = match self.resources.get(v.resource.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                ..
            }) => (*desc, desc.mip_levels),
            _ => return Err(Error::NotImplemented("buffers read through views")),
        };
        // Comparison sampling reads a texture not stored in a depth format through its depth twin.
        let (resource, depth) = if compared && !texture_desc.format.is_depth_stencil() {
            let (twin, format) = self.depth_twin(v.resource)?;
            (twin, Some(format))
        } else {
            (v.resource, None)
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
        // Array sizes past the texture's layers (such as -1) take the layers left.
        let left = texture_desc.depth_or_layers.saturating_sub(v.first_layer);
        let layer_count = match dimension {
            ViewDimension::D3 => 1,
            ViewDimension::Cube => 6,
            ViewDimension::CubeArray => v.layers.min(left / 6) * 6,
            _ if arrayed => v.layers.min(left),
            _ => 1,
        };
        self.note(resource, false);
        // The stencil of a depth-stencil texture, which Direct3D gives in the second component
        // and the interface's stencil views in the first: refused until views can swizzle.
        if matches!(v.format, 22 | 47) {
            return Err(Error::NotImplemented(
                "stencil read through a view (its component needs a view swizzle)",
            ));
        }
        let format = match depth {
            Some(f) => f,
            None => dxgi(v.format)?,
        };
        self.texture_view(
            resource,
            TextureViewDesc {
                format,
                dimension,
                base_mip: v.mip,
                mip_count,
                base_layer: v.first_layer,
                layer_count,
                stencil: false,
            },
        )
    }

    /// The back end's sampler for a sampler descriptor bound for `b`, made once per description;
    /// what the program applies itself is noted for its variant, with its helper values.
    fn sampler(
        &mut self,
        v: &crate::list::SamplerView,
        b: &StageBinding,
    ) -> Result<B::Sampler, Error> {
        let caps = self.backend.caps();
        let desc = crate::state::sampler(v, caps)?;
        self.bound_samplers
            .push((b.visibility, b.binding.slot(), *v));
        let (bias, border) = crate::state::in_program(v, caps);
        if bias || border {
            if !matches!(b.visibility, 0 | PIXEL) {
                return Err(Error::NotImplemented(
                    "level-of-detail bias, border or mirror-once sampling in a vertex or \
                     geometry shader",
                ));
            }
            let sampler = b.binding.slot();
            if sampler >= 16 {
                return Err(Error::NotImplemented(
                    "sampling in the program through a sampler slot past 15",
                ));
            }
            self.bound_sampling
                .push((b.visibility, sampler, bias, border));
            let state = crate::state::sampler_state(v, &desc);
            self.bound_helpers.extend([
                (
                    b.visibility,
                    HelperValue::LodBias { sampler },
                    pull::words(&[v.lod_bias]),
                ),
                (
                    b.visibility,
                    HelperValue::BorderColor { sampler },
                    pull::words(&v.border),
                ),
                (b.visibility, HelperValue::SamplerState { sampler }, state),
            ]);
        }
        if let Some(s) = self.samplers.get(&desc) {
            return Ok(s.clone());
        }
        let s = self.backend.create_sampler(&desc)?;
        self.samplers.insert(desc, s.clone());
        Ok(s)
    }

    /// Writes an upload or shared buffer's memory to the GPU, once per `ExecuteCommandLists`. Of a
    /// shared buffer, the bytes GPU work wrote and the program has not got yet are left out (the
    /// GPU's copy is newer); those another buffer over the same memory got are waited for first.
    pub(super) fn upload(&mut self, id: ResourceId) -> Result<(), Error> {
        let Some(Resource {
            heap,
            storage: Storage::Buffer {
                host: Some(host), ..
            },
            ..
        }) = self.resources.get(id.0 as usize)
        else {
            return Ok(());
        };
        if !heap.uploads() || self.uploaded.contains(&id) {
            return Ok(());
        }
        let mut skip = Vec::new();
        if *heap == HeapType::Shared {
            let (memory, at, len) = (host.memory.clone(), host.offset, host.len);
            let mut wait = Vec::new();
            for (i, r) in self.reads.iter().enumerate() {
                let Some(Resource {
                    storage: Storage::Buffer { host: Some(h), .. },
                    ..
                }) = self.resources.get(r.resource.0 as usize)
                else {
                    continue;
                };
                let start = h.offset + r.offset;
                let (start, end) = (start.max(at), (start + r.size).min(at + len));
                if !Rc::ptr_eq(&h.memory, &memory) || start >= end {
                    continue;
                }
                if r.resource == id {
                    skip.push((start - at, end - at));
                } else if r.submission.is_some() {
                    wait.push(i);
                } else {
                    return Err(Error::NotImplemented(
                        "a shared buffer read after an overlapping one was written in the same submission",
                    ));
                }
            }
            if !wait.is_empty() {
                self.backend.slow_paths().hit(SlowPath::ReadBackWait);
            }
            for (k, i) in wait.into_iter().enumerate() {
                self.finish_read(i - k);
            }
            skip.sort_unstable();
        }
        let Some(Resource {
            storage:
                Storage::Buffer {
                    buffer,
                    host: Some(host),
                },
            ..
        }) = self.resources.get_mut(id.0 as usize)
        else {
            return Ok(());
        };
        self.backend.slow_paths().hit(SlowPath::MappedUploadWhole);
        let bytes = host.bytes();
        let mut from = 0;
        for (start, end) in skip.into_iter().chain([(bytes.len(), bytes.len())]) {
            if start > from {
                self.backend.write_buffer(
                    buffer,
                    from as u64,
                    &bytes[from..start],
                    WriteMode::NoOverwrite,
                );
            }
            from = from.max(end);
        }
        self.uploaded.push(id);
        Ok(())
    }
}
