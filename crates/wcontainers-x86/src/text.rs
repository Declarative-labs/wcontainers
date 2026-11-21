//! SSE4.2's string compares (`pcmpestri`, `pcmpestrm`, `pcmpistri`, `pcmpistrm`), specialised on their
//! immediate at translate time into WebAssembly vector compares of every element pair.

use crate::X86_64;
use crate::XMM_BYTES;
use crate::decode::Instruction;
use crate::flags::{Flag, Flags};
use crate::operand::*;
use iced_x86::{Mnemonic, OpKind};
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_guest_cpu::Emitter;

/// The immediate's fields (Intel's manual, volume 2, 4.1).
const WORDS: u8 = crate::STRING_COMPARE_WORDS;
const SIGNED: u8 = 1 << 1;
const AGGREGATION_SHIFT: u8 = 2;
const AGGREGATION_BITS: u8 = 0b11;
const EQUAL_ANY: u8 = 0;
const RANGES: u8 = 1;
const EQUAL_EACH: u8 = 2;
const NEGATED: u8 = 1 << 4;
const MASKED: u8 = 1 << 5;
/// The most significant index, or a mask of whole elements.
const HIGH_OR_ELEMENTS: u8 = 1 << 6;

pub fn written(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Pcmpestri | Pcmpestri64 | Pcmpestrm | Pcmpestrm64 | Pcmpistri | Pcmpistrm
    )
}

/// The element layout: words or bytes.
#[derive(Clone, Copy)]
struct Layout {
    words: bool,
}

impl Layout {
    fn bytes(self) -> u8 {
        if self.words {
            size_of::<u16>() as u8
        } else {
            size_of::<u8>() as u8
        }
    }

    fn count(self) -> u8 {
        XMM_BYTES as u8 / self.bytes()
    }

    /// The byte shuffle whose element `j` is element `pick(j)` of the first vector (or of the second for
    /// `pick(j) >= count`).
    fn shuffle(self, pick: impl Fn(u8) -> u8) -> [u8; XMM_BYTES as usize] {
        let size = self.bytes();
        core::array::from_fn(|b| pick(b as u8 / size) * size + b as u8 % size)
    }

    /// Each element's index.
    fn indexes(self) -> i128 {
        i128::from_le_bytes(core::array::from_fn(|b| {
            let element = b as u8 / self.bytes();
            if (b as u8).is_multiple_of(self.bytes()) {
                element
            } else {
                0
            }
        }))
    }

    fn splat(self, s: &mut InstructionSink<'_>) {
        if self.words {
            s.i16x8_splat();
        } else {
            s.i8x16_splat();
        }
    }

    fn equal(self, s: &mut InstructionSink<'_>) {
        if self.words {
            s.i16x8_eq();
        } else {
            s.i8x16_eq();
        }
    }

    fn below(self, s: &mut InstructionSink<'_>) {
        if self.words {
            s.i16x8_lt_u();
        } else {
            s.i8x16_lt_u();
        }
    }

    fn at_least(self, s: &mut InstructionSink<'_>, signed: bool) {
        match (self.words, signed) {
            (true, true) => s.i16x8_ge_s(),
            (true, false) => s.i16x8_ge_u(),
            (false, true) => s.i8x16_ge_s(),
            (false, false) => s.i8x16_ge_u(),
        };
    }

    fn at_most(self, s: &mut InstructionSink<'_>, signed: bool) {
        match (self.words, signed) {
            (true, true) => s.i16x8_le_s(),
            (true, false) => s.i16x8_le_u(),
            (false, true) => s.i8x16_le_s(),
            (false, false) => s.i8x16_le_u(),
        };
    }

    fn bitmask(self, s: &mut InstructionSink<'_>) {
        if self.words {
            s.i16x8_bitmask();
        } else {
            s.i8x16_bitmask();
        }
    }

    /// Element `n` of the vector on the stack in every element.
    fn broadcast(self, s: &mut InstructionSink<'_>, vector: u32, n: u8) {
        s.local_get(vector)
            .local_get(vector)
            .i8x16_shuffle(self.shuffle(|_| n));
    }
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    let k = ins.immediate8();
    let layout = Layout {
        words: k & WORDS != 0,
    };
    let signed = k & SIGNED != 0;
    let count = layout.count();
    let explicit = matches!(m, Pcmpestri | Pcmpestri64 | Pcmpestrm | Pcmpestrm64);
    // The 16 bytes are read whole and need no alignment.
    let mem = memory_use(i).map(|kind| {
        let at = address(e, i);
        e.check(at, XMM_BYTES, check_kind(kind), 0);
        at
    });
    let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
    e.get_vector(xmm(ins.op_register(0)).expect("an xmm register"));
    e.sink().local_set(a);
    match (ins.op_kind(1), mem) {
        (OpKind::Register, _) => e.get_vector(xmm(ins.op_register(1)).expect("an xmm register")),
        (_, at) => {
            e.sink().local_get(at.expect("checked")).v128_load(memory());
        }
    }
    e.sink().local_set(b);

    // Each operand's length, at most `count`: from rax and rdx, or up to its first zero element.
    let lengths = [(a, RAX), (b, RDX)].map(|(vector, register)| {
        let length = e.local(ValType::I32);
        if explicit {
            let wide = matches!(m, Pcmpestri64 | Pcmpestrm64);
            let value = e.local(ValType::I64);
            e.get(register);
            let mut s = e.sink();
            if !wide {
                s.i32_wrap_i64().i64_extend_i32_s();
            }
            // The absolute value, read unsigned, so that the most negative is past every count.
            s.local_set(value)
                .i64_const(0)
                .local_get(value)
                .i64_sub()
                .local_get(value)
                .local_get(value)
                .i64_const(0)
                .i64_lt_s()
                .select()
                .local_tee(value)
                .i32_wrap_i64()
                .i32_const(count.into())
                .local_get(value)
                .i64_const(count.into())
                .i64_le_u()
                .select()
                .local_set(length);
        } else {
            let mut s = e.sink();
            s.local_get(vector).v128_const(0);
            layout.equal(&mut s);
            layout.bitmask(&mut s);
            s.i32_const(1 << count).i32_or().i32_ctz().local_set(length);
        }
        length
    });
    // Each operand's valid elements, all ones.
    let [valid_a, valid_b] = lengths.map(|length| {
        let valid = e.local(ValType::V128);
        let mut s = e.sink();
        s.v128_const(layout.indexes()).local_get(length);
        layout.splat(&mut s);
        layout.below(&mut s);
        s.local_set(valid);
        valid
    });

    let (result, bits) = (e.local(ValType::V128), e.local(ValType::I32));
    let mut s = e.sink();
    match k >> AGGREGATION_SHIFT & AGGREGATION_BITS {
        // Each element of b equal to any valid element of a.
        EQUAL_ANY => {
            s.v128_const(0);
            for n in 0..count {
                s.local_get(b);
                layout.broadcast(&mut s, a, n);
                layout.equal(&mut s);
                layout.broadcast(&mut s, valid_a, n);
                s.v128_and().v128_or();
            }
            s.local_get(valid_b).v128_and();
        }
        // Each element of b within any valid pair of a, the low bound first.
        RANGES => {
            s.v128_const(0);
            for n in (0..count).step_by(2) {
                s.local_get(b);
                layout.broadcast(&mut s, a, n);
                layout.at_least(&mut s, signed);
                s.local_get(b);
                layout.broadcast(&mut s, a, n + 1);
                layout.at_most(&mut s, signed);
                s.v128_and();
                layout.broadcast(&mut s, valid_a, n + 1);
                s.v128_and().v128_or();
            }
            s.local_get(valid_b).v128_and();
        }
        // Equal elements where both are valid; true where neither is.
        EQUAL_EACH => {
            s.local_get(a).local_get(b);
            layout.equal(&mut s);
            s.local_get(valid_a)
                .v128_and()
                .local_get(valid_b)
                .v128_and()
                .local_get(valid_a)
                .local_get(valid_b)
                .v128_or()
                .v128_not()
                .v128_or();
        }
        // Element j: a's valid elements found in b from j on, those past b's end counting as found.
        _ => {
            s.v128_const(-1);
            for n in 0..count {
                let from = |j: u8| if j + n < count { j + n } else { count };
                s.local_get(b)
                    .v128_const(0)
                    .i8x16_shuffle(layout.shuffle(from));
                layout.broadcast(&mut s, a, n);
                layout.equal(&mut s);
                s.local_get(valid_b)
                    .v128_const(0)
                    .i8x16_shuffle(layout.shuffle(from))
                    .v128_and();
                layout.broadcast(&mut s, valid_a, n);
                s.v128_not().v128_or();
                // The elements whose comparisons stop before this one.
                s.v128_const(-1)
                    .v128_const(0)
                    .i8x16_shuffle(layout.shuffle(|j| if j + n < count { count } else { j }))
                    .v128_or()
                    .v128_and();
            }
        }
    }
    if k & NEGATED != 0 {
        if k & MASKED != 0 {
            s.local_get(valid_b);
        } else {
            s.v128_const(-1);
        }
        s.v128_xor();
    }
    s.local_tee(result);
    layout.bitmask(&mut s);
    s.local_set(bits);

    // CF: any result; ZF and SF: b and a shorter than a register; OF: the first element's result.
    let each = Flag::ALL.map(|_| e.local(ValType::I32));
    let mut s = e.sink();
    for local in each {
        s.i32_const(0).local_set(local);
    }
    s.local_get(bits)
        .i32_const(0)
        .i32_ne()
        .local_set(each[Flag::C.index()]);
    for (flag, length) in [(Flag::S, lengths[0]), (Flag::Z, lengths[1])] {
        s.local_get(length)
            .i32_const(count.into())
            .i32_lt_u()
            .local_set(each[flag.index()]);
    }
    s.local_get(bits)
        .i32_const(1)
        .i32_and()
        .local_set(each[Flag::O.index()]);
    e.set_flags(Flags::Each(each));

    let high_or_elements = k & HIGH_OR_ELEMENTS != 0;
    if matches!(m, Pcmpestri | Pcmpestri64 | Pcmpistri) {
        // ecx: the least or most significant result's index, or the count when there is none.
        let index = e.local(ValType::I64);
        let mut s = e.sink();
        s.i32_const(count.into());
        if high_or_elements {
            s.i32_const((u32::BITS - 1) as i32)
                .local_get(bits)
                .i32_clz()
                .i32_sub();
        } else {
            s.local_get(bits).i32_ctz();
        }
        s.local_get(bits)
            .i32_eqz()
            .select()
            .i64_extend_i32_u()
            .local_set(index);
        set_general(
            e,
            General {
                number: RCX,
                width: W32,
                high: false,
            },
            index,
        );
    } else {
        // xmm0: the result as whole elements, or as bits.
        let mut s = e.sink();
        if high_or_elements {
            s.local_get(result);
        } else {
            s.v128_const(0)
                .local_get(bits)
                .i64_extend_i32_u()
                .i64x2_replace_lane(0);
        }
        e.set_vector(0);
    }
}
