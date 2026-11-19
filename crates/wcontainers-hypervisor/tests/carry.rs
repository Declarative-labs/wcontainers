//! The native output's pieces at look-ahead none, for every decoded form, have the shape the position map and
//! the floor rely on (docs/native.md section 4): every host word is an instruction we present, the map covers
//! every byte in order, a call into the inside sits on the side of the commit point its kind needs, the count's
//! lead is the add ahead of its event and nothing after the commit point, nothing but the add touches the count,
//! and no exit leaves a guest value relocated. Seeded: `WCONTAINERS_CARRY_SEED` and `WCONTAINERS_CARRY_WORDS`,
//! the words drawn per form, both decimal.

use std::collections::BTreeMap;

use wcontainers_arm64::carry::COUNT;
use wcontainers_arm64::encode;
use wcontainers_arm64::{Arm64, Host, Instruction as I, Reg, decode, forms};
use wcontainers_guest_cpu::{Guest, InsideCall, Native, Side, counted, effects};
use wcontainers_translator::translate_native;

#[path = "../../wcontainers-arm64/tests/common/mod.rs"]
mod common;
use common::{WORDS_PER_FORM, knob, words_of};
const SEED: u64 = 0x9fb2_1c65_1e98_df25;
/// Where the drawn instructions are placed: far enough from 0 and from the top of the reach that branches in
/// both directions land on guest addresses.
const AT: u64 = 0x40_0000_1000;

const HARDWARE: Host = Host {
    fused_multiply_add: true,
    arm_nans: true,
};

/// What is wrong with the piece of `word`, if anything.
fn fault(word: u32) -> Option<String> {
    let i = decode(word);
    let piece = translate_native::<Arm64>(&word.to_le_bytes(), AT).expect("a word decodes");
    let add = encode::add_immediate(COUNT, COUNT, 1).expect("fits");
    let event = counted(AT, &Arm64::flow(&i));
    let unit = Arm64::MAX_LENGTH as usize;
    if piece.bytes.is_empty() || !piece.bytes.len().is_multiple_of(unit) {
        return Some(format!("{} bytes", piece.bytes.len()));
    }
    if piece.spans.first().map(|s| s.start) != Some(0)
        || piece.spans.windows(2).any(|w| w[0].start >= w[1].start)
    {
        return Some("spans out of order".into());
    }
    let mut adds = 0;
    for (k, chunk) in piece.bytes.chunks(unit).enumerate() {
        let host = u32::from_le_bytes(chunk.try_into().expect("a word"));
        let offset = (k * unit) as u32;
        let span = piece.span(offset).expect("every byte has a span");
        let h = decode(host);
        if matches!(h, I::Undefined | I::Unsupported) {
            return Some(format!(
                "host word {host:#010x} at {offset} is not presented"
            ));
        }
        let e = effects::<Arm64>(&h, HARDWARE, ());
        let touches_count = e
            .uses
            .union(e.defs)
            .has_bit(Arm64::reg_bit(Reg::X(COUNT)).expect("a bit"));
        if host == add {
            adds += 1;
            if offset != 0 || span.lead != 0 {
                return Some("the count's add is not first".into());
            }
        } else if touches_count {
            return Some(format!(
                "host word {host:#010x} at {offset} touches the count"
            ));
        }
        let lead = match (offset, span.side) {
            (0, _) => 0,
            (_, Side::Before) => u32::from(event),
            (_, Side::After) => 0,
        };
        if span.lead != lead {
            return Some(format!("lead {} at {offset} on {:?}", span.lead, span.side));
        }
        if let I::Breakpoint { comment } = h {
            let call = InsideCall::from_code(comment).expect("our own calls only");
            let side = match call {
                InsideCall::Exit | InsideCall::Unreachable | InsideCall::FloatAfter => Side::After,
                InsideCall::Answer
                | InsideCall::FloatBefore
                | InsideCall::Trap
                | InsideCall::Misaligned => Side::Before,
            };
            if span.side != side {
                return Some(format!("{call:?} at {offset} on {:?}", span.side));
            }
            if call == InsideCall::Exit && !span.relocated.is_empty() {
                return Some(format!("exit at {offset} with {:?}", span.relocated));
            }
        }
    }
    if adds != u32::from(event) {
        return Some(format!("{adds} adds for {event}"));
    }
    None
}

#[test]
fn every_decoded_form_carries_over_in_shape() {
    let seed = knob("WCONTAINERS_CARRY_SEED", SEED);
    let draws = knob("WCONTAINERS_CARRY_WORDS", WORDS_PER_FORM);
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut routes: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut carried = 0u64;
    for form in forms::all() {
        for word in words_of(form, draws, &mut rng) {
            if !forms::form(word).is_some_and(|f| std::ptr::eq(f, form)) {
                continue;
            }
            carried += 1;
            *routes
                .entry(format!("{:?}", Arm64::route(&decode(word))))
                .or_default() += 1;
            let result = std::panic::catch_unwind(|| fault(word));
            match result {
                Ok(None) => {}
                Ok(Some(why)) => failures.push(format!("{} {word:#010x}: {why}", form.name)),
                Err(_) => failures.push(format!("{} {word:#010x}: panicked", form.name)),
            }
        }
    }
    println!(
        "seed {seed}, {draws} words drawn per form: {carried} words carried; routes {routes:?}"
    );
    let shown = failures.len().min(usize::from(u8::MAX));
    assert!(
        failures.is_empty(),
        "{} of {carried} words carried out of shape (seed {seed}):\n{}",
        failures.len(),
        failures[..shown].join("\n")
    );
}
