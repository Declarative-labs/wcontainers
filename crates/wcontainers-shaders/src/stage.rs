//! The stage helper of plan section 7: the vertex and geometry stages run as compute work before the
//! draw, on back ends with compute shaders, and a pass-through vertex program draws what they made.
//!
//! The data between them, in buffers the helper owns:
//! - Vertex records: the vertex stage's outputs, one record per API vertex (in draw order) and API
//!   instance, record `instance * count + vertex`. A record is four-word slots: the position, then
//!   one per user location the geometry stage reads, in location order.
//! - Geometry output: records of the same shape for the vertices each geometry invocation emits,
//!   invocation i owning `max_vertices` records from `i * max_vertices`; then, from word
//!   `INDEX_BASE`, `2 * max_vertices` index slots per invocation: the emitted vertices' numbers in
//!   order, each cut strip ended with the restart index, unused slots restarting.
//!
//! Its own bindings live in set [`STAGE_SET`] of its compute programs, which no compute program of the
//! API uses (set 1 is the pixel stage's).

use crate::ConvertError;
use crate::build::{self, Builder, Expr};
use alloc::format;
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, FunctionResult, Handle,
    Module, ResourceBinding, Scalar, ScalarKind, Span, StorageAccess, StructMember, Type,
    TypeInner,
};
use wcontainers_gpu::{Interpolation, Reflection, ScalarKind as ValueKind, SystemValue};

mod helper;
pub use helper::{
    FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages, StreamOutput,
    StreamOutputEntry, StreamTargets,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// The set of the stage helper's own bindings in its compute programs.
pub(crate) const STAGE_SET: u32 = 1;
/// Its draw's values (a uniform block of u32 words, [`word`]).
pub(crate) const VALUES_BINDING: u32 = 0;
/// The draw's index buffer, read as u32 words.
pub(crate) const INDICES_BINDING: u32 = 1;
/// The vertex records.
pub(crate) const VERTICES_BINDING: u32 = 2;
/// The geometry output.
pub(crate) const GEOMETRY_BINDING: u32 = 3;
/// Invocations per workgroup of the helper's compute programs.
pub(crate) const WORKGROUP: u32 = 64;

/// The words of the values block.
pub(crate) mod word {
    /// Vertices per API instance: the API draw's vertex or index count.
    pub const COUNT: u32 = 0;
    /// The API draw's first vertex, or first index.
    pub const FIRST: u32 = 1;
    /// The API draw's base vertex (i32 bits), for indexed draws.
    pub const BASE_VERTEX: u32 = 2;
    /// Bytes per index, 2 or 4; 0 for a draw without indices.
    pub const INDEX_BYTES: u32 = 3;
    /// Bytes from the bound index words' start to the API's index buffer offset.
    pub const INDEX_OFFSET: u32 = 4;
    pub const FIRST_INSTANCE: u32 = 5;
    pub const INSTANCES: u32 = 6;
    /// 1 when the API's topology is a strip.
    pub const STRIP: u32 = 7;
    /// Input primitives per API instance.
    pub const PRIMITIVES: u32 = 8;
    /// The word of the geometry output where index slots start.
    pub const INDEX_BASE: u32 = 9;
    pub const COUNT_OF: u32 = 12;
}

pub(crate) fn binding(binding: u32) -> ResourceBinding {
    ResourceBinding {
        group: STAGE_SET,
        binding,
    }
}

/// Words per record of a stage holding `locations` user locations.
pub(crate) fn record_words(locations: usize) -> u32 {
    (1 + locations as u32) * 4
}

/// The values block's global.
pub(crate) fn values(module: &mut Module) -> Handle<naga::GlobalVariable> {
    let u32_ty = build::scalar(module, Scalar::U32);
    let members = (0..word::COUNT_OF)
        .map(|i| StructMember {
            name: Some(format!("w{i}")),
            ty: u32_ty,
            binding: None,
            offset: i * 4,
        })
        .collect();
    let ty = module.types.insert(
        Type {
            name: Some("StageValues".into()),
            inner: TypeInner::Struct {
                members,
                span: word::COUNT_OF * 4,
            },
        },
        Span::UNDEFINED,
    );
    build::global(
        module,
        "stage_values",
        AddressSpace::Uniform,
        Some(binding(VALUES_BINDING)),
        ty,
    )
}

/// Word `w` of the values block, given a pointer to it.
pub(crate) fn value(b: &mut Builder, values: Expr, w: u32) -> Expr {
    let p = b.field(values, w);
    b.load(p)
}

/// The user locations of a stage's values (inputs or outputs) that are not system values, sorted:
/// the order of a record's slots after the position.
pub fn locations(values: &[wcontainers_gpu::StageValue]) -> Vec<u32> {
    let mut locations: Vec<u32> = values
        .iter()
        .filter(|v| v.system.is_none())
        .map(|v| v.location)
        .collect();
    locations.sort_unstable();
    locations.dedup();
    locations
}

/// The compute form of `vertex` (converted with every input pulled) that writes the vertex records
/// `geometry` (converted) reads, which [`Stages`] runs.
pub fn vertex_program(
    vertex: &crate::Converted,
    geometry: &crate::Converted,
) -> Result<wcontainers_gpu::ShaderModule> {
    vertex_program_for(vertex, &locations(&geometry.reflection.inputs))
}

/// The compute form of `vertex` writing records with a slot for each of `inputs` (locations).
pub(crate) fn vertex_program_for(
    vertex: &crate::Converted,
    inputs: &[u32],
) -> Result<wcontainers_gpu::ShaderModule> {
    let source = &vertex.modules[0];
    let module = vertex_as_compute(&source.module, inputs)?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key).update(b"vertex records");
    for l in inputs {
        key.update(&l.to_le_bytes());
    }
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: "main".into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *key.finalize().as_bytes(),
    })
}

/// The stream output program (`stream_out.wgsl`), which [`Stages`] runs after the last stage.
pub fn stream_program() -> Result<wcontainers_gpu::ShaderModule> {
    let text = include_str!("stage/stream_out.wgsl");
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| ConvertError::Malformed(format!("the stream output program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: "main".into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// A converted vertex program as a compute program writing vertex records with a slot for each of
/// `locations` (the geometry stage's input locations). Its inputs must all be pulled
/// (`VariantKey::pulled`), so it reads only the vertex and instance numbers, which it makes as a draw
/// would: the first vertex plus the invocation's place, or the index there plus the base vertex.
pub(crate) fn vertex_as_compute(vertex: &Module, locations: &[u32]) -> Result<Module> {
    let mut module = vertex.clone();
    if module.entry_points.len() != 1 || module.entry_points[0].stage != naga::ShaderStage::Vertex {
        return Err(ConvertError::Malformed("not a vertex program".into()));
    }
    let entry = module.entry_points.remove(0);
    let mut inner = entry.function;
    let arguments = core::mem::take(&mut inner.arguments);
    let mut numbers = Vec::new();
    for a in &arguments {
        match a.binding {
            Some(Binding::BuiltIn(BuiltIn::VertexIndex)) => numbers.push(0),
            Some(Binding::BuiltIn(BuiltIn::InstanceIndex)) => numbers.push(1),
            Some(Binding::Location { .. }) => {
                return Err(ConvertError::Unsupported("VertexInputNotPulled"));
            }
            _ => return Err(ConvertError::Unsupported("VertexInputForGeometry")),
        }
    }
    inner.arguments = arguments
        .iter()
        .map(|a| FunctionArgument {
            binding: None,
            ..a.clone()
        })
        .collect();
    let result = inner.result.take().ok_or(ConvertError::Malformed(
        "a vertex program without outputs".into(),
    ))?;
    inner.result = Some(FunctionResult {
        ty: result.ty,
        binding: None,
    });
    // Where each output is: the position and the locations, as members of the result or the result.
    let mut position = None;
    let mut slots: Vec<Option<(u32, ScalarKind)>> = alloc::vec![None; locations.len()];
    let mut place = |binding: &Binding, member: Option<u32>, inner: &TypeInner| -> Result<()> {
        let kind = match inner {
            TypeInner::Vector { scalar, size } if *size as u32 == 4 => scalar.kind,
            _ => return Err(ConvertError::Unsupported("VertexOutputShape")),
        };
        match binding {
            Binding::BuiltIn(BuiltIn::Position { .. }) => position = Some(member),
            Binding::Location { location, .. } => {
                if let Some(i) = locations.iter().position(|l| l == location) {
                    slots[i] = Some((member.unwrap_or(0), kind));
                }
            }
            _ => return Err(ConvertError::Unsupported("VertexOutputForGeometry")),
        }
        Ok(())
    };
    match (&result.binding, &module.types[result.ty].inner) {
        (Some(binding), inner) => place(binding, None, inner)?,
        (None, TypeInner::Struct { members, .. }) => {
            for (i, m) in members.iter().enumerate() {
                let binding = m
                    .binding
                    .as_ref()
                    .ok_or(ConvertError::Malformed("an unbound output".into()))?;
                place(binding, Some(i as u32), &module.types[m.ty].inner)?;
            }
        }
        _ => return Err(ConvertError::Malformed("vertex outputs".into())),
    }
    let position = position.ok_or(ConvertError::Unsupported("VertexWithoutPosition"))?;
    inner.name = Some("vertex_shader".into());
    let callee = module.functions.append(inner, Span::UNDEFINED);

    let values = values(&mut module);
    let u32_ty = build::scalar(&mut module, Scalar::U32);
    let words = build::runtime_array(&mut module, u32_ty, 4);
    let indices = build::global(
        &mut module,
        "stage_indices",
        AddressSpace::Storage {
            access: StorageAccess::LOAD,
        },
        Some(binding(INDICES_BINDING)),
        words,
    );
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let records = build::runtime_array(&mut module, uvec4, 16);
    let vertices = build::global(
        &mut module,
        "stage_vertices",
        AddressSpace::Storage {
            access: StorageAccess::LOAD | StorageAccess::STORE,
        },
        Some(binding(VERTICES_BINDING)),
        records,
    );
    let uvec3 = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: naga::VectorSize::Tri,
                scalar: Scalar::U32,
            },
        },
        Span::UNDEFINED,
    );

    let mut b = Builder::new("main");
    b.f.arguments.push(FunctionArgument {
        name: Some("id".into()),
        ty: uvec3,
        binding: Some(Binding::BuiltIn(BuiltIn::GlobalInvocationId)),
    });
    let id = b.argument(0);
    let x = b.field(id, 0);
    let instance = b.field(id, 1);
    let v = b.global(values);
    let count = value(&mut b, v, word::COUNT);
    let past = b.binary(BinaryOperator::GreaterEqual, x, count);
    b.when(past, |b| b.ret(None));
    let first = value(&mut b, v, word::FIRST);
    let place = b.add(first, x);
    let number = b.local(u32_ty, Some(place));
    let index_bytes = value(&mut b, v, word::INDEX_BYTES);
    let zero = b.u32(0);
    let indexed = b.binary(BinaryOperator::NotEqual, index_bytes, zero);
    b.when(indexed, |b| {
        let offset = value(b, v, word::INDEX_OFFSET);
        let along = b.mul(place, index_bytes);
        let byte = b.add(offset, along);
        let two = b.u32(2);
        let w = b.binary(BinaryOperator::ShiftRight, byte, two);
        let p = b.global(indices);
        let p = b.at(p, w);
        let word = b.load(p);
        let half = b.binary(BinaryOperator::And, byte, two);
        let eight = b.u32(8);
        let shift = b.mul(half, eight);
        let shifted = b.binary(BinaryOperator::ShiftRight, word, shift);
        let mask = b.u32(0xffff);
        let short = b.binary(BinaryOperator::And, shifted, mask);
        let four = b.u32(4);
        let wide = b.binary(BinaryOperator::Equal, index_bytes, four);
        let index = b.select(wide, word, short);
        let base = value(b, v, word::BASE_VERTEX);
        let with_base = b.add(index, base);
        b.store(number, with_base);
    });
    let first_instance = value(&mut b, v, word::FIRST_INSTANCE);
    let instance_number = b.add(first_instance, instance);
    let vertex_number = b.load(number);
    let passed = numbers
        .iter()
        .map(|&n| {
            if n == 0 {
                vertex_number
            } else {
                instance_number
            }
        })
        .collect();
    let out = b.call(callee, passed, true).unwrap();
    let before = b.mul(instance, count);
    let r = b.add(before, x);
    let per = b.u32(record_words(locations.len()) / 4);
    let record = b.mul(r, per);
    let write = |b: &mut Builder, slot: u32, value: Option<(Option<u32>, ScalarKind)>| {
        let value = match value {
            Some((member, kind)) => {
                let v = match member {
                    Some(m) => b.field(out, m),
                    None => out,
                };
                if kind == ScalarKind::Uint {
                    v
                } else {
                    b.bitcast(v, ScalarKind::Uint)
                }
            }
            None => b.expr(naga::Expression::ZeroValue(uvec4)),
        };
        let s = b.u32(slot);
        let at = b.add(record, s);
        let p = b.global(vertices);
        let p = b.at(p, at);
        b.store(p, value);
    };
    write(&mut b, 0, Some((position, ScalarKind::Float)));
    for (i, slot) in slots.iter().enumerate() {
        let member = slot.map(|(m, kind)| {
            let member = match &module.types[result.ty].inner {
                TypeInner::Struct { .. } => Some(m),
                _ => None,
            };
            (member, kind)
        });
        write(&mut b, 1 + i as u32, member);
    }
    b.ret(None);
    module.entry_points.push(naga::EntryPoint {
        name: "main".into(),
        stage: naga::ShaderStage::Compute,
        early_depth_test: None,
        workgroup_size: [WORKGROUP, 1, 1],
        workgroup_size_overrides: None,
        function: b.finish(),
        mesh_info: None,
        task_payload: None,
        incoming_ray_payload: None,
    });
    Ok(module)
}

/// The vertex program of the draw after a geometry stage: each vertex attribute is a four-word slot of
/// the geometry output's record (location 0 the position, 1 + i the stage's i-th output location),
/// passed on with the type and interpolation the geometry shader's reflection gives.
pub(crate) fn passthrough(reflection: &Reflection) -> Result<Module> {
    let mut module = Module::default();
    let locations = locations(&reflection.outputs);
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let mut b = Builder::new("main");
    let mut members = Vec::new();
    let mut components = Vec::new();
    for slot in 0..=locations.len() {
        b.f.arguments.push(FunctionArgument {
            name: Some(format!("slot{slot}")),
            ty: uvec4,
            binding: Some(location(slot as u32, None)),
        });
        let raw = b.argument(slot as u32);
        let (kind, binding) = match slot {
            0 => (
                ScalarKind::Float,
                Binding::BuiltIn(BuiltIn::Position { invariant: false }),
            ),
            _ => {
                let loc = locations[slot - 1];
                let value = reflection
                    .outputs
                    .iter()
                    .find(|v| v.system.is_none() && v.location == loc)
                    .unwrap();
                let kind = match value.scalar {
                    ValueKind::Float => ScalarKind::Float,
                    ValueKind::Sint => ScalarKind::Sint,
                    ValueKind::Uint => ScalarKind::Uint,
                };
                let interpolation = match kind {
                    ScalarKind::Float => value.interpolation,
                    _ => Interpolation::Flat,
                };
                (kind, location(loc, Some(interpolation)))
            }
        };
        let ty = build::vec4(&mut module, Scalar { kind, width: 4 });
        let v = if kind == ScalarKind::Uint {
            raw
        } else {
            b.bitcast(raw, kind)
        };
        members.push(StructMember {
            name: Some(format!("v{slot}")),
            ty,
            binding: Some(binding),
            offset: 16 * slot as u32,
        });
        components.push(v);
    }
    if !reflection
        .outputs
        .iter()
        .any(|v| v.system == Some(SystemValue::Position))
    {
        return Err(ConvertError::Unsupported("GeometryWithoutPosition"));
    }
    let ty = module.types.insert(
        Type {
            name: Some("Outputs".into()),
            inner: TypeInner::Struct {
                members,
                span: 16 * (1 + locations.len() as u32),
            },
        },
        Span::UNDEFINED,
    );
    let out = b.compose(ty, components);
    b.ret(Some(out));
    let mut f = b.finish();
    f.result = Some(FunctionResult { ty, binding: None });
    module.entry_points.push(naga::EntryPoint {
        name: "main".into(),
        stage: naga::ShaderStage::Vertex,
        early_depth_test: None,
        workgroup_size: [0; 3],
        workgroup_size_overrides: None,
        function: f,
        mesh_info: None,
        task_payload: None,
        incoming_ray_payload: None,
    });
    Ok(module)
}

/// A location binding: a vertex input (`None`) or an output interpolated as given.
fn location(location: u32, interpolation: Option<Interpolation>) -> Binding {
    use naga::{Interpolation as I, Sampling as S};
    let (interpolation, sampling) = match interpolation {
        None => (None, None),
        Some(i) => {
            let (i, s) = match i {
                Interpolation::Flat => (I::Flat, None),
                Interpolation::Perspective => (I::Perspective, Some(S::Center)),
                Interpolation::Linear => (I::Linear, Some(S::Center)),
                Interpolation::PerspectiveCentroid => (I::Perspective, Some(S::Centroid)),
                Interpolation::LinearCentroid => (I::Linear, Some(S::Centroid)),
                Interpolation::PerspectiveSample => (I::Perspective, Some(S::Sample)),
                Interpolation::LinearSample => (I::Linear, Some(S::Sample)),
            };
            (Some(i), s)
        }
    };
    Binding::Location {
        location,
        interpolation,
        sampling,
        blend_src: None,
        per_primitive: false,
    }
}
