//! dxbc-spirv's intermediate form, read from its serialised token form (its `ir/ir.md`, "Serialization").
//! Only the numbers we lower are named here; they are dxbc-spirv's own enum values (`ir/ir.h`).

use alloc::string::String;
use alloc::vec::Vec;

/// One instruction. Its id is its index in [`Ir::ops`] plus one; id 0 is null.
#[derive(Clone, Debug)]
pub struct Op {
    pub code: u16,
    pub flags: u8,
    pub ty: Ty,
    /// Operands naming other instructions, by id (0 for null).
    pub refs: Vec<u32>,
    /// Literal operands, after the references.
    pub lits: Vec<u64>,
}

/// A return type: arrays of a struct of scalars or vectors. One member is a plain scalar or vector;
/// none is void.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ty {
    /// Element counts, innermost first; a last count of zero is an unbounded array.
    pub dims: Vec<u32>,
    pub members: Vec<Basic>,
}

/// A scalar type and a vector size (1 to 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Basic {
    pub scalar: u8,
    pub size: u8,
}

impl Ty {
    pub fn is_void(&self) -> bool {
        self.members.is_empty()
    }
    /// The single scalar or vector of a type without arrays and with one member.
    pub fn basic(&self) -> Option<Basic> {
        (self.dims.is_empty() && self.members.len() == 1).then(|| self.members[0])
    }
}

/// Scalar types.
pub mod scalar {
    pub const UNKNOWN: u8 = 1;
    pub const BOOL: u8 = 2;
    pub const I32: u8 = 5;
    pub const U32: u8 = 9;
    pub const F32: u8 = 12;
}

/// Opcodes.
#[allow(missing_docs)]
pub mod code {
    pub const ENTRY_POINT: u16 = 1;
    pub const SEMANTIC: u16 = 2;
    pub const DEBUG_NAME: u16 = 3;
    pub const DEBUG_MEMBER_NAME: u16 = 4;
    pub const CONSTANT: u16 = 8;
    pub const UNDEF: u16 = 9;
    pub const SET_CS_WORKGROUP_SIZE: u16 = 16;
    pub const SET_PS_EARLY_FRAGMENT_TEST: u16 = 21;
    pub const SET_PS_DEPTH_GREATER_EQUAL: u16 = 22;
    pub const SET_PS_DEPTH_LESS_EQUAL: u16 = 23;
    pub const SET_FP_MODE: u16 = 27;
    pub const DCL_INPUT: u16 = 32;
    pub const DCL_INPUT_BUILT_IN: u16 = 33;
    pub const DCL_OUTPUT: u16 = 34;
    pub const DCL_OUTPUT_BUILT_IN: u16 = 35;
    pub const DCL_SAMPLER: u16 = 38;
    pub const DCL_CBV: u16 = 39;
    pub const DCL_SRV: u16 = 40;
    pub const DCL_UAV: u16 = 41;
    pub const DCL_UAV_COUNTER: u16 = 42;
    pub const DCL_LDS: u16 = 43;
    pub const DCL_SCRATCH: u16 = 44;
    pub const DCL_TMP: u16 = 45;
    pub const DCL_PARAM: u16 = 46;
    pub const DCL_INPUT_TARGET: u16 = 48;
    pub const LAST_DECLARATIVE: u16 = 63;
    pub const FUNCTION: u16 = 64;
    pub const FUNCTION_END: u16 = 65;
    pub const FUNCTION_CALL: u16 = 66;
    pub const LABEL: u16 = 96;
    pub const BRANCH: u16 = 97;
    pub const BRANCH_CONDITIONAL: u16 = 98;
    pub const SWITCH: u16 = 99;
    pub const UNREACHABLE: u16 = 100;
    pub const RETURN: u16 = 101;
    pub const PHI: u16 = 102;
    pub const BARRIER: u16 = 160;
    pub const CONVERT_F_TO_F: u16 = 192;
    pub const CONVERT_F_TO_I: u16 = 193;
    pub const CONVERT_I_TO_F: u16 = 194;
    pub const CONVERT_I_TO_I: u16 = 195;
    pub const CONVERT_F32_TO_PACKED_F16: u16 = 196;
    pub const CONVERT_PACKED_F16_TO_F32: u16 = 197;
    pub const CAST: u16 = 198;
    pub const COMPOSITE_EXTRACT: u16 = 225;
    pub const COMPOSITE_CONSTRUCT: u16 = 226;
    pub const PARAM_LOAD: u16 = 288;
    pub const SCRATCH_LOAD: u16 = 291;
    pub const SCRATCH_STORE: u16 = 292;
    pub const LDS_LOAD: u16 = 293;
    pub const LDS_STORE: u16 = 294;
    pub const INPUT_LOAD: u16 = 297;
    pub const OUTPUT_LOAD: u16 = 298;
    pub const OUTPUT_STORE: u16 = 299;
    pub const DESCRIPTOR_LOAD: u16 = 300;
    pub const BUFFER_LOAD: u16 = 301;
    pub const BUFFER_STORE: u16 = 302;
    pub const BUFFER_QUERY_SIZE: u16 = 303;
    pub const CONSTANT_LOAD: u16 = 306;
    pub const LDS_ATOMIC: u16 = 320;
    pub const BUFFER_ATOMIC: u16 = 321;
    pub const IMAGE_ATOMIC: u16 = 322;
    pub const COUNTER_ATOMIC: u16 = 323;
    pub const IMAGE_LOAD: u16 = 352;
    pub const IMAGE_STORE: u16 = 353;
    pub const IMAGE_QUERY_SIZE: u16 = 354;
    pub const IMAGE_QUERY_MIPS: u16 = 355;
    pub const IMAGE_QUERY_SAMPLES: u16 = 356;
    pub const IMAGE_SAMPLE: u16 = 357;
    pub const IMAGE_GATHER: u16 = 358;
    pub const IMAGE_COMPUTE_LOD: u16 = 359;
    pub const DEMOTE: u16 = 448;
    pub const INTERPOLATE_AT_CENTROID: u16 = 480;
    pub const INTERPOLATE_AT_SAMPLE: u16 = 481;
    pub const INTERPOLATE_AT_OFFSET: u16 = 482;
    pub const DERIV_X: u16 = 512;
    pub const DERIV_Y: u16 = 513;
    pub const F_EQ: u16 = 576;
    pub const F_NE: u16 = 577;
    pub const F_LT: u16 = 578;
    pub const F_LE: u16 = 579;
    pub const F_GT: u16 = 580;
    pub const F_GE: u16 = 581;
    pub const F_IS_NAN: u16 = 582;
    pub const I_EQ: u16 = 583;
    pub const I_NE: u16 = 584;
    pub const S_LT: u16 = 585;
    pub const S_LE: u16 = 586;
    pub const S_GT: u16 = 587;
    pub const S_GE: u16 = 588;
    pub const U_LT: u16 = 589;
    pub const U_LE: u16 = 590;
    pub const U_GT: u16 = 591;
    pub const U_GE: u16 = 592;
    pub const B_AND: u16 = 608;
    pub const B_OR: u16 = 609;
    pub const B_EQ: u16 = 610;
    pub const B_NE: u16 = 611;
    pub const B_NOT: u16 = 612;
    pub const SELECT: u16 = 640;
    pub const F_ABS: u16 = 672;
    pub const F_NEG: u16 = 673;
    pub const F_ADD: u16 = 674;
    pub const F_SUB: u16 = 675;
    pub const F_MUL: u16 = 676;
    pub const F_MAD: u16 = 678;
    pub const F_DIV: u16 = 680;
    pub const F_RCP: u16 = 681;
    pub const F_SQRT: u16 = 682;
    pub const F_RSQ: u16 = 683;
    pub const F_EXP2: u16 = 684;
    pub const F_LOG2: u16 = 685;
    pub const F_FRACT: u16 = 686;
    pub const F_ROUND: u16 = 687;
    pub const F_MIN: u16 = 688;
    pub const F_MAX: u16 = 689;
    pub const F_DOT: u16 = 690;
    pub const F_CLAMP: u16 = 692;
    pub const F_SIN: u16 = 693;
    pub const F_COS: u16 = 694;
    pub const F_POW: u16 = 695;
    pub const F_SGN: u16 = 697;
    pub const I_AND: u16 = 704;
    pub const I_OR: u16 = 705;
    pub const I_XOR: u16 = 706;
    pub const I_NOT: u16 = 707;
    pub const I_BIT_INSERT: u16 = 708;
    pub const U_BIT_EXTRACT: u16 = 709;
    pub const S_BIT_EXTRACT: u16 = 710;
    pub const I_SHL: u16 = 711;
    pub const S_SHR: u16 = 712;
    pub const U_SHR: u16 = 713;
    pub const I_BIT_COUNT: u16 = 714;
    pub const I_BIT_REVERSE: u16 = 715;
    pub const I_FIND_LSB: u16 = 716;
    pub const S_FIND_MSB: u16 = 717;
    pub const U_FIND_MSB: u16 = 718;
    pub const I_ADD: u16 = 736;
    pub const I_ADD_CARRY: u16 = 737;
    pub const I_SUB: u16 = 738;
    pub const I_SUB_BORROW: u16 = 739;
    pub const I_ABS: u16 = 740;
    pub const I_NEG: u16 = 741;
    pub const I_MUL: u16 = 742;
    pub const S_MUL_EXTENDED: u16 = 743;
    pub const U_MUL_EXTENDED: u16 = 744;
    pub const U_DIV: u16 = 745;
    pub const U_MOD: u16 = 746;
    pub const S_MIN: u16 = 747;
    pub const S_MAX: u16 = 748;
    pub const S_CLAMP: u16 = 749;
    pub const U_MIN: u16 = 750;
    pub const U_MAX: u16 = 751;
    pub const U_CLAMP: u16 = 752;
}

/// Built-in values (`ir::BuiltIn`).
#[allow(missing_docs)]
pub mod builtin {
    pub const POSITION: u64 = 0;
    pub const CLIP_DISTANCE: u64 = 1;
    pub const CULL_DISTANCE: u64 = 2;
    pub const VERTEX_ID: u64 = 3;
    pub const INSTANCE_ID: u64 = 4;
    pub const PRIMITIVE_ID: u64 = 5;
    pub const LAYER_INDEX: u64 = 6;
    pub const VIEWPORT_INDEX: u64 = 7;
    pub const SAMPLE_COUNT: u64 = 15;
    pub const SAMPLE_ID: u64 = 16;
    pub const SAMPLE_POSITION: u64 = 17;
    pub const SAMPLE_MASK: u64 = 18;
    pub const IS_FRONT_FACE: u64 = 19;
    pub const DEPTH: u64 = 20;
    pub const STENCIL_REF: u64 = 21;
    pub const IS_FULLY_COVERED: u64 = 22;
    pub const WORKGROUP_ID: u64 = 23;
    pub const GLOBAL_THREAD_ID: u64 = 24;
    pub const LOCAL_THREAD_ID: u64 = 25;
    pub const LOCAL_THREAD_INDEX: u64 = 26;
}

/// Resource kinds (`ir::ResourceKind`).
#[allow(missing_docs)]
pub mod kind {
    pub const BUFFER_TYPED: u64 = 0;
    pub const BUFFER_STRUCTURED: u64 = 1;
    pub const BUFFER_RAW: u64 = 2;
    pub const IMAGE_1D: u64 = 3;
    pub const IMAGE_1D_ARRAY: u64 = 4;
    pub const IMAGE_2D: u64 = 5;
    pub const IMAGE_2D_ARRAY: u64 = 6;
    pub const IMAGE_2D_MS: u64 = 7;
    pub const IMAGE_2D_MS_ARRAY: u64 = 8;
    pub const IMAGE_CUBE: u64 = 9;
    pub const IMAGE_CUBE_ARRAY: u64 = 10;
    pub const IMAGE_3D: u64 = 11;
}

/// Shader stages (`ir::ShaderStage`, one bit each).
#[allow(missing_docs)]
pub mod stage {
    pub const PIXEL: u64 = 1;
    pub const VERTEX: u64 = 2;
    pub const GEOMETRY: u64 = 4;
    pub const HULL: u64 = 8;
    pub const DOMAIN: u64 = 16;
    pub const COMPUTE: u64 = 32;
}

/// Interpolation mode bits (`ir::InterpolationMode`).
pub mod interp {
    pub const FLAT: u64 = 1;
    pub const CENTROID: u64 = 2;
    pub const SAMPLE: u64 = 4;
    pub const NO_PERSPECTIVE: u64 = 8;
}

/// Structured constructs a label declares (`ir::Construct`).
pub mod construct {
    pub const SELECTION: u64 = 1;
    pub const LOOP: u64 = 2;
}

/// A decoded shader.
#[derive(Debug)]
pub struct Ir {
    pub ops: Vec<Op>,
}

impl Ir {
    /// The instruction with id `id`; a null or out-of-range id is malformed IR.
    pub fn op(&self, id: u32) -> Result<&Op, crate::ConvertError> {
        (id as usize)
            .checked_sub(1)
            .and_then(|i| self.ops.get(i))
            .ok_or_else(|| crate::ConvertError::Malformed(alloc::format!("no instruction {id}")))
    }

    /// Reads the serialised token form.
    pub fn decode(data: &[u8]) -> Option<Ir> {
        let mut r = Reader { data, at: 0 };
        let count = r.vle()? as usize;
        let mut ops = Vec::with_capacity(count);
        while r.at < data.len() {
            let id = ops.len() as u64 + 1;
            let token = r.vle()?;
            let operands = r.vle()? as usize;
            let ty = r.ty()?;
            let code = (token & 0x3ff) as u16;
            let flags = (token >> 10) as u8;
            let first_literal = first_literal(code, operands);
            let mut refs = Vec::with_capacity(first_literal);
            for _ in 0..first_literal {
                let delta = r.vle()?;
                refs.push(if delta == 0 {
                    0
                } else {
                    u32::try_from(id.wrapping_sub(delta)).ok()?
                });
            }
            let mut lits = Vec::with_capacity(operands - first_literal);
            for _ in first_literal..operands {
                lits.push(r.vle()?);
            }
            ops.push(Op {
                code,
                flags,
                ty,
                refs,
                lits,
            });
        }
        (ops.len() == count).then_some(Ir { ops })
    }
}

/// Where an instruction's literal operands start (`Op::getFirstLiteralOperandIndex`).
fn first_literal(code: u16, operands: usize) -> usize {
    match code {
        8 | 160 | 416 | 417 | 544 | 545 => 0,
        2..=4 | 16..=20 | 24..=27 | 32..=38 | 39..=41 | 47 | 48 | 512 | 513 | 687 | 384 | 136 => {
            1.min(operands)
        }
        301 | 304 => 2.min(operands),
        302 | 305 => 3.min(operands),
        1 | 96 | 320..=324 => operands.saturating_sub(1),
        358 => 6.min(operands),
        _ => operands,
    }
}

/// Decodes a string packed into literals, eight bytes each, low byte first, ended by a zero byte.
pub fn literal_string(lits: &[u64]) -> String {
    let mut s = String::new();
    'outer: for lit in lits {
        for i in 0..8 {
            let byte = (lit >> (8 * i)) as u8;
            if byte == 0 {
                break 'outer;
            }
            s.push(byte as char);
        }
    }
    s
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    /// A variable-length token: the leading one bits of the first byte count the bytes that follow;
    /// big-endian, sign-extended.
    fn vle(&mut self) -> Option<u64> {
        let header = *self.data.get(self.at)?;
        let len = header.leading_ones() as usize;
        let bytes = self.data.get(self.at + 1..self.at + 1 + len)?;
        self.at += 1 + len;
        if len == 0 {
            return Some(((header as i8) << 1 >> 1) as i64 as u64);
        }
        let mut sym: u64 = if len < 7 {
            let sign = 0x40u8 >> len;
            let value = (header & (sign - 1)) as u64;
            if header & sign != 0 {
                value | !((sign as u64) - 1)
            } else {
                value
            }
        } else if len == 7 {
            (bytes[0] >> 7) as u64 * u64::MAX
        } else {
            0
        };
        for &b in bytes {
            sym = (sym << 8) | b as u64;
        }
        Some(sym)
    }

    fn ty(&mut self) -> Option<Ty> {
        let header = self.vle()?;
        let dims = (header & 3) as usize;
        let members = (header >> 2) as usize;
        let mut ty = Ty::default();
        for _ in 0..dims {
            ty.dims.push(u32::try_from(self.vle()?).ok()?);
        }
        for _ in 0..members {
            let b = *self.data.get(self.at)?;
            self.at += 1;
            ty.members.push(Basic {
                scalar: b & 0x1f,
                size: (b >> 5) + 1,
            });
        }
        Some(ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_decode_as_dxbc_spirv_encodes_them() {
        let mut r = Reader {
            data: &[0x05, 0x7f, 0x80, 0x80, 0xbf, 0xff, 0xc0, 0x01, 0x00],
            at: 0,
        };
        assert_eq!(r.vle(), Some(5));
        assert_eq!(r.vle(), Some(u64::MAX));
        assert_eq!(r.vle(), Some(0x80));
        assert_eq!(r.vle(), Some(u64::MAX));
        assert_eq!(r.vle(), Some(0x100));
    }
}
