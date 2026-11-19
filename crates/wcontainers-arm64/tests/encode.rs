//! The native output's own encodings, each read back by the decoder (which `yaxpeax-arm` drives) as the
//! instruction it was meant to be. Seeded: `WCONTAINERS_ENCODE_SEED` and `WCONTAINERS_ENCODE_CASES`, decimal.

use wcontainers_arm64::encode::{self, ZERO, field, form};
use wcontainers_arm64::{
    Address, Bitfield, BranchRegister, Instruction as I, Logic, MoveWide, Operand, Reg, Width,
    decode,
};

const SEED: u64 = 0x5851_f42d_4c95_7f2d;
const CASES: u64 = 10_000;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |text| {
        text.parse()
            .unwrap_or_else(|_| panic!("{name} is not a number: {text}"))
    })
}

fn x(n: u8) -> Reg {
    Reg::X(n)
}

/// A drawn register other than the zero register.
fn register(rng: &mut fastrand::Rng) -> u8 {
    rng.u8(..ZERO)
}

#[test]
fn every_encoding_decodes_as_meant() {
    let seed = knob("WCONTAINERS_ENCODE_SEED", SEED);
    let cases = knob("WCONTAINERS_ENCODE_CASES", CASES);
    let mut rng = fastrand::Rng::with_seed(seed);
    let unit = 4;
    let mut logical = 0;
    for _ in 0..cases {
        let (rd, rn, rs) = (register(&mut rng), register(&mut rng), register(&mut rng));
        let value = rng.u64(..);
        let built: Vec<u64> = encode::move_constant(rd, value)
            .iter()
            .scan(0u64, |acc, &w| {
                match decode(w) {
                    I::MoveWide {
                        width: Width::X64,
                        op: MoveWide::Zero,
                        rd: r,
                        imm,
                        shift,
                    } if r == x(rd) => *acc = u64::from(imm) << shift,
                    I::MoveWide {
                        width: Width::X64,
                        op: MoveWide::Keep,
                        rd: r,
                        imm,
                        shift,
                    } if r == x(rd) => {
                        *acc = (*acc & !(u64::from(u16::MAX) << shift)) | (u64::from(imm) << shift)
                    }
                    other => panic!("{value:#x}: a move decoded as {other:?}"),
                }
                Some(*acc)
            })
            .collect();
        assert_eq!(built.last(), Some(&value), "moves of {value:#x}");

        let lsb = rng.u32(..u64::BITS);
        let bits = rng.u32(1..=u64::BITS - lsb);
        for (word, op) in [
            (encode::ubfx(rd, rn, lsb, bits), Bitfield::Unsigned),
            (encode::bfxil(rd, rn, lsb, bits), Bitfield::Insert),
            (encode::ubfiz(rd, rn, lsb, bits), Bitfield::Unsigned),
        ] {
            assert!(
                matches!(decode(word), I::Bitfield { width: Width::X64, op: o, rd: d, rn: n, .. } if o == op && d == x(rd) && n == x(rn)),
                "{word:#010x} {:?}",
                decode(word)
            );
        }

        // A drawn run of ones, rotated, repeated: always a logical immediate.
        let size = 1u32 << rng.u32(1..=u64::BITS.trailing_zeros());
        let ones = rng.u32(1..size);
        let element = (1u64 << ones) - 1;
        let rotate = rng.u32(..size);
        let mask = u64::MAX >> (u64::BITS - size);
        let rotated = ((element >> rotate) | (element << ((size - rotate) % size))) & mask;
        let pattern = (0..u64::BITS / size).fold(0u64, |p, k| p | (rotated << (k * size)));
        for (word, op) in [
            (encode::and_immediate(rd, rn, pattern), Logic::And),
            (encode::eor_immediate(rd, rn, pattern), Logic::Xor),
        ] {
            let word = word.unwrap_or_else(|_| panic!("{pattern:#x} is a logical immediate"));
            assert_eq!(
                decode(word),
                I::Logical {
                    width: Width::X64,
                    op,
                    invert: false,
                    set_flags: false,
                    rd: x(rd),
                    rn: x(rn),
                    operand: Operand::Imm(pattern)
                },
                "{pattern:#x}"
            );
            logical += 1;
        }
        // Any value: a logical immediate exactly when the decoder reads it back.
        if let Ok(word) = encode::and_immediate(rd, rn, value) {
            assert!(
                matches!(decode(word), I::Logical { operand: Operand::Imm(v), .. } if v == value)
            );
        }

        let offset = (rng.i64(-(1 << 18)..(1 << 18))) * unit;
        assert_eq!(
            decode(encode::compare_branch(true, rd, offset).unwrap()),
            I::CompareBranch {
                width: Width::X64,
                nonzero: true,
                rt: x(rd),
                offset
            }
        );
        let near = (rng.i64(-(1 << 13)..(1 << 13))) * unit;
        let bit = rng.u32(..u64::BITS);
        assert_eq!(
            decode(encode::test_branch_nonzero(rd, bit, near).unwrap()),
            I::TestBranch {
                nonzero: true,
                rt: x(rd),
                bit: bit as u8,
                offset: near
            }
        );
        let far = (rng.i64(-(1 << 25)..(1 << 25))) * unit;
        let link = rng.bool();
        assert_eq!(
            decode(encode::branch(link, far).unwrap()),
            I::Branch { link, offset: far }
        );
        let imm = rng.i64(-4095..=4095);
        assert!(matches!(
            decode(encode::add_immediate(rd, rn, imm).unwrap()),
            I::AddSub { width: Width::X64, subtract, set_flags: false, rd: d, rn: n, operand: Operand::Imm(v) }
                if subtract == (imm < 0) && d == x(rd) && n == x(rn) && v == imm.unsigned_abs()
        ));
        let words = rng.u64(..1 << 12);
        assert_eq!(
            decode(encode::load_store(form::LDR, rd, rn, words * 8, 8).unwrap()),
            I::Load {
                bytes: 8,
                signed: None,
                rt: x(rd),
                address: Address::Offset {
                    base: x(rn),
                    offset: (words * 8) as i64
                }
            }
        );
        assert!(matches!(
            decode(encode::load_add(form::LDADDB, rs, ZERO, rn)),
            I::Atomic { bytes: 1, rs: s, rt: Reg::Zero, rn: n, .. } if s == x(rs) && n == x(rn)
        ));
        for (name, size) in [(form::LDADDH, 2), (form::LDADD_32, 4), (form::LDADD_64, 8)] {
            assert!(matches!(
                decode(encode::load_add(name, rs, rd, rn)),
                I::Atomic { bytes, rs: s, rt: t, rn: n, .. } if bytes == size && s == x(rs) && t == x(rd) && n == x(rn)
            ));
        }
        for (name, size) in [
            (form::CASB, 1),
            (form::CASH, 2),
            (form::CAS_32, 4),
            (form::CAS, 8),
        ] {
            assert!(matches!(
                decode(encode::compare_swap_of(name, rs, rd, rn)),
                I::CompareSwap { bytes, pair: false, rs: s, rt: t, rn: n } if bytes == size && s == x(rs) && t == x(rd) && n == x(rn)
            ));
        }
        let (even, even2) = (rs & !1, rd & !1);
        assert!(matches!(
            decode(encode::compare_swap_of(form::CASP, even, even2, rn)),
            I::CompareSwap { bytes: 8, pair: true, rs: s, rt: t, rn: n } if s == x(even) && t == x(even2) && n == x(rn)
        ));
        for (name, size) in [(form::LDP_32, 4), (form::LDP_64, 8)] {
            assert!(matches!(
                decode(encode::load_pair(name, rd, rs, rn)),
                I::LoadPair { bytes, signed: false, rt, rt2, address: Address::Offset { base, offset: 0 } }
                    if bytes == size && rt == x(rd) && rt2 == x(rs) && base == x(rn)
            ));
        }
        let (signed, from_bits, shift) = (
            rng.bool(),
            [u8::BITS, u32::BITS, u64::BITS][rng.usize(..3)],
            rng.u32(..=4),
        );
        let subtract = rng.bool();
        let word = encode::add_extended(subtract, rd, rn, rs, signed, from_bits, shift).unwrap();
        assert!(
            matches!(
                decode(word),
                I::AddSub { width: Width::X64, subtract: s, set_flags: false, rd: d, rn: n, operand: Operand::Extended { rm, extend, amount: sh } }
                    if s == subtract && d == x(rd) && n == x(rn) && rm == x(rs) && extend.signed == signed && u32::from(extend.from_bits) == from_bits && u32::from(sh) == shift
            ),
            "{word:#x} {:?}",
            decode(word)
        );
        assert!(matches!(
            decode(encode::logical_register(form::EOR_SHIFTED, rd, rn, rs)),
            I::Logical { op: Logic::Xor, invert: false, set_flags: false, rd: d, rn: n, .. } if d == x(rd) && n == x(rn)
        ));
        assert!(matches!(
            decode(encode::move_register(rd, rs)),
            I::Logical { op: Logic::Or, rd: d, rn: Reg::Zero, .. } if d == x(rd)
        ));
        let (lsb, bits) = (rng.u32(..u64::BITS), 1);
        assert!(matches!(
            decode(encode::bfi(rd, rn, lsb, bits)),
            I::Bitfield { op: Bitfield::Insert, rd: d, rn: n, .. } if d == x(rd) && n == x(rn)
        ));
        // A copied branch retargeted and a copied load given another register.
        let moved =
            encode::retarget(encode::compare_branch(false, rd, offset).unwrap(), near).unwrap();
        assert!(matches!(decode(moved), I::CompareBranch { offset: o, .. } if o == near));
        let load = encode::load_store(form::LDR, rd, rn, 0, 8).unwrap();
        assert!(
            matches!(decode(encode::replace(load, field::RT, rs.into()).unwrap()), I::Load { rt, .. } if rt == x(rs))
        );
        for (name, op) in [
            (form::BR, BranchRegister::Br),
            (form::BLR, BranchRegister::Blr),
            (form::RET, BranchRegister::Ret),
        ] {
            assert!(
                matches!(decode(encode::branch_register(name, rn)), I::BranchRegister { op: o, rn: n } if o == op && n == x(rn))
            );
        }
    }
    assert!(encode::branch(false, 1 << 28).is_err());
    assert!(encode::and_immediate(0, 0, 0).is_err());
    println!("seed {seed}, {cases} cases, {logical} logical immediates");
}

/// The exception forms, which the CPU we present decodes as traps or not at all, read back by Arm's table.
#[test]
fn exception_forms_read_back() {
    for name in [form::BRK, form::UDF, form::HVC] {
        for imm in [0, 1, u16::MAX] {
            let word = encode::exception(name, imm);
            let found = wcontainers_arm64::forms::form(word).expect("a form");
            assert_eq!(found.name, name);
            assert_eq!(found.field(word, field::IMM16), Some(imm.into()));
        }
    }
    let eret = encode::exception_return();
    assert_eq!(
        wcontainers_arm64::forms::form(eret).map(|f| f.name.as_str()),
        Some(form::ERET)
    );
}
