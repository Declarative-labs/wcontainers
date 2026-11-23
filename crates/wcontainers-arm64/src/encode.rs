//! The few ARM64 instructions the native output writes of its own (docs/native.md section 3.6): each word is
//! its form's fixed bits from Arm's table ([`crate::forms`]) with the named fields filled in, so no opcode or
//! bit position is written here. Each builder's form and fields are found in the table once, on first use.

use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

use wcontainers_guest_cpu::Guest;

use crate::Arm64;
use crate::forms;

/// A [`Form`] constant for each of the names in [`name`], and their list.
macro_rules! written_forms {
    ($($constant:ident,)*) => {
        /// The forms the native output writes.
        pub mod form {
            use super::{Form, name};

            /// Each form's place in [`ALL`].
            #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
            enum Place {
                $($constant,)*
            }

            $(pub const $constant: Form = Form(Place::$constant as usize);)*

            /// Arm's name of each form, in [`ALL`]'s order.
            pub(super) const NAMES: [&str; [$(name::$constant),*].len()] = [$(name::$constant),*];

            /// Every form above.
            pub const ALL: [Form; NAMES.len()] = [$($constant),*];
        }
    };
}

/// A form the native output writes: its place in [`form::ALL`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Form(usize);

impl std::fmt::Debug for Form {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

impl Form {
    /// Arm's name of the form.
    pub fn name(self) -> &'static str {
        form::NAMES[self.0]
    }

    /// The form in Arm's table.
    fn arm(self) -> &'static forms::Form {
        ENCODER_FORMS[self.0]
    }
}

/// Arm's names of the forms the native output writes.
mod name {
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
}

written_forms! {
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
}

/// Arm's names of the fields the encoder and its callers name.
mod field_name {
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
    pub const LOAD: &str = "L";
    pub const OPTION: &str = "option";
    pub const IMM3: &str = "imm3";
    pub const IMM7: &str = "imm7";
    pub const O0: &str = "o0";
    pub const OP1: &str = "op1";
    pub const CRN: &str = "CRn";
    pub const CRM: &str = "CRm";
    pub const OP2: &str = "op2";
    pub const REGISTER: &str = "R";
}

/// A [`FieldName`] constant for each of the names in [`field_name`], and their list.
macro_rules! written_fields {
    ($($(#[$attribute:meta])* $constant:ident,)*) => {
        /// The fields the encoder and its callers name.
        pub mod field {
            use super::{FieldName, field_name};

            /// Each name's place in [`NAMES`].
            #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
            enum Place {
                $($constant,)*
            }

            $($(#[$attribute])* pub const $constant: FieldName = FieldName(Place::$constant as u16);)*

            /// Arm's name of each field above, in their order: the first places of [`FieldName`].
            pub(super) const NAMES: [&str; [$(field_name::$constant),*].len()] = [$(field_name::$constant),*];

            /// What the names of register fields start with.
            pub const REGISTER: &str = field_name::REGISTER;
        }
    };
}

written_fields! {
    RD,
    RN,
    RT,
    RS,
    IMM16,
    HW,
    IMMR,
    IMMS,
    N,
    IMM12,
    IMM19,
    IMM14,
    IMM26,
    IMMHI,
    IMMLO,
    B5,
    B40,
    RM,
    RT2,
    /// An exclusive's load bit: 1 loads, 0 stores.
    LOAD,
    OPTION,
    IMM3,
    IMM7,
    /// A system register move's register's low bit of op0.
    O0,
    OP1,
    CRN,
    CRM,
    OP2,
}

/// A name Arm's table gives fields: its place among every such name, [`field`]'s first and then the others in
/// the table's order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FieldName(u16);

impl std::fmt::Debug for FieldName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

impl FieldName {
    /// Arm's name.
    pub fn name(self) -> &'static str {
        FIELDS.names[usize::from(self.0)]
    }
}

/// Every field of Arm's table by its [`FieldName`], read once.
struct Fields {
    /// Arm's name of each [`FieldName`], by its place.
    names: Vec<&'static str>,
    /// The fields of each form of [`forms::all`], in its order, each in the order of the form's `fields`.
    of_form: Vec<Box<[(FieldName, Field)]>>,
}

static FIELDS: LazyLock<Fields> = LazyLock::new(|| {
    let mut names = field::NAMES.to_vec();
    let mut places: HashMap<&str, FieldName> = HashMap::new();
    let mut named = |name: &'static str| {
        *places.entry(name).or_insert_with(|| {
            let place = names.iter().position(|&n| n == name).unwrap_or_else(|| {
                names.push(name);
                names.len() - 1
            });
            FieldName(u16::try_from(place).expect("a field name's place fits its type"))
        })
    };
    let of_form = (forms::all().iter())
        .map(|form| {
            (form.fields.iter())
                .map(|(name, low, width)| {
                    let (low, width) = (*low, *width);
                    (named(name), Field { low, width })
                })
                .collect()
        })
        .collect();
    Fields { names, of_form }
});

/// The fields of `form`, a form of [`forms::all`].
fn fields_of(form: &forms::Form) -> &'static [(FieldName, Field)] {
    let place = (forms::all().element_offset(form)).expect("a form of Arm's table");
    &FIELDS.of_form[place]
}

/// The names of the fields of `form`, a form of [`forms::all`], in the order of its `fields`.
pub fn field_names(form: &forms::Form) -> impl Iterator<Item = FieldName> + use<> {
    fields_of(form).iter().map(|&(name, _)| name)
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

/// The form named `name` in Arm's table.
fn arm(name: &str) -> &'static forms::Form {
    (forms::all().iter())
        .find(|form| form.name == name)
        .unwrap_or_else(|| panic!("{name} is not in Arm's table"))
}

/// Each of [`form::ALL`], from Arm's table.
static ENCODER_FORMS: LazyLock<[&forms::Form; form::ALL.len()]> =
    LazyLock::new(|| form::NAMES.map(arm));

/// A field of a form: its lowest bit and its width.
#[derive(Clone, Copy)]
struct Field {
    low: u32,
    width: u32,
}

impl Field {
    /// The field `name` of `form`, a form of [`forms::all`], if it has one.
    fn of(form: &forms::Form, name: FieldName) -> Option<Field> {
        (fields_of(form).iter())
            .find(|&&(field, _)| field == name)
            .map(|&(_, field)| field)
    }

    /// The field `field` of `form`, which has it.
    fn written(form: Form, field: FieldName) -> Field {
        Field::of(form.arm(), field).unwrap_or_else(|| panic!("{form:?} has no field {field:?}"))
    }

    /// `value` in the field's place; a value too wide for the field does not fit.
    fn place(self, value: u32) -> Result<u32, DoesNotFit> {
        if u64::from(value) >> self.width != 0 {
            return Err(DoesNotFit);
        }
        Ok(value << self.low)
    }

    /// `word` with the field set to `value`; a value too wide for the field does not fit.
    fn set(self, word: u32, value: u32) -> Result<u32, DoesNotFit> {
        let mask = ((1u64 << self.width) - 1) as u32;
        Ok(word & !(mask << self.low) | self.place(value)?)
    }
}

/// A form's fixed bits and `N` of its fields, in the order their values are given: what a builder needs of
/// the form, found once in Arm's table.
#[derive(Clone, Copy)]
struct Shape<const N: usize> {
    fixed: u32,
    fields: [Field; N],
}

impl<const N: usize> Shape<N> {
    /// The form `form` with the fields `fields`, which it has.
    fn written(form: Form, fields: [FieldName; N]) -> Self {
        Shape {
            fixed: fixed(form.arm()),
            fields: fields.map(|field| Field::written(form, field)),
        }
    }

    /// The word with `values` in the fields; a value too wide for its field does not fit.
    fn word(&self, values: [u32; N]) -> Result<u32, DoesNotFit> {
        let mut word = self.fixed;
        for (field, value) in self.fields.iter().zip(values) {
            word |= field.place(value)?;
        }
        Ok(word)
    }
}

/// The fixed bits of a form.
fn fixed(form: &forms::Form) -> u32 {
    form.value & form.mask
}

/// The fields `fields` of every form of [`form::ALL`], each form's fixed bits and those of the fields it has,
/// in [`form::ALL`]'s order: what a builder that is given its form can write.
struct Kind<const N: usize> {
    fields: [FieldName; N],
    forms: [(u32, [Option<Field>; N]); form::ALL.len()],
}

impl<const N: usize> Kind<N> {
    fn of(fields: [FieldName; N]) -> Self {
        let forms =
            ENCODER_FORMS.map(|form| (fixed(form), fields.map(|name| Field::of(form, name))));
        Kind { fields, forms }
    }

    /// The word of `form` with `values` in the kind's fields, placed in their order; a value too wide for its
    /// field does not fit, and `form` has every field up to the last value placed.
    fn word(&self, form: Form, values: [u32; N]) -> Result<u32, DoesNotFit> {
        let (mut word, fields) = self.forms[form.0];
        for ((field, value), name) in fields.into_iter().zip(values).zip(self.fields) {
            let field = field.unwrap_or_else(|| panic!("{form:?} has no field {name:?}"));
            word |= field.place(value)?;
        }
        Ok(word)
    }
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

/// The fields of a wide move, in the order of [`move_of`]'s values.
const MOVE_FIELDS: [FieldName; 3] = [field::RD, field::IMM16, field::HW];
static MOVES: LazyLock<Kind<3>> = LazyLock::new(|| Kind::of(MOVE_FIELDS));
static MOVZ: LazyLock<Shape<3>> = LazyLock::new(|| Shape::written(form::MOVZ, MOVE_FIELDS));
static MOVK: LazyLock<Shape<3>> = LazyLock::new(|| Shape::written(form::MOVK, MOVE_FIELDS));
static MOVN: LazyLock<Shape<3>> = LazyLock::new(|| Shape::written(form::MOVN, MOVE_FIELDS));

fn move_of(shape: Shape<{ MOVE_FIELDS.len() }>, rd: Register, imm16: u16, half: u32) -> u32 {
    shape
        .word([rd.into(), imm16.into(), half])
        .expect("a move fits")
}

/// A wide move by form: `movz`, `movk` or `movn rd, #imm16, lsl #(16 * half)`.
pub fn move_wide(form: Form, rd: Register, imm16: u16, half: u32) -> u32 {
    MOVES
        .word(form, [rd.into(), imm16.into(), half])
        .expect("a move fits")
}

/// `movz rd, #imm16, lsl #(16 * half)`.
pub fn movz(rd: Register, imm16: u16, half: u32) -> u32 {
    move_of(*MOVZ, rd, imm16, half)
}

/// `movk rd, #imm16, lsl #(16 * half)`.
pub fn movk(rd: Register, imm16: u16, half: u32) -> u32 {
    move_of(*MOVK, rd, imm16, half)
}

/// The moves that build `value` in `rd`: a `movz` of its lowest half, or a `movn` of it when more halves are
/// all ones than are zero, then a `movk` for each higher half the first move did not already give.
pub fn move_constant(rd: Register, value: u64) -> Vec<u32> {
    let halves = u64::BITS / MOVE_BITS;
    let half = |h: u32| (value >> (h * MOVE_BITS)) as u16;
    let all = |part: u16| (0..halves).filter(|&h| half(h) == part).count();
    let (first, filled) = if all(u16::MAX) > all(0) {
        (move_of(*MOVN, rd, !half(0), 0), u16::MAX)
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
        u16::MAX => move_of(*MOVN, rd, !half(0), 0),
        _ => return move_constant(rd, value),
    };
    std::iter::once(first)
        .chain((1..halves - 1).map(|h| movk(rd, half(h), h)))
        .collect()
}

/// How far a direct branch reaches either way, in bytes.
pub fn branch_reach() -> u64 {
    let [imm26] = B.fields;
    (UNIT as u64) << (imm26.width - 1)
}

/// How far `adrp` reaches either way from its page, in bytes, for pages of `page` bytes.
pub fn page_reach(page: u64) -> u64 {
    static PAGES: LazyLock<Shape<2>> =
        LazyLock::new(|| Shape::written(form::ADRP, [field::IMMHI, field::IMMLO]));
    let [high, low] = PAGES.fields;
    page << (high.width + low.width - 1)
}

/// The fields of a bit field move, in the order of [`bitfield`]'s values.
const BITFIELD_FIELDS: [FieldName; 4] = [field::RD, field::RN, field::IMMR, field::IMMS];
static UBFM: LazyLock<Shape<4>> = LazyLock::new(|| Shape::written(form::UBFM, BITFIELD_FIELDS));
static BFM: LazyLock<Shape<4>> = LazyLock::new(|| Shape::written(form::BFM, BITFIELD_FIELDS));

fn bitfield(
    shape: Shape<{ BITFIELD_FIELDS.len() }>,
    rd: Register,
    rn: Register,
    immr: u32,
    imms: u32,
) -> u32 {
    shape
        .word([rd.into(), rn.into(), immr, imms])
        .expect("a bit field of a whole register fits")
}

/// `ubfx rd, rn, #lsb, #bits`: `bits` bits from `lsb`, at the bottom.
pub fn ubfx(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(*UBFM, rd, rn, lsb, lsb + bits - 1)
}

/// `ubfiz rd, rn, #lsb, #bits`: the low `bits` bits moved up to `lsb`, the rest zero.
pub fn ubfiz(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(*UBFM, rd, rn, (u64::BITS - lsb) % u64::BITS, bits - 1)
}

/// `bfxil rd, rn, #lsb, #bits`: `bits` bits of `rn` from `lsb` into the bottom of `rd`, the rest kept.
pub fn bfxil(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(*BFM, rd, rn, lsb, lsb + bits - 1)
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
    let [.., imms] = AND_IMMEDIATE.fields;
    let imms_mask = (1u32 << imms.width) - 1;
    let imms = (!(2 * size - 1) & imms_mask) | (ones - 1);
    Some((u32::from(size == u64::BITS), immr, imms))
}

/// The fields of a logical immediate, in the order of [`logical`]'s values.
const LOGICAL_FIELDS: [FieldName; 5] = [field::RD, field::RN, field::N, field::IMMR, field::IMMS];
static AND_IMMEDIATE: LazyLock<Shape<5>> =
    LazyLock::new(|| Shape::written(form::AND_IMMEDIATE, LOGICAL_FIELDS));
static EOR_IMMEDIATE: LazyLock<Shape<5>> =
    LazyLock::new(|| Shape::written(form::EOR_IMMEDIATE, LOGICAL_FIELDS));

fn logical(
    shape: Shape<{ LOGICAL_FIELDS.len() }>,
    rd: Register,
    rn: Register,
    value: u64,
) -> Result<u32, DoesNotFit> {
    let (n, immr, imms) = logical_immediate(value).ok_or(DoesNotFit)?;
    shape.word([rd.into(), rn.into(), n, immr, imms])
}

/// `and rd, rn, #value`.
pub fn and_immediate(rd: Register, rn: Register, value: u64) -> Result<u32, DoesNotFit> {
    logical(*AND_IMMEDIATE, rd, rn, value)
}

/// `eor rd, rn, #value`.
pub fn eor_immediate(rd: Register, rn: Register, value: u64) -> Result<u32, DoesNotFit> {
    logical(*EOR_IMMEDIATE, rd, rn, value)
}

/// The fields of an immediate add, in the order of [`add_immediate`]'s values.
const ADD_IMMEDIATE_FIELDS: [FieldName; 3] = [field::RD, field::RN, field::IMM12];
static ADD_IMMEDIATE: LazyLock<Shape<3>> =
    LazyLock::new(|| Shape::written(form::ADD_IMMEDIATE, ADD_IMMEDIATE_FIELDS));
static SUB_IMMEDIATE: LazyLock<Shape<3>> =
    LazyLock::new(|| Shape::written(form::SUB_IMMEDIATE, ADD_IMMEDIATE_FIELDS));

/// `add rd, rn, #value` (`sub` for a negative value), unshifted.
pub fn add_immediate(rd: Register, rn: Register, value: i64) -> Result<u32, DoesNotFit> {
    let shape = if value < 0 {
        *SUB_IMMEDIATE
    } else {
        *ADD_IMMEDIATE
    };
    let imm = u32::try_from(value.unsigned_abs()).map_err(|_| DoesNotFit)?;
    shape.word([rd.into(), rn.into(), imm])
}

/// The fields of a compare and branch, in the order of [`compare_branch`]'s values.
const COMPARE_BRANCH_FIELDS: [FieldName; 2] = [field::RT, field::IMM19];
static CBZ: LazyLock<Shape<2>> = LazyLock::new(|| Shape::written(form::CBZ, COMPARE_BRANCH_FIELDS));
static CBNZ: LazyLock<Shape<2>> =
    LazyLock::new(|| Shape::written(form::CBNZ, COMPARE_BRANCH_FIELDS));

/// `cbz rt, offset` (`nonzero`: `cbnz`), the offset in bytes from this instruction.
pub fn compare_branch(nonzero: bool, rt: Register, offset: i64) -> Result<u32, DoesNotFit> {
    let shape = if nonzero { *CBNZ } else { *CBZ };
    let [_, imm19] = shape.fields;
    let imm19 = signed(offset, UNIT, imm19.width)?;
    shape.word([rt.into(), imm19])
}

static TBNZ: LazyLock<Shape<4>> =
    LazyLock::new(|| Shape::written(form::TBNZ, [field::RT, field::B5, field::B40, field::IMM14]));

/// `tbnz rt, #bit, offset`.
pub fn test_branch_nonzero(rt: Register, bit: u32, offset: i64) -> Result<u32, DoesNotFit> {
    let [_, _, b40, imm14] = TBNZ.fields;
    let imm14 = signed(offset, UNIT, imm14.width)?;
    let low = b40.width;
    TBNZ.word([rt.into(), bit >> low, bit & ((1 << low) - 1), imm14])
}

static B: LazyLock<Shape<1>> = LazyLock::new(|| Shape::written(form::B, [field::IMM26]));
static BL: LazyLock<Shape<1>> = LazyLock::new(|| Shape::written(form::BL, [field::IMM26]));

/// `b offset` (`link`: `bl`).
pub fn branch(link: bool, offset: i64) -> Result<u32, DoesNotFit> {
    let shape = if link { *BL } else { *B };
    let [imm26] = shape.fields;
    shape.word([signed(offset, UNIT, imm26.width)?])
}

static BRANCH_REGISTERS: LazyLock<Kind<1>> = LazyLock::new(|| Kind::of([field::RN]));

/// `br rn`, `blr rn` or `ret rn`, by form.
pub fn branch_register(form: Form, rn: Register) -> u32 {
    BRANCH_REGISTERS
        .word(form, [rn.into()])
        .expect("a register fits")
}

static LOADS_AND_STORES: LazyLock<Kind<3>> =
    LazyLock::new(|| Kind::of([field::RT, field::RN, field::IMM12]));

/// `ldr rt, [rn, #offset]`, `str` or `ldrb`, by form, the offset a multiple of the access's `unit`.
pub fn load_store(
    form: Form,
    rt: Register,
    rn: Register,
    offset: u64,
    unit: u64,
) -> Result<u32, DoesNotFit> {
    if !offset.is_multiple_of(unit) {
        return Err(DoesNotFit);
    }
    let scaled = u32::try_from(offset / unit).map_err(|_| DoesNotFit)?;
    LOADS_AND_STORES.word(form, [rt.into(), rn.into(), scaled])
}

/// The field `field` of a word of any form Arm's table has, set to `value`: a copied instruction with a register
/// changed.
pub fn replace(word: u32, field: FieldName, value: u32) -> Result<u32, DoesNotFit> {
    let form = forms::form(word).ok_or(DoesNotFit)?;
    Field::of(form, field).ok_or(DoesNotFit)?.set(word, value)
}

/// The fields that hold a direct branch's offset.
const OFFSET_FIELDS: [FieldName; 3] = [field::IMM26, field::IMM19, field::IMM14];

/// A copied direct branch of any form, its offset changed to `offset` bytes.
pub fn retarget(word: u32, offset: i64) -> Result<u32, DoesNotFit> {
    let form = forms::form(word).ok_or(DoesNotFit)?;
    let &(_, field) = (fields_of(form).iter())
        .find(|(name, _)| OFFSET_FIELDS.contains(name))
        .ok_or(DoesNotFit)?;
    field.set(word, signed(offset, UNIT, field.width)?)
}

/// The fields of an extended-register add, in the order of [`add_extended`]'s values.
const ADD_EXTENDED_FIELDS: [FieldName; 5] =
    [field::RD, field::RN, field::RM, field::OPTION, field::IMM3];
static ADD_EXTENDED: LazyLock<Shape<5>> =
    LazyLock::new(|| Shape::written(form::ADD_EXTENDED, ADD_EXTENDED_FIELDS));
static SUB_EXTENDED: LazyLock<Shape<5>> =
    LazyLock::new(|| Shape::written(form::SUB_EXTENDED, ADD_EXTENDED_FIELDS));

/// How an extended-register operand is extended: from `from_bits` bits, signed or not.
fn option(signed: bool, from_bits: u32) -> u32 {
    let [_, _, _, option, _] = ADD_EXTENDED.fields;
    u32::from(signed) << (option.width - 1) | (from_bits / u8::BITS).trailing_zeros()
}

static ADR: LazyLock<Shape<3>> =
    LazyLock::new(|| Shape::written(form::ADR, [field::RD, field::IMMLO, field::IMMHI]));

/// `adr rd, #offset`: the address `offset` bytes from the instruction's own.
pub fn adr(rd: Register, offset: i64) -> Result<u32, DoesNotFit> {
    let [_, immlo, immhi] = ADR.fields;
    let low = immlo.width;
    let value = signed(offset, 1, low + immhi.width)?;
    ADR.word([rd.into(), value & ((1 << low) - 1), value >> low])
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
    let shape = if subtract {
        *SUB_EXTENDED
    } else {
        *ADD_EXTENDED
    };
    let values = [
        rd.into(),
        rn.into(),
        rm.into(),
        option(signed, from_bits),
        shift,
    ];
    shape.word(values)
}

/// The fields of a shifted-register logical operation, in the order of [`logical_register`]'s values.
const LOGICAL_REGISTER_FIELDS: [FieldName; 3] = [field::RD, field::RN, field::RM];
static LOGICAL_REGISTERS: LazyLock<Kind<3>> = LazyLock::new(|| Kind::of(LOGICAL_REGISTER_FIELDS));
static ORR_SHIFTED: LazyLock<Shape<3>> =
    LazyLock::new(|| Shape::written(form::ORR_SHIFTED, LOGICAL_REGISTER_FIELDS));

fn logical_register_of(
    shape: Shape<{ LOGICAL_REGISTER_FIELDS.len() }>,
    rd: Register,
    rn: Register,
    rm: Register,
) -> u32 {
    shape
        .word([rd.into(), rn.into(), rm.into()])
        .expect("registers fit")
}

/// `eor rd, rn, rm` or `orr rd, rn, rm`, by form, unshifted.
pub fn logical_register(form: Form, rd: Register, rn: Register, rm: Register) -> u32 {
    LOGICAL_REGISTERS
        .word(form, [rd.into(), rn.into(), rm.into()])
        .expect("registers fit")
}

/// `mov rd, rn` of general registers (`orr rd, xzr, rn`).
pub fn move_register(rd: Register, rn: Register) -> u32 {
    logical_register_of(*ORR_SHIFTED, rd, ZERO, rn)
}

/// `bfi rd, rn, #lsb, #bits`: the low `bits` bits of `rn` put at `lsb` in `rd`, the rest kept.
pub fn bfi(rd: Register, rn: Register, lsb: u32, bits: u32) -> u32 {
    bitfield(*BFM, rd, rn, (u64::BITS - lsb) % u64::BITS, bits - 1)
}

static LOAD_PAIRS: LazyLock<Kind<4>> =
    LazyLock::new(|| Kind::of([field::RT, field::RT2, field::RN, field::IMM7]));

/// `ldp rt, rt2, [rn]` of 32-bit or 64-bit registers, by form.
pub fn load_pair(form: Form, rt: Register, rt2: Register, rn: Register) -> u32 {
    LOAD_PAIRS
        .word(form, [rt.into(), rt2.into(), rn.into(), 0])
        .expect("registers fit")
}

/// The forms of an atomic on a status register, a data register and a base: loads and adds, and compares and
/// swaps.
static ATOMICS: LazyLock<Kind<3>> = LazyLock::new(|| Kind::of([field::RS, field::RT, field::RN]));

/// `ldadd{b,h} rs, rt, [rn]` and the other sizes, by form; with `rt` the zero register it is `stadd`.
pub fn load_add(form: Form, rs: Register, rt: Register, rn: Register) -> u32 {
    ATOMICS
        .word(form, [rs.into(), rt.into(), rn.into()])
        .expect("registers fit")
}

/// `cas{b,h} rs, rt, [rn]` of any size, or `casp` of two even pairs, by form.
pub fn compare_swap_of(form: Form, rs: Register, rt: Register, rn: Register) -> u32 {
    ATOMICS
        .word(form, [rs.into(), rt.into(), rn.into()])
        .expect("registers fit")
}

/// `eret`: back to the level and address the exception registers hold.
pub fn exception_return() -> u32 {
    static ERET: LazyLock<Shape<0>> = LazyLock::new(|| Shape::written(form::ERET, []));
    ERET.word([]).expect("no fields")
}

/// The system instruction `tlbi vmalle1is`: every TLB entry of the EL1&0 regime, on every CPU of the inner
/// shareable domain (Arm ARM, TLBI VMALLE1IS), with its register the zero register.
const INVALIDATE_WHOLE_TLB: [u32; 4] = [0, 8, 3, 0];
static SYSTEM: LazyLock<Shape<5>> = LazyLock::new(|| {
    Shape::written(
        form::SYS,
        [field::OP1, field::CRN, field::CRM, field::OP2, field::RT],
    )
});

/// `tlbi vmalle1is`.
pub fn invalidate_whole_tlb() -> u32 {
    let [op1, crn, crm, op2] = INVALIDATE_WHOLE_TLB;
    SYSTEM
        .word([op1, crn, crm, op2, ZERO.into()])
        .expect("the system instruction's fields fit")
}

/// The fields of a system register a move names, in the order of [`write_system_register`]'s values.
const SYSTEM_REGISTER_FIELDS: [FieldName; 5] =
    [field::O0, field::OP1, field::CRN, field::CRM, field::OP2];
static MSR: LazyLock<Shape<6>> = LazyLock::new(|| {
    let [o0, op1, crn, crm, op2] = SYSTEM_REGISTER_FIELDS;
    Shape::written(form::MSR, [o0, op1, crn, crm, op2, field::RT])
});

/// `msr <register>, rt`, the system register given by its `o0`, `op1`, `CRn`, `CRm` and `op2`.
pub fn write_system_register(
    register: [u32; SYSTEM_REGISTER_FIELDS.len()],
    rt: Register,
) -> Result<u32, DoesNotFit> {
    let [o0, op1, crn, crm, op2] = register;
    MSR.word([o0, op1, crn, crm, op2, rt.into()])
}

static EXCEPTIONS: LazyLock<Kind<1>> = LazyLock::new(|| Kind::of([field::IMM16]));

/// `brk #imm16`, `udf #imm16` or `hvc #imm16`, by form.
pub fn exception(form: Form, imm16: u16) -> u32 {
    EXCEPTIONS
        .word(form, [imm16.into()])
        .expect("the immediate fits its field")
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

static MRS_RT: LazyLock<Field> = LazyLock::new(|| Field::written(form::MRS, field::RT));
static MSR_RT: LazyLock<Field> = LazyLock::new(|| Field::written(form::MSR, field::RT));

/// The moves with register x0: every system register an `msr` names, decoded by that form alone, once; the two
/// writes the native routing singles out, the stops telling them apart; each read is the `mrs` of its write's
/// register.
fn moves_of_x0() -> FloatStatusMoves {
    use wcontainers_guest_cpu::{Native, Route, Stopping};
    let (mrs, msr) = (form::MRS.arm(), form::MSR.arm());
    let Field {
        low: rt_low,
        width: rt_width,
    } = *MSR_RT;
    let routed = |word| {
        let instruction = crate::decode_forms::decode(word, msr)?;
        (<Arm64 as Native>::route(&(), &instruction, &()) == Route::FloatStatus)
            .then(|| <Arm64 as Native>::stopping(&instruction, 0) == Stopping::FloatStatusWrite)
    };
    let (mut status, mut control) = (Vec::new(), Vec::new());
    for word in (0..=!msr.mask)
        .step_by(1 << (rt_low + rt_width))
        .map(|free| msr.word(free))
    {
        match routed(word) {
            Some(true) => status.push(word),
            Some(false) => control.push(word),
            None => {}
        }
    }
    let (&[write_fpsr], &[write_fpcr]) = (status.as_slice(), control.as_slice()) else {
        panic!("one routed move of each float status register: {status:x?} {control:x?}");
    };
    let read = |write: u32| mrs.word(write & !msr.mask);
    let (read_fpsr, read_fpcr) = (read(write_fpsr), read(write_fpcr));
    assert!(
        msr.presented(write_fpsr) && msr.presented(write_fpcr),
        "the writes are presented"
    );
    assert!(
        mrs.presented(read_fpsr) && mrs.presented(read_fpcr),
        "the reads of the registers the writes name are presented"
    );
    let read_routed = crate::decode_forms::decode(read_fpsr, mrs)
        .is_some_and(|i| <Arm64 as Native>::route(&(), &i, &()) == Route::FloatStatus);
    assert!(read_routed, "the read of the status register is routed");
    FloatStatusMoves {
        read_fpsr,
        read_fpcr,
        write_fpsr,
        write_fpcr,
    }
}

impl FloatStatusMoves {
    pub fn with(rt: Register) -> FloatStatusMoves {
        static OF_X0: OnceLock<FloatStatusMoves> = OnceLock::new();
        let of_x0 = OF_X0.get_or_init(moves_of_x0);
        let into = |field: Field, word: u32| word | (u32::from(rt) << field.low);
        FloatStatusMoves {
            read_fpsr: into(*MRS_RT, of_x0.read_fpsr),
            read_fpcr: into(*MRS_RT, of_x0.read_fpcr),
            write_fpsr: into(*MSR_RT, of_x0.write_fpsr),
            write_fpcr: into(*MSR_RT, of_x0.write_fpcr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DoesNotFit, FieldName, FloatStatusMoves, Form, Register, UNIT, ZERO, arm, field, form,
        logical_immediate, signed,
    };
    use crate::forms;

    /// The seed and the number of cases, decimal.
    const SEED: &str = "WCONTAINERS_ENCODE_SEED";
    const CASES: &str = "WCONTAINERS_ENCODE_CASES";
    const DEFAULT_SEED: u64 = 0x6a09_e667_f3bc_c908;
    const DEFAULT_CASES: u64 = 10_000;

    fn knob(name: &str, default: u64) -> u64 {
        std::env::var(name).map_or(default, |text| {
            text.parse()
                .unwrap_or_else(|_| panic!("{name:?} is not a number: {text}"))
        })
    }

    /// The width of a field, as `forms` reads Arm's table.
    fn width(name: Form, field: FieldName) -> u32 {
        let form = arm(name.name());
        (form.fields.iter())
            .find(|(f, ..)| f == field.name())
            .map(|&(_, _, width)| width)
            .unwrap_or_else(|| panic!("{name:?} has no field {field:?}"))
    }

    /// The field named `name` of a form, as `forms` reads Arm's table: its lowest bit and width.
    fn field_at(form: &forms::Form, name: FieldName) -> Option<(u32, u32)> {
        (form.fields.iter())
            .find(|(f, ..)| f == name.name())
            .map(|&(_, low, width)| (low, width))
    }

    /// `word` with the field at `low` of `width` bits set to `value`, if it fits.
    fn set(word: u32, (low, width): (u32, u32), value: u32) -> Result<u32, DoesNotFit> {
        if u64::from(value) >> width != 0 {
            return Err(DoesNotFit);
        }
        let mask = ((1u64 << width) - 1) as u32;
        Ok(word & !(mask << low) | value << low)
    }

    /// The word of form `name` with `fields` filled in, found by name in `forms`' reading of Arm's table: the
    /// reference the builders are held to.
    fn by_name(name: Form, fields: &[(FieldName, u32)]) -> Result<u32, DoesNotFit> {
        let form = arm(name.name());
        let mut word = form.value & form.mask;
        for &(field, value) in fields {
            let field =
                field_at(form, field).unwrap_or_else(|| panic!("{name:?} has no {field:?}"));
            word = set(word, field, value)?;
        }
        Ok(word)
    }

    /// Every builder's word, or its refusal, against the reference, over drawn values that fit and that do not.
    #[test]
    fn every_builder_places_its_fields_as_the_table_read_by_name_does() {
        let seed = knob(SEED, DEFAULT_SEED);
        let cases = knob(CASES, DEFAULT_CASES);
        let mut rng = fastrand::Rng::with_seed(seed);
        let bits = u64::BITS;
        let (mut replaced, mut retargeted) = (0u64, 0u64);
        for _ in 0..cases {
            // Registers as the builders that refuse take them, past 31 a quarter of the time, and as those that
            // cannot refuse take them.
            let mut register = || -> Register {
                if rng.u8(..4) == 0 {
                    rng.u8(..)
                } else {
                    rng.u8(..=ZERO)
                }
            };
            let wide = [register(), register(), register(), register()];
            let [xd, xn, xm, xt] = wide;
            let [wd, wn, wm, wt] = wide.map(u32::from);
            let [rd, rn, rm, rt] = wide.map(|r| r & ZERO);
            let [d, n, m, t] = [rd, rn, rm, rt].map(u32::from);
            let imm16 = rng.u16(..);
            let half = rng.u32(..4);
            for (name, built) in [
                (form::MOVZ, super::movz(rd, imm16, half)),
                (form::MOVK, super::movk(rd, imm16, half)),
                (form::MOVN, super::move_wide(form::MOVN, rd, imm16, half)),
            ] {
                let fields = [
                    (field::RD, d),
                    (field::IMM16, imm16.into()),
                    (field::HW, half),
                ];
                assert_eq!(Ok(built), by_name(name, &fields), "{name:?}");
            }
            let lsb = rng.u32(..bits);
            let count = rng.u32(1..=bits - lsb);
            let bitfield = |name, immr, imms| {
                by_name(
                    name,
                    &[
                        (field::RD, d),
                        (field::RN, n),
                        (field::IMMR, immr),
                        (field::IMMS, imms),
                    ],
                )
            };
            let rotated = (bits - lsb) % bits;
            assert_eq!(
                Ok(super::ubfx(rd, rn, lsb, count)),
                bitfield(form::UBFM, lsb, lsb + count - 1)
            );
            assert_eq!(
                Ok(super::ubfiz(rd, rn, lsb, count)),
                bitfield(form::UBFM, rotated, count - 1)
            );
            assert_eq!(
                Ok(super::bfxil(rd, rn, lsb, count)),
                bitfield(form::BFM, lsb, lsb + count - 1)
            );
            assert_eq!(
                Ok(super::bfi(rd, rn, lsb, count)),
                bitfield(form::BFM, rotated, count - 1)
            );
            let value = if rng.bool() {
                rng.u64(..)
            } else {
                let run = rng.u32(1..bits);
                (u64::MAX >> (bits - run)).rotate_right(rng.u32(..bits))
            };
            for (name, built) in [
                (form::AND_IMMEDIATE, super::and_immediate(xd, xn, value)),
                (form::EOR_IMMEDIATE, super::eor_immediate(xd, xn, value)),
            ] {
                let reference =
                    logical_immediate(value)
                        .ok_or(DoesNotFit)
                        .and_then(|(nf, r, s)| {
                            let fields = [
                                (field::RD, wd),
                                (field::RN, wn),
                                (field::N, nf),
                                (field::IMMR, r),
                                (field::IMMS, s),
                            ];
                            by_name(name, &fields)
                        });
                assert_eq!(built, reference, "{name:?} {value:#x}");
            }
            let immediate = rng.i64(-(1 << 14)..1 << 14);
            let (name, magnitude) = if immediate < 0 {
                (form::SUB_IMMEDIATE, immediate.unsigned_abs())
            } else {
                (form::ADD_IMMEDIATE, immediate.unsigned_abs())
            };
            let fields = [
                (field::RD, wd),
                (field::RN, wn),
                (field::IMM12, magnitude as u32),
            ];
            assert_eq!(
                super::add_immediate(xd, xn, immediate),
                by_name(name, &fields),
                "{immediate}"
            );
            let bit = rng.u32(..bits + 8);
            // An offset within twice the reach of the field `field` of `name` in units of `unit`, half the time
            // made a multiple of the unit.
            let mut draw = |name: Form, field: FieldName, unit: i64| {
                let reach = unit << (width(name, field) - 1);
                rng.i64(-2 * reach..2 * reach) & !rng.choice([0, unit - 1]).expect("a choice")
            };
            for (nonzero, name) in [(false, form::CBZ), (true, form::CBNZ)] {
                let [offset, moved] = [(); 2].map(|()| draw(name, field::IMM19, UNIT));
                let reference = |offset| {
                    signed(offset, UNIT, width(name, field::IMM19))
                        .and_then(|imm| by_name(name, &[(field::RT, wt), (field::IMM19, imm)]))
                };
                let built = super::compare_branch(nonzero, xt, offset);
                assert_eq!(built, reference(offset), "{name:?} {offset}");
                if let Ok(word) = built {
                    let retargeted = super::retarget(word, moved);
                    assert_eq!(retargeted, reference(moved), "{name:?} {moved}");
                }
            }
            let b40 = width(form::TBNZ, field::B40);
            let [branch, moved] = [(); 2].map(|()| draw(form::TBNZ, field::IMM14, UNIT));
            let reference = |offset| {
                signed(offset, UNIT, width(form::TBNZ, field::IMM14)).and_then(|imm| {
                    let fields = [
                        (field::RT, wt),
                        (field::B5, bit >> b40),
                        (field::B40, bit & ((1 << b40) - 1)),
                        (field::IMM14, imm),
                    ];
                    by_name(form::TBNZ, &fields)
                })
            };
            let built = super::test_branch_nonzero(xt, bit, branch);
            assert_eq!(built, reference(branch), "{bit} {branch}");
            if let Ok(word) = built {
                assert_eq!(super::retarget(word, moved), reference(moved), "{moved}");
            }
            for (link, name) in [(false, form::B), (true, form::BL)] {
                let [offset, moved] = [(); 2].map(|()| draw(name, field::IMM26, UNIT));
                let reference = |offset| {
                    signed(offset, UNIT, width(name, field::IMM26))
                        .and_then(|imm| by_name(name, &[(field::IMM26, imm)]))
                };
                let built = super::branch(link, offset);
                assert_eq!(built, reference(offset), "{name:?} {offset}");
                if let Ok(word) = built {
                    assert_eq!(super::retarget(word, moved), reference(moved), "{moved}");
                }
            }
            let low = width(form::ADR, field::IMMLO);
            let adr_bits = low + width(form::ADR, field::IMMHI);
            let near = rng.i64(-(1 << adr_bits)..1 << adr_bits);
            let reference = signed(near, 1, adr_bits).and_then(|v| {
                let fields = [
                    (field::RD, wd),
                    (field::IMMLO, v & ((1 << low) - 1)),
                    (field::IMMHI, v >> low),
                ];
                by_name(form::ADR, &fields)
            });
            assert_eq!(super::adr(xd, near), reference, "{near}");
            for name in [form::BR, form::BLR, form::RET] {
                let reference = by_name(name, &[(field::RN, n)]);
                assert_eq!(Ok(super::branch_register(name, rn)), reference);
            }
            let unit = 1 << rng.u32(..5);
            let place = rng.u64(..1 << 16);
            for name in [
                form::LDR,
                form::STR,
                form::LDRB,
                form::LDR_32,
                form::LDRH,
                form::LDRSW,
                form::LDR_S,
                form::LDR_D,
                form::LDR_Q,
                form::STR_Q,
            ] {
                let reference = if place.is_multiple_of(unit) {
                    let scaled = u32::try_from(place / unit).expect("small");
                    by_name(
                        name,
                        &[(field::RT, wt), (field::RN, wn), (field::IMM12, scaled)],
                    )
                } else {
                    Err(DoesNotFit)
                };
                let built = super::load_store(name, xt, xn, place, unit);
                assert_eq!(built, reference, "{name:?} {place} {unit}");
            }
            let (signed_extend, from) = (rng.bool(), 8 << rng.u32(..4));
            let shift = rng.u32(..8);
            let option_sign = width(form::ADD_EXTENDED, field::OPTION) - 1;
            let option =
                u32::from(signed_extend) << option_sign | (from / u8::BITS).trailing_zeros();
            for (subtract, name) in [(false, form::ADD_EXTENDED), (true, form::SUB_EXTENDED)] {
                let fields = [
                    (field::RD, wd),
                    (field::RN, wn),
                    (field::RM, wm),
                    (field::OPTION, option),
                    (field::IMM3, shift),
                ];
                let built = super::add_extended(subtract, xd, xn, xm, signed_extend, from, shift);
                assert_eq!(built, by_name(name, &fields), "{name:?} {from} {shift}");
            }
            let registers = [(field::RD, d), (field::RN, n), (field::RM, m)];
            for name in [form::EOR_SHIFTED, form::ORR_SHIFTED] {
                let built = super::logical_register(name, rd, rn, rm);
                assert_eq!(Ok(built), by_name(name, &registers), "{name:?}");
            }
            let moved = [(field::RD, d), (field::RN, u32::from(ZERO)), (field::RM, n)];
            assert_eq!(
                Ok(super::move_register(rd, rn)),
                by_name(form::ORR_SHIFTED, &moved)
            );
            for name in [form::LDP_32, form::LDP_64] {
                let fields = [
                    (field::RT, t),
                    (field::RT2, d),
                    (field::RN, n),
                    (field::IMM7, 0),
                ];
                let built = super::load_pair(name, rt, rd, rn);
                assert_eq!(Ok(built), by_name(name, &fields), "{name:?}");
            }
            let atomic = [(field::RS, d), (field::RT, t), (field::RN, n)];
            for name in [
                form::LDADDB,
                form::LDADDH,
                form::LDADD_32,
                form::LDADD_64,
                form::CAS,
                form::CASB,
                form::CASH,
                form::CAS_32,
                form::CASP,
            ] {
                assert_eq!(
                    Ok(super::load_add(name, rd, rt, rn)),
                    by_name(name, &atomic)
                );
                let built = super::compare_swap_of(name, rd, rt, rn);
                assert_eq!(Ok(built), by_name(name, &atomic), "{name:?}");
            }
            for name in [form::BRK, form::UDF, form::HVC] {
                let reference = by_name(name, &[(field::IMM16, imm16.into())]);
                assert_eq!(Ok(super::exception(name, imm16)), reference, "{name:?}");
            }
            let register = [2, 8, 16, 16, 8].map(|values| rng.u32(..values));
            let moving = rng.u8(..2 * (ZERO + 1));
            let names = [field::O0, field::OP1, field::CRN, field::CRM, field::OP2];
            let mut system: Vec<_> = names.into_iter().zip(register).collect();
            system.push((field::RT, moving.into()));
            assert_eq!(
                super::write_system_register(register, moving),
                by_name(form::MSR, &system)
            );
            let moves = FloatStatusMoves::with(rt);
            let of_x0 = FloatStatusMoves::with(0);
            let into = |name, word: u32| {
                let form = arm(Form::name(name));
                let &(_, low, _) = (form.fields.iter())
                    .find(|(f, ..)| f == field::RT.name())
                    .expect("a register field");
                word | (t << low)
            };
            assert_eq!(moves.read_fpsr, into(form::MRS, of_x0.read_fpsr));
            assert_eq!(moves.read_fpcr, into(form::MRS, of_x0.read_fpcr));
            assert_eq!(moves.write_fpsr, into(form::MSR, of_x0.write_fpsr));
            assert_eq!(moves.write_fpcr, into(form::MSR, of_x0.write_fpcr));
            // A word of any form of Arm's table, one of its fields or a field the encoder names replaced, and
            // the word retargeted, against the field found by name in the word's form.
            // Most forms are of features the CPU we present has not, so a few draws find a presented word.
            let (drawn, word) = (0..16)
                .map(|_| {
                    let drawn = rng.choice(forms::all()).expect("forms");
                    (drawn, drawn.word(rng.u32(..)))
                })
                .find(|&(_, word)| forms::form(word).is_some())
                .unwrap_or_else(|| {
                    let drawn = rng.choice(forms::all()).expect("forms");
                    (drawn, drawn.word(rng.u32(..)))
                });
            let named = forms::form(word);
            let own: Vec<FieldName> = super::field_names(drawn).collect();
            let names = own.iter().map(|name| name.name());
            assert!(
                names.eq(drawn.fields.iter().map(|(name, ..)| name)),
                "{}",
                drawn.name
            );
            let name = match rng.choice(own) {
                Some(name) if rng.bool() => name,
                _ => {
                    let ours = u16::try_from(field::NAMES.len()).expect("few");
                    FieldName(rng.u16(..ours))
                }
            };
            let value = rng.u32(..) >> rng.u32(20..u32::BITS);
            let reference = named
                .and_then(|found| field_at(found, name))
                .ok_or(DoesNotFit)
                .and_then(|field| set(word, field, value));
            assert_eq!(
                super::replace(word, name, value),
                reference,
                "{word:#x} {name:?}"
            );
            replaced += u64::from(reference.is_ok());
            let offset = rng.i64(..) >> rng.u32(34..u64::BITS)
                & !rng.choice([0, UNIT - 1]).expect("a choice");
            let reference = named
                .and_then(|found| {
                    let offsets = [field::IMM26, field::IMM19, field::IMM14].map(FieldName::name);
                    (found.fields.iter()).find(|(f, ..)| offsets.contains(&f.as_str()))
                })
                .ok_or(DoesNotFit)
                .and_then(|&(_, low, width)| set(word, (low, width), signed(offset, UNIT, width)?));
            assert_eq!(
                super::retarget(word, offset),
                reference,
                "{word:#x} {offset}"
            );
            retargeted += u64::from(reference.is_ok());
        }
        assert_eq!(Ok(super::exception_return()), by_name(form::ERET, &[]));
        let tlbi = [
            (field::OP1, 0),
            (field::CRN, 8),
            (field::CRM, 3),
            (field::OP2, 0),
            (field::RT, ZERO.into()),
        ];
        assert_eq!(Ok(super::invalidate_whole_tlb()), by_name(form::SYS, &tlbi));
        let reach = (UNIT as u64) << (width(form::B, field::IMM26) - 1);
        assert_eq!(super::branch_reach(), reach);
        let page_bits = width(form::ADRP, field::IMMHI) + width(form::ADRP, field::IMMLO) - 1;
        for page in [1 << 12, 1 << 14, 1 << 16] {
            assert_eq!(super::page_reach(page), page << page_bits);
        }
        println!(
            "seed {seed}, {cases} cases, {replaced} drawn words replaced, {retargeted} retargeted"
        );
    }

    /// A builder given a form of another kind places the values it can in order, so a value that does not fit
    /// before the missing field is refused, and the missing field is a fault of the caller.
    #[test]
    fn a_form_of_another_kind_is_refused_or_missing_in_field_order() {
        assert_eq!(
            super::load_store(form::CBZ, ZERO + 1, 1, 0, 1),
            Err(DoesNotFit)
        );
        let missing = std::panic::catch_unwind(|| super::load_store(form::CBZ, 1, 1, 0, 1));
        assert!(missing.is_err(), "a missing field is a fault");
    }
}
