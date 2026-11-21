//! The accesses a mask selects (plan 5.4): AVX's masked moves (`vmaskmovps`, `vmaskmovpd`) and AVX2's
//! (`vpmaskmovd`, `vpmaskmovq`), and AVX2's gathers. Masked-off elements are never accessed, so they never
//! fault. A masked move checks the span of its selected elements before it moves any; a gather goes through
//! its elements in order, each checked, loaded and its mask element cleared, so that a fault leaves the
//! elements before it gathered.

use crate::decode::Instruction;
use crate::guest::Trap;
use crate::operand::*;
use crate::wide::{HALVES, halves, put};
use crate::{X86_64, X86Block, XMM_BYTES};
use iced_x86::{Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, MemArg, ValType};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::{Access, AccessBytes, Emitter, Guest, Index, MemoryAccess};

/// What a masked access is made of.
struct Shape {
    /// Bytes of each element, and how many elements there are.
    element: u32,
    count: u32,
    /// The mask register's xmm number.
    mask: u8,
    /// A gather's index register (its xmm number) and the bits of each of its elements.
    index: Option<(u8, u32)>,
}

fn vector(r: Register) -> Option<u8> {
    ymm(r).or_else(|| xmm(r))
}

/// The bytes of a vector register: 32 for a ymm one, 16 for an xmm one.
fn register_bytes(r: Register) -> u32 {
    if ymm(r).is_some() {
        2 * XMM_BYTES
    } else {
        XMM_BYTES
    }
}

fn shape(i: &Instruction) -> Option<Shape> {
    use Mnemonic::*;
    let ins = &i.iced;
    if crate::decode::claim(ins.code()) == crate::claims::CLAIM_NEVER {
        return None;
    }
    let dword = size_of::<u32>() as u32;
    let qword = size_of::<u64>() as u32;
    match ins.mnemonic() {
        Vmaskmovps | Vmaskmovpd | Vpmaskmovd | Vpmaskmovq => {
            let element = if matches!(ins.mnemonic(), Vmaskmovps | Vpmaskmovd) {
                dword
            } else {
                qword
            };
            Some(Shape {
                element,
                count: memory_bytes(i) / element,
                mask: vector(ins.op_register(1))?,
                index: None,
            })
        }
        m @ (Vgatherdps | Vgatherdpd | Vgatherqps | Vgatherqpd | Vpgatherdd | Vpgatherdq
        | Vpgatherqd | Vpgatherqq) => {
            let element = if matches!(m, Vgatherdps | Vgatherqps | Vpgatherdd | Vpgatherqd) {
                dword
            } else {
                qword
            };
            let index_bits = if matches!(m, Vgatherdps | Vgatherdpd | Vpgatherdd | Vpgatherdq) {
                u32::BITS
            } else {
                u64::BITS
            };
            let r = ins.memory_index();
            let count = (register_bytes(ins.op_register(0)) / element)
                .min(register_bytes(r) * u8::BITS / index_bits);
            Some(Shape {
                element,
                count,
                mask: vector(ins.op_register(2))?,
                index: Some((vector(r)?, index_bits)),
            })
        }
        _ => None,
    }
}

pub fn written(i: &Instruction) -> bool {
    shape(i).is_some()
}

/// The instruction's access as `Guest::accesses` describes it: `Masked` bytes, and a gather's `Lanes` index.
pub fn access(i: &Instruction, access: MemoryAccess<u8>) -> MemoryAccess<u8> {
    let Some(shape) = shape(i) else {
        return access;
    };
    MemoryAccess {
        bytes: AccessBytes::Masked {
            element: shape.element as u8,
            count: shape.count as u8,
        },
        index: match shape.index {
            Some((reg, bits)) => Some(Index::Lanes {
                reg,
                lane_bits: bits as u8,
                shift: i.iced.memory_index_scale().trailing_zeros() as u8,
            }),
            None => access.index,
        },
        ..access
    }
}

/// A vector register's 32 bytes in the block, its upper half from `ymm_high`.
fn register(block: &X86Block, x: u8) -> [u8; 2 * XMM_BYTES as usize] {
    let mut v = [0; 2 * XMM_BYTES as usize];
    v[..XMM_BYTES as usize].copy_from_slice(&block.fxsave.xmm[usize::from(x)]);
    v[XMM_BYTES as usize..].copy_from_slice(&block.ymm_high[usize::from(x)]);
    v
}

/// Whether element `n` of `element` bytes has its top bit set.
fn selected(v: &[u8], element: u32, n: u32) -> bool {
    v[((n + 1) * element - 1) as usize] >> (u8::BITS - 1) != 0
}

/// The bytes of the access that met a fault, first and last, from the block at the fault: a masked move's span
/// from its first selected element to its last, a gather's first element whose mask is still set. `start` is
/// the operand's address before the address size's wrap and the segment base, which `finish` adds.
pub fn fault_span(
    i: &Instruction,
    block: &X86Block,
    start: u64,
    finish: impl Fn(u64) -> u64,
) -> Option<(u64, u64)> {
    let shape = shape(i)?;
    let mask = register(block, shape.mask);
    let mut chosen = (0..shape.count).filter(|&n| selected(&mask, shape.element, n));
    let first = chosen.next()?;
    let span = |from: u64, elements: u32| {
        (
            from,
            from.wrapping_add(u64::from(elements * shape.element) - 1),
        )
    };
    match shape.index {
        None => {
            let last = chosen.next_back().unwrap_or(first);
            let from = finish(start).wrapping_add(u64::from(first * shape.element));
            Some(span(from, last - first + 1))
        }
        Some((index, bits)) => {
            let lanes = register(block, index);
            let bytes = (bits / u8::BITS) as usize;
            let at = first as usize * bytes;
            let mut raw = [0; size_of::<u64>()];
            raw[..bytes].copy_from_slice(&lanes[at..at + bytes]);
            let unused = u64::BITS - bits;
            let lane = ((u64::from_le_bytes(raw) << unused) as i64 >> unused) as u64;
            let shift = i.iced.memory_index_scale().trailing_zeros();
            Some(span(finish(start.wrapping_add(lane << shift)), 1))
        }
    }
}

/// An element's place in a ymm register: its half and its lane number there.
fn place(n: u32, bytes: u32) -> (usize, u32) {
    (
        (n * bytes / XMM_BYTES) as usize,
        n * bytes % XMM_BYTES / bytes,
    )
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let shape = shape(i).expect("a masked access");
    if shape.index.is_some() {
        gather(e, i, &shape);
    } else {
        masked_move(e, i, &shape);
    }
}

/// The memory argument of an element `offset` bytes past the operand's data address.
fn lane_memory(offset: u32) -> MemArg {
    MemArg {
        offset: offset.into(),
        ..memory()
    }
}

fn masked_move<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, shape: &Shape) {
    let ins = &i.iced;
    let load = ins.op_kind(0) == OpKind::Register;
    let element = shape.element;
    let per_half = XMM_BYTES / element;
    let mask = halves(e, i, 1, None);
    let data = if load {
        None
    } else {
        Some(halves(e, i, 2, None))
    };
    // The selected elements, one bit each.
    let bits = e.local(ValType::I32);
    {
        let mut s = e.sink();
        for (h, &half) in mask.iter().enumerate() {
            s.local_get(half);
            if element == size_of::<u32>() as u32 {
                s.i32x4_bitmask();
            } else {
                s.i64x2_bitmask();
            }
            if h > 0 {
                s.i32_const(per_half as i32).i32_shl().i32_or();
            }
        }
        s.local_set(bits);
    }
    let at = address(e, i);
    let (first, last) = (e.local(ValType::I64), e.local(ValType::I64));
    e.sink()
        .local_get(bits)
        .i32_eqz()
        .i32_eqz()
        .if_(BlockType::Empty);
    // S, the first byte of the first selected element, and E, the last byte of the last one.
    e.sink()
        .local_get(at)
        .local_get(bits)
        .i32_ctz()
        .i64_extend_i32_u()
        .i64_const(element.into())
        .i64_mul()
        .i64_add()
        .local_set(first)
        .local_get(at)
        .i32_const(u32::BITS as i32)
        .local_get(bits)
        .i32_clz()
        .i32_sub()
        .i64_extend_i32_u()
        .i64_const(element.into())
        .i64_mul()
        .i64_add()
        .i64_const(1)
        .i64_sub()
        .local_set(last);
    if load {
        // The first byte not allowed of [S, E]: S, or the start of E's page when that is after S.
        e.sink()
            .local_get(last)
            .i64_const(!(PAGE_SIZE as i64 - 1))
            .i64_and()
            .local_get(first)
            .local_get(last)
            .i64_const(!(PAGE_SIZE as i64 - 1))
            .i64_and()
            .local_get(first)
            .i64_gt_u()
            .select()
            .local_set(last);
    }
    let access = if load { Access::Read } else { Access::Write };
    e.check(first, 1, access, 0);
    e.check(last, 1, access, 0);
    e.sink().end();
    let data_at = e.local(ValType::I64);
    X86_64::data_address(&mut e.sink(), at);
    e.sink().local_set(data_at);
    let result = [e.local(ValType::V128), e.local(ValType::V128)];
    for &r in &result {
        e.sink().v128_const(0).local_set(r);
    }
    for n in 0..shape.count {
        let (h, lane) = place(n, element);
        let mut s = e.sink();
        s.local_get(bits)
            .i32_const(1 << n)
            .i32_and()
            .if_(BlockType::Empty)
            .local_get(data_at);
        let memarg = lane_memory(n * element);
        match (&data, element == size_of::<u32>() as u32) {
            (None, true) => {
                s.local_get(result[h])
                    .v128_load32_lane(memarg, lane as u8)
                    .local_set(result[h]);
            }
            (None, false) => {
                s.local_get(result[h])
                    .v128_load64_lane(memarg, lane as u8)
                    .local_set(result[h]);
            }
            (Some(d), true) => {
                s.local_get(d[h]).v128_store32_lane(memarg, lane as u8);
            }
            (Some(d), false) => {
                s.local_get(d[h]).v128_store64_lane(memarg, lane as u8);
            }
        }
        s.end();
    }
    if load {
        put(e, i, None, result);
    }
}

fn gather<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, shape: &Shape) {
    let ins = &i.iced;
    let destination = vector(ins.op_register(0)).expect("a vector destination");
    let (index, index_bits) = shape.index.expect("a gather's index");
    // The destination, the index and the mask must be three registers.
    if destination == index || destination == shape.mask || index == shape.mask {
        e.trap(Trap::Undefined);
        return;
    }
    let element = shape.element;
    let index_bytes = index_bits / u8::BITS;
    let result = halves(e, i, 0, None);
    let mask = halves(e, i, 2, None);
    let wide_index = ymm(ins.memory_index()).is_some();
    let lanes: [u32; HALVES] = core::array::from_fn(|_| e.local(ValType::V128));
    e.get_vector(index);
    e.sink().local_set(lanes[0]);
    if wide_index {
        e.get_vector(ymm_high(index));
    } else {
        e.sink().v128_const(0);
    }
    e.sink().local_set(lanes[1]);
    // The base and displacement, without the index (VSIB's is a vector register).
    let start = e.local(ValType::I64);
    e.sink().i64_const(ins.memory_displacement64() as i64);
    if let Some(g) = general(ins.memory_base()) {
        e.get(g.number);
        e.sink().i64_add();
    }
    e.sink().local_set(start);
    let shift = ins.memory_index_scale().trailing_zeros();
    let wide_destination = ymm(ins.op_register(0)).is_some();
    // A 128-bit form's upper halves change only when the gather is done.
    let write = |e: &mut E, values: [u32; HALVES], x: u8, whole: bool| {
        e.sink().local_get(values[0]);
        e.set_vector(x);
        if whole || wide_destination {
            e.sink().local_get(values[1]);
            e.set_vector(ymm_high(x));
        }
    };
    // First each mask element becomes all ones or zero by its sign, which a fault at the first element
    // leaves (seen on the reference).
    for &half in &mask {
        let mut s = e.sink();
        s.local_get(half);
        if element == size_of::<u32>() as u32 {
            s.i32_const(u32::BITS as i32 - 1).i32x4_shr_s();
        } else {
            s.i32_const(u64::BITS as i32 - 1).i64x2_shr_s();
        }
        s.local_set(half);
    }
    write(e, mask, shape.mask, false);
    for n in 0..shape.count {
        let (h, lane) = place(n, element);
        let (index_half, index_lane) = place(n, index_bytes);
        let at = e.local(ValType::I64);
        let mut s = e.sink();
        s.local_get(mask[h]);
        if element == size_of::<u32>() as u32 {
            s.i32x4_extract_lane(lane as u8).i32_const(0).i32_lt_s();
        } else {
            s.i64x2_extract_lane(lane as u8).i64_const(0).i64_lt_s();
        }
        s.if_(BlockType::Empty)
            .local_get(start)
            .local_get(lanes[index_half]);
        if index_bytes == size_of::<u32>() as u32 {
            s.i32x4_extract_lane(index_lane as u8).i64_extend_i32_s();
        } else {
            s.i64x2_extract_lane(index_lane as u8);
        }
        if shift > 0 {
            s.i64_const(shift.into()).i64_shl();
        }
        s.i64_add();
        if i.address32 {
            s.i64_const(u32::MAX.into()).i64_and();
        }
        if let Some(base) = segment(i) {
            e.get(base);
            e.sink().i64_add();
        }
        e.sink().local_set(at);
        e.check(at, element, Access::Read, 0);
        let mut s = e.sink();
        s.local_get(at).local_get(result[h]);
        if element == size_of::<u32>() as u32 {
            s.v128_load32_lane(memory(), lane as u8)
                .local_set(result[h])
                .local_get(mask[h])
                .i32_const(0)
                .i32x4_replace_lane(lane as u8);
        } else {
            s.v128_load64_lane(memory(), lane as u8)
                .local_set(result[h])
                .local_get(mask[h])
                .i64_const(0)
                .i64x2_replace_lane(lane as u8);
        }
        s.local_set(mask[h]);
        // The element is done: a fault at a later one leaves it gathered and its mask element clear.
        write(e, result, destination, false);
        write(e, mask, shape.mask, false);
        e.sink().end();
    }
    // When all are done the mask is zero, and past the elements the destination and the upper halves of a
    // 128-bit form are too.
    let zero = e.local(ValType::V128);
    e.sink().v128_const(0).local_set(zero);
    write(e, [zero, zero], shape.mask, true);
    let done = shape.count * element;
    if done < XMM_BYTES {
        e.sink()
            .local_get(result[0])
            .v128_const((u128::MAX >> (u128::BITS - done * u8::BITS)) as i128)
            .v128_and()
            .local_set(result[0]);
    }
    if !wide_destination {
        e.sink().v128_const(0).local_set(result[1]);
    }
    write(e, result, destination, true);
}
