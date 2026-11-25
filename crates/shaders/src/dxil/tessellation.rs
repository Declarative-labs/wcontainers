//! DXIL hull and domain shaders. dxil-spirv writes the tessellation execution models, which naga's
//! reader lacks, so the SPIR-V is made a compute program (a hull shader) or a vertex program (a domain
//! shader) first, with the stage values the passes of `crate::lower::hull` fill or read made private.
//! Those passes, which the DXBC route uses, then build the hull's compute form around its control
//! point and patch constant functions, and the domain program's inputs.

use super::staged::{self, Entry, Ids, Interface, Privatise};
use crate::ConvertError;
use crate::container::Element;
use crate::ir::builtin;
use crate::lower::geometry::Side;
use crate::lower::hull::{
    self, DOMAIN_ISOLINE, DOMAIN_QUAD, DOMAIN_TRIANGLE, DomainPass, HullPass, OUTPUT_CLOCKWISE,
    OUTPUT_COUNTER_CLOCKWISE, PARTITIONING_FRACTIONAL_EVEN, PARTITIONING_FRACTIONAL_ODD,
    PARTITIONING_INTEGER, Tessellation, TessellationValue,
};
use crate::stage::tess;
use alloc::vec::Vec;
use naga::Module;
use rspirv::dr::{self, Operand};
use spirv::{BuiltIn, ExecutionMode, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::{SourceStage, StageExtras, StageValue};

/// The prefix of the names given to the stage values' variables.
const VALUE_NAME: &str = "ts_value";
/// dxil-spirv's names of a hull shader's control point function and patch constant function.
const CONTROL_POINT_FUNCTION: &str = "hull_main";
const PATCH_CONSTANT_FUNCTION: &str = "patch_main";

/// Why a DXIL hull or domain shader is malformed.
const NO_DOMAIN: &str = "a tessellation stage without its domain";
const NO_OUTPUT: &str = "a hull shader without its partitioning or output primitive";
const NO_INPUT_POINTS: &str = "a tessellation stage without its input control point count";

/// `OpTypeArray`'s length, after its element type.
const ARRAY_LENGTH: usize = 1;
/// The pointer an access chain starts from and a load reads.
const READ_POINTER: usize = 0;
/// The pointer a store or copy writes, and the one a copy reads.
const WRITTEN_POINTER: usize = 0;
const COPIED_POINTER: usize = WRITTEN_POINTER + 1;
/// Where an instruction a function runs names a pointer it reads or writes through.
const POINTER_OPERANDS: [(Op, &[usize]); 6] = [
    (Op::AccessChain, &[READ_POINTER]),
    (Op::InBoundsAccessChain, &[READ_POINTER]),
    (Op::PtrAccessChain, &[READ_POINTER]),
    (Op::Load, &[READ_POINTER]),
    (Op::Store, &[WRITTEN_POINTER]),
    (Op::CopyMemory, &[WRITTEN_POINTER, COPIED_POINTER]),
];
/// `OpFunctionCall`'s first argument, after the function.
const CALL_ARGUMENTS: usize = 1;

/// What a hull or domain shader declares, read from its SPIR-V before the reader takes it.
pub(crate) struct Declared {
    stage: SourceStage,
    tessellation: Tessellation,
    entry: Entry,
}

fn domain(entry: &Entry) -> Result<u8, ConvertError> {
    [
        (ExecutionMode::Triangles, DOMAIN_TRIANGLE),
        (ExecutionMode::Quads, DOMAIN_QUAD),
        (ExecutionMode::Isolines, DOMAIN_ISOLINE),
    ]
    .into_iter()
    .find(|(m, _)| entry.has_mode(*m))
    .map(|(_, d)| d)
    .ok_or_else(|| ConvertError::Malformed(NO_DOMAIN.into()))
}

/// The hull shader's partitioning and output primitive. dxil-spirv writes Direct3D's pow2
/// partitioning as integer partitioning, as dxbc-spirv does for the DXBC route.
fn hull_output(entry: &Entry, domain: u8) -> Result<(u8, u8), ConvertError> {
    let partitioning = [
        (ExecutionMode::SpacingEqual, PARTITIONING_INTEGER),
        (
            ExecutionMode::SpacingFractionalOdd,
            PARTITIONING_FRACTIONAL_ODD,
        ),
        (
            ExecutionMode::SpacingFractionalEven,
            PARTITIONING_FRACTIONAL_EVEN,
        ),
    ]
    .into_iter()
    .find(|(m, _)| entry.has_mode(*m))
    .map(|(_, p)| p);
    let output = if entry.has_mode(ExecutionMode::PointMode) {
        Some(tess::OUTPUT_POINT)
    } else if entry.has_mode(ExecutionMode::VertexOrderCw) {
        Some(OUTPUT_CLOCKWISE)
    } else if entry.has_mode(ExecutionMode::VertexOrderCcw) {
        Some(OUTPUT_COUNTER_CLOCKWISE)
    } else {
        (domain == DOMAIN_ISOLINE).then_some(tess::OUTPUT_LINE)
    };
    partitioning
        .zip(output)
        .ok_or_else(|| ConvertError::Malformed(NO_OUTPUT.into()))
}

/// The values a stage makes private: all of a hull shader's, a domain shader's inputs.
fn staged_values(stage: SourceStage, entry: &Entry) -> Vec<&Interface> {
    entry
        .values
        .iter()
        .filter(|v| stage == SourceStage::Hull || v.side == Side::Input)
        .collect()
}

/// Makes a hull shader's SPIR-V a compute program and a domain shader's a vertex program naga's reader
/// takes, and returns what it declared; `None` for any other execution model. `dxil` is the container.
pub(crate) fn prepare(
    spirv: &mut dr::Module,
    dxil: &[u8],
) -> Result<Option<Declared>, ConvertError> {
    let (stage, entry) = match staged::entry(spirv, ExecutionModel::TessellationControl) {
        Some(e) => (SourceStage::Hull, e),
        None => match staged::entry(spirv, ExecutionModel::TessellationEvaluation) {
            Some(e) => (SourceStage::Domain, e),
            None => return Ok(None),
        },
    };
    let domain = domain(&entry)?;
    let points_in = crate::container::patch_input_points(dxil)
        .ok_or_else(|| ConvertError::Malformed(NO_INPUT_POINTS.into()))?;
    let entry = match stage == SourceStage::Domain && points_in == 0 {
        true => {
            drop_arrays_of_none(spirv, &entry)?;
            staged::entry(spirv, ExecutionModel::TessellationEvaluation)
                .ok_or_else(|| ConvertError::Malformed(staged::NO_MAIN.into()))?
        }
        false => entry,
    };
    let mut tessellation = Tessellation {
        domain,
        points_in,
        ..Tessellation::default()
    };
    let (model, classes) = match stage {
        SourceStage::Hull => {
            (tessellation.partitioning, tessellation.output) = hull_output(&entry, domain)?;
            tessellation.points_out = entry
                .mode_literal(ExecutionMode::OutputVertices)
                .unwrap_or(0);
            (
                ExecutionModel::GLCompute,
                &[StorageClass::Input, StorageClass::Output][..],
            )
        }
        _ => (ExecutionModel::Vertex, &[StorageClass::Input][..]),
    };
    staged::privatise(
        spirv,
        Ids::of(spirv),
        &Privatise {
            entry: &entry,
            values: &staged_values(stage, &entry),
            prefix: VALUE_NAME,
            model,
            classes,
            stand_ins: &[],
            calls: &|_| Ok(None),
        },
    )?;
    Ok(Some(Declared {
        stage,
        tessellation,
        entry,
    }))
}

/// A domain shader's control point inputs of a patch of none: dxil-spirv declares them input arrays of
/// none, which naga's reader refuses. They are dropped (variables, their types, names and decorations)
/// where no function reads them, and refused by name otherwise.
fn drop_arrays_of_none(spirv: &mut dr::Module, entry: &Entry) -> Result<(), ConvertError> {
    let (mut arrays, mut pointers, mut variables) = (Vec::new(), Vec::new(), Vec::new());
    for i in &spirv.types_global_values {
        let Some(result) = i.result_id else {
            continue;
        };
        match i.class.opcode {
            Op::TypeArray
                if staged::id(i, ARRAY_LENGTH).and_then(|n| entry.constant(n)) == Some(0) =>
            {
                arrays.push(result)
            }
            Op::TypePointer
                if i.operands.get(staged::STORAGE_CLASS)
                    == Some(&Operand::StorageClass(StorageClass::Input))
                    && staged::id(i, staged::POINTEE).is_some_and(|t| arrays.contains(&t)) =>
            {
                pointers.push(result)
            }
            Op::Variable if i.result_type.is_some_and(|t| pointers.contains(&t)) => {
                variables.push(result)
            }
            _ => {}
        }
    }
    if variables.is_empty() {
        return Ok(());
    }
    // Whether an instruction reads or writes through one of the variables.
    let reaches = |i: &dr::Instruction| {
        let op = i.class.opcode;
        let at: Vec<usize> = match op {
            Op::FunctionCall => (CALL_ARGUMENTS..i.operands.len()).collect(),
            _ => POINTER_OPERANDS
                .iter()
                .find(|(o, _)| *o == op)
                .map_or(Vec::new(), |(_, w)| w.to_vec()),
        };
        at.into_iter()
            .any(|w| staged::id(i, w).is_some_and(|p| variables.contains(&p)))
    };
    if spirv
        .functions
        .iter()
        .flat_map(|f| f.all_inst_iter())
        .any(reaches)
    {
        return Err(ConvertError::Unsupported(
            crate::refusal::DOMAIN_WITHOUT_CONTROL_POINTS,
        ));
    }
    let dropped =
        |id: u32| arrays.contains(&id) || pointers.contains(&id) || variables.contains(&id);
    staged::forget(spirv, &dropped);
    spirv
        .types_global_values
        .retain(|i| !i.result_id.is_some_and(dropped));
    Ok(())
}

/// A SPIR-V built-in as `ir::builtin` numbers it, or the refusal naming it.
fn builtin_of(b: u32, side: Side, stage: SourceStage) -> Result<u64, ConvertError> {
    let found = match (BuiltIn::from_u32(b), side) {
        (Some(BuiltIn::Position), Side::Input) => Some(builtin::POSITION),
        (Some(BuiltIn::PrimitiveId), Side::Input) => Some(builtin::PRIMITIVE_ID),
        (Some(BuiltIn::InvocationId), Side::Input) => Some(builtin::TESS_CONTROL_POINT_ID),
        (Some(BuiltIn::PatchVertices), Side::Input) => Some(builtin::TESS_CONTROL_POINT_COUNT_IN),
        (Some(BuiltIn::TessCoord), Side::Input) => Some(builtin::TESS_COORD),
        (Some(BuiltIn::TessLevelOuter), _) => Some(builtin::TESS_FACTOR_OUTER),
        (Some(BuiltIn::TessLevelInner), _) => Some(builtin::TESS_FACTOR_INNER),
        _ => None,
    };
    found.ok_or(ConvertError::Unsupported(match (stage, side) {
        (SourceStage::Hull, Side::Input) => crate::refusal::HULL_INPUT,
        (SourceStage::Hull, Side::Output) => crate::refusal::HULL_OUTPUT,
        _ => crate::refusal::DOMAIN_INPUT,
    }))
}

/// The stage values as the passes take them.
fn values(module: &Module, declared: &Declared) -> Result<Vec<TessellationValue>, ConvertError> {
    let shape = match declared.stage {
        SourceStage::Hull => crate::refusal::HULL_INPUT_SHAPE,
        _ => crate::refusal::DOMAIN_INPUT_SHAPE,
    };
    let mut values = Vec::new();
    for (k, v) in staged_values(declared.stage, &declared.entry)
        .into_iter()
        .enumerate()
    {
        let var = staged::value_variable(module, VALUE_NAME, k)?;
        let builtin = v
            .builtin
            .map(|b| builtin_of(b, v.side, declared.stage))
            .transpose()?;
        let location = v.location.map(staged::location);
        values.push(TessellationValue {
            var,
            side: v.side,
            location,
            builtin,
            per_point: !v.patch && (location.is_some() || builtin == Some(builtin::POSITION)),
            ty: staged::ir_ty(module, module.global_variables[var].ty, shape)?,
        });
    }
    Ok(values)
}

/// Builds the hull or domain shader's form in `module` as read from the SPIR-V [`prepare`] made: a
/// hull shader's compute entry point replacing the reader's, a domain program's inputs filled from its
/// vertex number before its function runs. Returns the stage, its extras and its input values, placed
/// by the container's signatures.
pub(crate) fn finish(
    module: &mut Module,
    declared: &Declared,
    signatures: [&[Element]; 2],
) -> Result<(SourceStage, StageExtras, Vec<StageValue>), ConvertError> {
    let tessellation = declared.tessellation;
    if declared.stage == SourceStage::Hull {
        let values = values(module, declared)?;
        let main = staged::function(module, CONTROL_POINT_FUNCTION)?;
        let constants = staged::function(module, PATCH_CONSTANT_FUNCTION)?;
        module.entry_points.clear();
        let mut pass = HullPass {
            module,
            tessellation,
            values: &values,
            reflected: Vec::new(),
        };
        let extras = pass.entry_point(main, Some(constants))?;
        let [inputs, _] = staged::place(pass.reflected, |_| 0, signatures);
        return Ok((SourceStage::Hull, extras, inputs));
    }
    let values = values(module, declared)?;
    let (function, extras) = DomainPass {
        module,
        tessellation,
        values: &values,
    }
    .inputs()?;
    hull::call_domain_inputs(module, function)?;
    Ok((SourceStage::Domain, extras, Vec::new()))
}
