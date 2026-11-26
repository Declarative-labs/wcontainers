/// The bytes of the [vendor identification string](https://www.felixcloutier.com/x86/cpuid) of leaf 0.
const VENDOR_IDENTIFICATION_BYTES: usize = 12;

/// The design and revision of an x86-64 logical processor as CPUID gives them, and its microcode revision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "bake", derive(databake::Bake), databake(path = wcontainers_processor_architecture_x86_64))]
pub struct CoreIdentity {
    /// From EBX, EDX and ECX of leaf 0.
    pub vendor_identification: [u8; VENDOR_IDENTIFICATION_BYTES],
    /// DisplayFamily of leaf 1.
    pub family: u16,
    /// DisplayModel of leaf 1.
    pub model: u8,
    pub stepping: u8,
    /// Leaf 0x1A's core type, on a hybrid processor only.
    pub hybrid_core_type: Option<u8>,
    /// What the system reports as loaded; none where it reports none, as in most virtual machines.
    pub microcode_version: Option<u32>,
}
