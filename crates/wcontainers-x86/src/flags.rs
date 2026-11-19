//! The six arithmetic flags, remembered lazily: a flag-setting instruction keeps its operands in locals and
//! each flag is worked out from them only when read. Every shape gives all six, so a flag the instruction
//! leaves alone is kept in a local of its own (`inc` and `dec` keep CF).

use crate::X86_64;
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
            Flag::C => 0,
            Flag::P => 2,
            Flag::A => 4,
            Flag::Z => 6,
            Flag::S => 7,
            Flag::O => 11,
        }
    }
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

fn parity(s: &mut InstructionSink<'_>, result: u32) {
    s.local_get(result)
        .i32_wrap_i64()
        .i32_const(0xff)
        .i32_and()
        .i32_popcnt()
        .i32_const(1)
        .i32_and()
        .i32_const(1)
        .i32_xor();
}

fn adjust(s: &mut InstructionSink<'_>, a: u32, b: u32, result: u32) {
    s.local_get(a)
        .local_get(b)
        .i64_xor()
        .local_get(result)
        .i64_xor();
    bit_of(s, 4);
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
        Flags::Packed(word) => {
            s.local_get(word);
            if flag.bit() > 0 {
                s.i32_const(flag.bit() as i32).i32_shr_u();
            }
            s.i32_const(1).i32_and();
        }
    }
}

/// Push whether condition `cond` (the low four bits of a `jcc` opcode) holds, from the flags one by one.
pub fn condition_from_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E, cond: u8) {
    match cond >> 1 {
        0 => e.flag(Flag::O),
        1 => e.flag(Flag::C),
        2 => e.flag(Flag::Z),
        3 => {
            e.flag(Flag::C);
            e.flag(Flag::Z);
            e.sink().i32_or();
        }
        4 => e.flag(Flag::S),
        5 => e.flag(Flag::P),
        6 => {
            e.flag(Flag::S);
            e.flag(Flag::O);
            e.sink().i32_ne();
        }
        _ => {
            e.flag(Flag::Z);
            e.flag(Flag::S);
            e.flag(Flag::O);
            e.sink().i32_ne().i32_or();
        }
    }
    if cond & 1 == 1 {
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
}

/// The operand widths, smallest first.
const WIDTHS: [u8; 4] = [
    u8::BITS as u8,
    u16::BITS as u8,
    u32::BITS as u8,
    u64::BITS as u8,
];

/// Shapes of each width: the plain sum or difference, with a carry or borrow in (adc, sbb), with CF kept (inc,
/// dec), the logic instructions and the multiplications.
const PER_WIDTH: usize = 8;

/// Every shape, in flags-word function order.
pub const SHAPES: [Shape; PER_WIDTH * WIDTHS.len()] = {
    let mut shapes = [Shape::Logic { width: 0 }; PER_WIDTH * WIDTHS.len()];
    let mut w = 0;
    while w < WIDTHS.len() {
        let width = WIDTHS[w];
        let each = [
            Shape::Add {
                width,
                carry: false,
                kept: false,
            },
            Shape::Add {
                width,
                carry: true,
                kept: false,
            },
            Shape::Add {
                width,
                carry: false,
                kept: true,
            },
            Shape::Sub {
                width,
                borrow: false,
                kept: false,
            },
            Shape::Sub {
                width,
                borrow: true,
                kept: false,
            },
            Shape::Sub {
                width,
                borrow: false,
                kept: true,
            },
            Shape::Logic { width },
            Shape::Multiply { width },
        ];
        let mut k = 0;
        while k < PER_WIDTH {
            shapes[PER_WIDTH * w + k] = each[k];
            k += 1;
        }
        w += 1;
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
        Flags::Packed(_) => None,
    }
}

use wasm_encoder::ValType;

/// The locals' types: the operands and the result, then a carry or borrow in, then a kept CF.
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
    }
}

/// The shape over these locals, in `shape_locals` order.
pub fn with_locals(shape: Shape, locals: &[u32]) -> Flags {
    let extra = locals.get(3).copied();
    match shape {
        Shape::Add { width, carry, kept } => Flags::Add {
            width,
            a: locals[0],
            b: locals[1],
            result: locals[2],
            carry: extra.filter(|_| carry),
            kept_carry: extra.filter(|_| kept),
        },
        Shape::Sub {
            width,
            borrow,
            kept,
        } => Flags::Sub {
            width,
            a: locals[0],
            b: locals[1],
            result: locals[2],
            borrow: extra.filter(|_| borrow),
            kept_carry: extra.filter(|_| kept),
        },
        Shape::Logic { width } => Flags::Logic {
            width,
            result: locals[0],
        },
        Shape::Multiply { width } => Flags::Multiply {
            width,
            result: locals[0],
            overflow: locals[1],
        },
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

/// The conditions by the low four bits of `jcc`, in pairs: the even one holds, the odd one is its negation.
pub(crate) const BELOW: u8 = 2;
pub(crate) const EQUAL: u8 = 4;
pub(crate) const BELOW_OR_EQUAL: u8 = 6;
const SIGN: u8 = 8;
pub(crate) const LESS: u8 = 12;
pub(crate) const LESS_OR_EQUAL: u8 = 14;

/// Push whether `cond` holds as one comparison of the shape's operands, for a plain difference (cmp, sub)
/// and the logic instructions (test, and); false where the flags must be worked out.
pub fn condition_direct(s: &mut InstructionSink<'_>, flags: Flags, cond: u8) -> bool {
    let holds = cond & !1;
    match flags {
        Flags::Sub {
            width,
            a,
            b,
            result,
            borrow: None,
            kept_carry: None,
        } => match holds {
            BELOW => {
                s.local_get(a).local_get(b).i64_lt_u();
            }
            EQUAL => {
                s.local_get(a).local_get(b).i64_eq();
            }
            BELOW_OR_EQUAL => {
                s.local_get(a).local_get(b).i64_le_u();
            }
            SIGN => sign(s, width, result),
            LESS => {
                signed(s, a, width);
                signed(s, b, width);
                s.i64_lt_s();
            }
            LESS_OR_EQUAL => {
                signed(s, a, width);
                signed(s, b, width);
                s.i64_le_s();
            }
            _ => return false,
        },
        // CF and OF clear: below never holds, below or equal is ZF, less is SF, less or equal ZF or SF.
        Flags::Logic { width, result } => match holds {
            BELOW => {
                s.i32_const(0);
            }
            EQUAL | BELOW_OR_EQUAL => zero(s, result),
            SIGN | LESS => sign(s, width, result),
            LESS_OR_EQUAL => {
                signed(s, result, width);
                s.i64_const(0).i64_le_s();
            }
            _ => return false,
        },
        _ => return false,
    }
    if cond & 1 == 1 {
        s.i32_eqz();
    }
    true
}
