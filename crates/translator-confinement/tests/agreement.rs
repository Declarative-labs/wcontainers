//! The checker's decoder against the translator's, over words drawn from every A64 form: every access the translator's
//! decoder finds is one access to the checker, at the same base, offset and size, and every general register an
//! instruction's meaning writes is one the checker lets it write. Two decoders written apart, each the other's check.

use wcontainers_arm64::{Address, Arm64, Host, Reg as TheirReg, decode as theirs, forms};
use wcontainers_guest_cpu::effects;
use wcontainers_translator_confinement::{
    Access, Base, Extend, Moved, Offset, Reg, Source, Step, decode,
};

/// The hardware's own answer to what the meanings leave open.
const HARDWARE: Host = Host {
    fused_multiply_add: true,
    arm_nans: true,
};

const WORDS_PER_FORM: usize = 24;

fn source(r: TheirReg) -> Source {
    match r {
        TheirReg::X(n) => Source::Reg(Reg::X(n)),
        TheirReg::Sp => Source::Reg(Reg::Sp),
        TheirReg::Zero => Source::Zero,
    }
}

/// The access the translator's decoder finds, as the checker writes one, its loads and size aside.
fn expected(address: Address) -> (Base, Offset, Option<Moved>) {
    match address {
        Address::Offset { base, offset } => (Base::Reg(source(base)), Offset::Constant(offset, offset), None),
        Address::PreIndex { base, offset } => (
            Base::Reg(source(base)),
            Offset::Constant(offset, offset),
            Some(Moved::By(offset)),
        ),
        Address::PostIndex { base, offset } => {
            (Base::Reg(source(base)), Offset::Constant(0, 0), Some(Moved::By(offset)))
        }
        Address::Indexed {
            base,
            index,
            extend,
            shift,
        } => (
            Base::Reg(source(base)),
            Offset::Index {
                rm: source(index),
                extend: if extend.signed {
                    Extend::Signed(extend.from_bits.into())
                } else {
                    Extend::Unsigned(extend.from_bits.into())
                },
                shift: shift.into(),
            },
            None,
        ),
        Address::Literal { offset } => (Base::Here, Offset::Constant(offset, offset), None),
    }
}

/// The general registers (31 the stack pointer) a step may write, as bits.
fn written(step: &Step) -> Option<u32> {
    let bit = |r: &Reg| match r {
        Reg::X(n) => 1u32 << n,
        Reg::Sp => 1 << 31,
    };
    match step {
        Step::Plain(writes) => Some(writes.iter().fold(0, |b, (r, _)| b | bit(r))),
        Step::Access(Access {
            loads, moved, base, ..
        }) => {
            let mut bits = loads.iter().fold(0, |b, (r, _)| b | bit(r));
            if let (Some(_), Base::Reg(Source::Reg(r))) = (moved, base) {
                bits |= bit(r);
            }
            Some(bits)
        }
        Step::Branch { link, .. } | Step::Jump { link, .. } => Some(if *link { 1 << 30 } else { 0 }),
        // The checker refuses these, or the run ends at them.
        Step::Leaves | Step::SystemCall | Step::ForbiddenRegister | Step::Unknown => None,
    }
}

fn words() -> Vec<u32> {
    let mut rng = fastrand::Rng::with_seed(0x5eed_c0f1);
    forms::all()
        .iter()
        .flat_map(|form| (0..WORDS_PER_FORM).map(|_| form.word(rng.u32(..))).collect::<Vec<_>>())
        .collect()
}

#[test]
fn every_access_the_translator_finds_is_the_checker_s() {
    let mut differences = Vec::new();
    for word in words() {
        let Some(m) = theirs(word).memory_access() else {
            continue;
        };
        let (base, offset, moved) = expected(m.address);
        match decode(word) {
            Step::Access(a)
                if a.base == base
                    && a.offset == offset
                    && a.bytes == u64::from(m.bytes)
                    && a.write == m.write
                    && (moved.is_none() || a.moved == moved) => {}
            other => differences.push(format!("{word:#010x}: translator {m:?}, checker {other:?}")),
        }
    }
    assert!(differences.is_empty(), "{} differ: {:#?}", differences.len(), &differences[..differences.len().min(20)]);
}

#[test]
fn every_register_a_meaning_writes_the_checker_lets_it_write() {
    let mut differences = Vec::new();
    for word in words() {
        let step = decode(word);
        let Some(mine) = written(&step) else {
            continue;
        };
        let defs = effects::<Arm64>(&theirs(word), HARDWARE, ()).defs.0;
        let general = (0..32).fold(0u32, |b, n| b | (((defs >> n) & 1) as u32) << n);
        if general & !mine != 0 {
            differences.push(format!("{word:#010x}: meaning writes {general:#x}, checker {mine:#x} ({step:?})"));
        }
    }
    assert!(differences.is_empty(), "{} differ: {:#?}", differences.len(), &differences[..differences.len().min(20)]);
}
