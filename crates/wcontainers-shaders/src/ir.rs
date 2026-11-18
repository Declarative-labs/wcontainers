//! dxbc-spirv's intermediate form, read from its serialised token form (its `ir/ir.md`, "Serialization").
//! The numbers come from dxbc-spirv's own enums (`ir/ir.h`), through build.rs.

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

// dxbc-spirv's enums (`ScalarType`, `OpCode`, `OpFlag`, `BuiltIn`, `ResourceKind`, `ShaderStage`,
// `PrimitiveType`, `InterpolationMode`, `Construct`, `UavFlag`) as modules of constants, written by build.rs
// from its `ir/ir.h`.
include!(concat!(env!("OUT_DIR"), "/ir_codes.rs"));

/// A decoded shader.
#[derive(Debug)]
pub struct Ir {
    pub ops: Vec<Op>,
    /// Shader model 5.1, which only Direct3D 12 runs: resources are numbered by its rule.
    pub spaces: bool,
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
        (ops.len() == count).then_some(Ir { ops, spaces: false })
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
