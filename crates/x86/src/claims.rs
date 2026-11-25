// What a program may be told its processor has, in iced's `CpuidFeature` names. The build script and the crate
// both include this file.

/// The features every x86-64 processor has, which no list a program is told switches off: the instructions
/// iced files by the CPU that brought them, the x87 unit, and the bits of `cpuid` the kernel interface's list
/// has no field for.
pub const BASELINE_FEATURES: &[&str] = &[
    "INTEL8086",
    "INTEL186",
    "INTEL286",
    "INTEL386",
    "INTEL486",
    "X64",
    "CPUID",
    "PAUSE",
    "MULTIBYTENOP",
    "FPU",
    "FPU287",
    "FPU387",
    "TSC",
    "CX8",
    "CMOV",
    "MMX",
    "FXSR",
    "SSE",
    "SSE2",
    "CLFSH",
    "SEP",
    "SYSCALL",
];

/// The features a program may be told of, each switched per process by its list, in the kernel interface's
/// names (iced's, but for `LAHF_SAHF_64`, whose two instructions iced files under `INTEL8086`): the instruction
/// families with meanings written, and two hints that name no instruction.
pub const TOLD_FEATURES: &[&str] = &[
    "SSE3",
    "SSSE3",
    "SSE4_1",
    "SSE4_2",
    "POPCNT",
    "CMPXCHG16B",
    "MOVBE",
    "LZCNT",
    "BMI1",
    "ADX",
    "RDTSCP",
    "FSGSBASE",
    "LAHF_SAHF_64",
    "AVX",
    "AVX2",
    "FMA",
    "F16C",
    "BMI2",
    "XSAVE",
    "ENH_REP_MOVSB_STOSB",
    "FAST_SHORT_REP_MOVSB",
];

/// The family of `lahf` and `sahf` in 64-bit mode.
pub const LAHF_SAHF_FAMILY: &str = "LAHF_SAHF_64";

/// The features of kernel-only instructions that every x86-64 CPU has, and so the CPU presented has: a user
/// program reaching one gets the fault Linux gives, not illegal instruction.
pub const KERNEL_FEATURES: &[&str] = &["MSR", "RDPMC", "SMM"];

/// A `Code`'s claim, one byte per `Code` in the table the build script writes: the baseline, a told feature's
/// place in [`TOLD_FEATURES`] after [`CLAIM_FIRST_TOLD`], kernel only, or never offered.
pub const CLAIM_BASELINE: u8 = 0;
pub const CLAIM_FIRST_TOLD: u8 = 1;
pub const CLAIM_KERNEL: u8 = u8::MAX - 1;
pub const CLAIM_NEVER: u8 = u8::MAX;
