//! The identification registers a program reads with `mrs`, which the processor traps at EL0 and
//! Linux answers for it
//! (<https://docs.kernel.org/arch/arm64/cpu-feature-registers.html>): the processor the kernel
//! presents, the one `AT_HWCAP` and /proc/cpuinfo describe.

use wcontainers_kernel::arm64_features::{
    Aes, Arm64Features, Atomic, Bf16, Cssc, CtrEl0, DczidEl0, Dpb, FlagManipulation,
    IdAa64Isar0El1, IdAa64Isar1El1, IdAa64Isar2El1, IdAa64Isar3El1, IdAa64Mmfr2El1, IdAa64Pfr0El1,
    IdAa64Pfr1El1, Implementer, Log2Words, Lrcpc, MidrEl1, PartNum, Revision, Sha2, Ssbs,
    UnlistedImplementer, Variant,
};

/// The processor programs are told they run on: the base instruction set with floating point,
/// vectors, the CRC32 instructions and the single-instruction atomics (`AT_HWCAP`'s `fp asimd
/// crc32 atomics`), from implementer 0, part 0, as /proc/cpuinfo says. Its cache type and zero
/// block size are not read through a trap.
pub(crate) const PRESENTED: Arm64Features = Arm64Features {
    midr: MidrEl1 {
        implementer: Implementer::Unlisted(match UnlistedImplementer::new(0) {
            Some(software) => software,
            None => panic!("0 is reserved for software"),
        }),
        variant: match Variant::new(0) {
            Some(variant) => variant,
            None => panic!("a variant fits"),
        },
        part_num: match PartNum::new(0) {
            Some(part) => part,
            None => panic!("a part number fits"),
        },
        revision: match Revision::new(0) {
            Some(revision) => revision,
            None => panic!("a revision fits"),
        },
    },
    ctr: CtrEl0 {
        dic: false,
        idc: false,
        d_min_line: Log2Words::Bytes64,
        i_min_line: Log2Words::Bytes64,
    },
    dczid: DczidEl0 {
        dzp: false,
        bs: Log2Words::Bytes64,
    },
    isar0: IdAa64Isar0El1 {
        rndr: false,
        ts: None,
        fhm: false,
        dp: false,
        sm4: false,
        sm3: false,
        sha3: false,
        rdm: false,
        atomic: Some(Atomic::Lse),
        crc32: true,
        sha2: None,
        sha1: false,
        aes: None,
    },
    isar1: IdAa64Isar1El1 {
        i8mm: false,
        dgh: false,
        bf16: None,
        sb: false,
        frintts: false,
        lrcpc: None,
        fcma: false,
        jscvt: false,
        dpb: None,
    },
    isar2: IdAa64Isar2El1 {
        lut: false,
        cssc: None,
        rprfm: false,
        bc: false,
        mops: false,
    },
    isar3: IdAa64Isar3El1 {
        fprcvt: false,
        lsfe: false,
        faminmax: false,
    },
    pfr0: IdAa64Pfr0El1 {
        dit: false,
        fp16: false,
    },
    pfr1: IdAa64Pfr1El1 { ssbs: None },
    mmfr2: IdAa64Mmfr2El1 { at: false },
};

/// The encoding fields op0, op1 and CRn of every identification register Linux answers
/// (<https://docs.kernel.org/arch/arm64/cpu-feature-registers.html#:~:text=The%20infrastructure%20emulates%20only%20the%20following%20system%20register%20space>).
const IDENTIFICATION_SPACE: (u8, u8, u8) = (3, 0, 0);

/// The CRm of MIDR_EL1, MPIDR_EL1 and REVIDR_EL1, whose other op2 Linux does not answer.
const MAIN_IDENTIFICATION_CRM: u8 = 0;
/// The CRm Linux does not answer at all.
const UNANSWERED_CRM: u8 = 1;
/// The CRm past the space.
const SPACE_CRM_END: u8 = 8;

/// The op2 of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const MIDR_OP2: u8 = 0;
/// The op2 of [MPIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MPIDR-EL1--Multiprocessor-Affinity-Register).
const MPIDR_OP2: u8 = 5;
/// The op2 of [REVIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/REVIDR-EL1--Revision-ID-Register).
const REVIDR_OP2: u8 = 6;

/// What a program reads of MPIDR_EL1: only its RES1 bit 31, as no program may learn which
/// processor it is on this way
/// (<https://docs.kernel.org/arch/arm64/cpu-feature-registers.html#:~:text=MPIDR%5FEL1>).
const MPIDR_SEEN: u64 = 1 << 31;
/// What a program reads of REVIDR_EL1, which is implementation defined: zero.
const REVIDR_SEEN: u64 = 0;

/// MIDR_EL1's architecture field, 0xF for every processor that gives its features in the ID
/// registers.
const MIDR_ARCHITECTURE: u64 = 0xF;

/// One identification register's (CRm, op2).
type Place = (u8, u8);
/// [ID_AA64PFR0_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR0-EL1--AArch64-Processor-Feature-Register-0).
const PFR0: Place = (4, 0);
/// [ID_AA64PFR1_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR1-EL1--AArch64-Processor-Feature-Register-1).
const PFR1: Place = (4, 1);
/// [ID_AA64ISAR0_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR0-EL1--AArch64-Instruction-Set-Attribute-Register-0).
const ISAR0: Place = (6, 0);
/// [ID_AA64ISAR1_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR1-EL1--AArch64-Instruction-Set-Attribute-Register-1).
const ISAR1: Place = (6, 1);
/// [ID_AA64ISAR2_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR2-EL1--AArch64-Instruction-Set-Attribute-Register-2).
const ISAR2: Place = (6, 2);
/// [ID_AA64ISAR3_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR3-EL1--AArch64-Instruction-Set-Attribute-Register-3).
const ISAR3: Place = (6, 3);
/// [ID_AA64MMFR2_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64MMFR2-EL1--AArch64-Memory-Model-Feature-Register-2).
const MMFR2: Place = (7, 2);

/// Returns what a program reads from the identification register of encoding `(op0, op1, CRn,
/// CRm)` and `op2` on a processor with `features`, as Linux answers the trapped read; `None`
/// where Linux does not answer and the read is an undefined instruction.
pub(crate) fn read(features: &Arm64Features, encoding: (u8, u8, u8, u8), op2: u8) -> Option<u64> {
    let (op0, op1, crn, crm) = encoding;
    if (op0, op1, crn) != IDENTIFICATION_SPACE || crm == UNANSWERED_CRM || crm >= SPACE_CRM_END {
        return None;
    }
    if crm == MAIN_IDENTIFICATION_CRM {
        return match op2 {
            MIDR_OP2 => Some(main_identification(&features.midr)),
            MPIDR_OP2 => Some(MPIDR_SEEN),
            REVIDR_OP2 => Some(REVIDR_SEEN),
            _ => None,
        };
    }
    // A register Linux tracks shows the fields it lets a program see; any other register of the
    // space reads as zero.
    Some(match (crm, op2) {
        PFR0 => processor_features_0(&features.pfr0),
        PFR1 => processor_features_1(&features.pfr1),
        ISAR0 => instruction_set_0(&features.isar0),
        ISAR1 => instruction_set_1(&features.isar1),
        ISAR2 => instruction_set_2(&features.isar2),
        ISAR3 => instruction_set_3(&features.isar3),
        MMFR2 => memory_model_2(&features.mmfr2),
        _ => 0,
    })
}

/// A register built from `(value, lowest bit)` fields.
fn fields(parts: &[(u64, u32)]) -> u64 {
    parts
        .iter()
        .fold(0, |register, (value, at)| register | value << at)
}

/// A one-bit feature as its four-bit field: 1 where present.
fn present(feature: bool) -> u64 {
    u64::from(feature)
}

/// A field whose levels count from 1 for the first, 0 where absent.
fn level<T: Copy + Ord>(feature: Option<T>, levels: &[T]) -> u64 {
    feature.map_or(0, |feature| {
        let below = levels.iter().filter(|known| **known < feature).count();
        u64::try_from(below).expect("a level fits") + 1
    })
}

fn main_identification(midr: &MidrEl1) -> u64 {
    let implementer = match midr.implementer {
        Implementer::Arm => 0x41,
        Implementer::Broadcom => 0x42,
        Implementer::Cavium => 0x43,
        Implementer::DigitalEquipment => 0x44,
        Implementer::Fujitsu => 0x46,
        Implementer::Infineon => 0x49,
        Implementer::Freescale => 0x4D,
        Implementer::Nvidia => 0x4E,
        Implementer::AppliedMicro => 0x50,
        Implementer::Qualcomm => 0x51,
        Implementer::Marvell => 0x56,
        Implementer::Intel => 0x69,
        Implementer::Ampere => 0xC0,
        Implementer::Unlisted(code) => code.get(),
    };
    // MIDR_EL1: Implementer [31:24], Variant [23:20], Architecture [19:16], PartNum [15:4],
    // Revision [3:0].
    fields(&[
        (u64::from(implementer), 24),
        (u64::from(midr.variant.get()), 20),
        (MIDR_ARCHITECTURE, 16),
        (u64::from(midr.part_num.get()), 4),
        (u64::from(midr.revision.get()), 0),
    ])
}

/// ID_AA64PFR0_EL1: EL0 [3:0] and EL1 [7:4] are 1 (AArch64 only), FP [19:16] and AdvSIMD [23:20]
/// are 0 present and 1 with half precision, DIT [51:48].
fn processor_features_0(pfr0: &IdAa64Pfr0El1) -> u64 {
    let aarch64_only = 1;
    fields(&[
        (aarch64_only, 0),
        (aarch64_only, 4),
        (present(pfr0.fp16), 16),
        (present(pfr0.fp16), 20),
        (present(pfr0.dit), 48),
    ])
}

/// ID_AA64PFR1_EL1: SSBS [7:4].
fn processor_features_1(pfr1: &IdAa64Pfr1El1) -> u64 {
    fields(&[(level(pfr1.ssbs, &[Ssbs::Ssbs, Ssbs::Ssbs2]), 4)])
}

/// ID_AA64ISAR0_EL1: AES [7:4], SHA1 [11:8], SHA2 [15:12], CRC32 [19:16], Atomic [23:20] (2 for
/// LSE, 3 for LSE128), RDM [31:28], SHA3 [35:32], SM3 [39:36], SM4 [43:40], DP [47:44], FHM
/// [51:48], TS [55:52], RNDR [63:60].
fn instruction_set_0(isar0: &IdAa64Isar0El1) -> u64 {
    let atomic = match isar0.atomic {
        None => 0,
        Some(Atomic::Lse) => 2,
        Some(Atomic::Lse128) => 3,
    };
    fields(&[
        (level(isar0.aes, &[Aes::Aes, Aes::Pmull]), 4),
        (present(isar0.sha1), 8),
        (level(isar0.sha2, &[Sha2::Sha256, Sha2::Sha512]), 12),
        (present(isar0.crc32), 16),
        (atomic, 20),
        (present(isar0.rdm), 28),
        (present(isar0.sha3), 32),
        (present(isar0.sm3), 36),
        (present(isar0.sm4), 40),
        (present(isar0.dp), 44),
        (present(isar0.fhm), 48),
        (
            level(
                isar0.ts,
                &[FlagManipulation::FlagM, FlagManipulation::FlagM2],
            ),
            52,
        ),
        (present(isar0.rndr), 60),
    ])
}

/// ID_AA64ISAR1_EL1: DPB [3:0], JSCVT [15:12], FCMA [19:16], LRCPC [23:20], FRINTTS [35:32], SB
/// [39:36], BF16 [47:44], DGH [51:48], I8MM [55:52].
fn instruction_set_1(isar1: &IdAa64Isar1El1) -> u64 {
    fields(&[
        (level(isar1.dpb, &[Dpb::Dpb, Dpb::Dpb2]), 0),
        (present(isar1.jscvt), 12),
        (present(isar1.fcma), 16),
        (
            level(isar1.lrcpc, &[Lrcpc::Lrcpc, Lrcpc::Lrcpc2, Lrcpc::Lrcpc3]),
            20,
        ),
        (present(isar1.frintts), 32),
        (present(isar1.sb), 36),
        (level(isar1.bf16, &[Bf16::Bf16, Bf16::Ebf16]), 44),
        (present(isar1.dgh), 48),
        (present(isar1.i8mm), 52),
    ])
}

/// ID_AA64ISAR2_EL1: MOPS [19:16], BC [23:20], RPRFM [51:48], CSSC [55:52], LUT [59:56].
fn instruction_set_2(isar2: &IdAa64Isar2El1) -> u64 {
    fields(&[
        (present(isar2.mops), 16),
        (present(isar2.bc), 20),
        (present(isar2.rprfm), 48),
        (level(isar2.cssc, &[Cssc::Cssc, Cssc::Cmpbr]), 52),
        (present(isar2.lut), 56),
    ])
}

/// ID_AA64ISAR3_EL1: FAMINMAX [7:4], LSFE [19:16], FPRCVT [31:28].
fn instruction_set_3(isar3: &IdAa64Isar3El1) -> u64 {
    fields(&[
        (present(isar3.faminmax), 4),
        (present(isar3.lsfe), 16),
        (present(isar3.fprcvt), 28),
    ])
}

/// ID_AA64MMFR2_EL1: AT [35:32].
fn memory_model_2(mmfr2: &IdAa64Mmfr2El1) -> u64 {
    fields(&[(present(mmfr2.at), 32)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the presented processor's registers read, as the translator's own table has them
    /// (crates/arm64 decode_forms.rs) but for MIDR_EL1, which follows /proc/cpuinfo here.
    #[test]
    fn the_presented_processor_reads_as_its_hardware_capabilities_say() {
        let read = |crm, op2| read(&PRESENTED, (3, 0, 0, crm), op2);
        assert_eq!(read(0, 0), Some(0x000F_0000), "MIDR_EL1");
        assert_eq!(read(0, 5), Some(0x8000_0000), "MPIDR_EL1");
        assert_eq!(read(0, 6), Some(0), "REVIDR_EL1");
        assert_eq!(read(4, 0), Some(0x11), "ID_AA64PFR0_EL1");
        assert_eq!(read(6, 0), Some(0x0021_0000), "ID_AA64ISAR0_EL1");
        assert_eq!(read(6, 1), Some(0), "ID_AA64ISAR1_EL1");
        assert_eq!(
            read(5, 0),
            Some(0),
            "ID_AA64DFR0_EL1, which Linux shows no field of here"
        );
        assert_eq!(read(7, 7), Some(0), "a reserved register of the space");
    }

    #[test]
    fn reads_outside_what_linux_answers_are_undefined() {
        assert_eq!(read(&PRESENTED, (3, 0, 0, 0), 1), None, "CRm 0, op2 1");
        assert_eq!(read(&PRESENTED, (3, 0, 0, 1), 0), None, "CRm 1");
        assert_eq!(read(&PRESENTED, (3, 0, 0, 8), 0), None, "past the space");
        assert_eq!(read(&PRESENTED, (3, 1, 0, 0), 0), None, "CCSIDR_EL1");
        assert_eq!(read(&PRESENTED, (3, 3, 0, 0), 1), None, "CTR_EL0");
        assert_eq!(read(&PRESENTED, (3, 0, 1, 0), 0), None, "SCTLR_EL1");
    }

    #[test]
    fn each_feature_lands_in_its_field() {
        let mut features = PRESENTED;
        features.isar0.atomic = Some(Atomic::Lse128);
        features.isar0.aes = Some(Aes::Pmull);
        features.isar0.rndr = true;
        features.isar1.lrcpc = Some(Lrcpc::Lrcpc3);
        features.pfr0.fp16 = true;
        features.pfr0.dit = true;
        features.pfr1.ssbs = Some(Ssbs::Ssbs2);
        features.mmfr2.at = true;
        features.midr.implementer = Implementer::Arm;
        let read = |crm, op2| read(&features, (3, 0, 0, crm), op2).expect("answered");
        assert_eq!(read(6, 0), 0x1000_0000_0031_0020, "ID_AA64ISAR0_EL1");
        assert_eq!(read(6, 1), 0x0030_0000, "ID_AA64ISAR1_EL1");
        assert_eq!(read(4, 0), 0x0001_0000_0011_0011, "ID_AA64PFR0_EL1");
        assert_eq!(read(4, 1), 0x20, "ID_AA64PFR1_EL1");
        assert_eq!(read(7, 2), 0x1_0000_0000, "ID_AA64MMFR2_EL1");
        assert_eq!(read(0, 0), 0x410F_0000, "MIDR_EL1");
    }
}
