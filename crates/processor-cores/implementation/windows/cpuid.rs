//! Reading what the logical processor the thread runs on says it is, through CPUID.

use core::arch::x86_64::__cpuid_count;

use wcontainers_processor_architecture_x86_64::CoreIdentity;

/// The bits of a register that hold one field.
struct BitField {
    lowest_bit: u32,
    width: u32,
}

impl BitField {
    fn read(&self, register: u32) -> u32 {
        (register >> self.lowest_bit) & ((1 << self.width) - 1)
    }
}

/// The [subleaf](https://www.felixcloutier.com/x86/cpuid) in ECX that every leaf read here is asked with.
const SUBLEAF: u32 = 0;
/// [Leaf 0](https://www.felixcloutier.com/x86/cpuid): the highest basic leaf in EAX, the vendor in EBX, EDX and ECX.
const VENDOR_LEAF: u32 = 0;
/// [Leaf 1](https://www.felixcloutier.com/x86/cpuid): stepping, model and family in EAX.
const VERSION_LEAF: u32 = 1;
/// [Leaf 7](https://www.felixcloutier.com/x86/cpuid): structured extended features, the hybrid flag among them.
const STRUCTURED_EXTENDED_FEATURE_LEAF: u32 = 7;
/// [Leaf 0x1A](https://www.felixcloutier.com/x86/cpuid): the core type of a hybrid processor.
const HYBRID_INFORMATION_LEAF: u32 = 0x1A;
/// [Bit 15 of leaf 7's EDX](https://www.felixcloutier.com/x86/cpuid), set on a hybrid processor.
const HYBRID_FLAG: u32 = 1 << 15;
/// [Bits 3 to 0 of leaf 1's EAX](https://www.felixcloutier.com/x86/cpuid).
const STEPPING_ID: BitField = BitField {
    lowest_bit: 0,
    width: 4,
};
/// [Bits 7 to 4 of leaf 1's EAX](https://www.felixcloutier.com/x86/cpuid).
const MODEL_ID: BitField = BitField {
    lowest_bit: 4,
    width: 4,
};
/// [Bits 11 to 8 of leaf 1's EAX](https://www.felixcloutier.com/x86/cpuid).
const FAMILY_ID: BitField = BitField {
    lowest_bit: 8,
    width: 4,
};
/// [Bits 19 to 16 of leaf 1's EAX](https://www.felixcloutier.com/x86/cpuid).
const EXTENDED_MODEL_ID: BitField = BitField {
    lowest_bit: 16,
    width: 4,
};
/// [Bits 27 to 20 of leaf 1's EAX](https://www.felixcloutier.com/x86/cpuid).
const EXTENDED_FAMILY_ID: BitField = BitField {
    lowest_bit: 20,
    width: 8,
};
/// [Bits 31 to 24 of leaf 0x1A's EAX](https://www.felixcloutier.com/x86/cpuid).
const CORE_TYPE: BitField = BitField {
    lowest_bit: 24,
    width: 8,
};
/// The [family ID](https://www.felixcloutier.com/x86/cpuid) whose DisplayFamily adds the extended family and whose DisplayModel adds the extended model.
const FAMILY_WITH_EXTENSIONS: u32 = 0xF;
/// The other [family ID](https://www.felixcloutier.com/x86/cpuid) whose DisplayModel adds the extended model.
const FAMILY_WITH_EXTENDED_MODEL: u32 = 0x6;

/// Returns what the processor the thread runs on says it is, with no microcode revision, which CPUID does not give.
pub(crate) fn read_identity_by_cpuid() -> CoreIdentity {
    let vendor = __cpuid_count(VENDOR_LEAF, SUBLEAF);
    let vendor_identification: Vec<u8> = [vendor.ebx, vendor.edx, vendor.ecx]
        .iter()
        .flat_map(|register| register.to_le_bytes())
        .collect();
    let version = __cpuid_count(VERSION_LEAF, SUBLEAF).eax;
    let family_id = FAMILY_ID.read(version);
    let model_id = MODEL_ID.read(version);
    let family = match family_id {
        FAMILY_WITH_EXTENSIONS => family_id + EXTENDED_FAMILY_ID.read(version),
        _ => family_id,
    };
    let model = match family_id {
        FAMILY_WITH_EXTENSIONS | FAMILY_WITH_EXTENDED_MODEL => {
            (EXTENDED_MODEL_ID.read(version) << MODEL_ID.width) | model_id
        }
        _ => model_id,
    };
    let highest_leaf = vendor.eax;
    let hybrid = highest_leaf >= STRUCTURED_EXTENDED_FEATURE_LEAF
        && __cpuid_count(STRUCTURED_EXTENDED_FEATURE_LEAF, SUBLEAF).edx & HYBRID_FLAG != 0;
    let hybrid_core_type = (hybrid && highest_leaf >= HYBRID_INFORMATION_LEAF)
        .then(|| CORE_TYPE.read(__cpuid_count(HYBRID_INFORMATION_LEAF, SUBLEAF).eax) as u8);
    CoreIdentity {
        // Three registers of four bytes always fill the twelve.
        vendor_identification: vendor_identification.try_into().unwrap_or_default(),
        family: family as u16,
        model: model as u8,
        stepping: STEPPING_ID.read(version) as u8,
        hybrid_core_type,
        microcode_version: None,
    }
}
