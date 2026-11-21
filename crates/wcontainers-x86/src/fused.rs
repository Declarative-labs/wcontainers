//! FMA's fused multiply-adds (Intel SDM Vol. 2, VFMADD132PS and its kin): a product and an addend rounded once.
//! The fast path is WebAssembly's relaxed multiply-add where the host's probe sees it fuse, in MXCSR's default
//! mode; everything else goes to the float hook (`crate::exact`), counted "software fused multiply-add" where
//! only the host keeps it from the fast path.

use crate::decode::Instruction;
use crate::exact::{
    FUSED_OPERANDS, Fused, FusedOrder, Operation, Part, encode, fused_immediate, fused_operands,
    fused_signs,
};
use crate::float::{Lanes, chosen_nans, exact};
use crate::operand::*;
use crate::wide::{HALVES, halves, put};
use crate::{X86_64, XMM_BYTES};
use iced_x86::Mnemonic;
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::Emitter;

/// The kind, the order, whether double and whether scalar of each FMA mnemonic.
fn form(m: Mnemonic) -> Option<(Fused, FusedOrder, bool, bool)> {
    use FusedOrder::*;
    use Mnemonic::*;
    let (kind, order, element) = match m {
        Vfmadd132ps => (Fused::Add, Order132, Element::Packed32),
        Vfmadd132pd => (Fused::Add, Order132, Element::Packed64),
        Vfmadd132ss => (Fused::Add, Order132, Element::Scalar32),
        Vfmadd132sd => (Fused::Add, Order132, Element::Scalar64),
        Vfmadd213ps => (Fused::Add, Order213, Element::Packed32),
        Vfmadd213pd => (Fused::Add, Order213, Element::Packed64),
        Vfmadd213ss => (Fused::Add, Order213, Element::Scalar32),
        Vfmadd213sd => (Fused::Add, Order213, Element::Scalar64),
        Vfmadd231ps => (Fused::Add, Order231, Element::Packed32),
        Vfmadd231pd => (Fused::Add, Order231, Element::Packed64),
        Vfmadd231ss => (Fused::Add, Order231, Element::Scalar32),
        Vfmadd231sd => (Fused::Add, Order231, Element::Scalar64),
        Vfmsub132ps => (Fused::Subtract, Order132, Element::Packed32),
        Vfmsub132pd => (Fused::Subtract, Order132, Element::Packed64),
        Vfmsub132ss => (Fused::Subtract, Order132, Element::Scalar32),
        Vfmsub132sd => (Fused::Subtract, Order132, Element::Scalar64),
        Vfmsub213ps => (Fused::Subtract, Order213, Element::Packed32),
        Vfmsub213pd => (Fused::Subtract, Order213, Element::Packed64),
        Vfmsub213ss => (Fused::Subtract, Order213, Element::Scalar32),
        Vfmsub213sd => (Fused::Subtract, Order213, Element::Scalar64),
        Vfmsub231ps => (Fused::Subtract, Order231, Element::Packed32),
        Vfmsub231pd => (Fused::Subtract, Order231, Element::Packed64),
        Vfmsub231ss => (Fused::Subtract, Order231, Element::Scalar32),
        Vfmsub231sd => (Fused::Subtract, Order231, Element::Scalar64),
        Vfnmadd132ps => (Fused::NegatedAdd, Order132, Element::Packed32),
        Vfnmadd132pd => (Fused::NegatedAdd, Order132, Element::Packed64),
        Vfnmadd132ss => (Fused::NegatedAdd, Order132, Element::Scalar32),
        Vfnmadd132sd => (Fused::NegatedAdd, Order132, Element::Scalar64),
        Vfnmadd213ps => (Fused::NegatedAdd, Order213, Element::Packed32),
        Vfnmadd213pd => (Fused::NegatedAdd, Order213, Element::Packed64),
        Vfnmadd213ss => (Fused::NegatedAdd, Order213, Element::Scalar32),
        Vfnmadd213sd => (Fused::NegatedAdd, Order213, Element::Scalar64),
        Vfnmadd231ps => (Fused::NegatedAdd, Order231, Element::Packed32),
        Vfnmadd231pd => (Fused::NegatedAdd, Order231, Element::Packed64),
        Vfnmadd231ss => (Fused::NegatedAdd, Order231, Element::Scalar32),
        Vfnmadd231sd => (Fused::NegatedAdd, Order231, Element::Scalar64),
        Vfnmsub132ps => (Fused::NegatedSubtract, Order132, Element::Packed32),
        Vfnmsub132pd => (Fused::NegatedSubtract, Order132, Element::Packed64),
        Vfnmsub132ss => (Fused::NegatedSubtract, Order132, Element::Scalar32),
        Vfnmsub132sd => (Fused::NegatedSubtract, Order132, Element::Scalar64),
        Vfnmsub213ps => (Fused::NegatedSubtract, Order213, Element::Packed32),
        Vfnmsub213pd => (Fused::NegatedSubtract, Order213, Element::Packed64),
        Vfnmsub213ss => (Fused::NegatedSubtract, Order213, Element::Scalar32),
        Vfnmsub213sd => (Fused::NegatedSubtract, Order213, Element::Scalar64),
        Vfnmsub231ps => (Fused::NegatedSubtract, Order231, Element::Packed32),
        Vfnmsub231pd => (Fused::NegatedSubtract, Order231, Element::Packed64),
        Vfnmsub231ss => (Fused::NegatedSubtract, Order231, Element::Scalar32),
        Vfnmsub231sd => (Fused::NegatedSubtract, Order231, Element::Scalar64),
        Vfmaddsub132ps => (Fused::SubtractAdd, Order132, Element::Packed32),
        Vfmaddsub132pd => (Fused::SubtractAdd, Order132, Element::Packed64),
        Vfmaddsub213ps => (Fused::SubtractAdd, Order213, Element::Packed32),
        Vfmaddsub213pd => (Fused::SubtractAdd, Order213, Element::Packed64),
        Vfmaddsub231ps => (Fused::SubtractAdd, Order231, Element::Packed32),
        Vfmaddsub231pd => (Fused::SubtractAdd, Order231, Element::Packed64),
        Vfmsubadd132ps => (Fused::AddSubtract, Order132, Element::Packed32),
        Vfmsubadd132pd => (Fused::AddSubtract, Order132, Element::Packed64),
        Vfmsubadd213ps => (Fused::AddSubtract, Order213, Element::Packed32),
        Vfmsubadd213pd => (Fused::AddSubtract, Order213, Element::Packed64),
        Vfmsubadd231ps => (Fused::AddSubtract, Order231, Element::Packed32),
        Vfmsubadd231pd => (Fused::AddSubtract, Order231, Element::Packed64),
        _ => return None,
    };
    let (double, scalar) = match element {
        Element::Packed32 => (false, false),
        Element::Packed64 => (true, false),
        Element::Scalar32 => (false, true),
        Element::Scalar64 => (true, true),
    };
    Some((kind, order, double, scalar))
}

/// The elements a form works on.
enum Element {
    Packed32,
    Packed64,
    Scalar32,
    Scalar64,
}

pub fn written(i: &Instruction) -> bool {
    // The EVEX forms of these mnemonics are AVX-512's, of features never told.
    crate::decode::claim(i.iced.code()) != crate::claims::CLAIM_NEVER
        && form(i.iced.mnemonic()).is_some()
}

/// A vector with the sign bit of each element `negated` names, of `bits` bits.
fn signs(bits: u32, negated: impl Fn(usize) -> bool) -> i128 {
    (0..(u128::BITS / bits) as usize)
        .filter(|&n| negated(n))
        .fold(0u128, |v, n| v | 1 << (n as u32 * bits + bits - 1)) as i128
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let (kind, order, double, scalar) = form(m).expect("an FMA instruction");
    let at = memory_use(i).map(|access| {
        let at = address(e, i);
        e.check(at, memory_bytes(i), check_kind(access), 0);
        at
    });
    let operands: [[u32; HALVES]; FUSED_OPERANDS] =
        core::array::from_fn(|n| halves(e, i, n as u32, at));
    let wide = memory_bytes(i) == 2 * XMM_BYTES || ymm(i.iced.op_register(0)).is_some();
    let count = if wide { HALVES } else { 1 };
    let lanes = Lanes { double };
    let bits = if double { u64::BITS } else { u32::BITS };
    let mut results = [None; HALVES];
    for (h, result) in results.iter_mut().enumerate().take(count) {
        let [a, b, c] = operands.map(|o| o[h]);
        let r = e.local(ValType::V128);
        let part = match (wide, h) {
            (false, _) => Part::Whole,
            (true, 0) => Part::LowLane,
            (true, _) => Part::HighLane,
        };
        let code = encode(
            Operation::Fused,
            double,
            scalar,
            fused_immediate(kind, order),
            part,
        );
        let fast = |e: &mut E| {
            let [x, y, z] = fused_operands(order, [a, b, c]);
            let (product, addend) = (
                signs(bits, |n| fused_signs(kind, n).0),
                signs(bits, |n| fused_signs(kind, n).1),
            );
            let mut s = e.sink();
            s.local_get(x);
            if product != 0 {
                s.v128_const(product).v128_xor();
            }
            s.local_get(y).local_get(z);
            if addend != 0 {
                s.v128_const(addend).v128_xor();
            }
            if double {
                s.f64x2_relaxed_madd();
            } else {
                s.f32x4_relaxed_madd();
            }
            s.local_set(r);
            // The probe tries no multiply-add's not-a-numbers, and the operands were negated.
            chosen_nans(e, &lanes, r, &[x, y, z]);
            if scalar {
                let mut s = e.sink();
                lanes.merge_low(&mut s, a, r);
                s.local_set(r);
            }
        };
        if !e.host().fused_multiply_add {
            exact(e, code, [a, b], Some(c), r);
        } else if e.float_mode() {
            e.sink().if_(BlockType::Empty);
            exact(e, code, [a, b], Some(c), r);
            e.sink().else_();
            fast(e);
            e.sink().end();
        } else {
            fast(e);
        }
        *result = Some(r);
    }
    let high = results[1].unwrap_or_else(|| {
        let zero = e.local(ValType::V128);
        e.sink().v128_const(0).local_set(zero);
        zero
    });
    put(e, i, None, [results[0].expect("the low half"), high]);
}
