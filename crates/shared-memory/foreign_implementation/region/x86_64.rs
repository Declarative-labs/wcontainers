//! The x86-64 routines: `rep movsb` for copies, `xchg` and `lock cmpxchg` for words.

use core::arch::asm;

/// Copies `length` bytes from `from` to `to`, reading each byte of `from` once and writing each byte of `to` once, and
/// touching no byte outside either range.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(unsafe_code, reason = "the one copy routine, in both directions")]
pub(super) unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    // SAFETY: `rep movsb` moves exactly `rcx` bytes from `[rsi]` to `[rdi]`, upward because the direction flag is clear on
    // entry to every asm block (Reference, asm.rules.x86-df), and leaves the flag clear; so it reaches only the two
    // ranges, at their last byte too, whatever lies past them. Clobbers: rcx, rsi and rdi, declared. It changes no
    // status flag (`preserves_flags`) and does not touch the stack (`nostack`); it reads and writes memory, so it has none
    // of `pure`, `nomem` or `readonly`.
    unsafe {
        asm!(
            "rep movsb",
            inout("rcx") length => _,
            inout("rsi") from => _,
            inout("rdi") to => _,
            options(nostack, preserves_flags),
        );
    }
}

/// Stores the value in the word and returns the value it held, in one locked access ordered both ways.
///
/// # Safety
/// The 4 bytes at `word` are writable and aligned to 4.
#[expect(unsafe_code, reason = "the processor's swap of a 4-byte word")]
pub(super) unsafe fn swap_u32(word: *mut u32, value: u32) -> u32 {
    let old: u32;
    // SAFETY: `xchg` with a memory operand is locked, and reaches only the 4 bytes at `word`. Clobbers: the value's
    // register, which returns the old value. It changes no flag (`preserves_flags`) and does not touch the stack
    // (`nostack`).
    unsafe {
        asm!(
            "xchg dword ptr [{word}], {value:e}",
            word = in(reg) word,
            value = inout(reg) value => old,
            options(nostack, preserves_flags),
        );
    }
    old
}

/// Stores the value in the word and returns the value it held, in one locked access ordered both ways.
///
/// # Safety
/// The 8 bytes at `word` are writable and aligned to 8.
#[expect(unsafe_code, reason = "the processor's swap of an 8-byte word")]
pub(super) unsafe fn swap_u64(word: *mut u64, value: u64) -> u64 {
    let old: u64;
    // SAFETY: as `swap_u32`, on the 8 bytes at `word`.
    unsafe {
        asm!(
            "xchg qword ptr [{word}], {value}",
            word = in(reg) word,
            value = inout(reg) value => old,
            options(nostack, preserves_flags),
        );
    }
    old
}

/// Stores `new` in the word if it holds `current`, and returns the value it held, in one locked access ordered both ways.
///
/// # Safety
/// The 4 bytes at `word` are writable and aligned to 4.
#[expect(
    unsafe_code,
    reason = "the processor's compare-and-exchange of a 4-byte word"
)]
pub(super) unsafe fn compare_exchange_u32(word: *mut u32, current: u32, new: u32) -> u32 {
    let old: u32;
    // SAFETY: `lock cmpxchg` reaches only the 4 bytes at `word`, comparing them with eax and leaving in eax the value they
    // held. Clobbers: eax, declared, and the status flags, so no `preserves_flags`. It does not touch the stack
    // (`nostack`).
    unsafe {
        asm!(
            "lock cmpxchg dword ptr [{word}], {new:e}",
            word = in(reg) word,
            new = in(reg) new,
            inout("eax") current => old,
            options(nostack),
        );
    }
    old
}

/// Stores `new` in the word if it holds `current`, and returns the value it held, in one locked access ordered both ways.
///
/// # Safety
/// The 8 bytes at `word` are writable and aligned to 8.
#[expect(
    unsafe_code,
    reason = "the processor's compare-and-exchange of an 8-byte word"
)]
pub(super) unsafe fn compare_exchange_u64(word: *mut u64, current: u64, new: u64) -> u64 {
    let old: u64;
    // SAFETY: as `compare_exchange_u32`, on the 8 bytes at `word` and with rax.
    unsafe {
        asm!(
            "lock cmpxchg qword ptr [{word}], {new}",
            word = in(reg) word,
            new = in(reg) new,
            inout("rax") current => old,
            options(nostack),
        );
    }
    old
}
