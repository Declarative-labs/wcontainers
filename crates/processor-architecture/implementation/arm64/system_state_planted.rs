//! The bugs planted in the PSTATE controls and in the feature sets of HINT, MSR (immediate) and SYS, numbered from 4701.

use crate::Instruction;
use crate::planted::{PlantedBug, PlantedBy};

pub const SYSTEM_STATE_PLANTED_BUGS: [PlantedBug; 7] = [
    PlantedBug {
        number: 4701,
        instruction: Instruction::MsrImmediate,
        planted_by: PlantedBy::RestOfTheSet,
        what: "MSR (immediate) writes the inverse of CRm's bit 0 to SSBS, DIT and TCO",
    },
    PlantedBug {
        number: 4702,
        instruction: Instruction::Mrs,
        planted_by: PlantedBy::RestOfTheSet,
        what: "MRS of a PSTATE control reads the other two controls",
    },
    PlantedBug {
        number: 4703,
        instruction: Instruction::MsrRegister,
        planted_by: PlantedBy::RestOfTheSet,
        what: "MSR (register) of DIT changes nothing",
    },
    PlantedBug {
        number: 4704,
        instruction: Instruction::MsrImmediate,
        planted_by: PlantedBy::RestOfTheSet,
        what: "MSR (immediate) of DIT also clears SSBS",
    },
    PlantedBug {
        number: 4705,
        instruction: Instruction::Hint,
        planted_by: PlantedBy::RestOfTheSet,
        what: "hint 54 (SRLS) is undefined, not NOP",
    },
    PlantedBug {
        number: 4706,
        instruction: Instruction::Sys,
        planted_by: PlantedBy::RestOfTheSet,
        what: "with FEAT_MTE, DC GVA, DC GZVA and the tag cleans are still undefined",
    },
    PlantedBug {
        number: 4707,
        instruction: Instruction::Sys,
        planted_by: PlantedBy::RestOfTheSet,
        what: "DC CVADP never faults, as DC CVAU",
    },
];
