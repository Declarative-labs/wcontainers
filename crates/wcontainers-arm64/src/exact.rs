//! Floating point done in software, for the slow path: when a program has set a rounding mode, flush to
//! zero or default not-a-number, or asked for the cumulative exception flags, and for fused multiply-add on
//! hosts whose engine does not fuse. It runs on the host behind [`crate::Hook::Float`].
//!
//! The arithmetic is `rustc_apfloat`'s (and `ieee-apsqrt`'s square root); what is Arm's own is here: which
//! not-a-number comes out, flushing to zero, when underflow is raised, and the estimate instructions.

use crate::instruction::Rounding;
use rustc_apfloat::ieee::{Double, Half, Single};
use rustc_apfloat::{Float, FloatConvert, Round, Status, StatusAnd};

/// One operation on elements of `size` bytes, as the meaning passes it to the hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Add,
    Sub,
    Mul,
    Div,
    Max,
    Min,
    MaxNumber,
    MinNumber,
    MulExtended,
    /// `a + b * c`, fused.
    MulAdd,
    /// `2 - a * b` and `(3 - a * b) / 2`, fused.
    ReciprocalStep,
    ReciprocalSqrtStep,
    Sqrt,
    Round(Rounding),
    /// NZCV of the comparison in bits 31 to 28.
    Compare {
        signaling: bool,
    },
    ToInteger {
        signed: bool,
        bits: u8,
        rounding: Rounding,
        fraction_bits: u8,
    },
    FromInteger {
        signed: bool,
        bits: u8,
        fraction_bits: u8,
    },
    /// To another size; `odd` rounds to odd (fcvtxn).
    Convert {
        to: u8,
        odd: bool,
    },
    ReciprocalEstimate,
    ReciprocalSqrtEstimate,
    ReciprocalExponent,
    /// FPSR is being read while its flags are not kept: only counted.
    ReadStatus,
}

const ROUNDINGS: [Rounding; 7] = [
    Rounding::Nearest,
    Rounding::Away,
    Rounding::Floor,
    Rounding::Ceiling,
    Rounding::Zero,
    Rounding::Current,
    Rounding::CurrentExact,
];

impl Operation {
    /// Whether it is a fused multiply-add, which a host without a fused one does here in every mode.
    pub fn fused(self) -> bool {
        matches!(
            self,
            Operation::MulAdd | Operation::ReciprocalStep | Operation::ReciprocalSqrtStep
        )
    }

    /// The i32 the hook receives: kind, size, flags and a number.
    pub fn encode(self, size: u8) -> i32 {
        use Operation as O;
        let rounding = |r: Rounding| ROUNDINGS.iter().position(|&x| x == r).unwrap() as u32;
        let (kind, flags, number): (u32, u32, u32) = match self {
            O::Add => (0, 0, 0),
            O::Sub => (1, 0, 0),
            O::Mul => (2, 0, 0),
            O::Div => (3, 0, 0),
            O::Max => (4, 0, 0),
            O::Min => (5, 0, 0),
            O::MaxNumber => (6, 0, 0),
            O::MinNumber => (7, 0, 0),
            O::MulExtended => (8, 0, 0),
            O::MulAdd => (9, 0, 0),
            O::ReciprocalStep => (10, 0, 0),
            O::ReciprocalSqrtStep => (11, 0, 0),
            O::Sqrt => (12, 0, 0),
            O::Round(r) => (13, rounding(r), 0),
            O::Compare { signaling } => (14, u32::from(signaling), 0),
            O::ToInteger {
                signed,
                bits,
                rounding: r,
                fraction_bits,
            } => (
                15,
                rounding(r) | u32::from(signed) << 3 | u32::from(bits == 64) << 4,
                fraction_bits.into(),
            ),
            O::FromInteger {
                signed,
                bits,
                fraction_bits,
            } => (
                16,
                u32::from(signed) << 3 | u32::from(bits == 64) << 4,
                fraction_bits.into(),
            ),
            O::Convert { to, odd } => (17, u32::from(odd), to.into()),
            O::ReciprocalEstimate => (18, 0, 0),
            O::ReciprocalSqrtEstimate => (19, 0, 0),
            O::ReciprocalExponent => (20, 0, 0),
            O::ReadStatus => (21, 0, 0),
        };
        (kind | u32::from(size) << 8 | flags << 16 | number << 24) as i32
    }

    pub fn decode(code: i32) -> (Operation, u8) {
        use Operation as O;
        let code = code as u32;
        let (kind, size, flags, number) = (
            code & 0xff,
            (code >> 8) as u8,
            (code >> 16) & 0xff,
            (code >> 24) as u8,
        );
        let rounding = ROUNDINGS[(flags & 7) as usize];
        let signed = flags & 8 != 0;
        let bits = if flags & 16 != 0 { 64 } else { 32 };
        let op = match kind {
            0 => O::Add,
            1 => O::Sub,
            2 => O::Mul,
            3 => O::Div,
            4 => O::Max,
            5 => O::Min,
            6 => O::MaxNumber,
            7 => O::MinNumber,
            8 => O::MulExtended,
            9 => O::MulAdd,
            10 => O::ReciprocalStep,
            11 => O::ReciprocalSqrtStep,
            12 => O::Sqrt,
            13 => O::Round(rounding),
            14 => O::Compare {
                signaling: flags & 1 != 0,
            },
            15 => O::ToInteger {
                signed,
                bits,
                rounding,
                fraction_bits: number,
            },
            16 => O::FromInteger {
                signed,
                bits,
                fraction_bits: number,
            },
            17 => O::Convert {
                to: number,
                odd: flags & 1 != 0,
            },
            18 => O::ReciprocalEstimate,
            19 => O::ReciprocalSqrtEstimate,
            20 => O::ReciprocalExponent,
            _ => O::ReadStatus,
        };
        (op, size)
    }
}

// FPSR's cumulative flags.
const IOC: u32 = 1;
const DZC: u32 = 2;
const OFC: u32 = 4;
const UFC: u32 = 8;
const IXC: u32 = 16;
const IDC: u32 = 128;

// FPCR's bits.
const AHP: u32 = 1 << 26;
const DN: u32 = 1 << 25;
const FZ: u32 = 1 << 24;

/// Run one operation under `fpcr`, adding the exceptions it raises to `fpsr`.
pub fn run(code: i32, a: u64, b: u64, c: u64, fpcr: u32, fpsr: &mut u32) -> u64 {
    let (op, size) = Operation::decode(code);
    if op == Operation::ReadStatus {
        return 0;
    }
    let mut x = Context { fpcr, flags: 0 };
    let result = match size {
        2 => x.run::<Half>(op, a, b, c),
        4 => x.run::<Single>(op, a, b, c),
        _ => x.run::<Double>(op, a, b, c),
    };
    *fpsr |= x.flags;
    result
}

struct Context {
    fpcr: u32,
    flags: u32,
}

/// The raw bits of a format's parts.
trait Bits: Float + FloatConvert<Half> + FloatConvert<Single> + FloatConvert<Double> {
    fn sign_bit() -> u128 {
        1 << (Self::BITS - 1)
    }
    fn quiet_bit() -> u128 {
        1 << (Self::PRECISION - 2)
    }
    fn fraction_mask() -> u128 {
        (1 << (Self::PRECISION - 1)) - 1
    }
    fn mask() -> u128 {
        if Self::BITS == 128 {
            u128::MAX
        } else {
            (1 << Self::BITS) - 1
        }
    }
    fn default_nan() -> u128 {
        (Self::mask() >> 1) & !(Self::fraction_mask() >> 1)
    }
}

impl Bits for Half {}
impl Bits for Single {}
impl Bits for Double {}

fn zero<F: Bits>(negative: bool) -> F {
    if negative { -F::ZERO } else { F::ZERO }
}

impl Context {
    fn round(&self) -> Round {
        match (self.fpcr >> 22) & 3 {
            0 => Round::NearestTiesToEven,
            1 => Round::TowardPositive,
            2 => Round::TowardNegative,
            _ => Round::TowardZero,
        }
    }

    fn rounding(&self, r: Rounding) -> Round {
        match r {
            Rounding::Nearest => Round::NearestTiesToEven,
            Rounding::Away => Round::NearestTiesToAway,
            Rounding::Floor => Round::TowardNegative,
            Rounding::Ceiling => Round::TowardPositive,
            Rounding::Zero => Round::TowardZero,
            Rounding::Current | Rounding::CurrentExact => self.round(),
        }
    }

    /// Flush to zero applies to single and double precision only.
    fn flushes<F: Bits>(&self) -> bool {
        self.fpcr & FZ != 0 && F::BITS > 16
    }

    /// An operand, flushed to zero (raising input denormal) when FZ asks.
    fn unpack<F: Bits>(&mut self, bits: u64) -> F {
        let value = F::from_bits(u128::from(bits) & F::mask());
        if self.flushes::<F>() && value.is_denormal() {
            self.flags |= IDC;
            return zero(value.is_negative());
        }
        value
    }

    /// Arm's choice among not-a-number operands: the first signaling one, quieted, else the first quiet
    /// one; the default one under DN. `None` when no operand is one.
    fn nans<F: Bits>(&mut self, operands: &[F]) -> Option<F> {
        let chosen = operands
            .iter()
            .find(|v| v.is_signaling())
            .or_else(|| operands.iter().find(|v| v.is_nan()))?;
        if chosen.is_signaling() || operands.iter().any(|v| v.is_signaling()) {
            self.flags |= IOC;
        }
        Some(self.quiet(*chosen))
    }

    fn quiet<F: Bits>(&self, v: F) -> F {
        if self.fpcr & DN != 0 {
            F::from_bits(F::default_nan())
        } else {
            F::from_bits(v.to_bits() | F::quiet_bit())
        }
    }

    fn default_nan<F: Bits>(&mut self) -> F {
        self.flags |= IOC;
        F::from_bits(F::default_nan())
    }

    /// A rounded result of an operation on numbers: its exceptions, Arm's underflow (tiny after rounding and
    /// inexact), and flushing to zero (tiny before rounding).
    ///
    /// `op(round, false)` is the operation; `op(round, true)` is half of it, computed so that halving is
    /// exact before the rounding wherever the result can be near the largest number. Rounding towards zero
    /// gives the largest number with only inexact raised; whether that was an overflow is whether half the
    /// exact result reaches half the overflow threshold.
    fn finish<F: Bits>(&mut self, op: impl Fn(Round, bool) -> StatusAnd<F>) -> F {
        let StatusAnd { mut status, value } = op(self.round(), false);
        if value.is_finite()
            && value.abs() == F::largest()
            && status.contains(Status::INEXACT)
            && op(Round::TowardZero, true).value.abs() >= F::from_u128(1).value.scalbn(F::MAX_EXP)
        {
            status |= Status::OVERFLOW;
        }
        if value.is_nan() {
            return self.default_nan();
        }
        let inexact = status.contains(Status::INEXACT);
        // Tiny: the exact result is below the smallest normal number, which is when rounding towards zero
        // gives less than it.
        let tiny = value.is_finite()
            && (inexact || !value.is_zero())
            && op(Round::TowardZero, false).value.abs() < F::smallest_normalized();
        if self.flushes::<F>() && tiny {
            self.flags |= UFC;
            return zero(value.is_negative());
        }
        self.status(status, value, tiny)
    }

    /// Raise a rounded result's exceptions. Arm raises underflow when the exact result is tiny (below the
    /// smallest normal number, before rounding) and inexact.
    fn status<F: Bits>(&mut self, status: Status, value: F, tiny: bool) -> F {
        if status.contains(Status::INVALID_OP) {
            self.flags |= IOC;
        }
        if status.contains(Status::DIV_BY_ZERO) {
            self.flags |= DZC;
        }
        if status.contains(Status::OVERFLOW) {
            self.flags |= OFC;
        }
        if status.contains(Status::INEXACT) {
            self.flags |= IXC;
            if tiny {
                self.flags |= UFC;
            }
        }
        value
    }

    fn run<F: Bits>(&mut self, op: Operation, a: u64, b: u64, c: u64) -> u64 {
        use Operation as O;
        let bits = |v: F| v.to_bits() as u64;
        match op {
            O::FromInteger {
                signed,
                bits: width,
                fraction_bits,
            } => {
                let value = if signed {
                    i128::from(if width == 64 {
                        a as i64
                    } else {
                        a as i32 as i64
                    })
                } else {
                    i128::from(if width == 64 { a } else { a & 0xffff_ffff })
                };
                let scale = -i32::from(fraction_bits) as rustc_apfloat::ExpInt;
                let result = self.finish(|round, _| {
                    F::from_i128_r(value, round).map(|v| v.scalbn_r(scale, round))
                });
                return bits(result);
            }
            O::Convert { to, odd } => return self.convert::<F>(a, to, odd),
            _ => {}
        }
        let (x, y, z) = (
            self.unpack::<F>(a),
            self.unpack::<F>(b),
            self.unpack::<F>(c),
        );
        let result: F = match op {
            O::Add | O::Sub | O::Mul | O::Div => {
                if let Some(n) = self.nans(&[x, y]) {
                    n
                } else {
                    self.finish(|round, half| {
                        let (hx, hy) = if half {
                            (x.scalbn(-1), y.scalbn(-1))
                        } else {
                            (x, y)
                        };
                        match op {
                            O::Add => hx.add_r(hy, round),
                            O::Sub => hx.sub_r(hy, round),
                            O::Mul => hx.mul_r(y, round),
                            // x / (2y), or (x / 2) / y when 2y is not finite (x is then large).
                            _ if half && y.scalbn(1).is_finite() => x.div_r(y.scalbn(1), round),
                            _ => hx.div_r(y, round),
                        }
                    })
                }
            }
            O::Max | O::Min | O::MaxNumber | O::MinNumber => {
                let max = matches!(op, O::Max | O::MaxNumber);
                let (mut x, mut y) = (x, y);
                if matches!(op, O::MaxNumber | O::MinNumber) {
                    // A quiet not-a-number against a number is treated as the infinity that loses.
                    let losing = if max { -F::INFINITY } else { F::INFINITY };
                    let quiet = |v: F| v.is_nan() && !v.is_signaling();
                    if quiet(x) && !quiet(y) {
                        x = losing;
                    } else if quiet(y) && !quiet(x) {
                        y = losing;
                    }
                }
                if let Some(n) = self.nans(&[x, y]) {
                    n
                } else if x.is_zero() && y.is_zero() && x.is_negative() != y.is_negative() {
                    zero(!max)
                } else {
                    let x_larger = x > y;
                    let picked = if x_larger == max { x } else { y };
                    // A result is still rounded, so flush to zero applies to it.
                    self.finish(|_, _| StatusAnd {
                        status: Status::OK,
                        value: picked,
                    })
                }
            }
            O::MulExtended => {
                if let Some(n) = self.nans(&[x, y]) {
                    n
                } else if (x.is_infinite() && y.is_zero()) || (x.is_zero() && y.is_infinite()) {
                    let two = F::from_u128(2).value;
                    if x.is_negative() != y.is_negative() {
                        -two
                    } else {
                        two
                    }
                } else {
                    self.finish(|round, half| if half { x.scalbn(-1) } else { x }.mul_r(y, round))
                }
            }
            O::MulAdd => {
                // Arm's order: the addend first.
                let invalid_product =
                    (y.is_infinite() && z.is_zero()) || (y.is_zero() && z.is_infinite());
                if x.is_nan() && !x.is_signaling() && invalid_product {
                    self.default_nan()
                } else if let Some(n) = self.nans(&[x, y, z]) {
                    n
                } else {
                    self.finish(|round, half| {
                        if half {
                            y.scalbn(-1).mul_add_r(z, x.scalbn(-1), round)
                        } else {
                            y.mul_add_r(z, x, round)
                        }
                    })
                }
            }
            O::ReciprocalStep | O::ReciprocalSqrtStep => {
                let x = -x;
                let sqrt = op == O::ReciprocalSqrtStep;
                if let Some(n) = self.nans(&[x, y]) {
                    n
                } else if (x.is_infinite() && y.is_zero()) || (x.is_zero() && y.is_infinite()) {
                    let value: u128 = if sqrt { 3 } else { 2 };
                    let v = F::from_u128(value).value;
                    if sqrt { v.scalbn(-1) } else { v }
                } else if x.is_infinite() || y.is_infinite() {
                    let inf = F::INFINITY;
                    if x.is_negative() != y.is_negative() {
                        -inf
                    } else {
                        inf
                    }
                } else {
                    let two = F::from_u128(2).value;
                    let three = F::from_u128(3).value;
                    if !sqrt {
                        self.finish(|round, half| {
                            if half {
                                x.scalbn(-1).mul_add_r(y, F::from_u128(1).value, round)
                            } else {
                                x.mul_add_r(y, two, round)
                            }
                        })
                    } else if x.abs() >= F::smallest_normalized().scalbn(1) {
                        // (3 + x * y) / 2 as x / 2 * y + 3 / 2, exact before its one rounding.
                        let half = x.scalbn(-1);
                        self.finish(|round, halved| {
                            if halved {
                                half.scalbn(-1).mul_add_r(y, three.scalbn(-2), round)
                            } else {
                                half.mul_add_r(y, three.scalbn(-1), round)
                            }
                        })
                    } else {
                        // A small x: the sum cannot overflow, and halving it is exact.
                        self.finish(|round, _| x.mul_add_r(y, three, round).map(|v| v.scalbn(-1)))
                    }
                }
            }
            O::Sqrt => {
                if let Some(n) = self.nans(&[x]) {
                    n
                } else if x.is_zero() {
                    x
                } else if x.is_negative() {
                    self.default_nan()
                } else {
                    self.finish(|round, _| sqrt::<F>(x, round))
                }
            }
            O::Round(r) => {
                if let Some(n) = self.nans(&[x]) {
                    n
                } else {
                    let StatusAnd { status, value } = x.round_to_integral(self.rounding(r));
                    if r == Rounding::CurrentExact && status.contains(Status::INEXACT) {
                        self.flags |= IXC;
                    }
                    // Rounding keeps the sign of zero.
                    if value.is_zero() {
                        zero(x.is_negative())
                    } else {
                        value
                    }
                }
            }
            O::Compare { signaling } => {
                let nan = x.is_nan() || y.is_nan();
                if (signaling && nan) || x.is_signaling() || y.is_signaling() {
                    self.flags |= IOC;
                }
                let nzcv: u64 = if nan {
                    0b0011
                } else if x == y {
                    0b0110
                } else if x < y {
                    0b1000
                } else {
                    0b0010
                };
                return nzcv << 28;
            }
            O::ToInteger {
                signed,
                bits: width,
                rounding,
                fraction_bits,
            } => {
                if x.is_nan() {
                    self.flags |= IOC;
                    return 0;
                }
                let scaled = x.scalbn(fraction_bits.into());
                let mut exact = true;
                let round = self.rounding(rounding);
                let width = usize::from(width);
                let value = if signed {
                    let v = scaled.to_i128_r(width, round, &mut exact);
                    if v.status.contains(Status::INVALID_OP) {
                        self.flags |= IOC;
                    } else if !exact && !(scaled.is_zero()) {
                        self.flags |= IXC;
                    }
                    v.value as u64
                } else {
                    let v = scaled.to_u128_r(width, round, &mut exact);
                    if v.status.contains(Status::INVALID_OP) {
                        self.flags |= IOC;
                    } else if !exact && !scaled.is_zero() {
                        self.flags |= IXC;
                    }
                    v.value as u64
                };
                return if width == 32 {
                    value & 0xffff_ffff
                } else {
                    value
                };
            }
            O::ReciprocalEstimate => self.reciprocal_estimate(x),
            O::ReciprocalSqrtEstimate => self.reciprocal_sqrt_estimate(x),
            O::ReciprocalExponent => {
                if let Some(n) = self.nans(&[x]) {
                    n
                } else {
                    let raw = x.to_bits();
                    let sign = raw & F::sign_bit();
                    let exponent_mask = (F::mask() >> 1) & !F::fraction_mask();
                    // Zeros and denormals: the largest finite exponent.
                    let exponent = if raw & exponent_mask == 0 {
                        // The largest exponent of a finite number.
                        exponent_mask - (F::fraction_mask() + 1)
                    } else {
                        !raw & exponent_mask
                    };
                    F::from_bits(sign | exponent)
                }
            }
            O::FromInteger { .. } | O::Convert { .. } | O::ReadStatus => unreachable!(),
        };
        bits(result)
    }

    fn convert<F: Bits>(&mut self, a: u64, to: u8, odd: bool) -> u64 {
        match to {
            2 => self.convert_to::<F, Half>(a, odd),
            4 => self.convert_to::<F, Single>(a, odd),
            _ => self.convert_to::<F, Double>(a, odd),
        }
    }

    fn convert_to<F: Bits + FloatConvert<T>, T: Bits>(&mut self, a: u64, odd: bool) -> u64 {
        let alternative_in = F::BITS == 16 && self.fpcr & AHP != 0;
        if alternative_in && (a >> 10) & 0x1f == 0x1f {
            // The alternative half precision has no infinity or not-a-number: the top exponent is a number.
            // Every such number is exact in single precision.
            let single = ((a & 0x8000) << 16) | ((0x1f + 112) << 23) | ((a & 0x3ff) << 13);
            return self.convert::<Single>(single, (T::BITS / 8) as u8, odd);
        }
        let x = self.unpack::<F>(a);
        let alternative = T::BITS == 16 && self.fpcr & AHP != 0;
        if x.is_nan() {
            if alternative {
                // No not-a-number in the alternative format: zero with the sign, and invalid.
                self.flags |= IOC;
                return if x.is_negative() {
                    T::sign_bit() as u64
                } else {
                    0
                };
            }
            if x.is_signaling() {
                self.flags |= IOC;
            }
            if self.fpcr & DN != 0 {
                return T::default_nan() as u64;
            }
            // Keep the sign and the top of the payload.
            let raw = x.to_bits();
            let payload = raw & F::fraction_mask();
            let payload = if T::PRECISION > F::PRECISION {
                payload << (T::PRECISION - F::PRECISION)
            } else {
                payload >> (F::PRECISION - T::PRECISION)
            };
            let sign = if raw & F::sign_bit() != 0 {
                T::sign_bit()
            } else {
                0
            };
            return (sign | T::default_nan() | payload) as u64;
        }
        if alternative && x.is_infinite() {
            self.flags |= IOC;
            let sign = if x.is_negative() { T::sign_bit() } else { 0 };
            return (sign | ((T::mask() >> 1) & T::mask())) as u64;
        }
        let round = if odd { Round::TowardZero } else { self.round() };
        if alternative && x.abs() > F::from_u128(65504).value {
            // The alternative format's top exponent holds numbers: twice what the exponent below holds, so
            // convert half the value and move it up one exponent.
            let mut l = false;
            let StatusAnd { status, value }: StatusAnd<T> = x.scalbn(-1).convert_r(round, &mut l);
            let sign = if x.is_negative() { T::sign_bit() } else { 0 };
            if value.is_infinite()
                || (value.abs() == T::largest() && x.abs() >= F::from_u128(131072).value)
            {
                self.flags |= IOC;
                return (sign | ((T::mask() >> 1) & T::mask())) as u64;
            }
            let raw = value.to_bits() & (T::mask() >> 1);
            let exponent_one = T::fraction_mask() + 1;
            // Below the top exponent the format is the usual one: only values that reach it move.
            if raw >= 30 * exponent_one {
                return self
                    .status(status, T::from_bits(sign | (raw + exponent_one)), false)
                    .to_bits() as u64;
            }
        }
        let mut loses = false;
        let StatusAnd { status, value }: StatusAnd<T> = x.convert_r(round, &mut loses);
        if alternative && value.is_infinite() {
            // No infinity in the alternative format: the largest number, and invalid.
            self.flags |= IOC;
            let sign = if value.is_negative() {
                T::sign_bit()
            } else {
                0
            };
            return (sign | ((T::mask() >> 1) & T::mask())) as u64;
        }
        let mut status = status;
        if value.is_finite() && value.abs() == T::largest() && status.contains(Status::INEXACT) {
            let mut l = false;
            let half: T = x.scalbn(-1).convert_r(Round::TowardZero, &mut l).value;
            if half.abs() >= T::from_u128(1).value.scalbn(T::MAX_EXP) {
                status |= Status::OVERFLOW;
            }
        }
        let value = if odd && status.contains(Status::INEXACT) {
            T::from_bits(value.to_bits() | 1)
        } else {
            value
        };
        let tiny = value.is_finite() && !x.is_zero() && {
            let mut l = false;
            let toward_zero: T = x.convert_r(Round::TowardZero, &mut l).value;
            toward_zero.abs() < T::smallest_normalized()
        };
        if self.flushes::<T>() && tiny {
            self.flags |= UFC;
            return zero::<T>(value.is_negative()).to_bits() as u64;
        }
        self.status(status, value, tiny).to_bits() as u64
    }

    fn reciprocal_estimate<F: Bits>(&mut self, x: F) -> F {
        if let Some(n) = self.nans(&[x]) {
            return n;
        }
        let negative = x.is_negative();
        if x.is_infinite() {
            return zero(negative);
        }
        if x.is_zero() {
            self.flags |= DZC;
            return if negative { -F::INFINITY } else { F::INFINITY };
        }
        let (tiny, huge) = if F::BITS == 32 {
            (
                F::from_u128(1).value.scalbn(-128),
                F::from_u128(1).value.scalbn(126),
            )
        } else {
            (
                F::from_u128(1).value.scalbn(-1024),
                F::from_u128(1).value.scalbn(1022),
            )
        };
        if x.abs() < tiny {
            let to_infinity = match (self.fpcr >> 22) & 3 {
                0 => true,
                1 => !negative,
                2 => negative,
                _ => false,
            };
            self.flags |= OFC | IXC;
            let v = if to_infinity {
                F::INFINITY
            } else {
                F::largest()
            };
            return if negative { -v } else { v };
        }
        if self.flushes::<F>() && x.abs() >= huge {
            self.flags |= UFC;
            return zero(negative);
        }
        F::from_bits(reciprocal_estimate_bits(x.to_bits() as u64, F::BITS as u32).into())
    }

    fn reciprocal_sqrt_estimate<F: Bits>(&mut self, x: F) -> F {
        if let Some(n) = self.nans(&[x]) {
            return n;
        }
        if x.is_zero() {
            self.flags |= DZC;
            return if x.is_negative() {
                -F::INFINITY
            } else {
                F::INFINITY
            };
        }
        if x.is_negative() {
            return self.default_nan();
        }
        if x.is_infinite() {
            return F::ZERO;
        }
        F::from_bits(reciprocal_sqrt_estimate_bits(x.to_bits() as u64, F::BITS as u32).into())
    }
}

fn sqrt<F: Bits>(x: F, round: Round) -> StatusAnd<F> {
    // `ieee-apsqrt` rounds some denormal inputs wrongly away from nearest: take the root of x * 2^64, a
    // normal number, and scale it back, both exact.
    if x.is_denormal() {
        return sqrt(x.scalbn(64), round).map(|v| v.scalbn(-32));
    }
    let raw = x.to_bits();
    if F::BITS == 32 {
        ieee_apsqrt::sqrt_accurate(raw as u32, round)
            .0
            .map(|v| F::from_bits(v.into()))
    } else {
        ieee_apsqrt::sqrt_accurate(raw as u64, round)
            .0
            .map(|v| F::from_bits(v.into()))
    }
}

/// Arm's `RecipEstimate`: a 9 bit estimate of 1/x for x = a/512, a in 256 to 511.
pub fn recip_estimate(a: u64) -> u64 {
    let a = a * 2 + 1;
    let b = (1 << 19) / a;
    b.div_ceil(2)
}

/// Arm's `RecipSqrtEstimate`: a 9 bit estimate of 1/sqrt(x) for a in 128 to 511.
pub fn recip_sqrt_estimate(a: u64) -> u64 {
    let a = if a < 256 {
        a * 2 + 1
    } else {
        ((a >> 1) << 1) * 2 + 2
    };
    let mut b = 512u64;
    while a * (b + 1) * (b + 1) < 1 << 28 {
        b += 1;
    }
    b.div_ceil(2)
}

/// The estimate of a finite, normal-range, non-zero number's reciprocal, by Arm's `FPRecipEstimate`.
fn reciprocal_estimate_bits(raw: u64, bits: u32) -> u64 {
    let (fraction_bits, exponent_bits) = if bits == 32 { (23, 8) } else { (52, 11) };
    let sign = raw >> (bits - 1);
    let mut exponent = ((raw >> fraction_bits) & ((1 << exponent_bits) - 1)) as i64;
    // The fraction as 52 bits.
    let mut fraction = (raw & ((1 << fraction_bits) - 1)) << (52 - fraction_bits);
    if exponent == 0 {
        if fraction >> 51 == 0 {
            exponent = -1;
            fraction = (fraction & ((1 << 50) - 1)) << 2;
        } else {
            fraction = (fraction & ((1 << 51) - 1)) << 1;
        }
    }
    let scaled = (1 << 8) | (fraction >> 44);
    let mut result_exponent = if bits == 32 { 253 } else { 2045 } - exponent;
    let estimate = recip_estimate(scaled);
    let mut fraction = (estimate & 0xff) << 44;
    if result_exponent == 0 {
        fraction = (1 << 51) | (fraction >> 1);
    } else if result_exponent == -1 {
        fraction = (1 << 50) | (fraction >> 2);
        result_exponent = 0;
    }
    (sign << (bits - 1))
        | ((result_exponent as u64) & ((1 << exponent_bits) - 1)) << fraction_bits
        | fraction >> (52 - fraction_bits)
}

/// The estimate of a positive, finite, non-zero number's reciprocal square root, by Arm's
/// `FPRSqrtEstimate`.
fn reciprocal_sqrt_estimate_bits(raw: u64, bits: u32) -> u64 {
    let (fraction_bits, exponent_bits) = if bits == 32 { (23, 8) } else { (52, 11) };
    let mut exponent = ((raw >> fraction_bits) & ((1 << exponent_bits) - 1)) as i64;
    let mut fraction = (raw & ((1 << fraction_bits) - 1)) << (52 - fraction_bits);
    if exponent == 0 {
        while fraction >> 51 == 0 {
            fraction = (fraction << 1) & ((1 << 52) - 1);
            exponent -= 1;
        }
        fraction = (fraction << 1) & ((1 << 52) - 1);
    }
    let scaled = if exponent & 1 == 0 {
        (1 << 8) | (fraction >> 44)
    } else {
        (1 << 7) | (fraction >> 45)
    };
    let result_exponent = (if bits == 32 { 380 } else { 3068 } - exponent) / 2;
    let estimate = recip_sqrt_estimate(scaled);
    ((result_exponent as u64) & ((1 << exponent_bits) - 1)) << fraction_bits
        | (estimate & 0xff) << (fraction_bits - 8)
}
