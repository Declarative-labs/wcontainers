// What the CPU claims, in iced's `CpuidFeature` names. The build script and the crate both include this file.

/// The features claimed by default: every instruction of each maps onto WebAssembly without a software slow
/// path, and the x87 unit, which every x86-64 program may use without asking.
pub const DEFAULT_FEATURES: &[&str] = &[
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
    "SSE3",
    "SSSE3",
    "SSE4_1",
    "SSE4_2",
    "POPCNT",
    "CMPXCHG16B",
    "LZCNT",
    "BMI1",
    "ADX",
    "MOVBE",
    "RDTSCP",
    "FSGSBASE",
    "CLFSH",
    "SEP",
    "SYSCALL",
];

/// The features the AVX setting adds.
pub const AVX_FEATURES: &[&str] = &["AVX", "AVX2", "FMA", "F16C", "BMI2", "XSAVE"];

/// A `Code`'s claim group, one byte per `Code` in the table the build script writes.
pub const GROUP_DEFAULT: u8 = 0;
pub const GROUP_AVX: u8 = 1;
pub const GROUP_NEVER: u8 = 2;
pub const GROUP_KERNEL: u8 = 3;
