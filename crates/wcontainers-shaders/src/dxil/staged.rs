//! Stages naga's reader lacks (geometry, hull, domain) made stages it takes, in the SPIR-V words: the
//! entry point's stage variables made private and named, so a pass of `crate::lower` builds the stage's
//! form around the program's own functions, and the instructions the reader lacks made calls of empty
//! functions that pass fills.

use super::{registers, words};
use crate::ConvertError;
use crate::container::Element;
use crate::ir::{Basic, Ty, sysval};
use crate::lower::geometry::{Reflected, Side};
use crate::lower::{self, Placed};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{Handle, Module, TypeInner};
use spirv::{Decoration, ExecutionMode, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::StageValue;

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
/// `OpFunction`'s function type, after its result type, result and control.
const FUNCTION_TYPE: usize = 4;
/// `OpFunction`'s words.
const FUNCTION_WORDS: usize = FUNCTION_TYPE + 1;
/// `OpFunctionCall`'s words without arguments: the result type, the result and the function.
const CALL_WORDS: usize = 4;
/// `OpName`'s words before its string: the target.
const NAME_WORDS: usize = 2;
/// `OpLabel`'s words.
const LABEL_WORDS: usize = 2;
/// `OpTypePointer`'s storage class.
const POINTER_STORAGE_CLASS: usize = 2;
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
    pub modes: Vec<(u32, u32)>,
    pub values: Vec<Interface>,
    /// Every integer constant's id and value.
    pub constants: Vec<(u32, u32)>,
}

impl Entry {
    pub fn constant(&self, id: u32) -> Option<u32> {
        self.constants.iter().find(|c| c.0 == id).map(|c| c.1)
    }

    pub fn has_mode(&self, mode: ExecutionMode) -> bool {
        self.modes.iter().any(|m| m.0 == mode as u32)
    }

    pub fn mode_literal(&self, mode: ExecutionMode) -> Option<u32> {
        self.modes.iter().find(|m| m.0 == mode as u32).map(|m| m.1)
    }
}

/// One instruction's words, from `at`.
pub(super) fn instruction(spirv: &[u32], at: usize) -> Option<&[u32]> {
    let count = (*spirv.get(at)? >> words::COUNT_SHIFT) as usize;
    (count != 0 && at + count <= spirv.len()).then(|| &spirv[at..at + count])
}

pub(super) fn opcode(i: &[u32]) -> u32 {
    i[0] & words::OPCODE_MASK
}

pub(super) fn header(op: Op, count: usize) -> u32 {
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

/// The entry point of `spirv` if it is of `model`, with its stage variables.
pub(super) fn entry(spirv: &[u32], model: ExecutionModel) -> Option<Entry> {
    let mut found = None;
    let mut modes = Vec::new();
    let mut constants = Vec::new();
    let mut classes = Vec::new();
    let mut decorations = Vec::new();
    let mut interface = Vec::new();
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        match Op::from_u32(opcode(i)) {
            Some(Op::EntryPoint) => {
                if i[ENTRY_MODEL] != model as u32 {
                    return None;
                }
                found = Some(i[ENTRY_FUNCTION]);
                let names = string_words(&i[ENTRY_NAME..]);
                interface.extend_from_slice(&i[ENTRY_NAME + names..]);
            }
            Some(Op::ExecutionMode) => modes.push((
                i[MODE_ENTRY],
                i[MODE],
                i.get(MODE_LITERAL).copied().unwrap_or(0),
            )),
            Some(Op::Decorate) if i.len() > words::decorate::DECORATION => decorations.push((
                i[words::decorate::TARGET],
                i[words::decorate::DECORATION],
                i.get(words::decorate::LITERAL).copied(),
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
    let main = found?;
    let mut values = Vec::new();
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
        let literal = |d: Decoration| decoration(d).flatten();
        values.push(Interface {
            id,
            side,
            location: literal(Decoration::Location),
            builtin: literal(Decoration::BuiltIn),
            stream: literal(Decoration::Stream).unwrap_or(0),
            patch: decoration(Decoration::Patch).is_some(),
        });
    }
    Some(Entry {
        main,
        modes: modes
            .into_iter()
            .filter(|m| m.0 == main)
            .map(|m| (m.1, m.2))
            .collect(),
        values,
        constants,
    })
}

/// The stand-in function an instruction becomes a call of, if any.
pub(super) type StandInCall<'a> = dyn Fn(&[u32]) -> Result<Option<u32>, ConvertError> + 'a;

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
    pub fn of(spirv: &[u32]) -> Ids {
        Ids(spirv[words::BOUND])
    }

    pub fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0 - 1
    }

    /// The module's bound past the ids given.
    pub fn bound(&self) -> u32 {
        self.0
    }
}

/// Whether `op` belongs before the module's annotations (names included).
pub(super) fn preamble(op: Op) -> bool {
    matches!(
        op,
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
}

/// An `OpName` naming `id` `s`.
pub(super) fn name(out: &mut Vec<u32>, id: u32, s: &str) {
    let s = string(s);
    out.push(header(Op::Name, NAME_WORDS + s.len()));
    out.push(id);
    out.extend(s);
}

/// `OpEntryPoint` instruction `i` without the interface variables `staged` names.
pub(super) fn without_interface(i: &[u32], staged: &dyn Fn(u32) -> bool) -> Vec<u32> {
    let names = string_words(&i[ENTRY_NAME..]);
    let kept: Vec<u32> = i[ENTRY_NAME + names..]
        .iter()
        .copied()
        .filter(|&id| !staged(id))
        .collect();
    let mut out = Vec::from(&i[..ENTRY_NAME + names]);
    out[0] = header(Op::EntryPoint, ENTRY_NAME + names + kept.len());
    out.extend(kept);
    out
}

/// Makes `p.entry`'s values private and named, its entry point of `p.model` (one invocation a group
/// for compute) without its execution modes, and the instructions `p.calls` names calls of the
/// stand-in functions, placed before the first function.
pub(super) fn privatise(
    spirv: &mut Vec<u32>,
    ids: &mut Ids,
    p: &Privatise,
) -> Result<(), ConvertError> {
    let main = p.entry.main;
    let staged = |id: u32| p.values.iter().any(|v| v.id == id);
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
    let labels: Vec<u32> = p.stand_ins.iter().map(|_| ids.next()).collect();
    let private = |class: u32| p.classes.iter().any(|&c| c as u32 == class);

    let mut out = Vec::from(&spirv[..words::HEADER]);
    let mut named = false;
    let mut functions_placed = false;
    let mut at = words::HEADER;
    while let Some(i) = instruction(spirv, at) {
        at += i.len();
        let op = Op::from_u32(opcode(i));
        if !named && !op.is_some_and(preamble) {
            named = true;
            for (k, v) in p.values.iter().enumerate() {
                name(&mut out, v.id, &value_name(p.prefix, k));
            }
            for (f, n) in p.stand_ins {
                name(&mut out, *f, n);
            }
        }
        if let Some(target) = (p.calls)(i)? {
            let result = ids.next();
            out.extend([header(Op::FunctionCall, CALL_WORDS), void, result, target]);
            continue;
        }
        match op {
            Some(Op::EntryPoint) => {
                let mut kept = without_interface(i, &staged);
                kept[ENTRY_MODEL] = p.model as u32;
                out.extend(kept);
                if p.model == ExecutionModel::GLCompute {
                    out.push(header(Op::ExecutionMode, MODE_LITERAL + LOCAL_SIZE.len()));
                    out.extend([i[ENTRY_FUNCTION], ExecutionMode::LocalSize as u32]);
                    out.extend(LOCAL_SIZE);
                }
            }
            Some(Op::ExecutionMode) if i[MODE_ENTRY] == main => {}
            Some(Op::Name) if staged(i[words::RESULT_ID]) => {}
            Some(Op::Decorate) if staged(i[words::decorate::TARGET]) => {}
            Some(Op::TypePointer) | Some(Op::Variable) => {
                let class = match op {
                    Some(Op::TypePointer) => POINTER_STORAGE_CLASS,
                    _ => words::VARIABLE_STORAGE_CLASS,
                };
                let mut i = Vec::from(i);
                if private(i[class]) {
                    i[class] = StorageClass::Private as u32;
                }
                out.extend(i);
            }
            Some(Op::Function) if !functions_placed => {
                functions_placed = true;
                for (&(f, _), &label) in p.stand_ins.iter().zip(&labels) {
                    let control = spirv::FunctionControl::NONE.bits();
                    out.extend([
                        header(Op::Function, FUNCTION_WORDS),
                        void,
                        f,
                        control,
                        function_type,
                    ]);
                    out.extend([header(Op::Label, LABEL_WORDS), label]);
                    out.extend([header(Op::Return, 1), header(Op::FunctionEnd, 1)]);
                }
                out.extend_from_slice(i);
            }
            _ => out.extend_from_slice(i),
        }
    }
    out[words::BOUND] = ids.bound();
    *spirv = out;
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
pub(super) fn ir_ty(
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
pub(super) fn place(
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
