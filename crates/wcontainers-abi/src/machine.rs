//! What the guest is told about the CPU. The kernel reports it and the translator implements exactly it, so
//! the two can never disagree.
//!
//! Told: the base instruction set, floating point, vectors, checksum instructions and single instruction
//! atomics. Never told: the encryption extensions and scalable vectors. An instruction outside this set
//! raises [`crate::Exit::IllegalInstruction`]; it is never emulated in software.

/// Linux's `AT_HWCAP` bit names for ARM64, for the features we offer.
pub mod hwcap {
    /// Floating point.
    pub const FP: u64 = 1 << 0;
    /// Vectors (NEON).
    pub const ASIMD: u64 = 1 << 1;
    /// CRC32 checksum instructions.
    pub const CRC32: u64 = 1 << 7;
    /// Single instruction atomics (LSE).
    pub const ATOMICS: u64 = 1 << 8;
}

/// The value of `AT_HWCAP`.
pub const HWCAP: u64 = hwcap::FP | hwcap::ASIMD | hwcap::CRC32 | hwcap::ATOMICS;

/// The value of `AT_HWCAP2`.
pub const HWCAP2: u64 = 0;
