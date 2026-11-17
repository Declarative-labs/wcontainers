//! The software floating point path on cases whose answer is known from Arm's rules.

use rustc_apfloat::ieee::{Double, Quad, Single};
use rustc_apfloat::{Float, FloatConvert};
use wcontainers_arm64::exact::{Operation, run};

fn single(op: Operation, a: f32, b: f32, fpcr: u32) -> (u32, u32) {
    let mut fpsr = 0;
    let r = run(
        op.encode(4),
        a.to_bits().into(),
        b.to_bits().into(),
        0,
        fpcr,
        &mut fpsr,
    );
    (r as u32, fpsr)
}

#[test]
fn overflow_raises_overflow_and_inexact() {
    assert_eq!(
        single(Operation::Mul, 1e30, 1e30, 0),
        (f32::INFINITY.to_bits(), 0x14)
    );
    // Towards zero: the largest number, and still overflow.
    assert_eq!(
        single(Operation::Mul, 1e30, 1e30, 3 << 22),
        (f32::MAX.to_bits(), 0x14)
    );
}

#[test]
fn not_a_numbers_follow_arm() {
    let snan = f32::from_bits(0x7f80_0001);
    let qnan = f32::from_bits(0xffc0_0002);
    assert_eq!(single(Operation::Add, qnan, snan, 0), (0x7fc0_0001, 1));
    assert_eq!(single(Operation::Add, qnan, 1.0, 0), (0xffc0_0002, 0));
    assert_eq!(
        single(Operation::Sub, f32::INFINITY, f32::INFINITY, 0),
        (0x7fc0_0000, 1)
    );
}

/// A square root's bits for `size` (4 or 8) in rounding mode `mode` (FPCR's RMode), and FPSR after.
fn root(x: u64, size: u8, mode: u32) -> (u64, u32) {
    let mut fpsr = 0;
    let r = run(Operation::Sqrt.encode(size), x, 0, 0, mode << 22, &mut fpsr);
    (if size == 4 { r & 0xffff_ffff } else { r }, fpsr)
}

/// A positive float's bits as a quad, where the products of two doubles are exact.
fn quad(bits: u64, size: u8) -> Quad {
    let mut lost = false;
    if size == 4 {
        Single::from_bits(bits.into()).convert(&mut lost).value
    } else {
        Double::from_bits(bits.into()).convert(&mut lost).value
    }
}

/// What is wrong with the square root of `x`, judged by exact squares: rounding up gives the least float whose
/// square is at least x, down or toward zero the greatest whose square is at most x, nearest the one whose
/// neighbouring midpoints' squares lie either side of x; inexact is raised when the square is not x.
fn wrong_root(x: u64, size: u8, mode: u32) -> Option<String> {
    let (r, fpsr) = root(x, size, mode);
    let (xq, rq, below, above) = (
        quad(x, size),
        quad(r, size),
        quad(r - 1, size),
        quad(r + 1, size),
    );
    let square = |q: Quad| (q * q).value;
    let exact = square(rq) == xq;
    let rounded = match mode {
        0 => {
            let low = (below + rq).value.scalbn(-1);
            let high = (rq + above).value.scalbn(-1);
            square(low) < xq && xq < square(high)
        }
        1 => square(rq) >= xq && square(below) < xq,
        _ => square(rq) <= xq && square(above) > xq,
    };
    (!rounded || (fpsr & 0x10 != 0) == exact)
        .then(|| format!("size {size} mode {mode}: root of {x:#x} gave {r:#x}, fpsr {fpsr:#x}"))
}

/// Exact squares k^2 * 2^j (normal and denormal) and their neighbours, in every rounding mode.
#[test]
fn square_roots_round_as_asked() {
    let mut wrong = Vec::new();
    for (size, exponents) in [
        (
            8,
            &[-1074, -1060, -1022, -1000, -500, -2, 0, 2, 50, 1000][..],
        ),
        (4, &[-149, -140, -126, -100, -2, 0, 2, 100][..]),
    ] {
        for k in 1..=3000u128 {
            for &j in exponents {
                let x = if size == 4 {
                    Single::from_u128(k * k).value.scalbn(j).to_bits() as u64
                } else {
                    Double::from_u128(k * k).value.scalbn(j).to_bits() as u64
                };
                for x in [x - 1, x, x + 1].into_iter().filter(|&x| x > 0) {
                    for mode in 0..4 {
                        wrong.extend(wrong_root(x, size, mode));
                    }
                }
            }
        }
    }
    // The root of infinity is infinity, exact; of minus infinity the default not-a-number, invalid.
    for mode in 0..4 {
        assert_eq!(
            root(f64::INFINITY.to_bits(), 8, mode),
            (f64::INFINITY.to_bits(), 0)
        );
        assert_eq!(
            root(f32::INFINITY.to_bits().into(), 4, mode),
            (f32::INFINITY.to_bits().into(), 0)
        );
        assert_eq!(
            root((-f64::INFINITY).to_bits(), 8, mode),
            (0x7ff8_0000_0000_0000, 1)
        );
    }
    assert!(
        wrong.is_empty(),
        "{} wrong, first: {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(10)]
    );
}
