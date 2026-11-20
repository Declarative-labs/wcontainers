//! The few ARM64 instructions the native output writes of its own (docs/native.md section 3.6): each word is
//! its form's fixed bits from Arm's table ([`crate::forms`]) with the named fields filled in, so no opcode or
//! bit position is written here.

use std::collections::HashMap;
use std::sync::OnceLock;

use wcontainers_guest_cpu::Guest;

use crate::Arm64;
use crate::forms::{self, Form};

/// The forms the native output writes, by Arm's names.
pub mod form {
    pub const MOVZ: &str = "MOVZ_64_movewide";
    pub const MOVK: &str = "MOVK_64_movewide";
    pub const MOVN: &str = "MOVN_64_movewide";
    pub const UBFM: &str = "UBFM_64M_bitfield";
    pub const BFM: &str = "BFM_64M_bitfield";
    pub const AND_IMMEDIATE: &str = "AND_64_log_imm";
    pub const EOR_IMMEDIATE: &str = "EOR_64_log_imm";
    pub const ADD_IMMEDIATE: &str = "ADD_64_addsub_imm";
    pub const SUB_IMMEDIATE: &str = "SUB_64_addsub_imm";
    pub const CBZ: &str = "CBZ_64_compbranch";
    pub const CBNZ: &str = "CBNZ_64_compbranch";
    pub const TBNZ: &str = "TBNZ_only_testbranch";
    pub const B: &str = "B_only_branch_imm";
    pub const BL: &str = "BL_only_branch_imm";
    pub const BR: &str = "BR_64_branch_reg";
    pub const BLR: &str = "BLR_64_branch_reg";
    pub const RET: &str = "RET_64R_branch_reg";
    pub const LDR: &str = "LDR_64_ldst_pos";
    pub const STR: &str = "STR_64_ldst_pos";
    pub const LDRB: &str = "LDRB_32_ldst_pos";
    pub const LDADDB: &str = "LDADDB_32_memop";
    pub const CAS: &str = "CAS_C64_comswap";
    pub const BRK: &str = "BRK_EX_exception";
    pub const UDF: &str = "UDF_only_perm_undef";
    pub const ADD_EXTENDED: &str = "ADD_64_addsub_ext";
    pub const SUB_EXTENDED: &str = "SUB_64_addsub_ext";
    pub const EOR_SHIFTED: &str = "EOR_64_log_shift";
    pub const ORR_SHIFTED: &str = "ORR_64_log_shift";
    pub const LDR_32: &str = "LDR_32_ldst_pos";
    pub const LDRH: &str = "LDRH_32_ldst_pos";
    pub const LDRSW: &str = "LDRSW_64_ldst_pos";
    pub const LDR_S: &str = "LDR_S_ldst_pos";
    pub const LDR_D: &str = "LDR_D_ldst_pos";
    pub const LDR_Q: &str = "LDR_Q_ldst_pos";
    pub const LDP_32: &str = "LDP_32_ldstpair_off";
    pub const LDP_64: &str = "LDP_64_ldstpair_off";
    pub const LDADDH: &str = "LDADDH_32_memop";
    pub const LDADD_32: &str = "LDADD_32_memop";
    pub const LDADD_64: &str = "LDADD_64_memop";
    pub const CASB: &str = "CASB_C32_comswap";
    pub const CASH: &str = "CASH_C32_comswap";
    pub const CAS_32: &str = "CAS_C32_comswap";
    pub const CASP: &str = "CASP_CP64_comswappr";
    pub const HVC: &str = "HVC_EX_exception";
    pub const ERET: &str = "ERET_64E_branch_reg";
    pub const STR_Q: &str = "STR_Q_ldst_pos";
    pub const MRS: &str = "MRS_RS_systemmove";
    pub const MSR: &str = "MSR_SR_systemmove";
    pub const SYS: &str = "SYS_CR_systeminstrs";
    pub const ADRP: &str = "ADRP_only_pcreladdr";
    pub const ADR: &str = "ADR_only_pcreladdr";

    /// Every form above.
    pub const ALL: [&str; 51] = [
        MOVZ,
        MOVK,
        MOVN,
        UBFM,
        BFM,
        AND_IMMEDIATE,
        EOR_IMMEDIATE,
        ADD_IMMEDIATE,
        SUB_IMMEDIATE,
        CBZ,
        CBNZ,
        TBNZ,
        B,
        BL,
        BR,
        BLR,
        RET,
        LDR,
        STR,
        LDRB,
        LDADDB,
        CAS,
        BRK,
        UDF,
        ADD_EXTENDED,
        SUB_EXTENDED,
        EOR_SHIFTED,
        ORR_SHIFTED,
        LDR_32,
        LDRH,
        LDRSW,
        LDR_S,
        LDR_D,
        LDR_Q,
        LDP_32,
        LDP_64,
        LDADDH,
        LDADD_32,
        LDADD_64,
        CASB,
        CASH,
        CAS_32,
        CASP,
        HVC,
        ERET,
        STR_Q,
        MRS,
        MSR,
        SYS,
        ADRP,
        ADR,
    ];
}

/// Arm's names of the fields filled in.
pub mod field {
    pub const RD: &str = "Rd";
    pub const RN: &str = "Rn";
    pub const RT: &str = "Rt";
    pub const RS: &str = "Rs";
    pub const IMM16: &str = "imm16";
    pub const HW: &str = "hw";
    pub const IMMR: &str = "immr";
    pub const IMMS: &str = "imms";
    pub const N: &str = "N";
    pub const IMM12: &str = "imm12";
    pub const IMM19: &str = "imm19";
    pub const IMM14: &str = "imm14";
    pub const IMM26: &str = "imm26";
    pub const IMMHI: &str = "immhi";
    pub const IMMLO: &str = "immlo";
    pub const B5: &str = "b5";
    pub const B40: &str = "b40";
    pub const RM: &str = "Rm";
    pub const RT2: &str = "Rt2";
    /// An exclusive's load bit: 1 loads, 0 stores.
    pub const LOAD: &str = "L";
    pub const OPTION: &str = "option";
    pub const IMM3: &str = "imm3";
    pub const IMM7: &str = "imm7";
    pub const OP1: &str = "op1";
    pub const CRN: &str = "CRn";
    pub const CRM: &str = "CRm";
    pub const OP2: &str = "op2";
    /// What the names of register fields start with.
    pub const REGISTER: &str = "R";
}

pub use crate::thread_state::Register;

/// The register number that is the zero register (or the stack pointer, as the field's form reads it).
pub const ZERO: Register = 31;

/// The bits of a register field.
pub const REGISTER_BITS: u32 = u8::BITS - ZERO.leading_zeros();

/// The bytes of one instruction: branch offsets count in them.
const UNIT: i64 = Arm64::MAX_LENGTH as i64;

/// The bits of a move's immediate.
const MOVE_BITS: u32 = u16::BITS;

/// An encoding whose values do not fit its fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DoesNotFit;

fn table() -> &'static HashMap<&'static str, &'static Form> {
    static TABLE: OnceLock<HashMap<&'static str, &'static Form>> = OnceLock::new();
    TABLE.get_or_init(|| {
        form::ALL
            .iter()
            .map(|&name| {
                let found = forms::all()
                    .iter()
                    .find(|f| f.name == name)
                    .unwrap_or_else(|| panic!("{name} is not in Arm's table"));
                (name, found)
            })
            .collect()
    })
}

fn field_of(name: &str, field: &str) -> (u32, u32) {
    table()[name]
        .fields
        .iter()
        .find(|(f, ..)| f == field)
        .map(|&(_, low, width)| (low, width))
        .unwrap_or_else(|| panic!("{name} has no field {field}"))
}

/// The bits of a field.
fn width(name: &str, field: &str) -> u32 {
    field_of(name, field).1
}

/// The word of form `name` with `fields` filled in; a value too wide for its field does not fit.
pub fn word(name: &str, fields: &[(&str, u32)]) -> Result<u32, DoesNotFit> {
    let form = table()[name];
    let mut word = form.value & form.mask;
    for &(field, value) in fields {
        let (low, width) = field_of(name, field);
        if u64::from(value) >> width != 0 {
            return Err(DoesNotFit);
        }
        word |= value << low;
    }
    Ok(word)
}

/// A signed field of `bits` bits holding `value` in units of `unit`, as branch offsets are.
fn signed(value: i64, unit: i64, bits: u32) -> Result<u32, DoesNotFit> {
    if value % unit != 0 {
        return Err(DoesNotFit);
    }
    let scaled = value / unit;
    let half = 1i64 << (bits - 1);
    if scaled < -half || scaled >= half {
        return Err(DoesNotFit);
    }
    Ok((scaled as u32) & ((1u32 << bits) - 1))
}

/// A wide move by form: `movz`, `movk` or `movn rd, #imm16, lsl #(16 * half)`.
pub fn move_wide(name: &str, rd: Register, imm16: u16, half: u32) -> u32 {
    let fields = [
        (field::RD, rd.into()),
        (field::IMM16, imm16.into()),
        (field::HW, half),
    ];
    word(name, &fields).expect("a move fits")
}

/// `movz rd, #imm16, lsl #(16 * half)`.
pub fn movz(rd: Register, imm16: u16, half: u32) -> u32 {
    move_wide(form::MOVZ, rd, imm16, half)
}

/// `movk rd, #imm16, lsl #(16 * half)`.
pub fn movk(rd: Register, imm16: u16, half: u32) -> u32 {
    move_wide(form::MOVK, rd, imm16, half)
}

/// The moves that build `value` in `rd`: a `movz` of its lowest half, or a `movn` of it when more halves are
/// all ones than are zero, then a `movk` for each higher half the first move did not already give.
pub fn move_constant(rd: Register, value: u64) -> Vec<u32> {
    let halves = u64::BITS / MOVE_BITS;
    let half = |h: u32| (value >> (h * MOVE_BITS)) as u16;
    let all = |part: u16| (0..halves).filter(|&h| half(h) == part).count();
    let (first, filled) = if all(u16::MAX) > all(0) {
        (move_wide(form::MOVN, rd, !half(0), 0), u16::MAX)
    } else {
        (movz(rd, half(0), 0), 0)
    };
    std::iter::once(first)
        .chain(
            (1..halves)
                .filter(|&h| half(h) != filled)
                .map(|h| movk(rd, half(h), h)),
        )
        .collect()
}

/// The moves that build `value` in `rd` in as many words for every value whose top half is the same all zeros or
/// all ones (an address under 2^48, or an entry slot's address): a `movz`, or a `movn` when the top half is all ones,
/// of the lowest half, then a `movk` for each half below the top. Any other value takes [`move_constant`]'s.
pub fn move_address(rd: Register, value: u64) -> Vec<u32> {
    let halves = u64::BITS / MOVE_BITS;
    let half = |h: u32| (value >> (h * MOVE_BITS)) as u16;
    let first = match half(halves - 1) {
        0 => movz(rd, half(0), 0),
        u16::MAX => move_wide(form::MOVN, rd, !half(0), 0),
        _ => return move_constant(rd, value),
    };
    std::iter::once(first)
        .chain((1..halves - 1).map(|h| movk(rd, half(h), h)))
        .collect()
}

/// How far a direct branch reaches either way, in bytes.
pub fn branch_reach() -> u64 {
    (UNIT as u64) << (width(form::B, field::IMM26) - 1)
}

/// How far `adrp` reaches either way from its page, in bytes, for pages of `page` bytes.
pub fn page_reach(page: u64) -> u64 {
    page << (width(form::ADRP, field::IMMHI) + width(form::ADRP, field::IMMLO) - 1)
}

fn bitfield(name: &str, rd: Register, rn: Register, immr: u32, imms: u32) -> u32 {
    let fields = [
        (field::RD, rd.into()),
        (field::RN, rn.into()),
        (field::IMMR, immr),
        (field::IMMS, imms),
    ];
    word(name, &fields).expect("a bit field of a whole register fits")
}

/// `ubfx rd, rn, #lsb, #bits`: `bits` bits from `lsb`, at the bottom.
pub fn ubfx(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(form::UBFM, rd, rn, lsb, lsb + bits - 1)
}

/// `ubfiz rd, rn, #lsb, #bits`: the low `bits` bits moved up to `lsb`, the rest zero.
pub fn ubfiz(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(form::UBFM, rd, rn, (u64::BITS - lsb) % u64::BITS, bits - 1)
}

/// `bfxil rd, rn, #lsb, #bits`: `bits` bits of `rn` from `lsb` into the bottom of `rd`, the rest kept.
pub fn bfxil(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(form::BFM, rd, rn, lsb, lsb + bits - 1)
}

/// The N, immr and imms of a 64-bit logical immediate, if `value` is one: a run of ones, rotated, repeated in
/// elements of 2 to 64 bits.
pub fn logical_immediate(value: u64) -> Option<(u32, u32, u32)> {
    if value == 0 || value == u64::MAX {
        return None;
    }
    let mut size = u64::BITS;
    while size > 2 {
        let half = size / 2;
        let mask = (1u64 << half) - 1;
        if value & mask != (value >> half) & mask {
            break;
        }
        size = half;
    }
    let mask = u64::MAX >> (u64::BITS - size);
    let element = value & mask;
    let ones = element.count_ones();
    let run = (1u64 << ones) - 1;
    let ror = |r: u32| ((run >> r) | (run << ((size - r) % size))) & mask;
    let immr = (0..size).find(|&r| ror(r) == element)?;
    // imms: ones above a zero mark the element's size, and below them the run's length less one.
    let imms_mask = (1u32 << width(form::AND_IMMEDIATE, field::IMMS)) - 1;
    let imms = (!(2 * size - 1) & imms_mask) | (ones - 1);
    Some((u32::from(size == u64::BITS), immr, imms))
}

fn logical(name: &str, rd: Register, rn: Register, value: u64) -> Result<u32, DoesNotFit> {
    let (n, immr, imms) = logical_immediate(value).ok_or(DoesNotFit)?;
    let fields = [
        (field::RD, rd.into()),
        (field::RN, rn.into()),
        (field::N, n),
        (field::IMMR, immr),
        (field::IMMS, imms),
    ];
    word(name, &fields)
}

/// `and rd, rn, #value`.
pub fn and_immediate(rd: Register, rn: Register, value: u64) -> Result<u32, DoesNotFit> {
    logical(form::AND_IMMEDIATE, rd, rn, value)
}

/// `eor rd, rn, #value`.
pub fn eor_immediate(rd: Register, rn: Register, value: u64) -> Result<u32, DoesNotFit> {
    logical(form::EOR_IMMEDIATE, rd, rn, value)
}

/// `add rd, rn, #value` (`sub` for a negative value), unshifted.
pub fn add_immediate(rd: Register, rn: Register, value: i64) -> Result<u32, DoesNotFit> {
    let name = if value < 0 {
        form::SUB_IMMEDIATE
    } else {
        form::ADD_IMMEDIATE
    };
    let imm = u32::try_from(value.unsigned_abs()).map_err(|_| DoesNotFit)?;
    let fields = [
        (field::RD, rd.into()),
        (field::RN, rn.into()),
        (field::IMM12, imm),
    ];
    word(name, &fields)
}

/// `cbz rt, offset` (`nonzero`: `cbnz`), the offset in bytes from this instruction.
pub fn compare_branch(nonzero: bool, rt: Register, offset: i64) -> Result<u32, DoesNotFit> {
    let name = if nonzero { form::CBNZ } else { form::CBZ };
    let imm19 = signed(offset, UNIT, width(name, field::IMM19))?;
    word(name, &[(field::RT, rt.into()), (field::IMM19, imm19)])
}

/// `tbnz rt, #bit, offset`.
pub fn test_branch_nonzero(rt: Register, bit: u32, offset: i64) -> Result<u32, DoesNotFit> {
    let imm14 = signed(offset, UNIT, width(form::TBNZ, field::IMM14))?;
    let low = width(form::TBNZ, field::B40);
    let fields = [
        (field::RT, rt.into()),
        (field::B5, bit >> low),
        (field::B40, bit & ((1 << low) - 1)),
        (field::IMM14, imm14),
    ];
    word(form::TBNZ, &fields)
}

/// `b offset` (`link`: `bl`).
pub fn branch(link: bool, offset: i64) -> Result<u32, DoesNotFit> {
    let name = if link { form::BL } else { form::B };
    let imm26 = signed(offset, UNIT, width(name, field::IMM26))?;
    word(name, &[(field::IMM26, imm26)])
}

/// `br rn`, `blr rn` or `ret rn`, by form.
pub fn branch_register(name: &str, rn: Register) -> u32 {
    word(name, &[(field::RN, rn.into())]).expect("a register fits")
}

/// `ldr rt, [rn, #offset]`, `str` or `ldrb`, by form, the offset a multiple of the access's `unit`.
pub fn load_store(
    name: &str,
    rt: Register,
    rn: Register,
    offset: u64,
    unit: u64,
) -> Result<u32, DoesNotFit> {
    if !offset.is_multiple_of(unit) {
        return Err(DoesNotFit);
    }
    let scaled = u32::try_from(offset / unit).map_err(|_| DoesNotFit)?;
    let fields = [
        (field::RT, rt.into()),
        (field::RN, rn.into()),
        (field::IMM12, scaled),
    ];
    word(name, &fields)
}

/// The field `field` of a word of any form Arm's table has, set to `value`: a copied instruction with a register
/// changed.
pub fn replace(word: u32, field: &str, value: u32) -> Result<u32, DoesNotFit> {
    let form = forms::form(word).ok_or(DoesNotFit)?;
    let &(_, low, width) = form
        .fields
        .iter()
        .find(|(f, ..)| f == field)
        .ok_or(DoesNotFit)?;
    if u64::from(value) >> width != 0 {
        return Err(DoesNotFit);
    }
    let mask = ((1u64 << width) - 1) as u32;
    Ok(word & !(mask << low) | value << low)
}

/// A copied direct branch of any form, its offset changed to `offset` bytes.
pub fn retarget(word: u32, offset: i64) -> Result<u32, DoesNotFit> {
    let form = forms::form(word).ok_or(DoesNotFit)?;
    let &(ref name, _, width) = form
        .fields
        .iter()
        .find(|(f, ..)| [field::IMM26, field::IMM19, field::IMM14].contains(&f.as_str()))
        .ok_or(DoesNotFit)?;
    replace(word, name, signed(offset, UNIT, width)?)
}

/// How an extended-register operand is extended: from `from_bits` bits, signed or not.
fn option(signed: bool, from_bits: u32) -> u32 {
    let sign = width(form::ADD_EXTENDED, field::OPTION) - 1;
    u32::from(signed) << sign | (from_bits / u8::BITS).trailing_zeros()
}

/// `adr rd, #offset`: the address `offset` bytes from the instruction's own.
pub fn adr(rd: Register, offset: i64) -> Result<u32, DoesNotFit> {
    let low = width(form::ADR, field::IMMLO);
    let value = signed(offset, 1, low + width(form::ADR, field::IMMHI))?;
    word(
        form::ADR,
        &[
            (field::RD, rd.into()),
            (field::IMMLO, value & ((1 << low) - 1)),
            (field::IMMHI, value >> low),
        ],
    )
}

/// `add rd, rn, rm, extend #shift` (`subtract`: `sub`), where `rn` 31 is the stack pointer.
pub fn add_extended(
    subtract: bool,
    rd: Register,
    rn: Register,
    rm: Register,
    signed: bool,
    from_bits: u32,
    shift: u32,
) -> Result<u32, DoesNotFit> {
    let name = if subtract {
        form::SUB_EXTENDED
    } else {
        form::ADD_EXTENDED
    };
    let fields = [
        (field::RD, rd.into()),
        (field::RN, rn.into()),
        (field::RM, rm.into()),
        (field::OPTION, option(signed, from_bits)),
        (field::IMM3, shift),
    ];
    word(name, &fields)
}

/// `eor rd, rn, rm` or `orr rd, rn, rm`, by form, unshifted.
pub fn logical_register(name: &str, rd: Register, rn: Register, rm: Register) -> u32 {
    let fields = [
        (field::RD, rd.into()),
        (field::RN, rn.into()),
        (field::RM, rm.into()),
    ];
    word(name, &fields).expect("registers fit")
}

/// `mov rd, rn` of general registers (`orr rd, xzr, rn`).
pub fn move_register(rd: Register, rn: Register) -> u32 {
    logical_register(form::ORR_SHIFTED, rd, ZERO, rn)
}

/// `bfi rd, rn, #lsb, #bits`: the low `bits` bits of `rn` put at `lsb` in `rd`, the rest kept.
pub fn bfi(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(form::BFM, rd, rn, (u64::BITS - lsb) % u64::BITS, bits - 1)
}

/// `ldp rt, rt2, [rn]` of 32-bit or 64-bit registers, by form.
pub fn load_pair(name: &str, rt: Register, rt2: Register, rn: Register) -> u32 {
    let fields = [
        (field::RT, rt.into()),
        (field::RT2, rt2.into()),
        (field::RN, rn.into()),
        (field::IMM7, 0),
    ];
    word(name, &fields).expect("registers fit")
}

/// `ldadd{b,h} rs, rt, [rn]` and the other sizes, by form; with `rt` the zero register it is `stadd`.
pub fn load_add(name: &str, rs: Register, rt: Register, rn: Register) -> u32 {
    let fields = [
        (field::RS, rs.into()),
        (field::RT, rt.into()),
        (field::RN, rn.into()),
    ];
    word(name, &fields).expect("registers fit")
}

/// `cas{b,h} rs, rt, [rn]` of any size, or `casp` of two even pairs, by form.
pub fn compare_swap_of(name: &str, rs: Register, rt: Register, rn: Register) -> u32 {
    let fields = [
        (field::RS, rs.into()),
        (field::RT, rt.into()),
        (field::RN, rn.into()),
    ];
    word(name, &fields).expect("registers fit")
}

/// `eret`: back to the level and address the exception registers hold.
pub fn exception_return() -> u32 {
    word(form::ERET, &[]).expect("no fields")
}

/// The system instruction `tlbi vmalle1is`: every TLB entry of the EL1&0 regime, on every CPU of the inner
/// shareable domain (Arm ARM, TLBI VMALLE1IS).
const INVALIDATE_WHOLE_TLB: [(&str, u32); 4] = [
    (field::OP1, 0),
    (field::CRN, 8),
    (field::CRM, 3),
    (field::OP2, 0),
];

/// `tlbi vmalle1is`.
pub fn invalidate_whole_tlb() -> u32 {
    let mut fields = INVALIDATE_WHOLE_TLB.to_vec();
    fields.push((field::RT, ZERO.into()));
    word(form::SYS, &fields).expect("the system instruction's fields fit")
}

/// `brk #imm16`, `udf #imm16` or `hvc #imm16`, by form.
pub fn exception(name: &str, imm16: u16) -> u32 {
    word(name, &[(field::IMM16, imm16.into())]).expect("the immediate fits its field")
}

/// The moves of the float status registers between them and general register `rt`, found by trying every
/// system register a move can name and keeping those the native routing and stops single out, so no system
/// register number is written here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloatStatusMoves {
    pub read_fpsr: u32,
    pub read_fpcr: u32,
    pub write_fpsr: u32,
    pub write_fpcr: u32,
}

/// The one word of `form_name` with register `rt` that `wanted` picks out of all its system registers.
fn only_move(
    form_name: &str,
    rt: Register,
    wanted: impl Fn(&crate::instruction::Instruction) -> bool,
) -> u32 {
    let form = table()[form_name];
    let (rt_low, rt_width) = field_of(form_name, field::RT);
    let mut found = (0..=!form.mask)
        .step_by(1 << (rt_low + rt_width))
        .map(|free| form.word(free) | (u32::from(rt) << rt_low))
        .filter(|&word| form.presented(word) && wanted(&crate::decode::decode(word)));
    let word = found.next().expect("a move of the register");
    assert_eq!(found.next(), None, "one move of the register");
    word
}

impl FloatStatusMoves {
    pub fn with(rt: Register) -> FloatStatusMoves {
        use wcontainers_guest_cpu::{Native, Route, Stopping};
        let float_status = |i: &_| <Arm64 as Native>::route(i) == Route::FloatStatus;
        let status_write = |i: &_| <Arm64 as Native>::stopping(i, 0) == Stopping::FloatStatusWrite;
        let write_fpsr = only_move(form::MSR, rt, status_write);
        let write_fpcr = only_move(form::MSR, rt, |i| float_status(i) && !status_write(i));
        let read_fpsr = only_move(form::MRS, rt, float_status);
        let mrs = table()[form::MRS];
        let msr = table()[form::MSR];
        let read_fpcr = mrs.word(write_fpcr & !msr.mask);
        assert!(
            mrs.presented(read_fpcr),
            "the read of the register FPCR's write names"
        );
        FloatStatusMoves {
            read_fpsr,
            read_fpcr,
            write_fpsr,
            write_fpcr,
        }
    }
}
