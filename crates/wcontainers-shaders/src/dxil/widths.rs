//! Integer and float widths WebGPU lacks. 16-bit integers (minimum precision, and native 16-bit
//! types) become 32-bit words holding the 16-bit value's bits with zeros above: a result that may
//! carry past 16 bits is masked, and an operand read as signed is sign-extended first, so every
//! value equals the 16-bit one; an instruction not listed here is refused, naming it. 64-bit types
//! are refused by name.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;
use naga::{Module, ScalarKind, TypeInner};
use rspirv::binary::Assemble;
use rspirv::dr::{self, Instruction, Operand};
use spirv::{GlslStd450Op as GLOp, Op, StorageClass};

use super::staged::{self, Ids, id, literal};
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
/// The name the GLSL.std.450 extended instruction set is imported by.
const GLSL_STD_450: &str = "GLSL.std.450";
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

/// `OpTypeInt`'s and `OpTypeFloat`'s width, `OpTypeVector`'s component type and count, and the
/// element of an array or matrix type.
const WIDTH: usize = 0;
const VECTOR_COMPONENT: usize = 0;
const VECTOR_COUNT: usize = 1;
const ELEMENT: usize = 0;
/// `OpMemberDecorate`'s decoration, after its target and member.
const MEMBER_DECORATION: usize = 2;
/// `OpExtInst`'s set and instruction, then its arguments.
const EXT_SET: usize = 0;
const EXT_INSTRUCTION: usize = 1;
const EXT_ARGUMENTS: usize = 2;

/// The constants a narrow type needs: its mask and its shift by the narrow width.
#[derive(Clone, Copy)]
struct Narrow {
    mask: u32,
    shift: u32,
}

/// The name of the 64-bit type `spirv` declares, if any.
fn double_width(spirv: &dr::Module) -> Option<&'static str> {
    spirv.types_global_values.iter().find_map(|i| {
        match (i.class.opcode, literal(i, WIDTH) == Some(DOUBLE_BITS)) {
            (Op::TypeInt, true) => Some("Int64"),
            (Op::TypeFloat, true) => Some(crate::refusal::FLOAT_64),
            _ => None,
        }
    })
}

/// Reads `spirv` with naga's reader, refusing a module with 64-bit types by name: where the reader
/// fails on them, and where the module keeps them (WebGPU has neither).
pub(crate) fn read(
    spirv: &dr::Module,
    options: &naga::front::spv::Options,
) -> Result<Module, ConvertError> {
    let double = double_width(spirv);
    let module = naga::front::spv::Frontend::new(spirv.assemble().into_iter(), options)
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
                ScalarKind::Float => crate::refusal::FLOAT_64,
                _ => "Int64",
            }));
        }
    }
    Ok(module)
}

/// A 32-bit constant of type `ty`.
fn constant(ty: u32, id: u32, value: u32) -> Instruction {
    let value = alloc::vec![Operand::LiteralBit32(value)];
    Instruction::new(Op::Constant, Some(ty), Some(id), value)
}

/// An instruction of type `ty` on the ids `operands`.
fn on_ids(op: Op, ty: u32, id: u32, operands: &[u32]) -> Instruction {
    let operands = operands.iter().map(|&o| Operand::IdRef(o)).collect();
    Instruction::new(op, Some(ty), Some(id), operands)
}

/// Makes every 16-bit integer of `spirv` a 32-bit one holding the same bits (see the module's text).
/// A 16-bit integer in memory with a layout (a buffer's member), between stages, or under an
/// instruction not listed here is refused by name.
pub(crate) fn widen(spirv: &mut dr::Module) -> Result<(), ConvertError> {
    let narrow_scalar =
        |i: &Instruction| i.class.opcode == Op::TypeInt && literal(i, WIDTH) == Some(NARROW_BITS);
    if !spirv.types_global_values.iter().any(narrow_scalar) {
        return Ok(());
    }
    let mut ids = Ids::of(spirv);

    // The narrow scalar and vector types, the types holding them, and the GLSL.std.450 set.
    let mut narrow: BTreeMap<u32, Narrow> = BTreeMap::new();
    let mut holding: BTreeSet<u32> = BTreeSet::new();
    let glsl = spirv
        .ext_inst_imports
        .iter()
        .find(|i| i.operands.first() == Some(&Operand::LiteralString(GLSL_STD_450.into())))
        .and_then(|i| i.result_id);
    let laid_out = spirv
        .annotations
        .iter()
        .filter_map(|i| match i.class.opcode {
            Op::MemberDecorate
                if i.operands.get(MEMBER_DECORATION)
                    == Some(&Operand::Decoration(spirv::Decoration::Offset)) =>
            {
                id(i, 0)
            }
            _ if staged::decorates(i, spirv::Decoration::ArrayStride) => id(i, 0),
            _ => None,
        });
    let laid_out: Vec<u32> = laid_out.collect();
    for i in &spirv.types_global_values {
        let Some(result) = i.result_id else {
            continue;
        };
        let held = |n: usize| id(i, n).is_some_and(|t| holding.contains(&t));
        match i.class.opcode {
            Op::TypeInt if narrow_scalar(i) => {
                let n = Narrow {
                    mask: ids.next(),
                    shift: ids.next(),
                };
                narrow.insert(result, n);
                holding.insert(result);
            }
            Op::TypeVector if id(i, VECTOR_COMPONENT).is_some_and(|c| narrow.contains_key(&c)) => {
                let n = Narrow {
                    mask: ids.next(),
                    shift: ids.next(),
                };
                narrow.insert(result, n);
                holding.insert(result);
            }
            Op::TypeArray | Op::TypeRuntimeArray | Op::TypeMatrix if held(ELEMENT) => {
                holding.insert(result);
            }
            Op::TypeStruct if (0..i.operands.len()).any(held) => {
                holding.insert(result);
            }
            Op::TypePointer if held(staged::POINTEE) => {
                holding.insert(result);
            }
            Op::Variable
                if i.result_type.is_some_and(|t| holding.contains(&t))
                    && matches!(
                        i.operands.get(staged::STORAGE_CLASS),
                        Some(Operand::StorageClass(
                            StorageClass::Input | StorageClass::Output
                        ))
                    ) =>
            {
                return Err(ConvertError::Unsupported(
                    crate::refusal::INT_16_STAGE_VALUE,
                ));
            }
            _ => {}
        }
    }
    if laid_out.iter().any(|t| holding.contains(t)) {
        return Err(ConvertError::Unsupported(crate::refusal::INT_16_STORAGE));
    }

    // Each value's type.
    let types: BTreeMap<u32, u32> = spirv
        .all_inst_iter()
        .filter_map(|i| Some((i.result_id?, i.result_type?)))
        .collect();
    let is_narrow = |id: u32| types.get(&id).is_some_and(|t| narrow.contains_key(t));

    spirv
        .capabilities
        .retain(|c| c.operands.first() != Some(&Operand::Capability(spirv::Capability::Int16)));
    let mut globals = Vec::with_capacity(spirv.types_global_values.len());
    for mut i in core::mem::take(&mut spirv.types_global_values) {
        let narrowed = |t: Option<u32>| t.and_then(|t| narrow.get(&t).copied().map(|n| (t, n)));
        match i.class.opcode {
            Op::TypeInt if narrow_scalar(&i) => {
                let n = narrowed(i.result_id);
                i.operands[WIDTH] = Operand::LiteralBit32(WIDE_BITS);
                globals.push(i);
                if let Some((id, n)) = n {
                    globals.push(constant(id, n.mask, NARROW_MASK));
                    globals.push(constant(id, n.shift, NARROW_BITS));
                }
            }
            Op::TypeVector => {
                let vector = narrowed(i.result_id);
                let component = narrowed(id(&i, VECTOR_COMPONENT));
                let count = literal(&i, VECTOR_COUNT).unwrap_or(0) as usize;
                globals.push(i);
                if let (Some((id, n)), Some((_, c))) = (vector, component) {
                    let splat = |of: u32, v: u32| {
                        let operands = alloc::vec![Operand::IdRef(v); count];
                        Instruction::new(Op::ConstantComposite, Some(id), Some(of), operands)
                    };
                    globals.push(splat(n.mask, c.mask));
                    globals.push(splat(n.shift, c.shift));
                }
            }
            Op::Constant | Op::SpecConstant if narrowed(i.result_type).is_some() => {
                if let Some(Operand::LiteralBit32(value)) = i.operands.first_mut() {
                    *value &= NARROW_MASK;
                }
                globals.push(i);
            }
            _ => globals.push(i),
        }
    }
    spirv.types_global_values = globals;

    for block in spirv.functions.iter_mut().flat_map(|f| &mut f.blocks) {
        let mut out = Vec::with_capacity(block.instructions.len());
        for mut i in core::mem::take(&mut block.instructions) {
            let op = i.class.opcode;
            let result = i.result_type.zip(i.result_id);
            let narrow_result = result.is_some_and(|(t, _)| narrow.contains_key(&t));
            let ids_at = |i: &Instruction, at: core::ops::Range<usize>| {
                at.filter_map(|a| id(i, a)).any(is_narrow)
            };
            // The operand positions read as signed, and whether the result is masked.
            let (signed, carry): (Vec<usize>, bool) =
                if op == Op::ExtInst && glsl.is_some() && id(&i, EXT_SET) == glsl {
                    let code = match i.operands.get(EXT_INSTRUCTION) {
                        Some(&Operand::LiteralExtInstInteger(c)) => GLOp::from_u32(c),
                        _ => None,
                    };
                    let Some(code) = code else {
                        return Err(unlisted(op));
                    };
                    let args = EXT_ARGUMENTS..i.operands.len();
                    let touches = narrow_result || ids_at(&i, args.clone());
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
                    if ids_at(&i, 0..1) != narrow_result {
                        return Err(ConvertError::Unsupported(crate::refusal::INT_16_BITCAST));
                    }
                    (Vec::new(), false)
                } else if SIGNED.contains(&op) {
                    ((0..i.operands.len()).collect(), CARRY.contains(&op))
                } else if SIGNED_BASE.contains(&op) {
                    (alloc::vec![0], true)
                } else if CARRY.contains(&op) {
                    (Vec::new(), true)
                } else if KEEP.contains(&op) || CONTROL.contains(&op) {
                    (Vec::new(), false)
                } else {
                    if narrow_result || ids_at(&i, 0..i.operands.len()) {
                        return Err(unlisted(op));
                    }
                    (Vec::new(), false)
                };
            for at in signed {
                let Some((value, ty)) = id(&i, at).and_then(|v| Some((v, *types.get(&v)?))) else {
                    continue;
                };
                let Some(n) = narrow.get(&ty).copied() else {
                    continue;
                };
                // Sign-extended: shifted up to the top, then arithmetically back down.
                let (up, down) = (ids.next(), ids.next());
                out.push(on_ids(Op::ShiftLeftLogical, ty, up, &[value, n.shift]));
                out.push(on_ids(Op::ShiftRightArithmetic, ty, down, &[up, n.shift]));
                i.operands[at] = Operand::IdRef(down);
            }
            match result {
                Some((ty, id)) if carry && narrow_result => {
                    let unmasked = ids.next();
                    i.result_id = Some(unmasked);
                    out.push(i);
                    out.push(on_ids(
                        Op::BitwiseAnd,
                        ty,
                        id,
                        &[unmasked, narrow[&ty].mask],
                    ));
                }
                _ => out.push(i),
            }
        }
        block.instructions = out;
    }
    ids.store(spirv);
    Ok(())
}
