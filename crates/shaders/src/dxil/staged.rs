//! Stages naga's reader lacks (geometry, hull, domain) made stages it takes, in the SPIR-V words: the
//! entry point's stage variables made private and named, so a pass of `crate::lower` builds the stage's
//! form around the program's own functions, and the instructions the reader lacks made calls of empty
//! functions that pass fills.

use super::registers;
use crate::ConvertError;
use crate::container::Element;
use crate::ir::{Basic, Ty, sysval};
use crate::lower::geometry::{Reflected, Side};
use crate::lower::{self, Placed};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{Handle, Module, TypeInner};
use rspirv::dr::{self, Instruction, Operand};
use spirv::{Decoration, ExecutionMode, ExecutionModel, FunctionControl, Op, StorageClass};
use wcontainers_gpu::StageValue;

/// `OpEntryPoint`'s operands (3.32.5): the execution model, the function, its name, then its
/// interface.
const ENTRY_MODEL: usize = 0;
const ENTRY_FUNCTION: usize = 1;
const ENTRY_INTERFACE: usize = 3;
/// `OpExecutionMode`'s operands: the entry point, the mode, then its literals.
const MODE_ENTRY: usize = 0;
const MODE_LITERAL: usize = 2;
/// `OpDecorate`'s operands: the target, the decoration, then its literal.
pub(super) const DECORATION_TARGET: usize = 0;
pub(super) const DECORATION: usize = 1;
pub(super) const DECORATION_LITERAL: usize = 2;
/// `OpFunction`'s function type, after its control.
const FUNCTION_TYPE: usize = 1;
/// `OpTypePointer`'s and `OpVariable`'s storage class, and the pointer's pointee.
pub(super) const STORAGE_CLASS: usize = 0;
pub(super) const POINTEE: usize = 1;
/// `OpTypeInt`'s signedness, after its width.
pub(super) const SIGNEDNESS: usize = 1;
/// A compute program's one invocation a group.
const LOCAL_SIZE: [u32; 3] = [1, 1, 1];

/// Why a stage's SPIR-V is malformed.
pub(super) const NO_MAIN: &str = "a stage without its function";
const NO_VALUE: &str = "a stage value the reader dropped";

/// A stage variable of the SPIR-V.
pub(super) struct Interface {
    pub id: u32,
    pub side: Side,
    /// The location as `registers::encode` gave it.
    pub location: Option<u32>,
    pub builtin: Option<u32>,
    pub stream: u32,
    /// Decorated `Patch`: one value for the patch.
    pub patch: bool,
}

/// The entry point of a SPIR-V program as read before the reader takes it.
pub(super) struct Entry {
    pub main: u32,
    /// The entry point's execution modes, with their first literal (zero where none).
    pub modes: Vec<(ExecutionMode, u32)>,
    pub values: Vec<Interface>,
    /// Every 32-bit constant's id and value.
    pub constants: Vec<(u32, u32)>,
}

impl Entry {
    pub fn constant(&self, id: u32) -> Option<u32> {
        self.constants.iter().find(|c| c.0 == id).map(|c| c.1)
    }

    pub fn has_mode(&self, mode: ExecutionMode) -> bool {
        self.modes.iter().any(|m| m.0 == mode)
    }

    pub fn mode_literal(&self, mode: ExecutionMode) -> Option<u32> {
        self.modes.iter().find(|m| m.0 == mode).map(|m| m.1)
    }
}

/// The SPIR-V dxil-spirv wrote, read into rspirv's module.
pub(super) fn load(words: &[u32]) -> Result<dr::Module, ConvertError> {
    rspirv::dr::load_words(words).map_err(|e| ConvertError::Malformed(format!("SPIR-V: {e}")))
}

/// Operand `n` of `i` if it is an id.
pub(super) fn id(i: &Instruction, n: usize) -> Option<u32> {
    match i.operands.get(n) {
        Some(&Operand::IdRef(id)) => Some(id),
        _ => None,
    }
}

/// Operand `n` of `i` if it is a 32-bit literal (a built-in counted as its number).
pub(super) fn literal(i: &Instruction, n: usize) -> Option<u32> {
    match i.operands.get(n) {
        Some(&Operand::LiteralBit32(v)) => Some(v),
        Some(&Operand::BuiltIn(b)) => Some(b as u32),
        _ => None,
    }
}

/// Whether `i` is an `OpDecorate` of `decoration`.
pub(super) fn decorates(i: &Instruction, decoration: Decoration) -> bool {
    i.class.opcode == Op::Decorate
        && i.operands.get(DECORATION) == Some(&Operand::Decoration(decoration))
}

/// The entry point of `spirv` if it is of `model`, with its stage variables.
pub(super) fn entry(spirv: &dr::Module, model: ExecutionModel) -> Option<Entry> {
    let mut main = None;
    let mut interface = Vec::new();
    for e in &spirv.entry_points {
        if e.operands.get(ENTRY_MODEL) != Some(&Operand::ExecutionModel(model)) {
            return None;
        }
        main = id(e, ENTRY_FUNCTION);
        interface.extend(
            e.operands
                .iter()
                .skip(ENTRY_INTERFACE)
                .filter_map(Operand::id_ref_any),
        );
    }
    let main = main?;
    let modes = spirv
        .execution_modes
        .iter()
        .filter(|m| id(m, MODE_ENTRY) == Some(main))
        .filter_map(|m| match m.operands.get(MODE_ENTRY + 1) {
            Some(&Operand::ExecutionMode(mode)) => Some((
                mode,
                literal(m, MODE_LITERAL)
                    .or_else(|| id(m, MODE_LITERAL))
                    .unwrap_or(0),
            )),
            _ => None,
        })
        .collect();
    let globals = &spirv.types_global_values;
    let constants = globals
        .iter()
        .filter(|i| i.class.opcode == Op::Constant)
        .filter_map(|i| Some((i.result_id?, literal(i, 0)?)))
        .collect();
    let class = |id: u32| {
        globals
            .iter()
            .find_map(|i| match i.operands.get(STORAGE_CLASS) {
                Some(&Operand::StorageClass(c))
                    if i.class.opcode == Op::Variable && i.result_id == Some(id) =>
                {
                    Some(c)
                }
                _ => None,
            })
    };
    let mut values = Vec::new();
    for &value in &interface {
        let side = match class(value) {
            Some(StorageClass::Input) => Side::Input,
            Some(StorageClass::Output) => Side::Output,
            _ => continue,
        };
        let decoration = |d: Decoration| {
            spirv
                .annotations
                .iter()
                .find(|i| decorates(i, d) && id(i, DECORATION_TARGET) == Some(value))
        };
        let decorated = |d: Decoration| decoration(d).and_then(|i| literal(i, DECORATION_LITERAL));
        values.push(Interface {
            id: value,
            side,
            location: decorated(Decoration::Location),
            builtin: decorated(Decoration::BuiltIn),
            stream: decorated(Decoration::Stream).unwrap_or(0),
            patch: decoration(Decoration::Patch).is_some(),
        });
    }
    Some(Entry {
        main,
        modes,
        values,
        constants,
    })
}

/// The stand-in function an instruction becomes a call of, if any.
pub(super) type StandInCall<'a> = dyn Fn(&Instruction) -> Result<Option<u32>, ConvertError> + 'a;

/// What [`privatise`] makes of a program.
pub(super) struct Privatise<'a> {
    pub entry: &'a Entry,
    /// The values made private, each named [`value_name`] of `prefix`.
    pub values: &'a [&'a Interface],
    pub prefix: &'a str,
    /// The execution model the entry point takes.
    pub model: ExecutionModel,
    /// The storage classes made private.
    pub classes: &'a [StorageClass],
    /// Empty functions added (id and name), and the instructions replaced by a call of one of them.
    pub stand_ins: &'a [(u32, String)],
    pub calls: &'a StandInCall<'a>,
}

/// The name of private value `k` of `prefix`.
pub(super) fn value_name(prefix: &str, k: usize) -> String {
    format!("{prefix}{k}")
}

/// Fresh ids past the module's bound.
pub(super) struct Ids(u32);

impl Ids {
    pub fn of(spirv: &dr::Module) -> Ids {
        Ids(spirv.header.as_ref().map_or(1, |h| h.bound))
    }

    pub fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0 - 1
    }

    /// Makes the module's bound pass the ids given.
    pub fn store(self, spirv: &mut dr::Module) {
        if let Some(h) = spirv.header.as_mut() {
            h.bound = self.0;
        }
    }
}

/// An `OpName` naming `id` `s`.
pub(super) fn name(id: u32, s: &str) -> Instruction {
    Instruction::new(
        Op::Name,
        None,
        None,
        alloc::vec![Operand::IdRef(id), s.into()],
    )
}

/// Drops the names and decorations of the ids `dropped` names, and the ids from the entry points'
/// interfaces.
pub(super) fn forget(spirv: &mut dr::Module, dropped: &dyn Fn(u32) -> bool) {
    for e in &mut spirv.entry_points {
        let mut at = 0;
        e.operands.retain(|o| {
            at += 1;
            at <= ENTRY_INTERFACE || !o.id_ref_any().is_some_and(dropped)
        });
    }
    let named = |i: &Instruction, op: Op| i.class.opcode == op && id(i, 0).is_some_and(dropped);
    spirv.debug_names.retain(|i| !named(i, Op::Name));
    spirv.annotations.retain(|i| !named(i, Op::Decorate));
}

/// Makes `p.entry`'s values private and named, its entry point of `p.model` (one invocation a group
/// for compute) without its execution modes, and the instructions `p.calls` names calls of the
/// stand-in functions, placed before the first function.
pub(super) fn privatise(
    spirv: &mut dr::Module,
    mut ids: Ids,
    p: &Privatise,
) -> Result<(), ConvertError> {
    let main = p.entry.main;
    // main's own result type (void) and function type serve the stand-ins.
    let def = spirv
        .functions
        .iter()
        .find(|f| f.def_id() == Some(main))
        .and_then(|f| f.def.as_ref());
    let (void, function_type) = def
        .and_then(|d| Some((d.result_type?, id(d, FUNCTION_TYPE)?)))
        .ok_or(ConvertError::Malformed(NO_MAIN.into()))?;
    let labels: Vec<u32> = p.stand_ins.iter().map(|_| ids.next()).collect();

    forget(spirv, &|id| p.values.iter().any(|v| v.id == id));
    for (k, v) in p.values.iter().enumerate() {
        spirv.debug_names.push(name(v.id, &value_name(p.prefix, k)));
    }
    for (f, n) in p.stand_ins {
        spirv.debug_names.push(name(*f, n));
    }
    for f in &mut spirv.functions {
        for i in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            if let Some(target) = (p.calls)(i)? {
                let call = alloc::vec![Operand::IdRef(target)];
                *i = Instruction::new(Op::FunctionCall, Some(void), Some(ids.next()), call);
            }
        }
    }
    for model in spirv
        .entry_points
        .iter_mut()
        .filter_map(|e| e.operands.get_mut(ENTRY_MODEL))
    {
        *model = Operand::ExecutionModel(p.model);
    }
    spirv
        .execution_modes
        .retain(|m| id(m, MODE_ENTRY) != Some(main));
    if p.model == ExecutionModel::GLCompute {
        let mut operands = alloc::vec![
            Operand::IdRef(main),
            Operand::ExecutionMode(ExecutionMode::LocalSize)
        ];
        operands.extend(LOCAL_SIZE.map(Operand::LiteralBit32));
        let mode = Instruction::new(Op::ExecutionMode, None, None, operands);
        spirv.execution_modes.insert(0, mode);
    }
    for i in &mut spirv.types_global_values {
        if let (Op::TypePointer | Op::Variable, Some(Operand::StorageClass(class))) =
            (i.class.opcode, i.operands.get_mut(STORAGE_CLASS))
            && p.classes.contains(class)
        {
            *class = StorageClass::Private;
        }
    }
    let stand_ins = p.stand_ins.iter().zip(labels).map(|(&(f, _), label)| {
        let control = Operand::FunctionControl(FunctionControl::NONE);
        let operands = alloc::vec![control, Operand::IdRef(function_type)];
        dr::Function {
            def: Some(Instruction::new(
                Op::Function,
                Some(void),
                Some(f),
                operands,
            )),
            end: Some(Instruction::new(Op::FunctionEnd, None, None, Vec::new())),
            parameters: Vec::new(),
            blocks: alloc::vec![dr::Block {
                label: Some(Instruction::new(Op::Label, None, Some(label), Vec::new())),
                instructions: alloc::vec![Instruction::new(Op::Return, None, None, Vec::new())],
            }],
        }
    });
    spirv.functions.splice(0..0, stand_ins);
    ids.store(spirv);
    Ok(())
}

/// The handle of the function the reader named `name`.
pub(super) fn function(
    module: &Module,
    name: &str,
) -> Result<Handle<naga::Function>, ConvertError> {
    module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(name))
        .map(|(h, _)| h)
        .ok_or(ConvertError::Malformed(NO_MAIN.into()))
}

/// The private global the reader made of value `k` of `prefix`.
pub(super) fn value_variable(
    module: &Module,
    prefix: &str,
    k: usize,
) -> Result<Handle<naga::GlobalVariable>, ConvertError> {
    let name = value_name(prefix, k);
    module
        .global_variables
        .iter()
        .find(|(_, g)| g.name.as_deref() == Some(name.as_str()))
        .map(|(h, _)| h)
        .ok_or(ConvertError::Malformed(NO_VALUE.into()))
}

/// The IR type of a stage variable's naga type: arrays of a scalar or vector, else refused as `shape`.
pub(crate) fn ir_ty(
    module: &Module,
    mut ty: Handle<naga::Type>,
    shape: &'static str,
) -> Result<Ty, ConvertError> {
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
        _ => return Err(ConvertError::Unsupported(shape)),
    };
    Ok(Ty {
        dims,
        members: alloc::vec![Basic {
            scalar: lower::scalar_code(scalar),
            size,
        }],
    })
}

/// The signature element a value of `stream` at `register` and `component` is, among `signature`'s
/// user values.
pub(super) fn element_at(
    signature: &[Element],
    register: u32,
    component: u32,
    stream: u32,
) -> Option<&Element> {
    signature.iter().find(|e| {
        e.register == register
            && e.stream == stream
            && e.system == sysval::NONE
            && e.mask.trailing_zeros() == component
    })
}

/// The reflection of the values a pass reflected, placed by the container's input and output
/// signatures; `stream` gives a value's stream by its index.
pub(crate) fn place(
    reflected: Vec<Reflected>,
    stream: impl Fn(usize) -> u32,
    signatures: [&[Element]; 2],
) -> [Vec<StageValue>; 2] {
    let mut placed: [Vec<StageValue>; 2] = Default::default();
    for r in reflected {
        let side = match r.side {
            Side::Input => 0,
            Side::Output => 1,
        };
        let stream = stream(r.value);
        let element = r
            .location
            .and_then(|register| element_at(signatures[side], register, r.component, stream));
        let (semantic, index) =
            element.map_or_else(Default::default, |e| (e.semantic.clone(), e.index));
        let place = Placed {
            location: r.location,
            component: r.component,
            member: r.member,
            modes: r.modes,
            system: r.system,
            stream: stream as u8,
        };
        placed[side].push(lower::stage_value(semantic, index, &place, false));
    }
    for (values, signature) in placed.iter_mut().zip(signatures) {
        crate::container::place_system_values(values, signature);
    }
    placed
}

/// The location `registers::encode` gave, as a register and component.
pub(super) fn location(encoded: u32) -> (u32, u32) {
    registers::decode(encoded)
}
