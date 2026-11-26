//! The bugs planted in the models of `sve_remaining`, numbered from 3001 for the integer instructions and from 3101 for
//! the floating-point ones, each caught where no state of its instruction agrees with the chip.

use crate::Instruction;
use crate::planted::{PlantedBug, PlantedBy};

const fn bug(
    number: u32,
    instruction: Instruction<core::convert::Infallible>,
    what: &'static str,
) -> PlantedBug {
    PlantedBug {
        number,
        instruction,
        planted_by: PlantedBy::ScalableRemaining,
        what,
    }
}

pub const REMAINING_PLANTED_BUGS: [PlantedBug; 44] = [
    bug(
        3001,
        Instruction::SmlaltZZZI,
        "the top form reads the bottom elements",
    ),
    bug(
        3002,
        Instruction::UsdotZZZ,
        "the second source is read unsigned",
    ),
    bug(
        3003,
        Instruction::CdotZZZ,
        "the 0 and 180 degree rotations add the imaginary product",
    ),
    bug(
        3004,
        Instruction::CmlaZZZI16,
        "the imaginary part's product takes the wrong sign",
    ),
    bug(
        3005,
        Instruction::SmmlaZZZ,
        "the second matrix is read unsigned",
    ),
    bug(3006, Instruction::Sshllb, "shifts one place further"),
    bug(3007, Instruction::Sqrshrnb, "no rounding constant"),
    bug(3008, Instruction::Uqxtnb, "the source is read signed"),
    bug(
        3009,
        Instruction::AdrZAzDU32Scaled,
        "the low word is sign-extended",
    ),
    bug(
        3010,
        Instruction::PmullbZZZ,
        "the top bit of the first source is left out",
    ),
    bug(
        3011,
        Instruction::Adclb,
        "the carry is the even element's bit",
    ),
    bug(3012, Instruction::Sqcadd, "the rotations are swapped"),
    bug(
        3013,
        Instruction::Histseg,
        "an element does not count itself",
    ),
    bug(
        3014,
        Instruction::Histcnt,
        "counts the elements above it too",
    ),
    bug(3015, Instruction::Rax1ZZZ64, "rotates by two"),
    bug(
        3016,
        Instruction::MlaZZZI16,
        "the halfword index leaves out i3h",
    ),
    bug(3017, Instruction::SqrdmlahZZZ, "no rounding constant"),
    bug(3018, Instruction::SqdmulhZZZI, "saturates as unsigned"),
    bug(
        3019,
        Instruction::SqdmlalbZZZI64,
        "the doubled product is not saturated before the sum",
    ),
    bug(3020, Instruction::Bgrp, "the unselected bits are dropped"),
    bug(
        3021,
        Instruction::AesmcZZ8,
        "mixes the register at bits 9 to 5",
    ),
    bug(
        3101,
        Instruction::FlogbZPmZ,
        "a zero raises no Invalid Operation",
    ),
    bug(
        3102,
        Instruction::FaddpZPmZZ,
        "an odd element's pair comes from the first source",
    ),
    bug(3103, Instruction::FmlaZZZI32, "the product is negated"),
    bug(3104, Instruction::FmulZZZI16, "the index is ignored"),
    bug(
        3105,
        Instruction::FcmlaZPmZZ,
        "the real part's product takes the wrong sign",
    ),
    bug(
        3106,
        Instruction::FcaddZPmZ,
        "the imaginary part's addend takes the wrong sign",
    ),
    bug(
        3107,
        Instruction::Fadda,
        "each element is the first operand of its sum",
    ),
    bug(3108, Instruction::FcvtntZPZS2h, "writes the bottom half"),
    bug(3109, Instruction::FcvtltZPZS2d, "reads the bottom half"),
    bug(
        3110,
        Instruction::BfcvtZPmZ,
        "the top half keeps the destination's",
    ),
    bug(3111, Instruction::Ftssel, "the second bit does not negate"),
    bug(
        3112,
        Instruction::Ftsmul,
        "the sign is not taken from the second source",
    ),
    bug(
        3113,
        Instruction::Ftmad,
        "a negative second source keeps the sine coefficients",
    ),
    bug(3114, Instruction::BfdotZZZ, "the second pair is swapped"),
    bug(3115, Instruction::BfmlalbZZZ, "the addend is negated"),
    bug(
        3116,
        Instruction::BfmmlaZZZHtoS,
        "the second matrix is read by columns",
    ),
    bug(3117, Instruction::FmlalbZZZISHH, "the index is ignored"),
    bug(
        3118,
        Instruction::Faddv,
        "each sum takes the upper half first",
    ),
    bug(
        3119,
        Instruction::Fexpa,
        "the table index loses its lowest bit",
    ),
    bug(
        3120,
        Instruction::FrecpeZZ,
        "estimates the reciprocal square root",
    ),
    bug(
        3121,
        Instruction::FcvtxZPmZDtoS,
        "rounds as FPCR asks instead of to odd",
    ),
    bug(
        3122,
        Instruction::FminvVPZ,
        "an inactive element is negative infinity",
    ),
    bug(3123, Instruction::FcmlaZZZI16, "the index is ignored"),
];
