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
use alloc::string::String;
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, FunctionResult, Handle,
    Module, ResourceBinding, Scalar, ScalarKind, Span, StorageAccess, StructMember, Type,
    TypeInner, VectorSize,
};
use wcontainers_gpu::{
    Interpolation, Reflection, ScalarKind as ValueKind, VariantFlags, VariantKey,
};

mod helper;
mod replay;
mod tessellate;
pub use helper::{
    DomainSets, FinalDraw, InputTopology, STREAM_BUFFERS, STREAM_FILLED_WORDS, STREAMS, StageDraw,
    StageError, StageIndex, Stages, StreamOutput, StreamOutputEntry, StreamTargets,
    TessellatedDraw,
};
pub use replay::{GeometryReplay, ReplayDraw};
pub use tessellate::{Tessellation, Tessellator};

type Result<T> = core::result::Result<T, ConvertError>;

/// The set of the stage helper's own bindings in its compute programs.
pub(crate) const STAGE_SET: u32 = 1;

/// Bytes of one four-word slot of a record or block.
pub(crate) const RECORD_SLOT_BYTES: u32 = build::WORD * build::VEC4;
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
/// Vertices of a triangle, the most a primitive without adjacency has.
pub const TRIANGLE_VERTICES: u32 = 3;
/// Vertices of a line and of a triangle with adjacency.
pub const LINE_ADJACENCY_VERTICES: u32 = 4;
pub const TRIANGLE_ADJACENCY_VERTICES: u32 = 6;
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
    /// 1 when an indexed strip was made an indexed list ([`super::strip_program`]): the first word at
    /// `INDICES_BINDING` is the listed indices' total, and the draw's places past it up to `COUNT`
    /// hold no vertex: the vertex stage writes their records outside every clip volume, the
    /// geometry stage closes their slots and stream output skips their primitives.
    pub const LISTED_TOTAL: u32 = 12;
    /// The stream a geometry shader's replay form keeps the vertices of, for one of several streams.
    pub const STREAM: u32 = 13;
    pub const COUNT_OF: u32 = 14;
}

/// The position's w of a record that no clip volume holds.
pub(crate) const OUTSIDE_W: f32 = -1.0;

pub(crate) fn binding(binding: u32) -> ResourceBinding {
    ResourceBinding {
        group: STAGE_SET,
        binding,
    }
}

/// Output primitive slots of one geometry invocation in the replay form: one from each emitted vertex
/// that can begin a primitive of `corners` vertices among `max` emitted.
pub(crate) fn replay_slots(max: u32, corners: u32) -> u32 {
    (max + 1).saturating_sub(corners).max(1)
}

/// Words per record of a stage holding `locations` user locations.
pub(crate) fn record_words(locations: usize) -> u32 {
    (1 + locations as u32) * build::VEC4
}

/// The values block's global.
pub(crate) fn values(module: &mut Module) -> Handle<naga::GlobalVariable> {
    let u32_ty = build::scalar(module, Scalar::U32);
    let members = (0..word::COUNT_OF)
        .map(|i| StructMember {
            name: Some(format!("w{i}")),
            ty: u32_ty,
            binding: None,
            offset: i * build::WORD,
        })
        .collect();
    let ty = module.types.insert(
        Type {
            name: Some(VALUES_TYPE.into()),
            inner: TypeInner::Struct {
                members,
                span: word::COUNT_OF * build::WORD,
            },
        },
        Span::UNDEFINED,
    );
    build::global(
        module,
        VALUES_VARIABLE,
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
        VERTEX_RECORDS,
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
    compute_form(source, &module, inputs, DOMAIN_RECORDS)
}

fn compute_form(
    source: &wcontainers_gpu::ShaderModule,
    module: &Module,
    inputs: &[u32],
    what: &[u8],
) -> Result<wcontainers_gpu::ShaderModule> {
    compute_module(
        source,
        vertex_as_compute(module, inputs, false)?,
        inputs,
        what,
    )
}

/// A compute program made of `source` with the key of what it is and the record locations it writes.
fn compute_module(
    source: &wcontainers_gpu::ShaderModule,
    module: Module,
    inputs: &[u32],
    what: &[u8],
) -> Result<wcontainers_gpu::ShaderModule> {
    derived_module(
        source,
        module,
        inputs,
        what,
        wcontainers_gpu::ShaderStage::Compute,
    )
}

fn derived_module(
    source: &wcontainers_gpu::ShaderModule,
    module: Module,
    inputs: &[u32],
    what: &[u8],
    stage: wcontainers_gpu::ShaderStage,
) -> Result<wcontainers_gpu::ShaderModule> {
    let mut key = blake3::Hasher::new();
    key.update(&source.key).update(what);
    for l in inputs {
        key.update(&l.to_le_bytes());
    }
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage,
        key: *key.finalize().as_bytes(),
    })
}

/// The vertex program whose captured outputs a geometry shader's replay form reads
/// (`VariantFlags::GEOMETRY_REPLAY`): `vertex` (converted, its inputs as drawn) writing the records
/// `geometry` (converted) reads as flat four-word values from location 0, the position first, then
/// a slot for each of the geometry shader's input locations.
pub fn capture_program(
    vertex: &crate::Converted,
    geometry: &crate::Converted,
) -> Result<wcontainers_gpu::ShaderModule> {
    let inputs = locations(&geometry.reflection.inputs);
    capture_program_for(vertex, &inputs, (0, 0), CAPTURED_RECORDS)
}

/// A vertex program's stream output on the replay route ([`replay::GeometryReplay::streamed`]).
pub(crate) struct StreamedPrograms {
    /// The vertex program writing records of its position, each output location (`locations`) and
    /// its clip distances' slots as flat four-word values from location 0.
    pub(crate) capture: wcontainers_gpu::ShaderModule,
    /// The vertex program drawing those records, passing on every output with its own binding and
    /// the clip distances.
    pub(crate) passthrough: wcontainers_gpu::ShaderModule,
    pub(crate) locations: Vec<u32>,
    /// Four-word slots per record.
    pub(crate) slots: u32,
}

/// The programs of `vertex` (converted, its inputs as drawn) whose output is streamed on the replay
/// route. They take its outputs from its module, so every rewrite its key made reaches the pixel
/// program as it would from the vertex program.
pub(crate) fn streamed_programs(vertex: &crate::Converted) -> Result<StreamedPrograms> {
    let source = &vertex.modules[0];
    let drawn = drawn_outputs(source)?;
    if drawn.cull != 0 {
        return Err(ConvertError::Unsupported(STREAMED_CULL_DISTANCE));
    }
    let capture = capture_program_for(vertex, &drawn.locations, (drawn.clip, 0), STREAMED_RECORDS)?;
    let passed = drawn.passthrough_values();
    let passthrough = passthrough_of(&passed, false, drawn.clip, None, None)?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key).update(STREAMED_PASSTHROUGH);
    Ok(StreamedPrograms {
        capture,
        passthrough: vertex_module(passthrough, key)?,
        slots: 1 + drawn.locations.len() as u32 + distance_slots(drawn.clip),
        locations: drawn.locations,
    })
}

/// [`capture_program`] writing a slot for each of `inputs` (locations), then the slots of the
/// program's `clip` clip distances and of its `cull` cull distances, keyed as `what`.
fn capture_program_for(
    vertex: &crate::Converted,
    inputs: &[u32],
    (clip, cull): (u32, u32),
    what: &[u8],
) -> Result<wcontainers_gpu::ShaderModule> {
    let source = &vertex.modules[0];
    let mut module = source.module.clone();
    if module.entry_points.len() != 1 || module.entry_points[0].stage != naga::ShaderStage::Vertex {
        return Err(ConvertError::Malformed(NOT_A_VERTEX_PROGRAM.into()));
    }
    let mut entry = module.entry_points.remove(0);
    let mut inner = core::mem::take(&mut entry.function);
    let arguments = inner.arguments.clone();
    for a in &mut inner.arguments {
        a.binding = None;
    }
    let result = inner
        .result
        .as_mut()
        .ok_or(ConvertError::Malformed(VERTEX_WITHOUT_OUTPUTS.into()))?;
    // Each output's binding, member and type.
    let outputs: Vec<(Binding, Option<u32>, Handle<Type>)> =
        match (result.binding.take(), &module.types[result.ty].inner) {
            (Some(binding), _) => alloc::vec![(binding, None, result.ty)],
            (None, TypeInner::Struct { members, .. }) => members
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    m.binding
                        .clone()
                        .map(|b| (b, Some(i as u32), m.ty))
                        .ok_or(ConvertError::Malformed(UNBOUND_OUTPUT.into()))
                })
                .collect::<Result<_>>()?,
            _ => return Err(ConvertError::Malformed(VERTEX_OUTPUTS.into())),
        };
    inner.name = Some(CAPTURED_FUNCTION.into());
    let callee = module.functions.append(inner, Span::UNDEFINED);
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let vec4 = build::vec4(&mut module, Scalar::F32);

    let mut b = Builder::new(crate::ENTRY_POINT);
    let mut passed = Vec::new();
    for (i, a) in arguments.into_iter().enumerate() {
        b.f.arguments.push(a);
        passed.push(b.argument(i as u32));
    }
    let out = b.call(callee, passed, true).expect("a result");
    let value_of = |b: &mut Builder, wanted: &dyn Fn(&Binding) -> bool| -> Result<Expr> {
        let Some((_, member, ty)) = outputs.iter().find(|(binding, ..)| wanted(binding)) else {
            return Ok(b.expr(naga::Expression::ZeroValue(uvec4)));
        };
        let (kind, size) = match module.types[*ty].inner {
            TypeInner::Vector { scalar, size } => (scalar.kind, size as u32),
            TypeInner::Scalar(scalar) => (scalar.kind, 1),
            _ => return Err(ConvertError::Unsupported(VERTEX_OUTPUT_SHAPE)),
        };
        let v = match member {
            Some(m) => b.field(out, *m),
            None => out,
        };
        Ok(as_slot(b, uvec4, v, kind, size))
    };
    let position = value_of(&mut b, &|binding| is_position(binding))?;
    let mut slots = alloc::vec![position];
    for &l in inputs {
        let at = |binding: &Binding| matches!(binding, Binding::Location { location, .. } if *location == l);
        slots.push(value_of(&mut b, &at)?);
    }
    for (built_in, count) in [
        (BuiltIn::ClipDistances, clip),
        (BuiltIn::CullDistance, cull),
    ] {
        if count == 0 {
            continue;
        }
        let member = outputs
            .iter()
            .find(|(binding, ..)| *binding == Binding::BuiltIn(built_in))
            .and_then(|(_, member, _)| *member)
            .ok_or(ConvertError::Malformed(VERTEX_OUTPUTS.into()))?;
        let distances = b.field(out, member);
        for k in 0..distance_slots(count) {
            let zero = b.u32(0);
            let parts: Vec<Expr> = (0..build::VEC4)
                .map(|c| match k * build::VEC4 + c {
                    i if i < count => {
                        let d = b.field(distances, i);
                        b.bitcast(d, ScalarKind::Uint)
                    }
                    _ => zero,
                })
                .collect();
            slots.push(b.compose(uvec4, parts));
        }
    }
    let mut members = Vec::new();
    for i in 0..slots.len() as u32 {
        members.push(StructMember {
            name: Some(format!("slot{i}")),
            ty: uvec4,
            binding: Some(location(i, Some(Interpolation::Flat))),
            offset: RECORD_SLOT_BYTES * i,
        });
    }
    let position = b.bitcast(slots[0], ScalarKind::Float);
    members.push(StructMember {
        name: Some(POSITION_MEMBER.into()),
        ty: vec4,
        binding: Some(Binding::BuiltIn(BuiltIn::Position { invariant: false })),
        offset: RECORD_SLOT_BYTES * slots.len() as u32,
    });
    slots.push(position);
    let ty = module.types.insert(
        Type {
            name: Some(OUTPUTS_TYPE.into()),
            inner: TypeInner::Struct {
                span: RECORD_SLOT_BYTES * members.len() as u32,
                members,
            },
        },
        Span::UNDEFINED,
    );
    let value = b.compose(ty, slots);
    b.ret(Some(value));
    let mut f = b.finish();
    f.result = Some(FunctionResult { ty, binding: None });
    entry.function = f;
    module.entry_points.push(entry);
    derived_module(
        source,
        module,
        inputs,
        what,
        wcontainers_gpu::ShaderStage::Vertex,
    )
}

/// `v`, of `size` components of `kind`, as a record slot: its bits in the first components, the rest
/// zero.
fn as_slot(b: &mut Builder, uvec4: Handle<Type>, v: Expr, kind: ScalarKind, size: u32) -> Expr {
    let v = if kind == ScalarKind::Uint {
        v
    } else {
        b.bitcast(v, ScalarKind::Uint)
    };
    match size {
        build::VEC4 => v,
        1 => {
            let zero = b.u32(0);
            b.compose(uvec4, alloc::vec![v, zero, zero, zero])
        }
        n => {
            let mut parts: Vec<_> = (0..n).map(|c| b.field(v, c)).collect();
            let zero = b.u32(0);
            parts.resize(build::VEC4 as usize, zero);
            b.compose(uvec4, parts)
        }
    }
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

const STRIP_PROGRAM: &str = include_str!("stage/strips.wgsl");

/// The program listing an indexed strip's primitives (`strips.wgsl`), which [`Stages`] runs before the
/// vertex stage of an indexed strip.
pub fn strip_program() -> Result<wcontainers_gpu::ShaderModule> {
    let module = naga::front::wgsl::parse_str(STRIP_PROGRAM)
        .map_err(|e| ConvertError::Malformed(format!("the strip program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(STRIP_PROGRAM.as_bytes()).as_bytes(),
    })
}

/// The cull program's source: one invocation per API primitive writes its vertices' record numbers
/// into the index list, or the empty record's for each when the primitive is dropped.
const CULL_PROGRAM: &str = r#"// Cull distances (Direct3D drops a primitive when all its vertices are below zero on one of them): one
// invocation per API primitive writes its vertices' record numbers into the index list, in the API's
// order, or the empty record's number for each when the primitive is dropped. Triangles of a strip
// are listed with their first vertex leading and their winding kept. Invocation 0 also writes the
// empty record: a position outside every clip volume and zeros.

struct Values {
    // API primitives times API instances.
    units: u32,
    // API primitives per instance.
    primitives: u32,
    // API vertices per instance: records per instance.
    count: u32,
    // Vertices per primitive: 1, 2 or 3.
    vertices: u32,
    // Primitives come from a strip.
    strip: u32,
    // Words per record.
    record_words: u32,
    // The word of a record where the cull distances start, and how many there are.
    cull_word: u32,
    cull_count: u32,
    // The word where the index list starts.
    index_base: u32,
    // The empty record's number.
    empty: u32,
    pad0: u32,
    pad1: u32,
}

// The records, then the index list.
@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(1) var<storage, read_write> data: array<u32>;

const MOST_VERTICES: u32 = 3u;
// The position's w, which no clip volume holds.
const OUTSIDE_W: f32 = -1.0;
const W_COMPONENT: u32 = 3u;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let unit = id.x;
    if unit == 0u {
        let empty = values.empty * values.record_words;
        for (var w = 0u; w < values.record_words; w++) {
            data[empty + w] = 0u;
        }
        data[empty + W_COMPONENT] = bitcast<u32>(OUTSIDE_W);
    }
    if unit >= values.units {
        return;
    }
    let instance = unit / values.primitives;
    let primitive = unit % values.primitives;
    let n = values.vertices;
    let strip = values.strip != 0u;
    var first = primitive * n;
    if strip {
        first = primitive;
    }
    var records: array<u32, MOST_VERTICES>;
    for (var k = 0u; k < n; k++) {
        var v = first + k;
        // An odd triangle of a strip: (first, first + 2, first + 1).
        if strip && n == MOST_VERTICES && (primitive & 1u) == 1u && k != 0u {
            v = first + MOST_VERTICES - k;
        }
        records[k] = instance * values.count + v;
    }
    var dropped = false;
    for (var d = 0u; d < values.cull_count; d++) {
        var below = true;
        for (var k = 0u; k < n; k++) {
            let at = records[k] * values.record_words + values.cull_word + d;
            below = below && bitcast<f32>(data[at]) < 0.0;
        }
        dropped = dropped || below;
    }
    for (var k = 0u; k < n; k++) {
        data[values.index_base + unit * n + k] = select(records[k], values.empty, dropped);
    }
}
"#;

/// The cull program ([`CULL_PROGRAM`]), which [`Stages`] runs after a vertex stage writing cull
/// distances.
pub fn cull_program() -> Result<wcontainers_gpu::ShaderModule> {
    let text = CULL_PROGRAM;
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| ConvertError::Malformed(format!("the cull program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// A vertex program writing cull distances, drawn through the stage helper
/// ([`wcontainers_gpu::SlowPath::CullDistanceAsGeometry`]).
pub struct CullPrograms {
    /// Its compute form, writing records of its position, each output location (`locations`), its
    /// clip distances' slots and its cull distances' slots.
    pub compute: wcontainers_gpu::ShaderModule,
    /// The vertex program drawing those records, passing on every output with its own binding and
    /// the clip distances.
    pub passthrough: wcontainers_gpu::ShaderModule,
    pub locations: Vec<u32>,
    pub clip_distances: u32,
    pub cull_distances: u32,
}

/// Names in the pass-through program: its layer and viewport slot's argument, the run's type, members
/// and variable, and its outputs' type.
const SELECTION_ARGUMENT: &str = "selection";
const RUN_TYPE: &str = "Run";
const RUN_MEMBERS: [&str; 3] = ["layer", "viewport", "layer_count"];
/// The member of the selection slot and of the run naming the layer.
const LAYER_MEMBER: u32 = 0;
const LAYER_COUNT_MEMBER: u32 = 2;
const NAMED_LAYER_ARGUMENT: &str = "named_layer";
const COVERAGE_ARGUMENT: &str = "line_coverage";
const PRIMITIVE_ARGUMENT: &str = "primitive_id";

/// A last slot of the records a pass-through draws, and the location it passes it on at.
#[derive(Clone, Copy)]
enum ExtraSlot {
    /// Line coverage values, four f32 without perspective.
    LineCoverage(u32),
    /// The primitive's number in the first word of four flat u32: whole registers on both sides, as
    /// Direct3D 12 links stage values register by register.
    PrimitiveId(u32),
}
const RUN_VARIABLE: &str = "draw_run";
const OUTPUTS_TYPE: &str = "Outputs";
/// What the vertex and domain stages' compute forms are, in their keys.
const VERTEX_RECORDS: &[u8] = b"vertex records";
/// The key part of [`capture_program`]'s module.
const CAPTURED_RECORDS: &[u8] = b"captured records";
/// What [`StreamedPrograms::capture`] and [`StreamedPrograms::passthrough`] are, in their keys.
const STREAMED_RECORDS: &[u8] = b"streamed records";
const STREAMED_PASSTHROUGH: &[u8] = b"streamed pass-through";
/// A vertex program writing cull distances whose output is streamed on the replay route.
const STREAMED_CULL_DISTANCE: &str = "StreamOutCullDistance";
/// The function the vertex program's entry point becomes in [`capture_program`].
const CAPTURED_FUNCTION: &str = "captured_vertex_shader";
/// The position's member in [`capture_program`]'s outputs.
pub(crate) const POSITION_MEMBER: &str = "position";
const DOMAIN_RECORDS: &[u8] = b"domain records";
/// Names in the helper's programs: its values block's type and variable, the vertex compute form's
/// index and record variables, its invocation argument, and the output type of a vertex program
/// given a position.
const VALUES_TYPE: &str = "StageValues";
const VALUES_VARIABLE: &str = "stage_values";
const INDICES_VARIABLE: &str = "stage_indices";
pub(crate) const VERTICES_VARIABLE: &str = "stage_vertices";
/// The name of a geometry shader's compute form's bound list of primitives.
pub const LIST_VARIABLE: &str = "stage_list";
pub(crate) const ID_ARGUMENT: &str = "id";
const POSITIONED_OUTPUTS_TYPE: &str = "PositionedOutputs";
/// Refusals of a vertex program by the compute form, and what is malformed in one.
pub const VERTEX_INPUT_NOT_PULLED: &str = "VertexInputNotPulled";
const VERTEX_INPUT_FOR_GEOMETRY: &str = "VertexInputForGeometry";
const VERTEX_WITHOUT_POSITION: &str = "VertexWithoutPosition";
const NOT_A_VERTEX_PROGRAM: &str = "not a vertex program";
const VERTEX_WITHOUT_OUTPUTS: &str = "a vertex program without outputs";
const UNBOUND_OUTPUT: &str = "an unbound output";
const VERTEX_OUTPUTS: &str = "vertex outputs";
/// What [`CullPrograms::compute`] is, in its key.
const CULL_RECORDS: &[u8] = b"cull records";
/// What [`CullPrograms::passthrough`] is, in its key.
const CULL_PASSTHROUGH: &[u8] = b"cull pass-through";
const VERTEX_OUTPUT_SHAPE: &str = "VertexOutputShape";
const VERTEX_OUTPUT_FOR_GEOMETRY: &str = "VertexOutputForGeometry";
/// The refusal of a vertex program without cull distances on their route.
const NO_CULL_DISTANCES: &str = "a vertex program without cull distances";

/// The programs of `vertex` (converted with every input pulled) on the cull route. They take its
/// outputs from its module, so every rewrite its key made (values linked for the pixel program
/// among them) reaches the pixel program as it would from the vertex program.
pub fn cull_programs(vertex: &crate::Converted) -> Result<CullPrograms> {
    let source = &vertex.modules[0];
    let drawn = drawn_outputs(source)?;
    if drawn.cull == 0 {
        return Err(ConvertError::Malformed(NO_CULL_DISTANCES.into()));
    }
    let compute = compute_module(
        source,
        vertex_as_compute(&source.module, &drawn.locations, true)?,
        &drawn.locations,
        CULL_RECORDS,
    )?;
    let slots = drawn.passthrough_values();
    let passthrough = passthrough_of(&slots, false, drawn.clip, None, None)?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key).update(CULL_PASSTHROUGH);
    Ok(CullPrograms {
        compute,
        passthrough: vertex_module(passthrough, key)?,
        locations: drawn.locations,
        clip_distances: drawn.clip,
        cull_distances: drawn.cull,
    })
}

/// A vertex program on the cull or the primitive route without compute
/// ([`replay::GeometryReplay::culled`], [`replay::GeometryReplay::primitive_numbered`]).
pub(crate) struct ReplayedPrimitivePrograms {
    /// The vertex program writing records of its position, each output location, its clip distances'
    /// slots and its cull distances' slots as flat four-word values from location 0.
    pub(crate) capture: wcontainers_gpu::ShaderModule,
    /// The route's pass-through ([`CullPrograms::passthrough`], [`PrimitivePrograms::passthrough`]).
    pub(crate) passthrough: wcontainers_gpu::ShaderModule,
    /// Slots of a vertex record the pass-through draws: the position, the locations and the clip
    /// distances; then, on the primitive route, the number's slot.
    pub(crate) drawn_slots: u32,
    pub(crate) cull_distances: u32,
}

/// What [`ReplayedPrimitivePrograms::capture`] is, in its key.
const REPLAYED_PRIMITIVE_RECORDS: &[u8] = b"replayed primitive records";

/// The programs of `vertex` (converted, its inputs as drawn) on the cull route without compute. They
/// take its outputs from its module, as [`cull_programs`] does.
pub(crate) fn cull_capture_programs(
    vertex: &crate::Converted,
) -> Result<ReplayedPrimitivePrograms> {
    let drawn = drawn_outputs(&vertex.modules[0])?;
    if drawn.cull == 0 {
        return Err(ConvertError::Malformed(NO_CULL_DISTANCES.into()));
    }
    replayed_primitive_programs(vertex, &drawn, None)
}

/// The programs of `vertex` (converted, its inputs as drawn) on the primitive route without compute,
/// for a pixel program converted with `pixel`'s key, as [`primitive_programs`] makes them.
pub(crate) fn primitive_capture_programs(
    vertex: &crate::Converted,
    pixel: &VariantKey,
) -> Result<ReplayedPrimitivePrograms> {
    let drawn = drawn_outputs(&vertex.modules[0])?;
    if drawn.cull != 0 {
        return Err(ConvertError::Unsupported(PRIMITIVE_CULL_DISTANCE));
    }
    replayed_primitive_programs(vertex, &drawn, Some(primitive_id_location(pixel)))
}

/// The capture program of `vertex` with its `drawn` outputs, and the pass-through passing the
/// primitive's number on at `numbered`, or none.
fn replayed_primitive_programs(
    vertex: &crate::Converted,
    drawn: &DrawnOutputs,
    numbered: Option<u32>,
) -> Result<ReplayedPrimitivePrograms> {
    let source = &vertex.modules[0];
    let capture = capture_program_for(
        vertex,
        &drawn.locations,
        (drawn.clip, drawn.cull),
        REPLAYED_PRIMITIVE_RECORDS,
    )?;
    let passed = drawn.passthrough_values();
    let extra = numbered.map(ExtraSlot::PrimitiveId);
    let passthrough = passthrough_of(&passed, false, drawn.clip, None, extra)?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key);
    match numbered {
        Some(at) => key.update(PRIMITIVE_PASSTHROUGH).update(&at.to_le_bytes()),
        None => key.update(CULL_PASSTHROUGH),
    };
    let record_slots = 1 + drawn.locations.len() as u32 + distance_slots(drawn.clip);
    Ok(ReplayedPrimitivePrograms {
        capture,
        passthrough: vertex_module(passthrough, key)?,
        drawn_slots: record_slots + u32::from(numbered.is_some()),
        cull_distances: drawn.cull,
    })
}

/// A vertex program of the pass-through kind, keyed by `key`.
fn vertex_module(module: Module, key: blake3::Hasher) -> Result<wcontainers_gpu::ShaderModule> {
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Vertex,
        key: *key.finalize().as_bytes(),
    })
}

/// A vertex program's outputs as the stage helper's records hold them when it draws them itself: the
/// located values (location, kind, binding) in location order, and its clip and cull distances.
struct DrawnOutputs {
    /// Each location output's location, kind, width (`None` a scalar) and binding.
    passed: Vec<(u32, ScalarKind, Option<VectorSize>, Binding)>,
    locations: Vec<u32>,
    clip: u32,
    cull: u32,
}

impl DrawnOutputs {
    /// The values a pass-through writes, each as the vertex program declares it.
    fn passthrough_values(&self) -> Vec<(ScalarKind, Option<VectorSize>, Binding)> {
        self.passed
            .iter()
            .map(|(_, k, size, b)| (*k, *size, b.clone()))
            .collect()
    }
}

fn drawn_outputs(source: &wcontainers_gpu::ShaderModule) -> Result<DrawnOutputs> {
    let module = &source.module;
    let ep = module
        .entry_points
        .first()
        .ok_or(ConvertError::Malformed(NOT_A_VERTEX_PROGRAM.into()))?;
    let result = ep
        .function
        .result
        .as_ref()
        .ok_or(ConvertError::Malformed(VERTEX_WITHOUT_OUTPUTS.into()))?;
    let outputs: Vec<(Handle<Type>, Binding)> =
        match (&result.binding, &module.types[result.ty].inner) {
            (Some(b), _) => alloc::vec![(result.ty, b.clone())],
            (None, TypeInner::Struct { members, .. }) => members
                .iter()
                .filter_map(|m| m.binding.clone().map(|b| (m.ty, b)))
                .collect(),
            _ => Vec::new(),
        };
    let mut passed: Vec<(u32, ScalarKind, Option<VectorSize>, Binding)> = Vec::new();
    let (mut clip, mut cull) = (0, 0);
    for (ty, binding) in outputs {
        let inner = &module.types[ty].inner;
        match (&binding, inner) {
            (Binding::Location { location, .. }, TypeInner::Vector { scalar, size }) => {
                passed.push((*location, scalar.kind, Some(*size), binding.clone()));
            }
            (Binding::Location { location, .. }, TypeInner::Scalar(scalar)) => {
                passed.push((*location, scalar.kind, None, binding.clone()));
            }
            (
                Binding::BuiltIn(b),
                TypeInner::Array {
                    size: naga::ArraySize::Constant(n),
                    ..
                },
            ) => match b {
                BuiltIn::ClipDistances => clip = n.get(),
                BuiltIn::CullDistance => cull = n.get(),
                _ => return Err(ConvertError::Unsupported(VERTEX_OUTPUT_SHAPE)),
            },
            (Binding::BuiltIn(BuiltIn::Position { .. }), _) => {}
            _ => return Err(ConvertError::Unsupported(VERTEX_OUTPUT_FOR_GEOMETRY)),
        }
    }
    passed.sort_unstable_by_key(|(l, ..)| *l);
    let locations: Vec<u32> = passed.iter().map(|(l, ..)| *l).collect();
    Ok(DrawnOutputs {
        passed,
        locations,
        clip,
        cull,
    })
}

/// A vertex program's lines drawn antialiased through the stage helper
/// ([`wcontainers_gpu::SlowPath::AntialiasedLinesAsQuads`]).
pub struct LinePrograms {
    /// The vertex stage writing records of its position, each output location (`locations`) and its
    /// clip distances' slots: its compute form ([`line_programs`]), or the vertex program whose
    /// output is captured ([`line_capture_programs`]).
    pub records: wcontainers_gpu::ShaderModule,
    /// The vertex program drawing the rectangles' corners: a record as `records` writes it, then the
    /// coverage slot, passed on at the pixel program's [`line_coverage_location`].
    pub passthrough: wcontainers_gpu::ShaderModule,
    pub locations: Vec<u32>,
    pub clip_distances: u32,
    /// The record slots after the position taken along the line at a corner (bit i: slot 1 + i), the
    /// others the line's first vertex's; and those of them interpolated without perspective.
    pub interpolated: u32,
    pub linear: u32,
}

/// The members of the line coverage values the stage helper's rectangle passes on: the distance from
/// the line's axis, the distance along it from its first end, and its length, in pixels.
pub(crate) mod coverage {
    pub const ACROSS: u32 = 0;
    pub const ALONG: u32 = 1;
    pub const LENGTH: u32 = 2;
}

/// What [`LinePrograms`] are, in their keys.
const LINE_RECORDS: &[u8] = b"line records";
const LINE_CAPTURED: &[u8] = b"line captured records";
const LINE_PASSTHROUGH: &[u8] = b"line pass-through";
/// The refusals of the line route by name: cull distances, and more record slots than its masks hold.
const LINE_CULL_DISTANCE: &str = "AntialiasedLinesCullDistance";
const LINE_SLOTS: &str = "AntialiasedLinesSlots";

/// Where a pixel program made with `VariantFlags::LINE_COVERAGE` reads the coverage values, and the
/// pass-through of [`line_programs`] writes them: the location after the values `linked` holds.
pub fn line_coverage_location(pixel: &VariantKey) -> u32 {
    named_layer_location(pixel) + u32::from(pixel.flags.contains(VariantFlags::LAYER_FROM_STAGE))
}

/// The programs of `vertex` (converted with every input pulled) drawing its lines antialiased for a
/// pixel program converted with `pixel`'s key. They take its outputs from its module, so every
/// rewrite its key made reaches the pixel program as it would from the vertex program.
pub fn line_programs(vertex: &crate::Converted, pixel: &VariantKey) -> Result<LinePrograms> {
    let source = &vertex.modules[0];
    line_drawing(source, pixel, |drawn| {
        compute_module(
            source,
            vertex_as_compute(&source.module, &drawn.locations, true)?,
            &drawn.locations,
            LINE_RECORDS,
        )
    })
}

/// [`line_programs`] for a back end without compute ([`replay::GeometryReplay::antialiased_lines`]):
/// `vertex` converted with its inputs as drawn, its records written as flat four-word values from
/// location 0 for capture.
pub fn line_capture_programs(
    vertex: &crate::Converted,
    pixel: &VariantKey,
) -> Result<LinePrograms> {
    line_drawing(&vertex.modules[0], pixel, |drawn| {
        capture_program_for(vertex, &drawn.locations, (drawn.clip, 0), LINE_CAPTURED)
    })
}

/// [`line_programs`] of `source` with `records` making the vertex stage's program writing the
/// records.
fn line_drawing(
    source: &wcontainers_gpu::ShaderModule,
    pixel: &VariantKey,
    records: impl FnOnce(&DrawnOutputs) -> Result<wcontainers_gpu::ShaderModule>,
) -> Result<LinePrograms> {
    let drawn = drawn_outputs(source)?;
    if drawn.cull != 0 {
        return Err(ConvertError::Unsupported(LINE_CULL_DISTANCE));
    }
    let slots = drawn.passed.len() as u32 + distance_slots(drawn.clip);
    if slots > u32::BITS {
        return Err(ConvertError::Unsupported(LINE_SLOTS));
    }
    let records = records(&drawn)?;
    let (mut interpolated, mut linear) = (0u32, 0u32);
    for (i, (_, kind, _, binding)) in drawn.passed.iter().enumerate() {
        let Binding::Location { interpolation, .. } = binding else {
            continue;
        };
        let bit = 1 << i;
        match (kind, interpolation) {
            (ScalarKind::Float, Some(naga::Interpolation::Linear)) => {
                interpolated |= bit;
                linear |= bit;
            }
            (ScalarKind::Float, Some(naga::Interpolation::Perspective) | None) => {
                interpolated |= bit
            }
            _ => {}
        }
    }
    for k in drawn.passed.len() as u32..slots {
        interpolated |= 1 << k;
    }
    let coverage = line_coverage_location(pixel);
    let passed = drawn.passthrough_values();
    let passthrough = passthrough_of(
        &passed,
        false,
        drawn.clip,
        None,
        Some(ExtraSlot::LineCoverage(coverage)),
    )?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key)
        .update(LINE_PASSTHROUGH)
        .update(&coverage.to_le_bytes());
    Ok(LinePrograms {
        records,
        passthrough: vertex_module(passthrough, key)?,
        locations: drawn.locations,
        clip_distances: drawn.clip,
        interpolated,
        linear,
    })
}

/// Where a pixel program made with `VariantFlags::PRIMITIVE_ID_FROM_STAGE` reads its primitive's
/// number, and the pass-through of [`primitive_programs`] writes it: after the line coverage values.
pub fn primitive_id_location(pixel: &VariantKey) -> u32 {
    line_coverage_location(pixel) + u32::from(pixel.flags.contains(VariantFlags::LINE_COVERAGE))
}

/// A vertex program drawn through the stage helper so that its pixel program reads its primitive's
/// number from it ([`wcontainers_gpu::SlowPath::PrimitiveIdAsGeometry`]).
pub struct PrimitivePrograms {
    /// Its compute form, writing records of its position, each output location (`locations`) and its
    /// clip distances' slots.
    pub compute: wcontainers_gpu::ShaderModule,
    /// The vertex program drawing each primitive's copies of those records, then the number's slot,
    /// passed on at the pixel program's [`primitive_id_location`].
    pub passthrough: wcontainers_gpu::ShaderModule,
    pub locations: Vec<u32>,
    pub clip_distances: u32,
}

/// What [`PrimitivePrograms`] are, in their keys.
const PRIMITIVE_RECORDS: &[u8] = b"primitive records";
const PRIMITIVE_PASSTHROUGH: &[u8] = b"primitive pass-through";
/// The refusal of the primitive route for a vertex program writing cull distances.
const PRIMITIVE_CULL_DISTANCE: &str = "PrimitiveIdCullDistance";

/// The programs of `vertex` (converted with every input pulled) passing the primitive's number to a
/// pixel program converted with `pixel`'s key. They take its outputs from its module, so every
/// rewrite its key made reaches the pixel program as it would from the vertex program.
pub fn primitive_programs(
    vertex: &crate::Converted,
    pixel: &VariantKey,
) -> Result<PrimitivePrograms> {
    let source = &vertex.modules[0];
    let drawn = drawn_outputs(source)?;
    if drawn.cull != 0 {
        return Err(ConvertError::Unsupported(PRIMITIVE_CULL_DISTANCE));
    }
    let compute = compute_module(
        source,
        vertex_as_compute(&source.module, &drawn.locations, true)?,
        &drawn.locations,
        PRIMITIVE_RECORDS,
    )?;
    let at = primitive_id_location(pixel);
    let passed = drawn.passthrough_values();
    let passthrough = passthrough_of(
        &passed,
        false,
        drawn.clip,
        None,
        Some(ExtraSlot::PrimitiveId(at)),
    )?;
    let mut key = blake3::Hasher::new();
    key.update(&source.key)
        .update(PRIMITIVE_PASSTHROUGH)
        .update(&at.to_le_bytes());
    Ok(PrimitivePrograms {
        compute,
        passthrough: vertex_module(passthrough, key)?,
        locations: drawn.locations,
        clip_distances: drawn.clip,
    })
}

/// The primitive program's source: one invocation per API primitive copies its vertices' records with
/// its number.
const PRIMITIVE_PROGRAM: &str = include_str!("stage/primitives.wgsl");

/// The primitive program ([`PRIMITIVE_PROGRAM`]), which [`Stages`] runs after the vertex stage on the
/// primitive route.
pub fn primitive_program() -> Result<wcontainers_gpu::ShaderModule> {
    let text = PRIMITIVE_PROGRAM;
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| ConvertError::Malformed(format!("the primitive program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// The line program's source: one invocation per API line writes the four corners of a rectangle
/// around it and its two triangles' indices.
const LINE_PROGRAM: &str = include_str!("stage/lines.wgsl");

/// The line program ([`LINE_PROGRAM`]), which [`Stages`] runs after the vertex stage on the line
/// route.
pub fn line_program() -> Result<wcontainers_gpu::ShaderModule> {
    let text = LINE_PROGRAM;
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| ConvertError::Malformed(format!("the line program: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// The line replay program's body: one invocation per corner of each line's two triangles, its
/// record for capture.
const LINE_REPLAY_PROGRAM: &str = include_str!("stage/line_replay.wgsl");
/// What it is, in its errors, and its function giving a vertex's record.
const LINE_REPLAY: &str = "the line replay program";
const LINE_REPLAY_FUNCTION: &str = "line_corner";

/// The line replay program ([`LINE_REPLAY_PROGRAM`]) writing corner records of `slots` four-word
/// slots, which [`replay::GeometryReplay::antialiased_lines`] captures after the vertex stage.
pub(crate) fn line_replay_program(slots: u32) -> Result<wcontainers_gpu::ShaderModule> {
    replay_program(
        LINE_REPLAY,
        &format!("const SLOTS: u32 = {slots}u;\n{LINE_REPLAY_PROGRAM}"),
        slots,
        LINE_REPLAY_FUNCTION,
    )
}

/// The primitive replay program's body: one invocation per vertex of each primitive, its record (or
/// the empty record) and its primitive's number for capture.
const PRIMITIVE_REPLAY_PROGRAM: &str = include_str!("stage/primitive_replay.wgsl");
/// What it is, in its errors, and its function giving a vertex's record.
const PRIMITIVE_REPLAY: &str = "the primitive replay program";
const PRIMITIVE_REPLAY_FUNCTION: &str = "replayed";

/// The primitive replay program ([`PRIMITIVE_REPLAY_PROGRAM`]) writing records of `slots` four-word
/// slots from captured records of `record_slots`, which [`replay::GeometryReplay::culled`] and
/// [`replay::GeometryReplay::primitive_numbered`] capture after the vertex stage.
pub(crate) fn primitive_replay_program(
    slots: u32,
    record_slots: u32,
) -> Result<wcontainers_gpu::ShaderModule> {
    replay_program(
        PRIMITIVE_REPLAY,
        &format!(
            "const SLOTS: u32 = {slots}u;\nconst RECORD_SLOTS: u32 = {record_slots}u;\n\
             {PRIMITIVE_REPLAY_PROGRAM}"
        ),
        slots,
        PRIMITIVE_REPLAY_FUNCTION,
    )
}

/// A vertex program for capture of `body`, whose `function` gives vertex `v`'s record of `slots`
/// four-word slots, written as flat values from location 0.
fn replay_program(
    what: &str,
    body: &str,
    slots: u32,
    function: &str,
) -> Result<wcontainers_gpu::ShaderModule> {
    let members: String = (0..slots)
        .map(|i| format!("    @location({i}) @interpolate(flat) s{i}: vec4<u32>,\n"))
        .collect();
    let written: String = (0..slots)
        .map(|i| format!("    out.s{i} = c[{i}];\n"))
        .collect();
    let text = format!(
        "struct Replayed {{\n    @builtin(position) position: vec4<f32>,\n{members}}}\n\
         {body}\n\
         @vertex fn {entry}(@builtin(vertex_index) v: u32) -> Replayed {{\n    \
         let c = {function}(v);\n    var out: Replayed;\n    \
         out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);\n{written}    return out;\n}}\n",
        entry = crate::ENTRY_POINT,
    );
    let module = naga::front::wgsl::parse_str(&text)
        .map_err(|e| ConvertError::Malformed(format!("{what}: {e}")))?;
    Ok(wcontainers_gpu::ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: wcontainers_gpu::ShaderStage::Vertex,
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
    /// Words of a record's tessellation factors: outer 0 to 3 and inner 0 and 1, in two slots.
    pub const FACTOR_WORDS: u32 = 8;
    /// Direct3D's largest tessellation factor (`D3D11_TESSELLATOR_MAX_TESSELLATION_FACTOR`).
    pub const MAX_FACTOR: f32 = 64.0;
    /// Points along an edge past the factor rounded up: a segment more to reach the next odd or even
    /// count, and the closing point.
    const EDGE_POINTS_PAST_FACTOR: u32 = 3;
    /// Indices of two triangles, the most a point adds.
    const INDICES_PER_POINT: u32 = 6;
    /// The hull shader's output primitive codes ([`wcontainers_gpu::StageExtras::Hull`]): points and
    /// lines; the others are triangles.
    pub const OUTPUT_POINT: u8 = 0;
    pub const OUTPUT_LINE: u8 = 1;

    /// Vertices per primitive the tessellator makes for `output` ([`OUTPUT_POINT`], ...).
    pub fn output_vertices(output: u8) -> u32 {
        match output {
            OUTPUT_POINT => 1,
            OUTPUT_LINE => 2,
            _ => super::TRIANGLE_VERTICES,
        }
    }

    /// The most points and indices one patch can make with factors up to `max_factor`, for sizing
    /// the tessellator's output: at most `n` points along any edge, where `n` is the factor rounded up
    /// to the next odd or even number of segments plus one.
    pub fn patch_bounds(max_factor: f32) -> (u32, u32) {
        let f = if max_factor.is_nan() {
            MAX_FACTOR
        } else {
            max_factor.clamp(1.0, MAX_FACTOR)
        };
        // At the largest factor, which is even, no segment is added.
        let most = MAX_FACTOR as u32 + EDGE_POINTS_PAST_FACTOR - 1;
        let n = (f.ceil() as u32 + EDGE_POINTS_PAST_FACTOR).min(most);
        (n * n, INDICES_PER_POINT * n * n)
    }
}

/// A converted vertex program as a compute program writing vertex records with a slot for each of
/// `locations` (the geometry stage's input locations), then, with `distances`, the clip distances'
/// slots and the cull distances' ([`distance_slots`] each). Its inputs must all be pulled
/// (`VariantKey::pulled`), so it reads only the vertex and instance numbers, which it makes as a draw
/// would: the first vertex plus the invocation's place, or the index there plus the base vertex.
pub(crate) fn vertex_as_compute(
    vertex: &Module,
    locations: &[u32],
    distances: bool,
) -> Result<Module> {
    let mut module = vertex.clone();
    if module.entry_points.len() != 1 || module.entry_points[0].stage != naga::ShaderStage::Vertex {
        return Err(ConvertError::Malformed(NOT_A_VERTEX_PROGRAM.into()));
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
                return Err(ConvertError::Unsupported(VERTEX_INPUT_NOT_PULLED));
            }
            _ => return Err(ConvertError::Unsupported(VERTEX_INPUT_FOR_GEOMETRY)),
        }
    }
    inner.arguments = arguments
        .iter()
        .map(|a| FunctionArgument {
            binding: None,
            ..a.clone()
        })
        .collect();
    let result = inner
        .result
        .take()
        .ok_or(ConvertError::Malformed(VERTEX_WITHOUT_OUTPUTS.into()))?;
    inner.result = Some(FunctionResult {
        ty: result.ty,
        binding: None,
    });
    // Where each output is: the position and the locations, as members of the result or the result.
    let mut position = None;
    // Each location's member, scalar kind and component count (a record's slot holds four).
    let mut slots: Vec<Option<(u32, ScalarKind, u32)>> = alloc::vec![None; locations.len()];
    // The clip and the cull distances' member and count.
    let mut clip = None;
    let mut cull = None;
    let mut place = |binding: &Binding, member: Option<u32>, inner: &TypeInner| -> Result<()> {
        let (kind, size) = match inner {
            TypeInner::Vector { scalar, size } => (scalar.kind, *size as u32),
            TypeInner::Scalar(scalar) => (scalar.kind, 1),
            // Without `distances` no stage after reads them: geometry and hull shaders reading clip
            // distances are refused at their own conversion, and only the last vertex stage's clip
            // and cull.
            TypeInner::Array {
                size: naga::ArraySize::Constant(count),
                ..
            } if matches!(
                binding,
                Binding::BuiltIn(BuiltIn::ClipDistances | BuiltIn::CullDistance)
            ) =>
            {
                let found = Some((member, count.get()));
                match binding {
                    Binding::BuiltIn(BuiltIn::ClipDistances) => clip = found,
                    _ => cull = found,
                }
                return Ok(());
            }
            _ => return Err(ConvertError::Unsupported(VERTEX_OUTPUT_SHAPE)),
        };
        match binding {
            Binding::BuiltIn(BuiltIn::Position { .. }) => position = Some(member),
            Binding::Location { location, .. } => {
                if let Some(i) = locations.iter().position(|l| l == location) {
                    slots[i] = Some((member.unwrap_or(0), kind, size));
                }
            }
            _ => return Err(ConvertError::Unsupported(VERTEX_OUTPUT_FOR_GEOMETRY)),
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
                    .ok_or(ConvertError::Malformed(UNBOUND_OUTPUT.into()))?;
                place(binding, Some(i as u32), &module.types[m.ty].inner)?;
            }
        }
        _ => return Err(ConvertError::Malformed(VERTEX_OUTPUTS.into())),
    }
    let position = position.ok_or(ConvertError::Unsupported(VERTEX_WITHOUT_POSITION))?;
    inner.name = Some("vertex_shader".into());
    let callee = module.functions.append(inner, Span::UNDEFINED);

    let values = values(&mut module);
    let u32_ty = build::scalar(&mut module, Scalar::U32);
    let words = build::runtime_array(&mut module, u32_ty, build::WORD);
    let indices = build::global(
        &mut module,
        INDICES_VARIABLE,
        AddressSpace::Storage {
            access: StorageAccess::LOAD,
        },
        Some(binding(INDICES_BINDING)),
        words,
    );
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let records = build::runtime_array(&mut module, uvec4, RECORD_SLOT_BYTES);
    let vertices = build::global(
        &mut module,
        VERTICES_VARIABLE,
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
        name: Some(ID_ARGUMENT.into()),
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
    let (clip, cull) = if distances {
        (clip, cull)
    } else {
        (None, None)
    };
    let distance_count = |d: Option<(Option<u32>, u32)>| d.map_or(0, |(_, n)| distance_slots(n));
    let slot_count = locations.len() as u32 + distance_count(clip) + distance_count(cull);
    let per = b.u32(slot_count + 1);
    let before = b.mul(instance, count);
    let r = b.add(before, x);
    let record = b.mul(r, per);
    // An indexed strip made a list: the places past its listed indices hold no vertex.
    let listed_total = value(&mut b, v, word::LISTED_TOTAL);
    let zero = b.u32(0);
    let capped = b.binary(BinaryOperator::NotEqual, listed_total, zero);
    let total = {
        let p = b.global(indices);
        let p = b.at(p, zero);
        b.load(p)
    };
    let beyond = b.binary(BinaryOperator::GreaterEqual, x, total);
    let none = b.binary(BinaryOperator::LogicalAnd, capped, beyond);
    b.when(none, |b| {
        for slot in 0..=slot_count {
            let value = if slot == 0 {
                let zero = b.u32(0);
                let w = b.u32(OUTSIDE_W.to_bits());
                b.compose(uvec4, alloc::vec![zero, zero, zero, w])
            } else {
                b.expr(naga::Expression::ZeroValue(uvec4))
            };
            let s = b.u32(slot);
            let at = b.add(record, s);
            let p = b.global(vertices);
            let p = b.at(p, at);
            b.store(p, value);
        }
        b.ret(None);
    });
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
        let eight = b.u32(u8::BITS);
        let shift = b.mul(half, eight);
        let shifted = b.binary(BinaryOperator::ShiftRight, word, shift);
        let mask = b.u32(u32::from(u16::MAX));
        let short = b.binary(BinaryOperator::And, shifted, mask);
        let four = b.u32(build::WORD);
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
    let write = |b: &mut Builder, slot: u32, value: Option<(Option<u32>, ScalarKind, u32)>| {
        let value = match value {
            Some((member, kind, size)) => {
                let v = match member {
                    Some(m) => b.field(out, m),
                    None => out,
                };
                as_slot(b, uvec4, v, kind, size)
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
    // The distances, four a slot, the last slot's unused components zero.
    let mut next = 1 + locations.len() as u32;
    for (member, count) in [clip, cull].into_iter().flatten() {
        let array = match member {
            Some(m) => b.field(out, m),
            None => out,
        };
        for slot in 0..distance_slots(count) {
            let zero = b.u32(0);
            let parts = (0..build::VEC4)
                .map(|c| {
                    let index = slot * build::VEC4 + c;
                    if index < count {
                        let d = b.field(array, index);
                        b.bitcast(d, ScalarKind::Uint)
                    } else {
                        zero
                    }
                })
                .collect();
            let value = b.compose(uvec4, parts);
            let s = b.u32(next);
            let at = b.add(record, s);
            let p = b.global(vertices);
            let p = b.at(p, at);
            b.store(p, value);
            next += 1;
        }
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
                        .ok_or(ConvertError::Malformed(UNBOUND_OUTPUT.into()))?;
                    outputs.push((m.ty, b, Some(i as u32)));
                }
            }
            _ => return Err(ConvertError::Malformed(VERTEX_OUTPUTS.into())),
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
    let span = RECORD_SLOT_BYTES * members.len() as u32;
    let ty = module.types.insert(
        Type {
            name: Some(POSITIONED_OUTPUTS_TYPE.into()),
            inner: TypeInner::Struct {
                members: members
                    .into_iter()
                    .enumerate()
                    .map(|(i, (ty, binding))| StructMember {
                        name: Some(format!("v{i}")),
                        ty,
                        binding: Some(binding),
                        offset: RECORD_SLOT_BYTES * i as u32,
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

/// The clip distances a stage's outputs hold, counted from their components.
pub(crate) fn clip_distances(reflection: &Reflection) -> u32 {
    reflection
        .outputs
        .iter()
        .filter(|v| v.system == Some(wcontainers_gpu::SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones())
        .sum()
}

/// The record slots holding `distances` clip or cull distances, four a slot.
pub(crate) fn distance_slots(distances: u32) -> u32 {
    distances.div_ceil(build::VEC4)
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
        offset: build::WORD,
    },
];

/// The helper constant the pass-through made with `VariantFlags::LAYER_FROM_STAGE` reads beside
/// [`PASSTHROUGH_HELPERS`].
pub const PASSTHROUGH_LAYER_COUNT: wcontainers_gpu::HelperSlot = wcontainers_gpu::HelperSlot {
    value: wcontainers_gpu::HelperValue::LayerCount,
    offset: build::WORD * LAYER_COUNT_MEMBER,
};

/// Where a pixel program made with `VariantFlags::LAYER_FROM_STAGE` reads the layer its primitive
/// names: the location after the values `linked` holds.
pub(crate) fn named_layer_location(variant: &VariantKey) -> u32 {
    let distances = match variant.flags.contains(VariantFlags::CLIP_DISTANCE) {
        true => u32::from(variant.linked.clip_distances),
        false => 0,
    };
    let clamp = u32::from(variant.flags.contains(VariantFlags::DEPTH_CLAMP));
    variant.linked.location + (distances + clamp).div_ceil(build::VEC4)
}

/// The vertex program of the draw after a geometry stage: each vertex attribute is a four-word slot of
/// the geometry output's record (location 0 the position, 1 + i the stage's i-th output location),
/// passed on with the type and interpolation the geometry shader's reflection gives. When the stage
/// writes a layer or viewport, the record's last slot holds them and the program keeps only the
/// vertices of the run its helper constants name ([`PASSTHROUGH_HELPERS`], at the vertex stage's
/// helper binding), placing the others outside every clip volume. With `named_layer`, the location
/// it passes the named layer at, a vertex naming a layer at or past [`PASSTHROUGH_LAYER_COUNT`] is
/// kept in layer 0's run.
pub(crate) fn passthrough(reflection: &Reflection, named_layer: Option<u32>) -> Result<Module> {
    let passed: Vec<(ScalarKind, Option<VectorSize>, Binding)> = locations(&reflection.outputs)
        .into_iter()
        .map(|loc| {
            // The drawn stream's value where several streams place one there.
            let value = reflection
                .outputs
                .iter()
                .filter(|v| v.system.is_none() && v.location == loc)
                .min_by_key(|v| v.stream)
                .expect("a listed location");
            let kind = match value.scalar {
                ValueKind::Float => ScalarKind::Float,
                ValueKind::Sint => ScalarKind::Sint,
                ValueKind::Uint => ScalarKind::Uint,
            };
            let interpolation = match kind {
                ScalarKind::Float => value.interpolation,
                _ => Interpolation::Flat,
            };
            (
                kind,
                Some(VectorSize::Quad),
                location(loc, Some(interpolation)),
            )
        })
        .collect();
    let selects = selects(reflection);
    passthrough_of(
        &passed,
        selects,
        clip_distances(reflection),
        named_layer.filter(|_| selects),
        None,
    )
}

/// The pass-through vertex program reading records of a position slot, a slot for each of `passed`
/// (written with its kind, its width's first components and its binding), the layer and viewport slot when `selects`, then the slots of
/// `distances` clip distances, and the named layer as a flat value at `named_layer`; then the `extra`
/// slot, passed on at its location.
fn passthrough_of(
    passed: &[(ScalarKind, Option<VectorSize>, Binding)],
    selects: bool,
    distances: u32,
    named_layer: Option<u32>,
    extra: Option<ExtraSlot>,
) -> Result<Module> {
    let mut module = Module::default();
    let uvec4 = build::vec4(&mut module, Scalar::U32);
    let mut b = Builder::new(crate::ENTRY_POINT);
    let mut members = Vec::new();
    let mut components = Vec::new();
    for slot in 0..=passed.len() {
        b.f.arguments.push(FunctionArgument {
            name: Some(format!("slot{slot}")),
            ty: uvec4,
            binding: Some(location(slot as u32, None)),
        });
        let raw = b.argument(slot as u32);
        let (kind, size, binding) = match slot {
            0 => (
                ScalarKind::Float,
                Some(VectorSize::Quad),
                Binding::BuiltIn(BuiltIn::Position { invariant: false }),
            ),
            _ => passed[slot - 1].clone(),
        };
        let scalar = Scalar {
            kind,
            width: Scalar::U32.width,
        };
        let inner = match size {
            Some(size) => TypeInner::Vector { size, scalar },
            None => TypeInner::Scalar(scalar),
        };
        let ty = module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED);
        let v = if kind == ScalarKind::Uint {
            raw
        } else {
            b.bitcast(raw, kind)
        };
        let v = match size {
            Some(VectorSize::Quad) => v,
            Some(size) => b.expr(naga::Expression::Swizzle {
                size,
                vector: v,
                pattern: [
                    naga::SwizzleComponent::X,
                    naga::SwizzleComponent::Y,
                    naga::SwizzleComponent::Z,
                    naga::SwizzleComponent::W,
                ],
            }),
            None => b.field(v, 0),
        };
        members.push(StructMember {
            name: Some(format!("v{slot}")),
            ty,
            binding: Some(binding),
            offset: RECORD_SLOT_BYTES * slot as u32,
        });
        components.push(v);
    }
    let mut named_layer_value = None;
    if selects {
        let slot = 1 + passed.len() as u32;
        b.f.arguments.push(FunctionArgument {
            name: Some(SELECTION_ARGUMENT.into()),
            ty: uvec4,
            binding: Some(location(slot, None)),
        });
        let selection = b.argument(slot);
        let u32_ty = build::scalar(&mut module, Scalar::U32);
        let helpers_ty = module.types.insert(
            Type {
                name: Some(RUN_TYPE.into()),
                inner: TypeInner::Struct {
                    members: RUN_MEMBERS
                        .iter()
                        .enumerate()
                        .map(|(i, name)| StructMember {
                            name: Some((*name).into()),
                            ty: u32_ty,
                            binding: None,
                            offset: build::WORD * i as u32,
                        })
                        .collect(),
                    span: RECORD_SLOT_BYTES,
                },
            },
            Span::UNDEFINED,
        );
        let helpers = build::global(
            &mut module,
            RUN_VARIABLE,
            AddressSpace::Uniform,
            Some(ResourceBinding {
                group: wcontainers_gpu::HELPER_SET,
                binding: wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Vertex),
            }),
            helpers_ty,
        );
        let run = b.global(helpers);
        let mut matches = Vec::new();
        for i in 0..PASSTHROUGH_HELPERS.len() as u32 {
            let p = b.field(run, i);
            let want = b.load(p);
            let have = b.field(selection, i);
            matches.push(b.binary(BinaryOperator::Equal, have, want));
        }
        if let Some(at) = named_layer {
            // Direct3D draws a primitive naming a layer past the views' into the first layer.
            let p = b.field(run, LAYER_COUNT_MEMBER);
            let count = b.load(p);
            let named = b.field(selection, LAYER_MEMBER);
            let past = b.binary(BinaryOperator::GreaterEqual, named, count);
            let p = b.field(run, LAYER_MEMBER);
            let run_layer = b.load(p);
            let zero = b.expr(naga::Expression::Literal(naga::Literal::U32(0)));
            let first = b.binary(BinaryOperator::Equal, run_layer, zero);
            let into_first = b.binary(BinaryOperator::LogicalAnd, past, first);
            let layer = LAYER_MEMBER as usize;
            matches[layer] = b.binary(BinaryOperator::LogicalOr, matches[layer], into_first);
            named_layer_value = Some((at, named));
        }
        let keep = matches
            .into_iter()
            .reduce(|k, same| b.binary(BinaryOperator::LogicalAnd, k, same));
        let vec4 = build::vec4(&mut module, Scalar::F32);
        let lit =
            |b: &mut Builder, x: f32| b.expr(naga::Expression::Literal(naga::Literal::F32(x)));
        let zero = lit(&mut b, 0.0);
        let below = lit(&mut b, -1.0);
        let outside = b.compose(vec4, alloc::vec![zero, zero, zero, below]);
        components[0] = b.select(keep.unwrap(), components[0], outside);
    }
    let mut span = RECORD_SLOT_BYTES * (1 + passed.len() as u32);
    if distances > 0 {
        let first = 1 + passed.len() as u32 + u32::from(selects);
        let mut parts = Vec::new();
        for k in 0..distance_slots(distances) {
            b.f.arguments.push(FunctionArgument {
                name: Some(format!("clip{k}")),
                ty: uvec4,
                binding: Some(location(first + k, None)),
            });
            let raw = b.argument(b.f.arguments.len() as u32 - 1);
            let slot = b.bitcast(raw, ScalarKind::Float);
            for c in 0..build::VEC4 {
                if (parts.len() as u32) < distances {
                    parts.push(b.field(slot, c));
                }
            }
        }
        let f32_ty = build::scalar(&mut module, Scalar::F32);
        let array = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Array {
                    base: f32_ty,
                    size: naga::ArraySize::Constant(
                        core::num::NonZeroU32::new(distances).expect("some distances"),
                    ),
                    stride: build::WORD,
                },
            },
            Span::UNDEFINED,
        );
        members.push(StructMember {
            name: None,
            ty: array,
            binding: Some(Binding::BuiltIn(BuiltIn::ClipDistances)),
            offset: span,
        });
        components.push(b.compose(array, parts));
        span += distance_slots(distances) * RECORD_SLOT_BYTES;
    }
    if let Some(extra) = extra {
        let slot = 1 + passed.len() as u32 + distance_slots(distances);
        let name = match extra {
            ExtraSlot::LineCoverage(_) => COVERAGE_ARGUMENT,
            ExtraSlot::PrimitiveId(_) => PRIMITIVE_ARGUMENT,
        };
        b.f.arguments.push(FunctionArgument {
            name: Some(name.into()),
            ty: uvec4,
            binding: Some(location(slot, None)),
        });
        let raw = b.argument(b.f.arguments.len() as u32 - 1);
        let (ty, binding, value, bytes) = match extra {
            ExtraSlot::LineCoverage(at) => (
                build::vec4(&mut module, Scalar::F32),
                location(at, Some(Interpolation::Linear)),
                b.bitcast(raw, ScalarKind::Float),
                RECORD_SLOT_BYTES,
            ),
            ExtraSlot::PrimitiveId(at) => (
                uvec4,
                location(at, Some(Interpolation::Flat)),
                raw,
                RECORD_SLOT_BYTES,
            ),
        };
        members.push(StructMember {
            name: Some(name.into()),
            ty,
            binding: Some(binding),
            offset: span,
        });
        components.push(value);
        span += bytes;
    }
    if let Some((at, named)) = named_layer_value {
        members.push(StructMember {
            name: Some(NAMED_LAYER_ARGUMENT.into()),
            ty: build::scalar(&mut module, Scalar::U32),
            binding: Some(location(at, Some(Interpolation::Flat))),
            offset: span,
        });
        components.push(named);
        span += build::WORD;
    }
    let ty = module.types.insert(
        Type {
            name: Some(OUTPUTS_TYPE.into()),
            inner: TypeInner::Struct { members, span },
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
        workgroup_size: Default::default(),
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
