//! DXIL geometry shaders. dxil-spirv writes the Geometry execution model, which naga's reader lacks,
//! so the SPIR-V is made a compute program first: its stage variables private, its emits and cuts
//! calls of empty functions. The geometry pass the DXBC route uses then fills those functions and
//! builds the compute or replay form around the program's own function.

use super::staged::{self, Entry, Ids, Interface, Privatise};
use crate::container::Element;
use crate::ir::{builtin, primitive};
use crate::lower::geometry::{
    CUT, EMIT, Geometry, GeometryPass, GeometryValue, STREAM_PAST_DIRECT3D, Side,
};
use crate::{ConvertError, stage};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{Module, Statement};
use rspirv::dr::{self, Instruction};
use spirv::{BuiltIn, ExecutionMode, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::{StageExtras, StageValue, VariantKey};

/// The prefix of the names given to the stage values' variables.
const VALUE_NAME: &str = "gs_value";
/// `OpEmitStreamVertex`'s and `OpEndStreamPrimitive`'s stream.
const STREAM_OPERAND: usize = 0;

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
/// declared; `None` for any other execution model. A patch input, which no execution mode names, is
/// the container's `dxil`.
pub(crate) fn prepare(
    spirv: &mut dr::Module,
    dxil: &[u8],
) -> Result<Option<Declared>, ConvertError> {
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
        if let Some(p) = input_primitive(mode) {
            declared.input = p;
        }
        if let Some(p) = output_primitive(mode) {
            declared.output = p;
        }
    }
    if let Some(p) = crate::container::geometry_input_primitive(dxil)
        .and_then(|p| p.checked_sub(crate::container::FIRST_PATCH_PRIMITIVE))
    {
        declared.input = primitive::PATCH + 1 + u64::from(p);
    }
    let within = |stream: u32| match (stream as usize) < stage::STREAMS {
        true => Ok(stream),
        false => Err(ConvertError::Malformed(STREAM_PAST_DIRECT3D.into())),
    };
    for v in &declared.entry.values {
        if v.side == Side::Output {
            declared.stream_mask |= 1 << within(v.stream)?;
        }
    }
    // Streams emitted into, which the declarations may leave undecorated.
    let stream = |i: &Instruction| {
        staged::id(i, STREAM_OPERAND)
            .and_then(|s| declared.entry.constant(s))
            .ok_or_else(|| ConvertError::Malformed(STREAM_NOT_CONSTANT.into()))
            .and_then(within)
    };
    for i in spirv.all_inst_iter() {
        if matches!(
            i.class.opcode,
            Op::EmitStreamVertex | Op::EndStreamPrimitive
        ) {
            declared.stream_mask |= 1 << stream(i)?;
        }
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
    let calls = |i: &Instruction| -> Result<Option<u32>, ConvertError> {
        Ok(match i.class.opcode {
            Op::EmitVertex => Some(emits[0]),
            Op::EmitStreamVertex => Some(emits[stream(i)? as usize]),
            Op::EndPrimitive | Op::EndStreamPrimitive => Some(cut),
            _ => None,
        })
    };
    let values: Vec<&Interface> = declared.entry.values.iter().collect();
    staged::privatise(
        spirv,
        ids,
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
        (Some(BuiltIn::PrimitiveId), _) => Ok(builtin::PRIMITIVE_ID),
        (Some(BuiltIn::InvocationId), Side::Input) => Ok(builtin::GS_INSTANCE_ID),
        (Some(BuiltIn::Layer), Side::Output) => Ok(builtin::LAYER_INDEX),
        (Some(BuiltIn::ViewportIndex), Side::Output) => Ok(builtin::VIEWPORT_INDEX),
        (Some(BuiltIn::ClipDistance), _) => Ok(builtin::CLIP_DISTANCE),
        (Some(BuiltIn::CullDistance), _) => Ok(builtin::CULL_DISTANCE),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rspirv::dr::Operand;
    use spirv::Decoration;

    /// A stream number past Direct3D's, emitted into or declared, is refused by name before any
    /// stream mask is made of it: dxc's build of `gs_rows_streams` with its emitted stream's constant,
    /// then its outputs' stream decorations, set to the first stream past Direct3D's and to the
    /// mask's width.
    #[test]
    fn a_stream_past_direct3d_is_refused_by_name() {
        let dxil = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/own/gs_rows_streams.dxil"
        ));
        let (words, _) = super::super::to_spirv(dxil, 0).unwrap();
        let spirv = staged::load(&words).unwrap();
        let emitted: Vec<u32> = spirv
            .all_inst_iter()
            .filter(|i| i.class.opcode == Op::EmitStreamVertex)
            .filter_map(|i| staged::id(i, STREAM_OPERAND))
            .collect();
        let stream = |i: &Instruction| staged::decorates(i, Decoration::Stream);
        assert!(!emitted.is_empty() && spirv.annotations.iter().any(stream));
        for past in [stage::STREAMS as u32, u64::BITS] {
            // The constants of the emitted streams.
            let mut emits = spirv.clone();
            for i in &mut emits.types_global_values {
                if i.class.opcode == Op::Constant
                    && i.result_id.is_some_and(|r| emitted.contains(&r))
                {
                    i.operands[0] = Operand::LiteralBit32(past);
                }
            }
            let mut declares = spirv.clone();
            for i in declares.annotations.iter_mut().filter(|i| stream(i)) {
                i.operands[staged::DECORATION_LITERAL] = Operand::LiteralBit32(past);
            }
            for mut module in [emits, declares] {
                match prepare(&mut module, dxil) {
                    Err(ConvertError::Malformed(why)) => assert_eq!(why, STREAM_PAST_DIRECT3D),
                    other => panic!("{past}: {:?}", other.map(|d| d.map(|d| d.stream_mask))),
                }
            }
        }
    }
}
