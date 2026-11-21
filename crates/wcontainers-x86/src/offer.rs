//! What a program is told its processor has, as the translator and `cpuid` take it per process: the list the
//! kernel chose for the program when it started, checked against the families this crate has meanings for.

use crate::claims::TOLD_FEATURES;
use wcontainers_kernel::x86_64_features::{
    Cpuid0DSub1Eax, Cpuid01Ecx, Cpuid07Ebx, Cpuid07Ecx, Cpuid07Edx, Cpuid07Sub1Eax, Cpuid07Sub1Edx,
    Cpuid80000001Ecx, Cpuid80000001Edx, ProcessorSignature, X86_64Features,
};

/// A program's processor features, each of them one this crate runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Offer {
    features: X86_64Features,
    /// Bit n set when `TOLD_FEATURES[n]` is in the list.
    told: u32,
}

/// A feature in a list that this crate has no meanings for, by the kernel interface's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotBuilt(pub &'static str);

/// The name of the AVX10 version field, which no meaning here serves.
const AVX10: &str = "AVX10";

const _: () = assert!(TOLD_FEATURES.len() <= u32::BITS as usize);

/// The reference CPU's identity (the i7-8750H: family 6, model 0x9E, stepping 10).
pub const REFERENCE_FAMILY: u16 = 6;
pub const REFERENCE_MODEL: u8 = 0x9e;
pub const REFERENCE_STEPPING: u8 = 10;

/// What a program is told when nothing narrower is chosen: the reference CPU's identity and every family each
/// instruction of which maps onto WebAssembly with no software slow path (plan 5.10, the x86-64-v2 level).
pub const FULL_SPEED: X86_64Features = X86_64Features {
    signature: ProcessorSignature::new(REFERENCE_FAMILY, REFERENCE_MODEL, REFERENCE_STEPPING)
        .expect("the reference's signature"),
    cpuid_01_ecx: Cpuid01Ecx::SSE3
        .union(Cpuid01Ecx::SSSE3)
        .union(Cpuid01Ecx::CMPXCHG16B)
        .union(Cpuid01Ecx::SSE4_1)
        .union(Cpuid01Ecx::SSE4_2)
        .union(Cpuid01Ecx::MOVBE)
        .union(Cpuid01Ecx::POPCNT),
    cpuid_07_ebx: Cpuid07Ebx::FSGSBASE
        .union(Cpuid07Ebx::BMI1)
        .union(Cpuid07Ebx::ADX),
    cpuid_07_ecx: Cpuid07Ecx::empty(),
    cpuid_07_edx: Cpuid07Edx::empty(),
    cpuid_07_1_eax: Cpuid07Sub1Eax::empty(),
    cpuid_07_1_edx: Cpuid07Sub1Edx::empty(),
    cpuid_0d_1_eax: Cpuid0DSub1Eax::empty(),
    cpuid_80000001_ecx: Cpuid80000001Ecx::LAHF_SAHF_64.union(Cpuid80000001Ecx::LZCNT),
    cpuid_80000001_edx: Cpuid80000001Edx::RDTSCP,
    avx10: None,
};

/// [`FULL_SPEED`] with the x86-64-v3 level's families: AVX, AVX2, FMA, F16C, BMI2 and XSAVE.
pub const V3: X86_64Features = X86_64Features {
    cpuid_01_ecx: FULL_SPEED
        .cpuid_01_ecx
        .union(Cpuid01Ecx::FMA)
        .union(Cpuid01Ecx::XSAVE)
        .union(Cpuid01Ecx::AVX)
        .union(Cpuid01Ecx::F16C),
    cpuid_07_ebx: FULL_SPEED
        .cpuid_07_ebx
        .union(Cpuid07Ebx::AVX2)
        .union(Cpuid07Ebx::BMI2),
    ..FULL_SPEED
};

impl Offer {
    /// The list as an offer, refused if it holds a feature with no meanings here.
    pub fn new(features: X86_64Features) -> Result<Self, NotBuilt> {
        if features.avx10.is_some() {
            return Err(NotBuilt(AVX10));
        }
        let f = features;
        fn name<F>((n, _): (&'static str, F)) -> &'static str {
            n
        }
        let names = (f.cpuid_01_ecx.iter_names().map(name))
            .chain(f.cpuid_07_ebx.iter_names().map(name))
            .chain(f.cpuid_07_ecx.iter_names().map(name))
            .chain(f.cpuid_07_edx.iter_names().map(name))
            .chain(f.cpuid_07_1_eax.iter_names().map(name))
            .chain(f.cpuid_07_1_edx.iter_names().map(name))
            .chain(f.cpuid_0d_1_eax.iter_names().map(name))
            .chain(f.cpuid_80000001_ecx.iter_names().map(name))
            .chain(f.cpuid_80000001_edx.iter_names().map(name));
        let mut told = 0;
        for name in names {
            let at = TOLD_FEATURES
                .iter()
                .position(|&n| n == name)
                .ok_or(NotBuilt(name))?;
            told |= 1 << at;
        }
        Ok(Self { features, told })
    }

    pub fn features(self) -> X86_64Features {
        self.features
    }

    /// Whether the program is told of `TOLD_FEATURES[at]`.
    pub fn tells(self, at: usize) -> bool {
        self.told >> at & 1 != 0
    }

    /// Whether the program is told of the feature of this name.
    pub fn tells_named(self, name: &str) -> bool {
        TOLD_FEATURES
            .iter()
            .position(|&n| n == name)
            .is_some_and(|at| self.tells(at))
    }

    pub fn avx(self) -> bool {
        self.features.cpuid_01_ecx.contains(Cpuid01Ecx::AVX)
    }

    pub fn xsave(self) -> bool {
        self.features.cpuid_01_ecx.contains(Cpuid01Ecx::XSAVE)
    }

    /// The offer as one number, for the cache key and the description section: the told features' bits, and
    /// above them family, model and stepping, a byte each but the family's two.
    pub fn key(self) -> u64 {
        let s = self.features.signature;
        let signature = u64::from(s.family()) << (2 * u8::BITS)
            | u64::from(s.model()) << u8::BITS
            | u64::from(s.stepping());
        signature << u32::BITS | u64::from(self.told)
    }
}

impl Default for Offer {
    fn default() -> Self {
        Self::new(FULL_SPEED).expect("every family of the full-speed list has meanings")
    }
}
