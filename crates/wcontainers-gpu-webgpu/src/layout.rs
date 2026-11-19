//! Bind group layouts derived from a program's own modules and the layout keys of the sets it is drawn
//! with (plan section 4.5).

use crate::ShaderObj;
use naga::{AddressSpace, ImageClass, ImageDimension, ScalarKind, StorageAccess, TypeInner};
use wcontainers_gpu::{BindingLayoutKey, MAX_BINDING_SETS};
use wgpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

/// The texture bindings whose bit a layout key may set start here (plan section 6.4).
const FIRST_TEXTURE_BINDING: u32 = 32;

/// The bit of a layout key that stands for texture `binding`, if it has one.
pub fn texture_bit(binding: u32) -> Option<u32> {
    binding
        .checked_sub(FIRST_TEXTURE_BINDING)
        .filter(|bit| *bit < u128::BITS)
}

/// The entries of each bind group, sorted by binding, or a rule the program broke.
pub type GroupEntries = [Vec<BindGroupLayoutEntry>; MAX_BINDING_SETS];

/// The texture bits, per set, of the textures an entry point samples through a sampler: the only
/// textures whose layout depends on the bound view, since a texture only loaded from is
/// `unfilterable-float` whatever is bound.
pub fn paired(module: &naga::Module, info: &naga::valid::FunctionInfo) -> [u128; MAX_BINDING_SETS] {
    let mut sets = [0; MAX_BINDING_SETS];
    for k in info.sampling_set.iter() {
        if let Some(b) = &module.global_variables[k.image].binding
            && let Some(bit) = texture_bit(b.binding)
            && let Some(set) = sets.get_mut(b.group as usize)
        {
            *set |= 1u128 << bit;
        }
    }
    sets
}

/// Reads every global the stages' entry points use and gives each its binding type. Uniform buffers
/// get dynamic offsets in set and binding order, up to `max_dynamic`.
pub fn derive(
    stages: &[&ShaderObj],
    keys: &[BindingLayoutKey; MAX_BINDING_SETS],
    max_dynamic: u32,
) -> Result<GroupEntries, &'static str> {
    let mut groups: GroupEntries = Default::default();
    let mut sampled = [0u128; MAX_BINDING_SETS];
    for shader in stages {
        for (all, own) in sampled.iter_mut().zip(shader.paired) {
            *all |= own;
        }
    }
    for shader in stages {
        let visibility = match shader.stage {
            naga::ShaderStage::Vertex => ShaderStages::VERTEX,
            naga::ShaderStage::Fragment => ShaderStages::FRAGMENT,
            _ => ShaderStages::COMPUTE,
        };
        let module = &shader.module;
        let info = shader.info.get_entry_point(shader.entry_index);
        // Whether a texture's bit is set in the program's sampled textures and in the set's key.
        let bit_in = |image: naga::Handle<naga::GlobalVariable>, of: &dyn Fn(usize) -> u128| {
            let Some(b) = &module.global_variables[image].binding else {
                return false;
            };
            texture_bit(b.binding).is_some_and(|bit| of(b.group as usize) & (1u128 << bit) != 0)
        };
        let sampled = |image| bit_in(image, &|g| sampled.get(g).copied().unwrap_or(0));
        let unfilterable = |image| {
            bit_in(image, &|g| {
                keys.get(g).map_or(0, |k| k.unfilterable_textures)
            })
        };
        for (handle, var) in module.global_variables.iter() {
            if info[handle].is_empty() {
                continue;
            }
            let Some(binding) = &var.binding else {
                continue;
            };
            let group = binding.group as usize;
            if group >= MAX_BINDING_SETS {
                return Err("a program binds a set past the fourth");
            }
            let ty = match (&var.space, &module.types[var.ty].inner) {
                (AddressSpace::Uniform, _) => BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                (AddressSpace::Storage { access }, _) => BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: !access.contains(StorageAccess::STORE),
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                (
                    AddressSpace::Handle,
                    TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    },
                ) => {
                    let view_dimension = image_dimension(*dim, *arrayed);
                    match *class {
                        ImageClass::Sampled { kind, multi } => BindingType::Texture {
                            sample_type: match kind {
                                ScalarKind::Sint => wgpu::TextureSampleType::Sint,
                                ScalarKind::Uint => wgpu::TextureSampleType::Uint,
                                _ => wgpu::TextureSampleType::Float {
                                    filterable: !multi && sampled(handle) && !unfilterable(handle),
                                },
                            },
                            view_dimension,
                            multisampled: multi,
                        },
                        ImageClass::Depth { multi } => BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension,
                            multisampled: multi,
                        },
                        ImageClass::Storage { format, access } => BindingType::StorageTexture {
                            access: if access.contains(StorageAccess::LOAD | StorageAccess::STORE) {
                                wgpu::StorageTextureAccess::ReadWrite
                            } else if access.contains(StorageAccess::STORE) {
                                wgpu::StorageTextureAccess::WriteOnly
                            } else {
                                wgpu::StorageTextureAccess::ReadOnly
                            },
                            format: wgpu_naga_bridge::map_storage_format_from_naga(format),
                            view_dimension,
                        },
                        ImageClass::External => return Err("external textures are not bound"),
                    }
                }
                (AddressSpace::Handle, TypeInner::Sampler { comparison }) => {
                    BindingType::Sampler(if *comparison {
                        wgpu::SamplerBindingType::Comparison
                    } else if info
                        .sampling_set
                        .iter()
                        .any(|k| k.sampler == handle && unfilterable(k.image))
                    {
                        wgpu::SamplerBindingType::NonFiltering
                    } else {
                        wgpu::SamplerBindingType::Filtering
                    })
                }
                _ => return Err("a program binds a resource kind the interface does not bind"),
            };
            let entries = &mut groups[group];
            match entries.iter_mut().find(|e| e.binding == binding.binding) {
                Some(e) if e.ty == ty => e.visibility |= visibility,
                Some(_) => return Err("two stages bind one slot as different kinds"),
                None => entries.push(BindGroupLayoutEntry {
                    binding: binding.binding,
                    visibility,
                    ty,
                    count: None,
                }),
            }
        }
    }
    let mut dynamic = 0;
    for entries in &mut groups {
        entries.sort_by_key(|e| e.binding);
        let mut in_group = 0;
        for e in entries {
            if let BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset,
                ..
            } = &mut e.ty
                && dynamic < max_dynamic
                && in_group < crate::draw::MAX_GROUP_DYNAMIC
            {
                *has_dynamic_offset = true;
                dynamic += 1;
                in_group += 1;
            }
        }
    }
    Ok(groups)
}

fn image_dimension(dim: ImageDimension, arrayed: bool) -> wgpu::TextureViewDimension {
    use wgpu::TextureViewDimension as V;
    match (dim, arrayed) {
        (ImageDimension::D1, _) => V::D1,
        (ImageDimension::D2, false) => V::D2,
        (ImageDimension::D2, true) => V::D2Array,
        (ImageDimension::D3, _) => V::D3,
        (ImageDimension::Cube, false) => V::Cube,
        (ImageDimension::Cube, true) => V::CubeArray,
    }
}
