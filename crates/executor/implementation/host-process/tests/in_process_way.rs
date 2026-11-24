//! The in-process way's translated code (docs/native-without-virtual-machine.md section 3.2 item 1) under
//! unicorn-engine, against the same guest instruction run there at the guest's own addresses: every presented form
//! that reaches memory or writes or reads the stack pointer, with seeded registers near the window's edges, outside
//! it, tagged, and in the stack. The translated code runs in slot 1's window; the guest's pages at their own
//! addresses and in slot 2's window are mapped too, and touching any of them fails the case, as does any access
//! outside slot 1's window and the unmapped 4 GiB beside it. Outcomes must agree: the same registers, flags,
//! vector registers and memory; a fault at the same guest address; an access the reference faults on outside the
//! window reaching the mark; an alignment fault reaching the floor's misalignment entry; a stack pointer written
//! outside the window reaching the mark; a stack probe faulting only where the reference's stack pointer left the
//! mapped stack. Stops are not resolved here (the stop crate does not know the way yet): a fault's registers are
//! not compared.
//!
//! Knobs: `WCHP_IN_PROCESS_CASES` cases per form, `WCHP_SEED`.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use unicorn_engine::{Arch, Arm64CpuModel, HookType, Mode, Prot, RegisterARM64, Unicorn};
use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_arm64::thread_state::{COUNT, PAGE, RegisterPage, SCRATCH, TAG_LOW, WITHHELD};
use wcontainers_arm64::way::{
    BASE, CHECK, IN_PROCESS_ENTRIES, IN_PROCESS_WITHHELD_AT, OUTSIDE, PROCESS_ENTRY_BYTES,
    WINDOW_BITS, Way,
};
use wcontainers_arm64::{Address, Arm64, Instruction as I, Reg, decode, forms};
use wcontainers_guest_cpu::{Guest, InsideCall, Native, Route};
use wcontainers_translator::{LookAhead, NativeRequest, translate_native_request};

const CASES_VARIABLE: &str = "WCHP_IN_PROCESS_CASES";
const SEED_VARIABLE: &str = "WCHP_SEED";
/// Cases per form unless told otherwise.
const DEFAULT_CASES: u64 = 4;
/// The seed unless told otherwise.
const DEFAULT_SEED: u64 = 0x2545_f491_4f6c_dd1d;
/// Random words tried per case wanted before a form is given up.
const TRIES: u64 = 64;

/// The slot whose window the translated code runs in, and another whose window it must never reach.
const SLOT: u16 = 1;
const OTHER_SLOT: u16 = 2;
/// The general registers, x0 to x30.
const GENERAL: usize = 31;
/// The vector registers.
const VECTORS: usize = 32;
/// Where the guest's instruction is.
const CODE: u64 = 0x40000;
/// The top of the guest's mapped stack, and its pages.
const STACK_TOP: u64 = 0x90000;
const STACK_PAGES: u64 = 16;
/// The window's top pages, just under its ceiling.
const TOP: u64 = (1 << WINDOW_BITS) - LOWEST_ADDRESS - 2 * PAGE_SIZE;
/// The guest's mapped regions: start, pages, permissions.
const REGIONS: [(u64, u64, Prot); 5] = [
    (LOWEST_ADDRESS, 2, Prot(Prot::READ.0 | Prot::WRITE.0)),
    (LOWEST_ADDRESS + 2 * PAGE_SIZE, 1, Prot::READ),
    (CODE, 1, Prot(Prot::READ.0 | Prot::EXEC.0)),
    (
        STACK_TOP - STACK_PAGES * PAGE_SIZE,
        STACK_PAGES,
        Prot(Prot::READ.0 | Prot::WRITE.0),
    ),
    (TOP, 2, Prot(Prot::READ.0 | Prot::WRITE.0)),
];
/// Where the translated code and the thread's register page are, in our area.
const HOST_CODE: u64 = (1 << 45) + (1 << 40);
const REGISTER_PAGE: u64 = (1 << 45) + (1 << 41);
/// The most instructions a translated case runs.
const STEPS: usize = 10_000;

/// A seeded case: an instruction and the guest's state before it.
#[derive(Clone, Debug)]
struct Case {
    word: u32,
    x: [u64; GENERAL],
    sp: u64,
    nzcv: u64,
    q: [u128; VECTORS],
    /// A tag the translated side's base register carries in its top byte, which the reference's does not.
    tag: Option<(usize, u64)>,
    /// Each region's bytes.
    memory: Vec<Vec<u8>>,
}

/// The guest's state after an instruction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    x: [u64; GENERAL],
    sp: u64,
    nzcv: u64,
    q: [u128; VECTORS],
    memory: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Outcome {
    Done(Box<State>),
    /// A fault at a guest address.
    Fault(u64),
    /// The emulator's exception, by its number.
    Exception(u32),
    /// The in-process way's mark.
    Mark,
    /// A call into the floor.
    Call(InsideCall),
    /// An access outside the slot's window and the 4 GiB beside it, or to a canary: never allowed.
    Stray(u64),
}

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

fn x_register(n: usize) -> i32 {
    match n {
        0..=28 => RegisterARM64::X0 as i32 + n as i32,
        29 => RegisterARM64::X29 as i32,
        30 => RegisterARM64::X30 as i32,
        _ => panic!("no general register x{n}"),
    }
}

fn q_register(n: usize) -> i32 {
    RegisterARM64::Q0 as i32 + n as i32
}

fn emulator() -> Unicorn<'static, ()> {
    let mut unicorn = Unicorn::new(Arch::ARM64, Mode::ARM).expect("an ARM64 emulator");
    unicorn
        .ctl_set_cpu_model(Arm64CpuModel::MAX as i32)
        .expect("the most capable CPU model");
    unicorn
}

/// Maps the guest's regions with the case's bytes, `offset` bytes past their guest addresses.
fn map_regions(unicorn: &mut Unicorn<'static, ()>, case: &Case, offset: u64) {
    for ((start, pages, prot), bytes) in REGIONS.iter().zip(&case.memory) {
        unicorn
            .mem_map(offset + start, pages * PAGE_SIZE, *prot)
            .expect("a region");
        unicorn.mem_write(offset + start, bytes).expect("its bytes");
    }
}

fn read_regions(unicorn: &Unicorn<'static, ()>, offset: u64) -> Vec<Vec<u8>> {
    REGIONS
        .iter()
        .map(|(start, pages, _)| {
            (unicorn.mem_read_as_vec(offset + start, (pages * PAGE_SIZE) as usize))
                .expect("a region's bytes")
        })
        .collect()
}

fn read_vectors(unicorn: &Unicorn<'static, ()>) -> [u128; VECTORS] {
    std::array::from_fn(|n| {
        let bytes = unicorn
            .reg_read_long(q_register(n))
            .expect("a vector register");
        u128::from_le_bytes(bytes[..size_of::<u128>()].try_into().expect("16 bytes"))
    })
}

fn write_vectors(unicorn: &Unicorn<'static, ()>, q: &[u128; VECTORS]) {
    for (n, value) in q.iter().enumerate() {
        (unicorn.reg_write_long(q_register(n), &value.to_le_bytes())).expect("a vector register");
    }
}

/// The first fault a run met, kept by its memory hook.
type Faults = Rc<RefCell<Option<Outcome>>>;

fn first(faults: &Faults, outcome: Outcome) {
    faults.borrow_mut().get_or_insert(outcome);
}

/// The guest instruction run at the guest's own addresses.
fn reference(case: &Case) -> Outcome {
    let mut unicorn = emulator();
    map_regions(&mut unicorn, case, 0);
    unicorn
        .mem_write(CODE, &case.word.to_le_bytes())
        .expect("the instruction");
    for (n, &value) in case.x.iter().enumerate() {
        unicorn.reg_write(x_register(n), value).expect("a register");
    }
    unicorn.reg_write(RegisterARM64::SP, case.sp).expect("sp");
    unicorn
        .reg_write(RegisterARM64::NZCV, case.nzcv)
        .expect("flags");
    write_vectors(&unicorn, &case.q);
    let faults: Faults = Rc::default();
    let into = faults.clone();
    unicorn
        .add_mem_hook(HookType::MEM_INVALID, 1, 0, move |_, _, address, _, _| {
            first(&into, Outcome::Fault(address));
            false
        })
        .expect("the fault hook");
    let into = faults.clone();
    unicorn
        .add_intr_hook(move |unicorn, number| {
            first(&into, Outcome::Exception(number));
            unicorn.emu_stop().expect("stopped");
        })
        .expect("the exception hook");
    let ran = unicorn.emu_start(CODE, CODE + Arm64::MAX_LENGTH, 0, 1);
    if let Some(outcome) = faults.take() {
        return outcome;
    }
    if ran.is_err() {
        return Outcome::Exception(u32::MAX);
    }
    Outcome::Done(Box::new(State {
        x: std::array::from_fn(|n| unicorn.reg_read(x_register(n)).expect("a register")),
        sp: unicorn.reg_read(RegisterARM64::SP).expect("sp"),
        nzcv: unicorn.reg_read(RegisterARM64::NZCV).expect("flags"),
        q: read_vectors(&unicorn),
        memory: read_regions(&unicorn, 0),
    }))
}

/// The register page's word that keeps withheld register `r`'s guest value in the in-process way.
fn withheld_word(way: Way, r: u8) -> u64 {
    let k = way
        .withheld()
        .iter()
        .position(|&w| w == r)
        .expect("withheld");
    if k < WITHHELD.len() {
        (core::mem::offset_of!(RegisterPage, withheld) + k * size_of::<u64>()) as u64
    } else {
        (IN_PROCESS_WITHHELD_AT + (k - WITHHELD.len()) * size_of::<u64>()) as u64
    }
}

/// The case's instruction translated for the in-process way and run in slot [`SLOT`]'s window.
fn translated(case: &Case) -> Outcome {
    let way = Way::InProcess { slot: SLOT };
    let base = way.window_base();
    let other = Way::InProcess { slot: OTHER_SLOT }.window_base();
    let request = NativeRequest {
        look_ahead: LookAhead::None,
        exact_replay: false,
        stop: false,
        step: false,
    };
    let code =
        translate_native_request::<Arm64>(&case.word.to_le_bytes(), CODE, &request, &way, &())
            .expect("translated");
    let mut unicorn = emulator();
    map_regions(&mut unicorn, case, base);
    unicorn
        .mem_write(base + CODE, &case.word.to_le_bytes())
        .expect("the instruction");
    let faults: Faults = Rc::default();
    // The guest's pages at their own addresses and in another window: reaching one is a stray access.
    for offset in [0, other] {
        map_regions(&mut unicorn, case, offset);
        for (start, pages, _) in REGIONS {
            let into = faults.clone();
            unicorn
                .add_mem_hook(
                    HookType::MEM_VALID,
                    offset + start,
                    offset + start + pages * PAGE_SIZE - 1,
                    move |unicorn, _, address, _, _| {
                        first(&into, Outcome::Stray(address));
                        unicorn.emu_stop().expect("stopped");
                        true
                    },
                )
                .expect("a canary hook");
        }
    }
    let length = (code.bytes.len() as u64).next_multiple_of(PAGE_SIZE);
    unicorn
        .mem_map(HOST_CODE, length, Prot::ALL)
        .expect("the code");
    unicorn
        .mem_write(HOST_CODE, &code.bytes)
        .expect("its bytes");
    unicorn
        .mem_map(REGISTER_PAGE, PAGE_SIZE, Prot(Prot::READ.0 | Prot::WRITE.0))
        .expect("the register page");
    // The guest's registers, the tag on its base.
    let guest: [u64; GENERAL] = std::array::from_fn(|n| match case.tag {
        Some((t, tag)) if t == n => case.x[n] | tag << TAG_LOW,
        _ => case.x[n],
    });
    for &r in way.withheld() {
        let at = REGISTER_PAGE + withheld_word(way, r);
        unicorn
            .mem_write(at, &guest[usize::from(r)].to_le_bytes())
            .expect("a withheld register's guest value");
    }
    let next_slot = way.slot(CODE + Arm64::MAX_LENGTH);
    let entries = IN_PROCESS_ENTRIES & !(PAGE_SIZE - 1);
    for page in [next_slot & !(PAGE_SIZE - 1), entries] {
        unicorn
            .mem_map(page, PAGE_SIZE, Prot::ALL)
            .expect("a landing page");
    }
    let into = faults.clone();
    unicorn
        .add_code_hook(next_slot, next_slot, move |unicorn, _, _| {
            first(&into, Outcome::Done(Box::default()));
            unicorn.emu_stop().expect("stopped");
        })
        .expect("the exit's hook");
    let into = faults.clone();
    unicorn
        .add_code_hook(
            entries,
            entries + PAGE_SIZE - 1,
            move |unicorn, address, _| {
                let code = (address - IN_PROCESS_ENTRIES) / PROCESS_ENTRY_BYTES;
                let call = u16::try_from(code).ok().and_then(InsideCall::from_code);
                first(&into, call.map_or(Outcome::Stray(address), Outcome::Call));
                unicorn.emu_stop().expect("stopped");
            },
        )
        .expect("the entries' hook");
    let into = faults.clone();
    unicorn
        .add_mem_hook(HookType::MEM_INVALID, 1, 0, move |_, _, address, _, _| {
            let guest = address.wrapping_sub(base);
            first(
                &into,
                if address == base + OUTSIDE {
                    Outcome::Mark
                } else if guest < 1 << WINDOW_BITS
                    || guest.wrapping_neg() <= 1 << WINDOW_BITS
                    || (guest >> WINDOW_BITS) == 1
                {
                    Outcome::Fault(guest)
                } else {
                    Outcome::Stray(address)
                },
            );
            false
        })
        .expect("the fault hook");
    let into = faults.clone();
    unicorn
        .add_intr_hook(move |unicorn, number| {
            first(&into, Outcome::Exception(number));
            unicorn.emu_stop().expect("stopped");
        })
        .expect("the exception hook");
    for (n, &guest_value) in guest.iter().enumerate() {
        let r = n as u8;
        let value = match r {
            _ if r == BASE => base,
            _ if r == PAGE => REGISTER_PAGE,
            _ if r == COUNT => 0,
            _ if r == SCRATCH || r == CHECK => u64::MAX,
            _ => guest_value,
        };
        unicorn.reg_write(x_register(n), value).expect("a register");
    }
    unicorn
        .reg_write(RegisterARM64::SP, base + case.sp)
        .expect("sp");
    unicorn
        .reg_write(RegisterARM64::NZCV, case.nzcv)
        .expect("flags");
    write_vectors(&unicorn, &case.q);
    let ran = unicorn.emu_start(HOST_CODE, 0, 0, STEPS);
    let outcome = faults.take();
    match outcome {
        Some(Outcome::Done(_)) => {}
        Some(outcome) => return outcome,
        None => panic!("the translated code ended without an outcome: {ran:?}"),
    }
    let mut page = vec![0; PAGE_SIZE as usize];
    unicorn
        .mem_read(REGISTER_PAGE, &mut page)
        .expect("the register page");
    let word = |at: u64| {
        let at = at as usize;
        u64::from_le_bytes(page[at..at + size_of::<u64>()].try_into().expect("a word"))
    };
    Outcome::Done(Box::new(State {
        x: std::array::from_fn(|n| {
            if way.withheld().contains(&(n as u8)) {
                word(withheld_word(way, n as u8))
            } else {
                unicorn.reg_read(x_register(n)).expect("a register")
            }
        }),
        sp: unicorn
            .reg_read(RegisterARM64::SP)
            .expect("sp")
            .wrapping_sub(base),
        nzcv: unicorn.reg_read(RegisterARM64::NZCV).expect("flags"),
        q: read_vectors(&unicorn),
        memory: read_regions(&unicorn, base),
    }))
}

/// Whether the instruction is one the in-process way changes: an access, a cache or zeroing operation by address,
/// or an instruction naming the stack pointer as a value.
fn wanted(i: &I) -> bool {
    let route = Arm64::route(&Way::InProcess { slot: SLOT }, i, &());
    match route {
        // A collision's result is the meaning's choice, which the emulator need not make.
        Route::Collision => false,
        Route::Copy | Route::Split | Route::Aligned | Route::Exclusive => {
            i.memory_access().is_some()
                || matches!(i, I::ZeroBlock { .. } | I::CleanCache { .. })
                || stack_value(i)
        }
        Route::Rebase => matches!(i, I::Load { .. } | I::LoadVector { .. }),
        _ => false,
    }
}

/// Whether the instruction reads or writes the stack pointer as a value.
fn stack_value(i: &I) -> bool {
    match *i {
        I::AddSub { rd, rn, .. } => rd == Reg::Sp || rn == Reg::Sp,
        I::Logical { rd, .. } => rd == Reg::Sp,
        _ => false,
    }
}

fn writes_stack(i: &I) -> bool {
    matches!(
        *i,
        I::AddSub { rd: Reg::Sp, .. }
            | I::Logical { rd: Reg::Sp, .. }
            | I::SimdMemory {
                rn: Reg::Sp,
                post: Some(Reg::X(_)),
                ..
            }
    )
}

/// The general registers an instruction names besides its base, which a tag on the base would reach: data
/// stored, compared or loaded, an index, a post-index.
fn other_registers(i: &I) -> Vec<Reg> {
    let pair = |r: Reg| match r {
        Reg::X(n) => vec![Reg::X(n), Reg::X(n + 1)],
        other => vec![other],
    };
    let mut named = match *i {
        I::Store { rt, .. }
        | I::StoreRelease { rt, .. }
        | I::Load { rt, .. }
        | I::LoadAcquire { rt, .. } => vec![rt],
        I::StorePair { rt, rt2, .. }
        | I::LoadPair { rt, rt2, .. }
        | I::LoadExclusive { rt, rt2, .. } => vec![rt, rt2],
        I::Atomic { rs, rt, .. } => vec![rs, rt],
        I::CompareSwap { rs, rt, .. } => [pair(rs), pair(rt)].concat(),
        I::StoreExclusive { rs, rt, rt2, .. } => vec![rs, rt, rt2],
        I::SimdMemory { post: Some(r), .. } => vec![r],
        _ => Vec::new(),
    };
    if let Some(Address::Indexed { index, .. }) = i.memory_access().map(|m| m.address) {
        named.push(index);
    }
    named
}

/// The fields in which two states differ.
fn differences(expected: &State, got: &State) -> String {
    let mut out = Vec::new();
    for n in 0..GENERAL {
        if expected.x[n] != got.x[n] {
            out.push(format!("x{n} {:#x} not {:#x}", got.x[n], expected.x[n]));
        }
    }
    for n in 0..VECTORS {
        if expected.q[n] != got.q[n] {
            out.push(format!("q{n} {:#x} not {:#x}", got.q[n], expected.q[n]));
        }
    }
    if (expected.sp, expected.nzcv) != (got.sp, got.nzcv) {
        out.push(format!(
            "sp, nzcv {:#x} {:#x} not {:#x} {:#x}",
            got.sp, got.nzcv, expected.sp, expected.nzcv
        ));
    }
    for (k, (a, b)) in expected.memory.iter().zip(&got.memory).enumerate() {
        if let Some(at) = a.iter().zip(b).position(|(a, b)| a != b) {
            out.push(format!("region {k} from byte {at:#x}"));
        }
    }
    out.join(", ")
}

/// The base register of an access, and its address before the immediate on the reference's registers.
fn start_address(i: &I, x: &[u64; GENERAL], sp: u64) -> Option<(Reg, u64)> {
    let value = |r: Reg| match r {
        Reg::X(n) => x[usize::from(n)],
        Reg::Sp => sp,
        Reg::Zero => 0,
    };
    let m = i.memory_access()?;
    Some(match m.address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. } => (base, value(base)),
        Address::Indexed {
            base,
            index,
            extend,
            shift,
        } => {
            let raw = value(index);
            let bits = u32::from(extend.from_bits);
            let low = if bits >= u64::BITS {
                raw
            } else if extend.signed {
                (((raw << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)) as u64
            } else {
                raw & ((1 << bits) - 1)
            };
            (base, value(base).wrapping_add(low << u32::from(shift)))
        }
        Address::Literal { .. } => return None,
    })
}

/// A seeded value for a register: near a mapped region's edges, near the window's edges, outside it, or any.
fn value(rng: &mut fastrand::Rng) -> u64 {
    let window = 1u64 << WINDOW_BITS;
    let near = |rng: &mut fastrand::Rng, at: u64| at.wrapping_add_signed(rng.i64(-512..=512));
    match rng.u32(0..8) {
        0..=2 => {
            let (start, pages, _) = REGIONS[rng.usize(0..REGIONS.len())];
            let page = rng.u64(0..=pages);
            near(rng, start + page * PAGE_SIZE)
        }
        3 => near(rng, window),
        4 => rng.u64(0..LOWEST_ADDRESS),
        5 => near(rng, 0),
        6 => window * rng.u64(1..1 << 8) + rng.u64(0..window),
        _ => rng.u64(..),
    }
}

fn case_for(word: u32, rng: &mut fastrand::Rng) -> Case {
    let i = decode(word);
    let x: [u64; GENERAL] = std::array::from_fn(|_| value(rng));
    let stack_bottom = STACK_TOP - STACK_PAGES * PAGE_SIZE;
    let sp = (stack_bottom + rng.u64(0..STACK_PAGES * PAGE_SIZE)) & !15;
    // A tag on the base register, where nothing stores or compares it as data.
    let tag = match start_address(&i, &x, sp) {
        Some((Reg::X(n), _)) if rng.u32(0..4) == 0 && !other_registers(&i).contains(&Reg::X(n)) => {
            Some((usize::from(n), rng.u64(1..1 << u8::BITS)))
        }
        _ => None,
    };
    Case {
        word,
        x: std::array::from_fn(|n| match tag {
            Some((t, _)) if t == n => x[n] & ((1 << TAG_LOW) - 1),
            _ => x[n],
        }),
        sp,
        nzcv: u64::from(rng.u32(0..16)) << 28,
        q: std::array::from_fn(|_| rng.u128(..)),
        tag,
        memory: REGIONS
            .iter()
            .map(|(_, pages, _)| (0..pages * PAGE_SIZE).map(|_| rng.u8(..)).collect())
            .collect(),
    }
}

/// What the translated outcome must be for the reference's, or why it may differ; None when it agrees.
fn disagreement(
    case: &Case,
    reference: &Outcome,
    translated: &Outcome,
    tally: &mut Tally,
) -> Option<String> {
    let i = decode(case.word);
    let outside = |a: u64| (a & ((1 << TAG_LOW) - 1)) >> WINDOW_BITS != 0;
    match (reference, translated) {
        (_, Outcome::Stray(address)) => Some(format!("reached {address:#x}, outside its window")),
        // The emulator ignores no top byte: an address whose sum carried into it faults there, where Linux's Top
        // Byte Ignore leaves the address below it.
        (Outcome::Fault(a), t) if a >> TAG_LOW != 0 && *t != Outcome::Mark => {
            tally.top_byte += 1;
            None
        }
        (Outcome::Done(r), Outcome::Done(t)) => {
            let mut expected = r.clone();
            if let Some((n, tag)) = case.tag {
                expected.x[n] = expected.x[n].wrapping_add(tag << TAG_LOW);
            }
            (expected != *t).then(|| format!("state differs: {}", differences(&expected, t)))
        }
        (Outcome::Fault(a), Outcome::Fault(b)) => {
            let untag = |v: u64| v & ((1 << TAG_LOW) - 1);
            let (a, b) = (untag(*a), untag(*b));
            // An access across a page whose second page faults: its probe of the last byte faults there, and the
            // stop gives the first byte of that page, which the reference faults at.
            if a != b && a.is_multiple_of(PAGE_SIZE) && (a + 1..a + PAGE_SIZE).contains(&b) {
                tally.crossing += 1;
                return None;
            }
            (a != b).then(|| format!("fault at {b:#x}, the reference's at {a:#x}"))
        }
        (Outcome::Fault(_), Outcome::Mark) => {
            match start_address(&i, &case.x, case.sp) {
                Some((_, a)) if outside(a) => {
                    tally.outside += 1;
                    None
                }
                // A literal outside the window is known when translated.
                None if matches!(Arm64::route(&Way::Machine, &i, &()), Route::Rebase) => {
                    tally.outside += 1;
                    None
                }
                _ => Some("the mark for an access whose address is in the window".into()),
            }
        }
        (Outcome::Exception(_), Outcome::Call(InsideCall::Misaligned)) => {
            tally.misaligned += 1;
            None
        }
        // The CPU we present has no FEAT_LSE2: these forms fault when not aligned, as the meaning has them; the
        // emulator's CPU has it.
        (_, Outcome::Call(InsideCall::Misaligned))
            if matches!(
                Arm64::route(&Way::Machine, &i, &()),
                Route::Aligned | Route::Exclusive
            ) && start_address(&i, &case.x, case.sp).is_some_and(|(_, a)| {
                let bytes = i.memory_access().map_or(1, |m| u64::from(m.bytes));
                !a.is_multiple_of(bytes)
            }) =>
        {
            tally.misaligned += 1;
            None
        }
        // A store exclusive the monitor does not allow is a write access that changes nothing, as the meaning has
        // it, so it faults where the emulator's does not write at all.
        (Outcome::Done(r), Outcome::Fault(_) | Outcome::Mark) if matches!(i, I::StoreExclusive { rs, .. } if rs == Reg::Zero || matches!(rs, Reg::X(n) if r.x[usize::from(n)] == 1)) =>
        {
            tally.unmonitored += 1;
            None
        }
        // Cache maintenance by address needs the address to be readable; the emulator does none.
        (Outcome::Done(_), Outcome::Mark) if matches!(i, I::CleanCache { rn: Reg::X(n) } if outside(case.x[usize::from(n)])) =>
        {
            tally.outside += 1;
            None
        }
        (Outcome::Done(r), Outcome::Mark) if writes_stack(&i) && r.sp >> WINDOW_BITS != 0 => {
            tally.displaced += 1;
            None
        }
        (Outcome::Done(r), Outcome::Fault(_)) if writes_stack(&i) => {
            let bottom = STACK_TOP - STACK_PAGES * PAGE_SIZE;
            if !(bottom..=STACK_TOP).contains(&r.sp) {
                tally.probes += 1;
                None
            } else {
                Some("a stack probe faulted inside the mapped stack".into())
            }
        }
        (Outcome::Exception(_), _) => {
            tally.emulator += 1;
            None
        }
        _ => Some(format!(
            "translated {translated:x?}, reference {reference:x?}"
        )),
    }
}

#[derive(Debug, Default)]
struct Tally {
    cases: u64,
    forms: u64,
    outside: u64,
    misaligned: u64,
    displaced: u64,
    probes: u64,
    emulator: u64,
    unmonitored: u64,
    crossing: u64,
    top_byte: u64,
}

#[test]
fn in_process_way_agrees_with_the_reference_and_stays_in_its_window() {
    let cases = setting(CASES_VARIABLE, DEFAULT_CASES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut tally = Tally::default();
    let mut failures: BTreeMap<String, String> = BTreeMap::new();
    for form in forms::all() {
        let mut found = 0;
        for _ in 0..cases * TRIES {
            if found == cases {
                break;
            }
            let word = form.word(rng.u32(..));
            if !form.presented(word) || !wanted(&decode(word)) {
                continue;
            }
            found += 1;
            let case = case_for(word, &mut rng);
            let reference = reference(&case);
            let translated = translated(&case);
            tally.cases += 1;
            if let Some(why) = disagreement(&case, &reference, &translated, &mut tally) {
                failures.entry(form.name.clone()).or_insert_with(|| {
                    format!("{word:#010x} {:?}: {why}\ncase {case:x?}", decode(word))
                });
            }
        }
        tally.forms += u64::from(found > 0);
    }
    eprintln!(
        "seed {seed:#x}, {cases} cases per form: {tally:?}, {} forms failing",
        failures.len()
    );
    for (form, why) in &failures {
        eprintln!(
            "FAIL {form}: {}",
            why.chars().take(1500).collect::<String>()
        );
    }
    assert!(failures.is_empty(), "{} forms disagree", failures.len());
}
