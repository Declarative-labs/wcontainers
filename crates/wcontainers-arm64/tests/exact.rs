//! The software floating point path on cases whose answer is known from Arm's rules.

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
