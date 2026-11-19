//! Integer and float widths WebGPU lacks. 16-bit integers (minimum precision, and native 16-bit
//! types) become 32-bit words holding the 16-bit value's bits with zeros above: a result that may
//! carry past 16 bits is masked, and an operand read as signed is sign-extended first, so every
//! value equals the 16-bit one; an instruction not listed here is refused, naming it. 64-bit types
//! are refused by name.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;
use naga::{Module, ScalarKind, TypeInner};
use spirv::{GlslStd450Op as GLOp, Op, StorageClass};

use super::words;
use crate::ConvertError;

/// The bits of the narrow integers, and the mask keeping them.
const NARROW_BITS: u32 = u16::BITS;
const NARROW_MASK: u32 = u16::MAX as u32;
/// The width of the integers they become.
const WIDE_BITS: u32 = u32::BITS;
/// The width of the types WebGPU has no form for.
const DOUBLE_BITS: u32 = u64::BITS;

/// Results that may carry past 16 bits, masked after.
const CARRY: [Op; 16] = [
    Op::IAdd,
    Op::ISub,
    Op::IMul,
    Op::ShiftLeftLogical,
    Op::Not,
    Op::SNegate,
    Op::SDiv,
    Op::SRem,
    Op::SMod,
    Op::ShiftRightArithmetic,
    Op::BitFieldInsert,
    Op::BitFieldSExtract,
    Op::ConvertFToU,
    Op::ConvertFToS,
    Op::UConvert,
    Op::SConvert,
];
/// Instructions reading every value operand as signed.
const SIGNED: [Op; 9] = [
    Op::SDiv,
    Op::SRem,
    Op::SMod,
    Op::SLessThan,
    Op::SLessThanEqual,
    Op::SGreaterThan,
    Op::SGreaterThanEqual,
    Op::ConvertSToF,
    Op::SConvert,
];
/// Instructions reading only their first value operand as signed (the base; not the shift or the
/// bit range).
const SIGNED_BASE: [Op; 2] = [Op::ShiftRightArithmetic, Op::BitFieldSExtract];
/// Instructions for which bits with zeros above are the value as it is.
const KEEP: [Op; 35] = [
    Op::Load,
    Op::Store,
    Op::CopyObject,
    Op::Phi,
    Op::Select,
    Op::CompositeConstruct,
    Op::CompositeExtract,
    Op::CompositeInsert,
    Op::VectorShuffle,
    Op::VectorExtractDynamic,
    Op::VectorInsertDynamic,
    Op::BitwiseAnd,
    Op::BitwiseOr,
    Op::BitwiseXor,
    Op::IEqual,
    Op::INotEqual,
    Op::ULessThan,
    Op::ULessThanEqual,
    Op::UGreaterThan,
    Op::UGreaterThanEqual,
    Op::UDiv,
    Op::UMod,
    Op::ShiftRightLogical,
    Op::ConvertUToF,
    Op::BitCount,
    Op::BitFieldUExtract,
    Op::Variable,
    Op::AccessChain,
    Op::InBoundsAccessChain,
    Op::FunctionCall,
    Op::ReturnValue,
    Op::Function,
    Op::FunctionParameter,
    Op::FunctionEnd,
    Op::Undef,
];
/// Instructions of a function's body with no result type (their first operand is no type).
const NO_RESULT_TYPE: [Op; 18] = [
    Op::Label,
    Op::Store,
    Op::Branch,
    Op::BranchConditional,
    Op::Switch,
    Op::SelectionMerge,
    Op::LoopMerge,
    Op::Return,
    Op::ReturnValue,
    Op::Kill,
    Op::Unreachable,
    Op::DemoteToHelperInvocation,
    Op::ControlBarrier,
    Op::MemoryBarrier,
    Op::ImageWrite,
    Op::AtomicStore,
    Op::Line,
    Op::NoLine,
];
/// Control flow, which takes a 16-bit selector's bits as they are.
const CONTROL: [Op; 11] = [
    Op::Label,
    Op::Branch,
    Op::BranchConditional,
    Op::Switch,
    Op::SelectionMerge,
    Op::LoopMerge,
    Op::Return,
    Op::Kill,
    Op::Unreachable,
    Op::Line,
    Op::NoLine,
];
/// GLSL.std.450 instructions reading their operands as signed.
const GLSL_SIGNED: [GLOp; 6] = [
    GLOp::SAbs,
    GLOp::SSign,
    GLOp::SMin,
    GLOp::SMax,
    GLOp::SClamp,
    GLOp::FindSMsb,
];
/// GLSL.std.450 instructions whose results may carry past 16 bits (a sign-extended result, or -1).
const GLSL_CARRY: [GLOp; 8] = [
    GLOp::SAbs,
    GLOp::SSign,
    GLOp::SMin,
    GLOp::SMax,
    GLOp::SClamp,
    GLOp::FindILsb,
    GLOp::FindSMsb,
    GLOp::FindUMsb,
];
/// GLSL.std.450 instructions keeping bits with zeros above.
const GLSL_KEEP: [GLOp; 3] = [GLOp::UMin, GLOp::UMax, GLOp::UClamp];

/// The refusal of an instruction on 16-bit integers that this module does not widen.
fn unlisted(op: Op) -> ConvertError {
    ConvertError::Reader(format!("{op:?} on 16-bit integers"))
}

/// An instruction: its opcode and its operand words.
struct Instruction {
    op: u32,
    operands: Vec<u32>,
}

impl Instruction {
    fn new(op: Op, operands: Vec<u32>) -> Self {
        Self {
            op: op as u32,
            operands,
        }
    }
}

/// The instructions of `spirv` after its header, or `None` where a word count runs past its end.
fn parse(spirv: &[u32]) -> Option<Vec<Instruction>> {
    let mut list = Vec::new();
    let mut at = words::HEADER;
    while at < spirv.len() {
        let count = (spirv[at] >> words::COUNT_SHIFT) as usize;
        if count == 0 || at + count > spirv.len() {
            return None;
        }
        list.push(Instruction {
            op: spirv[at] & words::OPCODE_MASK,
            operands: spirv[at + 1..at + count].to_vec(),
        });
        at += count;
    }
    Some(list)
}

/// The constants a narrow type needs: its mask and its shift by the narrow width.
#[derive(Clone, Copy)]
struct Narrow {
    mask: u32,
    shift: u32,
}

/// The name of the 64-bit type `spirv` declares, if any.
fn double_width(spirv: &[u32]) -> Option<&'static str> {
    let list = parse(spirv)?;
    list.iter().find_map(|i| match Op::from_u32(i.op) {
        Some(Op::TypeInt) if i.operands.get(1) == Some(&DOUBLE_BITS) => Some("Int64"),
        Some(Op::TypeFloat) if i.operands.get(1) == Some(&DOUBLE_BITS) => Some("Float64"),
        _ => None,
    })
}

/// Reads `spirv` with naga's reader, refusing a module with 64-bit types by name: where the reader
/// fails on them, and where the module keeps them (WebGPU has neither).
pub(crate) fn read(
    spirv: &[u32],
    options: &naga::front::spv::Options,
) -> Result<Module, ConvertError> {
    let double = double_width(spirv);
    let module = naga::front::spv::Frontend::new(spirv.iter().copied(), options)
        .parse()
        .map_err(|e| match double {
            Some(name) => ConvertError::Unsupported(name),
            None => ConvertError::Reader(format!("{e}")),
        })?;
    let wide = |s: naga::Scalar| (s.width as u32) * u8::BITS == DOUBLE_BITS;
    for (_, ty) in module.types.iter() {
        let scalar = match ty.inner {
            TypeInner::Scalar(s) | TypeInner::Vector { scalar: s, .. } => s,
            TypeInner::Matrix { scalar, .. } => scalar,
            _ => continue,
        };
        if wide(scalar) {
            return Err(ConvertError::Unsupported(match scalar.kind {
                ScalarKind::Float => "Float64",
                _ => "Int64",
            }));
        }
    }
    Ok(module)
}

/// Makes every 16-bit integer of `spirv` a 32-bit one holding the same bits (see the module's text).
/// A 16-bit integer in memory with a layout (a buffer's member), between stages, or under an
/// instruction not listed here is refused by name.
pub(crate) fn widen(spirv: &mut Vec<u32>) -> Result<(), ConvertError> {
    let Some(list) = parse(spirv) else {
        return Ok(());
    };
    let narrow_scalar =
        |i: &Instruction| i.op == Op::TypeInt as u32 && i.operands.get(1) == Some(&NARROW_BITS);
    if !list.iter().any(narrow_scalar) {
        return Ok(());
    }
    let mut bound = spirv[words::BOUND];
    let mut next = || {
        bound += 1;
        bound - 1
    };

    // The narrow scalar and vector types, the types holding them, and the GLSL.std.450 set.
    let mut narrow: BTreeMap<u32, Narrow> = BTreeMap::new();
    let mut holding: BTreeSet<u32> = BTreeSet::new();
    let mut glsl = None;
    let mut laid_out = Vec::new();
    for i in &list {
        let w = &i.operands;
        match Op::from_u32(i.op) {
            Some(Op::TypeInt) if narrow_scalar(i) => {
                let n = Narrow {
                    mask: next(),
                    shift: next(),
                };
                narrow.insert(w[0], n);
                holding.insert(w[0]);
            }
            Some(Op::TypeVector) if narrow.contains_key(&w[1]) => {
                let n = Narrow {
                    mask: next(),
                    shift: next(),
                };
                narrow.insert(w[0], n);
                holding.insert(w[0]);
            }
            Some(Op::TypeArray | Op::TypeRuntimeArray | Op::TypeMatrix)
                if holding.contains(&w[1]) =>
            {
                holding.insert(w[0]);
            }
            Some(Op::TypeStruct) if w[1..].iter().any(|m| holding.contains(m)) => {
                holding.insert(w[0]);
            }
            Some(Op::TypePointer) if holding.contains(&w[2]) => {
                holding.insert(w[0]);
            }
            Some(Op::ExtInstImport) => {
                let name: Vec<u8> = w[1..].iter().flat_map(|x| x.to_le_bytes()).collect();
                if name.starts_with(b"GLSL.std.450\0") {
                    glsl = Some(w[0]);
                }
            }
            Some(Op::MemberDecorate) if w.get(2) == Some(&(spirv::Decoration::Offset as u32)) => {
                laid_out.push(w[0]);
            }
            Some(Op::Decorate) if w.get(1) == Some(&(spirv::Decoration::ArrayStride as u32)) => {
                laid_out.push(w[0]);
            }
            Some(Op::Variable)
                if holding.contains(&w[0])
                    && [StorageClass::Input as u32, StorageClass::Output as u32]
                        .contains(&w[2]) =>
            {
                return Err(ConvertError::Unsupported("Int16StageValue"));
            }
            _ => {}
        }
    }
    if laid_out.iter().any(|t| holding.contains(t)) {
        return Err(ConvertError::Unsupported("Int16Storage"));
    }

    // Each value's type.
    let mut types: BTreeMap<u32, u32> = BTreeMap::new();
    let mut in_function = false;
    for i in &list {
        let Some(op) = Op::from_u32(i.op) else {
            continue;
        };
        match op {
            Op::Function => in_function = true,
            Op::FunctionEnd => in_function = false,
            _ => {}
        }
        let typed = matches!(
            op,
            Op::Constant
                | Op::ConstantComposite
                | Op::ConstantNull
                | Op::SpecConstant
                | Op::SpecConstantComposite
                | Op::Variable
                | Op::Undef
                | Op::Function
                | Op::FunctionParameter
        ) || (in_function && !NO_RESULT_TYPE.contains(&op) && op != Op::FunctionEnd);
        if typed && i.operands.len() >= 2 {
            types.insert(i.operands[1], i.operands[0]);
        }
    }
    let is_narrow = |id: u32| types.get(&id).is_some_and(|t| narrow.contains_key(t));

    let mut out: Vec<Instruction> = Vec::with_capacity(list.len());
    let mut in_function = false;
    for mut i in list {
        let Some(op) = Op::from_u32(i.op) else {
            out.push(i);
            continue;
        };
        match op {
            Op::Function => in_function = true,
            Op::FunctionEnd => in_function = false,
            _ => {}
        }
        if !in_function {
            match op {
                Op::Capability
                    if i.operands.first() == Some(&(spirv::Capability::Int16 as u32)) =>
                {
                    continue;
                }
                Op::TypeInt if narrow_scalar(&i) => {
                    let id = i.operands[0];
                    i.operands[1] = WIDE_BITS;
                    out.push(i);
                    let n = narrow[&id];
                    out.push(Instruction::new(
                        Op::Constant,
                        alloc::vec![id, n.mask, NARROW_MASK],
                    ));
                    out.push(Instruction::new(
                        Op::Constant,
                        alloc::vec![id, n.shift, NARROW_BITS],
                    ));
                }
                Op::TypeVector if narrow.contains_key(&i.operands[0]) => {
                    let (id, component, count) = (i.operands[0], i.operands[1], i.operands[2]);
                    out.push(i);
                    let (n, c) = (narrow[&id], narrow[&component]);
                    let splat = |of: u32, v: u32| {
                        let mut w = alloc::vec![id, of];
                        w.extend(core::iter::repeat_n(v, count as usize));
                        Instruction::new(Op::ConstantComposite, w)
                    };
                    out.push(splat(n.mask, c.mask));
                    out.push(splat(n.shift, c.shift));
                }
                Op::Constant | Op::SpecConstant if narrow.contains_key(&i.operands[0]) => {
                    if let Some(value) = i.operands.get_mut(2) {
                        *value &= NARROW_MASK;
                    }
                    out.push(i);
                }
                _ => out.push(i),
            }
            continue;
        }

        let result = if NO_RESULT_TYPE.contains(&op) || i.operands.len() < 2 {
            None
        } else {
            Some((i.operands[0], i.operands[1]))
        };
        let narrow_result = result.is_some_and(|(t, _)| narrow.contains_key(&t));
        // The operand positions read as signed, and whether the result is masked.
        let (signed, carry): (Vec<usize>, bool) =
            if op == Op::ExtInst && i.operands.get(2) == glsl.as_ref() {
                let Some(code) = i.operands.get(3).and_then(|c| GLOp::from_u32(*c)) else {
                    return Err(unlisted(op));
                };
                let args = 4..i.operands.len();
                let touches = narrow_result || args.clone().any(|a| is_narrow(i.operands[a]));
                if !touches {
                    (Vec::new(), false)
                } else if GLSL_SIGNED.contains(&code) {
                    (args.collect(), GLSL_CARRY.contains(&code))
                } else if GLSL_CARRY.contains(&code) {
                    (Vec::new(), true)
                } else if GLSL_KEEP.contains(&code) {
                    (Vec::new(), false)
                } else {
                    return Err(ConvertError::Reader(format!("{code:?} on 16-bit integers")));
                }
            } else if op == Op::Bitcast {
                let source = i.operands.get(2).copied().is_some_and(is_narrow);
                if source != narrow_result {
                    return Err(ConvertError::Unsupported("Int16Bitcast"));
                }
                (Vec::new(), false)
            } else if SIGNED.contains(&op) {
                ((2..i.operands.len()).collect(), CARRY.contains(&op))
            } else if SIGNED_BASE.contains(&op) {
                (alloc::vec![2], true)
            } else if CARRY.contains(&op) {
                (Vec::new(), true)
            } else if KEEP.contains(&op) || CONTROL.contains(&op) {
                (Vec::new(), false)
            } else {
                let first = if result.is_some() { 2 } else { 0 };
                if narrow_result
                    || i.operands[first.min(i.operands.len())..]
                        .iter()
                        .any(|&w| is_narrow(w))
                {
                    return Err(unlisted(op));
                }
                (Vec::new(), false)
            };
        for at in signed {
            let value = i.operands[at];
            let Some(ty) = types.get(&value).copied() else {
                continue;
            };
            let Some(n) = narrow.get(&ty).copied() else {
                continue;
            };
            // Sign-extended: shifted up to the top, then arithmetically back down.
            let (up, down) = (next(), next());
            out.push(Instruction::new(
                Op::ShiftLeftLogical,
                alloc::vec![ty, up, value, n.shift],
            ));
            out.push(Instruction::new(
                Op::ShiftRightArithmetic,
                alloc::vec![ty, down, up, n.shift],
            ));
            i.operands[at] = down;
        }
        match result {
            Some((ty, id)) if carry && narrow_result => {
                let unmasked = next();
                i.operands[1] = unmasked;
                out.push(i);
                out.push(Instruction::new(
                    Op::BitwiseAnd,
                    alloc::vec![ty, id, unmasked, narrow[&ty].mask],
                ));
            }
            _ => out.push(i),
        }
    }

    let mut words_out: Vec<u32> = spirv[..words::HEADER].to_vec();
    words_out[words::BOUND] = bound;
    for i in out {
        let count = (i.operands.len() + 1) as u32;
        words_out.push((count << words::COUNT_SHIFT) | i.op);
        words_out.extend(i.operands);
    }
    *spirv = words_out;
    Ok(())
}
