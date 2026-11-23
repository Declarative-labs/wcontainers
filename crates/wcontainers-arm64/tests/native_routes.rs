//! Every decoded form has one route, and the route agrees with what the form's meaning touches: a form that is
//! copied touches only what the hardware holds as presented, so no copied instruction can observe the host,
//! and an access the hardware makes has the operand its fault is resolved from (docs/native.md section 3.4,
//! the static test). Seeded: `WCONTAINERS_ROUTES_SEED` and
//! `WCONTAINERS_ROUTES_WORDS`, the words drawn per form, both decimal.

use std::collections::BTreeMap;

use wasm_encoder::{InstructionSink, ValType};
use wasmparser::{BinaryReader, Operator, OperatorsReader};
use wcontainers_arm64::{Arm64, Control, Hook, Host, Reg, Trap, decode, forms};
use wcontainers_guest_cpu::{
    Access, Base, Emitter, Guest, Native, NativeClass, Route, Stopping, Target,
};

mod common;
use common::{SWEPT_BITS, WORDS_PER_FORM, knob, words_of};

const SEED: u64 = 0x2545_f491_4f6c_dd1d;
const SEED_VARIABLE: &str = "WCONTAINERS_ROUTES_SEED";
const WORDS_VARIABLE: &str = "WCONTAINERS_ROUTES_WORDS";

/// The hardware's own answer to what WebAssembly leaves open: it fuses and follows Arm's not-a-numbers.
const HARDWARE: Host = Host {
    fused_multiply_add: true,
    arm_nans: true,
};

/// What one meaning touches.
#[derive(Default)]
struct Seen {
    /// Any register, flag or vector read or written.
    registers: bool,
    this_address: bool,
    memory: Vec<Access>,
    hooks: Vec<(Hook, Option<i64>)>,
    controls: Vec<(Control, bool)>,
    syscall: bool,
    traps: Vec<Trap>,
    transfer: bool,
    raise_stop: bool,
}

struct Recorder {
    seen: Seen,
    bytes: Vec<u8>,
    /// Where the bytes since the last hook start.
    since_hook: usize,
    locals: u32,
}

impl Recorder {
    /// The first `i32.const` written since the last hook: the hook's first argument where it is a constant.
    fn first_constant(&self) -> Option<i64> {
        let reader = BinaryReader::new(&self.bytes[self.since_hook..], 0);
        let mut operators = OperatorsReader::new(reader);
        while !operators.eof() {
            match operators.read() {
                Ok(Operator::I32Const { value }) => return Some(value.into()),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        None
    }
}

impl Emitter<Arm64> for Recorder {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.bytes)
    }
    fn local(&mut self, _ty: ValType) -> u32 {
        self.locals += 1;
        self.locals
    }
    fn get(&mut self, _r: <Arm64 as Guest>::Reg) {
        self.seen.registers = true;
    }
    fn set(&mut self, _r: <Arm64 as Guest>::Reg) {
        self.seen.registers = true;
    }
    fn this_address(&mut self, _by: i64) {
        self.seen.this_address = true;
    }
    fn set_flags(&mut self, _flags: <Arm64 as Guest>::Flags) {
        self.seen.registers = true;
    }
    fn flag(&mut self, _flag: <Arm64 as Guest>::Flag) {
        self.seen.registers = true;
    }
    fn get_vector(&mut self, _v: u8) {
        self.seen.registers = true;
    }
    fn set_vector(&mut self, _v: u8) {
        self.seen.registers = true;
    }
    fn get_control(&mut self, control: Control) {
        self.seen.controls.push((control, false));
    }
    fn set_control(&mut self, control: Control) {
        self.seen.controls.push((control, true));
    }
    fn hook(&mut self, hook: Hook) {
        let first = self.first_constant();
        self.seen.hooks.push((hook, first));
        self.since_hook = self.bytes.len();
    }
    fn host(&self) -> Host {
        HARDWARE
    }
    fn offer(&self) {}
    fn check(&mut self, _address: u32, _bytes: u32, access: Access, _which: u8) {
        self.seen.memory.push(access);
    }
    fn check_range(&mut self, address: u32, _length: u32, access: Access) -> u32 {
        self.seen.memory.push(access);
        address
    }
    fn warn(&mut self, _reason: <Arm64 as Guest>::Reason) {}
    fn raise_stop(&mut self) {
        self.seen.raise_stop = true;
    }
    fn jump(&mut self, _target: Target) {
        self.seen.transfer = true;
    }
    fn jump_if(&mut self, _target: Target) {
        self.seen.transfer = true;
    }
    fn call(&mut self, _target: Target) {
        self.seen.transfer = true;
    }
    fn ret(&mut self, _target: u32) {
        self.seen.transfer = true;
    }
    fn syscall(&mut self) {
        self.seen.syscall = true;
    }
    fn trap(&mut self, trap: Trap) {
        self.seen.traps.push(trap);
    }
    fn next(&mut self) {}
}

fn seen(word: u32) -> Seen {
    let mut recorder = Recorder {
        seen: Seen::default(),
        bytes: Vec::new(),
        since_hook: 0,
        locals: 0,
    };
    Arm64::emit(&mut recorder, &decode(word));
    recorder.seen
}

/// Routes that carry a guest memory access.
const ACCESS_ROUTES: [Route; 7] = [
    Route::Copy,
    Route::Split,
    Route::Collision,
    Route::Aligned,
    Route::Rebase,
    Route::Exclusive,
    Route::Kernel,
];

/// Routes whose host instruction is the guest's own, run as it is.
const COPIES: [Route; 4] = [Route::Copy, Route::Split, Route::Collision, Route::Aligned];

/// Whether an access of the instruction is based on the stack pointer, whose alignment the hardware checks
/// as the meaning does (SCTLR_EL1.SA0 is set from `Guest::SP_ALIGNMENT_CHECKED`).
fn stack_based(word: u32) -> bool {
    Arm64::accesses(&decode(word))
        .iter()
        .flatten()
        .any(|a| a.base == Some(Base::Register(Reg::Sp)))
}

/// Why a route does not fit what the meaning touches, if it does not.
fn mismatch(route: Route, seen: &Seen, word: u32) -> Option<String> {
    let mut routed: Vec<Route> = Vec::new();
    for &(hook, first) in &seen.hooks {
        match Arm64::hook_class(hook, first) {
            NativeClass::Routed(r) => routed.push(r),
            NativeClass::Hardware | NativeClass::Bookkeeping => {}
        }
    }
    for &(control, write) in &seen.controls {
        match Arm64::control_class(control, write) {
            NativeClass::Routed(r) => routed.push(r),
            NativeClass::Hardware | NativeClass::Bookkeeping => {}
        }
    }
    // The inside answers a form routed to it wholly, as its meaning does, whatever state that keeps.
    if let Some(&other) = routed
        .iter()
        .find(|&&r| r != route && route != Route::Kernel)
    {
        return Some(format!("touches what the {other:?} route keeps"));
    }
    if seen.syscall && route != Route::Kernel {
        return Some("a system call not routed to the inside".into());
    }
    let unconditional = seen.traps.iter().any(|t| !matches!(t, Trap::Alignment(_)));
    if unconditional && route != Route::Trap {
        return Some("a trap not routed as a trap".into());
    }
    if seen.traps.iter().any(|t| matches!(t, Trap::Alignment(_)))
        && !matches!(route, Route::Aligned | Route::Exclusive)
        && !(stack_based(word) && Arm64::SP_ALIGNMENT_CHECKED)
    {
        return Some("an alignment trap without the alignment test".into());
    }
    if seen.transfer && route != Route::Transfer {
        return Some("a transfer not routed as one".into());
    }
    if seen.this_address && !matches!(route, Route::Rebase | Route::Transfer) {
        return Some("reads its own address but is not rebuilt on it".into());
    }
    if seen.raise_stop {
        return Some("raises the stop, which no ARM64 route answers".into());
    }
    if !seen.memory.is_empty() && !ACCESS_ROUTES.contains(&route) {
        return Some("a memory access on a route that makes none".into());
    }
    // Every route but the inside's own runs the access on the hardware, whose fault the inside resolves from
    // the access's operand.
    if !seen.memory.is_empty()
        && route != Route::Kernel
        && !matches!(Arm64::stopping(&decode(word), 0), Stopping::Access(_))
    {
        return Some("a memory access whose fault has no operand to resolve it".into());
    }
    let touches = seen.registers
        || seen.this_address
        || !seen.memory.is_empty()
        || !seen.hooks.is_empty()
        || !seen.controls.is_empty()
        || seen.syscall
        || !seen.traps.is_empty()
        || seen.transfer;
    if touches == (route == Route::Nothing) {
        return Some(if touches {
            "routed as nothing but its meaning does something".into()
        } else {
            "its meaning does nothing but it is not routed as nothing".into()
        });
    }
    if COPIES.contains(&route) && !routed.is_empty() {
        return Some("copied but observes the host".into());
    }
    None
}

#[test]
fn every_decoded_form_has_one_fitting_route() {
    let seed = knob(SEED_VARIABLE, SEED);
    let draws = knob(WORDS_VARIABLE, WORDS_PER_FORM);
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut routes: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let (mut decoded, mut forms_seen) = (0u64, 0u64);
    for form in forms::all() {
        let mut any = false;
        for word in words_of(form, draws, &mut rng) {
            if !forms::form(word).is_some_and(|f| std::ptr::eq(f, form)) {
                continue;
            }
            any = true;
            decoded += 1;
            let route = Arm64::route(&(), &decode(word), &());
            *routes.entry(format!("{route:?}")).or_default() += 1;
            if let Some(why) = mismatch(route, &seen(word), word) {
                failures.push(format!(
                    "{} {word:#010x} {:?} as {route:?}: {why}",
                    form.name,
                    decode(word)
                ));
            }
        }
        forms_seen += u64::from(any);
    }
    println!(
        "seed {seed}, {draws} words drawn per form (forms with at most {SWEPT_BITS} other free bits swept whole): {decoded} decoded words of {forms_seen} forms; routes {routes:?}"
    );
    let shown = failures.len().min(usize::from(u8::MAX));
    assert!(
        failures.is_empty(),
        "{} words routed against their meaning (seed {seed}):\n{}",
        failures.len(),
        failures[..shown].join("\n")
    );
}
