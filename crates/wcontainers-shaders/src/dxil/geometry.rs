//! DXIL geometry shaders. dxil-spirv writes the Geometry execution model, which naga's reader lacks,
//! so the SPIR-V is made a compute program first: its stage variables private, its emits and cuts
//! calls of empty functions. The geometry pass the DXBC route uses then fills those functions and
//! builds the compute or replay form around the program's own function.

use super::{registers, words};
use crate::ConvertError;
use crate::container::Element;
use crate::ir::{Basic, Ty, builtin, primitive, sysval};
use crate::lower::geometry::{CUT, EMIT, Geometry, GeometryPass, GeometryValue, Side};
use crate::lower::{self, Placed};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{Handle, Module, Statement, TypeInner};
use spirv::{BuiltIn, Decoration, ExecutionMode, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::{StageExtras, StageValue, VariantKey};

/// The name given to stage value `k`'s variable, followed by `k`.
const VALUE_NAME: &str = "gs_value";

/// `OpEntryPoint`'s words (3.32.5): the execution model, the function, then its name.
const ENTRY_MODEL: usize = 1;
const ENTRY_FUNCTION: usize = 2;
const ENTRY_NAME: usize = 3;
/// `OpExecutionMode`'s words: the entry point, the mode, then its literals.
const MODE_ENTRY: usize = 1;
const MODE: usize = 2;
const MODE_LITERAL: usize = 3;
/// `OpConstant`'s value, after its result type and result.
const CONSTANT_VALUE: usize = 3;
/// `OpEmitStreamVertex`'s and `OpEndStreamPrimitive`'s stream.
const STREAM_OPERAND: usize = 1;
/// `OpFunction`'s function type, after its result type, result and control.
const FUNCTION_TYPE: usize = 4;
/// `OpFunction`'s words.
const FUNCTION_WORDS: usize = FUNCTION_TYPE + 1;
/// `OpFunctionCall`'s words without arguments: the result type, the result and the function.
const CALL_WORDS: usize = 4;
/// `OpTypePointer`'s storage class.
const POINTER_STORAGE_CLASS: usize = 2;
/// The compute program's one invocation a group.
const LOCAL_SIZE: [u32; 3] = [1, 1, 1];

/// Why a DXIL geometry shader is malformed.
const NO_MAIN: &str = "a geometry shader without its function";
const NO_VALUE: &str = "a geometry shader's stage value the reader dropped";
const STREAM_NOT_CONSTANT: &str = "a geometry stream that is no constant";

/// A stage variable of the SPIR-V.
struct Interface {
    id: u32,
    side: Side,
    /// The location as `registers::encode` gave it.
    location: Option<u32>,
    builtin: Option<u32>,
    stream: u32,
}

/// What a geometry shader declares, read from its SPIR-V before the reader takes it.
pub(crate) struct Declared {
    input: u64,
    output: u64,
    max_vertices: u32,
    instances: u32,
    stream_mask: u64,
    values: Vec<Interface>,
}

/// One instruction's words, from `at`.
fn instruction(spirv: &[u32], at: usize) -> Option<&[u32]> {
    let count = (*spirv.get(at)? >> words::COUNT_SHIFT) as usize;
    (count != 0 && at + count <= spirv.len()).then(|| &spirv[at..at + count])
}

fn opcode(i: &[u32]) -> u32 {
    i[0] & words::OPCODE_MASK
}

fn header(op: Op, count: usize) -> u32 {
    ((count as u32) << words::COUNT_SHIFT) | op as u32
}

/// The words a literal string takes, its terminating zero included.
fn string_words(w: &[u32]) -> usize {
    w.iter()
        .position(|word| word.to_le_bytes().contains(&0))
        .map_or(w.len(), |p| p + 1)
}

/// A literal string's words.
fn string(s: &str) -> Vec<u32> {
    let mut bytes = Vec::from(s.as_bytes());
    bytes.push(0);
    bytes.resize(bytes.len().next_multiple_of(size_of::<u32>()), 0);
    bytes
        .as_chunks::<{ size_of::<u32>() }>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
        .collect()
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
    let mut main = None;
    let mut declared = Declared {
        input: 0,
        output: 0,
        max_vertices: 0,
        instances: 1,
        stream_mask: 0,
        values: Vec::new(),
    };
    let mut constants = Vec::new();
    let mut classes = Vec::new();
    let mut decorations = Vec::new();
    let mut interface = Vec::new();
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        match Op::from_u32(opcode(i)) {
            Some(Op::EntryPoint) => {
                if i[ENTRY_MODEL] != ExecutionModel::Geometry as u32 {
                    return Ok(None);
                }
                main = Some(i[ENTRY_FUNCTION]);
                let names = string_words(&i[ENTRY_NAME..]);
                interface.extend_from_slice(&i[ENTRY_NAME + names..]);
            }
            Some(Op::ExecutionMode) => {
                let mode = ExecutionMode::from_u32(i[MODE]);
                let literal = i.get(MODE_LITERAL).copied().unwrap_or(0);
                match mode {
                    Some(ExecutionMode::Invocations) => declared.instances = literal,
                    Some(ExecutionMode::OutputVertices) => declared.max_vertices = literal,
                    Some(m) => {
                        if let Some(p) = input_primitive(m) {
                            declared.input = p;
                        }
                        if let Some(p) = output_primitive(m) {
                            declared.output = p;
                        }
                    }
                    None => {}
                }
            }
            Some(Op::Decorate) if i.len() > words::decorate::LITERAL => decorations.push((
                i[words::decorate::TARGET],
                i[words::decorate::DECORATION],
                i[words::decorate::LITERAL],
            )),
            Some(Op::Constant) if i.len() > CONSTANT_VALUE => {
                constants.push((i[words::RESULT_AFTER_TYPE], i[CONSTANT_VALUE]))
            }
            Some(Op::Variable) if i.len() > words::VARIABLE_STORAGE_CLASS => classes.push((
                i[words::RESULT_AFTER_TYPE],
                i[words::VARIABLE_STORAGE_CLASS],
            )),
            _ => {}
        }
        at += i.len();
    }
    let Some(main) = main else {
        return Ok(None);
    };
    for &id in &interface {
        let side = match classes.iter().find(|c| c.0 == id).map(|c| c.1) {
            Some(c) if c == StorageClass::Input as u32 => Side::Input,
            Some(c) if c == StorageClass::Output as u32 => Side::Output,
            _ => continue,
        };
        let decoration = |d: Decoration| {
            decorations
                .iter()
                .find(|x| x.0 == id && x.1 == d as u32)
                .map(|x| x.2)
        };
        let stream = decoration(Decoration::Stream).unwrap_or(0);
        if side == Side::Output {
            declared.stream_mask |= 1 << stream;
        }
        declared.values.push(Interface {
            id,
            side,
            location: decoration(Decoration::Location),
            builtin: decoration(Decoration::BuiltIn),
            stream,
        });
    }
    let constant = |id: u32| constants.iter().find(|c| c.0 == id).map(|c| c.1);
    // Streams emitted into, which the declarations may leave undecorated.
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        if matches!(
            Op::from_u32(opcode(i)),
            Some(Op::EmitStreamVertex | Op::EndStreamPrimitive)
        ) {
            let stream = constant(i[STREAM_OPERAND])
                .ok_or_else(|| ConvertError::Malformed(STREAM_NOT_CONSTANT.into()))?;
            declared.stream_mask |= 1 << stream;
        }
        at += i.len();
    }
    declared.stream_mask = declared.stream_mask.max(1);
    let streams = u64::BITS - declared.stream_mask.leading_zeros();
    rewrite(spirv, main, &declared, streams, &constant)?;
    Ok(Some(declared))
}

/// The SPIR-V made a compute program: see [`prepare`].
fn rewrite(
    spirv: &mut Vec<u32>,
    main: u32,
    declared: &Declared,
    streams: u32,
    constant: &dyn Fn(u32) -> Option<u32>,
) -> Result<(), ConvertError> {
    let staged = |id: u32| declared.values.iter().any(|v| v.id == id);
    let mut bound = spirv[words::BOUND];
    let mut fresh = || {
        bound += 1;
        bound - 1
    };
    let emits: Vec<u32> = (0..streams).map(|_| fresh()).collect();
    let cut = fresh();
    // main's own result type (void) and function type serve the stand-ins.
    let mut signature = None;
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        if Op::from_u32(opcode(i)) == Some(Op::Function) && i[words::RESULT_AFTER_TYPE] == main {
            signature = Some((i[words::RESULT_TYPE], i[FUNCTION_TYPE]));
        }
        at += i.len();
    }
    let (void, function_type) = signature.ok_or(ConvertError::Malformed(NO_MAIN.into()))?;
    let labels: Vec<u32> = (0..=streams).map(|_| fresh()).collect();

    let mut out = Vec::from(&spirv[..words::HEADER]);
    let mut named = false;
    let mut functions_placed = false;
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        at += i.len();
        let op = Op::from_u32(opcode(i));
        let preamble = matches!(
            op,
            Some(
                Op::Capability
                    | Op::Extension
                    | Op::ExtInstImport
                    | Op::MemoryModel
                    | Op::EntryPoint
                    | Op::ExecutionMode
                    | Op::String
                    | Op::SourceExtension
                    | Op::Source
                    | Op::SourceContinued
                    | Op::Name
                    | Op::MemberName
                    | Op::ModuleProcessed
            )
        );
        if !preamble && !named {
            named = true;
            let name = |out: &mut Vec<u32>, id: u32, s: &str| {
                let s = string(s);
                out.push(header(Op::Name, 2 + s.len()));
                out.push(id);
                out.extend(s);
            };
            for (k, v) in declared.values.iter().enumerate() {
                name(&mut out, v.id, &format!("{VALUE_NAME}{k}"));
            }
            for (s, &e) in emits.iter().enumerate() {
                name(&mut out, e, &emit_name(s));
            }
            name(&mut out, cut, CUT);
        }
        match op {
            Some(Op::EntryPoint) => {
                let names = string_words(&i[ENTRY_NAME..]);
                let kept: Vec<u32> = i[ENTRY_NAME + names..]
                    .iter()
                    .copied()
                    .filter(|&id| !staged(id))
                    .collect();
                out.push(header(Op::EntryPoint, ENTRY_NAME + names + kept.len()));
                out.push(ExecutionModel::GLCompute as u32);
                out.extend_from_slice(&i[ENTRY_FUNCTION..ENTRY_NAME + names]);
                out.extend(kept);
                out.push(header(Op::ExecutionMode, MODE_LITERAL + LOCAL_SIZE.len()));
                out.extend([i[ENTRY_FUNCTION], ExecutionMode::LocalSize as u32]);
                out.extend(LOCAL_SIZE);
            }
            Some(Op::ExecutionMode) if i[MODE_ENTRY] == main => {}
            Some(Op::Name) if staged(i[words::RESULT]) => {}
            Some(Op::Decorate) if staged(i[words::decorate::TARGET]) => {}
            Some(Op::TypePointer) | Some(Op::Variable) => {
                let class = match op {
                    Some(Op::TypePointer) => POINTER_STORAGE_CLASS,
                    _ => words::VARIABLE_STORAGE_CLASS,
                };
                let mut i = Vec::from(i);
                if i[class] == StorageClass::Input as u32 || i[class] == StorageClass::Output as u32
                {
                    i[class] = StorageClass::Private as u32;
                }
                out.extend(i);
            }
            Some(
                o @ (Op::EmitVertex
                | Op::EmitStreamVertex
                | Op::EndPrimitive
                | Op::EndStreamPrimitive),
            ) => {
                let stream = match o {
                    Op::EmitStreamVertex | Op::EndStreamPrimitive => constant(i[STREAM_OPERAND])
                        .ok_or(ConvertError::Malformed(NO_MAIN.into()))?,
                    _ => 0,
                };
                let target = match o {
                    Op::EmitVertex | Op::EmitStreamVertex => emits[stream as usize],
                    _ => cut,
                };
                let result = fresh();
                out.extend([header(Op::FunctionCall, CALL_WORDS), void, result, target]);
            }
            Some(Op::Function) if !functions_placed => {
                functions_placed = true;
                for (&f, &label) in emits.iter().chain([&cut]).zip(&labels) {
                    let control = spirv::FunctionControl::NONE.bits();
                    out.extend([
                        header(Op::Function, FUNCTION_WORDS),
                        void,
                        f,
                        control,
                        function_type,
                    ]);
                    out.extend([header(Op::Label, 2), label]);
                    out.extend([header(Op::Return, 1), header(Op::FunctionEnd, 1)]);
                }
                out.extend_from_slice(i);
            }
            _ => out.extend_from_slice(i),
        }
    }
    out[words::BOUND] = bound;
    *spirv = out;
    Ok(())
}

fn emit_name(stream: usize) -> String {
    match stream {
        0 => EMIT.into(),
        s => format!("{EMIT}{s}"),
    }
}

/// The IR type of a stage variable's naga type: arrays of a scalar or vector.
fn ir_ty(module: &Module, mut ty: Handle<naga::Type>) -> Result<Ty, ConvertError> {
    let mut dims = Vec::new();
    while let TypeInner::Array {
        base,
        size: naga::ArraySize::Constant(n),
        ..
    } = module.types[ty].inner
    {
        dims.push(n.get());
        ty = base;
    }
    dims.reverse();
    let (scalar, size) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (s, 1),
        TypeInner::Vector { size, scalar } => (scalar, size as u8),
        _ => {
            return Err(ConvertError::Unsupported(
                crate::refusal::GEOMETRY_INPUT_SHAPE,
            ));
        }
    };
    Ok(Ty {
        dims,
        members: alloc::vec![Basic {
            scalar: lower::scalar_code(scalar),
            size,
        }],
    })
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
        .ok_or(ConvertError::Malformed(NO_MAIN.into()))?;
    module.entry_points.clear();
    let function = |module: &Module, name: &str| {
        module
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(name))
            .map(|(h, _)| h)
            .ok_or(ConvertError::Malformed(NO_MAIN.into()))
    };
    let mut values = Vec::new();
    for (k, v) in declared.values.iter().enumerate() {
        let name = format!("{VALUE_NAME}{k}");
        let var = module
            .global_variables
            .iter()
            .find(|(_, g)| g.name.as_deref() == Some(name.as_str()))
            .map(|(h, _)| h)
            .ok_or(ConvertError::Malformed(NO_VALUE.into()))?;
        let builtin = v.builtin.map(|b| builtin_of(b, v.side)).transpose()?;
        values.push(GeometryValue {
            var,
            side: v.side,
            location: v.location.map(registers::decode),
            builtin,
            stream: v.stream,
            ty: ir_ty(module, module.global_variables[var].ty)?,
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
        .map(|s| function(module, &emit_name(s)))
        .collect::<Result<_, _>>()?;
    geometry.cut = Some(function(module, CUT)?);
    let mut pass = GeometryPass {
        module,
        variant,
        geometry: &mut geometry,
        values: &values,
        reflected: Vec::new(),
    };
    pass.functions()?;
    let extras = pass.entry_point(main)?;
    let mut placed: [Vec<StageValue>; 2] = Default::default();
    for r in pass.reflected {
        let side = match r.side {
            Side::Input => 0,
            Side::Output => 1,
        };
        let element = r.location.and_then(|register| {
            signatures[side].iter().find(|e| {
                e.register == register
                    && e.system == sysval::NONE
                    && e.mask.trailing_zeros() == r.component
            })
        });
        let (semantic, index) =
            element.map_or_else(Default::default, |e| (e.semantic.clone(), e.index));
        let place = Placed {
            location: r.location,
            component: r.component,
            member: r.member,
            modes: r.modes,
            system: r.system,
            stream: values[r.value].stream as u8,
        };
        placed[side].push(lower::stage_value(semantic, index, &place, false));
    }
    for (values, signature) in placed.iter_mut().zip(signatures) {
        crate::container::place_system_values(values, signature);
    }
    Ok((extras, placed))
}
