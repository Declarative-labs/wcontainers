// Layer 1's memory layout and wire format. The native reference runner includes this file too, so both
// sides always agree.

/// Both areas sit above 1 GiB, out of reach of the randomised start of the reference's own heap.
///
/// Start of the data area: writable pages, then one read-only page. The pages either side allow nothing.
pub const DATA: u64 = 0x4400_0000;
pub const DATA_WRITABLE: u64 = 3 * 4096;
pub const DATA_READ_ONLY: u64 = 4096;
pub const DATA_BYTES: u64 = DATA_WRITABLE + DATA_READ_ONLY;

/// Where the instruction under test sits. Everything from `CODE - CODE_SPAN` to `CODE + CODE_SPAN` may be read
/// and run, and is zero apart from the instruction; zero is `udf #0`, so control stops wherever it lands.
pub const CODE: u64 = 0x5000_0000;
pub const CODE_SPAN: u64 = 128 << 20;

/// A case: the instruction word, the data seed, then x0 to x30, sp, the flags (at their `pstate` bits) and
/// the thread pointer, all little endian.
pub const REQUEST_BYTES: usize = 4 + 8 + 34 * 8;

/// A result: signal, `si_code`, `si_addr`, then x0 to x30, sp, pc, the flags and the thread pointer, then the
/// number of changed data bytes and each one as a 4 byte offset from `DATA` and its new value.
pub const RESULT_HEADER_BYTES: usize = 4 + 4 + 8 + 35 * 8 + 4;

/// The data area's starting contents for a seed.
pub fn fill(seed: u64, into: &mut [u8]) {
    let mut state = seed;
    for chunk in into.chunks_mut(8) {
        // splitmix64
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_le_bytes()[..chunk.len()]);
    }
}
