//! Whether a run of the native output's instructions stays confined (docs/native.md, form I): from every starting
//! state an entry rule allows, each memory access is inside what the run is allowed, no instruction is a system call
//! or changes a register outside the list `LinuxProcess::run_code` names, x28 keeps the frame's address, and the
//! next instruction is one of the sequence or an exit. Decided conservatively over the bytes: a sequence this
//! checker passes is confined; one it refuses may still be.
//!
//! Each register holds a range of values or anything; the window's base, the frame and the instructions' place are
//! concrete addresses, so that each computed address is a range checked against concrete regions.

mod decode;

pub use decode::{Access, Base, Extend, Made, Moved, Offset, Reg, Source, Step, decode};

use std::collections::BTreeMap;

/// The bits of a guest address in the in-process window; a slot reserves twice that from its base.
pub const WINDOW_BITS: u32 = 32;
/// The bytes a slot reserves from its base: its window, then as many never mapped.
pub const SLOT_BYTES: u64 = 2 << WINDOW_BITS;
/// The shift of an address into its entry slot, and where the slot tables start.
pub const SLOT_SHIFT: u32 = 2;
pub const PROCESS_SLOTS: u64 = 1 << 46;
/// The floor's page, whose entries end a run, each this many bytes.
pub const FLOOR_PAGE: u64 = (1 << 45) + (1 << 32);
pub const FLOOR_ENTRY_BYTES: u64 = 16;
/// The words of the frame the instructions own.
pub const FRAME_BYTES: u64 = 32 * 8;
/// The frame register and the register that holds the window's base between pieces.
pub const FRAME: u8 = 28;
pub const BASE: u8 = 24;

/// Where one run is: its window's base, its frame, and where its instructions are mapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The window's base, a multiple of [`SLOT_BYTES`].
    pub base: u64,
    pub frame: u64,
    pub code: u64,
    /// The never mapped bytes just below the window that are reserved too (none today: the slot below may be no
    /// memory's).
    pub below: u64,
    /// Whether the code is the displaced code, whose stack pointer is the guest's own.
    pub displaced: bool,
}

impl Place {
    /// The slot table its jumps reach: its window's, or slot 0's for displaced code.
    fn slot_table(&self) -> (u64, u64) {
        let start = if self.displaced {
            PROCESS_SLOTS
        } else {
            PROCESS_SLOTS | self.base << SLOT_SHIFT
        };
        (start, start + (SLOT_BYTES << SLOT_SHIFT))
    }
}

/// What a register holds: none for anything, else every value in the range.
pub type Value = Option<(u64, u64)>;

/// The registers at one instruction: x0 to x30, then the stack pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State(pub [Value; 32]);

const SP: usize = 31;

impl State {
    /// Every register holding anything.
    pub fn anything() -> Self {
        Self([None; 32])
    }

    /// The entry rule: x28 the frame, x24 the window's base, and the stack pointer in the window unless displaced.
    pub fn entry(place: &Place) -> Self {
        let mut state = Self::anything();
        state.0[usize::from(FRAME)] = Some((place.frame, place.frame));
        state.0[usize::from(BASE)] = Some((place.base, place.base));
        if !place.displaced {
            state.0[SP] = Some((place.base, place.base + (1 << WINDOW_BITS) - 1));
        }
        state
    }

    fn get(&self, source: Source) -> Value {
        match source {
            Source::Zero => Some((0, 0)),
            Source::Reg(Reg::X(n)) => self.0[usize::from(n)],
            Source::Reg(Reg::Sp) => self.0[SP],
        }
    }

    fn set(&mut self, reg: Reg, value: Value) {
        match reg {
            Reg::X(n) => self.0[usize::from(n)] = value,
            Reg::Sp => self.0[SP] = value,
        }
    }

    /// The smallest state holding both.
    fn join(&self, other: &Self) -> Self {
        let mut joined = self.clone();
        for (into, from) in joined.0.iter_mut().zip(&other.0) {
            *into = match (*into, *from) {
                (Some((a, b)), Some((c, d))) => Some((a.min(c), b.max(d))),
                _ => None,
            };
        }
        joined
    }
}

/// Why a sequence is not confined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Breach {
    SystemCall,
    /// An access whose bytes may be outside every allowed region.
    AccessOutside,
    /// A write of x28 that may not be the frame's address.
    FrameChanged,
    /// A write of a register outside the list.
    ForbiddenRegister,
    UnknownInstruction,
    /// A branch that may go neither to an instruction of the sequence nor to an exit.
    BranchOutside,
    /// The sequence's last instruction goes on past its end.
    FallsOffEnd,
}

/// One breach: the byte offset of its instruction and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    pub at: usize,
    pub word: u32,
    pub breach: Breach,
}

/// How many times an instruction's state may grow before the registers that grew hold anything.
const WIDEN_AFTER: u32 = 8;
/// The most targets a jump into the sequence may have.
const MOST_TARGETS: u64 = 4096;

/// The range `[lo, hi]` taken modulo 2^64, as a register holds it; anything where it wraps part of the way.
fn wrapped(lo: i128, hi: i128) -> Value {
    let modulus = 1i128 << 64;
    let turns = lo.div_euclid(modulus);
    if hi.div_euclid(modulus) != turns {
        return None;
    }
    Some(((lo - turns * modulus) as u64, (hi - turns * modulus) as u64))
}

fn add_signed(value: Value, by: i64) -> Value {
    let (lo, hi) = value?;
    wrapped(i128::from(lo) + i128::from(by), i128::from(hi) + i128::from(by))
}

/// The range of `value` extended as `extend` says, then shifted left.
fn extended(value: Value, extend: Extend, shift: u32) -> Value {
    let (lo, hi) = match (value, extend) {
        (Some(range), _) => range,
        // Any value cut to fewer bits than a register's.
        (None, Extend::Unsigned(from)) if from < 64 => (0, (1u64 << from) - 1),
        (None, _) => return None,
    };
    let (lo, hi) = match extend {
        Extend::Unsigned(64) => (lo, hi),
        Extend::Unsigned(from) => {
            let mask = (1u64 << from) - 1;
            if lo >> from == hi >> from {
                (lo & mask, hi & mask)
            } else {
                (0, mask)
            }
        }
        // A signed extension keeps a range of values whose sign bit is clear.
        Extend::Signed(from) => {
            if hi < 1u64 << (from - 1) {
                (lo, hi)
            } else {
                return None;
            }
        }
    };
    Some((lo.checked_shl(shift)?, hi.checked_mul(1u64 << shift)?))
}

fn add(a: Value, b: Value, subtract: bool) -> Value {
    let ((a_lo, a_hi), (b_lo, b_hi)) = (a?, b?);
    let [a_lo, a_hi, b_lo, b_hi] = [a_lo, a_hi, b_lo, b_hi].map(i128::from);
    if subtract {
        wrapped(a_lo - b_hi, a_hi - b_lo)
    } else {
        wrapped(a_lo + b_lo, a_hi + b_hi)
    }
}

/// `width` bits from `lsb` of every value of the range.
fn field(value: Value, lsb: u32, width: u32) -> (u64, u64) {
    let mask = if width >= 64 { u64::MAX } else { (1 << width) - 1 };
    match value {
        Some((lo, hi)) if lsb + width >= 64 || lo >> (lsb + width) == hi >> (lsb + width) => {
            ((lo >> lsb) & mask, (hi >> lsb) & mask)
        }
        _ => (0, mask),
    }
}

fn evaluate(made: Made, state: &State, place: &Place, at: usize) -> Value {
    let here = place.code.wrapping_add(at as u64);
    match made {
        Made::Unknown => None,
        Made::Range(lo, hi) => Some((lo, hi)),
        Made::Add {
            rn,
            rm,
            extend,
            shift,
            subtract,
        } => add(state.get(rn), extended(state.get(rm), extend, shift), subtract),
        Made::AddConstant(rn, by) => add_signed(state.get(rn), by),
        Made::Code(by) => add_signed(Some((here, here)), by),
        Made::CodePage { at, plus } => {
            let page = here.checked_add_signed(at)? & !0xfff;
            add_signed(Some((page, page)), plus)
        }
        Made::Constant(c) => Some((c, c)),
        Made::Keep { rd, value, shift } => match state.0[usize::from(rd)] {
            Some((lo, hi)) if lo == hi => {
                let kept = (lo & !(0xffff << shift)) | u64::from(value) << shift;
                Some((kept, kept))
            }
            _ => None,
        },
        Made::Extract { rn, lsb, width } => Some(field(state.get(rn), lsb, width)),
        Made::Insert { rn, lsb, width } => {
            let (lo, hi) = field(state.get(rn), 0, width);
            Some((lo << lsb, hi << lsb))
        }
        Made::ExtractInto { rd, width } => {
            let (lo, hi) = state.0[usize::from(rd)]?;
            let mask = (1u64 << width) - 1;
            Some((lo & !mask, (hi & !mask) | mask))
        }
        Made::InsertInto { rd, lsb, width } => {
            let (lo, hi) = state.0[usize::from(rd)]?;
            let above = lsb + width;
            if above >= 64 {
                return None;
            }
            let keep = |v: u64| v >> above << above;
            Some((keep(lo), keep(hi) | ((1 << above) - 1)))
        }
        Made::And(rn, mask) => match state.get(rn) {
            Some((lo, hi)) if lo == hi => Some((lo & mask, lo & mask)),
            Some((_, hi)) => Some((0, hi.min(mask))),
            None => Some((0, mask)),
        },
        Made::Or(rn, mask) => match state.get(rn)? {
            (lo, hi) if lo == hi => Some((lo | mask, lo | mask)),
            _ => None,
        },
        Made::Xor(rn, mask) => {
            let (lo, hi) = state.get(rn)?;
            if lo == hi {
                return Some((lo ^ mask, lo ^ mask));
            }
            // One bit flipped in a range where every value has it alike.
            if mask.is_power_of_two() {
                let b = mask.trailing_zeros();
                if b < 63 && lo >> b == hi >> b {
                    return if lo & mask == 0 {
                        Some((lo + mask, hi + mask))
                    } else {
                        Some((lo - mask, hi - mask))
                    };
                }
            }
            None
        }
    }
}

/// Whether every byte of `[lo, hi]` is in one region a run may touch: its slot's reservation, its frame's words, or
/// its own instructions.
fn allowed(lo: u64, hi: u64, place: &Place, length: usize) -> bool {
    let inside = |start: u64, end: u64| start <= lo && hi < end;
    inside(place.base.saturating_sub(place.below), place.base + SLOT_BYTES)
        || inside(place.frame, place.frame + FRAME_BYTES)
        || inside(place.code, place.code + length as u64)
}

/// Where a step goes on: the byte offsets of the sequence it may go to next, or the breach.
enum Next {
    To(Vec<usize>),
    /// The run ends here (an exit or a raised signal).
    Ends,
}

/// Runs one step on `state`, giving the state after it and where it goes on, or the breach.
fn step(
    word: u32,
    at: usize,
    state: &State,
    place: &Place,
    length: usize,
) -> Result<(State, Next), Breach> {
    let mut after = state.clone();
    let next = at + 4;
    let fall = || {
        if next >= length {
            Err(Breach::FallsOffEnd)
        } else {
            Ok(Next::To(vec![next]))
        }
    };
    let write = |after: &mut State, before: &State, reg: Reg, made: Made| -> Result<(), Breach> {
        let value = evaluate(made, before, place, at);
        if reg == Reg::X(FRAME) && value != Some((place.frame, place.frame)) {
            return Err(Breach::FrameChanged);
        }
        after.set(reg, value);
        Ok(())
    };
    match decode(word) {
        Step::Plain(writes) => {
            for (reg, made) in writes {
                write(&mut after, state, reg, made)?;
            }
            Ok((after, fall()?))
        }
        Step::Access(access) => {
            let base = match access.base {
                Base::Reg(source) => state.get(source),
                Base::Here => {
                    let here = place.code + at as u64;
                    Some((here, here))
                }
            };
            let offset = match access.offset {
                Offset::Constant(lo, hi) => Some((lo, hi)),
                Offset::Index { rm, extend, shift } => extended(state.get(rm), extend, shift)
                    .and_then(|(lo, hi)| Some((i64::try_from(lo).ok()?, i64::try_from(hi).ok()?))),
            };
            let (Some((base_lo, base_hi)), Some((offset_lo, offset_hi))) = (base, offset) else {
                return Err(Breach::AccessOutside);
            };
            let first_lo = base_lo
                .checked_add_signed(offset_lo)
                .ok_or(Breach::AccessOutside)?;
            let last = base_hi
                .checked_add_signed(offset_hi)
                .and_then(|a| a.checked_add(access.bytes - 1))
                .ok_or(Breach::AccessOutside)?;
            if !allowed(first_lo, last, place, length) {
                return Err(Breach::AccessOutside);
            }
            // An access that did not fault touched the window, the only mapped part of the slot: its base was where
            // that puts it.
            let in_slot = first_lo >= place.base.saturating_sub(place.below) && last < place.base + SLOT_BYTES;
            if let (Base::Reg(Source::Reg(reg)), Offset::Constant(offset_lo, offset_hi), true) =
                (access.base, access.offset, in_slot)
            {
                let window_end = place.base + (1 << WINDOW_BITS) - access.bytes;
                let low = place.base.checked_add_signed(-offset_hi);
                let high = window_end.checked_add_signed(-offset_lo);
                if let (Some(low), Some(high)) = (low, high) {
                    let lo = base_lo.max(low);
                    let hi = base_hi.min(high);
                    if lo <= hi {
                        after.set(reg, Some((lo, hi)));
                    }
                }
            }
            if let (Some(moved), Base::Reg(Source::Reg(reg))) = (access.moved, access.base) {
                let before = after.clone();
                let made = match moved {
                    Moved::By(by) => Made::AddConstant(Source::Reg(reg), by),
                    Moved::ByRegister(rm) => Made::Add {
                        rn: Source::Reg(reg),
                        rm,
                        extend: Extend::Unsigned(64),
                        shift: 0,
                        subtract: false,
                    },
                };
                write(&mut after, &before, reg, made)?;
            }
            for (reg, made) in access.loads {
                let before = after.clone();
                write(&mut after, &before, reg, made)?;
            }
            Ok((after, fall()?))
        }
        Step::Branch {
            by,
            conditional,
            link,
        } => {
            let target = (at as i64).checked_add(by).ok_or(Breach::BranchOutside)?;
            let target = usize::try_from(target).map_err(|_| Breach::BranchOutside)?;
            if target >= length || !target.is_multiple_of(4) {
                return Err(Breach::BranchOutside);
            }
            if link {
                write(&mut after, state, Reg::X(30), Made::Code(4))?;
            }
            let mut to = vec![target];
            if conditional {
                let Next::To(through) = fall()? else {
                    unreachable!("falling through goes on")
                };
                to.extend(through);
            }
            Ok((after, Next::To(to)))
        }
        Step::Jump { rn, link } => {
            let target = state.get(if rn == 31 {
                Source::Zero
            } else {
                Source::Reg(Reg::X(rn))
            });
            if link {
                write(&mut after, state, Reg::X(30), Made::Code(4))?;
            }
            let Some((lo, hi)) = target else {
                return Err(Breach::BranchOutside);
            };
            // The floor's entries, each ending the run.
            let floor = lo == hi
                && (FLOOR_PAGE..FLOOR_PAGE + 4096).contains(&lo)
                && (lo - FLOOR_PAGE).is_multiple_of(FLOOR_ENTRY_BYTES);
            // The slot table, held inaccessible: the jump faults there.
            let (start, end) = place.slot_table();
            if floor || (start <= lo && hi < end) {
                return Ok((after, Next::Ends));
            }
            // Instructions of the sequence.
            let code_end = place.code + length as u64;
            if place.code <= lo && hi < code_end && (hi - lo) / 4 < MOST_TARGETS {
                let first = (lo - place.code).next_multiple_of(4);
                let targets = (first..=hi - place.code)
                    .step_by(4)
                    .map(|t| t as usize)
                    .collect();
                return Ok((after, Next::To(targets)));
            }
            Err(Breach::BranchOutside)
        }
        Step::Leaves => Ok((after, Next::Ends)),
        Step::SystemCall => Err(Breach::SystemCall),
        Step::ForbiddenRegister => Err(Breach::ForbiddenRegister),
        Step::Unknown => Err(Breach::UnknownInstruction),
    }
}

/// Checks the sequence `bytes` (whole words) entered at the byte offsets `entries`, each with its starting state:
/// every breach any run from them may reach, by instruction, and the state found at each instruction reached.
pub fn check(
    bytes: &[u8],
    place: &Place,
    entries: &[(usize, State)],
) -> (Vec<Found>, BTreeMap<usize, State>) {
    let words: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect();
    let length = words.len() * 4;
    let mut states: BTreeMap<usize, State> = BTreeMap::new();
    let mut grown: BTreeMap<usize, u32> = BTreeMap::new();
    let mut work: Vec<usize> = Vec::new();
    let mut found: BTreeMap<usize, Found> = BTreeMap::new();
    let mut arrive = |states: &mut BTreeMap<usize, State>, work: &mut Vec<usize>, at: usize, state: &State| {
        let merged = match states.get(&at) {
            None => state.clone(),
            Some(old) => {
                let joined = old.join(state);
                if joined == *old {
                    return;
                }
                let times = grown.entry(at).or_insert(0);
                *times += 1;
                if *times > WIDEN_AFTER {
                    let mut widened = joined.clone();
                    for (w, o) in widened.0.iter_mut().zip(&old.0) {
                        if *w != *o {
                            *w = None;
                        }
                    }
                    widened
                } else {
                    joined
                }
            }
        };
        states.insert(at, merged);
        work.push(at);
    };
    for (at, state) in entries {
        arrive(&mut states, &mut work, *at, state);
    }
    while let Some(at) = work.pop() {
        let state = states[&at].clone();
        let word = words[at / 4];
        match step(word, at, &state, place, length) {
            Ok((after, Next::To(next))) => {
                for to in next {
                    arrive(&mut states, &mut work, to, &after);
                }
            }
            Ok((_, Next::Ends)) => {}
            Err(breach) => {
                found.entry(at).or_insert(Found { at, word, breach });
            }
        }
    }
    (found.into_values().collect(), states)
}
