//! Loops whose accesses are proven allowed before they run. A loop that makes no call and no system call,
//! holds no other loop and has an exit that counts its turns gets a second copy in which the accesses whose
//! addresses move by a fixed step each turn (or not at all) have no permission check. Before the loop every
//! page those accesses can reach in the turns it may run is looked up; the copy runs only when all of them
//! allow the access, otherwise the loop runs as usual, so faults stay exact.

use super::*;
use wcontainers_arm64::Extend;

/// A value read at the loop's entry: a constant or a register that the loop does not write.
#[derive(Clone, Copy, Debug)]
enum Value {
    Imm(i64),
    Reg(u32),
}

/// A register the loop changes at one instruction only, by adding `step` (negated when `negate`), so it
/// moves by that step at most once a turn.
#[derive(Clone, Copy, Debug)]
struct Induction {
    at: u64,
    block: usize,
    width: Width,
    step: Value,
    negate: bool,
}

/// How a value is read: its low 32 bits zero or sign extended, or all 64 bits. For a comparison of 64 bits
/// the signedness is kept too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Domain {
    U32,
    S32,
    U64,
    S64,
}

impl Domain {
    /// The values a moving register is allowed to take in exact arithmetic: those it reaches without
    /// wrapping. The 64 bit ones are kept well inside i64 so no sum or product here overflows.
    fn bounds(self) -> (i64, i64) {
        match self {
            Domain::U32 => (0, (1 << 32) - 1),
            Domain::S32 => (-(1 << 31), (1 << 31) - 1),
            Domain::U64 => (0, 1 << 62),
            Domain::S64 => (-(1 << 62), 1 << 62),
        }
    }

    fn of_extend(extend: Extend) -> Option<Domain> {
        match (extend.from_bits, extend.signed) {
            (32, false) => Some(Domain::U32),
            (32, true) => Some(Domain::S32),
            (64, _) => Some(Domain::U64),
            _ => None,
        }
    }
}

/// The values a moving register has at an access: its value at the loop's entry plus `j` steps, with `j`
/// from `first` to the number of turns less `short`.
#[derive(Clone, Copy, Debug)]
struct Moving {
    bit: u32,
    induction: Induction,
    first: i64,
    short: i64,
}

/// What the check of one access (or of a group of accesses through one base) covers, in every turn.
#[derive(Clone, Debug)]
pub(super) struct Region {
    /// The instruction whose check the copy leaves out.
    pub(super) at: u64,
    this: u8,
    base: u32,
    /// The index register, how it is read and its shift.
    index: Option<(u32, Domain, u8)>,
    /// Added to the base before the access (offset and pre-indexed forms).
    offset: i64,
    /// The bytes covered, from the access's address.
    from: i64,
    len: u32,
    /// The base or the index, when one of them moves.
    moving: Option<Moving>,
}

/// How the counting exit continues the loop: while the counter compared with the bound is not equal, or below
/// it, or above it.
#[derive(Clone, Copy, Debug)]
enum Relation {
    Ne,
    Below { inclusive: bool },
    Above { inclusive: bool },
}

/// The exit that bounds the number of turns: a comparison of the counter with a bound at a block every turn
/// passes. In turn `t` (from 0) the comparison reads the counter's value at the entry plus `t + ahead`
/// steps.
#[derive(Clone, Copy, Debug)]
struct Counter {
    bit: u32,
    induction: Induction,
    domain: Domain,
    width: Width,
    ahead: i64,
    bound: Value,
    relation: Relation,
}

/// A loop that has a copy without the checks of `regions`.
#[derive(Clone, Debug)]
pub(super) struct Versioned {
    counter: Counter,
    pub(super) regions: Vec<Region>,
}

/// What [`versioned`] needs to know about the function.
pub(super) struct LoopShape<'a> {
    pub(super) function: &'a Function,
    pub(super) decoded: &'a HashMap<u64, Decoded>,
    pub(super) checks: &'a HashMap<u64, CheckPlan>,
    pub(super) body: &'a [bool],
    pub(super) latches: Vec<usize>,
    pub(super) dominates: &'a dyn Fn(usize, usize) -> bool,
}

/// The copy without checks for an innermost loop that makes no call and no system call, if it has a counting
/// exit and accesses whose checks it can leave out.
pub(super) fn versioned(shape: &LoopShape<'_>) -> Option<Versioned> {
    let function = shape.function;
    let blocks: Vec<usize> = (0..function.blocks.len())
        .filter(|&b| shape.body[b])
        .collect();
    let instructions = || {
        blocks.iter().flat_map(|&b| {
            (function.blocks[b].start..function.blocks[b].end)
                .step_by(4)
                .map(move |o| (b, o))
        })
    };
    let mut defs: HashMap<u32, Vec<(usize, u64)>> = HashMap::new();
    for (b, o) in instructions() {
        for bit in shape.decoded[&o]
            .effects
            .defs
            .bits()
            .filter(|&bit| bit < 32)
        {
            defs.entry(bit).or_default().push((b, o));
        }
    }
    let invariant = |bit: u32| !defs.contains_key(&bit);
    let induction = |bit: u32| -> Option<Induction> {
        let &[(block, at)] = defs.get(&bit)?.as_slice() else {
            return None;
        };
        let instruction = shape.decoded[&at].instruction;
        let (width, step, negate) = match instruction {
            Instruction::AddSub {
                width,
                subtract,
                rd,
                rn,
                operand,
                ..
            } if Registers::bit(rd) == Some(bit) && Registers::bit(rn) == Some(bit) => {
                let step = match operand {
                    Operand::Imm(k) => Value::Imm(k as i64),
                    Operand::Shifted {
                        rm: Reg::X(m),
                        amount: 0,
                        ..
                    } if invariant(m.into()) => Value::Reg(m.into()),
                    _ => return None,
                };
                (width, step, subtract)
            }
            _ => match instruction.memory_access()?.address {
                Address::PreIndex { base, offset } | Address::PostIndex { base, offset }
                    if Registers::bit(base) == Some(bit) =>
                {
                    (Width::X64, Value::Imm(offset), false)
                }
                _ => return None,
            },
        };
        Some(Induction {
            at,
            block,
            width,
            step,
            negate,
        })
    };
    // How many steps a register has moved, at most and at least, when it is read at `at` in `block`: one
    // more than the turns before when its change always comes first in the turn, the turns before when it
    // always comes after, and either otherwise.
    let steps = |i: &Induction, block: usize, at: u64| -> (i64, i64) {
        if i.block == block {
            if i.at < at { (1, 0) } else { (0, 1) }
        } else if (shape.dominates)(i.block, block) {
            (1, 0)
        } else if (shape.dominates)(block, i.block) {
            (0, 1)
        } else {
            (0, 0)
        }
    };

    let counter = blocks
        .iter()
        .filter(|&&e| shape.latches.iter().all(|&l| (shape.dominates)(e, l)))
        .find_map(|&e| counting_exit(shape, e, &invariant, &induction, &steps))?;

    let mut regions = Vec::new();
    for (b, o) in instructions() {
        let Some(access) = shape.decoded[&o].instruction.memory_access() else {
            continue;
        };
        let (from, len) = match shape.checks.get(&o) {
            Some(CheckPlan::Exact) => (0, access.bytes),
            Some(&CheckPlan::Group { from, len }) => (from, len),
            _ => continue,
        };
        let (base, index, offset) = match access.address {
            Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
                (base, None, offset)
            }
            Address::PostIndex { base, .. } => (base, None, 0),
            Address::Indexed {
                base,
                index,
                extend,
                shift,
            } => {
                let Some(domain) = Domain::of_extend(extend) else {
                    continue;
                };
                let Some(index) = Registers::bit(index) else {
                    continue;
                };
                (base, Some((index, domain, shift)), 0)
            }
            Address::Literal { .. } => continue,
        };
        let Some(base) = Registers::bit(base) else {
            continue;
        };
        let moves = |bit: u32, domain: Domain| -> Option<Option<Moving>> {
            if invariant(bit) {
                return Some(None);
            }
            let i = induction(bit)?;
            // A 32 bit change read as 64 bits is not a fixed step from a value with high bits set.
            if i.width == Width::W32 && matches!(domain, Domain::U64 | Domain::S64) {
                return None;
            }
            let (first, short) = steps(&i, b, o);
            Some(Some(Moving {
                bit,
                induction: i,
                first,
                short,
            }))
        };
        let Some(base_moves) = moves(base, Domain::U64) else {
            continue;
        };
        let index_moves = match index {
            Some((bit, domain, _)) => match moves(bit, domain) {
                Some(m) => m,
                None => continue,
            },
            None => None,
        };
        let moving = match (base_moves, index_moves) {
            (Some(_), Some(_)) => continue,
            (m, None) | (None, m) => m,
        };
        regions.push(Region {
            at: o,
            this: if access.write { PAGE_WRITE } else { PAGE_READ },
            base,
            index,
            offset,
            from,
            len,
            moving,
        });
    }
    if regions.is_empty() {
        return None;
    }
    Some(Versioned { counter, regions })
}

/// The counting exit at the end of block `e`, if it has one.
fn counting_exit(
    shape: &LoopShape<'_>,
    e: usize,
    invariant: &dyn Fn(u32) -> bool,
    induction: &dyn Fn(u32) -> Option<Induction>,
    steps: &dyn Fn(&Induction, usize, u64) -> (i64, i64),
) -> Option<Counter> {
    let function = shape.function;
    let block = &function.blocks[e];
    let last = block.end - 4;
    let inside = |offset: u64| {
        function
            .at
            .get(&offset)
            .is_some_and(|&b| shape.body[b] && function.blocks[b].start == offset)
    };
    let (taken, condition) = match shape.decoded[&last].instruction {
        Instruction::BranchCond { cond, offset } if cond.0 < 14 => {
            (last.wrapping_add_signed(offset), Ok(cond))
        }
        Instruction::CompareBranch {
            width,
            nonzero,
            rt,
            offset,
        } => (last.wrapping_add_signed(offset), Err((width, nonzero, rt))),
        _ => return None,
    };
    // The condition under which the loop goes on.
    let stays = match (inside(taken), inside(block.end)) {
        (true, false) => true,
        (false, true) => false,
        _ => return None,
    };
    let (bit, width, bound, relation, read_at) = match condition {
        Err((width, nonzero, rt)) => {
            if nonzero != stays {
                return None;
            }
            (
                Registers::bit(rt)?,
                width,
                Value::Imm(0),
                (Relation::Ne, false),
                last,
            )
        }
        Ok(cond) => {
            let cond = if stays { cond } else { Cond(cond.0 ^ 1) };
            // The last instruction before the branch that sets flags must be a comparison.
            let setter = (block.start..last)
                .step_by(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .find(|o| {
                    shape.decoded[o].effects.defs.intersect(Registers::FLAGS) != Registers::NONE
                })?;
            let Instruction::AddSub {
                width,
                subtract: true,
                set_flags: true,
                rn,
                operand,
                ..
            } = shape.decoded[&setter].instruction
            else {
                return None;
            };
            let other = match operand {
                Operand::Imm(k) => Value::Imm(k as i64),
                Operand::Shifted {
                    rm: Reg::X(m),
                    amount: 0,
                    ..
                } => Value::Reg(m.into()),
                _ => return None,
            };
            let rn = Registers::bit(rn)?;
            // Either the first operand moves and the second does not, or the other way round.
            let (bit, bound, swapped) = match other {
                Value::Reg(m) if invariant(rn) => (m, Value::Reg(rn), true),
                Value::Reg(m) if invariant(m) => (rn, other, false),
                Value::Imm(_) => (rn, other, false),
                _ => return None,
            };
            let relation = match (cond.0, swapped) {
                (1, _) => (Relation::Ne, false),
                (3, false) | (8, true) => (Relation::Below { inclusive: false }, false),
                (9, false) | (2, true) => (Relation::Below { inclusive: true }, false),
                (8, false) | (3, true) => (Relation::Above { inclusive: false }, false),
                (2, false) | (9, true) => (Relation::Above { inclusive: true }, false),
                (11, false) | (12, true) => (Relation::Below { inclusive: false }, true),
                (13, false) | (10, true) => (Relation::Below { inclusive: true }, true),
                (12, false) | (11, true) => (Relation::Above { inclusive: false }, true),
                (10, false) | (13, true) => (Relation::Above { inclusive: true }, true),
                _ => return None,
            };
            (bit, width, bound, relation, setter)
        }
    };
    let (relation, signed) = relation;
    let induction = induction(bit)?;
    if induction.width == Width::W32 && width == Width::X64 {
        return None;
    }
    let ahead = match steps(&induction, e, read_at) {
        (1, 0) => 1,
        // The change comes after the comparison, so it must come in every turn that goes on.
        (0, 1)
            if shape
                .latches
                .iter()
                .all(|&l| (shape.dominates)(induction.block, l)) =>
        {
            0
        }
        _ => return None,
    };
    let domain = match (width, signed) {
        (Width::W32, false) => Domain::U32,
        (Width::W32, true) => Domain::S32,
        (Width::X64, false) => Domain::U64,
        (Width::X64, true) => Domain::S64,
    };
    Some(Counter {
        bit,
        induction,
        domain,
        width,
        ahead,
        bound,
        relation,
    })
}

/// The largest step proven loops allow, so that no product here overflows.
const MAX_STEP: i64 = 1 << 24;
/// The most turns a proven loop may run.
const MAX_TURNS: i64 = 1 << 32;
/// Addresses at the loop's entry must lie within this far of zero.
const MAX_ADDRESS: i64 = 1 << 61;

impl FullEmitter<'_> {
    /// Push 1 when every page each region of the loop can reach allows its access, 0 otherwise.
    pub(super) fn proof(&mut self, v: &Versioned) {
        let mark = self.taken.len();
        let turns = self.temporary(ValType::I64);
        let step = self.temporary(ValType::I64);
        let x = self.temporary(ValType::I64);
        let y = self.temporary(ValType::I64);
        let low = self.temporary(ValType::I64);
        let high = self.temporary(ValType::I64);
        // Depth 0 inside this block leaves with 0.
        self.sink()
            .block(BlockType::Result(ValType::I32))
            .block(BlockType::Empty);
        let c = v.counter;
        let domain = c.domain;
        self.step_value(&c.induction, domain, step);
        let (dmin, dmax) = domain.bounds();
        // x: the counter as the comparison reads it in the first turn; y: the bound.
        self.read(Value::Reg(c.bit), domain, x);
        self.read(c.bound, domain, y);
        if matches!(domain, Domain::U64 | Domain::S64) && !matches!(c.relation, Relation::Ne) {
            for v in [x, y] {
                within(&mut self.sink(), v, dmin, dmax, 0);
            }
        }
        let mut s = self.sink();
        match c.relation {
            Relation::Ne => {
                // The step must be 1 or -1; the turns are then (bound - counter) * step - ahead modulo the
                // width, plus one.
                s.local_get(step)
                    .i64_const(1)
                    .i64_add()
                    .i64_const(!2)
                    .i64_and()
                    .i64_const(0)
                    .i64_ne()
                    .br_if(0);
                s.local_get(y)
                    .local_get(x)
                    .i64_sub()
                    .local_get(step)
                    .i64_mul()
                    .i64_const(c.ahead)
                    .i64_sub();
                if c.width == Width::W32 {
                    s.i64_const(0xffff_ffff).i64_and();
                }
                s.local_tee(turns)
                    .i64_const(MAX_TURNS)
                    .i64_ge_u()
                    .br_if(0)
                    .local_get(turns)
                    .i64_const(1)
                    .i64_add()
                    .local_set(turns);
            }
            Relation::Below { inclusive } | Relation::Above { inclusive } => {
                let below = matches!(c.relation, Relation::Below { .. });
                // Steps toward the bound only, and of a size that keeps every product small.
                s.local_get(step);
                if below {
                    s.i64_const(0).i64_le_s();
                } else {
                    s.i64_const(0).i64_ge_s();
                }
                s.local_get(step)
                    .i64_const(MAX_STEP)
                    .i64_add()
                    .i64_const(2 * MAX_STEP)
                    .i64_gt_u()
                    .i32_or()
                    .br_if(0);
                // The bound the counter must reach to stop: y, moved by one when it is inclusive.
                if inclusive {
                    s.local_get(y)
                        .i64_const(if below { 1 } else { -1 })
                        .i64_add()
                        .local_set(y);
                }
                // x moves to the first turn's value.
                s.local_get(x)
                    .i64_const(c.ahead)
                    .local_get(step)
                    .i64_mul()
                    .i64_add()
                    .local_set(x);
                // Turns: 1 when x is already past the bound, otherwise the steps to reach it plus 1.
                if below {
                    s.local_get(x).local_get(y).i64_ge_s();
                } else {
                    s.local_get(x).local_get(y).i64_le_s();
                }
                s.if_(BlockType::Result(ValType::I64)).i64_const(0).else_();
                if below {
                    s.local_get(y)
                        .local_get(x)
                        .i64_sub()
                        .local_get(step)
                        .i64_add()
                        .i64_const(1)
                        .i64_sub()
                        .local_get(step)
                        .i64_div_u();
                } else {
                    s.local_get(x)
                        .local_get(y)
                        .i64_sub()
                        .i64_const(0)
                        .local_get(step)
                        .i64_sub()
                        .i64_add()
                        .i64_const(1)
                        .i64_sub()
                        .i64_const(0)
                        .local_get(step)
                        .i64_sub()
                        .i64_div_u();
                }
                s.end()
                    .local_tee(turns)
                    .i64_const(MAX_TURNS)
                    .i64_ge_u()
                    .br_if(0);
                // The last value compared stays within the domain, so the counter never wraps.
                s.local_get(x)
                    .local_get(turns)
                    .local_get(step)
                    .i64_mul()
                    .i64_add()
                    .local_set(x);
                within(&mut s, x, dmin, dmax, 0);
                s.local_get(turns).i64_const(1).i64_add().local_set(turns);
            }
        }

        for region in &v.regions {
            self.region(region, turns, (step, x, y, low, high));
        }
        self.sink().i32_const(1).br(1).end().i32_const(0).end();
        let released: Vec<_> = self.taken.drain(mark..).collect();
        self.free.extend(released);
    }

    /// Branch out (depth 0) unless every page of `region` allows its access in every turn.
    fn region(
        &mut self,
        r: &Region,
        turns: u32,
        (step, a0, end, low, high): (u32, u32, u32, u32, u32),
    ) {
        // a0: the address in the turn the moving register has taken no step.
        let base = self.register(r.base);
        self.sink().local_get(base);
        if let Some((bit, domain, shift)) = r.index {
            self.read(Value::Reg(bit), domain, end);
            self.sink()
                .local_get(end)
                .i64_const(shift.into())
                .i64_shl()
                .i64_add();
        }
        let mut s = self.sink();
        s.i64_const(r.offset).i64_add().local_set(a0);
        within(&mut s, a0, -MAX_ADDRESS, MAX_ADDRESS - 1, 0);
        match r.moving {
            None => {
                s.local_get(a0).local_tee(low).local_set(high);
            }
            Some(m) => {
                let (domain, shift) = match r.index {
                    Some((bit, domain, shift)) if bit == m.bit => (domain, shift),
                    _ => (Domain::U64, 0),
                };
                self.step_value(&m.induction, domain, step);
                self.sink()
                    .local_get(step)
                    .i64_const(MAX_STEP)
                    .i64_add()
                    .i64_const(2 * MAX_STEP)
                    .i64_gt_u()
                    .br_if(0);
                if matches!(domain, Domain::U32 | Domain::S32) {
                    // The index's last value stays within its 32 bits, so it never wraps.
                    let (dmin, dmax) = domain.bounds();
                    self.read(Value::Reg(m.bit), domain, end);
                    let mut s = self.sink();
                    s.local_get(end)
                        .local_get(turns)
                        .i64_const(m.short)
                        .i64_sub()
                        .local_get(step)
                        .i64_mul()
                        .i64_add()
                        .local_set(end);
                    within(&mut s, end, dmin, dmax, 0);
                }
                // The addresses of the first and last turns, then the lower and the higher of them.
                let mut s = self.sink();
                s.local_get(a0)
                    .i64_const(m.first)
                    .local_get(step)
                    .i64_mul()
                    .i64_const(shift.into())
                    .i64_shl()
                    .i64_add()
                    .local_set(low)
                    .local_get(a0)
                    .local_get(turns)
                    .i64_const(m.short)
                    .i64_sub()
                    .local_get(step)
                    .i64_mul()
                    .i64_const(shift.into())
                    .i64_shl()
                    .i64_add()
                    .local_set(high)
                    .local_get(low)
                    .local_get(high)
                    .local_get(low)
                    .local_get(high)
                    .i64_lt_s()
                    .select()
                    .local_get(low)
                    .local_get(high)
                    .local_get(low)
                    .local_get(high)
                    .i64_gt_s()
                    .select()
                    .local_set(high)
                    .local_set(low);
            }
        }
        // The bytes covered: from low + from up to high + from + len, all within the table.
        let mut s = self.sink();
        s.local_get(low)
            .i64_const(r.from)
            .i64_add()
            .local_tee(low)
            .i64_const(0)
            .i64_lt_s()
            .br_if(0)
            .local_get(high)
            .i64_const(r.from + i64::from(r.len) - 1)
            .i64_add()
            .local_tee(high);
        Table::GLOBALS.pages.push(&mut s);
        s.i64_const(12).i64_shl().i64_ge_s().br_if(0);
        pages_allow(&mut s, low, high, r.this, a0);
    }

    /// Set i64 local `to` to a value as `domain` reads it.
    fn read(&mut self, value: Value, domain: Domain, to: u32) {
        match value {
            Value::Imm(k) => {
                self.sink().i64_const(k).local_set(to);
            }
            Value::Reg(bit) => {
                let local = self.register(bit);
                let mut s = self.sink();
                s.local_get(local);
                match domain {
                    Domain::U32 => {
                        s.i64_const(0xffff_ffff).i64_and();
                    }
                    Domain::S32 => {
                        s.i64_extend32_s();
                    }
                    Domain::U64 | Domain::S64 => {}
                }
                s.local_set(to);
            }
        }
    }

    /// Set i64 local `to` to an induction's step as `domain` reads its changes: sign extended from 32 bits
    /// when only the low 32 bits count.
    fn step_value(&mut self, i: &Induction, domain: Domain, to: u32) {
        let narrow = i.width == Width::W32 || matches!(domain, Domain::U32 | Domain::S32);
        self.read(i.step, if narrow { Domain::S32 } else { Domain::S64 }, to);
        let mut s = self.sink();
        if let Value::Imm(_) = i.step
            && narrow
        {
            s.local_get(to).i64_extend32_s().local_set(to);
        }
        if i.negate {
            s.i64_const(0).local_get(to).i64_sub().local_set(to);
        }
    }
}

/// Branch to `depth` unless the i64 local `v` lies within `min..=max`.
fn within(s: &mut InstructionSink<'_>, v: u32, min: i64, max: i64, depth: u32) {
    s.local_get(v)
        .i64_const(min)
        .i64_lt_s()
        .local_get(v)
        .i64_const(max)
        .i64_gt_s()
        .i32_or()
        .br_if(depth);
}

/// Branch out (depth 0) unless every page from the one holding address `low` to the one holding `high` (i64
/// locals, both within the table) allows an access as `this` says. Eight table bytes are read at a time
/// while eight or more pages are left. Uses `low` and `page` as its own.
fn pages_allow(s: &mut InstructionSink<'_>, low: u32, high: u32, this: u8, page: u32) {
    let every = i64::from(this) * 0x0101_0101_0101_0101;
    s.local_get(low)
        .i64_const(12)
        .i64_shr_u()
        .local_set(page)
        .local_get(high)
        .i64_const(12)
        .i64_shr_u()
        .local_set(high)
        .loop_(BlockType::Empty)
        .local_get(high)
        .local_get(page)
        .i64_sub()
        .i64_const(7)
        .i64_ge_u()
        .if_(BlockType::Empty);
    Table::GLOBALS.at.push(s);
    s.local_get(page)
        .i32_wrap_i64()
        .i32_add()
        .i64_load(memarg(MEMORY_STATE, 0, 0))
        .i64_const(every)
        .i64_and()
        .i64_const(every)
        .i64_ne()
        .br_if(2)
        .local_get(page)
        .i64_const(8)
        .i64_add()
        .local_set(page)
        .else_();
    Table::GLOBALS.at.push(s);
    s.local_get(page)
        .i32_wrap_i64()
        .i32_add()
        .i32_load8_u(memarg(MEMORY_STATE, 0, 0))
        .i32_const(this.into())
        .i32_and()
        .i32_eqz()
        .br_if(2)
        .local_get(page)
        .i64_const(1)
        .i64_add()
        .local_set(page)
        .end()
        .local_get(page)
        .local_get(high)
        .i64_le_u()
        .br_if(0)
        .end();
}
