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
    convert(word, decoded.opcode, &ops).unwrap_or(Instruction::Unsupported)
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

/// The address of an integer load or store, or `None` for forms with no address we translate.
fn address(word: u32, op: &Op) -> Option<Address> {
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
            let scale = bits(word, 30, 2) as u8;
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

fn convert(word: u32, opcode: Opcode, ops: &[Op; 4]) -> Option<Instruction> {
    use Instruction as I;
    use Opcode as O;
    let r0 = reg(&ops[0]);
    let (w, rd) = r0.unwrap_or((Width::X64, Reg::Zero));
    let rn = || reg(&ops[1]).map(|(_, r)| r);
    let rm = || reg(&ops[2]).map(|(_, r)| r);
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
        | O::LDTRSW
        | O::LDAR
        | O::LDARB
        | O::LDARH => {
            r0?;
            let (bytes, signed) = match opcode {
                O::LDRB | O::LDURB | O::LDTRB | O::LDARB => (1, None),
                O::LDRH | O::LDURH | O::LDTRH | O::LDARH => (2, None),
                O::LDRSB | O::LDURSB | O::LDTRSB => (1, Some(w)),
                O::LDRSH | O::LDURSH | O::LDTRSH => (2, Some(w)),
                O::LDRSW | O::LDURSW | O::LDTRSW => (4, Some(Width::X64)),
                _ => (if w == Width::X64 { 8 } else { 4 }, None),
            };
            I::Load {
                bytes,
                signed,
                rt: rd,
                address: address(word, &ops[1])?,
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
        | O::STTRH
        | O::STLR
        | O::STLRB
        | O::STLRH => {
            r0?;
            let bytes = match opcode {
                O::STRB | O::STURB | O::STTRB | O::STLRB => 1,
                O::STRH | O::STURH | O::STTRH | O::STLRH => 2,
                _ if w == Width::X64 => 8,
                _ => 4,
            };
            I::Store {
                bytes,
                rt: rd,
                address: address(word, &ops[1])?,
            }
        }
        O::LDP | O::LDPSW | O::LDNP | O::STP | O::STNP => {
            r0?;
            let rt2 = reg(&ops[1])?.1;
            let address = address(word, &ops[2])?;
            let bytes = if opcode == O::LDPSW || w == Width::W32 {
                4
            } else {
                8
            };
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
        O::PRFM
        | O::PRFUM
        | O::DMB(_)
        | O::DSB(_)
        | O::ISB
        | O::CLREX
        | O::SB
        | O::SSBB
        | O::PSSBB => I::Nothing,
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
