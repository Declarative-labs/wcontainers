//! Every module the translator makes of random code is valid WebAssembly, for each guest. Cases:
//! `PROPTEST_CASES` (default 256); seed: `PROPTEST_RNG_SEED` (default 1).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use wasmparser::{Operator, Parser, Payload};
use wcontainers_arm64::{Arm64, Host};
use wcontainers_guest_cpu::Guest;
use wcontainers_translator::{
    AccessOrdering, Error, LookAhead, Optimise, OrderedAccess, ProgressCount, thread_module,
    translate,
};
use wcontainers_translator_test::ordered::scan;
use wcontainers_translator_test::programs::LOAD;
use wcontainers_x86::{GUEST_MEMORY, Offer, X86_64};

/// A host that needs every fix-up and the software fused multiply-add, so their code is checked too.
const HOST: Host = Host {
    fused_multiply_add: false,
    arm_nans: false,
};

/// A host whose NaNs are not x86's, so the fix-ups' code is checked too.
const HOST_X86_64: wcontainers_x86::Host = wcontainers_x86::Host {
    x86_nans: false,
    fused_multiply_add: false,
};

/// The v3 list when `v3`, else the default list, so that random code reaches the VEX instructions too.
fn told(v3: bool) -> Offer {
    if v3 {
        Offer::new(wcontainers_x86::V3).expect("a built list")
    } else {
        Offer::default()
    }
}

/// Both forms of the output, without and with the progress count.
const COUNTS: [ProgressCount; 2] = [ProgressCount::Absent, ProgressCount::Kept];

/// Gives the progress count kept or not, as drawn.
fn count(kept: bool) -> ProgressCount {
    if kept {
        ProgressCount::Kept
    } else {
        ProgressCount::Absent
    }
}

/// Translations for size or for speed, as drawn.
fn optimise(size: bool) -> Optimise {
    if size {
        Optimise::Size
    } else {
        Optimise::Speed
    }
}

fn validate(wasm: &[u8]) {
    if let Err(error) = check(wasm) {
        panic!("invalid module: {error}");
    }
}

fn check(wasm: &[u8]) -> Result<(), String> {
    wcontainers_translator_test::ordered::validity(wasm)
}

/// The ordered form, as the runners' setting asks for it.
const ORDERED: AccessOrdering = AccessOrdering::Ordered(OrderedAccess::SequentiallyConsistent);
/// The form of ordered loads and plain stores, for x86 hosts.
const LOADS_ORDERED: AccessOrdering =
    AccessOrdering::LoadsOrdered(OrderedAccess::SequentiallyConsistent);
/// Every form.
const FORMS: [AccessOrdering; 3] = [AccessOrdering::Unordered, ORDERED, LOADS_ORDERED];

/// Locked instructions with a memory destination, each read, changed and written by one compare and exchange.
const LOCKED: [&[u8]; 10] = [
    // lock add [rax], rbx
    &[0xf0, 0x48, 0x01, 0x18],
    // lock add dword [rax], 1
    &[0xf0, 0x83, 0x00, 0x01],
    // lock inc qword [rax]
    &[0xf0, 0x48, 0xff, 0x00],
    // lock xadd [rax], rbx
    &[0xf0, 0x48, 0x0f, 0xc1, 0x18],
    // lock cmpxchg [rax], rbx
    &[0xf0, 0x48, 0x0f, 0xb1, 0x18],
    // xchg [rax], rbx, locked without a prefix
    &[0x48, 0x87, 0x18],
    // lock bts [rax], rbx
    &[0xf0, 0x48, 0x0f, 0xab, 0x18],
    // lock xor byte [rax], 1
    &[0xf0, 0x80, 0x30, 0x01],
    // lock neg word [rax]
    &[0xf0, 0x66, 0xf7, 0x18],
    // lock cmpxchg8b [rax]
    &[0xf0, 0x0f, 0xc7, 0x08],
];

/// How many compare and exchange operations on the guest's memory a module holds.
fn compare_exchanges(wasm: &[u8]) -> usize {
    let mut found = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        let Payload::CodeSectionEntry(body) = payload.expect("a module") else {
            continue;
        };
        for operator in body.get_operators_reader().expect("a body") {
            let memarg = match operator.expect("an operator") {
                Operator::I64AtomicRmwCmpxchg { memarg }
                | Operator::I64AtomicRmw32CmpxchgU { memarg }
                | Operator::I64AtomicRmw16CmpxchgU { memarg }
                | Operator::I64AtomicRmw8CmpxchgU { memarg } => memarg,
                _ => continue,
            };
            if memarg.target == wasmparser::MemArgTarget::Memory(GUEST_MEMORY) {
                found += 1;
            }
        }
    }
    found
}

/// In the ordered form a locked instruction is one compare and exchange; in the unordered form it has none.
#[test]
fn locked_instructions_compare_and_exchange_when_ordered_x86_64() {
    for one in LOCKED {
        let mut code = one.to_vec();
        code.push(RETURN);
        for look_ahead in [LookAhead::None, LookAhead::Full] {
            for ordering in FORMS {
                let translation = translate::<X86_64>(
                    &code,
                    0..code.len() as u64,
                    None,
                    &[0],
                    look_ahead,
                    Optimise::Speed,
                    wcontainers_translator::ProgressCount::Absent,
                    ordering,
                    HOST_X86_64,
                    Offer::default(),
                )
                .unwrap();
                let found: usize = translation
                    .modules
                    .iter()
                    .map(|module| {
                        validate(&module.wasm);
                        compare_exchanges(&module.wasm)
                    })
                    .sum();
                let at = format!("{one:02x?} at {look_ahead:?} {ordering:?}");
                if ordering != AccessOrdering::Unordered {
                    assert!(found > 0, "{at}: no compare and exchange");
                } else {
                    assert_eq!(found, 0, "{at}");
                }
            }
        }
    }
}

/// ARM64's meanings make no ordered accesses, so the ordered form is refused for it.
#[test]
fn the_ordered_form_is_refused_arm64() {
    // ret
    let code = 0xd65f03c0u32.to_le_bytes();
    let refused = translate::<Arm64>(
        &code,
        0..code.len() as u64,
        None,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        wcontainers_translator::ProgressCount::Absent,
        ORDERED,
        HOST,
        (),
    );
    assert_eq!(refused.err(), Some(Error::OrderedAccessesNotMade));
}

#[test]
fn thread_module_is_valid_arm64() {
    for count in COUNTS {
        validate(&thread_module::<Arm64>(count));
    }
}

#[test]
fn thread_module_is_valid_x86_64() {
    for count in COUNTS {
        validate(&thread_module::<X86_64>(count));
    }
}

/// The most random bytes an x86 case translates, 255: many instructions of every length.
const CASE_BYTES: usize = u8::MAX as usize;

/// An instruction that writes out an absolute address: `mov r32, imm32` (`B8+r`), `push imm32` (`68`) or
/// `movabs r64, imm64` (`REX.W B8+r`), of the register its draw names.
enum Absolute {
    Move(u8),
    Push,
    MoveWide(u8),
}

/// The opcodes, and REX.W.
const MOVE_IMMEDIATE: u8 = 0xb8;
const PUSH_IMMEDIATE: u8 = 0x68;
const REX_W: u8 = 0x48;
/// The registers `B8+r` names without REX.B.
const OPCODE_REGISTERS: u8 = 8;

impl Absolute {
    const FORMS: &[fn(u8) -> Absolute] = &[Absolute::Move, |_| Absolute::Push, Absolute::MoveWide];

    fn drawn(drawn: u8) -> Self {
        let forms = Self::FORMS.len() as u8;
        Self::FORMS[usize::from(drawn % forms)](drawn / forms % OPCODE_REGISTERS)
    }

    fn length(&self) -> usize {
        match self {
            Absolute::Move(_) | Absolute::Push => 1 + size_of::<u32>(),
            Absolute::MoveWide(_) => 2 + size_of::<u64>(),
        }
    }

    fn encode(&self, address: u64, code: &mut Vec<u8>) {
        match *self {
            Absolute::Move(r) => {
                code.push(MOVE_IMMEDIATE + r);
                code.extend((address as u32).to_le_bytes());
            }
            Absolute::Push => {
                code.push(PUSH_IMMEDIATE);
                code.extend((address as u32).to_le_bytes());
            }
            Absolute::MoveWide(r) => {
                code.extend([REX_W, MOVE_IMMEDIATE + r]);
                code.extend(address.to_le_bytes());
            }
        }
    }
}

/// The starts (sorted) where a whole x86 instruction starts within the bytes.
fn whole_x86_64(bytes: &[u8], starts: &[u64]) -> Vec<u64> {
    starts
        .iter()
        .copied()
        .filter(|&s| X86_64::decode(&bytes[s as usize..]).is_some())
        .collect()
}

/// The environment's settings, with a fixed seed when none is given.
fn config() -> Config {
    let mut config = Config::default();
    if config.rng_seed == RngSeed::Random {
        config.rng_seed = RngSeed::Fixed(1);
    }
    config
}

/// A branch whose target wraps below the blob's start.
#[test]
fn target_below_zero_arm64() {
    // tbz x0, #0, -32768
    let blob = 0x3604_0000u32.to_le_bytes();
    let translation = translate::<Arm64>(
        &blob,
        0..4,
        None,
        &[0],
        LookAhead::None,
        Optimise::Speed,
        wcontainers_translator::ProgressCount::Absent,
        wcontainers_translator::AccessOrdering::Unordered,
        HOST,
        (),
    )
    .unwrap();
    assert_eq!(translation.modules[0].pieces, [0]);
    validate(&translation.modules[0].wasm);
}

/// A chain of thousands of branches, each block nested in the next one's merge, with more blocks than one
/// piece may hold: translated at full on a small stack, cut in parts, and valid.
#[test]
fn long_chains_translate_on_a_small_stack_arm64() {
    let mut words = Vec::new();
    for _ in 0..4000 {
        words.push(0xb400_0041u32); // cbz x1, +8
        words.push(0x9100_0400); // add x0, x0, #1
    }
    words.push(0xd65f_03c0); // ret
    let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let translation = std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || {
            translate::<Arm64>(
                &blob,
                0..blob.len() as u64,
                None,
                &[0],
                LookAhead::Full,
                Optimise::Speed,
                wcontainers_translator::ProgressCount::Absent,
                wcontainers_translator::AccessOrdering::Unordered,
                HOST,
                (),
            )
        })
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    assert!(translation.pieces().count() > 1);
    for module in &translation.modules {
        validate(&module.wasm);
    }
}

/// Repeated string instructions one after another, as a compiler writes a run of fills and copies.
const REPEATED_STRINGS: [&[u8]; 3] = [
    // mov rdi, rsi; mov ecx, 11; rep stosq
    &[0x48, 0x89, 0xf7, 0xb9, 0x0b, 0, 0, 0, 0xf3, 0x48, 0xab],
    // mov ecx, 11; rep movsb
    &[0xb9, 0x0b, 0, 0, 0, 0xf3, 0xa4],
    // mov ecx, 11; repe cmpsb
    &[0xb9, 0x0b, 0, 0, 0, 0xf3, 0xa6],
];

/// `ret`, which ends the run.
const RETURN: u8 = 0xc3;

/// How many of them the longer run has.
const LONG_RUN: usize = 16;

/// The bytes of WebAssembly for `count` copies of `unit` and a return, at full look-ahead.
fn run_bytes_x86_64(unit: &[u8], count: usize, optimise: Optimise) -> usize {
    let mut blob = unit.repeat(count);
    blob.push(RETURN);
    let translation = translate::<X86_64>(
        &blob,
        0..blob.len() as u64,
        None,
        &[0],
        LookAhead::Full,
        optimise,
        wcontainers_translator::ProgressCount::Absent,
        wcontainers_translator::AccessOrdering::Unordered,
        HOST_X86_64,
        Offer::default(),
    )
    .unwrap();
    for module in &translation.modules {
        validate(&module.wasm);
    }
    translation.modules.iter().map(|m| m.wasm.len()).sum()
}

/// Each repeated string instruction of a run adds about as much code as the first: the code after one is
/// placed once, not once for each way out of it.
#[test]
fn a_run_of_repeated_strings_grows_linearly_x86_64() {
    for unit in REPEATED_STRINGS {
        for optimise in [Optimise::Speed, Optimise::Size] {
            let first = run_bytes_x86_64(unit, 2, optimise) - run_bytes_x86_64(unit, 1, optimise);
            let last = run_bytes_x86_64(unit, LONG_RUN + 1, optimise)
                - run_bytes_x86_64(unit, LONG_RUN, optimise);
            assert!(
                last <= 2 * first,
                "{unit:02x?} for {optimise:?}: the second adds {first} bytes, the {}th {last}",
                LONG_RUN + 1
            );
        }
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn code_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64), kept in any::<bool>(), size in any::<bool>()) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let starts: Vec<u64> = (0..words.len() as u64).map(|i| 4 * i).collect();
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, None, &starts, LookAhead::None, optimise(size), count(kept), AccessOrdering::Unordered, HOST, ()).unwrap();
        validate(&translation.modules[0].wasm);
    }

    /// Random words at full look-ahead, from the first word and from a few others.
    #[test]
    fn full_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64), picks in prop::collection::vec(any::<prop::sample::Index>(), 0..4), kept in any::<bool>(), size in any::<bool>()) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut starts = vec![0];
        starts.extend(picks.iter().map(|p| 4 * p.index(words.len()) as u64));
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, None, &starts, LookAhead::Full, optimise(size), count(kept), AccessOrdering::Unordered, HOST, ()).unwrap();
        validate(&translation.modules[0].wasm);
    }

    /// Random bytes translated from every byte address at look-ahead none: x86 code may start anywhere. A
    /// piece for every start but those whose instruction runs past the end of the bytes.
    #[test]
    fn code_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), v3 in any::<bool>(), kept in any::<bool>(), size in any::<bool>()) {
        let starts: Vec<u64> = (0..bytes.len() as u64).collect();
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, None, &starts, LookAhead::None, optimise(size), count(kept), AccessOrdering::Unordered, HOST_X86_64, told(v3)).unwrap();
        prop_assert_eq!(&translation.modules[0].pieces, &whole_x86_64(&bytes, &starts));
        validate(&translation.modules[0].wasm);
    }

    /// Random bytes at full look-ahead, from the first byte and from a few others.
    #[test]
    fn full_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), picks in prop::collection::vec(any::<prop::sample::Index>(), 0..4), v3 in any::<bool>(), kept in any::<bool>(), size in any::<bool>()) {
        let mut starts = vec![0];
        starts.extend(picks.iter().map(|p| p.index(bytes.len()) as u64));
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, None, &starts, LookAhead::Full, optimise(size), count(kept), AccessOrdering::Unordered, HOST_X86_64, told(v3)).unwrap();
        for module in &translation.modules {
            validate(&module.wasm);
        }
    }

    /// Random bytes at bounded look-ahead of random N, from a few byte addresses: one piece in the table each.
    #[test]
    fn bounded_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), picks in prop::collection::vec(any::<prop::sample::Index>(), 1..4), bound in 1..80u32, v3 in any::<bool>(), kept in any::<bool>(), size in any::<bool>()) {
        let mut starts: Vec<u64> = picks.iter().map(|p| p.index(bytes.len()) as u64).collect();
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, None, &starts, LookAhead::Bounded(bound), optimise(size), count(kept), AccessOrdering::Unordered, HOST_X86_64, told(v3)).unwrap();
        starts.sort();
        starts.dedup();
        prop_assert_eq!(&translation.modules[0].pieces, &whole_x86_64(&bytes, &starts));
        validate(&translation.modules[0].wasm);
    }

    /// Random bytes after `mov` of a 32-bit immediate, `push` of one and `movabs`, each naming through the
    /// link address a random byte of the code, the byte below it or the one past it: valid modules at full and
    /// at bounded look-ahead, and at full a piece at each named byte where a whole instruction starts.
    #[test]
    fn absolute_entries_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), named in prop::collection::vec((any::<u8>(), any::<prop::sample::Index>()), 1..4), bound in 1..80u32, kept in any::<bool>(), size in any::<bool>()) {
        let forms: Vec<Absolute> = named.iter().map(|&(drawn, _)| Absolute::drawn(drawn)).collect();
        let length = forms.iter().map(|f| f.length()).sum::<usize>() + bytes.len();
        let targets: Vec<i64> = named.iter().map(|(_, at)| at.index(length + 2) as i64 - 1).collect();
        let mut code = Vec::new();
        for (form, &target) in forms.iter().zip(&targets) {
            form.encode(LOAD.wrapping_add_signed(target), &mut code);
        }
        code.extend(&bytes);
        let starts: Vec<u64> = (0..code.len() as u64).filter(|&t| X86_64::decode(&code[t as usize..]).is_some()).collect();
        for look_ahead in [LookAhead::Full, LookAhead::Bounded(bound)] {
            let translation = translate::<X86_64>(&code, 0..code.len() as u64, Some(LOAD), &[0], look_ahead, optimise(size), count(kept), AccessOrdering::Unordered, HOST_X86_64, Offer::default()).unwrap();
            for module in &translation.modules {
                validate(&module.wasm);
            }
            if look_ahead == LookAhead::Full {
                let pieces: Vec<u64> = translation.pieces().collect();
                for target in targets.iter().filter_map(|&t| u64::try_from(t).ok()).filter(|t| starts.contains(t)) {
                    prop_assert!(pieces.contains(&target), "no piece at {target} of {code:02x?}");
                }
            }
        }
    }

    /// Random words at bounded look-ahead of random N, from a few starts: one piece in the table each.
    #[test]
    fn bounded_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64), picks in prop::collection::vec(any::<prop::sample::Index>(), 1..4), bound in 1..80u32, kept in any::<bool>(), size in any::<bool>()) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut starts: Vec<u64> = picks.iter().map(|p| 4 * p.index(words.len()) as u64).collect();
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, None, &starts, LookAhead::Bounded(bound), optimise(size), count(kept), AccessOrdering::Unordered, HOST, ()).unwrap();
        starts.sort();
        starts.dedup();
        prop_assert_eq!(&translation.modules[0].pieces, &starts);
        validate(&translation.modules[0].wasm);
    }
}

/// Words of every form of the CPU we present, each translated alone at look-ahead none and with others
/// at full, on a host that needs every fix-up: valid modules. `WC_VALID_WORDS` words a form (default 16).
#[test]
fn every_form_is_valid_arm64() {
    use proptest::test_runner::{RngAlgorithm, TestRng};
    let count: u32 = std::env::var("WC_VALID_WORDS").map_or(16, |v| v.parse().unwrap());
    let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[7; 32]);
    let mut words = Vec::new();
    for form in wcontainers_arm64::forms::all() {
        for _ in 0..count {
            let word = form.word(rng.random());
            if form.presented(word) {
                words.push(word);
            }
        }
    }
    let mut failures = Vec::new();
    std::panic::set_hook(Box::new(|_| {}));
    for &word in &words {
        let translated = std::panic::catch_unwind(|| {
            translate::<Arm64>(
                &word.to_le_bytes(),
                0..4,
                None,
                &[0],
                LookAhead::None,
                Optimise::Speed,
                wcontainers_translator::ProgressCount::Absent,
                wcontainers_translator::AccessOrdering::Unordered,
                HOST,
                (),
            )
            .unwrap()
        });
        let translation = match translated {
            Ok(t) => t,
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                failures.push(format!("{word:#010x}: panicked: {message}"));
                continue;
            }
        };
        if let Err(error) = check(&translation.modules[0].wasm) {
            failures.push(format!(
                "{word:#010x} {:?}: {error}",
                wcontainers_arm64::decode(word)
            ));
        }
    }
    let _ = std::panic::take_hook();
    failures.sort();
    failures.dedup_by(|a, b| a.split(':').nth(1) == b.split(':').nth(1));
    assert!(
        failures.is_empty(),
        "{} invalid:\n{}",
        failures.len(),
        failures.join("\n")
    );
    for chunk in words.chunks(64) {
        let blob: Vec<u8> = chunk.iter().flat_map(|w| w.to_le_bytes()).collect();
        let starts: Vec<u64> = (0..chunk.len() as u64).map(|i| 4 * i).collect();
        for look_ahead in [LookAhead::None, LookAhead::Bounded(16), LookAhead::Full] {
            for count in COUNTS {
                let translation = translate::<Arm64>(
                    &blob,
                    0..blob.len() as u64,
                    None,
                    &starts,
                    look_ahead,
                    Optimise::Speed,
                    count,
                    AccessOrdering::Unordered,
                    HOST,
                    (),
                )
                .unwrap();
                validate(&translation.modules[0].wasm);
            }
        }
    }
}

/// How many times in a row each instruction of `every_form_is_valid_x86_64` is written, so that a meaning that
/// does not end its piece runs on into more of the same.
const REPEATS: usize = 3;

/// Every layer 1 form of x86-64, a form beyond the default list told a list that has it: `WC_VALID_WORDS` cases
/// a form (default 16) seeded from `WC_VALID_SEED` (default 1) and the form's name, each instruction written
/// [`REPEATS`] times in a row and translated at none, bounded and full on a host that needs every fix-up:
/// valid modules.
#[test]
fn every_form_is_valid_x86_64() {
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::TestRunner;
    use wcontainers_translator_test::guest::GuestTest;
    let setting = |name: &str, default: u64| {
        std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
    };
    let count = setting("WC_VALID_WORDS", 16);
    let seed = setting("WC_VALID_SEED", 1);
    let mut failures = Vec::new();
    let mut cases = 0;
    for form in X86_64::forms() {
        let name = X86_64::form_name(form);
        let mut runner = TestRunner::new_with_rng(
            Config::default(),
            wcontainers_translator_test::seeded::generator(seed, &[], name),
        );
        let strategy = X86_64::cases(form);
        for _ in 0..count {
            let case = strategy.new_tree(&mut runner).expect("a case").current();
            let one = case.code.code();
            let code = one.repeat(REPEATS);
            cases += 1;
            for (look_ahead, count) in [LookAhead::None, LookAhead::Bounded(16), LookAhead::Full]
                .into_iter()
                .flat_map(|look_ahead| COUNTS.map(|count| (look_ahead, count)))
            {
                for ordering in FORMS {
                    let translation = translate::<X86_64>(
                        &code,
                        0..code.len() as u64,
                        None,
                        &[0],
                        look_ahead,
                        Optimise::Speed,
                        count,
                        ordering,
                        HOST_X86_64,
                        X86_64::case_offer(&case),
                    )
                    .unwrap();
                    let at = format!("{name} {one:02x?} at {look_ahead:?} {count:?} {ordering:?}");
                    for module in &translation.modules {
                        if let Err(error) = check(&module.wasm) {
                            failures.push(format!("{at}: {error}"));
                        }
                        if ordering != AccessOrdering::Unordered {
                            for left in scan(&module.wasm, ordering).wrong {
                                failures.push(format!("{at}: {left}"));
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("{cases} cases of {} forms", X86_64::forms().len());
    failures.dedup_by(|a, b| a.split(' ').next() == b.split(' ').next());
    assert!(
        failures.is_empty(),
        "{} invalid:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
