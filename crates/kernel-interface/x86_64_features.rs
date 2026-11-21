#![allow(
    reviewable::interface_item,
    reviewable::magic_literal,
    reviewable::constant_defined_twice,
    reviewable::literal_restates_constant,
    reviewable::number_constant_without_link,
    reason = "Processor identification data: CPUID bit positions, as Intel and AMD document them"
)]

use bitflags::bitflags;

/// What [CPUID](https://cdrdv2.intel.com/v1/dl/getContent/671368) reports of an x86-64 processor's identity and optional features.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct X86_64Features {
    pub signature: ProcessorSignature,
    pub cpuid_01_ecx: Cpuid01Ecx,
    pub cpuid_07_ebx: Cpuid07Ebx,
    pub cpuid_07_ecx: Cpuid07Ecx,
    pub cpuid_07_edx: Cpuid07Edx,
    pub cpuid_07_1_eax: Cpuid07Sub1Eax,
    pub cpuid_07_1_edx: Cpuid07Sub1Edx,
    pub cpuid_0d_1_eax: Cpuid0DSub1Eax,
    pub cpuid_80000001_ecx: Cpuid80000001Ecx,
    pub cpuid_80000001_edx: Cpuid80000001Edx,
    pub avx10: Option<Avx10Version>,
}

/// `CPUID.01H:EAX`, as Intel's DisplayFamily, DisplayModel and stepping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProcessorSignature {
    family: u16,
    model: u8,
    stepping: u8,
}

impl ProcessorSignature {
    pub const fn new(family: u16, model: u8, stepping: u8) -> Option<Self> {
        let extended_model = family == 0x6 || family >= 0xF;
        if family <= 0xF + 0xFF && stepping < 1 << 4 && (model < 1 << 4 || extended_model) {
            Some(Self {
                family,
                model,
                stepping,
            })
        } else {
            None
        }
    }
    pub const fn family(self) -> u16 {
        self.family
    }
    pub const fn model(self) -> u8 {
        self.model
    }
    pub const fn stepping(self) -> u8 {
        self.stepping
    }
}

/// `CPUID.24H.0:EBX[7:0]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Avx10Version {
    V1,
    V2,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid01Ecx: u32 {
        const SSE3 = 1 << 0;
        const PCLMULQDQ = 1 << 1;
        const SSSE3 = 1 << 9;
        const FMA = 1 << 12;
        const CMPXCHG16B = 1 << 13;
        const SSE4_1 = 1 << 19;
        const SSE4_2 = 1 << 20;
        const MOVBE = 1 << 22;
        const POPCNT = 1 << 23;
        const AES = 1 << 25;
        const XSAVE = 1 << 26;
        const AVX = 1 << 28;
        const F16C = 1 << 29;
        const RDRAND = 1 << 30;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid07Ebx: u32 {
        const FSGSBASE = 1 << 0;
        const BMI1 = 1 << 3;
        const AVX2 = 1 << 5;
        const BMI2 = 1 << 8;
        const ENH_REP_MOVSB_STOSB = 1 << 9;
        const AVX512F = 1 << 16;
        const AVX512DQ = 1 << 17;
        const RDSEED = 1 << 18;
        const ADX = 1 << 19;
        const AVX512_IFMA = 1 << 21;
        const CLFLUSHOPT = 1 << 23;
        const CLWB = 1 << 24;
        const AVX512CD = 1 << 28;
        const SHA = 1 << 29;
        const AVX512BW = 1 << 30;
        const AVX512VL = 1 << 31;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid07Ecx: u32 {
        const AVX512_VBMI = 1 << 1;
        const AVX512_VBMI2 = 1 << 6;
        const GFNI = 1 << 8;
        const VAES = 1 << 9;
        const VPCLMULQDQ = 1 << 10;
        const AVX512_VNNI = 1 << 11;
        const AVX512_BITALG = 1 << 12;
        const AVX512_VPOPCNTDQ = 1 << 14;
        const RDPID = 1 << 22;
        const CLDEMOTE = 1 << 25;
        const MOVDIRI = 1 << 27;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid07Edx: u32 {
        const FAST_SHORT_REP_MOVSB = 1 << 4;
        const AVX512_VP2INTERSECT = 1 << 8;
        const SERIALIZE = 1 << 14;
        const AMX_BF16 = 1 << 22;
        const AVX512_FP16 = 1 << 23;
        const AMX_TILE = 1 << 24;
        const AMX_INT8 = 1 << 25;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid07Sub1Eax: u32 {
        const SHA512 = 1 << 0;
        const SM3 = 1 << 1;
        const SM4 = 1 << 2;
        const RAO_INT = 1 << 3;
        const AVX_VNNI = 1 << 4;
        const AVX512_BF16 = 1 << 5;
        const CMPCCXADD = 1 << 7;
        const AMX_FP16 = 1 << 21;
        const AVX_IFMA = 1 << 23;
        const MOVRS = 1 << 31;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid07Sub1Edx: u32 {
        const AVX_VNNI_INT8 = 1 << 4;
        const AVX_NE_CONVERT = 1 << 5;
        const AMX_COMPLEX = 1 << 8;
        const AVX_VNNI_INT16 = 1 << 10;
        const PREFETCHI = 1 << 14;
        const APX_F = 1 << 21;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid0DSub1Eax: u32 {
        const XSAVEOPT = 1 << 0;
        const XSAVEC = 1 << 1;
        const XGETBV1 = 1 << 2;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid80000001Ecx: u32 {
        const LAHF_SAHF_64 = 1 << 0;
        const LZCNT = 1 << 5;
        const PREFETCHW = 1 << 8;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Cpuid80000001Edx: u32 {
        const RDTSCP = 1 << 27;
    }
}
