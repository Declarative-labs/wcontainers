//! Operands: iced's registers as ours, register reads and the x86 rules for narrow writes, memory addresses
//! and the accesses each instruction makes.

use crate::X86_64;
use crate::decode::Instruction;
use crate::flags::Condition;
use crate::simd::{BYTE, DWORD, QWORD, WORD};
use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, InstructionSink, MemArg, MemArgTarget, Ordering, ValType};
use wcontainers_guest_cpu::{
    Access, AccessBytes, AccessKind, AccessOrdering, Base, Emitter, Extend, Index, MemoryAccess,
    OrderedAccess, WriteBack,
};

pub use crate::{R8, R9, R10, R11, R15, RAX, RBP, RBX, RCX, RDI, RDX, RSI, RSP};
/// The FS and GS bases' numbers, after the general registers.
pub const FS: u8 = 16;
pub const GS: u8 = 17;

/// Operand widths in bits.
pub const W8: u8 = u8::BITS as u8;
pub const W16: u8 = u16::BITS as u8;
pub const W32: u8 = u32::BITS as u8;
pub const W64: u8 = u64::BITS as u8;
/// The bytes of a stack slot: a push, a pop and a return address.
pub const SLOT: u32 = size_of::<u64>() as u32;

/// A general register as iced names it: its number, its width in bits, and whether it is the second byte
/// (ah, ch, dh, bh).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct General {
    pub number: u8,
    pub width: u8,
    pub high: bool,
}

/// iced's general registers are in four runs of sixteen (the bytes al, cl, dl, bl, ah, ch, dh, bh, spl, bpl,
/// sil, dil, r8l to r15l first).
pub fn general(r: Register) -> Option<General> {
    let v = r as u32;
    let (number, width, high) = match v {
        1..=4 => (v - 1, 8, false),
        5..=8 => (v - 5, 8, true),
        9..=20 => (v - 5, 8, false),
        21..=36 => (v - 21, 16, false),
        37..=52 => (v - 37, 32, false),
        53..=68 => (v - 53, 64, false),
        _ => return None,
    };
    Some(General {
        number: number as u8,
        width,
        high,
    })
}

/// An xmm register's number (xmm0 to xmm15, the ones legacy SSE names).
pub fn xmm(r: Register) -> Option<u8> {
    let v = r as u32;
    (Register::XMM0 as u32..=Register::XMM15 as u32)
        .contains(&v)
        .then(|| (v - Register::XMM0 as u32) as u8)
}

/// A ymm register's number (ymm0 to ymm15).
pub fn ymm(r: Register) -> Option<u8> {
    let v = r as u32;
    (Register::YMM0 as u32..=Register::YMM15 as u32)
        .contains(&v)
        .then(|| (v - Register::YMM0 as u32) as u8)
}

/// The vector number of a ymm register's upper half.
pub fn ymm_high(n: u8) -> u8 {
    crate::guest::YMM_HIGH_VECTOR + n
}

/// An MMX register's number (mm0 to mm7).
pub fn mmx(r: Register) -> Option<u8> {
    let v = r as u32;
    (Register::MM0 as u32..=Register::MM7 as u32)
        .contains(&v)
        .then(|| (v - Register::MM0 as u32) as u8)
}

/// The register the translator keeps MMX register `n` in.
pub fn mmx_register(n: u8) -> u8 {
    crate::guest::FIRST_MMX as u8 + n
}

pub fn mask(width: u8) -> i64 {
    if width == 64 { -1 } else { (1i64 << width) - 1 }
}

/// Push a general register's value zero-extended from its width.
pub fn get_general<E: Emitter<X86_64> + ?Sized>(e: &mut E, g: General) {
    e.get(g.number);
    let mut s = e.sink();
    if g.high {
        s.i64_const(8).i64_shr_u();
    }
    if g.width < 64 {
        s.i64_const(mask(g.width)).i64_and();
    }
}

/// Write the i64 in `value` to a general register: a 32-bit write clears the upper half, an 8- or 16-bit one
/// keeps the rest.
pub fn set_general<E: Emitter<X86_64> + ?Sized>(e: &mut E, g: General, value: u32) {
    match g.width {
        64 => {
            e.sink().local_get(value);
        }
        32 => {
            e.sink().local_get(value).i64_const(0xffff_ffff).i64_and();
        }
        _ => {
            let shift = if g.high { 8 } else { 0 };
            let field = mask(g.width) << shift;
            e.get(g.number);
            let mut s = e.sink();
            s.i64_const(!field).i64_and().local_get(value);
            if shift > 0 {
                s.i64_const(shift).i64_shl();
            }
            s.i64_const(field).i64_and().i64_or();
        }
    }
    e.set(g.number);
}

/// The module's memory that is the guest's.
pub const GUEST_MEMORY: u32 = 0;

pub fn memory() -> MemArg {
    MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(GUEST_MEMORY),
        ordering: None,
    }
}

/// Push the i64 of `bytes` bytes at the data address in `at`, zero-extended.
pub fn load<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32) {
    load_at(e, at, 0, bytes);
}

/// Store the low `bytes` bytes of the i64 in `value` at the data address in `at`.
pub fn store<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32, value: u32) {
    store_at(e, at, 0, bytes, value);
}

/// Push the i64 of `bytes` bytes at the data address in `at` plus `offset`, zero-extended.
pub fn load_at<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, offset: u64, bytes: u32) {
    let Some(kind) = e.access_ordering().loads() else {
        let m = MemArg { offset, ..memory() };
        let mut s = e.sink();
        s.local_get(at);
        match bytes {
            BYTE => s.i64_load8_u(m),
            WORD => s.i64_load16_u(m),
            DWORD => s.i64_load32_u(m),
            _ => s.i64_load(m),
        };
        return;
    };
    ordered_load(e, kind, at, offset, bytes);
}

/// Store the low `bytes` bytes of the i64 local `value` at the data address in `at` plus `offset`.
pub fn store_at<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    offset: u64,
    bytes: u32,
    value: u32,
) {
    let Some(kind) = e.access_ordering().stores() else {
        let m = MemArg { offset, ..memory() };
        let mut s = e.sink();
        s.local_get(at).local_get(value);
        match bytes {
            BYTE => s.i64_store8(m),
            WORD => s.i64_store16(m),
            DWORD => s.i64_store32(m),
            _ => s.i64_store(m),
        };
        return;
    };
    ordered_store(e, kind, at, offset, bytes, value);
}

/// Push the 16 bytes at the data address in `at` plus `offset` as a v128.
pub fn load_vector_at<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, offset: u64) {
    let Some(kind) = e.access_ordering().loads() else {
        e.sink()
            .local_get(at)
            .v128_load(MemArg { offset, ..memory() });
        return;
    };
    // x86 makes no access above 8 bytes one access, so the halves are two, in address order.
    ordered_load(e, kind, at, offset, QWORD);
    e.sink().i64x2_splat();
    ordered_load(e, kind, at, offset + u64::from(QWORD), QWORD);
    e.sink().i64x2_replace_lane(1);
}

/// Store the v128 local `vector` at the data address in `at` plus `offset`.
pub fn store_vector_at<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, offset: u64, vector: u32) {
    let Some(kind) = e.access_ordering().stores() else {
        e.sink()
            .local_get(at)
            .local_get(vector)
            .v128_store(MemArg { offset, ..memory() });
        return;
    };
    let half = e.local(ValType::I64);
    for (lane, past) in [0, QWORD].into_iter().enumerate() {
        e.sink()
            .local_get(vector)
            .i64x2_extract_lane(lane as u8)
            .local_set(half);
        ordered_store(e, kind, at, offset + u64::from(past), QWORD, half);
    }
}

/// Load the element of `bytes` bytes (4 or 8) at the data address in `at` plus `offset` into lane `lane` of
/// the v128 local `vector`.
pub fn load_lane_at<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    offset: u64,
    bytes: u32,
    (vector, lane): (u32, u8),
) {
    let Some(kind) = e.access_ordering().loads() else {
        let m = MemArg { offset, ..memory() };
        let mut s = e.sink();
        s.local_get(at).local_get(vector);
        if bytes == DWORD {
            s.v128_load32_lane(m, lane);
        } else {
            s.v128_load64_lane(m, lane);
        }
        s.local_set(vector);
        return;
    };
    e.sink().local_get(vector);
    ordered_load(e, kind, at, offset, bytes);
    let mut s = e.sink();
    if bytes == DWORD {
        s.i32_wrap_i64().i32x4_replace_lane(lane);
    } else {
        s.i64x2_replace_lane(lane);
    }
    s.local_set(vector);
}

/// Store lane `lane`, of `bytes` bytes (4 or 8), of the v128 local `vector` at the data address in `at` plus
/// `offset`.
pub fn store_lane_at<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    offset: u64,
    bytes: u32,
    (vector, lane): (u32, u8),
) {
    let Some(kind) = e.access_ordering().stores() else {
        let m = MemArg { offset, ..memory() };
        let mut s = e.sink();
        s.local_get(at).local_get(vector);
        if bytes == DWORD {
            s.v128_store32_lane(m, lane);
        } else {
            s.v128_store64_lane(m, lane);
        }
        return;
    };
    let element = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_get(vector);
    if bytes == DWORD {
        s.i32x4_extract_lane(lane).i64_extend_i32_u();
    } else {
        s.i64x2_extract_lane(lane);
    }
    s.local_set(element);
    ordered_store(e, kind, at, offset, bytes, element);
}

/// Order the accesses of a bulk operation (a whole `rep movs` or `rep stos`) after every access before it and
/// before every access after it; inside it x86 orders its stores no more than the operation itself does
/// (Intel SDM Vol. 3A, 10.2.4 "Fast-String Operation and Out-of-Order Stores").
pub fn bulk_fence<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    if e.access_ordering() != AccessOrdering::Unordered {
        e.sink().atomic_fence(Ordering::SeqCst);
    }
}

/// Whether, in an ordered form, the instruction's memory operand is read, changed and written as one atomic
/// operation: a memory destination with the `lock` prefix, or `xchg` with memory, whose lock is implied (Intel SDM
/// Vol. 2B, XCHG).
pub fn locked<E: Emitter<X86_64> + ?Sized>(e: &E, i: &Instruction) -> bool {
    let ins = &i.iced;
    let memory = |n| ins.op_count() > n && ins.op_kind(n) == OpKind::Memory;
    e.access_ordering() != AccessOrdering::Unordered
        && match ins.mnemonic() {
            Mnemonic::Xchg => memory(0) || memory(1),
            _ => ins.has_lock_prefix() && memory(0),
        }
}

/// Open the loop of a locked instruction: its read of the memory operand, its work and its [`locked_write`] run
/// again until the write finds the operand as it was read.
pub fn locked_start<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    e.sink().loop_(BlockType::Empty);
}

/// Write the low `bytes` bytes of the i64 local `new` at `at` when they still hold `old` (the value read after
/// [`locked_start`]), as one atomic compare and exchange, else go round its loop again; then close the loop.
///
/// An operand not aligned to its size but inside one aligned 8-byte word is exchanged as that whole word. One that
/// crosses a word no WebAssembly atomic operation makes: it is counted and warned as
/// `Reason::LockedAccessNeedsGroupStop`, whose handler holds the process's other threads before it returns (no
/// `Parallel` form for a process that runs one, until the threads plan's group stop exists), then read again and
/// written by ordered accesses.
pub fn locked_write<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    (at, bytes): (u32, u32),
    old: u32,
    new: u32,
) {
    let Some(kind) = e.access_ordering().loads() else {
        unreachable!("only an ordered form locks");
    };
    let m = MemArg {
        offset: 0,
        align: bytes.trailing_zeros(),
        target: MemArgTarget::Memory(GUEST_MEMORY),
        ordering: kind.read_modify_write_ordering(),
    };
    let mut s = e.sink();
    aligned(&mut s, at, 0, bytes);
    s.if_(BlockType::Result(ValType::I32))
        .local_get(at)
        .local_get(old)
        .local_get(new);
    match bytes {
        BYTE => s.i64_atomic_rmw8_cmpxchg_u(m),
        WORD => s.i64_atomic_rmw16_cmpxchg_u(m),
        DWORD => s.i64_atomic_rmw32_cmpxchg_u(m),
        _ => s.i64_atomic_rmw_cmpxchg(m),
    };
    s.local_get(old).i64_ne().else_();
    let word = misaligned(e, at, 0, bytes);
    let seen = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_get(word.crosses)
        .if_(BlockType::Result(ValType::I32));
    e.warn(crate::guest::Reason::LockedAccessNeedsGroupStop);
    // Read again, now that no other thread runs: the value read before the warning may be stale.
    misaligned_load(&mut e.sink(), kind, &word, bytes);
    e.sink()
        .local_get(old)
        .i64_ne()
        .if_(BlockType::Result(ValType::I32));
    e.sink().i32_const(1).else_();
    misaligned_store(e, kind, &word, bytes, new);
    let mut s = e.sink();
    s.i32_const(0).end().else_();
    // Inside one word: exchange the word as it is when the operand still holds the value read.
    s.local_get(word.word)
        .i64_atomic_load(ordered_memory(kind, 0, QWORD))
        .local_tee(seen)
        .local_get(word.shift)
        .i64_shr_u();
    narrow(&mut s, bytes);
    s.local_get(old)
        .i64_ne()
        .if_(BlockType::Result(ValType::I32))
        .i32_const(1)
        .else_()
        .local_get(word.word)
        .local_get(seen);
    merged(&mut s, &word, (seen, new), bytes, false);
    s.i64_atomic_rmw_cmpxchg(MemArg {
        align: QWORD.trailing_zeros(),
        ..m
    })
    .local_get(seen)
    .i64_ne()
    .end()
    .end()
    .end()
    .br_if(0)
    .end();
}

/// The memory argument of an ordered access of `bytes` bytes at `offset` past its address, naturally aligned
/// as WebAssembly's atomic accesses must be.
fn ordered_memory(kind: OrderedAccess, offset: u64, bytes: u32) -> MemArg {
    MemArg {
        offset,
        align: bytes.trailing_zeros(),
        target: MemArgTarget::Memory(GUEST_MEMORY),
        ordering: kind.memarg_ordering(),
    }
}

/// Push an i32, not zero when the address in `at` plus `offset` is a multiple of `bytes`.
fn aligned(s: &mut InstructionSink<'_>, at: u32, offset: u64, bytes: u32) {
    s.local_get(at);
    if !offset.is_multiple_of(bytes.into()) {
        s.i64_const(offset as i64).i64_add();
    }
    s.i64_const(i64::from(bytes) - 1).i64_and().i64_eqz();
}

/// Keep the low `bytes` bytes of the i64 on the stack.
fn narrow(s: &mut InstructionSink<'_>, bytes: u32) {
    if bytes < QWORD {
        s.i64_const(mask((bytes * u8::BITS) as u8)).i64_and();
    }
}

/// A misaligned ordered access, made through the aligned 8-byte words that hold it (WebAssembly's widest atomic
/// accesses): i64 locals of the first word's address and the bit its first byte starts at, and an i32 local not
/// zero when it runs on into the next word.
struct Misaligned {
    word: u32,
    shift: u32,
    crosses: u32,
}

/// Work out where the access of `bytes` bytes at `at` plus `offset` lies in the words that hold it.
fn misaligned<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    offset: u64,
    bytes: u32,
) -> Misaligned {
    let word = Misaligned {
        word: e.local(ValType::I64),
        shift: e.local(ValType::I64),
        crosses: e.local(ValType::I32),
    };
    let in_word = i64::from(QWORD) - 1;
    let mut s = e.sink();
    s.local_get(at);
    if offset != 0 {
        s.i64_const(offset as i64).i64_add();
    }
    s.local_tee(word.word)
        .i64_const(in_word)
        .i64_and()
        .local_tee(word.shift)
        .i64_const(bytes.into())
        .i64_add()
        .i64_const(QWORD.into())
        .i64_gt_u()
        .local_set(word.crosses)
        .local_get(word.shift)
        .i64_const(u8::BITS.trailing_zeros().into())
        .i64_shl()
        .local_set(word.shift)
        .local_get(word.word)
        .i64_const(!in_word)
        .i64_and()
        .local_set(word.word);
    word
}

/// Push the i64 of `bytes` bytes of a misaligned access, read by an atomic load of each word it lies in, lowest
/// address first.
fn misaligned_load(
    s: &mut InstructionSink<'_>,
    kind: OrderedAccess,
    word: &Misaligned,
    bytes: u32,
) {
    s.local_get(word.word)
        .i64_atomic_load(ordered_memory(kind, 0, QWORD))
        .local_get(word.shift)
        .i64_shr_u()
        .local_get(word.crosses)
        .if_(BlockType::Result(ValType::I64))
        .local_get(word.word)
        .i64_atomic_load(ordered_memory(kind, QWORD.into(), QWORD))
        .i64_const(u64::BITS.into())
        .local_get(word.shift)
        .i64_sub()
        .i64_shl()
        .else_()
        .i64_const(0)
        .end()
        .i64_or();
    narrow(s, bytes);
}

/// Push the word `seen` with the bytes of a misaligned access that lie in it replaced by those of the i64 local
/// `value`: of the first word, or of the next when `next`.
fn merged(
    s: &mut InstructionSink<'_>,
    word: &Misaligned,
    (seen, value): (u32, u32),
    bytes: u32,
    next: bool,
) {
    let field = mask((bytes * u8::BITS) as u8);
    // The bytes of the access in the word, as a mask and then as the value.
    for part in [Some(field), None] {
        match part {
            Some(field) => s.i64_const(field),
            None => s.local_get(value),
        };
        if part.is_none() {
            narrow(s, bytes);
        }
        if next {
            s.i64_const(u64::BITS.into())
                .local_get(word.shift)
                .i64_sub()
                .i64_shr_u();
        } else {
            s.local_get(word.shift).i64_shl();
        }
        if part.is_some() {
            s.i64_const(-1).i64_xor().local_get(seen).i64_and();
        }
    }
    s.i64_or();
}

/// Store the low `bytes` bytes of the i64 local `value` in the words a misaligned access lies in, lowest address
/// first: each word by a compare and exchange of the word as read with it changed only in those bytes, retried
/// when another thread changed the word meanwhile.
fn misaligned_store<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    kind: OrderedAccess,
    word: &Misaligned,
    bytes: u32,
    value: u32,
) {
    let seen = e.local(ValType::I64);
    let mut s = e.sink();
    for next in [false, true] {
        let past = if next { u64::from(QWORD) } else { 0 };
        let m = ordered_memory(kind, past, QWORD);
        if next {
            s.local_get(word.crosses).if_(BlockType::Empty);
        }
        s.loop_(BlockType::Empty)
            .local_get(word.word)
            .local_get(word.word)
            .i64_atomic_load(m)
            .local_tee(seen);
        merged(&mut s, word, (seen, value), bytes, next);
        s.i64_atomic_rmw_cmpxchg(MemArg {
            ordering: kind.read_modify_write_ordering(),
            ..m
        })
        .local_get(seen)
        .i64_ne()
        .br_if(0)
        .end();
        if next {
            s.end();
        }
    }
}

/// Push the i64 of `bytes` bytes at `at` plus `offset` read by ordered accesses: one of its width where it is
/// aligned (WebAssembly's atomic accesses trap where it is not), else through the words that hold it.
fn ordered_load<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    kind: OrderedAccess,
    at: u32,
    offset: u64,
    bytes: u32,
) {
    let m = ordered_memory(kind, offset, bytes);
    if bytes == BYTE {
        e.sink().local_get(at).i64_atomic_load8_u(m);
        return;
    }
    let mut s = e.sink();
    aligned(&mut s, at, offset, bytes);
    s.if_(BlockType::Result(ValType::I64)).local_get(at);
    match bytes {
        WORD => s.i64_atomic_load16_u(m),
        DWORD => s.i64_atomic_load32_u(m),
        _ => s.i64_atomic_load(m),
    };
    s.else_();
    let word = misaligned(e, at, offset, bytes);
    let mut s = e.sink();
    misaligned_load(&mut s, kind, &word, bytes);
    s.end();
}

/// Store the low `bytes` bytes of the i64 local `value` at `at` plus `offset` by ordered accesses, as
/// [`ordered_load`] reads them.
fn ordered_store<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    kind: OrderedAccess,
    at: u32,
    offset: u64,
    bytes: u32,
    value: u32,
) {
    let m = ordered_memory(kind, offset, bytes);
    if bytes == BYTE {
        e.sink().local_get(at).local_get(value).i64_atomic_store8(m);
        return;
    }
    let mut s = e.sink();
    aligned(&mut s, at, offset, bytes);
    s.if_(BlockType::Empty).local_get(at).local_get(value);
    match bytes {
        WORD => s.i64_atomic_store16(m),
        DWORD => s.i64_atomic_store32(m),
        _ => s.i64_atomic_store(m),
    };
    s.else_();
    let word = misaligned(e, at, offset, bytes);
    misaligned_store(e, kind, &word, bytes, value);
    e.sink().end();
}

/// The segment base an operand adds: FS or GS, or none (the others are zero in 64-bit mode).
pub fn segment(i: &Instruction) -> Option<u8> {
    match i.iced.memory_segment() {
        Register::FS => Some(FS),
        Register::GS => Some(GS),
        _ => None,
    }
}

/// Wrap the i64 address on the stack to 32 bits when the instruction has an address-size prefix.
fn wrap<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    if i.address32 {
        e.sink().i64_const(0xffff_ffff).i64_and();
    }
}

/// The address of the instruction's memory operand, into a fresh i64 local.
pub fn address<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> u32 {
    let ins = &i.iced;
    let at = e.local(ValType::I64);
    match ins.memory_base() {
        // iced decodes at address 0, so its displacement is from this instruction.
        Register::RIP | Register::EIP => e.this_address(ins.memory_displacement64() as i64),
        base => {
            e.sink().i64_const(ins.memory_displacement64() as i64);
            if let Some(g) = general(base) {
                e.get(g.number);
                e.sink().i64_add();
            }
            // `xlat`'s index is al, zero-extended.
            if let Some(g) = general(ins.memory_index()) {
                get_general(e, g);
                let scale = ins.memory_index_scale().trailing_zeros();
                let mut s = e.sink();
                if scale > 0 {
                    s.i64_const(scale.into()).i64_shl();
                }
                s.i64_add();
            }
        }
    }
    wrap(e, i);
    if let Some(base) = segment(i) {
        e.get(base);
        e.sink().i64_add();
    }
    e.sink().local_set(at);
    at
}

/// The address `register + by`, wrapped as the instruction's address size says, into a fresh i64 local.
pub fn register_address<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    register: u8,
    by: i64,
) -> u32 {
    let at = e.local(ValType::I64);
    e.get(register);
    if by != 0 {
        e.sink().i64_const(by).i64_add();
    }
    wrap(e, i);
    e.sink().local_set(at);
    at
}

/// The bytes of the instruction's memory operand, for the integer and far-pointer sizes (iced's own sizes are
/// in its encoder and `instr_info` tables, which the shipped decoder leaves out; a test checks these against
/// them); 0 for any other.
pub fn memory_bytes(i: &Instruction) -> u32 {
    size_of_memory(i.iced.memory_size())
}

pub fn size_of_memory(size: MemorySize) -> u32 {
    use MemorySize::*;
    match size {
        UInt8 | Int8 => 1,
        UInt16 | Int16 | WordOffset | Packed16_UInt8 | Packed16_Int8 => 2,
        UInt32 | Int32 | DwordOffset | SegPtr16 | Float32 | Packed32_UInt8 | Packed32_Int8
        | Packed32_UInt16 | Packed32_Int16 => 4,
        UInt64 | Int64 | QwordOffset | Float64 | Packed64_UInt8 | Packed64_Int8
        | Packed64_UInt16 | Packed64_Int16 | Packed64_UInt32 | Packed64_Int32
        | Packed64_Float32 | Packed64_Float16 => 8,
        UInt128 | Int128 | Packed128_UInt8 | Packed128_Int8 | Packed128_UInt16
        | Packed128_Int16 | Packed128_UInt32 | Packed128_Int32 | Packed128_UInt64
        | Packed128_Int64 | Packed128_Float32 | Packed128_Float64 | Packed128_Float16
        | Float128 => 16,
        UInt256 | Int256 | Packed256_UInt8 | Packed256_Int8 | Packed256_UInt16
        | Packed256_Int16 | Packed256_UInt32 | Packed256_Int32 | Packed256_UInt64
        | Packed256_Int64 | Packed256_UInt128 | Packed256_Float32 | Packed256_Float64
        | Packed256_Float128 => 2 * crate::XMM_BYTES,
        SegPtr32 | Fword6 => 6,
        SegPtr64 | Fword10 => 10,
        Float80 | Bcd => crate::X87_REGISTER_BITS / u8::BITS,
        FpuEnv14 => crate::x87::ENVIRONMENT_16_BYTES,
        FpuEnv28 => crate::x87::ENVIRONMENT_32_BYTES,
        FpuState94 => crate::x87::ENVIRONMENT_16_BYTES + crate::x87::REGISTERS_BYTES,
        FpuState108 => crate::x87::ENVIRONMENT_32_BYTES + crate::x87::REGISTERS_BYTES,
        Fxsave_512Byte | Fxsave64_512Byte => size_of::<crate::Fxsave>() as u32,
        _ => 0,
    }
}

/// Whether operand `n` is the explicit memory operand.
pub fn is_memory(i: &Instruction, n: u32) -> bool {
    i.iced.op_kind(n) == OpKind::Memory
}

/// What the instruction does with its explicit memory operand, if it has one.
pub fn memory_use(i: &Instruction) -> Option<AccessKind> {
    let ins = &i.iced;
    let at = (0..ins.op_count()).find(|&n| is_memory(i, n))?;
    use Mnemonic::*;
    Some(match (at, ins.mnemonic()) {
        // A nop's memory operand is never reached: no access, no check, no fault.
        // Nor is a prefetch's: it is a hint that never faults.
        (_, Nop | Reservednop | Lea | Prefetchnta | Prefetcht0 | Prefetcht1 | Prefetcht2) => {
            return None;
        }
        (0, Ldmxcsr) => AccessKind::Read,
        (0, Stmxcsr) => AccessKind::Write,
        (
            0,
            Cmp | Test | Push | Call | Jmp | Mul | Imul | Div | Idiv | Bt | Clflush | Xlatb | Verr
            | Verw,
        ) => AccessKind::Read,
        (
            0,
            Mov | Pop | Movaps | Movups | Movapd | Movupd | Movdqa | Movdqu | Movq | Movd | Movnti
            | Movntdq | Movntps | Movntpd | Movss | Movsd | Movlps | Movlpd | Movhps | Movhpd
            | Pextrb | Pextrw | Pextrd | Pextrq | Extractps | Sldt | Str | Smsw | Sgdt | Sidt,
        ) => AccessKind::Write,
        (0, m) if setcc(m).is_some() => AccessKind::Write,
        // The x87 unit's and its state's: loads and operands read, stores written.
        (
            0,
            Fld | Fild | Fbld | Fadd | Fiadd | Fsub | Fisub | Fsubr | Fisubr | Fmul | Fimul | Fdiv
            | Fidiv | Fdivr | Fidivr | Fcom | Fcomp | Ficom | Ficomp | Fldcw | Fldenv | Frstor
            | Fxrstor | Fxrstor64 | Xrstor | Xrstor64,
        ) => AccessKind::Read,
        (
            0,
            Fst | Fstp | Fist | Fistp | Fisttp | Fbstp | Fnstcw | Fnstsw | Fnstenv | Fnsave
            | Fxsave | Fxsave64 | Xsave | Xsave64,
        ) => AccessKind::Write,
        (0, _) => AccessKind::ReadWrite,
        (_, Xchg) => AccessKind::ReadWrite,
        _ => AccessKind::Read,
    })
}

/// The check an access of this kind makes.
pub fn check_kind(kind: AccessKind) -> Access {
    match kind {
        AccessKind::Read => Access::Read,
        AccessKind::Write | AccessKind::ReadWrite => Access::Write,
    }
}

/// The condition of a `jcc`, `cmovcc` or `setcc` mnemonic.
pub fn condition(m: Mnemonic) -> Option<Condition> {
    jcc(m).or_else(|| cmovcc(m)).or_else(|| setcc(m))
}

pub fn jcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Jo => Condition::Overflow,
        Jno => Condition::NotOverflow,
        Jb => Condition::Below,
        Jae => Condition::AboveOrEqual,
        Je => Condition::Equal,
        Jne => Condition::NotEqual,
        Jbe => Condition::BelowOrEqual,
        Ja => Condition::Above,
        Js => Condition::Sign,
        Jns => Condition::NotSign,
        Jp => Condition::Parity,
        Jnp => Condition::NotParity,
        Jl => Condition::Less,
        Jge => Condition::GreaterOrEqual,
        Jle => Condition::LessOrEqual,
        Jg => Condition::Greater,
        _ => return None,
    })
}

pub fn cmovcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Cmovo => Condition::Overflow,
        Cmovno => Condition::NotOverflow,
        Cmovb => Condition::Below,
        Cmovae => Condition::AboveOrEqual,
        Cmove => Condition::Equal,
        Cmovne => Condition::NotEqual,
        Cmovbe => Condition::BelowOrEqual,
        Cmova => Condition::Above,
        Cmovs => Condition::Sign,
        Cmovns => Condition::NotSign,
        Cmovp => Condition::Parity,
        Cmovnp => Condition::NotParity,
        Cmovl => Condition::Less,
        Cmovge => Condition::GreaterOrEqual,
        Cmovle => Condition::LessOrEqual,
        Cmovg => Condition::Greater,
        _ => return None,
    })
}

pub fn setcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Seto => Condition::Overflow,
        Setno => Condition::NotOverflow,
        Setb => Condition::Below,
        Setae => Condition::AboveOrEqual,
        Sete => Condition::Equal,
        Setne => Condition::NotEqual,
        Setbe => Condition::BelowOrEqual,
        Seta => Condition::Above,
        Sets => Condition::Sign,
        Setns => Condition::NotSign,
        Setp => Condition::Parity,
        Setnp => Condition::NotParity,
        Setl => Condition::Less,
        Setge => Condition::GreaterOrEqual,
        Setle => Condition::LessOrEqual,
        Setg => Condition::Greater,
        _ => return None,
    })
}

/// The bytes a push or pop moves: 2 with an operand-size prefix, else 8.
pub fn stack_bytes(i: &Instruction) -> u32 {
    let ins = &i.iced;
    let width = match ins.op_kind(0) {
        // `pushf`, `popf`, `leave`, `enter` and the pushes and pops of fs and gs with 66 move words; they have no
        // general register or memory operand.
        _ if matches!(
            ins.code(),
            iced_x86::Code::Pushfw
                | iced_x86::Code::Popfw
                | iced_x86::Code::Leavew
                | iced_x86::Code::Enterw_imm16_imm8
                | iced_x86::Code::Pushw_FS
                | iced_x86::Code::Pushw_GS
                | iced_x86::Code::Popw_FS
                | iced_x86::Code::Popw_GS
        ) =>
        {
            u16::BITS as u8
        }
        OpKind::Register => general(ins.op_register(0)).map_or(64, |g| g.width),
        OpKind::Memory => memory_bytes(i) as u8 * 8,
        // An immediate's width is the operand size: `push imm16` has 66.
        _ if matches!(
            ins.code(),
            iced_x86::Code::Pushw_imm8 | iced_x86::Code::Push_imm16
        ) =>
        {
            16
        }
        _ => 64,
    };
    u32::from(width / 8)
}

/// The size of a string instruction's element.
pub fn element_bytes(i: &Instruction) -> u32 {
    memory_bytes(i)
}

/// An access through `register` (with `by` added first) of `bytes` bytes.
pub fn through(register: u8, by: i64, bytes: u32, kind: AccessKind) -> MemoryAccess<u8> {
    MemoryAccess {
        segment: None,
        base: Some(Base::Register(register)),
        index: None,
        displacement: by,
        address_bits: 64,
        write_back: WriteBack::None,
        bytes: AccessBytes::Fixed(bytes as u16),
        kind,
    }
}

/// The explicit memory operand as an access description.
pub fn operand_access(i: &Instruction, kind: AccessKind) -> MemoryAccess<u8> {
    let ins = &i.iced;
    let (base, displacement) = match ins.memory_base() {
        Register::RIP | Register::EIP => (Some(Base::Pc), ins.memory_displacement64() as i64),
        b => (
            general(b).map(|g| Base::Register(g.number)),
            ins.memory_displacement64() as i64,
        ),
    };
    let access = MemoryAccess {
        segment: segment(i),
        base,
        index: general(ins.memory_index()).map(|g| Index::Register {
            reg: g.number,
            shift: ins.memory_index_scale().trailing_zeros() as u8,
            extend: Extend {
                from_bits: g.width,
                signed: false,
            },
        }),
        displacement,
        address_bits: if i.address32 { 32 } else { 64 },
        write_back: WriteBack::None,
        bytes: AccessBytes::Fixed(memory_bytes(i) as u16),
        kind,
    };
    crate::masked::access(i, access)
}
