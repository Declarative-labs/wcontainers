//! Decoding by Arm's own forms: the form a word belongs to names the instruction, and its fields give the
//! operands. Used for every family `yaxpeax-arm` leaves out or gets wrong; the rest goes through
//! [`crate::decode`]'s first path.

use crate::forms::Form;
use crate::instruction::*;

/// The fields of one word of one form.
struct Fields<'a> {
    form: &'a Form,
    word: u32,
}

impl Fields<'_> {
    fn get(&self, name: &str) -> u32 {
        self.form
            .field(self.word, name)
            .unwrap_or_else(|| panic!("no field {name} in {}", self.form.name))
    }

    fn has(&self, name: &str) -> bool {
        self.form.field(self.word, name).is_some()
    }

    fn bit(&self, name: &str) -> bool {
        self.get(name) == 1
    }

    /// A general register where 31 is the zero register.
    fn zr(&self, name: &str) -> Reg {
        match self.get(name) {
            31 => Reg::Zero,
            n => Reg::X(n as u8),
        }
    }

    /// A general register where 31 is the stack pointer.
    fn sp(&self, name: &str) -> Reg {
        match self.get(name) {
            31 => Reg::Sp,
            n => Reg::X(n as u8),
        }
    }

    fn v(&self, name: &str) -> u8 {
        self.get(name) as u8
    }
}

/// Decode a word of a form, or `None` to leave it to the first path.
pub(crate) fn decode(word: u32, form: &Form) -> Option<Instruction> {
    let f = Fields { form, word };
    let (stem, group) = (
        form.name.split('_').next().unwrap_or(""),
        form.group.rsplit('/').next().unwrap_or(""),
    );
    match group {
        "dp_2src" if stem.starts_with("CRC32") => Some(crc32(&f)),
        "memop" => Some(atomic(&f)),
        "comswap" | "comswappr" => Some(compare_swap(&f, group == "comswappr")),
        "ldstexclr" | "ldstexclp" => Some(exclusive(&f, group == "ldstexclp")),
        "ldstord" => Some(ordered(&f)),
        "barriers" if stem == "CLREX" => Some(Instruction::ClearExclusive),
        "systeminstrs" => Some(system_instruction(&f)),
        "systemmove" => Some(system_move(&f)),
        "pstate" => Some(pstate(&f)),
        "floatimm" => Some(float_immediate(&f)),
        "floatdp1" | "floatdp2" | "floatdp3" | "floatcmp" | "floatccmp" | "floatsel"
        | "float2fix" => float(&f, stem, group),
        "float2int" if stem != "FMOV" => float(&f, stem, group),
        _ => crate::decode_simd::decode(word, form, stem, group),
    }
}

/// The element size in bytes of a two bit `size` field.
fn bytes_of(size: u32) -> u8 {
    1 << size
}

fn crc32(f: &Fields) -> Instruction {
    Instruction::Crc32 {
        castagnoli: f.bit("C"),
        bytes: bytes_of(f.get("sz")),
        rd: f.zr("Rd"),
        rn: f.zr("Rn"),
        rm: f.zr("Rm"),
    }
}

fn atomic(f: &Fields) -> Instruction {
    let op = if f.bit("o3") {
        AtomicOp::Swap
    } else {
        match f.get("opc") {
            0 => AtomicOp::Add,
            1 => AtomicOp::Clear,
            2 => AtomicOp::Xor,
            3 => AtomicOp::Set,
            4 => AtomicOp::SignedMax,
            5 => AtomicOp::SignedMin,
            6 => AtomicOp::UnsignedMax,
            _ => AtomicOp::UnsignedMin,
        }
    };
    Instruction::Atomic {
        op,
        bytes: bytes_of(f.get("size")),
        rs: f.zr("Rs"),
        rt: f.zr("Rt"),
        rn: f.sp("Rn"),
    }
}

fn compare_swap(f: &Fields, pair: bool) -> Instruction {
    let bytes = if pair {
        4 << f.get("sz")
    } else {
        bytes_of(f.get("size"))
    };
    // A pair names an even register; Arm's decode makes an odd one undefined.
    if pair && (f.get("Rs") % 2 == 1 || f.get("Rt") % 2 == 1) {
        return Instruction::Undefined;
    }
    Instruction::CompareSwap {
        bytes,
        pair,
        rs: f.zr("Rs"),
        rt: f.zr("Rt"),
        rn: f.sp("Rn"),
    }
}

fn exclusive(f: &Fields, pair: bool) -> Instruction {
    let bytes = if pair {
        4 << f.get("sz")
    } else {
        bytes_of(f.get("size"))
    };
    let (rt, rt2, rn) = (
        f.zr("Rt"),
        if pair { f.zr("Rt2") } else { Reg::Zero },
        f.sp("Rn"),
    );
    if f.bit("L") {
        Instruction::LoadExclusive {
            bytes,
            pair,
            rt,
            rt2,
            rn,
        }
    } else {
        Instruction::StoreExclusive {
            bytes,
            pair,
            rs: f.zr("Rs"),
            rt,
            rt2,
            rn,
        }
    }
}

fn ordered(f: &Fields) -> Instruction {
    let (bytes, rt, rn) = (bytes_of(f.get("size")), f.zr("Rt"), f.sp("Rn"));
    if f.bit("L") {
        Instruction::LoadAcquire { bytes, rt, rn }
    } else {
        Instruction::StoreRelease { bytes, rt, rn }
    }
}

/// `msr` to a PSTATE field. None is open to EL0 on the CPU we present; SSBS, DIT, the streaming mode
/// and TCO are, on CPUs with their features.
fn pstate(f: &Fields) -> Instruction {
    match (f.get("op1"), f.get("op2")) {
        (3, 1..=4) => Instruction::Unsupported,
        _ => Instruction::Undefined,
    }
}

/// `sys`: the cache and code maintenance unprivileged code may use. Everything else is undefined at EL0.
fn system_instruction(f: &Fields) -> Instruction {
    if f.bit("L") {
        return Instruction::Undefined;
    }
    let rn = f.zr("Rt");
    match (f.get("op1"), f.get("CRn"), f.get("CRm"), f.get("op2")) {
        (3, 7, 4, 1) => Instruction::ZeroBlock { rn },
        (3, 7, 11, 1) | (3, 7, 10, 1) | (3, 7, 14, 1) => Instruction::CleanCache { rn },
        (3, 7, 5, 1) => Instruction::CodeChanged { rn },
        // The tag (MTE) and persistence (DPB, DPB2) cache operations EL0 may use on CPUs with them.
        (3, 7, 4, 3 | 4) | (3, 7, 10 | 14, 3 | 5) | (3, 7, 12 | 13, 1 | 3 | 5) => {
            Instruction::Unsupported
        }
        _ => Instruction::Undefined,
    }
}

/// The value of `CTR_EL0` we present: 64 byte cache lines, a physically indexed instruction cache, and both
/// the clean to the point of unification (IDC clear) and the instruction cache invalidation (DIC clear)
/// required, so programs that write code tell us with `ic ivau`.
pub const CACHE_TYPE: u64 = 0x8444_c004;
/// The value of `DCZID_EL0`: `dc zva` zeroes 64 bytes.
pub const ZERO_BLOCK_ID: u64 = 4;
/// `MIDR_EL1` as we present it: implementer 0x41 (Arm), architecture 0xf, part 0xd08, a generic ARMv8 core.
pub const MAIN_ID: u64 = 0x410f_d083;
/// `MPIDR_EL1` as Linux shows it to user space.
const MULTIPROCESSOR_ID: u64 = 0x8000_0000;
/// `ID_AA64PFR0_EL1` as Linux shows it for the CPU we present: EL0 and EL1 64 bit only, floating point
/// and vectors present (zero), nothing else.
const PROCESSOR_FEATURES_0: u64 = 0x11;
/// `ID_AA64ISAR0_EL1`: CRC32 (bits 19:16 = 1) and the single instruction atomics (bits 23:20 = 2).
const INSTRUCTION_SET_0: u64 = 0x0021_0000;

/// What `mrs` of an identity register gives under Linux, which traps and answers them: `None` where it
/// raises illegal instruction.
fn identity(crm: u32, op2: u32) -> Option<u64> {
    Some(match (crm, op2) {
        (0, 0) => MAIN_ID,
        (0, 5) => MULTIPROCESSOR_ID,
        (0, 6) => 0,
        (0, _) | (1, _) => return None,
        (4, 0) => PROCESSOR_FEATURES_0,
        (6, 0) => INSTRUCTION_SET_0,
        // Every other register of the space is a feature register with nothing we present, or reserved.
        _ => 0,
    })
}

fn system_move(f: &Fields) -> Instruction {
    let read = f.bit("L");
    let key = (
        2 + f.get("o0"),
        f.get("op1"),
        f.get("CRn"),
        f.get("CRm"),
        f.get("op2"),
    );
    let rt = f.zr("Rt");
    let register = match key {
        (3, 3, 4, 2, 0) => SystemRegister::Nzcv,
        (3, 3, 4, 4, 0) => SystemRegister::Fpcr,
        (3, 3, 4, 4, 1) => SystemRegister::Fpsr,
        (3, 3, 13, 0, 2) => SystemRegister::TpidrEl0,
        (3, 3, 13, 0, 3) if read => SystemRegister::Fixed(0),
        (3, 3, 14, 0, 2) if read => SystemRegister::Counter,
        (3, 3, 14, 0, 0) if read => SystemRegister::Fixed(COUNTER_FREQUENCY),
        (3, 3, 0, 0, 1) if read => SystemRegister::Fixed(CACHE_TYPE),
        (3, 3, 0, 0, 7) if read => SystemRegister::Fixed(ZERO_BLOCK_ID),
        // SSBS, DIT, TCO, RNDR, RNDRRS and CNTVCTSS_EL0: EL0 may use them on CPUs with their features.
        (3, 3, 4, 2, 5..=7) | (3, 3, 14, 0, 6) => return Instruction::Unsupported,
        (3, 3, 2, 4, 0 | 1) if read => return Instruction::Unsupported,
        (3, 0, 0, crm, op2) if read && (crm == 0 || (2..8).contains(&crm)) => {
            match identity(crm, op2) {
                Some(value) => SystemRegister::Fixed(value),
                None => return Instruction::Undefined,
            }
        }
        _ => return Instruction::Undefined,
    };
    if read {
        Instruction::ReadSystem { register, rt }
    } else {
        Instruction::WriteSystem { register, rt }
    }
}

/// The size in bytes of a floating point `ftype`: single, double, or half (only for conversions; the CPU
/// we present has no half precision arithmetic, which Arm's form conditions already rule out).
fn float_size(ftype: u32) -> Option<u8> {
    match ftype {
        0 => Some(4),
        1 => Some(8),
        3 => Some(2),
        _ => None,
    }
}

fn float(f: &Fields, stem: &str, group: &str) -> Option<Instruction> {
    use Instruction as I;
    if f.has("M") && (f.bit("M") || f.bit("S")) {
        return Some(I::Undefined);
    }
    let Some(size) = float_size(f.get("ftype")) else {
        return Some(I::Undefined);
    };
    let rd = if f.has("Rd") { f.v("Rd") } else { 0 };
    let rn = f.v("Rn");
    Some(match group {
        "floatdp1" => {
            let op = match stem {
                "FCVT" => {
                    let to = float_size(f.get("opc"))?;
                    return Some(I::FloatConvert {
                        from: size,
                        to,
                        rd,
                        rn,
                    });
                }
                "FMOV" => FloatUnary::Move,
                "FABS" => FloatUnary::Abs,
                "FNEG" => FloatUnary::Negate,
                "FSQRT" => FloatUnary::Sqrt,
                "FRINTN" => FloatUnary::Round(Rounding::Nearest),
                "FRINTP" => FloatUnary::Round(Rounding::Ceiling),
                "FRINTM" => FloatUnary::Round(Rounding::Floor),
                "FRINTZ" => FloatUnary::Round(Rounding::Zero),
                "FRINTA" => FloatUnary::Round(Rounding::Away),
                "FRINTX" => FloatUnary::Round(Rounding::CurrentExact),
                "FRINTI" => FloatUnary::Round(Rounding::Current),
                _ => return None,
            };
            I::Float { op, size, rd, rn }
        }
        "floatdp2" => I::FloatBinary {
            op: match stem {
                "FMUL" => FloatBinary::Mul,
                "FDIV" => FloatBinary::Div,
                "FADD" => FloatBinary::Add,
                "FSUB" => FloatBinary::Sub,
                "FMAX" => FloatBinary::Max,
                "FMIN" => FloatBinary::Min,
                "FMAXNM" => FloatBinary::MaxNumber,
                "FMINNM" => FloatBinary::MinNumber,
                "FNMUL" => FloatBinary::NegatedMul,
                _ => return None,
            },
            size,
            rd,
            rn,
            rm: f.v("Rm"),
        },
        "floatdp3" => {
            let (o1, o0) = (f.bit("o1"), f.bit("o0"));
            I::FloatMulAdd {
                negate_addend: o1,
                negate_product: o1 != o0,
                size,
                rd,
                rn,
                rm: f.v("Rm"),
                ra: f.v("Ra"),
            }
        }
        "floatcmp" => I::FloatCompare {
            signaling: stem == "FCMPE",
            size,
            rn,
            rm: (!form_name_ends(f, "Z_floatcmp")).then(|| f.v("Rm")),
        },
        "floatccmp" => I::FloatCondCompare {
            signaling: stem == "FCCMPE",
            size,
            rn,
            rm: f.v("Rm"),
            nzcv: f.get("nzcv") as u8,
            cond: Cond(f.get("cond") as u8),
        },
        "floatsel" => I::FloatSelect {
            size,
            rd,
            rn,
            rm: f.v("Rm"),
            cond: Cond(f.get("cond") as u8),
        },
        "float2int" | "float2fix" => {
            let width = if f.bit("sf") { Width::X64 } else { Width::W32 };
            let fraction_bits = if group == "float2fix" {
                let scale = f.get("scale");
                // A 32 bit integer has at most 32 fraction bits.
                if width == Width::W32 && scale < 32 {
                    return Some(I::Undefined);
                }
                (64 - scale) as u8
            } else {
                0
            };
            let signed = stem.ends_with('S');
            match stem {
                "SCVTF" | "UCVTF" => I::IntegerToFloat {
                    size,
                    width,
                    signed: stem == "SCVTF",
                    fraction_bits,
                    rd,
                    rn: f.zr("Rn"),
                },
                _ => I::FloatToInteger {
                    size,
                    width,
                    signed,
                    rounding: match &stem[..5] {
                        "FCVTN" => Rounding::Nearest,
                        "FCVTA" => Rounding::Away,
                        "FCVTP" => Rounding::Ceiling,
                        "FCVTM" => Rounding::Floor,
                        _ => Rounding::Zero,
                    },
                    fraction_bits,
                    rd: f.zr("Rd"),
                    rn,
                },
            }
        }
        _ => return None,
    })
}

/// fmov (immediate): Arm's `VFPExpandImm` of imm8 into the low element, the rest cleared.
fn float_immediate(f: &Fields) -> Instruction {
    let imm8 = u64::from(f.get("imm8"));
    let (sign, b6, top, low) = (imm8 >> 7, (imm8 >> 6) & 1, (imm8 >> 4) & 3, imm8 & 15);
    let imm = match f.get("ftype") {
        0 => {
            let exponent = ((b6 ^ 1) << 7) | (if b6 == 1 { 0x1f } else { 0 } << 2) | top;
            (sign << 31) | (exponent << 23) | (low << 19)
        }
        1 => {
            let exponent = ((b6 ^ 1) << 10) | (if b6 == 1 { 0xff } else { 0 } << 2) | top;
            (sign << 63) | (exponent << 52) | (low << 48)
        }
        _ => return Instruction::Undefined,
    };
    Instruction::VectorImmediate {
        op: VectorImmediate::Move,
        rd: f.v("Rd"),
        imm,
        whole: false,
    }
}

fn form_name_ends(f: &Fields, suffix: &str) -> bool {
    f.form.name.ends_with(suffix)
}
