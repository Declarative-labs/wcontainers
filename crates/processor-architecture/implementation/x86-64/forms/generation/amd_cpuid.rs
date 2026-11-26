//! The CPUID bits of the AMD-only ISA sets, for which XED's database gives none, from the AMD64 manual's CPUID chapter.

use super::form::{CpuidAlternative, CpuidRequirement, CpuidSource};

/// CPUID Fn8000_0001, the extended processor features.
const EXTENDED_FEATURES_LEAF: u32 = 0x8000_0001;
/// CPUID Fn8000_0008, the address sizes and extended feature identifiers.
const EXTENDED_IDENTIFIERS_LEAF: u32 = 0x8000_0008;
const NO_SUBLEAF: u32 = 0;
const SET: u32 = 1;

/// The bit each AMD-only ISA set needs: its XED ISA set, the manual's feature name, the leaf, the register and the bit.
const AMD_FEATURES: [(&str, &str, u32, &str, u8); 8] = [
    ("3DNOW", "3DNow", EXTENDED_FEATURES_LEAF, "EDX", 31),
    ("SVM", "SVM", EXTENDED_FEATURES_LEAF, "ECX", 2),
    ("XOP", "XOP", EXTENDED_FEATURES_LEAF, "ECX", 11),
    ("LWP", "LWP", EXTENDED_FEATURES_LEAF, "ECX", 15),
    ("FMA4", "FMA4", EXTENDED_FEATURES_LEAF, "ECX", 16),
    ("TBM", "TBM", EXTENDED_FEATURES_LEAF, "ECX", 21),
    ("CLZERO", "CLZERO", EXTENDED_IDENTIFIERS_LEAF, "EBX", 0),
    (
        "AMD_INVLPGB",
        "INVLPGB",
        EXTENDED_IDENTIFIERS_LEAF,
        "EBX",
        3,
    ),
];

/// The 3DNow! instructions that the extensions to 3DNow! added, which need the 3DNowExt bit instead.
const EXTENDED_3DNOW_CLASSES: [&str; 5] = ["PF2IW", "PI2FW", "PFNACC", "PFPNACC", "PSWAPD"];
const EXTENDED_3DNOW: (&str, u32, &str, u8) = ("3DNowExt", EXTENDED_FEATURES_LEAF, "EDX", 30);

/// Returns the CPUID bit the AMD manual gives an AMD-only instruction, or none for every other instruction.
pub fn find_amd_cpuid(isa_set: &str, iclass: &str) -> Option<CpuidAlternative> {
    let &(set, name, leaf, register, bit) =
        AMD_FEATURES.iter().find(|(set, ..)| *set == isa_set)?;
    let (name, leaf, register, bit) = if EXTENDED_3DNOW_CLASSES.contains(&iclass) {
        EXTENDED_3DNOW
    } else {
        (name, leaf, register, bit)
    };
    Some(CpuidAlternative {
        group: set.to_owned(),
        source: CpuidSource::AmdManual,
        requirements: vec![CpuidRequirement {
            name: name.to_owned(),
            leaf,
            subleaf: NO_SUBLEAF,
            register: register.to_owned(),
            lowest_bit: bit,
            highest_bit: bit,
            value: SET,
        }],
    })
}
