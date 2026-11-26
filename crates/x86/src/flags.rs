//! The six arithmetic flags, remembered lazily: a flag-setting instruction keeps its operands in locals and
//! each flag is worked out from them only when read. Every shape gives all six, so a flag the instruction
//! leaves alone is kept in a local of its own (`inc` and `dec` keep CF).

pub use crate::condition::{Condition, Test};
use crate::{AF, CF, OF, PF, SF, X86_64, ZF};
use wasm_encoder::InstructionSink;
use wcontainers_guest_cpu::Emitter;

/// One of the six flags, as `Guest::FLAGS` lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    C,
    P,
    A,
    Z,
    S,
    O,
}

impl Flag {
    pub const ALL: [Flag; 6] = [Flag::C, Flag::P, Flag::A, Flag::Z, Flag::S, Flag::O];

    /// Its bit in rflags.
    pub const fn bit(self) -> u32 {
        match self {
            Flag::C => CF,
            Flag::P => PF,
            Flag::A => AF,
            Flag::Z => ZF,
            Flag::S => SF,
            Flag::O => OF,
        }
        .trailing_zeros()
    }

    /// Its place in `ALL`.
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The flags a shift leaves as they were when its masked count is zero, besides CF and OF.
pub const KEPT_BY_SHIFT: [Flag; 4] = [Flag::P, Flag::A, Flag::Z, Flag::S];

/// What a shift needs when its masked count may be zero: the count (i64) and the flags of
/// `KEPT_BY_SHIFT` as they were (i32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unmoved {
    pub count: u32,
    pub kept: [u32; KEPT_BY_SHIFT.len()],
}

/// How the flags were last set. Operand and result locals are i64 holding the value zero-extended from
/// `width` bits (8, 16, 32 or 64); a carry or a kept flag is an i32 local holding 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flags {
    /// `result = a + b + carry`; CF from `kept_carry` when the instruction keeps it (`inc`).
    Add {
        width: u8,
        a: u32,
        b: u32,
        carry: Option<u32>,
        result: u32,
        kept_carry: Option<u32>,
    },
    /// `result = a - b - borrow`; CF is the borrow, from `kept_carry` when the instruction keeps it (`dec`).
    Sub {
        width: u8,
        a: u32,
        b: u32,
        borrow: Option<u32>,
        result: u32,
        kept_carry: Option<u32>,
    },
    /// The logic instructions: CF and OF clear, AF clear as the reference CPU leaves it.
    Logic { width: u8, result: u32 },
    /// A multiplication: CF and OF from `overflow` (i32); as the reference CPU leaves the others, SF and PF
    /// from the low half of the result, ZF and AF clear.
    Multiply {
        width: u8,
        result: u32,
        overflow: u32,
    },
    /// shl, sal, shr and sar: CF and OF from i32 locals (already as they were for a count of zero), PF, ZF
    /// and SF from the result and AF clear, as the reference CPU leaves it; with `unmoved`, those four as
    /// they were when the count is zero.
    Shift {
        width: u8,
        result: u32,
        carry: u32,
        overflow: u32,
        unmoved: Option<Unmoved>,
    },
    /// Each flag in an i32 local of its own, in `Flag::ALL` order: what rotates, bt and its kin, clc, stc
    /// and cmc leave, CF (and OF) new and the rest as they were.
    Each([u32; Flag::ALL.len()]),
    /// An i32 local holding the flags at their rflags positions.
    Packed(u32),
}

/// Push the i32 bit `bit` of the i64 on the stack.
fn bit_of(s: &mut InstructionSink<'_>, bit: u8) {
    if bit > 0 {
        s.i64_const(bit.into()).i64_shr_u();
    }
    s.i32_wrap_i64().i32_const(1).i32_and();
}

/// PF counts the low byte of the result.
pub(crate) fn parity(s: &mut InstructionSink<'_>, result: u32) {
    s.local_get(result)
        .i32_wrap_i64()
        .i32_const(u8::MAX.into())
        .i32_and()
        .i32_popcnt()
        .i32_const(1)
        .i32_and()
        .i32_const(1)
        .i32_xor();
}

/// AF is the carry out of the low nibble.
fn adjust(s: &mut InstructionSink<'_>, a: u32, b: u32, result: u32) {
    const NIBBLE: u8 = (u8::BITS / 2) as u8;
    s.local_get(a)
        .local_get(b)
        .i64_xor()
        .local_get(result)
        .i64_xor();
    bit_of(s, NIBBLE);
}

fn zero(s: &mut InstructionSink<'_>, result: u32) {
    s.local_get(result).i64_eqz();
}

fn sign(s: &mut InstructionSink<'_>, width: u8, result: u32) {
    s.local_get(result);
    bit_of(s, width - 1);
}

/// Push one flag as an i32 0 or 1.
pub fn flag_value(s: &mut InstructionSink<'_>, flags: Flags, flag: Flag) {
    match flags {
        Flags::Add {
            width,
            a,
            b,
            carry,
            result,
            kept_carry,
        } => match flag {
            Flag::C => match kept_carry {
                Some(c) => {
                    s.local_get(c);
                }
                None => {
                    // The sum wrapped: it is below `a`, or equal to it with a carry in and `b` all ones.
                    s.local_get(result).local_get(a).i64_lt_u();
                    if let Some(c) = carry {
                        s.local_get(result)
                            .local_get(a)
                            .i64_eq()
                            .local_get(c)
                            .i32_and()
                            .i32_or();
                    }
                }
            },
            Flag::P => parity(s, result),
            Flag::A => adjust(s, a, b, result),
            Flag::Z => zero(s, result),
            Flag::S => sign(s, width, result),
            Flag::O => {
                s.local_get(a)
                    .local_get(result)
                    .i64_xor()
                    .local_get(b)
                    .local_get(result)
                    .i64_xor()
                    .i64_and();
                bit_of(s, width - 1);
            }
        },
        Flags::Sub {
            width,
            a,
            b,
            borrow,
            result,
            kept_carry,
        } => match flag {
            Flag::C => match kept_carry {
                Some(c) => {
                    s.local_get(c);
                }
                None => {
                    s.local_get(a).local_get(b).i64_lt_u();
                    if let Some(c) = borrow {
                        s.local_get(a)
                            .local_get(b)
                            .i64_eq()
                            .local_get(c)
                            .i32_and()
                            .i32_or();
                    }
                }
            },
            Flag::P => parity(s, result),
            Flag::A => adjust(s, a, b, result),
            Flag::Z => zero(s, result),
            Flag::S => sign(s, width, result),
            Flag::O => {
                s.local_get(a)
                    .local_get(b)
                    .i64_xor()
                    .local_get(a)
                    .local_get(result)
                    .i64_xor()
                    .i64_and();
                bit_of(s, width - 1);
            }
        },
        Flags::Logic { width, result } => match flag {
            Flag::C | Flag::A | Flag::O => {
                s.i32_const(0);
            }
            Flag::P => parity(s, result),
            Flag::Z => zero(s, result),
            Flag::S => sign(s, width, result),
        },
        Flags::Multiply {
            width,
            result,
            overflow,
        } => match flag {
            Flag::C | Flag::O => {
                s.local_get(overflow);
            }
            Flag::A | Flag::Z => {
                s.i32_const(0);
            }
            Flag::P => parity(s, result),
            Flag::S => sign(s, width, result),
        },
        Flags::Shift {
            width,
            result,
            carry,
            overflow,
            unmoved,
        } => {
            let new = |s: &mut InstructionSink<'_>| match flag {
                Flag::C => {
                    s.local_get(carry);
                }
                Flag::O => {
                    s.local_get(overflow);
                }
                Flag::A => {
                    s.i32_const(0);
                }
                Flag::P => parity(s, result),
                Flag::Z => zero(s, result),
                Flag::S => sign(s, width, result),
            };
            match (unmoved, KEPT_BY_SHIFT.iter().position(|&f| f == flag)) {
                (Some(Unmoved { count, kept }), Some(k)) => {
                    s.local_get(kept[k]);
                    new(s);
                    s.local_get(count).i64_eqz().select();
                }
                _ => new(s),
            }
        }
        Flags::Each(each) => {
            s.local_get(each[flag.index()]);
        }
        Flags::Packed(word) => {
            s.local_get(word);
            if flag.bit() > 0 {
                s.i32_const(flag.bit() as i32).i32_shr_u();
            }
            s.i32_const(1).i32_and();
        }
    }
}

/// Push whether `cond` holds, from the flags one by one.
pub fn condition_from_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E, cond: Condition) {
    match cond.test() {
        Test::Overflow => e.flag(Flag::O),
        Test::Below => e.flag(Flag::C),
        Test::Equal => e.flag(Flag::Z),
        Test::BelowOrEqual => {
            e.flag(Flag::C);
            e.flag(Flag::Z);
            e.sink().i32_or();
        }
        Test::Sign => e.flag(Flag::S),
        Test::Parity => e.flag(Flag::P),
        Test::Less => {
            e.flag(Flag::S);
            e.flag(Flag::O);
            e.sink().i32_ne();
        }
        Test::LessOrEqual => {
            e.flag(Flag::Z);
            e.flag(Flag::S);
            e.flag(Flag::O);
            e.sink().i32_ne().i32_or();
        }
    }
    if cond.negated() {
        e.sink().i32_eqz();
    }
}

/// A way of setting the flags, without its locals: the key of a flags-word function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Add { width: u8, carry: bool, kept: bool },
    Sub { width: u8, borrow: bool, kept: bool },
    Logic { width: u8 },
    Multiply { width: u8 },
    Shift { width: u8, unmoved: bool },
}

/// The operand widths, smallest first.
const WIDTHS: [u8; 4] = [
    u8::BITS as u8,
    u16::BITS as u8,
    u32::BITS as u8,
    u64::BITS as u8,
];

/// The shapes of each width, at width 0: the plain sum or difference, with a carry or borrow in (adc, sbb),
/// with CF kept (inc, dec), the logic instructions, the multiplications and the shifts, with a count known
/// not zero or not.
const OF_A_WIDTH: &[Shape] = &[
    Shape::Add {
        width: 0,
        carry: false,
        kept: false,
    },
    Shape::Add {
        width: 0,
        carry: true,
        kept: false,
    },
    Shape::Add {
        width: 0,
        carry: false,
        kept: true,
    },
    Shape::Sub {
        width: 0,
        borrow: false,
        kept: false,
    },
    Shape::Sub {
        width: 0,
        borrow: true,
        kept: false,
    },
    Shape::Sub {
        width: 0,
        borrow: false,
        kept: true,
    },
    Shape::Logic { width: 0 },
    Shape::Multiply { width: 0 },
    Shape::Shift {
        width: 0,
        unmoved: false,
    },
    Shape::Shift {
        width: 0,
        unmoved: true,
    },
];

impl Shape {
    const fn at_width(self, width: u8) -> Shape {
        match self {
            Shape::Add { carry, kept, .. } => Shape::Add { width, carry, kept },
            Shape::Sub { borrow, kept, .. } => Shape::Sub {
                width,
                borrow,
                kept,
            },
            Shape::Logic { .. } => Shape::Logic { width },
            Shape::Multiply { .. } => Shape::Multiply { width },
            Shape::Shift { unmoved, .. } => Shape::Shift { width, unmoved },
        }
    }
}

/// Every shape, in flags-word function order.
pub const SHAPES: [Shape; OF_A_WIDTH.len() * WIDTHS.len()] = {
    let mut shapes = [Shape::Logic { width: 0 }; OF_A_WIDTH.len() * WIDTHS.len()];
    let mut k = 0;
    while k < shapes.len() {
        shapes[k] = OF_A_WIDTH[k % OF_A_WIDTH.len()].at_width(WIDTHS[k / OF_A_WIDTH.len()]);
        k += 1;
    }
    shapes
};

pub fn shape(flags: Flags) -> Option<Shape> {
    match flags {
        Flags::Add {
            width,
            carry,
            kept_carry,
            ..
        } => Some(Shape::Add {
            width,
            carry: carry.is_some(),
            kept: kept_carry.is_some(),
        }),
        Flags::Sub {
            width,
            borrow,
            kept_carry,
            ..
        } => Some(Shape::Sub {
            width,
            borrow: borrow.is_some(),
            kept: kept_carry.is_some(),
        }),
        Flags::Logic { width, .. } => Some(Shape::Logic { width }),
        Flags::Multiply { width, .. } => Some(Shape::Multiply { width }),
        Flags::Shift { width, unmoved, .. } => Some(Shape::Shift {
            width,
            unmoved: unmoved.is_some(),
        }),
        Flags::Each(_) | Flags::Packed(_) => None,
    }
}

use wasm_encoder::ValType;

/// The locals' types: the operands and the result, then a carry or borrow in, then a kept CF; for a shift the
/// result, CF and OF, then the count and the kept flags.
pub fn shape_locals(shape: Shape) -> &'static [ValType] {
    use ValType::{I32, I64};
    match shape {
        Shape::Add {
            carry: false,
            kept: false,
            ..
        }
        | Shape::Sub {
            borrow: false,
            kept: false,
            ..
        } => &[I64, I64, I64],
        Shape::Add { .. } | Shape::Sub { .. } => &[I64, I64, I64, I32],
        Shape::Logic { .. } => &[I64],
        Shape::Multiply { .. } => &[I64, I32],
        Shape::Shift { unmoved: false, .. } => &[I64, I32, I32],
        Shape::Shift { unmoved: true, .. } => &[I64, I32, I32, I64, I32, I32, I32, I32],
    }
}

/// The shape over these locals, in `shape_locals` order.
pub fn with_locals(shape: Shape, locals: &[u32]) -> Flags {
    let short = || -> ! { panic!("{shape:?} over too few locals: {locals:?}") };
    match (shape, locals) {
        (Shape::Add { width, carry, kept }, &[a, b, result, ref extra @ ..]) => Flags::Add {
            width,
            a,
            b,
            result,
            carry: extra.first().copied().filter(|_| carry),
            kept_carry: extra.first().copied().filter(|_| kept),
        },
        (
            Shape::Sub {
                width,
                borrow,
                kept,
            },
            &[a, b, result, ref extra @ ..],
        ) => Flags::Sub {
            width,
            a,
            b,
            result,
            borrow: extra.first().copied().filter(|_| borrow),
            kept_carry: extra.first().copied().filter(|_| kept),
        },
        (Shape::Logic { width }, &[result, ..]) => Flags::Logic { width, result },
        (Shape::Multiply { width }, &[result, overflow, ..]) => Flags::Multiply {
            width,
            result,
            overflow,
        },
        (Shape::Shift { width, unmoved }, &[result, carry, overflow, ref extra @ ..]) => {
            Flags::Shift {
                width,
                result,
                carry,
                overflow,
                unmoved: match extra {
                    &[count, ref kept @ ..] if unmoved => Some(Unmoved {
                        count,
                        kept: kept.try_into().unwrap_or_else(|_| short()),
                    }),
                    _ => None,
                },
            }
        }
        _ => short(),
    }
}

/// Push an i64 local read signed at `width` bits, for a signed comparison.
fn signed(s: &mut InstructionSink<'_>, local: u32, width: u8) {
    s.local_get(local);
    let unused = i64::from(u64::BITS) - i64::from(width);
    if unused > 0 {
        s.i64_const(unused).i64_shl().i64_const(unused).i64_shr_s();
    }
}

/// Push whether `cond` holds as one comparison of the shape's operands, for a plain difference (cmp, sub)
/// and the logic instructions (test, and); false where the flags must be worked out.
pub fn condition_direct(s: &mut InstructionSink<'_>, flags: Flags, cond: Condition) -> bool {
    match flags {
        Flags::Sub {
            width,
            a,
            b,
            result,
            borrow: None,
            kept_carry: None,
        } => match cond.test() {
            Test::Below => {
                s.local_get(a).local_get(b).i64_lt_u();
            }
            Test::Equal => {
                s.local_get(a).local_get(b).i64_eq();
            }
            Test::BelowOrEqual => {
                s.local_get(a).local_get(b).i64_le_u();
            }
            Test::Sign => sign(s, width, result),
            Test::Less => {
                signed(s, a, width);
                signed(s, b, width);
                s.i64_lt_s();
            }
            Test::LessOrEqual => {
                signed(s, a, width);
                signed(s, b, width);
                s.i64_le_s();
            }
            Test::Overflow | Test::Parity => return false,
        },
        // CF and OF clear: below never holds, below or equal is ZF, less is SF, less or equal ZF or SF.
        Flags::Logic { width, result } => match cond.test() {
            Test::Below => {
                s.i32_const(0);
            }
            Test::Equal | Test::BelowOrEqual => zero(s, result),
            Test::Sign | Test::Less => sign(s, width, result),
            Test::LessOrEqual => {
                signed(s, result, width);
                s.i64_const(0).i64_le_s();
            }
            Test::Overflow | Test::Parity => return false,
        },
        _ => return false,
    }
    if cond.negated() {
        s.i32_eqz();
    }
    true
}
