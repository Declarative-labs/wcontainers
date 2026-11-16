//! Decoding AdvSIMD data processing, loads and stores of structures, by Arm's forms. The reserved element
//! sizes of each form are Arm's decode rules, which the forms list does not carry.

use crate::forms::Form;
use crate::instruction::*;

fn field(form: &Form, word: u32, name: &str) -> u32 {
    form.field(word, name)
        .unwrap_or_else(|| panic!("no field {name} in {}", form.name))
}

/// The SIMD operation a stem names in a group, by Arm's mnemonic.
fn op_of(group: &str, stem: &str) -> Option<SimdOp> {
    use SimdOp as S;
    let same = matches!(group, "asimdsame" | "asisdsame");
    let misc = matches!(group, "asimdmisc" | "asisdmisc");
    let shift = matches!(group, "asimdshf" | "asisdshf");
    let across = matches!(group, "asimdall" | "asisdpair");
    let signed = stem.starts_with('S');
    Some(match stem {
        "AND" => S::And,
        "BIC" => S::AndNot,
        "ORR" => S::Or,
        "ORN" => S::OrNot,
        "EOR" => S::Xor,
        "BSL" => S::Select,
        "BIT" => S::InsertIfTrue,
        "BIF" => S::InsertIfFalse,
        "ADD" => S::Add,
        "SUB" => S::Sub,
        "MUL" => S::Mul,
        "MLA" => S::MulAdd,
        "MLS" => S::MulSub,
        "PMUL" => S::PolyMul,
        "CMEQ" if same => S::Equal,
        "CMGT" | "CMHI" if same => S::Greater {
            signed: stem == "CMGT",
        },
        "CMGE" | "CMHS" if same => S::GreaterOrEqual {
            signed: stem == "CMGE",
        },
        "CMTST" => S::Test,
        "SHADD" | "UHADD" => S::HalvingAdd {
            signed,
            rounding: false,
        },
        "SRHADD" | "URHADD" => S::HalvingAdd {
            signed,
            rounding: true,
        },
        "SHSUB" | "UHSUB" => S::HalvingSub { signed },
        "SQADD" | "UQADD" => S::SaturatingAdd { signed },
        "SQSUB" | "UQSUB" => S::SaturatingSub { signed },
        "SSHL" | "USHL" => S::ShiftByRegister {
            signed,
            rounding: false,
            saturating: false,
        },
        "SRSHL" | "URSHL" => S::ShiftByRegister {
            signed,
            rounding: true,
            saturating: false,
        },
        "SQSHL" | "UQSHL" if same => S::ShiftByRegister {
            signed,
            rounding: false,
            saturating: true,
        },
        "SQRSHL" | "UQRSHL" => S::ShiftByRegister {
            signed,
            rounding: true,
            saturating: true,
        },
        "SMAX" | "UMAX" => S::Max { signed },
        "SMIN" | "UMIN" => S::Min { signed },
        "SABD" | "UABD" => S::AbsDiff { signed },
        "SABA" | "UABA" => S::AbsDiffAccumulate { signed },
        "ADDP" if same => S::AddPairwise,
        "SMAXP" | "UMAXP" => S::MaxPairwise { signed },
        "SMINP" | "UMINP" => S::MinPairwise { signed },
        "SQDMULH" => S::DoublingMulHigh { rounding: false },
        "SQRDMULH" => S::DoublingMulHigh { rounding: true },
        "REV16" => S::Reverse { group: 2 },
        "REV32" => S::Reverse { group: 4 },
        "REV64" => S::Reverse { group: 8 },
        "SADDLP" | "UADDLP" => S::AddLongPairwise {
            signed,
            accumulate: false,
        },
        "SADALP" | "UADALP" => S::AddLongPairwise {
            signed,
            accumulate: true,
        },
        "SUQADD" | "USQADD" => S::SaturatingAccumulate { signed },
        "CLS" => S::CountLeadingSign,
        "CLZ" => S::CountLeadingZeros,
        "CNT" => S::CountBits,
        "NOT" => S::Not,
        "RBIT" => S::ReverseBits,
        "SQABS" => S::SaturatingAbs,
        "SQNEG" => S::SaturatingNegate,
        "ABS" => S::Abs,
        "NEG" => S::Negate,
        "CMGT" if misc => S::CompareZero(Cond(12)),
        "CMGE" if misc => S::CompareZero(Cond(10)),
        "CMEQ" if misc => S::CompareZero(Cond(0)),
        "CMLE" => S::CompareZero(Cond(13)),
        "CMLT" => S::CompareZero(Cond(11)),
        "XTN" => S::Narrow,
        "SQXTN" => S::SaturatingNarrow {
            from_signed: true,
            to_signed: true,
        },
        "UQXTN" => S::SaturatingNarrow {
            from_signed: false,
            to_signed: false,
        },
        "SQXTUN" => S::SaturatingNarrow {
            from_signed: true,
            to_signed: false,
        },
        "SHLL" => S::ShiftLeftLongByWidth,
        "URECPE" => S::ReciprocalEstimateInteger,
        "URSQRTE" => S::ReciprocalSqrtEstimateInteger,
        "SSHR" | "USHR" => S::ShiftRight {
            signed,
            rounding: false,
            accumulate: false,
        },
        "SSRA" | "USRA" => S::ShiftRight {
            signed,
            rounding: false,
            accumulate: true,
        },
        "SRSHR" | "URSHR" => S::ShiftRight {
            signed,
            rounding: true,
            accumulate: false,
        },
        "SRSRA" | "URSRA" => S::ShiftRight {
            signed,
            rounding: true,
            accumulate: true,
        },
        "SHL" => S::ShiftLeft,
        "SQSHL" | "UQSHL" if shift => S::SaturatingShiftLeft {
            from_signed: signed,
            to_signed: signed,
        },
        "SQSHLU" => S::SaturatingShiftLeft {
            from_signed: true,
            to_signed: false,
        },
        "SHRN" => S::ShiftRightNarrow { rounding: false },
        "RSHRN" => S::ShiftRightNarrow { rounding: true },
        "SQSHRN" | "UQSHRN" | "SQRSHRN" | "UQRSHRN" | "SQSHRUN" | "SQRSHRUN" => {
            S::SaturatingShiftRightNarrow {
                from_signed: signed,
                to_signed: signed && !stem.ends_with("UN"),
                rounding: stem.contains("QR"),
            }
        }
        "SSHLL" | "USHLL" => S::ShiftLeftLong { signed },
        "SRI" => S::ShiftRightInsert,
        "SLI" => S::ShiftLeftInsert,
        "SADDL" | "UADDL" => S::AddLong { signed },
        "SSUBL" | "USUBL" => S::SubLong { signed },
        "SADDW" | "UADDW" => S::AddWide { signed },
        "SSUBW" | "USUBW" => S::SubWide { signed },
        "ADDHN" => S::AddNarrowHigh { rounding: false },
        "RADDHN" => S::AddNarrowHigh { rounding: true },
        "SUBHN" => S::SubNarrowHigh { rounding: false },
        "RSUBHN" => S::SubNarrowHigh { rounding: true },
        "SABDL" | "UABDL" => S::AbsDiffLong { signed },
        "SABAL" | "UABAL" => S::AbsDiffAccumulateLong { signed },
        "SMULL" | "UMULL" => S::MulLong { signed },
        "SMLAL" | "UMLAL" => S::MulAddLong { signed },
        "SMLSL" | "UMLSL" => S::MulSubLong { signed },
        "SQDMULL" => S::DoublingMulLong,
        "SQDMLAL" => S::DoublingMulAddLong,
        "SQDMLSL" => S::DoublingMulSubLong,
        "PMULL" => S::PolyMulLong,
        "FADD" => S::FloatAdd,
        "FSUB" => S::FloatSub,
        "FMUL" => S::FloatMul,
        "FDIV" => S::FloatDiv,
        "FMAX" => S::FloatMax,
        "FMIN" => S::FloatMin,
        "FMAXNM" => S::FloatMaxNumber,
        "FMINNM" => S::FloatMinNumber,
        "FMULX" => S::FloatMulExtended,
        "FMLA" => S::FloatMulAdd,
        "FMLS" => S::FloatMulSub,
        "FABD" => S::FloatAbsDiff,
        "FCMEQ" if same => S::FloatEqual,
        "FCMGE" if same => S::FloatGreaterOrEqual,
        "FCMGT" if same => S::FloatGreater,
        "FACGE" => S::FloatAbsGreaterOrEqual,
        "FACGT" => S::FloatAbsGreater,
        "FRECPS" => S::FloatReciprocalStep,
        "FRSQRTS" => S::FloatReciprocalSqrtStep,
        "FADDP" if same => S::FloatAddPairwise,
        "FMAXP" if same => S::FloatMaxPairwise,
        "FMINP" if same => S::FloatMinPairwise,
        "FMAXNMP" if same => S::FloatMaxNumberPairwise,
        "FMINNMP" if same => S::FloatMinNumberPairwise,
        "FABS" => S::FloatAbs,
        "FNEG" => S::FloatNegate,
        "FSQRT" => S::FloatSqrt,
        "FRINTN" => S::FloatRound(Rounding::Nearest),
        "FRINTM" => S::FloatRound(Rounding::Floor),
        "FRINTP" => S::FloatRound(Rounding::Ceiling),
        "FRINTZ" => S::FloatRound(Rounding::Zero),
        "FRINTA" => S::FloatRound(Rounding::Away),
        "FRINTX" => S::FloatRound(Rounding::CurrentExact),
        "FRINTI" => S::FloatRound(Rounding::Current),
        "FCMGT" if misc => S::FloatCompareZero(Cond(12)),
        "FCMGE" if misc => S::FloatCompareZero(Cond(10)),
        "FCMEQ" if misc => S::FloatCompareZero(Cond(0)),
        "FCMLE" => S::FloatCompareZero(Cond(13)),
        "FCMLT" => S::FloatCompareZero(Cond(11)),
        "FCVTNS" | "FCVTNU" | "FCVTMS" | "FCVTMU" | "FCVTAS" | "FCVTAU" | "FCVTPS" | "FCVTPU"
        | "FCVTZS" | "FCVTZU" => S::FloatToInteger {
            signed: stem.ends_with('S'),
            rounding: match &stem[..5] {
                "FCVTN" => Rounding::Nearest,
                "FCVTM" => Rounding::Floor,
                "FCVTA" => Rounding::Away,
                "FCVTP" => Rounding::Ceiling,
                _ => Rounding::Zero,
            },
        },
        "SCVTF" | "UCVTF" => S::IntegerToFloat { signed },
        "FCVTN" => S::FloatNarrow { odd: false },
        "FCVTXN" => S::FloatNarrow { odd: true },
        "FCVTL" => S::FloatWiden,
        "FRECPE" => S::FloatReciprocalEstimate,
        "FRSQRTE" => S::FloatReciprocalSqrtEstimate,
        "FRECPX" => S::FloatReciprocalExponent,
        "ADDV" => S::AddAcross,
        "ADDP" if across => S::AddAcross,
        "SADDLV" | "UADDLV" => S::AddLongAcross { signed },
        "SMAXV" | "UMAXV" => S::MaxAcross { signed },
        "SMINV" | "UMINV" => S::MinAcross { signed },
        "FMAXV" | "FMAXP" => S::FloatMaxAcross,
        "FMINV" | "FMINP" => S::FloatMinAcross,
        "FMAXNMV" | "FMAXNMP" => S::FloatMaxNumberAcross,
        "FMINNMV" | "FMINNMP" => S::FloatMinNumberAcross,
        "FADDP" => S::FloatAddAcross,
        _ => return None,
    })
}

impl SimdOp {
    /// Whether it works on floating point elements.
    pub fn is_float(self) -> bool {
        use SimdOp as S;
        matches!(
            self,
            S::FloatAdd
                | S::FloatSub
                | S::FloatMul
                | S::FloatDiv
                | S::FloatMax
                | S::FloatMin
                | S::FloatMaxNumber
                | S::FloatMinNumber
                | S::FloatMulExtended
                | S::FloatMulAdd
                | S::FloatMulSub
                | S::FloatAbsDiff
                | S::FloatEqual
                | S::FloatGreaterOrEqual
                | S::FloatGreater
                | S::FloatAbsGreaterOrEqual
                | S::FloatAbsGreater
                | S::FloatReciprocalStep
                | S::FloatReciprocalSqrtStep
                | S::FloatAddPairwise
                | S::FloatMaxPairwise
                | S::FloatMinPairwise
                | S::FloatMaxNumberPairwise
                | S::FloatMinNumberPairwise
                | S::FloatAbs
                | S::FloatNegate
                | S::FloatSqrt
                | S::FloatRound(_)
                | S::FloatCompareZero(_)
                | S::FloatToInteger { .. }
                | S::IntegerToFloat { .. }
                | S::FloatNarrow { .. }
                | S::FloatWiden
                | S::FloatReciprocalEstimate
                | S::FloatReciprocalSqrtEstimate
                | S::FloatReciprocalExponent
                | S::FloatMaxAcross
                | S::FloatMinAcross
                | S::FloatMaxNumberAcross
                | S::FloatMinNumberAcross
                | S::FloatAddAcross
        )
    }

    /// The result's elements are twice the width of `element` (long and wide operations).
    pub fn is_long(self) -> bool {
        use SimdOp as S;
        matches!(
            self,
            S::AddLongPairwise { .. }
                | S::ShiftLeftLongByWidth
                | S::ShiftLeftLong { .. }
                | S::AddLong { .. }
                | S::SubLong { .. }
                | S::AddWide { .. }
                | S::SubWide { .. }
                | S::AbsDiffLong { .. }
                | S::AbsDiffAccumulateLong { .. }
                | S::MulLong { .. }
                | S::MulAddLong { .. }
                | S::MulSubLong { .. }
                | S::DoublingMulLong
                | S::DoublingMulAddLong
                | S::DoublingMulSubLong
                | S::PolyMulLong
                | S::FloatWiden
        )
    }

    /// The sources' elements are twice the width of `element`, the result's are `element`.
    pub fn is_narrow(self) -> bool {
        use SimdOp as S;
        matches!(
            self,
            S::Narrow
                | S::SaturatingNarrow { .. }
                | S::ShiftRightNarrow { .. }
                | S::SaturatingShiftRightNarrow { .. }
                | S::AddNarrowHigh { .. }
                | S::SubNarrowHigh { .. }
                | S::FloatNarrow { .. }
        )
    }

    pub fn is_across(self) -> bool {
        use SimdOp as S;
        matches!(
            self,
            S::AddAcross
                | S::AddLongAcross { .. }
                | S::MaxAcross { .. }
                | S::MinAcross { .. }
                | S::FloatMaxAcross
                | S::FloatMinAcross
                | S::FloatMaxNumberAcross
                | S::FloatMinNumberAcross
                | S::FloatAddAcross
        )
    }
}

/// Whether Arm's decode allows this element size for the operation; reserved sizes are undefined.
fn allowed(op: SimdOp, group: &str, element: u8, q: bool, scalar: bool) -> bool {
    use SimdOp as S;
    let float = op.is_float();
    if group == "asimdall" {
        return if float {
            element == 4 && q
        } else {
            element <= 2 || (element == 4 && q)
        };
    }
    if group == "asisdpair" {
        return float || element == 8;
    }
    if float {
        return match op {
            S::FloatNarrow { odd: true } => element == 4,
            S::FloatNarrow { .. } | S::FloatWiden => true,
            _ => scalar || element == 4 || q,
        };
    }
    if op.is_long() || op.is_narrow() {
        return match op {
            S::PolyMulLong => element == 1,
            S::DoublingMulLong | S::DoublingMulAddLong | S::DoublingMulSubLong => {
                element == 2 || element == 4
            }
            _ => element <= 4,
        };
    }
    let any = scalar || element < 8 || q;
    match op {
        S::And
        | S::AndNot
        | S::Or
        | S::OrNot
        | S::Xor
        | S::Select
        | S::InsertIfTrue
        | S::InsertIfFalse
        | S::CountBits
        | S::Not
        | S::ReverseBits
        | S::PolyMul => element == 1,
        S::Mul
        | S::MulAdd
        | S::MulSub
        | S::HalvingAdd { .. }
        | S::HalvingSub { .. }
        | S::Max { .. }
        | S::Min { .. }
        | S::AbsDiff { .. }
        | S::AbsDiffAccumulate { .. }
        | S::MaxPairwise { .. }
        | S::MinPairwise { .. }
        | S::CountLeadingSign
        | S::CountLeadingZeros => element < 8,
        S::DoublingMulHigh { .. } => element == 2 || element == 4,
        S::Reverse { group } => element < group,
        S::ReciprocalEstimateInteger | S::ReciprocalSqrtEstimateInteger => element == 4,
        // Scalar forms of these only exist for 64 bit elements.
        S::Add
        | S::Sub
        | S::Equal
        | S::Greater { .. }
        | S::GreaterOrEqual { .. }
        | S::Test
        | S::Abs
        | S::Negate
        | S::CompareZero(_)
        | S::ShiftRight { .. }
        | S::ShiftLeft
        | S::ShiftRightInsert
        | S::ShiftLeftInsert
            if scalar =>
        {
            element == 8
        }
        S::ShiftByRegister {
            saturating: false, ..
        } if scalar => element == 8,
        _ => any,
    }
}

/// Decode an AdvSIMD form, or `None` to leave it to the first path.
pub(crate) fn decode(word: u32, form: &Form, stem: &str, group: &str) -> Option<Instruction> {
    let get = |name: &str| field(form, word, name);
    let has = |name: &str| form.field(word, name).is_some();
    match group {
        "asimdperm" => return Some(permute(word, form, stem)),
        "asimdtbl" => {
            let q = get("Q") == 1;
            return Some(Instruction::Table {
                extend: get("op") == 1,
                registers: get("len") as u8 + 1,
                whole: q,
                rd: get("Rd") as u8,
                rn: get("Rn") as u8,
                rm: get("Rm") as u8,
            });
        }
        "asisdone" => {
            let imm5 = get("imm5");
            let element = imm5.trailing_zeros();
            if element > 3 {
                return Some(Instruction::Undefined);
            }
            return Some(Instruction::ScalarElement {
                bytes: 1 << element,
                index: (imm5 >> (element + 1)) as u8,
                rd: get("Rd") as u8,
                rn: get("Rn") as u8,
            });
        }
        "asisdlse" | "asisdlsep" | "asisdlso" | "asisdlsop" => {
            return Some(memory(word, form, stem, group));
        }
        _ => {}
    }
    let scalar = group.starts_with("asisd");
    let simd = matches!(
        group,
        "asimdsame"
            | "asisdsame"
            | "asimdmisc"
            | "asisdmisc"
            | "asimdshf"
            | "asisdshf"
            | "asimddiff"
            | "asisddiff"
            | "asimdelem"
            | "asisdelem"
            | "asimdall"
            | "asisdpair"
    );
    if !simd {
        return None;
    }
    let op = op_of(group, stem)?;
    let q = !scalar && get("Q") == 1;
    let float = op.is_float();
    // Floating point forms name their precision `sz`, or the low bit of `size`.
    let sz = if has("sz") {
        get("sz")
    } else if has("size") {
        get("size") & 1
    } else {
        0
    };
    let mut shift = 0u8;
    let element: u8 = if matches!(group, "asimdshf" | "asisdshf") {
        let immh = get("immh");
        let size = 31 - immh.leading_zeros();
        let element = 1u8 << size;
        let immhb = (immh << 3) | get("immb");
        let bits = 8 * u32::from(element);
        let left = matches!(
            op,
            SimdOp::ShiftLeft
                | SimdOp::ShiftLeftInsert
                | SimdOp::SaturatingShiftLeft { .. }
                | SimdOp::ShiftLeftLong { .. }
        );
        shift = if left { immhb - bits } else { 2 * bits - immhb } as u8;
        if float && element < 4 {
            return Some(Instruction::Undefined);
        }
        // Narrowing shifts name the narrow result's size; their source is twice as wide. Scalar shifts
        // other than the saturating left ones exist only for 64 bit elements.
        let reserved = if op.is_narrow() || op.is_long() {
            element == 8
        } else if scalar {
            element != 8 && !matches!(op, SimdOp::SaturatingShiftLeft { .. }) && !float
        } else {
            element == 8 && !q
        };
        if reserved {
            return Some(Instruction::Undefined);
        }
        element
    } else if matches!(
        op,
        SimdOp::And
            | SimdOp::AndNot
            | SimdOp::Or
            | SimdOp::OrNot
            | SimdOp::Xor
            | SimdOp::Select
            | SimdOp::InsertIfTrue
            | SimdOp::InsertIfFalse
            | SimdOp::Not
            | SimdOp::ReverseBits
    ) {
        1
    } else if matches!(op, SimdOp::FloatNarrow { .. } | SimdOp::FloatWiden) {
        // The narrow side: half (sz 0) or single (sz 1).
        2 << sz
    } else if float || !has("size") {
        4 << sz
    } else {
        1 << get("size")
    };
    let (mut rm, mut index) = (if has("Rm") { get("Rm") as u8 } else { 0 }, None);
    if matches!(group, "asimdelem" | "asisdelem") {
        let (h, l, m) = (get("H"), get("L"), get("M"));
        let (i, r) = match (float, element) {
            (_, 2) => ((h << 2) | (l << 1) | m, get("Rm")),
            (_, 4) => ((h << 1) | l, (m << 4) | get("Rm")),
            (true, 8) if l == 0 => (h, (m << 4) | get("Rm")),
            _ => return Some(Instruction::Undefined),
        };
        if element == 1 || (!float && element == 8) || (float && element == 8 && !scalar && !q) {
            return Some(Instruction::Undefined);
        }
        rm = r as u8;
        index = Some(i as u8);
    }
    if !allowed(op, group, element, q, scalar) {
        return Some(Instruction::Undefined);
    }
    Some(Instruction::Simd {
        op,
        shape: Shape { element, q, scalar },
        rd: get("Rd") as u8,
        rn: get("Rn") as u8,
        rm,
        index,
        shift,
    })
}

fn permute(word: u32, form: &Form, stem: &str) -> Instruction {
    let get = |name: &str| field(form, word, name);
    let (element, whole) = (1u8 << get("size"), get("Q") == 1);
    if element == 8 && !whole {
        return Instruction::Undefined;
    }
    Instruction::Permute {
        op: match stem {
            "ZIP1" => Permute::Zip1,
            "ZIP2" => Permute::Zip2,
            "UZP1" => Permute::Unzip1,
            "UZP2" => Permute::Unzip2,
            "TRN1" => Permute::Transpose1,
            _ => Permute::Transpose2,
        },
        element,
        whole,
        rd: get("Rd") as u8,
        rn: get("Rn") as u8,
        rm: get("Rm") as u8,
    }
}

/// ld1 to ld4 and st1 to st4.
fn memory(word: u32, form: &Form, stem: &str, group: &str) -> Instruction {
    let get = |name: &str| field(form, word, name);
    let load = get("L") == 1;
    let whole = get("Q") == 1;
    let rn = match get("Rn") {
        31 => Reg::Sp,
        n => Reg::X(n as u8),
    };
    let post = group.ends_with('p').then(|| match get("Rm") {
        31 => Reg::Zero,
        n => Reg::X(n as u8),
    });
    let rt = get("Rt") as u8;
    let size = get("size");
    let structure = stem.as_bytes()[2] - b'0';
    if matches!(group, "asisdlse" | "asisdlsep") {
        let registers = match get("opcode") {
            0b0000 => 4,
            0b0010 => 4,
            0b0100 => 3,
            0b0110 => 3,
            0b0111 => 1,
            0b1000 => 2,
            _ => 2,
        };
        if size == 3 && !whole && structure > 1 {
            return Instruction::Undefined;
        }
        return Instruction::SimdMemory {
            load,
            structure,
            registers,
            element: 1 << size,
            whole,
            lane: None,
            replicate: false,
            rt,
            rn,
            post,
        };
    }
    let opcode = get("opcode");
    let s = get("S");
    let q = u32::from(whole);
    let (element, lane) = match opcode >> 1 {
        0 => (1, Some((q << 3) | (s << 2) | size)),
        1 if size & 1 == 0 => (2, Some((q << 2) | (s << 1) | (size >> 1))),
        2 if size == 0 => (4, Some((q << 1) | s)),
        2 if size == 1 && s == 0 => (8, Some(q)),
        3 if load && s == 0 => (1 << size, None),
        _ => return Instruction::Undefined,
    };
    Instruction::SimdMemory {
        load,
        structure,
        registers: structure,
        element,
        whole,
        lane: lane.map(|l| l as u8),
        replicate: lane.is_none(),
        rt,
        rn,
        post,
    }
}
