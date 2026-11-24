//! The ARM64 routines: `ldp`/`stp` of vector registers for copies, and exclusive pairs (`ldaxr`/`stlxr`), which every
//! ARM64 processor has, for words.

use core::arch::asm;

/// Copies `length` bytes from `from` to `to`, reading each byte of `from` once and writing each byte of `to` once, and
/// touching no byte outside either range.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(unsafe_code, reason = "the one copy routine, in both directions")]
pub(super) unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    // SAFETY: the loop moves 64 bytes at a time only while at least 64 remain, then the tail moves 32, 16, 8, 4, 2 and 1
    // bytes as the bits of the count left say; each load starts where the last one ended and none overlaps another, so
    // every byte is loaded once and stored once, and the last access ends at the last byte, whatever lies past it.
    // Unaligned vector and pair accesses are allowed on the normal memory both ranges are. Clobbers: the three address
    // and count registers and a scratch register, all declared, v0 to v3, declared, and the condition flags, so no
    // `preserves_flags`. It does not touch the stack (`nostack`); it reads and writes memory, so it has none of `pure`,
    // `nomem` or `readonly`.
    unsafe {
        asm!(
            "subs {count}, {count}, #64",
            "b.lo 3f",
            "2:",
            "ldp q0, q1, [{from}], #32",
            "ldp q2, q3, [{from}], #32",
            "stp q0, q1, [{to}], #32",
            "stp q2, q3, [{to}], #32",
            "subs {count}, {count}, #64",
            "b.hs 2b",
            "3:",
            "add {count}, {count}, #64",
            "tbz {count}, #5, 4f",
            "ldp q0, q1, [{from}], #32",
            "stp q0, q1, [{to}], #32",
            "4:",
            "tbz {count}, #4, 5f",
            "ldr q0, [{from}], #16",
            "str q0, [{to}], #16",
            "5:",
            "tbz {count}, #3, 6f",
            "ldr {scratch}, [{from}], #8",
            "str {scratch}, [{to}], #8",
            "6:",
            "tbz {count}, #2, 7f",
            "ldr {scratch:w}, [{from}], #4",
            "str {scratch:w}, [{to}], #4",
            "7:",
            "tbz {count}, #1, 8f",
            "ldrh {scratch:w}, [{from}], #2",
            "strh {scratch:w}, [{to}], #2",
            "8:",
            "tbz {count}, #0, 9f",
            "ldrb {scratch:w}, [{from}]",
            "strb {scratch:w}, [{to}]",
            "9:",
            to = inout(reg) to => _,
            from = inout(reg) from => _,
            count = inout(reg) length => _,
            scratch = out(reg) _,
            out("v0") _,
            out("v1") _,
            out("v2") _,
            out("v3") _,
            options(nostack),
        );
    }
}

/// Stores the value in the word and returns the value it held, in one exclusive pair that acquires and releases.
///
/// # Safety
/// The 4 bytes at `word` are writable and aligned to 4.
#[expect(unsafe_code, reason = "the processor's swap of a 4-byte word")]
pub(super) unsafe fn swap_u32(word: *mut u32, value: u32) -> u32 {
    let old: u32;
    // SAFETY: the pair reaches only the 4 bytes at `word`, which an exclusive access needs aligned, and repeats until the
    // store succeeds. Clobbers: the old value's and the status's registers, declared as outputs that share no register
    // with the inputs. No flag changes (`preserves_flags`), and it does not touch the stack (`nostack`).
    unsafe {
        asm!(
            "2:",
            "ldaxr {old:w}, [{word}]",
            "stlxr {status:w}, {value:w}, [{word}]",
            "cbnz {status:w}, 2b",
            word = in(reg) word,
            value = in(reg) value,
            old = out(reg) old,
            status = out(reg) _,
            options(nostack, preserves_flags),
        );
    }
    old
}

/// Stores the value in the word and returns the value it held, in one exclusive pair that acquires and releases.
///
/// # Safety
/// The 8 bytes at `word` are writable and aligned to 8.
#[expect(unsafe_code, reason = "the processor's swap of an 8-byte word")]
pub(super) unsafe fn swap_u64(word: *mut u64, value: u64) -> u64 {
    let old: u64;
    // SAFETY: as `swap_u32`, on the 8 bytes at `word`.
    unsafe {
        asm!(
            "2:",
            "ldaxr {old}, [{word}]",
            "stlxr {status:w}, {value}, [{word}]",
            "cbnz {status:w}, 2b",
            word = in(reg) word,
            value = in(reg) value,
            old = out(reg) old,
            status = out(reg) _,
            options(nostack, preserves_flags),
        );
    }
    old
}

/// Stores `new` in the word if it holds `current`, and returns the value it held: an exclusive pair that acquires and
/// releases, or, when the value differs, an acquiring load.
///
/// # Safety
/// The 4 bytes at `word` are writable and aligned to 4.
#[expect(
    unsafe_code,
    reason = "the processor's compare-and-exchange of a 4-byte word"
)]
pub(super) unsafe fn compare_exchange_u32(word: *mut u32, current: u32, new: u32) -> u32 {
    let old: u32;
    // SAFETY: the pair reaches only the 4 bytes at `word`, aligned, and repeats until the store succeeds or the value
    // differs, when `clrex` drops the reservation. Clobbers: the old value's and the status's registers, declared as
    // outputs that share no register with the inputs, and the condition flags, so no `preserves_flags`. It does not
    // touch the stack (`nostack`).
    unsafe {
        asm!(
            "2:",
            "ldaxr {old:w}, [{word}]",
            "cmp {old:w}, {current:w}",
            "b.ne 3f",
            "stlxr {status:w}, {new:w}, [{word}]",
            "cbnz {status:w}, 2b",
            "b 4f",
            "3:",
            "clrex",
            "4:",
            word = in(reg) word,
            current = in(reg) current,
            new = in(reg) new,
            old = out(reg) old,
            status = out(reg) _,
            options(nostack),
        );
    }
    old
}

/// Stores `new` in the word if it holds `current`, and returns the value it held: an exclusive pair that acquires and
/// releases, or, when the value differs, an acquiring load.
///
/// # Safety
/// The 8 bytes at `word` are writable and aligned to 8.
#[expect(
    unsafe_code,
    reason = "the processor's compare-and-exchange of an 8-byte word"
)]
pub(super) unsafe fn compare_exchange_u64(word: *mut u64, current: u64, new: u64) -> u64 {
    let old: u64;
    // SAFETY: as `compare_exchange_u32`, on the 8 bytes at `word`.
    unsafe {
        asm!(
            "2:",
            "ldaxr {old}, [{word}]",
            "cmp {old}, {current}",
            "b.ne 3f",
            "stlxr {status:w}, {new}, [{word}]",
            "cbnz {status:w}, 2b",
            "b 4f",
            "3:",
            "clrex",
            "4:",
            word = in(reg) word,
            current = in(reg) current,
            new = in(reg) new,
            old = out(reg) old,
            status = out(reg) _,
            options(nostack),
        );
    }
    old
}
