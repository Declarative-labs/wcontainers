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
use wcontainers_gpu::{Interpolation, Reflection, ScalarKind as ValueKind};

mod helper;
mod tessellate;
pub use helper::{
    DomainSets, FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages, StreamOutput,
    StreamOutputEntry, StreamTargets, TessellatedDraw,
};
pub use tessellate::{Tessellation, Tessellator};

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
/// The hull stage's output (`tess`).
pub(crate) const PATCHES_BINDING: u32 = 3;
/// The tessellator's points, which the domain stage's compute form reads.
pub(crate) const POINTS_BINDING: u32 = 4;
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
    /// 1 when the input primitives are listed after a tessellation stage: their total is the first
    /// word of the buffer at `INDICES_BINDING` over the vertices per primitive, and each primitive's
    /// record numbers follow from word `tess::INDEX_HEADER`; `PRIMITIVES` is then the most there can be.
    pub const LISTED: u32 = 10;
    /// Control points per patch in the API draw, which the hull stage steps by: a patch may have more
    /// than the hull shader declares, which then reads the first of them.
    pub const PATCH_POINTS: u32 = 11;
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
    compute_form(
        &vertex.modules[0],
        &vertex.modules[0].module,
        inputs,
        b"vertex records",
    )
}

/// The compute form of a domain program (converted), writing a record per tessellator point with a
/// slot for each of `inputs`: point `i` is record `i`. It reads the patches and points from the
/// helper's own set ([`PATCHES_BINDING`], [`POINTS_BINDING`]) where the drawn form reads them from its
/// helper set.
pub(crate) fn domain_program_for(
    domain: &crate::Converted,
    inputs: &[u32],
) -> Result<wcontainers_gpu::ShaderModule> {
    let source = &domain.modules[0];
    let mut module = source.module.clone();
    for (_, g) in module.global_variables.iter_mut() {
        let Some(rb) = &mut g.binding else { continue };
        if rb.group != wcontainers_gpu::HELPER_SET {
            continue;
        }
        match rb.binding {
            wcontainers_gpu::DOMAIN_PATCHES_BINDING => *rb = binding(PATCHES_BINDING),
            wcontainers_gpu::DOMAIN_POINTS_BINDING => *rb = binding(POINTS_BINDING),
            _ => {}
        }
    }
    compute_form(source, &module, inputs, b"domain records")
}

fn compute_form(
    source: &wcontainers_gpu::ShaderModule,
    module: &Module,
    inputs: &[u32],
    what: &[u8],
) -> Result<wcontainers_gpu::ShaderModule> {
    let module = vertex_as_compute(module, inputs)?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key).update(what);
    for l in inputs {
        key.update(&l.to_le_bytes());
    }
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
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
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// The tessellator program (`tessellator.wgsl`), which [`Stages`] runs after the hull stage.
pub fn tessellator_program() -> Result<wcontainers_gpu::ShaderModule> {
    let text = include_str!("stage/tessellator.wgsl");
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| ConvertError::Malformed(format!("the tessellator program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// The hull stage's output, which the tessellator and the domain program read: a header of
/// [`tess::HEADER_WORDS`] words the hull program writes, then one record per patch of the draw (all API
/// instances together). A record holds each output control point's slots (four words per location the
/// hull shader writes), then the patch constants' slots, then the tessellation factors (outer 0 to 3,
/// inner 0 and 1, as f32).
pub mod tess {
    /// Words per patch record.
    pub const PATCH_WORDS: u32 = 0;
    /// Words per control point.
    pub const POINT_WORDS: u32 = 1;
    /// The word of a record where the patch constants' slots start.
    pub const CONSTANT_BASE: u32 = 2;
    /// The word of a record where the factors start.
    pub const FACTOR_BASE: u32 = 3;
    /// From this word, the slot of each control point location (32), then of each patch constant
    /// location (32); [`NO_SLOT`] where the hull shader writes none.
    pub const SLOTS: u32 = 4;
    pub const NO_SLOT: u32 = u32::MAX;
    /// The slot of the control points' positions (`SV_Position` as control point data).
    pub const POSITION_SLOT: u32 = SLOTS + 2 * LOCATIONS;
    pub const HEADER_WORDS: u32 = 72;
    /// Locations a stage passes per control point and per patch, as Direct3D 11 has them.
    pub const LOCATIONS: u32 = 32;
    /// Words of the tessellator's counts buffer before its two words per patch: the indexed indirect
    /// draw's arguments, then the draw's point total.
    pub const COUNTS_BASE: u32 = 8;
    /// Words at the start of the tessellator's indices buffer before the indices: the draw's index
    /// total, then unused. The indexed indirect draw starts at this index.
    pub const INDEX_HEADER: u32 = 4;

    /// The most points and indices one patch can make with factors up to `max_factor`, for sizing
    /// the tessellator's output: at most `n` points along any edge, where `n` is the factor rounded up
    /// to the next odd or even number of segments plus one.
    pub fn patch_bounds(max_factor: f32) -> (u32, u32) {
        let f = if max_factor.is_nan() {
            64.0
        } else {
            max_factor.clamp(1.0, 64.0)
        };
        let n = (f.ceil() as u32 + 3).min(66);
        (n * n, 6 * n * n)
    }
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
    // Each location's member, scalar kind and component count (a record's slot holds four).
    let mut slots: Vec<Option<(u32, ScalarKind, u32)>> = alloc::vec![None; locations.len()];
    let mut place = |binding: &Binding, member: Option<u32>, inner: &TypeInner| -> Result<()> {
        let (kind, size) = match inner {
            TypeInner::Vector { scalar, size } => (scalar.kind, *size as u32),
            TypeInner::Scalar(scalar) => (scalar.kind, 1),
            // No stage after reads them: geometry and hull shaders reading clip distances are
            // refused at their own conversion, and their own outputs clip.
            _ if *binding == Binding::BuiltIn(BuiltIn::ClipDistances) => return Ok(()),
            _ => return Err(ConvertError::Unsupported("VertexOutputShape")),
        };
        match binding {
            Binding::BuiltIn(BuiltIn::Position { .. }) => position = Some(member),
            Binding::Location { location, .. } => {
                if let Some(i) = locations.iter().position(|l| l == location) {
                    slots[i] = Some((member.unwrap_or(0), kind, size));
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

    let mut b = Builder::new(crate::ENTRY_POINT);
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
    let write = |b: &mut Builder, slot: u32, value: Option<(Option<u32>, ScalarKind, u32)>| {
        let value = match value {
            Some((member, kind, size)) => {
                let v = match member {
                    Some(m) => b.field(out, m),
                    None => out,
                };
                let v = if kind == ScalarKind::Uint {
                    v
                } else {
                    b.bitcast(v, ScalarKind::Uint)
                };
                // Fewer than four components fill the slot's first ones; the rest are zero.
                match size {
                    4 => v,
                    1 => {
                        let zero = b.u32(0);
                        b.compose(uvec4, alloc::vec![v, zero, zero, zero])
                    }
                    n => {
                        let mut parts: Vec<_> = (0..n).map(|c| b.field(v, c)).collect();
                        let zero = b.u32(0);
                        parts.resize(4, zero);
                        b.compose(uvec4, parts)
                    }
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
    write(&mut b, 0, Some((position, ScalarKind::Float, 4)));
    for (i, slot) in slots.iter().enumerate() {
        let member = slot.map(|(m, kind, size)| {
            let member = match &module.types[result.ty].inner {
                TypeInner::Struct { .. } => Some(m),
                _ => None,
            };
            (member, kind, size)
        });
        write(&mut b, 1 + i as u32, member);
    }
    b.ret(None);
    module.entry_points.push(naga::EntryPoint {
        name: crate::ENTRY_POINT.into(),
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

fn is_position(b: &Binding) -> bool {
    matches!(b, Binding::BuiltIn(BuiltIn::Position { .. }))
}

/// Gives a vertex program that writes no position (one feeding stream output, a geometry or a hull
/// stage) the position (0, 0, 0, -1), outside every clip volume: drawn, it rasterises nothing, and its
/// compute form carries it as the record's position slot. The entry point becomes a function called by a
/// new entry point that adds the position to its outputs.
pub(crate) fn clip_unpositioned(module: &mut Module) -> Result<()> {
    let ep = &module.entry_points[0];
    if ep.stage != naga::ShaderStage::Vertex {
        return Ok(());
    }
    let mut outputs: Vec<(Handle<Type>, Binding, Option<u32>)> = Vec::new();
    if let Some(r) = &ep.function.result {
        match (&r.binding, &module.types[r.ty].inner) {
            (Some(b), _) => outputs.push((r.ty, b.clone(), None)),
            (None, TypeInner::Struct { members, .. }) => {
                for (i, m) in members.iter().enumerate() {
                    let b = m
                        .binding
                        .clone()
                        .ok_or(ConvertError::Malformed("an unbound output".into()))?;
                    outputs.push((m.ty, b, Some(i as u32)));
                }
            }
            _ => return Err(ConvertError::Malformed("vertex outputs".into())),
        }
    }
    if outputs.iter().any(|(_, b, _)| is_position(b)) {
        return Ok(());
    }
    let mut entry = module.entry_points.remove(0);
    let mut inner = core::mem::take(&mut entry.function);
    let arguments = inner.arguments.clone();
    for a in &mut inner.arguments {
        a.binding = None;
    }
    if let Some(r) = &mut inner.result {
        r.binding = None;
    }
    let returns = inner.result.is_some();
    inner.name = Some("vertex_shader".into());
    let callee = module.functions.append(inner, Span::UNDEFINED);
    let vec4 = build::vec4(module, Scalar::F32);

    let mut b = Builder::new(crate::ENTRY_POINT);
    let mut passed = Vec::new();
    for (i, a) in arguments.into_iter().enumerate() {
        b.f.arguments.push(a);
        passed.push(b.argument(i as u32));
    }
    let out = b.call(callee, passed, returns);
    let mut members = Vec::new();
    let mut components = Vec::new();
    for (ty, binding, member) in outputs {
        let v = match (out, member) {
            (Some(out), Some(m)) => b.field(out, m),
            (Some(out), None) => out,
            (None, _) => unreachable!("outputs without a result"),
        };
        members.push((ty, binding));
        components.push(v);
    }
    let lit = |b: &mut Builder, x: f32| b.expr(naga::Expression::Literal(naga::Literal::F32(x)));
    let zero = lit(&mut b, 0.0);
    let below = lit(&mut b, -1.0);
    let position = b.compose(vec4, alloc::vec![zero, zero, zero, below]);
    members.push((
        vec4,
        Binding::BuiltIn(BuiltIn::Position { invariant: false }),
    ));
    components.push(position);
    let span = 16 * members.len() as u32;
    let ty = module.types.insert(
        Type {
            name: Some("PositionedOutputs".into()),
            inner: TypeInner::Struct {
                members: members
                    .into_iter()
                    .enumerate()
                    .map(|(i, (ty, binding))| StructMember {
                        name: Some(format!("v{i}")),
                        ty,
                        binding: Some(binding),
                        offset: 16 * i as u32,
                    })
                    .collect(),
                span,
            },
        },
        Span::UNDEFINED,
    );
    let value = b.compose(ty, components);
    b.ret(Some(value));
    let mut f = b.finish();
    f.result = Some(FunctionResult { ty, binding: None });
    entry.function = f;
    module.entry_points.insert(0, entry);
    Ok(())
}

/// Whether a geometry stage's records end with the layer and viewport slot: when the shader writes
/// either.
pub fn selects(reflection: &Reflection) -> bool {
    reflection.outputs.iter().any(|v| {
        matches!(
            v.system,
            Some(wcontainers_gpu::SystemValue::RenderTargetArrayIndex)
                | Some(wcontainers_gpu::SystemValue::ViewportArrayIndex)
        )
    })
}

/// The helper constants of the vertex program after a geometry stage that writes a layer or viewport:
/// the draw's run, whose primitives alone it keeps.
pub const PASSTHROUGH_HELPERS: [wcontainers_gpu::HelperSlot; 2] = [
    wcontainers_gpu::HelperSlot {
        value: wcontainers_gpu::HelperValue::Layer,
        offset: 0,
    },
    wcontainers_gpu::HelperSlot {
        value: wcontainers_gpu::HelperValue::Viewport,
        offset: 4,
    },
];

/// The vertex program of the draw after a geometry stage: each vertex attribute is a four-word slot of
/// the geometry output's record (location 0 the position, 1 + i the stage's i-th output location),
/// passed on with the type and interpolation the geometry shader's reflection gives. When the stage
/// writes a layer or viewport, the record's last slot holds them and the program keeps only the
/// vertices of the run its helper constants name ([`PASSTHROUGH_HELPERS`], at the vertex stage's
/// helper binding), placing the others outside every clip volume.
pub(crate) fn passthrough(reflection: &Reflection) -> Result<Module> {
    let mut module = Module::default();
    let locations = locations(&reflection.outputs);
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let mut b = Builder::new(crate::ENTRY_POINT);
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
    if selects(reflection) {
        let slot = 1 + locations.len() as u32;
        b.f.arguments.push(FunctionArgument {
            name: Some("selection".into()),
            ty: uvec4,
            binding: Some(location(slot, None)),
        });
        let selection = b.argument(slot);
        let u32_ty = build::scalar(&mut module, Scalar::U32);
        let helpers_ty = module.types.insert(
            Type {
                name: Some("Run".into()),
                inner: TypeInner::Struct {
                    members: ["layer", "viewport"]
                        .iter()
                        .enumerate()
                        .map(|(i, name)| StructMember {
                            name: Some((*name).into()),
                            ty: u32_ty,
                            binding: None,
                            offset: 4 * i as u32,
                        })
                        .collect(),
                    span: 16,
                },
            },
            Span::UNDEFINED,
        );
        let helpers = build::global(
            &mut module,
            "run",
            AddressSpace::Uniform,
            Some(ResourceBinding {
                group: wcontainers_gpu::HELPER_SET,
                binding: wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Vertex),
            }),
            helpers_ty,
        );
        let run = b.global(helpers);
        let mut keep = None;
        for i in 0..2 {
            let p = b.field(run, i);
            let want = b.load(p);
            let have = b.field(selection, i);
            let same = b.binary(BinaryOperator::Equal, have, want);
            keep = Some(match keep {
                None => same,
                Some(k) => b.binary(BinaryOperator::LogicalAnd, k, same),
            });
        }
        let vec4 = build::vec4(&mut module, Scalar::F32);
        let lit =
            |b: &mut Builder, x: f32| b.expr(naga::Expression::Literal(naga::Literal::F32(x)));
        let zero = lit(&mut b, 0.0);
        let below = lit(&mut b, -1.0);
        let outside = b.compose(vec4, alloc::vec![zero, zero, zero, below]);
        components[0] = b.select(keep.unwrap(), components[0], outside);
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
        name: crate::ENTRY_POINT.into(),
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
