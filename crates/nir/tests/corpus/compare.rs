//! A converted shader held to the reference (plan 15.3): the program as given, run by lavapipe, and
//! the route's module, written back to SPIR-V by naga and run by lavapipe the same way, on the same
//! seeded inputs; their outputs compared, integers exactly and floats within a stated tolerance.

use crate::interface::{self, Kind, Slot, Ty};
use crate::lavapipe::{self, Attribute, Descriptor, Element, Image, Job, Lavapipe, Outputs};
use ash::vk;
use naga::{
    AddressSpace, ArraySize, Binding, ImageClass, ImageDimension, Interpolation, Module,
    ScalarKind, ShaderStage, Span, TypeInner,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use wcontainers_gpu::{reflect::ReflectedBinding, reflect::SlotKind, resource::CUBE_FACES};

/// The knob of the seed of every input filled in (and of the corpus's sample), and its value when
/// unset.
pub const SEED: &str = "WC_NIR_SEED";
pub const DEFAULT_SEED: u64 = 0x5eed;
/// The knob of how many input sets each comparison runs.
pub const INPUT_SETS: &str = "WC_NIR_INPUT_SETS";
/// The entry point of the vertex program made to feed a fragment program.
pub const FEEDER_ENTRY: &str = "feed";
/// Floats filled in are in [-FLOAT_RANGE, FLOAT_RANGE); integers in [-INTEGER_RANGE,
/// INTEGER_RANGE), unsigned ones from 0. Integers stay small: programs use them as indices and counts.
const FLOAT_RANGE: f32 = 2.0;
const INTEGER_RANGE: i32 = 4;
/// Elements of a runtime-sized array in a storage buffer.
pub const RUNTIME_ELEMENTS: u32 = 64;
/// The size of each dimension of a texture, and its layers when it is an array.
const TEXTURE_SIZE: u32 = 4;
const TEXTURE_LAYERS: u32 = 2;
/// The samples of a multisampled texture.
const MULTISAMPLE_SAMPLES: vk::SampleCountFlags = vk::SampleCountFlags::TYPE_4;
/// Where in its pixel a point is drawn, and the depth of every point.
const PIXEL_CENTRE: f32 = 0.5;
const POINT_DEPTH: f32 = 0.5;
/// The workgroups a compute program is dispatched with, along x.
pub const COMPUTE_GROUPS: u32 = 2;
/// The bits every colour target starts as, so that a pixel written by neither run is equal in both.
pub const CLEAR_BITS: u32 = 0x7fc0_dead;
/// Floats are equal when their bits are, both are NaN, they are within this many units in the last
/// place, or both are within FLOAT_ABSOLUTE of each other (cancellation near zero). Both runs are on
/// the same driver and processor, so only the order and fusing of operations can differ.
const FLOAT_ULPS: u32 = 1 << 10;
const FLOAT_ABSOLUTE: f32 = 1.0 / (1 << 20) as f32;
/// How many differing values a verdict names.
pub const NAMED_DIFFERENCES: usize = 3;
/// The SPIR-V version the route's module is written in for lavapipe (Vulkan 1.1's).
pub const SPIRV_VERSION: (u8, u8) = (1, 3);

/// Choices for one comparison.
#[derive(Clone, Copy)]
pub struct CompareOptions {
    pub seed: u64,
    /// Vertices, or fragments, each with inputs of its own.
    pub input_sets: u32,
}

/// What the comparison found.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Verdict {
    Matched,
    /// The outputs differ: the first differences named.
    Differs(String),
    /// Not compared, for the named reason (the harness's limits, or the reference's own failure).
    NotRun(String),
}

/// The bytes a value of each kind is filled from, and how it is compared.
pub fn fill_scalar(rng: &mut ChaCha8Rng, kind: Kind) -> u32 {
    match kind {
        Kind::Float => rng.random_range(-FLOAT_RANGE..FLOAT_RANGE).to_bits(),
        Kind::Sint => rng
            .random_range(-INTEGER_RANGE..INTEGER_RANGE)
            .cast_unsigned(),
        Kind::Uint => rng.random_range(0..INTEGER_RANGE.cast_unsigned()),
    }
}

fn floats_close(a: u32, b: u32) -> bool {
    let (x, y) = (f32::from_bits(a), f32::from_bits(b));
    if a == b || (x.is_nan() && y.is_nan()) || (x - y).abs() <= FLOAT_ABSOLUTE {
        return true;
    }
    x.is_sign_negative() == y.is_sign_negative() && (a & !SIGN).abs_diff(b & !SIGN) <= FLOAT_ULPS
}

const SIGN: u32 = 1 << (u32::BITS - 1);

fn kind_of(kind: ScalarKind) -> Option<Kind> {
    match kind {
        ScalarKind::Float => Some(Kind::Float),
        ScalarKind::Sint => Some(Kind::Sint),
        ScalarKind::Uint | ScalarKind::Bool => Some(Kind::Uint),
        ScalarKind::AbstractInt | ScalarKind::AbstractFloat => None,
    }
}

/// The 32-bit scalars of a type in a buffer, as (byte offset, kind), with a runtime-sized array
/// holding RUNTIME_ELEMENTS.
fn scalars(
    module: &Module,
    ty: naga::Handle<naga::Type>,
    at: u32,
    out: &mut Vec<(u32, Kind)>,
) -> Result<(), String> {
    let width = |s: naga::Scalar| {
        if u32::from(s.width) * u8::BITS == u32::BITS {
            kind_of(s.kind).ok_or_else(|| format!("{s:?} in a buffer"))
        } else {
            Err(format!("{}-byte scalars in a buffer", s.width))
        }
    };
    match module.types[ty].inner {
        TypeInner::Scalar(s) | TypeInner::Atomic(s) => out.push((at, width(s)?)),
        TypeInner::Vector { size, scalar } => {
            let kind = width(scalar)?;
            for i in 0..size as u32 {
                out.push((at + i * u32::from(scalar.width), kind));
            }
        }
        TypeInner::Matrix {
            columns,
            rows,
            scalar,
        } => {
            let kind = width(scalar)?;
            let column = naga::proc::Alignment::from(rows) * u32::from(scalar.width);
            for c in 0..columns as u32 {
                for r in 0..rows as u32 {
                    out.push((at + c * column + r * u32::from(scalar.width), kind));
                }
            }
        }
        TypeInner::Array { base, size, stride } => {
            let count = match size {
                ArraySize::Constant(n) => n.get(),
                ArraySize::Dynamic => RUNTIME_ELEMENTS,
                ArraySize::Pending(_) => return Err("an array sized by an override".into()),
            };
            for i in 0..count {
                scalars(module, base, at + i * stride, out)?;
            }
        }
        TypeInner::Struct { ref members, .. } => {
            for m in members {
                scalars(module, m.ty, at + m.offset, out)?;
            }
        }
        ref other => return Err(format!("{other:?} in a buffer")),
    }
    Ok(())
}

/// The bytes a buffer of the type takes (a runtime-sized array holding RUNTIME_ELEMENTS).
fn buffer_size(module: &Module, ty: naga::Handle<naga::Type>) -> u32 {
    let mut layouter = naga::proc::Layouter::default();
    layouter
        .update(module.to_ctx())
        .expect("a validated module lays out");
    let size = layouter[ty].size;
    match &module.types[ty].inner {
        TypeInner::Struct { members, .. } => {
            match members.last().map(|m| &module.types[m.ty].inner) {
                Some(TypeInner::Array {
                    size: ArraySize::Dynamic,
                    stride,
                    ..
                }) => size + stride * RUNTIME_ELEMENTS,
                _ => size,
            }
        }
        TypeInner::Array {
            size: ArraySize::Dynamic,
            stride,
            ..
        } => stride * RUNTIME_ELEMENTS,
        _ => size,
    }
}

fn filled(rng: &mut ChaCha8Rng, size: u32, layout: &[(u32, Kind)]) -> Vec<u8> {
    let mut bytes = vec![0; size as usize];
    for &(at, kind) in layout {
        let at = at as usize;
        bytes[at..at + size_of::<u32>()].copy_from_slice(&fill_scalar(rng, kind).to_le_bytes());
    }
    bytes
}

/// A storage image format: Vulkan's, the bytes of a texel, and how its components are filled and
/// compared (32-bit floats as floats; every narrower format by its bits, which both sides round
/// alike from the same values).
fn storage_format(format: naga::StorageFormat) -> Option<(vk::Format, u32, Option<Kind>)> {
    use naga::StorageFormat as S;
    let one = size_of::<u8>() as u32;
    let two = size_of::<u16>() as u32;
    let four = size_of::<u32>() as u32;
    let (format, bytes, float32) = match format {
        S::R32Float => (vk::Format::R32_SFLOAT, four, true),
        S::R32Uint => (vk::Format::R32_UINT, four, false),
        S::R32Sint => (vk::Format::R32_SINT, four, false),
        S::Rg32Float => (vk::Format::R32G32_SFLOAT, four * 2, true),
        S::Rgba32Float => (
            vk::Format::R32G32B32A32_SFLOAT,
            size_of::<[u32; 4]>() as u32,
            true,
        ),
        S::Rgba32Uint => (
            vk::Format::R32G32B32A32_UINT,
            size_of::<[u32; 4]>() as u32,
            false,
        ),
        S::Rgba32Sint => (
            vk::Format::R32G32B32A32_SINT,
            size_of::<[u32; 4]>() as u32,
            false,
        ),
        S::Rgba8Unorm => (vk::Format::R8G8B8A8_UNORM, four, false),
        S::Rgba8Snorm => (vk::Format::R8G8B8A8_SNORM, four, false),
        S::Rgba8Uint => (vk::Format::R8G8B8A8_UINT, four, false),
        S::Rgba8Sint => (vk::Format::R8G8B8A8_SINT, four, false),
        S::R8Unorm => (vk::Format::R8_UNORM, one, false),
        S::Rg8Unorm => (vk::Format::R8G8_UNORM, two, false),
        S::Rgb10a2Unorm => (vk::Format::A2B10G10R10_UNORM_PACK32, four, false),
        S::Rgb10a2Uint => (vk::Format::A2B10G10R10_UINT_PACK32, four, false),
        S::Rg11b10Ufloat => (vk::Format::B10G11R11_UFLOAT_PACK32, four, false),
        S::R8Snorm => (vk::Format::R8_SNORM, one, false),
        S::R8Uint => (vk::Format::R8_UINT, one, false),
        S::R8Sint => (vk::Format::R8_SINT, one, false),
        S::Rg8Snorm => (vk::Format::R8G8_SNORM, two, false),
        S::Rg8Uint => (vk::Format::R8G8_UINT, two, false),
        S::Rg8Sint => (vk::Format::R8G8_SINT, two, false),
        S::R16Float => (vk::Format::R16_SFLOAT, two, false),
        S::R16Uint => (vk::Format::R16_UINT, two, false),
        S::R16Sint => (vk::Format::R16_SINT, two, false),
        S::R16Unorm => (vk::Format::R16_UNORM, two, false),
        S::R16Snorm => (vk::Format::R16_SNORM, two, false),
        S::Rg16Float => (vk::Format::R16G16_SFLOAT, four, false),
        S::Rg16Uint => (vk::Format::R16G16_UINT, four, false),
        S::Rg16Sint => (vk::Format::R16G16_SINT, four, false),
        S::Rg16Unorm => (vk::Format::R16G16_UNORM, four, false),
        S::Rg16Snorm => (vk::Format::R16G16_SNORM, four, false),
        S::Rg32Uint => (vk::Format::R32G32_UINT, four * 2, false),
        S::Rg32Sint => (vk::Format::R32G32_SINT, four * 2, false),
        S::Rgba16Float => (vk::Format::R16G16B16A16_SFLOAT, four * 2, false),
        S::Rgba16Uint => (vk::Format::R16G16B16A16_UINT, four * 2, false),
        S::Rgba16Sint => (vk::Format::R16G16B16A16_SINT, four * 2, false),
        S::Rgba16Unorm => (vk::Format::R16G16B16A16_UNORM, four * 2, false),
        S::Rgba16Snorm => (vk::Format::R16G16B16A16_SNORM, four * 2, false),
        _ => return None,
    };
    Some((format, bytes, float32.then_some(Kind::Float)))
}

/// One resource of the route's module, with where each run binds it.
pub struct Resource {
    pub reference: (u32, u32),
    /// The program's element this resource is first, where the route split an array of resources
    /// into a binding an element.
    pub first_element: u32,
    route: (u32, u32),
    pub kind: vk::DescriptorType,
    slot: SlotKind,
    pub elements: Vec<Element>,
    /// A storage buffer's scalars, to compare its contents.
    layout: Vec<(u32, Kind)>,
    /// A storage image's components.
    pub image_kind: Option<Kind>,
}

fn image(
    rng: &mut ChaCha8Rng,
    dim: ImageDimension,
    arrayed: bool,
    format: vk::Format,
    texel: u32,
    kind: Option<Kind>,
    depth: bool,
) -> Image {
    let (view, extent, mut layers) = match dim {
        ImageDimension::D1 => (vk::ImageViewType::TYPE_1D, [TEXTURE_SIZE, 1, 1], 1),
        ImageDimension::D2 => (
            vk::ImageViewType::TYPE_2D,
            [TEXTURE_SIZE, TEXTURE_SIZE, 1],
            1,
        ),
        ImageDimension::D3 => (
            vk::ImageViewType::TYPE_3D,
            [TEXTURE_SIZE, TEXTURE_SIZE, TEXTURE_SIZE],
            1,
        ),
        ImageDimension::Cube => (
            vk::ImageViewType::CUBE,
            [TEXTURE_SIZE, TEXTURE_SIZE, 1],
            CUBE_FACES,
        ),
    };
    let view = match (view, arrayed) {
        (v, false) => v,
        (vk::ImageViewType::TYPE_1D, true) => vk::ImageViewType::TYPE_1D_ARRAY,
        (vk::ImageViewType::CUBE, true) => vk::ImageViewType::CUBE_ARRAY,
        (_, true) => vk::ImageViewType::TYPE_2D_ARRAY,
    };
    if arrayed {
        layers *= TEXTURE_LAYERS;
    }
    let [width, height, d] = extent;
    let texels = width * height * d * layers;
    let bytes: Vec<u8> = match (kind, depth) {
        (_, true) => (0..texels)
            .flat_map(|_| rng.random_range(0.0..1.0_f32).to_le_bytes())
            .collect(),
        (Some(kind), false) => (0..texels * texel / size_of::<u32>() as u32)
            .flat_map(|_| fill_scalar(rng, kind).to_le_bytes())
            .collect(),
        (None, false) => (0..texels * texel).map(|_| rng.random()).collect(),
    };
    Image {
        view,
        format,
        extent: vk::Extent3D {
            width,
            height,
            depth: d,
        },
        layers,
        depth,
        samples: vk::SampleCountFlags::TYPE_1,
        bytes,
    }
}

/// `single` made multisampled, every sample of every texel seeded on its own.
fn multisampled(rng: &mut ChaCha8Rng, single: Image, kind: Kind) -> Image {
    let words = single.bytes.len() * MULTISAMPLE_SAMPLES.as_raw() as usize / size_of::<u32>();
    Image {
        samples: MULTISAMPLE_SAMPLES,
        bytes: (0..words)
            .flat_map(|_| fill_scalar(rng, kind).to_le_bytes())
            .collect(),
        ..single
    }
}

/// Every resource of the route's module, filled from `rng`.
pub fn resources(
    module: &Module,
    reflection: &[ReflectedBinding],
    rng: &mut ChaCha8Rng,
) -> Result<(Vec<Resource>, Vec<u8>), String> {
    let mut out = Vec::new();
    let mut immediates = Vec::new();
    for (_, global) in module.global_variables.iter() {
        if global.space == AddressSpace::Immediate {
            let mut layout = Vec::new();
            scalars(module, global.ty, 0, &mut layout)?;
            immediates = filled(rng, buffer_size(module, global.ty), &layout);
            continue;
        }
        let Some(binding) = &global.binding else {
            continue;
        };
        // An array's elements split into bindings of their own lie in its reflected range.
        let reflected = reflection
            .iter()
            .find(|r| {
                r.set == binding.group
                    && (r.binding..r.binding + r.count.max(1)).contains(&binding.binding)
            })
            .ok_or_else(|| format!("no reflection of {binding:?}"))?;
        let (ty, count) = match module.types[global.ty].inner {
            TypeInner::BindingArray { base, .. } => (base, reflected.count),
            _ => (global.ty, 1),
        };
        let mut resource = Resource {
            reference: (reflected.space, reflected.register),
            first_element: binding.binding - reflected.binding,
            route: (binding.group, binding.binding),
            kind: vk::DescriptorType::SAMPLER,
            slot: reflected.kind,
            elements: Vec::new(),
            layout: Vec::new(),
            image_kind: None,
        };
        for _ in 0..count {
            let element = match (&module.types[ty].inner, global.space) {
                (_, AddressSpace::Uniform | AddressSpace::Storage { .. }) => {
                    resource.kind = if global.space == AddressSpace::Uniform {
                        vk::DescriptorType::UNIFORM_BUFFER
                    } else {
                        vk::DescriptorType::STORAGE_BUFFER
                    };
                    resource.layout.clear();
                    scalars(module, ty, 0, &mut resource.layout)?;
                    Element::Buffer(filled(rng, buffer_size(module, ty), &resource.layout))
                }
                (&TypeInner::Sampler { comparison }, _) => Element::Sampler { comparison },
                (
                    &TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    },
                    _,
                ) => match class {
                    ImageClass::Depth { multi: true } => {
                        return Err("multisampled depth textures are not filled here".into());
                    }
                    ImageClass::Sampled { kind, multi } => {
                        resource.kind = vk::DescriptorType::SAMPLED_IMAGE;
                        let kind = kind_of(kind).ok_or("an abstract texture")?;
                        let format = match kind {
                            Kind::Float => vk::Format::R32G32B32A32_SFLOAT,
                            Kind::Sint => vk::Format::R32G32B32A32_SINT,
                            Kind::Uint => vk::Format::R32G32B32A32_UINT,
                        };
                        let texel = lavapipe::texel_bytes(format);
                        let single = image(rng, dim, arrayed, format, texel, Some(kind), false);
                        Element::Image(if multi {
                            multisampled(rng, single, kind)
                        } else {
                            single
                        })
                    }
                    ImageClass::Depth { .. } => {
                        resource.kind = vk::DescriptorType::SAMPLED_IMAGE;
                        let format = vk::Format::D32_SFLOAT;
                        let texel = lavapipe::texel_bytes(format);
                        Element::Image(image(rng, dim, arrayed, format, texel, None, true))
                    }
                    ImageClass::Storage { format, .. } => {
                        resource.kind = vk::DescriptorType::STORAGE_IMAGE;
                        let (vk_format, texel, kind) = storage_format(format).ok_or_else(|| {
                            format!("storage format {format:?} is not set up here")
                        })?;
                        resource.image_kind = kind;
                        Element::Image(image(rng, dim, arrayed, vk_format, texel, kind, false))
                    }
                    ImageClass::External => return Err("external textures".into()),
                },
                (other, _) => return Err(format!("a resource of type {other:?}")),
            };
            resource.elements.push(element);
        }
        out.push(resource);
    }
    Ok((out, immediates))
}

/// The descriptors of each run: the route's as its module binds them; the reference's at the
/// program's own (set, binding), a texture and sampler split from one combined image sampler
/// joined again.
pub fn descriptors(resources: &[Resource]) -> (Vec<Descriptor>, Vec<Descriptor>) {
    let route = resources
        .iter()
        .map(|r| Descriptor {
            set: r.route.0,
            binding: r.route.1,
            kind: r.kind,
            elements: r.elements.clone(),
        })
        .collect();
    let mut reference: Vec<Descriptor> = Vec::new();
    let mut in_order: Vec<&Resource> = resources.iter().collect();
    in_order.sort_by_key(|r| r.first_element);
    for r in in_order {
        let (set, binding) = r.reference;
        let pair = resources.iter().find(|o| {
            o.reference == r.reference
                && o.first_element == r.first_element
                && (o.slot == SlotKind::Sampler) != (r.slot == SlotKind::Sampler)
        });
        let (kind, elements) = match (r.slot, pair) {
            (SlotKind::Sampler, Some(_)) => continue,
            (SlotKind::Texture, Some(sampler)) => (
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                r.elements
                    .iter()
                    .zip(&sampler.elements)
                    .map(|(t, s)| match (t, s) {
                        (Element::Image(image), Element::Sampler { comparison }) => {
                            Element::Combined(image.clone(), *comparison)
                        }
                        _ => t.clone(),
                    })
                    .collect(),
            ),
            _ => (r.kind, r.elements.clone()),
        };
        // The elements of a split array join the program's array again, in order.
        match reference
            .iter_mut()
            .find(|d| d.set == set && d.binding == binding && d.kind == kind)
        {
            Some(array) => array.elements.extend(elements),
            None => reference.push(Descriptor {
                set,
                binding,
                kind,
                elements,
            }),
        }
    }
    (reference, route)
}

pub fn attribute_format(ty: Ty) -> vk::Format {
    const FLOAT: [vk::Format; 4] = [
        vk::Format::R32_SFLOAT,
        vk::Format::R32G32_SFLOAT,
        vk::Format::R32G32B32_SFLOAT,
        vk::Format::R32G32B32A32_SFLOAT,
    ];
    const SINT: [vk::Format; 4] = [
        vk::Format::R32_SINT,
        vk::Format::R32G32_SINT,
        vk::Format::R32G32B32_SINT,
        vk::Format::R32G32B32A32_SINT,
    ];
    const UINT: [vk::Format; 4] = [
        vk::Format::R32_UINT,
        vk::Format::R32G32_UINT,
        vk::Format::R32G32B32_UINT,
        vk::Format::R32G32B32A32_UINT,
    ];
    let table = match ty.kind {
        Kind::Float => FLOAT,
        Kind::Sint => SINT,
        Kind::Uint => UINT,
    };
    table[ty.components as usize - 1]
}

/// One attribute a location, every input set's values one after another.
fn attributes(inputs: &[(u32, Ty)], rng: &mut ChaCha8Rng, sets: u32) -> Vec<Attribute> {
    let mut out = Vec::new();
    for &(location, ty) in inputs {
        for element in 0..ty.count {
            let one = Ty { count: 1, ..ty };
            out.push(Attribute {
                location: location + element,
                format: attribute_format(one),
                stride: one.bytes(),
                bytes: (0..sets * ty.components)
                    .flat_map(|_| fill_scalar(rng, ty.kind).to_le_bytes())
                    .collect(),
            });
        }
    }
    out
}

fn located(vars: &[interface::Var]) -> Vec<(u32, Ty)> {
    vars.iter()
        .filter_map(|v| match v.slot {
            Slot::Location(l) => Some((l, v.ty)),
            Slot::BuiltIn(_) => None,
        })
        .collect()
}

/// The vertex program that feeds a fragment program: a point at the centre of each pixel of a row,
/// carrying its input set's values to the fragment program's inputs.
fn feeder(inputs: &[(u32, Ty)]) -> Result<Vec<u32>, String> {
    let mut m = Module::default();
    let ty = |m: &mut Module, inner: TypeInner| {
        m.types
            .insert(naga::Type { name: None, inner }, Span::UNDEFINED)
    };
    let scalar = |k: Kind| match k {
        Kind::Float => naga::Scalar::F32,
        Kind::Sint => naga::Scalar::I32,
        Kind::Uint => naga::Scalar::U32,
    };
    let vec4 = ty(
        &mut m,
        TypeInner::Vector {
            size: naga::VectorSize::Quad,
            scalar: naga::Scalar::F32,
        },
    );
    let mut function = naga::Function::default();
    let mut members = Vec::new();
    let mut parts = Vec::new();
    let at = |n| u32::try_from(n).expect("a few arguments");
    function.arguments.push(naga::FunctionArgument {
        name: None,
        ty: vec4,
        binding: Some(Binding::Location {
            location: 0,
            interpolation: None,
            sampling: None,
            blend_src: None,
            per_primitive: false,
        }),
    });
    members.push((
        vec4,
        Binding::BuiltIn(naga::BuiltIn::Position { invariant: false }),
    ));
    parts.push(
        function
            .expressions
            .append(naga::Expression::FunctionArgument(0), Span::UNDEFINED),
    );
    for &(location, t) in inputs {
        let size = match t.components {
            1 => None,
            n => Some(
                [
                    naga::VectorSize::Bi,
                    naga::VectorSize::Tri,
                    naga::VectorSize::Quad,
                ]
                .into_iter()
                .find(|s| *s as u32 == n)
                .ok_or_else(|| format!("a vector of {n}"))?,
            ),
        };
        let inner = match size {
            None => TypeInner::Scalar(scalar(t.kind)),
            Some(size) => TypeInner::Vector {
                size,
                scalar: scalar(t.kind),
            },
        };
        let handle = ty(&mut m, inner);
        let interpolation = match t.kind {
            Kind::Float => Interpolation::Perspective,
            _ => Interpolation::Flat,
        };
        for element in 0..t.count {
            let index = at(function.arguments.len());
            function.arguments.push(naga::FunctionArgument {
                name: None,
                ty: handle,
                binding: Some(Binding::Location {
                    location: index,
                    interpolation: None,
                    sampling: None,
                    blend_src: None,
                    per_primitive: false,
                }),
            });
            parts.push(
                function
                    .expressions
                    .append(naga::Expression::FunctionArgument(index), Span::UNDEFINED),
            );
            members.push((
                handle,
                Binding::Location {
                    location: location + element,
                    interpolation: Some(interpolation),
                    sampling: (interpolation == Interpolation::Perspective)
                        .then_some(naga::Sampling::Center),
                    blend_src: None,
                    per_primitive: false,
                },
            ));
        }
    }
    let mut layouter = naga::proc::Layouter::default();
    let mut struct_members = Vec::new();
    let mut offset = 0;
    for (ty, binding) in members {
        layouter.update(m.to_ctx()).map_err(|e| e.to_string())?;
        let layout = layouter[ty];
        offset = layout.alignment.round_up(offset);
        struct_members.push(naga::StructMember {
            name: None,
            ty,
            binding: Some(binding),
            offset,
        });
        offset += layout.size;
    }
    let result = ty(
        &mut m,
        TypeInner::Struct {
            members: struct_members,
            span: naga::proc::Alignment::SIXTEEN.round_up(offset),
        },
    );
    let start = function.expressions.len();
    let value = function.expressions.append(
        naga::Expression::Compose {
            ty: result,
            components: parts,
        },
        Span::UNDEFINED,
    );
    function.body.push(
        naga::Statement::Emit(function.expressions.range_from(start)),
        Span::UNDEFINED,
    );
    function.body.push(
        naga::Statement::Return { value: Some(value) },
        Span::UNDEFINED,
    );
    function.result = Some(naga::FunctionResult {
        ty: result,
        binding: None,
    });
    m.entry_points.push(naga::EntryPoint {
        name: FEEDER_ENTRY.into(),
        stage: ShaderStage::Vertex,
        early_depth_test: None,
        workgroup_size: [0; 3],
        workgroup_size_overrides: None,
        function,
        mesh_info: None,
        task_payload: None,
        incoming_ray_payload: None,
    });
    write_spirv(
        &m,
        ShaderStage::Vertex,
        FEEDER_ENTRY,
        naga::back::spv::WriterFlags::FORCE_POINT_SIZE,
    )
}

pub fn write_spirv(
    module: &Module,
    stage: ShaderStage,
    entry: &str,
    flags: naga::back::spv::WriterFlags,
) -> Result<Vec<u32>, String> {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(module)
    .map_err(|e| {
        format!(
            "naga validation for the SPIR-V writer: {}",
            wcontainers_nir::spirv::chain(e.as_inner())
        )
    })?;
    let options = naga::back::spv::Options {
        lang_version: SPIRV_VERSION,
        flags,
        ..Default::default()
    };
    naga::back::spv::write_vec(
        module,
        &info,
        &options,
        Some(&naga::back::spv::PipelineOptions {
            shader_stage: stage,
            entry_point: entry.into(),
        }),
    )
    .map_err(|e| format!("naga's SPIR-V writer: {e}"))
}

/// The positions of the feeder's points: the centre of each pixel of a row `width` wide, in
/// Vulkan's clip space.
pub fn positions(width: u32) -> Vec<u8> {
    (0..width)
        .flat_map(|x| {
            let centre = (x as f32 + PIXEL_CENTRE) / width as f32 * 2.0 - 1.0;
            [centre, 0.0, POINT_DEPTH, 1.0]
        })
        .flat_map(f32::to_le_bytes)
        .collect()
}

pub fn colour_format(kind: Kind) -> vk::Format {
    match kind {
        Kind::Float => vk::Format::R32G32B32A32_SFLOAT,
        Kind::Sint => vk::Format::R32G32B32A32_SINT,
        Kind::Uint => vk::Format::R32G32B32A32_UINT,
    }
}

/// The differences between two runs' words, `kinds` saying how each word compares.
pub fn differences(
    what: &str,
    a: &[u8],
    b: &[u8],
    kinds: impl Fn(usize) -> Option<Kind>,
    out: &mut Vec<String>,
) {
    let words = |x: &[u8]| -> Vec<u32> {
        x.as_chunks::<{ size_of::<u32>() }>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect()
    };
    let (a, b) = (words(a), words(b));
    if a.len() != b.len() {
        out.push(format!("{what}: {} words against {}", a.len(), b.len()));
        return;
    }
    for (i, (&x, &y)) in a.iter().zip(&b).enumerate() {
        let same = match kinds(i) {
            None => true,
            Some(Kind::Float) => floats_close(x, y),
            Some(_) => x == y,
        };
        if !same {
            out.push(format!(
                "{what} word {i}: {x:#x} ({}) against {y:#x} ({})",
                f32::from_bits(x),
                f32::from_bits(y)
            ));
        }
    }
}

/// The kind of each word of a capture record laid out as `vars` are, by their slot.
fn record(vars: &[&interface::Var]) -> Vec<(Slot, u32, Ty)> {
    let mut at = 0;
    vars.iter()
        .map(|v| {
            let r = (v.slot, at, v.ty);
            at += v.ty.bytes();
            r
        })
        .collect()
}

/// Holds the route's module to the program it came from, on lavapipe.
#[allow(clippy::too_many_lines, reason = "one comparison, stage by stage")]
/// The line a child writes to its errors once the reference has run, so that a child stopped after
/// it is the route's run that did not end, and one stopped before it the reference's.
pub const REFERENCE_ENDED: &str = "corpus: the reference ended";

pub fn reference_ended() {
    eprintln!("{REFERENCE_ENDED}");
}

pub fn compare(
    original: &[u32],
    entry: &str,
    stage: lavapipe::Stage,
    module: &Module,
    reflection: &[ReflectedBinding],
    options: CompareOptions,
) -> Verdict {
    let not_run = Verdict::NotRun;
    let mut rng = ChaCha8Rng::seed_from_u64(options.seed);
    let route_entry = &module.entry_points[0];
    let naga_stage = route_entry.stage;
    let route = match write_spirv(
        module,
        naga_stage,
        &route_entry.name,
        naga::back::spv::WriterFlags::ADJUST_COORDINATE_SPACE,
    ) {
        Ok(w) => w,
        Err(e) => return not_run(e),
    };
    let (reference_io, route_io) =
        match (interface::interface(original), interface::interface(&route)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => return not_run(format!("interface: {e}")),
        };
    let (resources, immediates) = match resources(module, reflection, &mut rng) {
        Ok(r) => r,
        Err(e) => return not_run(e),
    };
    let (reference_descriptors, route_descriptors) = descriptors(&resources);
    let sets = options.input_sets;
    let inputs = located(&reference_io.inputs);
    let base = Job {
        stage,
        words: original,
        entry,
        descriptors: &reference_descriptors,
        immediates: &immediates,
        groups: [COMPUTE_GROUPS, 1, 1],
        vertices: sets,
        attributes: &[],
        feeder: None,
        capture_stride: 0,
        targets: &[],
        depth: false,
        clear: [CLEAR_BITS; 4],
    };
    let lavapipe = match Lavapipe::open() {
        Ok(l) => l,
        Err(e) => return not_run(e),
    };
    // Each run: the reference twice (a reference that differs from itself compares nothing), then
    // the route.
    let run_both = |reference: &Job, route: &Job| -> Result<(Outputs, Outputs), Verdict> {
        let first = lavapipe.run(reference).map_err(Verdict::NotRun)?;
        let again = lavapipe.run(reference).map_err(Verdict::NotRun)?;
        if first != again {
            return Err(Verdict::NotRun(
                "the reference's outputs differ between two runs".into(),
            ));
        }
        reference_ended();
        let route = lavapipe
            .run(route)
            .map_err(|e| Verdict::Differs(format!("the route's module did not run: {e}")))?;
        Ok((first, route))
    };
    let mut found = Vec::new();
    let (a, b) = match stage {
        lavapipe::Stage::Compute => {
            let reference = base;
            let route_job = Job {
                words: &route,
                entry: &route_entry.name,
                descriptors: &route_descriptors,
                ..reference
            };
            match run_both(&reference, &route_job) {
                Ok(x) => x,
                Err(v) => return v,
            }
        }
        lavapipe::Stage::Vertex => {
            let attributes = attributes(&inputs, &mut rng, sets);
            let reference_vars = interface::captured(&reference_io.outputs);
            let route_vars = interface::captured(&route_io.outputs);
            if let Some(v) = reference_vars.iter().find(|v| v.ty.components == 0) {
                return not_run(format!(
                    "an output of a type not captured here, {:?}",
                    v.slot
                ));
            }
            let (reference_words, reference_stride) =
                interface::with_capture(original, &reference_vars);
            let (route_words, route_stride) = interface::with_capture(&route, &route_vars);
            let reference = Job {
                words: &reference_words,
                attributes: &attributes,
                capture_stride: reference_stride,
                ..base
            };
            let route_job = Job {
                words: &route_words,
                entry: &route_entry.name,
                descriptors: &route_descriptors,
                capture_stride: route_stride,
                ..reference
            };
            let (a, b) = match run_both(&reference, &route_job) {
                Ok(x) => x,
                Err(v) => return v,
            };
            let (ra, rb) = (record(&reference_vars), record(&route_vars));
            for (&(slot, at, ty), var) in ra.iter().zip(&reference_vars) {
                let Some(&(_, bt, _)) = rb.iter().find(|r| r.0 == slot) else {
                    if interface::reaches(original, var) {
                        found.push(format!(
                            "{slot:?}: an output the program writes, missing from the route's module"
                        ));
                    }
                    continue;
                };
                for vertex in 0..sets {
                    let take = |x: &[u8], stride: u32, at: u32| {
                        let start = (vertex * stride + at) as usize;
                        x[start..start + ty.bytes() as usize].to_vec()
                    };
                    differences(
                        &format!("vertex {vertex} {slot:?}"),
                        &take(&a.captured, reference_stride, at),
                        &take(&b.captured, route_stride, bt),
                        |_| Some(ty.kind),
                        &mut found,
                    );
                }
            }
            (a, b)
        }
        lavapipe::Stage::Fragment => {
            let width = sets;
            let feeder_words = match feeder(&inputs) {
                Ok(w) => w,
                Err(e) => return not_run(e),
            };
            let mut attributes = vec![Attribute {
                location: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                stride: size_of::<[f32; 4]>() as u32,
                bytes: positions(width),
            }];
            let feeding = self::attributes(&inputs, &mut rng, sets);
            for (i, mut a) in (1..).zip(feeding) {
                a.location = i;
                attributes.push(a);
            }
            let outputs = located(&reference_io.outputs);
            let locations = outputs.iter().map(|(l, t)| l + t.count).max().unwrap_or(0);
            let mut targets = vec![None; locations as usize];
            let mut target_kinds = Vec::new();
            for &(location, ty) in &outputs {
                for element in 0..ty.count {
                    targets[(location + element) as usize] = Some(colour_format(ty.kind));
                }
                target_kinds.push((location, ty));
            }
            let depth = reference_io
                .outputs
                .iter()
                .any(|v| v.slot == Slot::BuiltIn(spirv::BuiltIn::FragDepth as u32));
            let reference = Job {
                attributes: &attributes,
                feeder: Some((&feeder_words, FEEDER_ENTRY)),
                targets: &targets,
                depth,
                ..base
            };
            let route_job = Job {
                words: &route,
                entry: &route_entry.name,
                descriptors: &route_descriptors,
                ..reference
            };
            let (a, b) = match run_both(&reference, &route_job) {
                Ok(x) => x,
                Err(v) => return v,
            };
            let mut target = 0;
            for (location, slot) in targets.iter().enumerate() {
                if slot.is_none() {
                    continue;
                }
                let (_, ty) = target_kinds
                    .iter()
                    .find(|(l, t)| (*l..l + t.count).contains(&(location as u32)))
                    .expect("a target has an output");
                let components = ty.components as usize;
                let per_texel = lavapipe::texel_bytes(vk::Format::R32G32B32A32_SFLOAT) as usize
                    / size_of::<u32>();
                differences(
                    &format!("target {location}"),
                    &a.targets[target],
                    &b.targets[target],
                    |i| (i % per_texel < components).then_some(ty.kind),
                    &mut found,
                );
                target += 1;
            }
            differences(
                "depth",
                &a.depth,
                &b.depth,
                |_| Some(Kind::Float),
                &mut found,
            );
            (a, b)
        }
    };
    let mut storage = resources
        .iter()
        .filter(|r| {
            matches!(
                r.kind,
                vk::DescriptorType::STORAGE_BUFFER | vk::DescriptorType::STORAGE_IMAGE
            )
        })
        .flat_map(|r| r.elements.iter().map(move |_| r));
    for (i, (x, y)) in a.storage.iter().zip(&b.storage).enumerate() {
        let r = storage.next().expect("a storage resource an element");
        let words: Vec<Option<Kind>> = if r.kind == vk::DescriptorType::STORAGE_IMAGE {
            vec![r.image_kind.or(Some(Kind::Uint)); x.len() / size_of::<u32>()]
        } else {
            let mut w = vec![None; x.len() / size_of::<u32>()];
            for &(at, kind) in &r.layout {
                w[at as usize / size_of::<u32>()] = Some(kind);
            }
            w
        };
        differences(
            &format!("storage {i} at {:?}", r.reference),
            x,
            y,
            |at| words.get(at).copied().flatten(),
            &mut found,
        );
    }
    if found.is_empty() {
        Verdict::Matched
    } else {
        let count = found.len();
        found.truncate(NAMED_DIFFERENCES);
        Verdict::Differs(format!("{count} values differ: {}", found.join("; ")))
    }
}
