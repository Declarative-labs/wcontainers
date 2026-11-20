//! SIMD floating point in software, for the exact path: MXCSR's mode is not the default, or its exception
//! flags are kept. It runs on the host behind `Hook::Float`, one call per instruction. The arithmetic is
//! rustc_apfloat's, done in quadruple precision rounded to odd and then rounded once; what is x86's own is
//! here: which not-a-number comes out, denormals are zero, flush to zero, which exceptions each operation
//! raises and in which order (Intel SDM Vol. 1 11.5.2), and that an unmasked one leaves the destination as
//! it was.

use crate::{
    CF, MXCSR_DENORMAL, MXCSR_DENORMALS_ARE_ZERO, MXCSR_DIVIDE_BY_ZERO, MXCSR_FLAGS,
    MXCSR_FLUSH_TO_ZERO, MXCSR_INVALID, MXCSR_MASK_SHIFT, MXCSR_OVERFLOW, MXCSR_PRECISION,
    MXCSR_ROUNDING, MXCSR_ROUNDING_SHIFT, MXCSR_UNDERFLOW, PF, ZF,
};
use rustc_apfloat::ieee::{Double, Quad, Single};
use rustc_apfloat::{Float, FloatConvert, Round, Status, StatusAnd};

/// One instruction's operation. With the element size, whether it is scalar and the immediate, it is the
/// i32 the hook receives (`encode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Operation {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    Sqrt,
    /// `cmpps`: the predicate is in the immediate's low bits.
    Compare,
    /// `comiss`, raising invalid for any not-a-number, and `ucomiss`: ZF, PF and CF at their rflags bits.
    OrderedCompare,
    UnorderedCompare,
    AddSub,
    HorizontalAdd,
    HorizontalSub,
    /// `roundps`: the immediate's rounding, or MXCSR's.
    Round,
    DotProduct,
    /// To the other float size.
    Convert,
    /// From 32-bit or 64-bit signed integers.
    FromInt32,
    FromInt64,
    /// To 32-bit or 64-bit signed integers, rounded as MXCSR says or truncated.
    ToInt32,
    ToInt64,
    TruncateToInt32,
    TruncateToInt64,
}

const OPERATIONS: [Operation; 22] = {
    use Operation::*;
    [
        Add,
        Sub,
        Mul,
        Div,
        Min,
        Max,
        Sqrt,
        Compare,
        OrderedCompare,
        UnorderedCompare,
        AddSub,
        HorizontalAdd,
        HorizontalSub,
        Round,
        DotProduct,
        Convert,
        FromInt32,
        FromInt64,
        ToInt32,
        ToInt64,
        TruncateToInt32,
        TruncateToInt64,
    ]
};

/// The parts of the i32 the hook receives: the operation in the low byte, the two bits below, and the
/// immediate in the third byte.
const DOUBLE: i32 = 1 << u8::BITS;
const SCALAR: i32 = DOUBLE << 1;
const IMMEDIATE_SHIFT: u32 = 2 * u8::BITS;
/// The destination xmm register's number, in the fourth byte: the hook reads its value from the block.
const DESTINATION_SHIFT: u32 = 3 * u8::BITS;

/// The comparison predicates of `cmpps` (Intel SDM Vol. 2 CMPPS, table 3-8), in the order of their numbers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Predicate {
    Equal,
    Less,
    LessOrEqual,
    Unordered,
    NotEqual,
    NotLess,
    NotLessOrEqual,
    Ordered,
}

const PREDICATES: [Predicate; 8] = {
    use Predicate::*;
    [
        Equal,
        Less,
        LessOrEqual,
        Unordered,
        NotEqual,
        NotLess,
        NotLessOrEqual,
        Ordered,
    ]
};

/// MXCSR's rounding control in the order of its values.
const ROUNDINGS: [Round; 4] = [
    Round::NearestTiesToEven,
    Round::TowardNegative,
    Round::TowardPositive,
    Round::TowardZero,
];

/// `roundps`'s immediate: the rounding, the bit that takes MXCSR's instead, the bit that keeps precision
/// quiet.
pub(crate) const ROUND_FROM_MXCSR: u8 = 1 << 2;
const ROUND_QUIET: u8 = 1 << 3;
/// `dpps`'s immediate: which products are summed (high half) and which lanes get the sum (low half).
pub(crate) const DOT_PRODUCT_SHIFT: u32 = u8::BITS / 2;

/// The exceptions found before the computation, which keep it from being done when one is unmasked.
const BEFORE: u32 = MXCSR_INVALID | MXCSR_DENORMAL | MXCSR_DIVIDE_BY_ZERO;

pub fn encode(
    operation: Operation,
    double: bool,
    scalar: bool,
    immediate: u8,
    destination: u8,
) -> i32 {
    operation as i32
        | if double { DOUBLE } else { 0 }
        | if scalar { SCALAR } else { 0 }
        | i32::from(immediate) << IMMEDIATE_SHIFT
        | i32::from(destination) << DESTINATION_SHIFT
}

/// The destination register's number in the hook's i32.
pub fn destination(code: i32) -> u8 {
    (code >> DESTINATION_SHIFT) as u8
}

/// Run one instruction on `a` (the destination's value) and `b` (the source's) with MXCSR as it is, adding
/// the flags raised. The result; or, for an unmasked exception, `a` unchanged and MXCSR's raised unmasked
/// flags, from which the kernel works out the `si_code` as Linux does (0 for none).
pub fn run(code: i32, a: u128, b: u128, mxcsr: &mut u32) -> (u128, u32) {
    let operation = OPERATIONS[(code & i32::from(u8::MAX)) as usize];
    let immediate = (code >> IMMEDIATE_SHIFT) as u8;
    let scalar = code & SCALAR != 0;
    let mut x = Context {
        mxcsr: *mxcsr,
        flags: 0,
    };
    let result = if code & DOUBLE != 0 {
        x.instruction::<Double, Single>(operation, scalar, immediate, a, b)
    } else {
        x.instruction::<Single, Double>(operation, scalar, immediate, a, b)
    };
    let masks = *mxcsr >> MXCSR_MASK_SHIFT;
    let mut flags = x.flags;
    if flags & BEFORE & !masks != 0 {
        flags &= BEFORE;
    }
    *mxcsr |= flags;
    if flags & !masks & MXCSR_FLAGS != 0 {
        (a, crate::mxcsr_raised(*mxcsr))
    } else {
        (result, 0)
    }
}

/// The raw bits of a format's parts.
trait Bits: Float + FloatConvert<Quad> {
    fn mask() -> u128 {
        u128::MAX >> (u128::BITS - Self::BITS as u32)
    }
    fn quiet_bit() -> u128 {
        1 << (Self::PRECISION - 2)
    }
    fn fraction_mask() -> u128 {
        (1 << (Self::PRECISION - 1)) - 1
    }
    fn sign_bit() -> u128 {
        1 << (Self::BITS - 1)
    }
    /// x86's default not-a-number: negative and quiet ("real indefinite").
    fn default_nan() -> Self {
        Self::from_bits(Self::mask() & !(Self::fraction_mask() >> 1))
    }
    fn quiet(self) -> Self {
        Self::from_bits(self.to_bits() | Self::quiet_bit())
    }
    fn signed_zero(negative: bool) -> Self {
        if negative { -Self::ZERO } else { Self::ZERO }
    }
}

impl Bits for Single {}
impl Bits for Double {}

/// A lane of `v`, of `F`'s width.
fn lane<F: Bits>(v: u128, n: usize) -> u128 {
    (v >> (n * F::BITS)) & F::mask()
}

fn with_lane<F: Bits>(v: u128, n: usize, bits: u128) -> u128 {
    let at = n * F::BITS;
    v & !(F::mask() << at) | (bits & F::mask()) << at
}

/// A not-a-number of one format as the other, quiet, with as much of its payload as fits.
fn convert_nan<F: Bits, T: Bits>(v: F) -> T {
    let bits = v.quiet().to_bits() & F::fraction_mask();
    let fraction = if T::PRECISION > F::PRECISION {
        bits << (T::PRECISION - F::PRECISION)
    } else {
        bits >> (F::PRECISION - T::PRECISION)
    };
    let sign = if v.is_negative() { T::sign_bit() } else { 0 };
    T::from_bits(sign | (T::mask() >> 1 & !T::fraction_mask()) | fraction)
}

struct Context {
    mxcsr: u32,
    flags: u32,
}

impl Context {
    fn round(&self) -> Round {
        ROUNDINGS[((self.mxcsr & MXCSR_ROUNDING) >> MXCSR_ROUNDING_SHIFT) as usize]
    }

    fn masked(&self, flag: u32) -> bool {
        self.mxcsr >> MXCSR_MASK_SHIFT & flag != 0
    }

    /// An operand of a number: zero when it is denormal and DAZ is set, else raising denormal if `denormal`
    /// says the operation reports it.
    fn number<F: Bits>(&mut self, v: F, denormal: bool) -> F {
        if !v.is_denormal() {
            v
        } else if self.mxcsr & MXCSR_DENORMALS_ARE_ZERO != 0 {
            F::signed_zero(v.is_negative())
        } else {
            if denormal {
                self.flags |= MXCSR_DENORMAL;
            }
            v
        }
    }

    /// A not-a-number result when an operand is one: the first such operand, quieted, raising invalid for a
    /// signaling one.
    fn nans<F: Bits>(&mut self, operands: &[F]) -> Option<F> {
        if operands.iter().any(|v| v.is_signaling()) {
            self.flags |= MXCSR_INVALID;
        }
        operands.iter().find(|v| v.is_nan()).map(|v| v.quiet())
    }

    fn invalid<F: Bits>(&mut self) -> F {
        self.flags |= MXCSR_INVALID;
        F::default_nan()
    }

    /// The exact result `q`, rounded to odd in quadruple precision, rounded once to `F` as MXCSR says, with
    /// overflow, underflow (tiny after rounding), flush to zero and precision.
    fn finish<F: Bits>(&mut self, q: Quad) -> F
    where
        Quad: FloatConvert<F>,
    {
        let round = self.round();
        let convert = |q: Quad| -> StatusAnd<F> { q.convert_r(round, &mut false) };
        if !q.is_finite() || q.is_zero() {
            return convert(q).value;
        }
        // Rounded to F's precision with the exponent unbounded, scaled by 2^-exponent.
        let unbounded = |exponent: rustc_apfloat::ExpInt| convert(q.scalbn(-exponent));
        // An unmasked overflow or underflow raises precision too when F's precision cannot hold the result.
        let inexact = if convert(q.scalbn(-q.ilogb()))
            .status
            .contains(Status::INEXACT)
        {
            MXCSR_PRECISION
        } else {
            0
        };
        let one = F::from_u128(1).value;
        let big = unbounded(F::MAX_EXP);
        if big.value.abs() >= one.scalbn(1) {
            self.flags |= MXCSR_OVERFLOW;
            if !self.masked(MXCSR_OVERFLOW) {
                self.flags |= inexact;
                return F::ZERO;
            }
            self.flags |= MXCSR_PRECISION;
            return convert(q).value;
        }
        let small = unbounded(F::MIN_EXP);
        if small.value.abs() < one {
            if !self.masked(MXCSR_UNDERFLOW) {
                self.flags |= MXCSR_UNDERFLOW | inexact;
                return F::ZERO;
            }
            if self.mxcsr & MXCSR_FLUSH_TO_ZERO != 0 {
                self.flags |= MXCSR_UNDERFLOW | MXCSR_PRECISION;
                return F::signed_zero(q.is_negative());
            }
            let r = convert(q);
            if r.status.contains(Status::INEXACT) {
                self.flags |= MXCSR_UNDERFLOW | MXCSR_PRECISION;
            }
            return r.value;
        }
        let r = convert(q);
        if r.status.contains(Status::INEXACT) {
            self.flags |= MXCSR_PRECISION;
        }
        r.value
    }

    /// An operation in quadruple precision rounded toward zero, made odd when inexact.
    fn odd(r: StatusAnd<Quad>) -> Quad {
        if r.status.contains(Status::INEXACT) {
            Quad::from_bits(r.value.to_bits() | 1)
        } else {
            r.value
        }
    }

    fn arithmetic<F: Bits>(&mut self, op: Operation, a: F, b: F) -> F
    where
        Quad: FloatConvert<F>,
    {
        if let Some(nan) = self.nans(&[a, b]) {
            return nan;
        }
        let before = self.flags;
        let (a, b) = (self.number(a, true), self.number(b, true));
        let denormal = self.flags & MXCSR_DENORMAL;
        self.flags = before;
        let wide = |v: F| -> Quad { v.convert(&mut false).value };
        let (x, y) = (wide(a), wide(b));
        let z = Round::TowardZero;
        let r = match op {
            Operation::Add => x.add_r(y, z),
            Operation::Sub => x.sub_r(y, z),
            Operation::Mul => x.mul_r(y, z),
            _ => x.div_r(y, z),
        };
        if r.status.contains(Status::INVALID_OP) {
            return self.invalid();
        }
        // Dividing by zero raises that alone.
        if r.status.contains(Status::DIV_BY_ZERO) {
            self.flags |= MXCSR_DIVIDE_BY_ZERO;
            return self.finish(r.value);
        }
        self.flags |= denormal;
        // An exact zero's sign is the rounding's.
        if r.value.is_zero() {
            let r = match op {
                Operation::Add => x.add_r(y, self.round()),
                Operation::Sub => x.sub_r(y, self.round()),
                Operation::Mul => x.mul_r(y, self.round()),
                _ => x.div_r(y, self.round()),
            };
            return self.finish(r.value);
        }
        self.finish(Self::odd(r))
    }

    fn sqrt<F>(&mut self, v: F) -> F
    where
        Quad: FloatConvert<F>,
        F: Bits + FloatConvert<Double>,
    {
        if let Some(nan) = self.nans(&[v]) {
            return nan;
        }
        let v = self.number(v, false);
        if v.is_zero() {
            return v;
        }
        if v.is_negative() {
            return self.invalid();
        }
        let v = self.number(v, true);
        if v.is_infinite() {
            return v;
        }
        // The host's root rounded to nearest, then moved off it to the side the remainder says, which is
        // rounding to odd: no point of F's grid or halfway between lies between that and the true root.
        let d: Double = v.convert(&mut false).value;
        let s = f64::from_bits(d.to_bits() as u64).sqrt();
        let q: Quad = Double::from_bits(s.to_bits().into())
            .convert(&mut false)
            .value;
        let wide: Quad = v.convert(&mut false).value;
        let remainder = q.mul_r(q, Round::NearestTiesToEven).value - wide;
        let q = match remainder.value.partial_cmp(&Quad::ZERO) {
            Some(core::cmp::Ordering::Greater) => q.next_down().value,
            Some(core::cmp::Ordering::Less) => q.next_up().value,
            _ => q,
        };
        self.finish(q)
    }

    fn min_max<F: Bits>(&mut self, max: bool, a: F, b: F) -> F {
        let (a, b) = (self.number(a, false), self.number(b, false));
        if a.is_nan() || b.is_nan() {
            self.flags |= MXCSR_INVALID;
            return b;
        }
        let (a, b) = (self.number(a, true), self.number(b, true));
        let first = if max { a > b } else { a < b };
        if first { a } else { b }
    }

    /// Whether `a` and `b` are unordered, less or equal, raising invalid as `signaling` says.
    fn compare<F: Bits>(&mut self, signaling: bool, a: F, b: F) -> Option<core::cmp::Ordering> {
        if a.is_nan() || b.is_nan() {
            if signaling || a.is_signaling() || b.is_signaling() {
                self.flags |= MXCSR_INVALID;
            }
            return None;
        }
        let (a, b) = (self.number(a, true), self.number(b, true));
        a.partial_cmp(&b)
    }

    fn predicate<F: Bits>(&mut self, immediate: u8, a: F, b: F) -> bool {
        use Predicate as P;
        use core::cmp::Ordering as O;
        let p = PREDICATES[usize::from(immediate) % PREDICATES.len()];
        let signaling = matches!(p, P::Less | P::LessOrEqual | P::NotLess | P::NotLessOrEqual);
        let order = self.compare(signaling, a, b);
        match p {
            P::Equal => order == Some(O::Equal),
            P::Less => order == Some(O::Less),
            P::LessOrEqual => matches!(order, Some(O::Less | O::Equal)),
            P::Unordered => order.is_none(),
            P::NotEqual => order != Some(O::Equal),
            P::NotLess => order != Some(O::Less),
            P::NotLessOrEqual => !matches!(order, Some(O::Less | O::Equal)),
            P::Ordered => order.is_some(),
        }
    }

    fn round_integral<F: Bits>(&mut self, immediate: u8, v: F) -> F {
        if let Some(nan) = self.nans(&[v]) {
            return nan;
        }
        let v = self.number(v, false);
        let round = if immediate & ROUND_FROM_MXCSR != 0 {
            self.round()
        } else {
            ROUNDINGS[usize::from(immediate) % ROUNDINGS.len()]
        };
        let r = v.round_to_integral(round);
        if r.status.contains(Status::INEXACT) && immediate & ROUND_QUIET == 0 {
            self.flags |= MXCSR_PRECISION;
        }
        r.value
    }

    fn integer_of<F: Bits>(&mut self, bits: usize, truncate: bool, v: F) -> u128 {
        let indefinite = 1u128 << (bits - 1);
        if v.is_nan() {
            self.flags |= MXCSR_INVALID;
            return indefinite;
        }
        let v = self.number(v, false);
        let round = if truncate {
            Round::TowardZero
        } else {
            self.round()
        };
        let mut exact = false;
        let r = v.to_i128_r(bits, round, &mut exact);
        if r.status.contains(Status::INVALID_OP) {
            self.flags |= MXCSR_INVALID;
            return indefinite;
        }
        if !exact {
            self.flags |= MXCSR_PRECISION;
        }
        r.value as u128 & (u128::MAX >> (u128::BITS as usize - bits))
    }

    fn float_of<F: Bits>(&mut self, value: i128) -> F {
        let r = F::from_i128_r(value, self.round());
        if r.status.contains(Status::INEXACT) {
            self.flags |= MXCSR_PRECISION;
        }
        r.value
    }

    /// `F` is the source's format, `T` the other one.
    fn instruction<F, T>(
        &mut self,
        op: Operation,
        scalar: bool,
        immediate: u8,
        a: u128,
        b: u128,
    ) -> u128
    where
        Quad: FloatConvert<F> + FloatConvert<T>,
        F: Bits + FloatConvert<Double> + FloatConvert<T>,
        T: Bits,
    {
        use Operation::*;
        let lanes = if scalar {
            1
        } else {
            u128::BITS as usize / F::BITS
        };
        let f = |v: u128, n: usize| F::from_bits(lane::<F>(v, n));
        let half = u128::BITS as usize / F::BITS / 2;
        let mut out = a;
        match op {
            Add | Sub | Mul | Div => {
                for n in 0..lanes {
                    let r = self.arithmetic(op, f(a, n), f(b, n));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            AddSub => {
                for n in 0..lanes {
                    let op = if n % 2 == 0 { Sub } else { Add };
                    let r = self.arithmetic(op, f(a, n), f(b, n));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            HorizontalAdd | HorizontalSub => {
                let op = if op == HorizontalAdd { Add } else { Sub };
                for n in 0..lanes {
                    let (v, k) = if n < half {
                        (a, 2 * n)
                    } else {
                        (b, 2 * (n - half))
                    };
                    let r = self.arithmetic(op, f(v, k), f(v, k + 1));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            Min | Max => {
                for n in 0..lanes {
                    let r = self.min_max(op == Max, f(a, n), f(b, n));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            Sqrt => {
                for n in 0..lanes {
                    let r = self.sqrt(f(b, n));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            Round => {
                for n in 0..lanes {
                    let r = self.round_integral(immediate, f(b, n));
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            Compare => {
                for n in 0..lanes {
                    let r = if self.predicate(immediate, f(a, n), f(b, n)) {
                        F::mask()
                    } else {
                        0
                    };
                    out = with_lane::<F>(out, n, r);
                }
            }
            OrderedCompare | UnorderedCompare => {
                use core::cmp::Ordering::*;
                out = match self.compare(op == OrderedCompare, f(a, 0), f(b, 0)) {
                    None => ZF | PF | CF,
                    Some(Less) => CF,
                    Some(Equal) => ZF,
                    Some(Greater) => 0,
                }
                .into();
            }
            DotProduct => {
                // The selected products, +0 for the rest, summed in pairs and then the pairs' sums.
                let products = u32::from(immediate) >> DOT_PRODUCT_SHIFT;
                let count = u128::BITS as usize / F::BITS;
                let mut terms: Vec<F> = (0..count)
                    .map(|n| {
                        if products >> n & 1 != 0 {
                            self.arithmetic(Mul, f(a, n), f(b, n))
                        } else {
                            F::ZERO
                        }
                    })
                    .collect();
                // Each lane adds its neighbour, then the other pair's sum, in the CPU's order: the
                // neighbour first in single precision's first step (seen on the reference).
                let mut distance = 1;
                while distance < count {
                    let swapped = distance == 1 && F::BITS == u32::BITS as usize;
                    terms = (0..count)
                        .map(|n| {
                            let (x, y) = (terms[n], terms[n ^ distance]);
                            if swapped {
                                self.arithmetic(Add, y, x)
                            } else {
                                self.arithmetic(Add, x, y)
                            }
                        })
                        .collect();
                    distance *= 2;
                }
                out = 0;
                for (n, sum) in terms.iter().enumerate() {
                    if immediate >> n & 1 != 0 {
                        out = with_lane::<F>(out, n, sum.to_bits());
                    }
                }
            }
            Convert => {
                let count = if scalar {
                    1
                } else {
                    u128::BITS as usize / F::BITS.max(T::BITS)
                };
                if !scalar {
                    out = 0;
                }
                for n in 0..count {
                    let v = f(b, n);
                    let r: T = if let Some(nan) = self.nans(&[v]) {
                        convert_nan(nan)
                    } else {
                        let v = self.number(v, true);
                        self.finish(v.convert(&mut false).value)
                    };
                    out = with_lane::<T>(out, n, r.to_bits());
                }
            }
            FromInt32 | FromInt64 => {
                let bits = if op == FromInt32 {
                    u32::BITS
                } else {
                    u64::BITS
                } as usize;
                for n in 0..lanes {
                    let raw = (b >> (n * bits)) as u64;
                    let value = if op == FromInt32 {
                        i128::from(raw as i32)
                    } else {
                        i128::from(raw as i64)
                    };
                    let r: F = self.float_of(value);
                    out = with_lane::<F>(out, n, r.to_bits());
                }
            }
            ToInt32 | ToInt64 | TruncateToInt32 | TruncateToInt64 => {
                let bits = if matches!(op, ToInt32 | TruncateToInt32) {
                    u32::BITS
                } else {
                    u64::BITS
                } as usize;
                let truncate = matches!(op, TruncateToInt32 | TruncateToInt64);
                out = 0;
                for n in 0..lanes {
                    let r = self.integer_of(bits, truncate, f(b, n));
                    out |= r << (n * bits);
                }
            }
        }
        out
    }
}
