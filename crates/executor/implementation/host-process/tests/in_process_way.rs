//! The in-process way's translated code (docs/native-without-virtual-machine.md section 3.2 item 1) under
//! unicorn-engine, against the same guest instruction run there at the guest's own addresses: every presented form
//! that reaches memory or writes or reads the stack pointer, with seeded registers near the window's edges, outside
//! it, tagged, and in the stack. The translated code runs in slot 1's window; the guest's pages at their own
//! addresses and in slot 2's window are mapped too, and touching any of them fails the case, as does any access
//! outside slot 1's window and the unmapped 4 GiB beside it. Outcomes must agree: the same registers, flags,
//! vector registers and memory; a fault at the same guest address; an access the reference faults on outside the
//! window reaching the mark; an alignment fault reaching the floor's misalignment entry; a stack pointer written
//! outside the window reaching the mark; a stack probe faulting only where the reference's stack pointer left the
//! mapped stack. Every stop is resolved through the stop crate's in-process way, and the guest state it gives must
//! be the reference's: the state before the instruction at a fault, an alignment call, the mark or a stack pointer
//! leaving the window, the state after it at a faulting stack probe; a fault at the reference's own address.
//! Pieces of two to eight such instructions are translated as one piece too, against the reference run one
//! instruction at a time (a piece naming x15 or x24 keeps u and the base in two it does not name, counted); and every form is run in displaced code (rule e), from a stack pointer outside the
//! window. Pieces with direct branches, forward inside them and past their end, and a loop counted down in a
//! register, after a part naming nearly every register in half of them (so that only the loop can keep u and the
//! base in registers of its own), run against the reference followed through its branches until it leaves the
//! piece's instructions. A collision (one register named twice where Arm leaves the result open) runs in the reference with the
//! registers the meaning does not write renamed to free ones.
//!
//! A quarter of the drawn words name the stack pointer, a quarter name one register twice, and a quarter have an immediate offset of zero (an access at
//! its base, which the way cuts by its own addressing), which random words rarely do. Knobs:
//! `WCHP_IN_PROCESS_CASES` cases per form, `WCHP_IN_PROCESS_PIECES` pieces of several instructions,
//! `WCHP_IN_PROCESS_BRANCHY` pieces with branches, `WCHP_SEED`.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use unicorn_engine::{Arch, Arm64CpuModel, HookType, MemType, Mode, Prot, RegisterARM64, Unicorn};
use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_arm64::carry::LINK;
use wcontainers_arm64::encode::{self, ZERO, field, form};
use wcontainers_arm64::thread_state::{
    COUNT, PAGE, RegisterPage, SCRATCH, STACK_ALIGNMENT, TAG_LOW, WITHHELD,
};
use wcontainers_arm64::way::{
    BASE, CHECK, IN_PROCESS_ENTRIES, IN_PROCESS_WITHHELD_AT, Kept, MARK, PROCESS_ENTRY_BYTES,
    WINDOW_BITS, Way,
};
use wcontainers_arm64::{Address, Arm64, Instruction as I, Reg, decode, forms};
use wcontainers_executor_arm64_stop::in_process::{self, Resolution};
use wcontainers_executor_arm64_stop::{
    AFTER_PAGE, AfterPage, FloatStatus, GuestState, Interrupted, Piece, Resolved, Stop, Taken,
};
use wcontainers_guest_cpu::{Guest, HostCode, InsideCall, Native, Route};
use wcontainers_translator::{LookAhead, NativeRequest, translate_native_request};

const CASES_VARIABLE: &str = "WCHP_IN_PROCESS_CASES";
const PIECES_VARIABLE: &str = "WCHP_IN_PROCESS_PIECES";
const BRANCHY_VARIABLE: &str = "WCHP_IN_PROCESS_BRANCHY";
/// Pieces of several instructions unless told otherwise.
const DEFAULT_PIECES: u64 = 1000;
/// The most instructions in a piece of several.
const PIECE_LENGTH: usize = 8;
const SEED_VARIABLE: &str = "WCHP_SEED";
/// Cases per form unless told otherwise.
const DEFAULT_CASES: u64 = 4;
/// The seed unless told otherwise.
const DEFAULT_SEED: u64 = 0x2545_f491_4f6c_dd1d;
/// Random words tried per case wanted before a form is given up.
const TRIES: u64 = 64;
/// The register fields that name the stack pointer as 31 in most forms that can: Rn, and Rd (Rt in an access).
const RN_FIELD: u32 = 0x1f << 5;
const RD_FIELD: u32 = 0x1f;
/// A pair's second data register field, Rt2.
const RT2_FIELD: u32 = 0x1f << 10;
/// The second source register field, Rm.
const RM_FIELD: u32 = 0x1f << 16;
/// Every field a register an access or a branch names is in.
const REGISTER_FIELDS: [u32; 4] = [RD_FIELD, RN_FIELD, RT2_FIELD, RM_FIELD];

/// Arm's names of the fields that hold an access's immediate offset from its base.
const OFFSET_FIELDS: [&str; 3] = ["imm12", "imm9", "imm7"];

/// The slot whose window the translated code runs in, and another whose window it must never reach.
const SLOT: u16 = 1;
const OTHER_SLOT: u16 = 2;
/// The way of the slot's ordinary code, and of its displaced code (rule e).
const WAY: Way = Way::InProcess {
    slot: SLOT,
    displaced: false,
    kept: Kept::USUAL,
};
const DISPLACED: Way = Way::InProcess {
    slot: SLOT,
    displaced: true,
    kept: Kept::USUAL,
};
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
/// The count when the translated code starts, which its leads are taken from.
const COUNT_START: u64 = 1 << 40;
/// The bytes of an instruction.
const INSTRUCTION: u64 = 4;
/// The furthest bytes an access reaches from its address before the immediate: the largest immediate offset of an
/// unsigned 12-bit field scaled by 8, and the largest access (Arm ARM, LDR (immediate); LD4).
const ACCESS_REACH: u64 = 4095 * 8 + 64;
/// Instructions past a piece's end whose entry slots end its run: the furthest a drawn branch goes.
const EXITS_PAST: u64 = 4;
/// The most instructions of a loop's body in a piece with branches.
const BODY_LENGTH: usize = 5;
/// The most turns of such a loop.
const TURNS: u64 = 4;
/// The most guest instructions the reference runs of one piece with branches.
const REFERENCE_STEPS: usize = 400;

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

/// The guest's state after an instruction, and where it goes on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    pc: u64,
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
    /// The translated code still running after [`STEPS`] host instructions.
    Running,
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

/// The hardware where translated code stopped, and what the host says it took.
#[derive(Clone, Debug)]
struct Trapped {
    h: Interrupted,
    taken: Taken,
}

/// The first fault a run met, kept by its hook, with the translated code's hardware there.
type Faults = Rc<RefCell<Option<(Outcome, Option<Trapped>)>>>;

fn first(faults: &Faults, outcome: Outcome) {
    faults.borrow_mut().get_or_insert((outcome, None));
}

fn first_trapped(faults: &Faults, outcome: Outcome, trapped: Trapped) {
    faults.borrow_mut().get_or_insert((outcome, Some(trapped)));
}

/// The emulator's general registers, stack pointer and flags, at host pc `pc`.
fn hardware(unicorn: &Unicorn<'_, ()>, pc: u64) -> Interrupted {
    Interrupted {
        x: std::array::from_fn(|n| unicorn.reg_read(x_register(n)).expect("a register")),
        sp: unicorn.reg_read(RegisterARM64::SP).expect("sp"),
        pc,
        state: unicorn.reg_read(RegisterARM64::NZCV).expect("flags"),
        fpsr: 0,
        fpcr: 0,
    }
}

/// The guest's registers on the translated side: the case's, its base carrying the tag.
fn guest_registers(case: &Case) -> [u64; GENERAL] {
    std::array::from_fn(|n| match case.tag {
        Some((t, tag)) if t == n => case.x[n] | tag << TAG_LOW,
        _ => case.x[n],
    })
}

/// What the reference runs for a collision, whose result Arm leaves open and the meaning chooses (crates
/// wcontainers-arm64, native.rs and carry.rs): the word with each register the meaning does not write from the
/// access renamed to a free one, which is restored after it.
struct Renamed {
    word: u32,
    /// Free general registers given a value before the word: a store's base, which the meaning stores as it was.
    set: Vec<(usize, u64)>,
    /// Free general registers given back their value after the word.
    restore: Vec<usize>,
    /// A free vector register given back its value after the word.
    vector: Option<usize>,
}

/// The reference's word for `case`: its own, unless it is a collision. The meaning loads a pair's first word into
/// a register named twice and drops the second, writes a load's written back base last, and stores a written back
/// base's value from before the access.
fn renamed(case: &Case) -> Renamed {
    let i = decode(case.word);
    let mut out = Renamed {
        word: case.word,
        set: Vec::new(),
        restore: Vec::new(),
        vector: None,
    };
    if !matches!(Arm64::route(&WAY, &i, &()), Route::Collision) {
        return out;
    }
    let base = match i.memory_access().map(|m| m.address) {
        Some(Address::PreIndex { base, .. } | Address::PostIndex { base, .. }) => Some(base),
        _ => None,
    };
    let is_base = |r: Reg| matches!((base, r), (Some(Reg::X(b)), Reg::X(n)) if b == n);
    let base_value = match base {
        Some(Reg::X(b)) => case.x[usize::from(b)],
        _ => case.sp,
    };
    let named: Vec<Reg> = [other_registers(&i), base.into_iter().collect()].concat();
    let mut free = (0..GENERAL).filter(|&n| !named.contains(&Reg::X(n as u8)));
    let mut rename = |word: u32, field, value: Option<u64>| {
        let n = free
            .next()
            .expect("a register the instruction does not name");
        out.restore.push(n);
        if let Some(value) = value {
            out.set.push((n, value));
        }
        encode::replace(word, field, n as u32).expect("a register fits")
    };
    let mut word = case.word;
    match i {
        I::Load { rt, .. } if is_base(rt) => word = rename(word, field::RT, None),
        I::Store { rt, .. } if is_base(rt) => word = rename(word, field::RT, Some(base_value)),
        I::LoadPair { rt, rt2, .. } => {
            if is_base(rt) || (rt == rt2 && rt == Reg::Zero) {
                word = rename(word, field::RT, None);
            }
            if rt == rt2 || is_base(rt2) {
                word = rename(word, field::RT2, None);
            }
        }
        I::StorePair { rt, rt2, .. } => {
            let stored = rename(word, field::RT, Some(base_value));
            let n = *out.restore.last().expect("the register just renamed");
            if is_base(rt) {
                word = stored;
            }
            if is_base(rt2) {
                word = encode::replace(word, field::RT2, n as u32).expect("a register fits");
            }
        }
        I::LoadVectorPair { rt, .. } => {
            let n = (usize::from(rt) + 1) % VECTORS;
            out.vector = Some(n);
            word = encode::replace(word, field::RT2, n as u32).expect("a register fits");
        }
        _ => unreachable!("only these collide: {i:?}"),
    }
    out.word = word;
    out
}

/// The guest instruction run at the guest's own addresses, at `pc`; a collision as [`renamed`] gives it.
fn reference(case: &Case, pc: u64) -> Outcome {
    let renamed = renamed(case);
    let mut unicorn = emulator();
    map_regions(&mut unicorn, case, 0);
    unicorn
        .mem_write(pc, &renamed.word.to_le_bytes())
        .expect("the instruction");
    // The emulator decodes the word after the one instruction it runs, and a branch's target, whatever their bytes:
    // each is a `udf` for the run, so that no word the emulator cannot decode is decoded, unless an access may read
    // it.
    let i = decode(case.word);
    let target = match i {
        I::Branch { offset, .. }
        | I::BranchCond { offset, .. }
        | I::CompareBranch { offset, .. }
        | I::TestBranch { offset, .. } => Some(pc.wrapping_add_signed(offset)),
        _ => None,
    };
    let reads = |at: u64| {
        start_address(&i, &case.x, case.sp).is_some_and(|(_, a)| a.abs_diff(at) <= ACCESS_REACH)
            || matches!(i.memory_access(), Some(m) if matches!(m.address, Address::Literal { .. }))
    };
    let guarded: Vec<(u64, [u8; size_of::<u32>()])> = [Some(pc + INSTRUCTION), target]
        .into_iter()
        .flatten()
        .filter(|&at| at != pc && (CODE..CODE + PAGE_SIZE).contains(&at) && !reads(at))
        .map(|at| {
            let mut bytes = [0; size_of::<u32>()];
            unicorn.mem_read(at, &mut bytes).expect("a code word");
            (at, bytes)
        })
        .collect();
    for &(at, _) in &guarded {
        unicorn
            .mem_write(at, &encode::exception(form::UDF, 0).to_le_bytes())
            .expect("a code word");
    }
    for (n, &value) in case.x.iter().enumerate() {
        unicorn.reg_write(x_register(n), value).expect("a register");
    }
    for &(n, value) in &renamed.set {
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
    let ran = unicorn.emu_start(pc, pc + Arm64::MAX_LENGTH, 0, 1);
    if let Some((outcome, _)) = faults.take() {
        return outcome;
    }
    if ran.is_err() {
        return Outcome::Exception(u32::MAX);
    }
    for (at, bytes) in guarded {
        unicorn.mem_write(at, &bytes).expect("a code word");
    }
    // The guest's own word in its code page, unless the access itself wrote there.
    let mut there = [0; size_of::<u32>()];
    unicorn.mem_read(pc, &mut there).expect("the instruction");
    if renamed.word != case.word && there == renamed.word.to_le_bytes() {
        unicorn
            .mem_write(pc, &case.word.to_le_bytes())
            .expect("the instruction");
    }
    let mut q = read_vectors(&unicorn);
    if let Some(n) = renamed.vector {
        q[n] = case.q[n];
    }
    Outcome::Done(Box::new(State {
        pc: unicorn.reg_read(RegisterARM64::PC).expect("pc"),
        x: std::array::from_fn(|n| {
            if renamed.restore.contains(&n) {
                case.x[n]
            } else {
                unicorn.reg_read(x_register(n)).expect("a register")
            }
        }),
        sp: unicorn.reg_read(RegisterARM64::SP).expect("sp"),
        nzcv: unicorn.reg_read(RegisterARM64::NZCV).expect("flags"),
        q,
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

/// The register page and the words after it, as translated code left them at `at`.
fn kept(unicorn: &Unicorn<'_, ()>, at: u64) -> (RegisterPage, AfterPage) {
    let word = |offset: usize| {
        let mut bytes = [0; size_of::<u64>()];
        unicorn
            .mem_read(at + offset as u64, &mut bytes)
            .expect("a register page word");
        u64::from_le_bytes(bytes)
    };
    let words = |from: usize, into: &mut [u64]| {
        for (k, w) in into.iter_mut().enumerate() {
            *w = word(from + k * size_of::<u64>());
        }
    };
    let mut page = RegisterPage::default();
    words(
        core::mem::offset_of!(RegisterPage, withheld),
        &mut page.withheld,
    );
    words(core::mem::offset_of!(RegisterPage, saved), &mut page.saved);
    page.link = word(core::mem::offset_of!(RegisterPage, link));
    let mut exclusive = page.exclusive();
    words(
        core::mem::offset_of!(RegisterPage, exclusive_bytes),
        &mut exclusive,
    );
    page.set_exclusive(exclusive);
    let mut after = [0; AFTER_PAGE];
    words(IN_PROCESS_WITHHELD_AT, &mut after);
    (page, after)
}

/// Guest instructions `words` at [`CODE`] as one piece for `way`: their bytes and the piece.
fn translation(words: &[u32], way: Way) -> (Vec<u8>, HostCode) {
    let look_ahead = match words.len() {
        1 => LookAhead::None,
        n => LookAhead::Bounded(u32::try_from(n).expect("a short piece")),
    };
    let request = NativeRequest {
        look_ahead,
        exact_replay: false,
        stop: false,
        step: false,
    };
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let code =
        translate_native_request::<Arm64>(&bytes, CODE, &request, &way, &()).expect("translated");
    (bytes, code)
}

/// Guest instructions `words` at [`CODE`], from the case's state, translated for `way`, an in-process way of slot
/// [`SLOT`], as one piece and run in its window, with what its stop resolves to.
fn translated(case: &Case, words: &[u32], way: Way) -> (Outcome, Option<Resolution>) {
    let base = way.window_base();
    let other = Way::InProcess {
        slot: OTHER_SLOT,
        displaced: false,
        kept: Kept::USUAL,
    }
    .window_base();
    let (bytes, code) = translation(words, way);
    let mut unicorn = emulator();
    map_regions(&mut unicorn, case, base);
    unicorn
        .mem_write(base + CODE, &bytes)
        .expect("the instructions");
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
    let guest = guest_registers(case);
    for &r in way.withheld().iter() {
        let at = REGISTER_PAGE + withheld_word(way, r);
        unicorn
            .mem_write(at, &guest[usize::from(r)].to_le_bytes())
            .expect("a withheld register's guest value");
    }
    // The entry slots of the piece's addresses and a few past its end, where its exits go: reaching one's first word
    // ends the run there.
    let slots = way.slot(CODE)..=way.slot(CODE + bytes.len() as u64 + EXITS_PAST * INSTRUCTION);
    let entries = IN_PROCESS_ENTRIES & !(PAGE_SIZE - 1);
    let slot_pages = (slots.start() & !(PAGE_SIZE - 1)..=*slots.end()).step_by(PAGE_SIZE as usize);
    for page in slot_pages.chain([entries]) {
        unicorn
            .mem_map(page, PAGE_SIZE, Prot::ALL)
            .expect("a landing page");
    }
    let into = faults.clone();
    unicorn
        .add_code_hook(*slots.start(), *slots.end(), move |unicorn, address, _| {
            let pc = way.slot_target(address);
            if way.slot(pc) == address {
                first(
                    &into,
                    Outcome::Done(Box::new(State {
                        pc,
                        ..State::default()
                    })),
                );
            } else {
                first(&into, Outcome::Stray(address));
            }
            unicorn.emu_stop().expect("stopped");
        })
        .expect("the exits' hook");
    let into = faults.clone();
    unicorn
        .add_code_hook(
            entries,
            entries + PAGE_SIZE - 1,
            move |unicorn, address, _| {
                let code = (address - IN_PROCESS_ENTRIES) / PROCESS_ENTRY_BYTES;
                let call = u16::try_from(code).ok().and_then(InsideCall::from_code);
                match call {
                    // The call is reported at its blr, before the return address in x30.
                    Some(call) => {
                        let link = unicorn.reg_read(RegisterARM64::X30).expect("x30");
                        let h = hardware(unicorn, link - INSTRUCTION);
                        let taken = Taken::Call(call.code());
                        first_trapped(&into, Outcome::Call(call), Trapped { h, taken });
                    }
                    None => first(&into, Outcome::Stray(address)),
                }
                unicorn.emu_stop().expect("stopped");
            },
        )
        .expect("the entries' hook");
    let into = faults.clone();
    unicorn
        .add_mem_hook(
            HookType::MEM_INVALID,
            1,
            0,
            move |unicorn, kind, address, _, _| {
                let guest = address.wrapping_sub(base);
                let outcome = if address == base + MARK {
                    Outcome::Mark
                } else if guest < 1 << WINDOW_BITS
                    || guest.wrapping_neg() <= 1 << WINDOW_BITS
                    || (guest >> WINDOW_BITS) == 1
                {
                    Outcome::Fault(guest)
                } else {
                    first(&into, Outcome::Stray(address));
                    return false;
                };
                let pc = unicorn.reg_read(RegisterARM64::PC).expect("pc");
                let taken = Taken::Access {
                    alignment: false,
                    fault: address,
                    write: matches!(
                        kind,
                        MemType::WRITE | MemType::WRITE_UNMAPPED | MemType::WRITE_PROT
                    ),
                };
                let h = hardware(unicorn, pc);
                first_trapped(&into, outcome, Trapped { h, taken });
                false
            },
        )
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
            _ if r == COUNT => COUNT_START,
            _ if r == SCRATCH || r == CHECK => u64::MAX,
            _ => guest_value,
        };
        unicorn.reg_write(x_register(n), value).expect("a register");
    }
    unicorn
        .reg_write(RegisterARM64::SP, way.stack_base().wrapping_add(case.sp))
        .expect("sp");
    unicorn
        .reg_write(RegisterARM64::NZCV, case.nzcv)
        .expect("flags");
    write_vectors(&unicorn, &case.q);
    let ran = unicorn.emu_start(HOST_CODE, 0, 0, STEPS);
    let pc = match faults.take() {
        Some((Outcome::Done(state), _)) => state.pc,
        Some((outcome, None)) => return (outcome, None),
        Some((outcome, Some(t))) => {
            let (page, after) = kept(&unicorn, REGISTER_PAGE);
            let piece = Piece {
                base: HOST_CODE,
                length: code.bytes.len(),
                spans: &code.spans,
            };
            let resolution = in_process::resolve(
                way,
                &piece,
                t.taken,
                &t.h,
                &page,
                &after,
                &mut FloatStatus::default(),
                &mut |at, into| {
                    if unicorn.mem_read(base + at, into).is_err() {
                        into.fill(0);
                    }
                },
            );
            return (outcome, resolution);
        }
        None if ran.is_ok() => return (Outcome::Running, None),
        None => panic!("the translated code ended without an outcome: {ran:?}"),
    };
    let mut page = vec![0; PAGE_SIZE as usize];
    unicorn
        .mem_read(REGISTER_PAGE, &mut page)
        .expect("the register page");
    let word = |at: u64| {
        let at = at as usize;
        u64::from_le_bytes(page[at..at + size_of::<u64>()].try_into().expect("a word"))
    };
    let done = Outcome::Done(Box::new(State {
        pc,
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
            .wrapping_sub(way.stack_base()),
        nzcv: unicorn.reg_read(RegisterARM64::NZCV).expect("flags"),
        q: read_vectors(&unicorn),
        memory: read_regions(&unicorn, base),
    }));
    (done, None)
}

/// Whether the instruction is one the in-process way changes: an access, a cache or zeroing operation by address,
/// or an instruction naming the stack pointer as a value.
fn wanted(i: &I) -> bool {
    let route = Arm64::route(&WAY, i, &());
    match route {
        Route::Copy | Route::Split | Route::Aligned | Route::Exclusive | Route::Collision => {
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
    if expected.pc != got.pc {
        out.push(format!("pc {:#x} not {:#x}", got.pc, expected.pc));
    }
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
    // Half the stack pointers within a page of the stack's ends, where a move of it leaves the stack.
    let sp = match rng.u32(0..4) {
        0 => stack_bottom + rng.u64(0..PAGE_SIZE),
        1 => STACK_TOP - rng.u64(0..=PAGE_SIZE),
        _ => stack_bottom + rng.u64(0..STACK_PAGES * PAGE_SIZE),
    } & !15;
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
        // An access whose first byte is the window's top faults at the mark's own host address.
        (Outcome::Fault(f), Outcome::Mark) => {
            match start_address(&i, &case.x, case.sp) {
                Some((_, a)) if outside(a) || outside(*f) => {
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

/// How a resolved guest state differs from the one expected: registers, stack pointer, pc and flags.
fn state_differences(
    x: &[u64; GENERAL],
    sp: u64,
    pc: u64,
    nzcv: u64,
    got: &GuestState,
) -> Option<String> {
    let mut out: Vec<String> = (0..GENERAL)
        .filter(|&n| x[n] != got.x[n])
        .map(|n| format!("x{n} {:#x} not {:#x}", got.x[n], x[n]))
        .collect();
    for (name, want, have) in [
        ("sp", sp, got.sp),
        ("pc", pc, got.pc),
        ("nzcv", nzcv, got.flags),
    ] {
        if want != have {
            out.push(format!("{name} {have:#x} not {want:#x}"));
        }
    }
    (!out.is_empty()).then(|| format!("resolved state differs: {}", out.join(", ")))
}

/// What the translated code's stop must resolve to for the reference's outcome, or why it may differ; None when
/// it agrees. Called once the outcomes agree.
fn resolution_disagreement(
    case: &Case,
    pc: u64,
    reference: &Outcome,
    translated: &Outcome,
    resolution: Option<&Resolution>,
) -> Option<String> {
    let i = decode(case.word);
    let before = |state: &GuestState| {
        state_differences(&guest_registers(case), case.sp, pc, case.nzcv, state)
    };
    // The top byte of a tagged address is the stop crate's rule (`untag_address`), which the emulator has none of.
    let below_tag = |a: u64| a & ((1 << TAG_LOW) - 1);
    let at = |address: u64, want: u64, state: &GuestState| {
        if below_tag(address) == below_tag(want) {
            before(state)
        } else {
            Some(format!("resolved a fault at {address:#x}, not {want:#x}"))
        }
    };
    match (reference, translated, resolution) {
        (_, Outcome::Done(_), _) => None,
        // The emulator's own top byte and store exclusives: what the outcomes' rules accepted.
        (Outcome::Fault(a), _, _) if a >> TAG_LOW != 0 => None,
        (Outcome::Done(_), _, _) if matches!(i, I::StoreExclusive { .. }) => None,
        (
            Outcome::Fault(a),
            Outcome::Fault(_),
            Some(Resolution::Resolved(Resolved::Stop(Stop::MemoryFault { address, .. }, state))),
        )
        | (Outcome::Fault(a), Outcome::Mark, Some(Resolution::Outside { address, state })) => {
            at(*address, *a, state)
        }
        (Outcome::Done(_), Outcome::Mark, Some(Resolution::Outside { address, state })) => {
            match i {
                I::CleanCache { rn: Reg::X(n) } => at(*address, case.x[usize::from(n)], state),
                _ => Some(format!("resolved outside at {address:#x}")),
            }
        }
        (Outcome::Done(_), Outcome::Mark, Some(Resolution::Displaced(state))) => before(state),
        (Outcome::Done(r), Outcome::Fault(_), Some(Resolution::Probed(state))) => {
            state_differences(&r.x, r.sp, pc + INSTRUCTION, r.nzcv, state)
        }
        (
            _,
            Outcome::Call(InsideCall::Misaligned),
            Some(Resolution::Resolved(Resolved::Stop(Stop::Alignment(_), state))),
        ) => before(state),
        (Outcome::Exception(_), _, _) => None,
        (_, _, resolution) => Some(format!("resolved to {resolution:x?}")),
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
    resolved: u64,
    at_base: u64,
    stack_unaligned: u64,
    collisions: u64,
    /// Pieces that keep u or the base elsewhere, naming their usual registers.
    kept_elsewhere: u64,
}

/// Whether the piece of `words` keeps u or the base in other registers than between pieces.
fn kept_elsewhere(way: Way, words: &[u32]) -> bool {
    let instructions: Vec<I> = words.iter().map(|&w| decode(w)).collect();
    (Arm64::part_ways(&way, &[&instructions], &[]).iter()).any(|w| w.moves_kept())
}

/// A word of `form`: a quarter name the stack pointer as their base or source, an eighth as their destination too
/// (rule c's moves); a quarter name one register twice, the base as the data or the data as both of a pair's
/// (collisions); and a quarter have an immediate offset of zero, which random words have rarely.
fn drawn(form: &forms::Form, rng: &mut fastrand::Rng) -> u32 {
    let raw = rng.u32(..);
    let named = match rng.u32(0..8) {
        0 => raw | RN_FIELD | RD_FIELD,
        1 => raw | RN_FIELD,
        2 => raw & !RD_FIELD | (raw & RN_FIELD) >> RN_FIELD.trailing_zeros(),
        3 => raw & !RT2_FIELD | (raw & RD_FIELD) << RT2_FIELD.trailing_zeros(),
        _ => raw,
    };
    let word = form.word(named);
    if rng.u32(0..4) == 0 {
        at_base(form, word)
    } else {
        word
    }
}

/// `word` of `form` with its immediate offset, if it has one, zero.
fn at_base(form: &forms::Form, word: u32) -> u32 {
    (form.fields.iter())
        .filter(|(name, ..)| OFFSET_FIELDS.contains(&name.as_str()))
        .fold(word, |word, &(_, low, width)| {
            word & !((((1u64 << width) - 1) as u32) << low)
        })
}

#[test]
fn in_process_way_agrees_with_the_reference_and_stays_in_its_window() {
    every_form(WAY, |_| None);
}

/// Every form as [`in_process_way_agrees_with_the_reference_and_stays_in_its_window`] runs it, in displaced code
/// (rule e) from a stack pointer outside the window: above it, below zero, far outside, tagged, and at the host
/// addresses of the translated code, the register page and the other slot's stack, where an access through the
/// stack pointer that was not checked and cut would succeed. Knobs as that test's.
#[test]
fn displaced_code_agrees_with_the_reference_and_stays_in_its_window() {
    every_form(DISPLACED, |rng| {
        let window = 1u64 << WINDOW_BITS;
        let other = Way::InProcess {
            slot: OTHER_SLOT,
            displaced: false,
            kept: Kept::USUAL,
        }
        .window_base();
        let sp = match rng.u32(0..6) {
            0 => window + rng.u64(0..PAGE_SIZE),
            1 => 0u64.wrapping_sub(rng.u64(1..=PAGE_SIZE)),
            2 => window * rng.u64(1..1 << 8) + rng.u64(0..window),
            3 => rng.u64(1..1 << (u64::BITS - TAG_LOW)) << TAG_LOW | rng.u64(0..STACK_TOP),
            4 => {
                let near = [HOST_CODE, REGISTER_PAGE, other + STACK_TOP - PAGE_SIZE];
                near[rng.usize(0..near.len())] + rng.u64(0..PAGE_SIZE)
            }
            _ => rng.u64(window..),
        };
        Some(sp & !(STACK_ALIGNMENT - 1))
    });
}

/// Every presented form the way changes, `WCHP_IN_PROCESS_CASES` cases each, translated for `way` and run against
/// the reference, from a stack pointer `stack` gives, if it gives one.
fn every_form(way: Way, stack: impl Fn(&mut fastrand::Rng) -> Option<u64>) {
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
            let word = drawn(form, &mut rng);
            if !form.presented(word) || !wanted(&decode(word)) {
                continue;
            }
            found += 1;
            tally.at_base += u64::from(matches!(
                decode(word).memory_access().map(|m| m.address),
                Some(Address::Offset { offset: 0, .. })
            ));
            let mut case = case_for(word, &mut rng);
            if let Some(sp) = stack(&mut rng) {
                case.sp = sp;
            }
            tally.collisions += u64::from(renamed(&case).word != word);
            let reference = reference(&case, CODE);
            let (translated, resolution) = translated(&case, &[word], way);
            tally.cases += 1;
            tally.kept_elsewhere += u64::from(kept_elsewhere(way, &[word]));
            let why = disagreement(&case, &reference, &translated, &mut tally).or_else(|| {
                resolution_disagreement(&case, CODE, &reference, &translated, resolution.as_ref())
            });
            tally.resolved += u64::from(resolution.is_some());
            if let Some(why) = why {
                failures.entry(form.name.clone()).or_insert_with(|| {
                    format!("{word:#010x} {:?}: {why}\ncase {case:x?}", decode(word))
                });
            }
        }
        tally.forms += u64::from(found > 0);
    }
    eprintln!(
        "{way:?}, seed {seed:#x}, {cases} cases per form: {tally:?}, {} forms failing",
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

/// The case of instruction `word` from the state an instruction before it left.
fn next_case(word: u32, state: &State) -> Case {
    Case {
        word,
        x: state.x,
        sp: state.sp,
        nzcv: state.nzcv,
        q: state.q,
        tag: None,
        memory: state.memory.clone(),
    }
}

/// The guest instruction a translated piece of `length` stopped at, by the guest state its stop resolved to: the
/// state before the instruction, or after it for a stack probe.
fn stopped_at(resolution: &Resolution, length: usize) -> Option<usize> {
    let (state, after) = match resolution {
        Resolution::Resolved(Resolved::Stop(_, state))
        | Resolution::Outside { state, .. }
        | Resolution::Displaced(state) => (state, 0),
        Resolution::Probed(state) => (state, 1),
        Resolution::Resolved(_) => return None,
    };
    let k = (state.pc.checked_sub(CODE)? / INSTRUCTION).checked_sub(after)?;
    usize::try_from(k).ok().filter(|&k| k < length)
}

/// Pieces of several instructions the way changes, drawn as the single instructions are and translated as one
/// piece, against the reference run one instruction at a time: the translated piece must stop at the instruction
/// where the reference stops, with that instruction's outcome and resolved state, or end with the reference's
/// state after the last. Knobs: `WCHP_IN_PROCESS_PIECES` pieces, `WCHP_SEED`.
#[test]
fn in_process_pieces_of_several_instructions_agree_with_the_reference() {
    let pieces = setting(PIECES_VARIABLE, DEFAULT_PIECES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let shows = |form: &forms::Form, word: u32| form.presented(word) && wanted(&decode(word));
    let changed: Vec<&forms::Form> = (forms::all().iter())
        .filter(|form| (0..TRIES).any(|_| shows(form, drawn(form, &mut rng))))
        .collect();
    let mut tally = Tally::default();
    let (mut ended, mut stopped_inside) = (0, 0);
    let mut failures: Vec<String> = Vec::new();
    for _ in 0..pieces {
        let length = rng.usize(2..=PIECE_LENGTH);
        let words: Vec<u32> = (0..length)
            .map(|_| {
                loop {
                    let form = changed[rng.usize(..changed.len())];
                    let word = drawn(form, &mut rng);
                    if shows(form, word) {
                        break word;
                    }
                }
            })
            .collect();
        tally.collisions += (words.iter())
            .filter(|&&w| matches!(Arm64::route(&WAY, &decode(w), &()), Route::Collision))
            .count() as u64;
        let mut case = case_for(words[0], &mut rng);
        case.tag = None;
        // Every instruction of the piece is in the guest's code page from the start, as the translated side has it.
        let code = (REGIONS.iter())
            .position(|&(start, ..)| start == CODE)
            .expect("the code region");
        let bytes = words.iter().flat_map(|w| w.to_le_bytes());
        for (into, byte) in case.memory[code].iter_mut().zip(bytes) {
            *into = byte;
        }
        let initial = case.clone();
        let (translated, resolution) = translated(&case, &words, WAY);
        let stop = resolution.as_ref().and_then(|r| stopped_at(r, length));
        tally.cases += 1;
        tally.kept_elsewhere += u64::from(kept_elsewhere(WAY, &words));
        tally.resolved += u64::from(resolution.is_some());
        let mut k = 0;
        let why = loop {
            let pc = CODE + k as u64 * INSTRUCTION;
            // An access through a stack pointer an earlier instruction left unaligned: the hardware's check (SA0),
            // which the emulator does not make, faults before any byte is touched. Only the floor's misaligned call
            // is known to be right; the emulator gives no reference for anything else.
            let through_stack = start_address(&decode(case.word), &case.x, case.sp)
                .is_some_and(|(base, _)| base == Reg::Sp);
            if through_stack && !case.sp.is_multiple_of(STACK_ALIGNMENT) {
                tally.stack_unaligned += 1;
                break match (&translated, stop) {
                    (Outcome::Call(InsideCall::Misaligned), Some(j)) if j == k => {
                        stopped_inside += u64::from(k > 0);
                        resolution_disagreement(
                            &case,
                            pc,
                            &Outcome::Exception(u32::MAX),
                            &translated,
                            resolution.as_ref(),
                        )
                    }
                    _ => None,
                };
            }
            let reference = reference(&case, pc);
            let here = match (&translated, stop) {
                (Outcome::Done(_), _) => k + 1 == length,
                (_, Some(j)) => j == k,
                (_, None) => k + 1 == length || !matches!(reference, Outcome::Done(_)),
            };
            if here {
                ended += u64::from(matches!(translated, Outcome::Done(_)));
                stopped_inside += u64::from(k > 0 && !matches!(translated, Outcome::Done(_)));
                break disagreement(&case, &reference, &translated, &mut tally).or_else(|| {
                    resolution_disagreement(&case, pc, &reference, &translated, resolution.as_ref())
                });
            }
            match reference {
                Outcome::Done(state) if k + 1 < length => {
                    k += 1;
                    case = next_case(words[k], &state);
                }
                // The emulator's own exceptions and top byte: the reference cannot go on.
                Outcome::Exception(_) => {
                    tally.emulator += 1;
                    break None;
                }
                Outcome::Fault(a) if a >> TAG_LOW != 0 => {
                    tally.top_byte += 1;
                    break None;
                }
                other => {
                    break Some(format!(
                        "the reference's instruction {k} gives {other:x?}, the translated piece {translated:x?} \
                         resolved to {resolution:x?}"
                    ));
                }
            }
        };
        if let Some(why) = why {
            let words: Vec<String> = words
                .iter()
                .map(|w| format!("{w:#010x} {:?}", decode(*w)))
                .collect();
            let outcome = match &translated {
                Outcome::Done(_) => "done".to_string(),
                other => format!("{other:x?}"),
            };
            failures.push(format!(
                "{}: {why}; the translated piece {outcome} at instruction {stop:?}, resolved to {resolution:x?}\n\
                 case {initial:x?}",
                words.join("; ")
            ));
        }
    }
    eprintln!(
        "seed {seed:#x}, {pieces} pieces of 2 to {PIECE_LENGTH}: {tally:?}, {ended} ran to their end, \
         {stopped_inside} stopped after their first instruction, {} failing",
        failures.len()
    );
    for why in failures.iter().take(8) {
        eprintln!("FAIL {}", why.chars().take(4000).collect::<String>());
    }
    assert!(failures.is_empty(), "{} pieces disagree", failures.len());
}

/// Whether any register field of `word` holds register `r`, whatever its form uses the field for.
fn names_register(word: u32, r: u8) -> bool {
    (REGISTER_FIELDS.iter()).any(|&f| (word & f) >> f.trailing_zeros() == u32::from(r))
}

/// Words naming every general register below the link register, two at a time, and writing none:
/// `orr xzr, xB, xC`.
fn crowded_words() -> Vec<u32> {
    (0..LINK)
        .step_by(2)
        .map(|b| encode::logical_register(form::ORR_SHIFTED, ZERO, b, b + 1))
        .collect()
}

/// A direct branch, call or test and branch.
fn direct(i: &I) -> bool {
    matches!(
        i,
        I::Branch { .. } | I::BranchCond { .. } | I::CompareBranch { .. } | I::TestBranch { .. }
    )
}

/// A presented word of a direct branch form, retargeted `offset` bytes from itself.
fn branch_word(form: &forms::Form, offset: i64, rng: &mut fastrand::Rng) -> u32 {
    loop {
        let word = form.word(rng.u32(..));
        if form.presented(word) && direct(&decode(word)) {
            break encode::retarget(word, offset).expect("a short branch");
        }
    }
}

/// The guest instruction a stop resolved to: the state's, before it, or the one before for a stack probe.
fn stopped_pc(resolution: &Resolution) -> Option<u64> {
    match resolution {
        Resolution::Resolved(Resolved::Stop(_, state))
        | Resolution::Outside { state, .. }
        | Resolution::Displaced(state) => Some(state.pc),
        Resolution::Probed(state) => Some(state.pc - INSTRUCTION),
        Resolution::Resolved(_) => None,
    }
}

/// Pieces with direct branches and a loop, against the reference followed through its branches. Each piece is,
/// in order: in half of them, a part naming every register below x30 ([`crowded_words`]); a body of one to
/// [`BODY_LENGTH`] instructions, accesses as the pieces of several draw them and, one in four, a direct branch
/// (b, bl, b.cond, bc.cond, cbz, cbnz, tbz, tbnz, of any register or condition); in three quarters a loop back to
/// the body's start, counted down in x15, x24 or another register from one to [`TURNS`] (`sub` then `cbnz`), which
/// no word of the body names; and up to two more such instructions. Each branch goes forward, inside the piece or
/// up to [`EXITS_PAST`] instructions past it, never to the loop's own branch. The translated piece runs once; the
/// reference runs from the piece's start one instruction at a time, from the state the one before left, through
/// its branches, until it leaves the piece's instructions, where the translated piece must have ended at the same
/// guest address (through that address's entry slot) with the same state; or until it stops, or the translated
/// piece's stop resolves to its instruction with an outcome the single instructions' rules accept, where the two
/// must agree as theirs do. A branch to an instruction the piece holds that leaves through its slot fails, but for
/// a call, whose target the translator does not follow.
/// Knobs: `WCHP_IN_PROCESS_BRANCHY` pieces, `WCHP_SEED`.
#[test]
fn in_process_pieces_with_branches_and_loops_agree_with_the_reference() {
    let pieces = setting(BRANCHY_VARIABLE, DEFAULT_PIECES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let shows = |form: &forms::Form, word: u32| form.presented(word) && wanted(&decode(word));
    let changed: Vec<&forms::Form> = (forms::all().iter())
        .filter(|form| (0..TRIES).any(|_| shows(form, drawn(form, &mut rng))))
        .collect();
    let branches: Vec<&forms::Form> = (forms::all().iter())
        .filter(|form| {
            (0..TRIES).any(|_| {
                let word = form.word(rng.u32(..));
                form.presented(word) && direct(&decode(word))
            })
        })
        .collect();
    let mut tally = Tally::default();
    let (mut crowded, mut looped, mut loops_kept_elsewhere, mut left, mut steps_run) =
        (0, 0, 0, 0, 0);
    let mut failures: Vec<String> = Vec::new();
    for _ in 0..pieces {
        let mut words = if rng.bool() {
            crowded_words()
        } else {
            Vec::new()
        };
        crowded += u64::from(!words.is_empty());
        let counter = match rng.u32(0..3) {
            0 => CHECK,
            1 => BASE,
            _ => loop {
                let r = rng.u8(..LINK);
                if !WITHHELD.contains(&r) {
                    break r;
                }
            },
        };
        let looping = rng.u32(0..4) != 0;
        let head = words.len();
        // Branches are written once the piece's length is known.
        let mut branching = Vec::new();
        let mut instruction = |words: &mut Vec<u32>, rng: &mut fastrand::Rng| {
            if rng.u32(0..4) == 0 {
                branching.push(words.len());
                words.push(0);
                return;
            }
            words.push(loop {
                let form = changed[rng.usize(..changed.len())];
                let word = drawn(form, rng);
                if shows(form, word) && !(looping && names_register(word, counter)) {
                    break word;
                }
            });
        };
        for _ in 0..rng.usize(1..=BODY_LENGTH) {
            instruction(&mut words, &mut rng);
        }
        let back = looping.then(|| {
            words.push(encode::add_immediate(counter, counter, -1).expect("one fits"));
            let at = words.len();
            let offset = -(((at - head) as u64 * INSTRUCTION) as i64);
            words.push(encode::compare_branch(true, counter, offset).expect("a short loop"));
            at
        });
        for _ in 0..rng.usize(0..=2) {
            instruction(&mut words, &mut rng);
        }
        let length = words.len();
        for at in branching {
            let to = loop {
                let to = rng.usize(at + 1..length + EXITS_PAST as usize);
                if Some(to) != back {
                    break to;
                }
            };
            let form = branches[rng.usize(..branches.len())];
            words[at] = branch_word(form, ((to - at) as u64 * INSTRUCTION) as i64, &mut rng);
        }
        looped += u64::from(looping);
        if let Some(back) = back.filter(|_| head > 0) {
            loops_kept_elsewhere += u64::from(kept_elsewhere(WAY, &words[head..=back]));
        }
        let mut case = case_for(words[0], &mut rng);
        case.tag = None;
        if looping {
            case.x[usize::from(counter)] = rng.u64(1..=TURNS);
        }
        let code = (REGIONS.iter())
            .position(|&(start, ..)| start == CODE)
            .expect("the code region");
        let bytes = words.iter().flat_map(|w| w.to_le_bytes());
        for (into, byte) in case.memory[code].iter_mut().zip(bytes) {
            *into = byte;
        }
        let initial = case.clone();
        let (translated, resolution) = translated(&case, &words, WAY);
        let held: BTreeSet<u64> = (translation(&words, WAY).1.spans.iter())
            .map(|s| s.guest)
            .collect();
        let stop = resolution.as_ref().and_then(stopped_pc);
        tally.cases += 1;
        tally.resolved += u64::from(resolution.is_some());
        let mut pc = CODE;
        let mut steps = 0;
        let why = loop {
            let k = usize::try_from((pc - CODE) / INSTRUCTION).expect("a short piece");
            case.word = words[k];
            let through_stack = start_address(&decode(case.word), &case.x, case.sp)
                .is_some_and(|(base, _)| base == Reg::Sp);
            if through_stack && !case.sp.is_multiple_of(STACK_ALIGNMENT) {
                tally.stack_unaligned += 1;
                break match (&translated, stop) {
                    (Outcome::Call(InsideCall::Misaligned), Some(p)) if p == pc => {
                        resolution_disagreement(
                            &case,
                            pc,
                            &Outcome::Exception(u32::MAX),
                            &translated,
                            resolution.as_ref(),
                        )
                    }
                    _ => None,
                };
            }
            let reference = reference(&case, pc);
            // In a loop the same instruction runs again: the turn the translated piece stopped in is the one whose
            // outcome and state its stop agrees with.
            let stops_here = stop == Some(pc)
                && disagreement(&case, &reference, &translated, &mut Tally::default()).is_none()
                && resolution_disagreement(&case, pc, &reference, &translated, resolution.as_ref())
                    .is_none();
            let Outcome::Done(state) = &reference else {
                break disagreement(&case, &reference, &translated, &mut tally).or_else(|| {
                    resolution_disagreement(&case, pc, &reference, &translated, resolution.as_ref())
                });
            };
            if stops_here {
                break disagreement(&case, &reference, &translated, &mut tally).or_else(|| {
                    resolution_disagreement(&case, pc, &reference, &translated, resolution.as_ref())
                });
            }
            steps += 1;
            if steps > REFERENCE_STEPS {
                break Some(format!(
                    "the reference ran {REFERENCE_STEPS} instructions in the piece"
                ));
            }
            // A call leaves through its exit even to an instruction the piece holds: its target is not followed.
            let call = matches!(decode(case.word), I::Branch { link: true, .. });
            let called = call && matches!(&translated, Outcome::Done(t) if t.pc == state.pc);
            if !held.contains(&state.pc) || called {
                left += u64::from(matches!(translated, Outcome::Done(_)));
                break match &translated {
                    Outcome::Done(_) => disagreement(&case, &reference, &translated, &mut tally),
                    other => Some(format!(
                        "the reference left the piece for {:#x}, the translated piece gives {other:x?}",
                        state.pc
                    )),
                };
            }
            pc = state.pc;
            case = next_case(0, state);
        };
        steps_run += steps;
        if let Some(why) = why {
            let words: Vec<String> = words
                .iter()
                .map(|w| format!("{w:#010x} {:?}", decode(*w)))
                .collect();
            failures.push(format!(
                "{}: {why}; the translated piece {translated:x?}, resolved to {resolution:x?}\ncase {initial:x?}",
                words.join("; ")
            ));
        }
    }
    eprintln!(
        "seed {seed:#x}, {pieces} pieces with branches: {crowded} crowded, {looped} looping, \
         {loops_kept_elsewhere} crowded loops keeping u or the base elsewhere, {left} left through an exit, \
         {steps_run} reference steps: {tally:?}, {} failing",
        failures.len()
    );
    for why in failures.iter().take(8) {
        eprintln!("FAIL {}", why.chars().take(4000).collect::<String>());
    }
    assert!(failures.is_empty(), "{} pieces disagree", failures.len());
}
