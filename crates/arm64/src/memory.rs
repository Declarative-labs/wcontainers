//! The meaning of atomics, exclusives, ordered accesses, cache maintenance and the AdvSIMD structure loads
//! and stores.

use crate::Arm64;
use crate::instruction::*;
use crate::meaning::{
    Access, Control, Emitter, Hook, Trap, address, get, load, memory, save, set, store, untag,
};
use wasm_encoder::{BlockType, MemArg, MemArgTarget, ValType};

/// An access of `bytes` naturally aligned, for the atomic instructions.
fn aligned(bytes: u8) -> MemArg {
    MemArg {
        offset: 0,
        align: bytes.trailing_zeros(),
        target: MemArgTarget::Memory(0),
        ordering: None,
    }
}

/// The address in `rn`, with the stack pointer's alignment checked when it is the base, untagged, checked to
/// be a multiple of `align` (an alignment fault if not) and then checked for the access.
fn exact_address<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    rn: Reg,
    align: u32,
    bytes: u32,
    access: Access,
) -> u32 {
    let (at, _) = address(
        e,
        Address::Offset {
            base: rn,
            offset: 0,
        },
    );
    if align > 1 {
        let untagged = e.local(ValType::I64);
        let mut s = e.sink();
        untag(&mut s, at);
        s.local_tee(untagged)
            .i64_const(i64::from(align) - 1)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .if_(BlockType::Empty);
        e.trap(Trap::Alignment(untagged));
        e.sink().end();
    }
    e.check(at, bytes, access, 0);
    at
}

/// Push the low `bytes` of a register, as an i64.
fn low<E: Emitter<Arm64> + ?Sized>(e: &mut E, r: Reg, bytes: u8) {
    e.get(r);
    if bytes < 8 {
        e.sink().i64_const((1i64 << (8 * bytes)) - 1).i64_and();
    }
}

/// Sign extend the low `bytes` of the i64 on the stack.
fn sign_extend<E: Emitter<Arm64> + ?Sized>(e: &mut E, bytes: u8) {
    let mut s = e.sink();
    match bytes {
        1 => s.i64_extend8_s(),
        2 => s.i64_extend16_s(),
        4 => s.i64_extend32_s(),
        _ => &mut s,
    };
}

/// Atomic compare and exchange of `bytes` at `at`: expected and replacement pushed, the old value left.
fn compare_exchange<E: Emitter<Arm64> + ?Sized>(e: &mut E, bytes: u8) {
    let m = aligned(bytes);
    let mut s = e.sink();
    match bytes {
        1 => s.i64_atomic_rmw8_cmpxchg_u(m),
        2 => s.i64_atomic_rmw16_cmpxchg_u(m),
        4 => s.i64_atomic_rmw32_cmpxchg_u(m),
        _ => s.i64_atomic_rmw_cmpxchg(m),
    };
}

fn atomic_load<E: Emitter<Arm64> + ?Sized>(e: &mut E, at: u32, bytes: u8) {
    let m = aligned(bytes);
    let mut s = e.sink();
    s.local_get(at);
    match bytes {
        1 => s.i64_atomic_load8_u(m),
        2 => s.i64_atomic_load16_u(m),
        4 => s.i64_atomic_load32_u(m),
        _ => s.i64_atomic_load(m),
    };
}

fn atomic_store<E: Emitter<Arm64> + ?Sized>(e: &mut E, bytes: u8) {
    let m = aligned(bytes);
    let mut s = e.sink();
    match bytes {
        1 => s.i64_atomic_store8(m),
        2 => s.i64_atomic_store16(m),
        4 => s.i64_atomic_store32(m),
        _ => s.i64_atomic_store(m),
    };
}

/// Write the meaning of a memory instruction this module covers.
pub(crate) fn emit<E: Emitter<Arm64> + ?Sized>(e: &mut E, instruction: &Instruction) {
    use Instruction as I;
    match *instruction {
        I::Atomic {
            op,
            bytes,
            rs,
            rt,
            rn,
        } => {
            let at = exact_address(e, rn, bytes.into(), bytes.into(), Access::Write);
            low(e, rs, bytes);
            let value = save(e, Width::X64);
            let old = e.local(ValType::I64);
            let m = aligned(bytes);
            let direct = !matches!(
                op,
                AtomicOp::SignedMax
                    | AtomicOp::SignedMin
                    | AtomicOp::UnsignedMax
                    | AtomicOp::UnsignedMin
            );
            if direct {
                let mut s = e.sink();
                s.local_get(at).local_get(value);
                if op == AtomicOp::Clear {
                    s.i64_const(-1).i64_xor();
                }
                match (op, bytes) {
                    (AtomicOp::Add, 1) => s.i64_atomic_rmw8_add_u(m),
                    (AtomicOp::Add, 2) => s.i64_atomic_rmw16_add_u(m),
                    (AtomicOp::Add, 4) => s.i64_atomic_rmw32_add_u(m),
                    (AtomicOp::Add, _) => s.i64_atomic_rmw_add(m),
                    (AtomicOp::Clear, 1) => s.i64_atomic_rmw8_and_u(m),
                    (AtomicOp::Clear, 2) => s.i64_atomic_rmw16_and_u(m),
                    (AtomicOp::Clear, 4) => s.i64_atomic_rmw32_and_u(m),
                    (AtomicOp::Clear, _) => s.i64_atomic_rmw_and(m),
                    (AtomicOp::Xor, 1) => s.i64_atomic_rmw8_xor_u(m),
                    (AtomicOp::Xor, 2) => s.i64_atomic_rmw16_xor_u(m),
                    (AtomicOp::Xor, 4) => s.i64_atomic_rmw32_xor_u(m),
                    (AtomicOp::Xor, _) => s.i64_atomic_rmw_xor(m),
                    (AtomicOp::Set, 1) => s.i64_atomic_rmw8_or_u(m),
                    (AtomicOp::Set, 2) => s.i64_atomic_rmw16_or_u(m),
                    (AtomicOp::Set, 4) => s.i64_atomic_rmw32_or_u(m),
                    (AtomicOp::Set, _) => s.i64_atomic_rmw_or(m),
                    (_, 1) => s.i64_atomic_rmw8_xchg_u(m),
                    (_, 2) => s.i64_atomic_rmw16_xchg_u(m),
                    (_, 4) => s.i64_atomic_rmw32_xchg_u(m),
                    (_, _) => s.i64_atomic_rmw_xchg(m),
                };
                s.local_set(old);
            } else {
                // Maximum and minimum have no atomic of their own: compare and exchange until it holds.
                let signed = matches!(op, AtomicOp::SignedMax | AtomicOp::SignedMin);
                let max = matches!(op, AtomicOp::SignedMax | AtomicOp::UnsignedMax);
                let (a, b) = (e.local(ValType::I64), e.local(ValType::I64));
                atomic_load(e, at, bytes);
                e.sink().local_set(old).loop_(BlockType::Empty);
                for (from, to) in [(old, a), (value, b)] {
                    e.sink().local_get(from);
                    if signed {
                        sign_extend(e, bytes);
                    }
                    e.sink().local_set(to);
                }
                let mut s = e.sink();
                s.local_get(at)
                    .local_get(old)
                    .local_get(value)
                    .local_get(old)
                    .local_get(a)
                    .local_get(b);
                match (signed, max) {
                    (true, true) => s.i64_lt_s(),
                    (true, false) => s.i64_gt_s(),
                    (false, true) => s.i64_lt_u(),
                    (false, false) => s.i64_gt_u(),
                };
                s.select();
                compare_exchange(e, bytes);
                let mut s = e.sink();
                s.local_tee(a)
                    .local_get(old)
                    .i64_ne()
                    .if_(BlockType::Empty)
                    .local_get(a)
                    .local_set(old)
                    .br(1)
                    .end()
                    .end();
            }
            e.sink().local_get(old);
            e.set(rt);
        }
        I::CompareSwap {
            bytes,
            pair: false,
            rs,
            rt,
            rn,
        } => {
            let at = exact_address(e, rn, bytes.into(), bytes.into(), Access::Write);
            e.sink().local_get(at);
            low(e, rs, bytes);
            low(e, rt, bytes);
            compare_exchange(e, bytes);
            e.set(rs);
        }
        I::CompareSwap {
            bytes,
            pair: true,
            rs,
            rt,
            rn,
        } => {
            let total = 2 * u32::from(bytes);
            let at = exact_address(e, rn, total, total, Access::Write);
            let (rs2, rt2) = (next(rs), next(rt));
            if bytes == 4 {
                // Both halves are one 64 bit compare and exchange.
                e.sink().local_get(at);
                pair32(e, rs, rs2);
                pair32(e, rt, rt2);
                compare_exchange(e, 8);
                let old = save(e, Width::X64);
                e.sink().local_get(old).i64_const(0xffff_ffff).i64_and();
                e.set(rs);
                e.sink().local_get(old).i64_const(32).i64_shr_u();
                e.set(rs2);
            } else {
                // No 128 bit compare and exchange exists; with one thread this is exact.
                let (first, second) = (e.local(ValType::I64), e.local(ValType::I64));
                atomic_load(e, at, 8);
                e.sink().local_set(first);
                load(e, at, 8, 8, None);
                e.sink().local_set(second);
                e.sink().local_get(first);
                e.get(rs);
                e.sink().i64_eq().local_get(second);
                e.get(rs2);
                e.sink().i64_eq().i32_and().if_(BlockType::Empty);
                store(e, at, 0, 8, rt);
                store(e, at, 8, 8, rt2);
                e.sink().end().local_get(first);
                e.set(rs);
                e.sink().local_get(second);
                e.set(rs2);
            }
        }
        I::LoadExclusive {
            bytes,
            pair,
            rt,
            rt2,
            rn,
        } => {
            let total = u32::from(bytes) * if pair { 2 } else { 1 };
            let at = exact_address(e, rn, total, total, Access::Read);
            let (first, second) = (e.local(ValType::I64), e.local(ValType::I64));
            if pair && bytes == 8 {
                load(e, at, 0, 8, None);
                e.sink().local_set(first);
                load(e, at, 8, 8, None);
                e.sink().local_set(second);
            } else {
                atomic_load(e, at, total as u8);
                e.sink().local_set(first).i64_const(0).local_set(second);
            }
            remember(e, at, total, first, second);
            if pair && bytes == 4 {
                e.sink().local_get(first).i64_const(0xffff_ffff).i64_and();
                e.set(rt);
                e.sink().local_get(first).i64_const(32).i64_shr_u();
                e.set(rt2);
            } else {
                e.sink().local_get(first);
                e.set(rt);
                if pair {
                    e.sink().local_get(second);
                    e.set(rt2);
                }
            }
        }
        I::StoreExclusive {
            bytes,
            pair,
            rs,
            rt,
            rt2,
            rn,
        } => {
            let total = u32::from(bytes) * if pair { 2 } else { 1 };
            let at = exact_address(e, rn, total, total, Access::Write);
            let status = e.local(ValType::I64);
            // Remembered for this address and size?
            e.get_control(Control::ExclusiveBytes);
            e.sink().i32_const(total as i32).i32_eq();
            e.get_control(Control::ExclusiveAddress);
            e.sink().local_get(at).i64_eq().i32_and();
            e.sink().if_(BlockType::Empty);
            if pair && bytes == 8 {
                // One thread: the remembered value is still there unless this thread changed it.
                load(e, at, 0, 8, None);
                e.get_control(Control::ExclusiveLow);
                e.sink().i64_eq();
                load(e, at, 8, 8, None);
                e.get_control(Control::ExclusiveHigh);
                e.sink()
                    .i64_eq()
                    .i32_and()
                    .i64_extend_i32_u()
                    .i64_eqz()
                    .i64_extend_i32_u()
                    .local_tee(status)
                    .i64_eqz()
                    .if_(BlockType::Empty);
                store(e, at, 0, 8, rt);
                store(e, at, 8, 8, rt2);
                e.sink().end();
            } else {
                e.sink().local_get(at);
                e.get_control(Control::ExclusiveLow);
                if pair {
                    pair32(e, rt, rt2);
                } else {
                    low(e, rt, bytes);
                }
                compare_exchange(e, total as u8);
                e.get_control(Control::ExclusiveLow);
                e.sink().i64_ne().i64_extend_i32_u().local_set(status);
            }
            e.sink().else_().i64_const(1).local_set(status).end();
            e.sink().i32_const(0);
            e.set_control(Control::ExclusiveBytes);
            e.sink().local_get(status);
            e.set(rs);
        }
        I::LoadAcquire { bytes, rt, rn } => {
            let at = exact_address(e, rn, bytes.into(), bytes.into(), Access::Read);
            atomic_load(e, at, bytes);
            e.set(rt);
        }
        I::StoreRelease { bytes, rt, rn } => {
            let at = exact_address(e, rn, bytes.into(), bytes.into(), Access::Write);
            e.sink().local_get(at);
            low(e, rt, bytes);
            atomic_store(e, bytes);
        }
        I::ClearExclusive => {
            e.sink().i32_const(0);
            e.set_control(Control::ExclusiveBytes);
        }
        I::ZeroBlock { rn } => {
            let at = e.local(ValType::I64);
            e.get(rn);
            e.sink().local_set(at);
            let mut s = e.sink();
            untag(&mut s, at);
            s.i64_const(!63).i64_and().local_set(at);
            e.check(at, 64, Access::Write, 0);
            e.sink()
                .local_get(at)
                .i32_const(0)
                .i64_const(64)
                .memory_fill(0);
        }
        I::CleanCache { rn } | I::CodeChanged { rn } => {
            let at = e.local(ValType::I64);
            e.get(rn);
            e.sink().local_set(at);
            e.check(at, 1, Access::Read, 0);
            if let I::CodeChanged { .. } = instruction {
                e.sink().local_get(at);
                e.hook(Hook::CodeChanged);
            }
        }
        I::SimdMemory { .. } => structures(e, instruction),
        _ => unreachable!("not a memory instruction: {instruction:?}"),
    }
}

/// The register after `r`, for pairs.
fn next(r: Reg) -> Reg {
    match r {
        Reg::X(n) if n < 30 => Reg::X(n + 1),
        _ => Reg::Zero,
    }
}

/// Push two registers' low 32 bits as one i64, the first in the low half.
fn pair32<E: Emitter<Arm64> + ?Sized>(e: &mut E, first: Reg, second: Reg) {
    low(e, first, 4);
    e.get(second);
    e.sink().i64_const(32).i64_shl().i64_or();
}

/// Remember an exclusive access: its address, size and the value loaded.
fn remember<E: Emitter<Arm64> + ?Sized>(e: &mut E, at: u32, bytes: u32, first: u32, second: u32) {
    e.sink().i32_const(bytes as i32);
    e.set_control(Control::ExclusiveBytes);
    e.sink().local_get(at);
    e.set_control(Control::ExclusiveAddress);
    e.sink().local_get(first);
    e.set_control(Control::ExclusiveLow);
    e.sink().local_get(second);
    e.set_control(Control::ExclusiveHigh);
}

/// ld1 to ld4 and st1 to st4.
fn structures<E: Emitter<Arm64> + ?Sized>(e: &mut E, instruction: &Instruction) {
    let Instruction::SimdMemory {
        load,
        structure,
        registers,
        element,
        whole,
        lane,
        replicate,
        rt,
        rn,
        post,
    } = *instruction
    else {
        unreachable!()
    };
    let width: u32 = if whole { 16 } else { 8 };
    let bytes = if lane.is_some() || replicate {
        u32::from(structure) * u32::from(element)
    } else {
        u32::from(registers) * width
    };
    let (at, _) = address(
        e,
        Address::Offset {
            base: rn,
            offset: 0,
        },
    );
    let base = e.local(ValType::I64);
    e.sink().local_get(at).local_set(base);
    e.check(
        at,
        bytes,
        if load { Access::Read } else { Access::Write },
        0,
    );
    let register = |k: u8| (rt + k) % 32;
    let lanes = width / u32::from(element);
    let lane_op = |s: &mut wasm_encoder::InstructionSink<'_>, offset: u64, lane: u8, load: bool| {
        let m = memory(offset);
        match (element, load) {
            (1, true) => s.v128_load8_lane(m, lane),
            (2, true) => s.v128_load16_lane(m, lane),
            (4, true) => s.v128_load32_lane(m, lane),
            (_, true) => s.v128_load64_lane(m, lane),
            (1, false) => s.v128_store8_lane(m, lane),
            (2, false) => s.v128_store16_lane(m, lane),
            (4, false) => s.v128_store32_lane(m, lane),
            (_, false) => s.v128_store64_lane(m, lane),
        };
    };
    if replicate {
        for k in 0..structure {
            let offset = u64::from(k) * u64::from(element);
            let mut s = e.sink();
            s.local_get(at);
            let m = memory(offset);
            match element {
                1 => s.v128_load8_splat(m),
                2 => s.v128_load16_splat(m),
                4 => s.v128_load32_splat(m),
                _ => s.v128_load64_splat(m),
            };
            if !whole {
                s.v128_const(0xffff_ffff_ffff_ffff).v128_and();
            }
            e.set_vector(register(k));
        }
    } else if let Some(index) = lane {
        for k in 0..structure {
            let offset = u64::from(k) * u64::from(element);
            if load {
                e.sink().local_get(at);
                e.get_vector(register(k));
                lane_op(&mut e.sink(), offset, index, true);
                e.set_vector(register(k));
            } else {
                e.sink().local_get(at);
                e.get_vector(register(k));
                lane_op(&mut e.sink(), offset, index, false);
            }
        }
    } else if structure == 1 {
        for k in 0..registers {
            let offset = u64::from(k) * u64::from(width);
            let m = memory(offset);
            if load {
                let mut s = e.sink();
                s.local_get(at);
                if whole {
                    s.v128_load(m);
                } else {
                    s.v128_load64_zero(m);
                }
                e.set_vector(register(k));
            } else {
                e.sink().local_get(at);
                e.get_vector(register(k));
                let mut s = e.sink();
                if whole {
                    s.v128_store(m);
                } else {
                    s.v128_store64_lane(m, 0);
                }
            }
        }
    } else {
        // Element j of structure k is the j-th element of register k.
        for k in 0..structure {
            if load {
                e.sink().v128_const(0);
            }
            for j in 0..lanes as u8 {
                let offset =
                    (u64::from(j) * u64::from(structure) + u64::from(k)) * u64::from(element);
                if load {
                    let v = e.local(ValType::V128);
                    e.sink().local_set(v).local_get(at).local_get(v);
                    lane_op(&mut e.sink(), offset, j, true);
                } else {
                    e.sink().local_get(at);
                    e.get_vector(register(k));
                    lane_op(&mut e.sink(), offset, j, false);
                }
            }
            if load {
                e.set_vector(register(k));
            }
        }
    }
    match post {
        Some(Reg::Zero) => {
            e.sink().local_get(base).i64_const(bytes.into()).i64_add();
            e.set(rn);
        }
        Some(rm) => {
            e.sink().local_get(base);
            e.get(rm);
            e.sink().i64_add();
            e.set(rn);
        }
        None => {}
    }
    let _ = (get::<E>, set::<E>);
}
