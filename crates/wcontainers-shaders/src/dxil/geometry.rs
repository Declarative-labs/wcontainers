//! DXIL geometry shaders. dxil-spirv writes the Geometry execution model, which naga's reader lacks,
//! so the SPIR-V is made a compute program first: its stage variables private, its emits and cuts
//! calls of empty functions. The geometry pass the DXBC route uses then fills those functions and
//! builds the compute or replay form around the program's own function.

use super::staged::{self, Entry, Ids, Interface, Privatise};
use crate::container::Element;
use crate::ir::{builtin, primitive};
use crate::lower::geometry::{CUT, EMIT, Geometry, GeometryPass, GeometryValue, Side};
use crate::{ConvertError, stage};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{Module, Statement};
use spirv::{BuiltIn, ExecutionMode, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::{StageExtras, StageValue, VariantKey};

/// The prefix of the names given to the stage values' variables.
const VALUE_NAME: &str = "gs_value";
/// `OpEmitStreamVertex`'s and `OpEndStreamPrimitive`'s stream.
const STREAM_OPERAND: usize = 1;

/// Why a DXIL geometry shader is malformed.
const STREAM_NOT_CONSTANT: &str = "a geometry stream that is no constant";
const LOCATION_BEFORE_STREAM: &str = "an output located before its stream's first row";

/// What a geometry shader declares, read from its SPIR-V before the reader takes it.
pub(crate) struct Declared {
    input: u64,
    output: u64,
    max_vertices: u32,
    instances: u32,
    stream_mask: u64,
    entry: Entry,
}

/// The input primitive an execution mode names, as `ir::primitive` numbers it.
fn input_primitive(mode: ExecutionMode) -> Option<u64> {
    Some(match mode {
        ExecutionMode::InputPoints => primitive::POINTS,
        ExecutionMode::InputLines => primitive::LINES,
        ExecutionMode::InputLinesAdjacency => primitive::LINES_ADJ,
        ExecutionMode::Triangles => primitive::TRIANGLES,
        ExecutionMode::InputTrianglesAdjacency => primitive::TRIANGLES_ADJ,
        _ => return None,
    })
}

fn output_primitive(mode: ExecutionMode) -> Option<u64> {
    Some(match mode {
        ExecutionMode::OutputPoints => primitive::POINTS,
        ExecutionMode::OutputLineStrip => primitive::LINES,
        ExecutionMode::OutputTriangleStrip => primitive::TRIANGLES,
        _ => return None,
    })
}

/// Makes a Geometry entry point's SPIR-V a compute program naga's reader takes, and returns what it
/// declared; `None` for any other execution model.
pub(crate) fn prepare(spirv: &mut Vec<u32>) -> Result<Option<Declared>, ConvertError> {
    let Some(entry) = staged::entry(spirv, ExecutionModel::Geometry) else {
        return Ok(None);
    };
    let mut declared = Declared {
        input: 0,
        output: 0,
        max_vertices: entry
            .mode_literal(ExecutionMode::OutputVertices)
            .unwrap_or(0),
        instances: entry.mode_literal(ExecutionMode::Invocations).unwrap_or(1),
        stream_mask: 0,
        entry,
    };
    for &(mode, _) in &declared.entry.modes {
        let Some(mode) = ExecutionMode::from_u32(mode) else {
            continue;
        };
        if let Some(p) = input_primitive(mode) {
            declared.input = p;
        }
        if let Some(p) = output_primitive(mode) {
            declared.output = p;
        }
    }
    for v in &declared.entry.values {
        if v.side == Side::Output {
            declared.stream_mask |= 1 << v.stream;
        }
    }
    // Streams emitted into, which the declarations may leave undecorated.
    let stream = |i: &[u32]| {
        declared
            .entry
            .constant(i[STREAM_OPERAND])
            .ok_or_else(|| ConvertError::Malformed(STREAM_NOT_CONSTANT.into()))
    };
    let mut at = super::words::HEADER;
    while let Some(i) = staged::instruction(spirv, at) {
        if matches!(
            Op::from_u32(staged::opcode(i)),
            Some(Op::EmitStreamVertex | Op::EndStreamPrimitive)
        ) {
            declared.stream_mask |= 1 << stream(i)?;
        }
        at += i.len();
    }
    declared.stream_mask = declared.stream_mask.max(1);
    let streams = u64::BITS - declared.stream_mask.leading_zeros();
    let mut ids = Ids::of(spirv);
    let emits: Vec<u32> = (0..streams).map(|_| ids.next()).collect();
    let cut = ids.next();
    let stand_ins: Vec<(u32, String)> = emits
        .iter()
        .enumerate()
        .map(|(s, &e)| (e, emit_name(s)))
        .chain([(cut, CUT.into())])
        .collect();
    let calls = |i: &[u32]| -> Result<Option<u32>, ConvertError> {
        Ok(match Op::from_u32(staged::opcode(i)) {
            Some(Op::EmitVertex) => Some(emits[0]),
            Some(Op::EmitStreamVertex) => Some(emits[stream(i)? as usize]),
            Some(Op::EndPrimitive | Op::EndStreamPrimitive) => Some(cut),
            _ => None,
        })
    };
    let values: Vec<&Interface> = declared.entry.values.iter().collect();
    staged::privatise(
        spirv,
        &mut ids,
        &Privatise {
            entry: &declared.entry,
            values: &values,
            prefix: VALUE_NAME,
            model: ExecutionModel::GLCompute,
            classes: &[StorageClass::Input, StorageClass::Output],
            stand_ins: &stand_ins,
            calls: &calls,
        },
    )?;
    Ok(Some(declared))
}

fn emit_name(stream: usize) -> String {
    match stream {
        0 => EMIT.into(),
        s => format!("{EMIT}{s}"),
    }
}

/// The location dxil-spirv gives each stream's first register: the rows of every stream before it
/// (`build_geometry_stream_row_offsets`), so that an output's register is its location less its
/// stream's.
fn stream_first_rows(outputs: &[Element]) -> [u32; stage::STREAMS] {
    let mut rows = [0; stage::STREAMS];
    for e in outputs {
        if let (Some(r), Some(end)) = (rows.get_mut(e.stream as usize), e.register.checked_add(1)) {
            *r = (*r).max(end);
        }
    }
    let mut first = [0; stage::STREAMS];
    for s in 1..stage::STREAMS {
        first[s] = first[s - 1] + rows[s - 1];
    }
    first
}

/// A SPIR-V built-in as `ir::builtin` numbers it, or the refusal naming it on `side`.
fn builtin_of(b: u32, side: Side) -> Result<u64, ConvertError> {
    let unsupported = |name| Err(ConvertError::Unsupported(name));
    match (BuiltIn::from_u32(b), side) {
        (Some(BuiltIn::Position), _) => Ok(builtin::POSITION),
        (Some(BuiltIn::PrimitiveId), Side::Input) => Ok(builtin::PRIMITIVE_ID),
        (Some(BuiltIn::InvocationId), Side::Input) => Ok(builtin::GS_INSTANCE_ID),
        (Some(BuiltIn::Layer), Side::Output) => Ok(builtin::LAYER_INDEX),
        (Some(BuiltIn::ViewportIndex), Side::Output) => Ok(builtin::VIEWPORT_INDEX),
        (Some(BuiltIn::ClipDistance), Side::Output) => Ok(builtin::CLIP_DISTANCE),
        (Some(BuiltIn::ClipDistance), Side::Input) => unsupported(crate::refusal::CLIP_DISTANCE),
        (Some(BuiltIn::CullDistance), _) => unsupported(crate::refusal::CULL_DISTANCE),
        (Some(BuiltIn::PrimitiveId), Side::Output) => unsupported(crate::refusal::PRIMITIVE_ID),
        (_, Side::Input) => unsupported(crate::refusal::GEOMETRY_INPUT),
        (_, Side::Output) => unsupported(crate::refusal::GEOMETRY_OUTPUT),
    }
}

/// Builds the geometry shader's form in `module` as read from the SPIR-V [`prepare`] made: the emit
/// and cut functions filled, the reader's compute entry point replaced. Returns the stage's extras and
/// its input and output values, placed by the container's signatures.
pub(crate) fn finish(
    module: &mut Module,
    declared: &Declared,
    variant: &VariantKey,
    signatures: [&[Element]; 2],
) -> Result<(StageExtras, [Vec<StageValue>; 2]), ConvertError> {
    let main = module
        .entry_points
        .first()
        .and_then(|ep| {
            ep.function.body.iter().find_map(|s| match *s {
                Statement::Call { function, .. } => Some(function),
                _ => None,
            })
        })
        .ok_or(ConvertError::Malformed(staged::NO_MAIN.into()))?;
    module.entry_points.clear();
    let first_rows = stream_first_rows(signatures[1]);
    let mut values = Vec::new();
    for (k, v) in declared.entry.values.iter().enumerate() {
        let var = staged::value_variable(module, VALUE_NAME, k)?;
        let builtin = v.builtin.map(|b| builtin_of(b, v.side)).transpose()?;
        let location = v
            .location
            .map(|encoded| {
                let (register, component) = staged::location(encoded);
                let first = match v.side {
                    Side::Input => 0,
                    Side::Output => first_rows.get(v.stream as usize).copied().unwrap_or(0),
                };
                register
                    .checked_sub(first)
                    .map(|register| (register, component))
                    .ok_or_else(|| ConvertError::Malformed(LOCATION_BEFORE_STREAM.into()))
            })
            .transpose()?;
        values.push(GeometryValue {
            var,
            side: v.side,
            location,
            builtin,
            stream: v.stream,
            ty: staged::ir_ty(
                module,
                module.global_variables[var].ty,
                crate::refusal::GEOMETRY_INPUT_SHAPE,
            )?,
        });
    }
    let mut geometry = Geometry::declared(
        declared.input,
        declared.output,
        declared.max_vertices,
        declared.instances,
        declared.stream_mask,
    )?;
    geometry.emit = (0..geometry.streams() as usize)
        .map(|s| staged::function(module, &emit_name(s)))
        .collect::<Result<_, _>>()?;
    geometry.cut = Some(staged::function(module, CUT)?);
    let mut pass = GeometryPass {
        module,
        variant,
        geometry: &mut geometry,
        values: &values,
        reflected: Vec::new(),
    };
    pass.functions()?;
    let extras = pass.entry_point(main)?;
    let placed = staged::place(pass.reflected, |v| values[v].stream, signatures);
    Ok((extras, placed))
}
