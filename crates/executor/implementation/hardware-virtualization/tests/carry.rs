//! The native output's pieces at look-ahead none, for every decoded form, have the shape the position map and
//! the floor rely on (docs/native.md section 4): every host word is an instruction we present, the map covers
//! every byte in order, a call into the inside sits on the side of the commit point its kind needs, the count's
//! lead is the add ahead of its event and nothing after the commit point, nothing but the add touches the count,
//! no exit leaves a guest value relocated, and each slow path is counted at the sites it is written. Seeded: `WCONTAINERS_CARRY_SEED` and `WCONTAINERS_CARRY_WORDS`,
//! the words drawn per form, both decimal.

use std::collections::BTreeMap;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_arm64::carry::{COUNT, SCRATCH, WITHHELD};
use wcontainers_arm64::encode;
use wcontainers_arm64::{
    Address, Arm64, BranchRegister, Host, Instruction as I, Reg, decode, forms,
};
use wcontainers_guest_cpu::{
    Guest, InsideCall, Native, NativeReason, Route, Side, counted, effects,
};
use wcontainers_translator::{LookAhead, translate_native};

#[path = "../../../../wcontainers-arm64/tests/common/mod.rs"]
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
    let unpaid = translate_native::<Arm64>(&word.to_le_bytes(), AT, LookAhead::None, false)
        .expect("a word decodes");
    let piece = translate_native::<Arm64>(&word.to_le_bytes(), AT, LookAhead::None, true)
        .expect("a word decodes");
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
    let mut page_tests = 0;
    let mut previous = 0;
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
        if let I::TestBranch {
            rt: Reg::X(SCRATCH),
            bit,
            ..
        } = h
            && u32::from(bit) == PAGE_SIZE.trailing_zeros()
            && !matches!(i, I::TestBranch { .. })
        {
            page_tests += 1;
        }
        // The three-word page test: a `cbz` of t after t's page-offset bits above the access's are kept.
        if let I::CompareBranch {
            nonzero: false,
            rt: Reg::X(SCRATCH),
            ..
        } = h
            && let Some(m) = i.memory_access()
            && Some(previous)
                == encode::and_immediate(
                    SCRATCH,
                    SCRATCH,
                    (PAGE_SIZE - 1) & !(u64::from(m.bytes) - 1),
                )
                .ok()
        {
            page_tests += 1;
        }
        previous = host;
        if let I::Breakpoint { comment } = h {
            let call = InsideCall::from_code(comment).expect("our own calls only");
            let side = match call {
                InsideCall::Exit | InsideCall::Unreachable | InsideCall::FloatAfter => Side::After,
                InsideCall::Answer
                | InsideCall::FloatBefore
                | InsideCall::Trap
                | InsideCall::Misaligned => Side::Before,
                // A call through a register has built x30 before its last target is compared.
                InsideCall::X87Control => return Some(format!("x86's {call:?} at {offset}")),
                InsideCall::LastTarget => match i {
                    I::BranchRegister {
                        op: BranchRegister::Blr,
                        ..
                    } => Side::After,
                    _ => Side::Before,
                },
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
    // Without exact replay the piece is the same less the add.
    let add_bytes = add.to_le_bytes();
    let less_add = match event {
        true => piece.bytes.strip_prefix(&add_bytes[..]),
        false => Some(&piece.bytes[..]),
    };
    let unpaid_leads = unpaid.spans.iter().all(|s| s.lead == 0);
    if !unpaid_leads || less_add.is_none_or(|rest| !branches_aside(rest, &unpaid.bytes)) {
        return Some("without exact replay the piece is not the same less the count's add".into());
    }
    // A literal load's crossing is known when translating: its probes are written with no test.
    let literal_crossing = match (i, i.memory_access()) {
        (
            I::Load {
                address: Address::Literal { offset },
                ..
            }
            | I::LoadVector {
                address: Address::Literal { offset },
                ..
            },
            Some(m),
        ) => AT.wrapping_add_signed(offset) % PAGE_SIZE + u64::from(m.bytes) > PAGE_SIZE,
        _ => false,
    };
    let page_sites = page_tests + u64::from(literal_crossing);
    let wide = i
        .memory_access()
        .is_some_and(|m| m.bytes > size_of::<u128>() as u32);
    let (probed, routed) = if wide {
        (page_sites, 0)
    } else {
        (0, page_sites)
    };
    let e = effects::<Arm64>(&i, HARDWARE, ());
    let named = WITHHELD.iter().any(|&w| {
        e.uses
            .union(e.defs)
            .has_bit(Arm64::reg_bit(Reg::X(w)).expect("a bit"))
    });
    let read_where_kept = matches!(
        Arm64::route(&(), &i, &()),
        Route::Kernel | Route::Trap | Route::Nothing
    );
    let expected = [
        (
            NativeReason::WithheldRegisterNamed,
            u64::from(named && !read_where_kept),
        ),
        (NativeReason::AccessMayCrossPage, probed),
        (NativeReason::HostModelResultRouted, routed),
    ];
    for (reason, sites) in expected {
        if piece.sites_of(reason) != sites {
            return Some(format!(
                "{} sites of {reason:?}, expected {sites}",
                piece.sites_of(reason)
            ));
        }
    }
    None
}

/// Whether two pieces are the same but for where their branches point (one is a word shorter in front).
fn branches_aside(a: &[u8], b: &[u8]) -> bool {
    let unit = Arm64::MAX_LENGTH as usize;
    a.len() == b.len()
        && a.chunks(unit).zip(b.chunks(unit)).all(|(x, y)| {
            let word = |c: &[u8]| u32::from_le_bytes(c.try_into().expect("a word"));
            let (x, y) = (word(x), word(y));
            x == y
                || matches!(
                    (encode::retarget(x, 0), encode::retarget(y, 0)),
                    (Ok(p), Ok(q)) if p == q
                )
        })
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
                .entry(format!("{:?}", Arm64::route(&(), &decode(word), &())))
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
