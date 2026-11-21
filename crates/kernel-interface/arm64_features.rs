#![allow(
    reviewable::interface_item,
    reviewable::magic_literal,
    reason = "Processor identification data: register fields and their checked ranges, as Arm documents them"
)]

/// The [identification registers](https://docs.kernel.org/arch/arm64/cpu-feature-registers.html) a program reads, by the fields Linux lets it see.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Arm64Features {
    pub midr: MidrEl1,
    pub ctr: CtrEl0,
    pub dczid: DczidEl0,
    pub isar0: IdAa64Isar0El1,
    pub isar1: IdAa64Isar1El1,
    pub isar2: IdAa64Isar2El1,
    pub isar3: IdAa64Isar3El1,
    pub pfr0: IdAa64Pfr0El1,
    pub pfr1: IdAa64Pfr1El1,
    pub mmfr2: IdAa64Mmfr2El1,
}

/// [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MidrEl1 {
    pub implementer: Implementer,
    pub variant: Variant,
    pub part_num: PartNum,
    pub revision: Revision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Implementer {
    Arm,
    Broadcom,
    Cavium,
    DigitalEquipment,
    Fujitsu,
    Infineon,
    Freescale,
    Nvidia,
    AppliedMicro,
    Qualcomm,
    Marvell,
    Intel,
    Ampere,
    Unlisted(UnlistedImplementer),
}

/// A code Arm assigned without publishing it, or 0, reserved for software.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnlistedImplementer(u8);

impl UnlistedImplementer {
    pub const fn new(code: u8) -> Option<Self> {
        match code {
            0x41..=0x44 | 0x46 | 0x49 | 0x4D | 0x4E | 0x50 | 0x51 | 0x56 | 0x69 | 0xC0 => None,
            _ => Some(Self(code)),
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Variant(u8);

impl Variant {
    pub const fn new(value: u8) -> Option<Self> {
        if value < 1 << 4 {
            Some(Self(value))
        } else {
            None
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PartNum(u16);

impl PartNum {
    pub const fn new(value: u16) -> Option<Self> {
        if value < 1 << 12 {
            Some(Self(value))
        } else {
            None
        }
    }
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Revision(u8);

impl Revision {
    pub const fn new(value: u8) -> Option<Self> {
        if value < 1 << 4 {
            Some(Self(value))
        } else {
            None
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CtrEl0 {
    pub dic: bool,
    pub idc: bool,
    pub d_min_line: Log2Words,
    pub i_min_line: Log2Words,
}

/// [DCZID_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/DCZID-EL0--Data-Cache-Zero-ID-Register).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DczidEl0 {
    pub dzp: bool,
    pub bs: Log2Words,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Log2Words {
    Bytes4,
    Bytes8,
    Bytes16,
    Bytes32,
    Bytes64,
    Bytes128,
    Bytes256,
    Bytes512,
    Bytes1024,
    Bytes2048,
}

/// [ID_AA64ISAR0_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR0-EL1--AArch64-Instruction-Set-Attribute-Register-0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Isar0El1 {
    pub rndr: bool,
    pub ts: Option<FlagManipulation>,
    pub fhm: bool,
    pub dp: bool,
    pub sm4: bool,
    pub sm3: bool,
    pub sha3: bool,
    pub rdm: bool,
    pub atomic: Option<Atomic>,
    pub crc32: bool,
    pub sha2: Option<Sha2>,
    pub sha1: bool,
    pub aes: Option<Aes>,
}

/// [ID_AA64ISAR1_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR1-EL1--AArch64-Instruction-Set-Attribute-Register-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Isar1El1 {
    pub i8mm: bool,
    pub dgh: bool,
    pub bf16: Option<Bf16>,
    pub sb: bool,
    pub frintts: bool,
    pub lrcpc: Option<Lrcpc>,
    pub fcma: bool,
    pub jscvt: bool,
    pub dpb: Option<Dpb>,
}

/// [ID_AA64ISAR2_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR2-EL1--AArch64-Instruction-Set-Attribute-Register-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Isar2El1 {
    pub lut: bool,
    pub cssc: Option<Cssc>,
    pub rprfm: bool,
    pub bc: bool,
    pub mops: bool,
}

/// [ID_AA64ISAR3_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64ISAR3-EL1--AArch64-Instruction-Set-Attribute-Register-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Isar3El1 {
    pub fprcvt: bool,
    pub lsfe: bool,
    pub faminmax: bool,
}

/// [ID_AA64PFR0_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR0-EL1--AArch64-Processor-Feature-Register-0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Pfr0El1 {
    pub dit: bool,
    pub fp16: bool,
}

/// [ID_AA64PFR1_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR1-EL1--AArch64-Processor-Feature-Register-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Pfr1El1 {
    pub ssbs: Option<Ssbs>,
}

/// [ID_AA64MMFR2_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64MMFR2-EL1--AArch64-Memory-Model-Feature-Register-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdAa64Mmfr2El1 {
    pub at: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FlagManipulation {
    FlagM,
    FlagM2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Atomic {
    Lse,
    Lse128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sha2 {
    Sha256,
    Sha512,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Aes {
    Aes,
    Pmull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Bf16 {
    Bf16,
    Ebf16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lrcpc {
    Lrcpc,
    Lrcpc2,
    Lrcpc3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dpb {
    Dpb,
    Dpb2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cssc {
    Cssc,
    Cmpbr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ssbs {
    Ssbs,
    Ssbs2,
}
