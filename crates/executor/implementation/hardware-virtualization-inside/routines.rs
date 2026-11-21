//! The few routines the inside runs that touch the guest's float state. The inside is built without float
//! instructions, so the guest's vector registers, FPSR and FPCR stay in the hardware while it runs; the monitor
//! writes these routines (made by the encoder) into a frame the inside names, and the inside runs them at its
//! own level to read and write that state.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The vector registers as a register block keeps them: each as its low half then its high half.
pub type Vectors = [[u64; 2]; wcontainers_abi::RegisterBlock::ZERO.v.len()];

/// FPSR and FPCR, as the routine that reads them returns them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloatControl {
    pub fpsr: u64,
    pub fpcr: u64,
}

crate::requests! {
    first: crate::translation::Request::END;
    /// Write the routines into the frame at guest-physical `at`.
    Routines { at } -> Placed;
}

/// Where each routine starts, in bytes from the frame's start.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Placed {
    /// `extern "C" fn() -> FloatControl`.
    pub read_float_control: u64,
    /// `extern "C" fn(fpsr: u64, fpcr: u64)`.
    pub write_float_control: u64,
    /// `extern "C" fn(into: *mut Vectors)`.
    pub save_vectors: u64,
    /// `extern "C" fn(from: *const Vectors)`.
    pub load_vectors: u64,
    /// `extern "C" fn()`, which invalidates every TLB entry of every identifier on every virtual CPU, without
    /// the barriers around it.
    pub invalidate_whole_tlb: u64,
}
