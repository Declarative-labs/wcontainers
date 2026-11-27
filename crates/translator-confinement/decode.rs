//! What one host instruction word does, as far as confinement needs it: the registers it writes and how, the memory
//! it touches, where it goes next. Decoded from Arm's encoding tables (Arm ARM, "A64 instruction set encoding"),
//! independently of the translator's own decoder. Any word outside the families named here is `Unknown`, which the
//! checker refuses: the decoder may be incomplete, never permissive.

/// A general register or the stack pointer; the zero register is never one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    X(u8),
    Sp,
}

/// A register a value is read from: a general register, the stack pointer, or the zero register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Reg(Reg),
    Zero,
}

/// How an index register is widened before it is shifted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extend {
    Unsigned(u32),
    Signed(u32),
}

/// How a written value is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Made {
    /// Anything.
    Unknown,
    /// A value in this range, whatever the inputs (a 32-bit result, a loaded byte).
    Range(u64, u64),
    /// `rn` plus or minus the extended and shifted `rm`; 64 bits.
    Add {
        rn: Source,
        rm: Source,
        extend: Extend,
        shift: u32,
        subtract: bool,
    },
    /// `rn` plus a constant; 64 bits.
    AddConstant(Source, i64),
    /// The instructions' first byte plus this.
    Code(i64),
    /// The page of the instructions' byte at this offset, plus a multiple of 4 KiB (`adrp`).
    CodePage { at: i64, plus: i64 },
    /// A constant.
    Constant(u64),
    /// `movk`: 16 bits of the register replaced.
    Keep { rd: u8, value: u16, shift: u32 },
    /// `ubfx`: `width` bits of `rn` from `lsb` moved to the bottom.
    Extract { rn: Source, lsb: u32, width: u32 },
    /// `ubfiz`: the low `width` bits of `rn` moved up to `lsb`.
    Insert { rn: Source, lsb: u32, width: u32 },
    /// `bfxil`: `width` bits of `rn` from `lsb` into the bottom of `rd`, its other bits kept.
    ExtractInto { rd: u8, width: u32 },
    /// `bfi`: the low `width` bits of `rn` into `rd` at `lsb`, its other bits kept.
    InsertInto { rd: u8, lsb: u32, width: u32 },
    And(Source, u64),
    Or(Source, u64),
    Xor(Source, u64),
}

/// A register write.
pub type Write = (Reg, Made);

/// Where an access's base is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Reg(Source),
    /// The access instruction's own address (a literal load).
    Here,
}

/// What an access adds to its base: a range of constant offsets, or an extended and shifted index register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offset {
    Constant(i64, i64),
    Index {
        rm: Source,
        extend: Extend,
        shift: u32,
    },
}

/// How a written-back base moves after the access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    By(i64),
    ByRegister(Source),
}

/// One memory access: its first byte at the base plus the offset, `bytes` long.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    pub base: Base,
    pub offset: Offset,
    pub bytes: u64,
    pub write: bool,
    /// The registers it loads.
    pub loads: Vec<Write>,
    pub moved: Option<Moved>,
}

/// What one instruction does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Writes these registers (none for flags, vectors, barriers and hints) and goes on.
    Plain(Vec<Write>),
    Access(Access),
    /// A direct branch by this many bytes; with `link`, x30 gets the next instruction's address.
    Branch {
        by: i64,
        conditional: bool,
        link: bool,
    },
    /// A branch to the address in `rn`; with `link`, x30 gets the next instruction's address.
    Jump { rn: u8, link: bool },
    /// Raises a signal (`brk`, `udf`, `hlt`): the run ends there.
    Leaves,
    SystemCall,
    /// Writes a register user space can write that is not in the allowed list.
    ForbiddenRegister,
    /// Not one of the families this decoder knows.
    Unknown,
}

fn bits(word: u32, low: u32, width: u32) -> u32 {
    (word >> low) & ((1 << width) - 1)
}

fn bit(word: u32, at: u32) -> bool {
    word >> at & 1 != 0
}

fn signed(value: u32, width: u32) -> i64 {
    let shift = 64 - width;
    (i64::from(value) << shift) >> shift
}

/// Register field `n` where 31 is the zero register.
fn or_zero(n: u32) -> Source {
    if n == 31 {
        Source::Zero
    } else {
        Source::Reg(Reg::X(n as u8))
    }
}

/// Register field `n` where 31 is the stack pointer.
fn or_sp(n: u32) -> Source {
    if n == 31 {
        Source::Reg(Reg::Sp)
    } else {
        Source::Reg(Reg::X(n as u8))
    }
}

/// A write to register field `n` where 31 is the zero register: none.
fn write_or_zero(n: u32, made: Made) -> Vec<Write> {
    if n == 31 {
        Vec::new()
    } else {
        vec![(Reg::X(n as u8), made)]
    }
}

/// A write to register field `n` where 31 is the stack pointer.
fn write_or_sp(n: u32, made: Made) -> Vec<Write> {
    let to = if n == 31 { Reg::Sp } else { Reg::X(n as u8) };
    vec![(to, made)]
}

/// The range of an unsigned value of `bytes` bytes, or anything for eight.
fn loaded(bytes: u64) -> Made {
    if bytes >= 8 {
        Made::Unknown
    } else {
        Made::Range(0, (1 << (8 * bytes)) - 1)
    }
}

const WORD: Made = Made::Range(0, u32::MAX as u64);

/// Decodes one instruction word.
pub fn decode(word: u32) -> Step {
    match bits(word, 25, 4) {
        0b1000 | 0b1001 => immediate(word),
        0b1010 | 0b1011 => branch_or_system(word),
        0b0100 | 0b0110 | 0b1100 | 0b1110 => load_store(word),
        0b0101 | 0b1101 => register(word),
        0b0111 | 0b1111 => simd(word),
        // `udf`: the permanently undefined instruction.
        _ if word >> 16 == 0 => Step::Leaves,
        _ => Step::Unknown,
    }
}

/// Arm's DecodeBitMasks for a logical immediate (Arm ARM, shared pseudocode); none where it is reserved.
fn bit_mask(sf: bool, n: u32, immr: u32, imms: u32) -> Option<u64> {
    let combined = (n << 6) | (!imms & 0x3f);
    if combined == 0 {
        return None;
    }
    let length = 31 - combined.leading_zeros();
    if length < 1 || (!sf && n == 1) {
        return None;
    }
    let size = 1u32 << length;
    let levels = size - 1;
    let s = imms & levels;
    let r = immr & levels;
    if s == levels {
        return None;
    }
    let ones: u64 = (1u64 << (s + 1)) - 1;
    let element = if r == 0 {
        ones
    } else {
        let mask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
        ((ones >> r) | (ones << (size - r))) & mask
    };
    let mut value = element;
    let mut width = size;
    let total = if sf { 64 } else { 32 };
    while width < total {
        value |= value << width;
        width *= 2;
    }
    Some(value)
}

fn immediate(word: u32) -> Step {
    let sf = bit(word, 31);
    let rd = bits(word, 0, 5);
    let rn = bits(word, 5, 5);
    match bits(word, 23, 3) {
        0b000 | 0b001 => {
            let offset = signed((bits(word, 5, 19) << 2) | bits(word, 29, 2), 21);
            let made = if sf {
                Made::CodePage {
                    at: 0,
                    plus: offset << 12,
                }
            } else {
                Made::Code(offset)
            };
            Step::Plain(write_or_zero(rd, made))
        }
        0b010 => {
            let set_flags = bit(word, 29);
            let shift = if bit(word, 22) { 12 } else { 0 };
            let imm = i64::from(bits(word, 10, 12)) << shift;
            let value = if bit(word, 30) { -imm } else { imm };
            let made = if sf {
                Made::AddConstant(or_sp(rn), value)
            } else {
                WORD
            };
            Step::Plain(if set_flags {
                write_or_zero(rd, made)
            } else {
                write_or_sp(rd, made)
            })
        }
        0b100 => {
            let opc = bits(word, 29, 2);
            let Some(mask) = bit_mask(sf, bits(word, 22, 1), bits(word, 16, 6), bits(word, 10, 6)) else {
                return Step::Unknown;
            };
            let made = match (sf, opc) {
                (false, _) => WORD,
                (true, 0b00 | 0b11) => Made::And(or_zero(rn), mask),
                (true, 0b01) => Made::Or(or_zero(rn), mask),
                (true, _) => Made::Xor(or_zero(rn), mask),
            };
            Step::Plain(if opc == 0b11 {
                write_or_zero(rd, made)
            } else {
                write_or_sp(rd, made)
            })
        }
        0b101 => {
            let hw = bits(word, 21, 2);
            if !sf && hw >= 2 {
                return Step::Unknown;
            }
            let shift = hw * 16;
            let imm = u64::from(bits(word, 5, 16)) << shift;
            let made = match bits(word, 29, 2) {
                0b00 if sf => Made::Constant(!imm),
                0b00 => Made::Constant(!imm & u64::from(u32::MAX)),
                0b10 => Made::Constant(imm),
                0b11 if sf => Made::Keep {
                    rd: rd as u8,
                    value: bits(word, 5, 16) as u16,
                    shift,
                },
                0b11 => WORD,
                _ => return Step::Unknown,
            };
            Step::Plain(write_or_zero(rd, made))
        }
        0b110 => {
            let n = bits(word, 22, 1);
            if (sf && n == 0) || (!sf && n == 1) {
                return Step::Unknown;
            }
            let immr = bits(word, 16, 6);
            let imms = bits(word, 10, 6);
            let made = match bits(word, 29, 2) {
                _ if !sf => WORD,
                0b10 if imms >= immr => Made::Extract {
                    rn: or_zero(rn),
                    lsb: immr,
                    width: imms - immr + 1,
                },
                0b10 => Made::Insert {
                    rn: or_zero(rn),
                    lsb: 64 - immr,
                    width: imms + 1,
                },
                0b01 if imms >= immr => Made::ExtractInto {
                    rd: rd as u8,
                    width: imms - immr + 1,
                },
                0b01 => Made::InsertInto {
                    rd: rd as u8,
                    lsb: 64 - immr,
                    width: imms + 1,
                },
                0b00 => Made::Unknown,
                _ => return Step::Unknown,
            };
            Step::Plain(write_or_zero(rd, made))
        }
        0b111 => Step::Plain(write_or_zero(rd, if sf { Made::Unknown } else { WORD })),
        // Add or subtract with tags, minimum and maximum (FEAT_MTE, FEAT_CSSC).
        _ => Step::Unknown,
    }
}

fn branch_or_system(word: u32) -> Step {
    if word & 0x7c00_0000 == 0x1400_0000 {
        return Step::Branch {
            by: signed(bits(word, 0, 26), 26) << 2,
            conditional: false,
            link: bit(word, 31),
        };
    }
    // b.cond and bc.cond.
    if word & 0xff00_0000 == 0x5400_0000 {
        return Step::Branch {
            by: signed(bits(word, 5, 19), 19) << 2,
            conditional: true,
            link: false,
        };
    }
    // cbz, cbnz.
    if word & 0x7e00_0000 == 0x3400_0000 {
        return Step::Branch {
            by: signed(bits(word, 5, 19), 19) << 2,
            conditional: true,
            link: false,
        };
    }
    // tbz, tbnz.
    if word & 0x7e00_0000 == 0x3600_0000 {
        return Step::Branch {
            by: signed(bits(word, 5, 14), 14) << 2,
            conditional: true,
            link: false,
        };
    }
    if word & 0xff00_0000 == 0xd400_0000 {
        return match (bits(word, 21, 3), bits(word, 2, 3), bits(word, 0, 2)) {
            (0b000, 0, 0b01..=0b11) => Step::SystemCall,
            // brk, hlt.
            (0b001 | 0b010, 0, 0b00) => Step::Leaves,
            _ => Step::Unknown,
        };
    }
    if word & 0xffc0_0000 == 0xd500_0000 {
        return system(word);
    }
    if word & 0xfe00_0000 == 0xd600_0000 {
        let opc = bits(word, 21, 4);
        let plain = bits(word, 16, 5) == 0b11111 && bits(word, 10, 6) == 0 && bits(word, 0, 5) == 0;
        return match opc {
            0b0000 | 0b0010 if plain => Step::Jump {
                rn: bits(word, 5, 5) as u8,
                link: false,
            },
            0b0001 if plain => Step::Jump {
                rn: bits(word, 5, 5) as u8,
                link: true,
            },
            _ => Step::Unknown,
        };
    }
    Step::Unknown
}

/// The system registers user space may change during a run: NZCV, FPCR, FPSR and TPIDR_EL0, as (op0, op1, CRn, CRm,
/// op2).
const WRITABLE: [(u32, u32, u32, u32, u32); 4] = [
    (3, 3, 4, 2, 0),
    (3, 3, 4, 4, 0),
    (3, 3, 4, 4, 1),
    (3, 3, 13, 0, 2),
];

/// The largest block `dc zva` zeroes and the largest cache line: DCZID_EL0.BS and CTR_EL0's line sizes are at most 2 KiB.
const LARGEST_BLOCK: i64 = 2048;

fn system(word: u32) -> Step {
    let load = bit(word, 21);
    let op0 = bits(word, 19, 2);
    let op1 = bits(word, 16, 3);
    let crn = bits(word, 12, 4);
    let crm = bits(word, 8, 4);
    let op2 = bits(word, 5, 3);
    let rt = bits(word, 0, 5);
    match (op0, load) {
        (0, false) if rt == 0b11111 => match crn {
            // Hints: nop, yield, wfe, wfi, sev, sevl, dgh, csdb and bti.
            0b0010 => match (crm, op2) {
                (0b0000, 0..=6) | (0b0010, 4) | (0b0100, 0 | 2 | 4 | 6) => Step::Plain(Vec::new()),
                _ => Step::Unknown,
            },
            // clrex, dsb, dmb, isb, sb.
            0b0011 if matches!(op2, 2 | 4 | 5 | 6 | 7) => Step::Plain(Vec::new()),
            // cfinv, xaflag, axflag: the flags alone.
            0b0100 if op1 == 0 && crm == 0 && op2 <= 2 => Step::Plain(Vec::new()),
            0b0100 => Step::ForbiddenRegister,
            _ => Step::Unknown,
        },
        (1, false) if op1 == 0b011 && crn == 0b0111 && op2 == 0b001 => match crm {
            // dc zva writes; ic ivau and dc cvau, cvac, cvap, cvadp, civac touch without changing.
            0b0100 | 0b0101 | 0b1010 | 0b1011 | 0b1100 | 0b1101 | 0b1110 => Step::Access(Access {
                base: Base::Reg(or_zero(rt)),
                offset: Offset::Constant(-(LARGEST_BLOCK - 1), 0),
                bytes: LARGEST_BLOCK as u64,
                write: crm == 0b0100,
                loads: Vec::new(),
                moved: None,
            }),
            _ => Step::Unknown,
        },
        (2 | 3, true) => Step::Plain(write_or_zero(rt, Made::Unknown)),
        (2 | 3, false) if WRITABLE.contains(&(op0, op1, crn, crm, op2)) => Step::Plain(Vec::new()),
        (2 | 3, false) => Step::ForbiddenRegister,
        _ => Step::Unknown,
    }
}

/// The extend an option field names (Arm ARM, ExtendType).
fn extend(option: u32) -> Extend {
    let from = 8 << (option & 3);
    if option & 4 == 0 {
        Extend::Unsigned(from)
    } else {
        Extend::Signed(from)
    }
}

fn load_store(word: u32) -> Step {
    let size = bits(word, 30, 2);
    let vector = bit(word, 26);
    let opc = bits(word, 22, 2);
    let rn = bits(word, 5, 5);
    let rt = bits(word, 0, 5);
    let rt2 = bits(word, 10, 5);
    let rs = bits(word, 16, 5);
    // One register's access: its bytes, whether it stores, and what it loads into a general register.
    let single = || -> Option<(u64, bool, Vec<Write>)> {
        if vector {
            let bytes = if opc & 2 != 0 {
                if size != 0 {
                    return None;
                }
                16
            } else {
                1u64 << size
            };
            Some((bytes, opc & 1 == 0, Vec::new()))
        } else {
            let bytes = 1u64 << size;
            match opc {
                0b00 => Some((bytes, true, Vec::new())),
                0b01 => Some((bytes, false, write_or_zero(rt, loaded(bytes)))),
                // A prefetch.
                0b10 if size == 3 => Some((bytes, false, Vec::new())),
                0b11 if size >= 2 => None,
                _ => Some((bytes, false, write_or_zero(rt, Made::Unknown))),
            }
        }
    };
    let plain = |offset: Offset, moved: Option<Moved>| -> Step {
        match single() {
            Some((bytes, write, loads)) => Step::Access(Access {
                base: Base::Reg(or_sp(rn)),
                offset,
                bytes,
                write,
                loads,
                moved,
            }),
            None => Step::Unknown,
        }
    };
    // Literal.
    if word & 0x3b00_0000 == 0x1800_0000 {
        let offset = signed(bits(word, 5, 19), 19) << 2;
        let (bytes, loads) = match (vector, size) {
            (false, 0b00) => (4, write_or_zero(rt, WORD)),
            (false, 0b01) => (8, write_or_zero(rt, Made::Unknown)),
            (false, 0b10) => (4, write_or_zero(rt, Made::Unknown)),
            (false, _) => (1, Vec::new()),
            (true, 0b00) => (4, Vec::new()),
            (true, 0b01) => (8, Vec::new()),
            (true, 0b10) => (16, Vec::new()),
            (true, _) => return Step::Unknown,
        };
        return Step::Access(Access {
            base: Base::Here,
            offset: Offset::Constant(offset, offset),
            bytes,
            write: false,
            loads,
            moved: None,
        });
    }
    // Register offset.
    if word & 0x3b20_0c00 == 0x3820_0800 {
        let option = bits(word, 13, 3);
        if option & 2 == 0 {
            return Step::Unknown;
        }
        let Some((bytes, ..)) = single() else {
            return Step::Unknown;
        };
        let shift = if bit(word, 12) { bytes.trailing_zeros() } else { 0 };
        return plain(
            Offset::Index {
                rm: or_zero(rs),
                extend: extend(option),
                shift,
            },
            None,
        );
    }
    // Unsigned immediate.
    if word & 0x3b00_0000 == 0x3900_0000 {
        let Some((bytes, ..)) = single() else {
            return Step::Unknown;
        };
        let offset = i64::from(bits(word, 10, 12)) * bytes as i64;
        return plain(Offset::Constant(offset, offset), None);
    }
    // Unscaled, post-index, unprivileged and pre-index immediates.
    if word & 0x3b20_0000 == 0x3800_0000 {
        let offset = signed(bits(word, 12, 9), 9);
        return match bits(word, 10, 2) {
            0b00 => plain(Offset::Constant(offset, offset), None),
            0b01 => plain(Offset::Constant(0, 0), Some(Moved::By(offset))),
            0b10 if !vector => plain(Offset::Constant(offset, offset), None),
            0b11 => plain(Offset::Constant(offset, offset), Some(Moved::By(offset))),
            _ => Step::Unknown,
        };
    }
    // Atomic memory operations: ldadd, ldclr, ldeor, ldset, ldsmax, ldsmin, ldumax, ldumin, swp, ldapr.
    if word & 0x3b20_0c00 == 0x3820_0000 {
        let operation = bits(word, 12, 4);
        let known = operation <= 0b1000 || (operation == 0b1100 && rs == 0b11111);
        if vector || !known {
            return Step::Unknown;
        }
        let bytes = 1u64 << size;
        return Step::Access(Access {
            base: Base::Reg(or_sp(rn)),
            offset: Offset::Constant(0, 0),
            bytes,
            write: operation != 0b1100,
            loads: write_or_zero(rt, loaded(bytes)),
            moved: None,
        });
    }
    // Pairs.
    if word & 0x3a00_0000 == 0x2800_0000 {
        let load = bit(word, 22);
        let (scale, made) = match (vector, size, load) {
            (false, 0b00, _) => (2, WORD),
            (false, 0b01, true) => (2, Made::Unknown),
            (false, 0b10, _) => (3, Made::Unknown),
            (true, 0b00, _) => (2, Made::Unknown),
            (true, 0b01, _) => (3, Made::Unknown),
            (true, 0b10, _) => (4, Made::Unknown),
            _ => return Step::Unknown,
        };
        let offset = signed(bits(word, 15, 7), 7) << scale;
        let (at, moved) = match bits(word, 23, 2) {
            0b00 | 0b10 => (offset, None),
            0b01 => (0, Some(Moved::By(offset))),
            _ => (offset, Some(Moved::By(offset))),
        };
        let loads = if load && !vector {
            let mut loads = write_or_zero(rt, made);
            loads.extend(write_or_zero(rt2, made));
            loads
        } else {
            Vec::new()
        };
        return Step::Access(Access {
            base: Base::Reg(or_sp(rn)),
            offset: Offset::Constant(at, at),
            bytes: 2 << scale,
            write: !load,
            loads,
            moved,
        });
    }
    // Exclusives, ordered accesses and compare and swap.
    if word & 0x3f00_0000 == 0x0800_0000 {
        let (o2, load, o1) = (bit(word, 23), bit(word, 22), bit(word, 21));
        let one = 1u64 << size;
        let (bytes, written, write): (u64, Vec<u32>, bool) = match (o2, o1) {
            (false, false) if load => (one, vec![rt], false),
            (false, false) => (one, vec![rs], true),
            (false, true) if size >= 2 && load => (2 << (size & 1) << 2, vec![rt, rt2], false),
            (false, true) if size >= 2 => (2 << (size & 1) << 2, vec![rs], true),
            // casp: an even pair compared and swapped.
            (false, true) => (2 << (size & 1) << 2, vec![rs, rs + 1], true),
            (true, false) if load => (one, vec![rt], false),
            (true, false) => (one, Vec::new(), true),
            (true, true) => (one, vec![rs], true),
        };
        let loads = written
            .into_iter()
            .filter(|&r| r < 31)
            .map(|r| (Reg::X(r as u8), Made::Unknown))
            .collect();
        return Step::Access(Access {
            base: Base::Reg(or_sp(rn)),
            offset: Offset::Constant(0, 0),
            bytes,
            write,
            loads,
            moved: None,
        });
    }
    // ldapur and stlur.
    if word & 0x3f20_0c00 == 0x1900_0000 && !vector {
        let offset = signed(bits(word, 12, 9), 9);
        let bytes = 1u64 << size;
        let loads = match opc {
            0b00 => Vec::new(),
            0b01 => write_or_zero(rt, loaded(bytes)),
            _ => write_or_zero(rt, Made::Unknown),
        };
        return Step::Access(Access {
            base: Base::Reg(or_sp(rn)),
            offset: Offset::Constant(offset, offset),
            bytes,
            write: opc == 0,
            loads,
            moved: None,
        });
    }
    // Vector structures, several registers.
    if word & 0xbfbf_0000 == 0x0c00_0000 || word & 0xbfa0_0000 == 0x0c80_0000 {
        let registers = match bits(word, 12, 4) {
            0b0000 | 0b0010 => 4,
            0b0100 | 0b0110 => 3,
            0b0111 => 1,
            0b1000 | 0b1010 => 2,
            _ => return Step::Unknown,
        };
        let bytes = registers * if bit(word, 30) { 16 } else { 8 };
        return structure(word, bytes);
    }
    // Vector structures, one element of each register or replicated.
    if word & 0xbf9f_0000 == 0x0d00_0000 || word & 0xbf80_0000 == 0x0d80_0000 {
        let elements = u64::from((bits(word, 13, 1) << 1) | bits(word, 21, 1)) + 1;
        let element = match bits(word, 14, 2) {
            0b00 => 1,
            0b01 => 2,
            0b10 if bit(word, 10) => 8,
            0b10 => 4,
            _ => 1u64 << bits(word, 10, 2),
        };
        return structure(word, elements * element);
    }
    Step::Unknown
}

/// A vector structure access of `bytes` at its base, written back after it when its form is the post-index one.
fn structure(word: u32, bytes: u64) -> Step {
    let rn = bits(word, 5, 5);
    let rm = bits(word, 16, 5);
    let moved = bit(word, 23).then_some({
        if rm == 31 {
            Moved::By(bytes as i64)
        } else {
            Moved::ByRegister(Source::Reg(Reg::X(rm as u8)))
        }
    });
    Step::Access(Access {
        base: Base::Reg(or_sp(rn)),
        offset: Offset::Constant(0, 0),
        bytes,
        write: !bit(word, 22),
        loads: Vec::new(),
        moved,
    })
}

fn register(word: u32) -> Step {
    let sf = bit(word, 31);
    let rd = bits(word, 0, 5);
    let rn = bits(word, 5, 5);
    let rm = bits(word, 16, 5);
    let set_flags = bit(word, 29);
    // Add and subtract, extended register.
    if word & 0x1f20_0000 == 0x0b20_0000 {
        let amount = bits(word, 10, 3);
        if amount > 4 {
            return Step::Unknown;
        }
        let made = if sf {
            Made::Add {
                rn: or_sp(rn),
                rm: or_zero(rm),
                extend: extend(bits(word, 13, 3)),
                shift: amount,
                subtract: bit(word, 30),
            }
        } else {
            WORD
        };
        return Step::Plain(if set_flags {
            write_or_zero(rd, made)
        } else {
            write_or_sp(rd, made)
        });
    }
    // Add and subtract, shifted register: only a left shift is followed. A destination of 31 is taken as the stack
    // pointer too, as later forms may name it there.
    if word & 0x1f20_0000 == 0x0b00_0000 {
        let made = match (sf, bits(word, 22, 2)) {
            (true, 0b00) => Made::Add {
                rn: or_zero(rn),
                rm: or_zero(rm),
                extend: Extend::Unsigned(64),
                shift: bits(word, 10, 6),
                subtract: bit(word, 30),
            },
            (true, _) => Made::Unknown,
            (false, _) => WORD,
        };
        let mut writes = write_or_zero(rd, made);
        if rd == 31 && !set_flags {
            writes.push((Reg::Sp, Made::Unknown));
        }
        return Step::Plain(writes);
    }
    // Logical, shifted register: `orr rd, xzr, rm` is a move.
    if word & 0x1f00_0000 == 0x0a00_0000 {
        let is_move = bits(word, 29, 2) == 0b01
            && !bit(word, 21)
            && rn == 31
            && bits(word, 10, 6) == 0
            && bits(word, 22, 2) == 0;
        let made = match (sf, is_move) {
            (true, true) => Made::AddConstant(or_zero(rm), 0),
            (true, false) => Made::Unknown,
            (false, _) => WORD,
        };
        return Step::Plain(write_or_zero(rd, made));
    }
    // Data processing, two sources: subp, irg and gmi (FEAT_MTE) are not followed.
    if word & 0x5fe0_0000 == 0x1ac0_0000 && matches!(bits(word, 10, 6), 0b000000 | 0b000100 | 0b000101) {
        return Step::Unknown;
    }
    // Conditional compare: flags alone.
    if word & 0x1fe0_0000 == 0x1a40_0000 {
        return Step::Plain(Vec::new());
    }
    Step::Plain(write_or_zero(rd, if sf { Made::Unknown } else { WORD }))
}

fn simd(word: u32) -> Step {
    let rd = bits(word, 0, 5);
    // Conversions between floating point and integers.
    if word & 0x5f20_fc00 == 0x1e20_0000 {
        let to_general = matches!(bits(word, 16, 3), 0b000 | 0b001 | 0b100 | 0b101 | 0b110);
        return Step::Plain(if to_general {
            write_or_zero(rd, Made::Unknown)
        } else {
            Vec::new()
        });
    }
    // Conversions between floating point and fixed point.
    if word & 0x5f20_0000 == 0x1e00_0000 {
        let to_general = matches!(bits(word, 16, 3), 0b000 | 0b001);
        return Step::Plain(if to_general {
            write_or_zero(rd, Made::Unknown)
        } else {
            Vec::new()
        });
    }
    // umov and smov.
    if word & 0xbfe0_8400 == 0x0e00_0400 && matches!(bits(word, 11, 4), 0b0101 | 0b0111) {
        return Step::Plain(write_or_zero(rd, Made::Unknown));
    }
    Step::Plain(Vec::new())
}
