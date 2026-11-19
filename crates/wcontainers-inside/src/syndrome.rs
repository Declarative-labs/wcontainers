//! Fields of an exception's syndrome (ESR_EL1) that the crates we use do not name.

/// The data or instruction fault status code in the syndrome's ISS.
pub const FAULT_STATUS: u64 = 0b11_1111;

/// The fault status code of an alignment fault.
pub const ALIGNMENT_FAULT: u64 = 0b10_0001;

/// The bit of a data abort's ISS set when the access wrote (WnR).
pub const WRITE_NOT_READ: u64 = 1 << 6;
