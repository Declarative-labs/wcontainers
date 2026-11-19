//! What `cpuid` answers (plan 5.10): the default claims, or with the AVX setting the x86-64-v3 level, and
//! designed answers that say nothing about the machine the guest runs on.

use crate::{Fxsave, VIRTUAL_ADDRESS_BITS, X86Block};

/// eax, ebx, ecx and edx.
pub const OUTPUTS: usize = 4;
pub type Registers = [u32; OUTPUTS];
/// ecx's place in [`Registers`].
pub const ECX: usize = 2;

/// The highest basic and extended leaves; above either, a leaf answers as the highest basic one.
pub const MAX_BASIC: u32 = TIME_STAMP_LEAF;
pub const EXTENDED: u32 = 0x8000_0000;
pub const MAX_EXTENDED: u32 = ADDRESS_SIZES_LEAF;

pub const FEATURES_LEAF: u32 = 1;
pub const CACHE_DESCRIPTORS_LEAF: u32 = 2;
pub const CACHES_LEAF: u32 = 4;
pub const EXTENDED_FEATURES_LEAF: u32 = 7;
pub const TOPOLOGY_LEAF: u32 = 0xb;
pub const XSAVE_LEAF: u32 = 0xd;
pub const TIME_STAMP_LEAF: u32 = 0x15;
const EXTENDED_FEATURES: u32 = EXTENDED + 1;
const BRAND_LEAF: u32 = EXTENDED + 2;
const BRAND_LEAVES: u32 = (BRAND_BYTES / size_of::<Registers>()) as u32;
/// The brand string's room, NUL-padded.
const BRAND_BYTES: usize = 48;
const L2_LEAF: u32 = EXTENDED + 6;
const POWER_LEAF: u32 = EXTENDED + 7;
const ADDRESS_SIZES_LEAF: u32 = EXTENDED + 8;

/// The vendor, as the behaviour matched (undefined flags included) is Intel's.
pub const VENDOR: &[u8; 12] = b"GenuineIntel";
/// The brand string, our own.
pub const BRAND: &str = "wcontainers x86-64";

/// The reference CPU's family, model and stepping (the i7-8750H: family 6, model 0x9E, stepping 10).
pub const FAMILY: u32 = 6;
pub const MODEL: u32 = 0x9e;
pub const STEPPING: u32 = 10;

/// Leaf 1's signature fields (Intel SDM Vol. 2A, CPUID, figure 3-6): stepping, model, family, extended model.
const MODEL_SHIFT: u32 = 4;
const FAMILY_SHIFT: u32 = 8;
const EXTENDED_MODEL_SHIFT: u32 = 16;
const MODEL_FIELD_BITS: u32 = 4;
pub const SIGNATURE: u32 = STEPPING
    | (MODEL & low_bits(MODEL_FIELD_BITS)) << MODEL_SHIFT
    | FAMILY << FAMILY_SHIFT
    | (MODEL >> MODEL_FIELD_BITS) << EXTENDED_MODEL_SHIFT;

const fn low_bits(n: u32) -> u32 {
    (1 << n) - 1
}

/// Leaf 1 ebx: the `clflush` line in 8-byte units, and the logical processors a package holds (one: each
/// processor the kernel reports is a package of its own, so no count is fixed here); the initial APIC
/// identifier stays 0, as a guest thread does not know where the host runs it.
const CLFLUSH_LINE_SHIFT: u32 = 8;
const LOGICAL_PROCESSORS_SHIFT: u32 = 16;
/// The cache line, in bytes.
pub const LINE_BYTES: u32 = 64;
const CLFLUSH_UNIT: u32 = u64::BITS / u8::BITS;
const LEAF1_EBX: u32 =
    (LINE_BYTES / CLFLUSH_UNIT) << CLFLUSH_LINE_SHIFT | 1 << LOGICAL_PROCESSORS_SHIFT;

/// Leaf 1 ecx and edx feature bits (Intel SDM Vol. 2A, CPUID, tables 3-10 and 3-11).
pub mod leaf1 {
    pub const SSE3: u32 = 1 << 0;
    pub const SSSE3: u32 = 1 << 9;
    pub const FMA: u32 = 1 << 12;
    pub const CX16: u32 = 1 << 13;
    pub const SSE4_1: u32 = 1 << 19;
    pub const SSE4_2: u32 = 1 << 20;
    pub const MOVBE: u32 = 1 << 22;
    pub const POPCNT: u32 = 1 << 23;
    pub const XSAVE: u32 = 1 << 26;
    pub const OSXSAVE: u32 = 1 << 27;
    pub const AVX: u32 = 1 << 28;
    pub const F16C: u32 = 1 << 29;

    pub const FPU: u32 = 1 << 0;
    pub const TSC: u32 = 1 << 4;
    pub const CX8: u32 = 1 << 8;
    pub const SEP: u32 = 1 << 11;
    pub const CMOV: u32 = 1 << 15;
    pub const CLFSH: u32 = 1 << 19;
    pub const MMX: u32 = 1 << 23;
    pub const FXSR: u32 = 1 << 24;
    pub const SSE: u32 = 1 << 25;
    pub const SSE2: u32 = 1 << 26;
}

/// Leaf 7 sub-leaf 0 ebx feature bits (table 3-8).
pub mod leaf7 {
    pub const FSGSBASE: u32 = 1 << 0;
    pub const BMI1: u32 = 1 << 3;
    pub const AVX2: u32 = 1 << 5;
    pub const BMI2: u32 = 1 << 8;
    pub const ADX: u32 = 1 << 19;
}

/// Leaf 0x8000_0001 ecx and edx feature bits.
pub mod extended {
    pub const LAHF_SAHF: u32 = 1 << 0;
    pub const LZCNT: u32 = 1 << 5;
    pub const SYSCALL: u32 = 1 << 11;
    pub const NX: u32 = 1 << 20;
    pub const RDTSCP: u32 = 1 << 27;
    pub const LM: u32 = 1 << 29;
    /// Leaf 0x8000_0007 edx: the time stamp counter runs at one rate.
    pub const INVARIANT_TSC: u32 = 1 << 8;
}

const LEAF1_ECX: u32 = leaf1::SSE3
    | leaf1::SSSE3
    | leaf1::CX16
    | leaf1::SSE4_1
    | leaf1::SSE4_2
    | leaf1::MOVBE
    | leaf1::POPCNT;
const LEAF1_ECX_AVX: u32 = leaf1::FMA | leaf1::XSAVE | leaf1::OSXSAVE | leaf1::AVX | leaf1::F16C;
const LEAF1_EDX: u32 = leaf1::FPU
    | leaf1::TSC
    | leaf1::CX8
    | leaf1::SEP
    | leaf1::CMOV
    | leaf1::CLFSH
    | leaf1::MMX
    | leaf1::FXSR
    | leaf1::SSE
    | leaf1::SSE2;
const LEAF7_EBX: u32 = leaf7::FSGSBASE | leaf7::BMI1 | leaf7::ADX;
const LEAF7_EBX_AVX: u32 = leaf7::AVX2 | leaf7::BMI2;

/// Leaf 2: one descriptor byte, 0xFF, "no descriptors here, read leaf 4", after the count of calls, 1.
const NO_DESCRIPTORS: u32 = 0xff;
const LEAF2_EAX: u32 = NO_DESCRIPTORS << u8::BITS | 1;

/// A cache of our design: the reference CPU's sizes, each shared by one logical processor.
struct Cache {
    level: u32,
    /// 1 data, 2 instructions, 3 unified (table 3-8, leaf 4 eax bits 4:0).
    kind: u32,
    ways: u32,
    sets: u32,
}

const DATA_CACHE: u32 = 1;
const INSTRUCTION_CACHE: u32 = 2;
const UNIFIED_CACHE: u32 = 3;
pub const KIB: u32 = 1 << 10;

const fn cache(level: u32, kind: u32, ways: u32, kib: u32) -> Cache {
    Cache {
        level,
        kind,
        ways,
        sets: kib * KIB / (ways * LINE_BYTES),
    }
}

const L1_WAYS: u32 = 8;
pub const L1_KIB: u32 = 32;
const L2_WAYS: u32 = 4;
pub const L2_KIB: u32 = 256;
const L3_WAYS: u32 = 12;
pub const L3_KIB: u32 = 9 * KIB;
const CACHES: [Cache; 4] = [
    cache(1, DATA_CACHE, L1_WAYS, L1_KIB),
    cache(1, INSTRUCTION_CACHE, L1_WAYS, L1_KIB),
    cache(2, UNIFIED_CACHE, L2_WAYS, L2_KIB),
    cache(3, UNIFIED_CACHE, L3_WAYS, L3_KIB),
];

/// Leaf 4's fields: the level above the kind, a cache that initialises itself, and in ebx the ways,
/// partitions (one) and line size, each less one.
const CACHE_LEVEL_SHIFT: u32 = 5;
const SELF_INITIALISING: u32 = 1 << 8;
const WAYS_SHIFT: u32 = 22;

const fn cache_leaf(c: &Cache) -> Registers {
    [
        c.kind | c.level << CACHE_LEVEL_SHIFT | SELF_INITIALISING,
        (c.ways - 1) << WAYS_SHIFT | (LINE_BYTES - 1),
        c.sets - 1,
        0,
    ]
}

/// Leaf 0x8000_0006 ecx: the L2's size in KiB, its ways (the code for four is 4) and its line.
const L2_KIB_SHIFT: u32 = 16;
const L2_WAYS_SHIFT: u32 = 12;
const L2_ECX: u32 = L2_KIB << L2_KIB_SHIFT | L2_WAYS << L2_WAYS_SHIFT | LINE_BYTES;

/// Leaf 0xB's levels: SMT (1) and core (2), each with one logical processor and no identifier bits.
const LEVEL_TYPE_SHIFT: u32 = 8;
const SMT_LEVEL: u32 = 1;
const CORE_LEVEL: u32 = 2;
const TOPOLOGY_LEVELS: [u32; 2] = [SMT_LEVEL, CORE_LEVEL];

/// Leaf 0x15: the time stamp counter runs at the crystal's rate (ratio 1/1), a 1 GHz crystal, as `rdtsc` gives
/// the clock's nanoseconds.
pub const TIME_STAMP_HZ: u32 = 1_000_000_000;

/// Leaf 0x8000_0008 eax: physical address bits (the reference's) and linear ones.
const PHYSICAL_ADDRESS_BITS: u32 = 39;
const LINEAR_ADDRESS_SHIFT: u32 = 8;

/// The XSAVE state the AVX setting presents: x87, SSE and the upper halves of the ymm registers, after the
/// legacy area and the 64-byte XSAVE header (Intel SDM Vol. 1 13.4).
const XSAVE_HEADER_BYTES: u32 = 64;
const XSAVE_X87: u32 = 1 << 0;
const XSAVE_SSE: u32 = 1 << 1;
const XSAVE_AVX: u32 = 1 << 2;
const AVX_STATE_SUB_LEAF: u32 = XSAVE_AVX.trailing_zeros();
const AVX_STATE_OFFSET: u32 = size_of::<Fxsave>() as u32 + XSAVE_HEADER_BYTES;
/// The upper halves of the ymm registers, the block's last field.
const AVX_STATE_BYTES: u32 =
    (size_of::<X86Block>() - core::mem::offset_of!(X86Block, ymm_high)) as u32;
const XSAVE_BYTES: u32 = AVX_STATE_OFFSET + AVX_STATE_BYTES;

/// Packs bytes into registers, four to each, little-endian, zero past the end.
const fn packed<const N: usize>(bytes: &[u8]) -> [u32; N] {
    const WORD: usize = size_of::<u32>();
    let mut out = [0; N];
    let mut i = 0;
    while i < bytes.len() && i < N * WORD {
        out[i / WORD] |= (bytes[i] as u32) << (u8::BITS as usize * (i % WORD));
        i += 1;
    }
    out
}

const VENDOR_WORDS: [u32; VENDOR.len() / size_of::<u32>()] = packed(VENDOR);
const BRAND_WORDS: [u32; BRAND_BYTES / size_of::<u32>()] = packed(BRAND.as_bytes());
const _: () = assert!(BRAND.len() < BRAND_BYTES);

/// The leaf `cpuid` answers for, after the rule for leaves above the highest.
pub const fn effective_leaf(leaf: u32) -> u32 {
    if (leaf > MAX_BASIC && leaf < EXTENDED) || leaf > MAX_EXTENDED {
        MAX_BASIC
    } else {
        leaf
    }
}

/// What `cpuid` gives for this leaf and sub-leaf, with the AVX setting or without.
pub fn answer(avx: bool, leaf: u32, sub_leaf: u32) -> Registers {
    let either = |without: u32, with: u32| if avx { without | with } else { without };
    match effective_leaf(leaf) {
        0 => [MAX_BASIC, VENDOR_WORDS[0], VENDOR_WORDS[2], VENDOR_WORDS[1]],
        FEATURES_LEAF => [
            SIGNATURE,
            LEAF1_EBX,
            either(LEAF1_ECX, LEAF1_ECX_AVX),
            LEAF1_EDX,
        ],
        CACHE_DESCRIPTORS_LEAF => [LEAF2_EAX, 0, 0, 0],
        CACHES_LEAF if (sub_leaf as usize) < CACHES.len() => cache_leaf(&CACHES[sub_leaf as usize]),
        EXTENDED_FEATURES_LEAF if sub_leaf == 0 => [0, either(LEAF7_EBX, LEAF7_EBX_AVX), 0, 0],
        TOPOLOGY_LEAF if (sub_leaf as usize) < TOPOLOGY_LEVELS.len() => [
            0,
            1,
            TOPOLOGY_LEVELS[sub_leaf as usize] << LEVEL_TYPE_SHIFT | sub_leaf,
            0,
        ],
        // An invalid level gives its own number back in ecx's low byte.
        TOPOLOGY_LEAF => [0, 0, sub_leaf & u8::MAX as u32, 0],
        XSAVE_LEAF if avx && sub_leaf == 0 => [
            XSAVE_X87 | XSAVE_SSE | XSAVE_AVX,
            XSAVE_BYTES,
            XSAVE_BYTES,
            0,
        ],
        XSAVE_LEAF if avx && sub_leaf == AVX_STATE_SUB_LEAF => {
            [AVX_STATE_BYTES, AVX_STATE_OFFSET, 0, 0]
        }
        TIME_STAMP_LEAF => [1, 1, TIME_STAMP_HZ, 0],
        EXTENDED => [MAX_EXTENDED, 0, 0, 0],
        EXTENDED_FEATURES => [
            0,
            0,
            extended::LAHF_SAHF | extended::LZCNT,
            extended::SYSCALL | extended::NX | extended::RDTSCP | extended::LM,
        ],
        l if (BRAND_LEAF..BRAND_LEAF + BRAND_LEAVES).contains(&l) => {
            let at = (l - BRAND_LEAF) as usize * OUTPUTS;
            BRAND_WORDS[at..][..OUTPUTS].try_into().expect("four words")
        }
        L2_LEAF => [0, 0, L2_ECX, 0],
        POWER_LEAF => [0, 0, 0, extended::INVARIANT_TSC],
        ADDRESS_SIZES_LEAF => [
            PHYSICAL_ADDRESS_BITS | VIRTUAL_ADDRESS_BITS << LINEAR_ADDRESS_SHIFT,
            0,
            0,
            0,
        ],
        _ => [0; OUTPUTS],
    }
}

/// The sub-leaves of `leaf` that have answers of their own; every other sub-leaf answers as `past`.
pub const fn sub_leaves(leaf: u32) -> u32 {
    match leaf {
        CACHES_LEAF => CACHES.len() as u32,
        EXTENDED_FEATURES_LEAF => 1,
        TOPOLOGY_LEAF => TOPOLOGY_LEVELS.len() as u32,
        XSAVE_LEAF => AVX_STATE_SUB_LEAF + 1,
        _ => 0,
    }
}

/// Whether a sub-leaf past the ones with answers of their own gives its number back in ecx's low byte, which
/// is all of that answer's ecx.
pub const fn echoes_sub_leaf(leaf: u32) -> bool {
    leaf == TOPOLOGY_LEAF
}

/// The leaves with an answer: every basic and extended one.
pub fn leaves() -> impl Iterator<Item = u32> {
    (0..=MAX_BASIC).chain(EXTENDED..=MAX_EXTENDED)
}

/// Whether reading this leaf and sub-leaf is a program asking whether AVX is available (leaf 1, or leaf 7
/// sub-leaf 0).
pub const fn asks_for_avx(leaf: u32, sub_leaf: u32) -> bool {
    leaf == FEATURES_LEAF || (leaf == EXTENDED_FEATURES_LEAF && sub_leaf == 0)
}
