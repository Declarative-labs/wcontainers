//! The decoder wrapper: one instruction word to our [`Instruction`]. `yaxpeax-arm` does the decoding; the
//! raw word settles what its operand types leave open and the reserved encodings it accepts.

use crate::instruction::*;
use yaxpeax_arch::{Decoder, U8Reader};
use yaxpeax_arm::armv8::a64::{InstDecoder, Opcode, Operand as Op, ShiftStyle, SizeCode};

/// Decode one instruction word.
pub fn decode(word: u32) -> Instruction {
    let Ok(decoded) = InstDecoder::default().decode(&mut U8Reader::new(&word.to_le_bytes())) else {
        return Instruction::Undefined;
    };
    let ops = decoded.operands;
    convert(word, decoded.opcode, &ops).unwrap_or_else(|| {
        if crate::forms::presented(word) {
            Instruction::Unsupported
        } else {
            Instruction::Undefined
        }
    })
}

fn bits(word: u32, low: u32, width: u32) -> u32 {
    (word >> low) & ((1 << width) - 1)
}

fn width(size: SizeCode) -> Width {
    match size {
        SizeCode::W => Width::W32,
        SizeCode::X => Width::X64,
    }
}

/// A register field where 31 means the zero register.
fn zr(n: u32) -> Reg {
    if n == 31 { Reg::Zero } else { Reg::X(n as u8) }
}

/// A register field where 31 means the stack pointer.
fn sp(n: u32) -> Reg {
    if n == 31 { Reg::Sp } else { Reg::X(n as u8) }
}

fn reg(op: &Op) -> Option<(Width, Reg)> {
    match *op {
        Op::Register(size, n) => Some((width(size), zr(n.into()))),
        Op::RegisterOrSP(size, n) => Some((width(size), sp(n.into()))),
        _ => None,
    }
}

fn shift(style: ShiftStyle) -> Option<Shift> {
    Some(match style {
        ShiftStyle::LSL => Shift::Lsl,
        ShiftStyle::LSR => Shift::Lsr,
        ShiftStyle::ASR => Shift::Asr,
        ShiftStyle::ROR => Shift::Ror,
        _ => return None,
    })
}

/// The extend named by a 3 bit `option` field.
fn extend(option: u32) -> Extend {
    Extend {
        signed: option & 4 != 0,
        from_bits: 8 << (option & 3),
    }
}

fn condition(op: &Op) -> Option<Cond> {
    match *op {
        Op::ConditionCode(c) => Some(Cond(c)),
        _ => None,
    }
}

fn immediate(op: &Op) -> Option<u64> {
    match *op {
        Op::Immediate(v) => Some(v.into()),
        Op::Imm64(v) => Some(v),
        Op::Imm16(v) => Some(v.into()),
        Op::ImmShift(v, s) => Some(u64::from(v) << s),
        _ => None,
    }
}

fn pc_offset(op: &Op) -> Option<i64> {
    match *op {
        Op::PCOffset(v) => Some(v),
        _ => None,
    }
}

/// The second source of an add, subtract or logical instruction, read from the raw word for the register
/// forms so that the shifted and extended forms cannot be confused.
fn second(word: u32, w: Width, op: &Op) -> Option<Operand> {
    if let Some(v) = immediate(op) {
        return Some(Operand::Imm(v));
    }
    let rm = zr(bits(word, 16, 5));
    let extended = bits(word, 24, 5) == 0b01011 && bits(word, 21, 1) == 1;
    if extended {
        let amount = bits(word, 10, 3);
        if amount > 4 {
            return None;
        }
        return Some(Operand::Extended {
            rm,
            extend: extend(bits(word, 13, 3)),
            amount: amount as u8,
        });
    }
    let Op::RegShift(style, amount, _, _) = *op else {
        return None;
    };
    if w == Width::W32 && amount >= 32 {
        return None;
    }
    Some(Operand::Shifted {
        rm,
        shift: shift(style)?,
        amount,
    })
}

fn system_register(word: u32) -> Option<SystemRegister> {
    // op0 is 2 + o0; the remaining fields name the register.
    let key = (
        bits(word, 19, 1),
        bits(word, 16, 3),
        bits(word, 12, 4),
        bits(word, 8, 4),
        bits(word, 5, 3),
    );
    Some(match key {
        (1, 3, 4, 2, 0) => SystemRegister::Nzcv,
        (1, 3, 13, 0, 2) => SystemRegister::TpidrEl0,
        _ => return None,
    })
}

/// The size in bytes and number of a SIMD and floating point register used whole.
fn vector(op: &Op) -> Option<(u8, u8)> {
    use yaxpeax_arm::armv8::a64::SIMDSizeCode as S;
    match *op {
        Op::SIMDRegister(size, n) => Some((
            match size {
                S::B => 1,
                S::H => 2,
                S::S => 4,
                S::D => 8,
                S::Q => 16,
            },
            n as u8,
        )),
        _ => None,
    }
}

/// The address of a load or store of `bytes` bytes, or `None` for forms with no address we translate.
fn address(word: u32, op: &Op, bytes: u8) -> Option<Address> {
    Some(match *op {
        Op::RegPreIndex(base, offset, false) => Address::Offset {
            base: sp(base.into()),
            offset: offset.into(),
        },
        Op::RegPreIndex(base, offset, true) => Address::PreIndex {
            base: sp(base.into()),
            offset: offset.into(),
        },
        Op::RegPostIndex(base, offset) => Address::PostIndex {
            base: sp(base.into()),
            offset: offset.into(),
        },
        Op::RegRegOffset(base, ..) => {
            let option = bits(word, 13, 3);
            if option & 2 == 0 {
                return None;
            }
            let scale = bytes.trailing_zeros() as u8;
            Address::Indexed {
                base: sp(base.into()),
                index: zr(bits(word, 16, 5)),
                extend: extend(option),
                shift: if bits(word, 12, 1) == 1 { scale } else { 0 },
            }
        }
        Op::PCOffset(offset) => Address::Literal { offset },
        _ => return None,
    })
}

/// An AdvSIMD modified immediate word: its operation and its 64 bit pattern, from Arm's
/// `AdvSIMDExpandImm`. `None` for the half precision `fmov`, which needs a feature we do not offer.
fn modified_immediate(word: u32) -> Option<(VectorImmediate, u64)> {
    let (op, cmode, o2) = (bits(word, 29, 1), bits(word, 12, 4), bits(word, 11, 1));
    if o2 == 1 {
        return None;
    }
    let imm8 = u64::from((bits(word, 16, 3) << 5) | bits(word, 5, 5));
    let replicate =
        |value: u64, bits: u32| (0..64 / bits).fold(0, |all, i| all | value << (i * bits));
    let imm = match cmode >> 1 {
        0 => replicate(imm8, 32),
        1 => replicate(imm8 << 8, 32),
        2 => replicate(imm8 << 16, 32),
        3 => replicate(imm8 << 24, 32),
        4 => replicate(imm8, 16),
        5 => replicate(imm8 << 8, 16),
        6 if cmode & 1 == 0 => replicate(imm8 << 8 | 0xff, 32),
        6 => replicate(imm8 << 16 | 0xffff, 32),
        _ => match (cmode & 1, op) {
            (0, 0) => replicate(imm8, 8),
            (0, _) => (0..8).fold(0, |all, i| all | (((imm8 >> i) & 1) * (0xff << (8 * i)))),
            (_, 0) => {
                // a:NOT(b):bbbbb:c:defgh, then 19 zeros
                let (a, b, low) = (imm8 >> 7, (imm8 >> 6) & 1, imm8 & 0x3f);
                let single = a << 31 | (b ^ 1) << 30 | (b * 0x1f) << 25 | low << 19;
                replicate(single, 32)
            }
            _ => {
                // a:NOT(b):bbbbbbbb:cdefgh, then 48 zeros
                let (a, b, low) = (imm8 >> 7, (imm8 >> 6) & 1, imm8 & 0x3f);
                a << 63 | (b ^ 1) << 62 | (b * 0xff) << 54 | low << 48
            }
        },
    };
    let logical = cmode < 0b1100 && cmode & 1 == 1;
    Some(match (logical, op) {
        (false, 0) => (VectorImmediate::Move, imm),
        // mvni, and movi of 64 bit patterns
        (false, _) if cmode >= 0b1110 => (VectorImmediate::Move, imm),
        (false, _) => (VectorImmediate::Move, !imm),
        (true, 0) => (VectorImmediate::Or, imm),
        (true, _) => (VectorImmediate::Clear, imm),
    })
}

/// Scalar floating point and AdvSIMD data processing. Operands come from the raw word. Reserved
/// arrangements of what we translate are undefined, as Arm's decoding says; `None` for words we do not
/// translate yet.
fn convert_vector(word: u32, opcode: Opcode) -> Option<Instruction> {
    use Instruction as I;
    use Opcode as O;
    let (rd, rn, rm) = (
        bits(word, 0, 5) as u8,
        bits(word, 5, 5) as u8,
        bits(word, 16, 5) as u8,
    );
    let whole = bits(word, 30, 1) == 1;
    let size = bits(word, 22, 2);
    let arrangement = |element: u32| Arrangement {
        element: 1 << element,
        whole,
    };
    let undefined = Some(I::Undefined);
    // 64 bit elements need the whole register.
    let sized = (size < 3 || whole).then(|| arrangement(size));
    if word & 0x9ff8_0400 == 0x0f00_0400 {
        let (op, imm) = modified_immediate(word)?;
        return Some(I::VectorImmediate { op, rd, imm, whole });
    }
    // Three registers of the same arrangement.
    if word & 0x9f20_0400 == 0x0e20_0400 {
        let u = bits(word, 29, 1);
        let op = match (bits(word, 11, 5), u) {
            (0b00011, _) => {
                let op = match (u, size) {
                    (0, 0) => VectorBinary::And,
                    (0, 1) => VectorBinary::AndNot,
                    (0, 2) => VectorBinary::Or,
                    (0, _) => VectorBinary::OrNot,
                    (_, 0) => VectorBinary::Xor,
                    (_, 1) => VectorBinary::Select,
                    (_, 2) => VectorBinary::InsertIfTrue,
                    _ => VectorBinary::InsertIfFalse,
                };
                return Some(I::VectorBinary {
                    op,
                    arrangement: arrangement(0),
                    rd,
                    rn,
                    rm,
                });
            }
            (0b10000, 0) => VectorBinary::Add,
            (0b10000, _) => VectorBinary::Sub,
            (0b10011, 0) if size == 3 => return undefined,
            (0b10011, 0) => VectorBinary::Mul,
            (0b10001, 0) => VectorBinary::Test,
            (0b10001, _) => VectorBinary::Equal,
            (0b00110, 0) => VectorBinary::SignedGreater,
            (0b00110, _) => VectorBinary::UnsignedGreater,
            (0b00111, 0) => VectorBinary::SignedGreaterOrEqual,
            (0b00111, _) => VectorBinary::UnsignedGreaterOrEqual,
            _ => return None,
        };
        let Some(arrangement) = sized else {
            return undefined;
        };
        return Some(I::VectorBinary {
            op,
            arrangement,
            rd,
            rn,
            rm,
        });
    }
    // Two register miscellaneous.
    if word & 0x9f3e_0c00 == 0x0e20_0800 {
        let op = match (bits(word, 12, 5), bits(word, 29, 1), size) {
            (0b00101, 0, 0) => VectorUnary::CountBits,
            (0b00101, 1, 0) => VectorUnary::Not,
            (0b00101, 0, _) | (0b00101, 1, 2..) => return undefined,
            _ => return None,
        };
        return Some(I::VectorUnary {
            op,
            arrangement: arrangement(0),
            rd,
            rn,
        });
    }
    // Across lanes: no 64 bit elements, and 32 bit ones only in the whole register.
    if word & 0x9f3e_0c00 == 0x0e30_0800 {
        let op = match opcode {
            O::ADDV => VectorAcross::Add,
            O::SMAXV => VectorAcross::SignedMax,
            O::SMINV => VectorAcross::SignedMin,
            O::UMAXV => VectorAcross::UnsignedMax,
            O::UMINV => VectorAcross::UnsignedMin,
            _ => return None,
        };
        if size == 3 || (size == 2 && !whole) {
            return undefined;
        }
        return Some(I::VectorAcross {
            op,
            arrangement: arrangement(size),
            rd,
            rn,
        });
    }
    // Copies between elements and general registers. imm5's lowest set bit gives the element size; the
    // bits above it, the index.
    if word & 0x9fe0_8400 == 0x0e00_0400 {
        let imm5 = bits(word, 16, 5);
        let element = imm5.trailing_zeros();
        if element > 3 {
            return undefined;
        }
        let bytes = 1u8 << element;
        let index = (imm5 >> (element + 1)) as u8;
        let general = zr(rn.into());
        let to_general = zr(rd.into());
        return Some(match opcode {
            O::DUP if element == 3 && !whole => return undefined,
            O::DUP if bits(word, 11, 4) == 0 => I::DuplicateElement {
                arrangement: arrangement(element),
                index,
                rd,
                rn,
            },
            O::DUP => I::DuplicateGeneral {
                arrangement: arrangement(element),
                rd,
                rn: general,
            },
            O::UMOV if (element == 3) == whole => I::MoveToGeneral {
                bytes,
                index,
                signed: None,
                rd: to_general,
                rn,
            },
            O::SMOV if element < 2 || (element == 2 && whole) => I::MoveToGeneral {
                bytes,
                index,
                signed: Some(if whole { Width::X64 } else { Width::W32 }),
                rd: to_general,
                rn,
            },
            O::INS if !whole => return undefined,
            O::INS if bits(word, 29, 1) == 0 => I::InsertGeneral {
                bytes,
                index,
                rd,
                rn: general,
            },
            O::INS => I::InsertElement {
                bytes,
                to: index,
                from: (bits(word, 11, 4) >> element) as u8,
                rd,
                rn,
            },
            O::UMOV | O::SMOV => return undefined,
            _ => return None,
        });
    }
    if word & 0xbf20_8400 == 0x2e00_0000 {
        let index = bits(word, 11, 4) as u8;
        if opcode != O::EXT {
            return None;
        }
        if bits(word, 22, 2) != 0 || (!whole && index >= 8) {
            return undefined;
        }
        return Some(I::ExtractVector {
            whole,
            index,
            rd,
            rn,
            rm,
        });
    }
    // fmov between general and SIMD and floating point registers, all but half precision.
    if opcode == O::FMOV && word & 0x7f26_fc00 == 0x1e26_0000 {
        let to_general = bits(word, 16, 1) == 0;
        let (bytes, index) = match (bits(word, 31, 1), bits(word, 22, 2), bits(word, 19, 2)) {
            (0, 0, 0) => (4, 0),
            (1, 1, 0) => (8, 0),
            (1, 2, 1) => (8, 1),
            _ => return None,
        };
        return Some(match (to_general, index) {
            (true, _) => I::MoveToGeneral {
                bytes,
                index,
                signed: None,
                rd: zr(rd.into()),
                rn,
            },
            (false, 0) => I::MoveFromGeneral {
                bytes,
                rd,
                rn: zr(rn.into()),
            },
            (false, _) => I::InsertGeneral {
                bytes,
                index,
                rd,
                rn: zr(rn.into()),
            },
        });
    }
    None
}

/// A logical immediate whose bit mask is reserved: no element size, or an element of all ones.
fn reserved_bit_mask(word: u32) -> bool {
    if bits(word, 23, 6) != 0b100100 {
        return false;
    }
    let (sf, n, imms) = (bits(word, 31, 1), bits(word, 22, 1), bits(word, 10, 6));
    let combined = (n << 6) | (!imms & 0x3f);
    if (sf == 0 && n == 1) || combined < 2 {
        return true;
    }
    let levels = (1 << (31 - combined.leading_zeros())) - 1;
    imms & levels == levels
}

fn convert(word: u32, opcode: Opcode, ops: &[Op; 4]) -> Option<Instruction> {
    use Instruction as I;
    use Opcode as O;
    let r0 = reg(&ops[0]);
    let (w, rd) = r0.unwrap_or((Width::X64, Reg::Zero));
    let rn = || reg(&ops[1]).map(|(_, r)| r);
    let rm = || reg(&ops[2]).map(|(_, r)| r);
    if reserved_bit_mask(word) {
        return Some(I::Undefined);
    }
    // Scalar floating point and AdvSIMD data processing.
    if bits(word, 25, 3) == 0b111 {
        return convert_vector(word, opcode);
    }
    // The hint space is all hints: pointer signing and branch marks included, as on a CPU without them.
    if word & 0xffff_f01f == 0xd503_201f {
        return Some(I::Nothing);
    }
    Some(match opcode {
        O::ADD | O::ADDS | O::SUB | O::SUBS => I::AddSub {
            width: w,
            subtract: matches!(opcode, O::SUB | O::SUBS),
            set_flags: matches!(opcode, O::ADDS | O::SUBS),
            rd,
            rn: rn()?,
            operand: second(word, w, &ops[2])?,
        },
        O::ADC | O::ADCS | O::SBC | O::SBCS => I::AddSubCarry {
            width: w,
            subtract: matches!(opcode, O::SBC | O::SBCS),
            set_flags: matches!(opcode, O::ADCS | O::SBCS),
            rd,
            rn: rn()?,
            rm: rm()?,
        },
        O::AND | O::ANDS | O::ORR | O::EOR | O::BIC | O::BICS | O::ORN | O::EON => {
            // Vector forms share these opcodes.
            r0?;
            I::Logical {
                width: w,
                op: match opcode {
                    O::AND | O::ANDS | O::BIC | O::BICS => Logic::And,
                    O::ORR | O::ORN => Logic::Or,
                    _ => Logic::Xor,
                },
                invert: matches!(opcode, O::BIC | O::BICS | O::ORN | O::EON),
                set_flags: matches!(opcode, O::ANDS | O::BICS),
                rd,
                rn: rn()?,
                operand: second(word, w, &ops[2])?,
            }
        }
        O::MOVZ | O::MOVN | O::MOVK => {
            let Op::ImmShift(imm, shift) = ops[1] else {
                return None;
            };
            I::MoveWide {
                width: w,
                op: match opcode {
                    O::MOVZ => MoveWide::Zero,
                    O::MOVN => MoveWide::Not,
                    _ => MoveWide::Keep,
                },
                rd,
                imm,
                shift,
            }
        }
        O::SBFM | O::BFM | O::UBFM => I::Bitfield {
            width: w,
            op: match opcode {
                O::SBFM => Bitfield::Signed,
                O::BFM => Bitfield::Insert,
                _ => Bitfield::Unsigned,
            },
            rd,
            rn: rn()?,
            immr: immediate(&ops[2])? as u8,
            imms: immediate(&ops[3])? as u8,
        },
        O::EXTR => {
            let lsb = immediate(&ops[3])? as u8;
            if w == Width::W32 && lsb >= 32 {
                return None;
            }
            I::Extract {
                width: w,
                rd,
                rn: rn()?,
                rm: rm()?,
                lsb,
            }
        }
        O::ADR | O::ADRP => I::PcRelative {
            rd,
            page: opcode == O::ADRP,
            offset: pc_offset(&ops[1])?,
        },
        O::CCMP | O::CCMN => I::CondCompare {
            width: w,
            negate: opcode == O::CCMN,
            rn: reg(&ops[0])?.1,
            operand: match reg(&ops[1]) {
                Some((_, rm)) => Operand::Shifted {
                    rm,
                    shift: Shift::Lsl,
                    amount: 0,
                },
                None => Operand::Imm(immediate(&ops[1])?),
            },
            nzcv: immediate(&ops[2])? as u8,
            cond: condition(&ops[3])?,
        },
        O::CSEL | O::CSINC | O::CSINV | O::CSNEG => I::CondSelect {
            width: w,
            op: match opcode {
                O::CSEL => Select::Plain,
                O::CSINC => Select::Increment,
                O::CSINV => Select::Invert,
                _ => Select::Negate,
            },
            rd,
            rn: rn()?,
            rm: rm()?,
            cond: condition(&ops[3])?,
        },
        O::RBIT | O::REV16 | O::REV32 | O::REV | O::CLZ | O::CLS => {
            r0?;
            I::Unary {
                width: w,
                op: match opcode {
                    O::RBIT => Unary::Rbit,
                    O::REV16 => Unary::Rev16,
                    O::REV32 => Unary::Rev32,
                    O::REV => Unary::Rev,
                    O::CLZ => Unary::Clz,
                    _ => Unary::Cls,
                },
                rd,
                rn: rn()?,
            }
        }
        O::UDIV | O::SDIV | O::LSLV | O::LSRV | O::ASRV | O::RORV => I::Binary {
            width: w,
            op: match opcode {
                O::UDIV => Binary::Udiv,
                O::SDIV => Binary::Sdiv,
                O::LSLV => Binary::Lslv,
                O::LSRV => Binary::Lsrv,
                O::ASRV => Binary::Asrv,
                _ => Binary::Rorv,
            },
            rd,
            rn: rn()?,
            rm: rm()?,
        },
        O::MADD | O::MSUB | O::SMADDL | O::SMSUBL | O::UMADDL | O::UMSUBL | O::SMULH | O::UMULH => {
            r0?;
            I::MulAdd {
                width: w,
                op: match opcode {
                    O::MADD => MulAdd::Madd,
                    O::MSUB => MulAdd::Msub,
                    O::SMADDL => MulAdd::Smaddl,
                    O::SMSUBL => MulAdd::Smsubl,
                    O::UMADDL => MulAdd::Umaddl,
                    O::UMSUBL => MulAdd::Umsubl,
                    O::SMULH => MulAdd::Smulh,
                    _ => MulAdd::Umulh,
                },
                rd,
                rn: rn()?,
                rm: rm()?,
                ra: reg(&ops[3]).map_or(Reg::Zero, |(_, r)| r),
            }
        }
        O::B | O::BL => I::Branch {
            link: opcode == O::BL,
            offset: pc_offset(&ops[0])?,
        },
        O::Bcc(c) => I::BranchCond {
            cond: Cond(c),
            offset: pc_offset(&ops[0])?,
        },
        O::CBZ | O::CBNZ => I::CompareBranch {
            width: w,
            nonzero: opcode == O::CBNZ,
            rt: rd,
            offset: pc_offset(&ops[1])?,
        },
        O::TBZ | O::TBNZ => I::TestBranch {
            nonzero: opcode == O::TBNZ,
            rt: rd,
            bit: immediate(&ops[1])? as u8,
            offset: pc_offset(&ops[2])?,
        },
        O::BR | O::BLR | O::RET => I::BranchRegister {
            op: match opcode {
                O::BR => BranchRegister::Br,
                O::BLR => BranchRegister::Blr,
                _ => BranchRegister::Ret,
            },
            rn: rd,
        },
        O::LDR | O::LDUR | O::STR | O::STUR if vector(&ops[0]).is_some() => {
            let (bytes, rt) = vector(&ops[0])?;
            let address = address(word, &ops[1], bytes)?;
            if matches!(opcode, O::STR | O::STUR) {
                I::StoreVector { bytes, rt, address }
            } else {
                I::LoadVector { bytes, rt, address }
            }
        }
        O::LDP | O::LDNP | O::STP | O::STNP if vector(&ops[0]).is_some() => {
            let (bytes, rt) = vector(&ops[0])?;
            let (_, rt2) = vector(&ops[1])?;
            let address = address(word, &ops[2], bytes)?;
            if matches!(opcode, O::STP | O::STNP) {
                I::StoreVectorPair {
                    bytes,
                    rt,
                    rt2,
                    address,
                }
            } else {
                I::LoadVectorPair {
                    bytes,
                    rt,
                    rt2,
                    address,
                }
            }
        }
        O::LDR
        | O::LDRB
        | O::LDRH
        | O::LDRSB
        | O::LDRSH
        | O::LDRSW
        | O::LDUR
        | O::LDURB
        | O::LDURH
        | O::LDURSB
        | O::LDURSH
        | O::LDURSW
        | O::LDTR
        | O::LDTRB
        | O::LDTRH
        | O::LDTRSB
        | O::LDTRSH
        | O::LDTRSW => {
            r0?;
            let (bytes, signed) = match opcode {
                O::LDRB | O::LDURB | O::LDTRB => (1, None),
                O::LDRH | O::LDURH | O::LDTRH => (2, None),
                O::LDRSB | O::LDURSB | O::LDTRSB => (1, Some(w)),
                O::LDRSH | O::LDURSH | O::LDTRSH => (2, Some(w)),
                O::LDRSW | O::LDURSW | O::LDTRSW => (4, Some(Width::X64)),
                _ => (if w == Width::X64 { 8 } else { 4 }, None),
            };
            I::Load {
                bytes,
                signed,
                rt: rd,
                address: address(word, &ops[1], bytes)?,
            }
        }
        O::STR
        | O::STRB
        | O::STRH
        | O::STUR
        | O::STURB
        | O::STURH
        | O::STTR
        | O::STTRB
        | O::STTRH => {
            r0?;
            let bytes = match opcode {
                O::STRB | O::STURB | O::STTRB => 1,
                O::STRH | O::STURH | O::STTRH => 2,
                _ if w == Width::X64 => 8,
                _ => 4,
            };
            I::Store {
                bytes,
                rt: rd,
                address: address(word, &ops[1], bytes)?,
            }
        }
        O::LDP | O::LDPSW | O::LDNP | O::STP | O::STNP => {
            r0?;
            let rt2 = reg(&ops[1])?.1;
            let bytes = if opcode == O::LDPSW || w == Width::W32 {
                4
            } else {
                8
            };
            let address = address(word, &ops[2], bytes)?;
            if matches!(opcode, O::STP | O::STNP) {
                I::StorePair {
                    bytes,
                    rt: rd,
                    rt2,
                    address,
                }
            } else {
                I::LoadPair {
                    bytes,
                    signed: opcode == O::LDPSW,
                    rt: rd,
                    rt2,
                    address,
                }
            }
        }
        O::PRFM | O::PRFUM | O::DMB(_) | O::DSB(_) | O::ISB | O::CLREX | O::SSBB | O::PSSBB => {
            I::Nothing
        }
        O::SVC => I::Syscall,
        O::BRK => I::Breakpoint {
            comment: immediate(&ops[0])? as u16,
        },
        // Instructions for higher exception levels, and the permanently undefined one.
        O::HVC | O::SMC | O::HLT | O::DCPS1 | O::DCPS2 | O::DCPS3 | O::ERET | O::DRPS | O::UDF => {
            I::Undefined
        }
        O::MRS => I::ReadSystem {
            register: system_register(word)?,
            rt: rd,
        },
        O::MSR if bits(word, 20, 1) == 1 => I::WriteSystem {
            register: system_register(word)?,
            rt: zr(bits(word, 0, 5)),
        },
        _ => return None,
    })
}
