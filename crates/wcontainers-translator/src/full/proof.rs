//! Loops whose accesses are proven allowed before they run. A loop that makes no call and no system call
//! gets a second copy in which some accesses have no permission check: those whose addresses do not move,
//! those whose index a comparison or a narrow load bounds (a switch's table), and, when the loop has an exit
//! that counts its turns, those whose addresses move by a fixed step each turn. Before the loop every page
//! those accesses can reach in the turns it may run is looked up; the copy runs only when all of them allow
//! the access, otherwise the loop runs as usual, so faults stay exact.

use super::*;
use wcontainers_guest_cpu::{Comparison, Effects, Extend, Relation as Compared, Value as Operand};

/// A value read at the loop's entry: a constant or a register that the loop does not write.
#[derive(Clone, Copy, Debug)]
enum Value {
    Imm(i64),
    Reg(u32),
}

/// A register the loop changes at one instruction only, by adding `step` (negated when `negate`), so it
/// moves by that step at most once a turn.
/// The widths a register is read at.
const U32_BITS: u8 = u32::BITS as u8;
const U64_BITS: u8 = u64::BITS as u8;

#[derive(Clone, Copy, Debug)]
struct Induction {
    at: u64,
    block: usize,
    /// 32 or 64.
    bits: u8,
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
            Domain::U32 => (0, i64::from(u32::MAX)),
            Domain::S32 => (i64::from(i32::MIN), i64::from(i32::MAX)),
            Domain::U64 => (0, 1 << (i64::BITS - 2)),
            Domain::S64 => (-(1 << (i64::BITS - 2)), 1 << (i64::BITS - 2)),
        }
    }

    fn of_extend(extend: Extend) -> Option<Domain> {
        match (extend.from_bits, extend.signed) {
            (U32_BITS, false) => Some(Domain::U32),
            (U32_BITS, true) => Some(Domain::S32),
            (U64_BITS, _) => Some(Domain::U64),
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
    /// The index's largest value, when it is bounded rather than read at the loop's entry (it is at least 0).
    index_max: Option<i64>,
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
    /// 32 or 64.
    bits: u8,
    ahead: i64,
    bound: Value,
    relation: Relation,
}

/// A loop that has a copy without the checks of `regions`.
#[derive(Clone, Debug)]
pub(super) struct Versioned {
    counter: Option<Counter>,
    pub(super) regions: Vec<Region>,
    /// The headers of the loops inside it.
    pub(super) inner: Vec<usize>,
}

/// What [`versioned`] needs to know about the function.
pub(super) struct LoopShape<'a, G: Guest> {
    pub(super) function: &'a Function,
    pub(super) decoded: &'a super::Decodes<G>,
    pub(super) checks: &'a FxHashMap<(u64, u8), CheckPlan>,
    pub(super) body: &'a [bool],
    /// Blocks of the body inside a loop within it.
    pub(super) nested: &'a [bool],
    pub(super) latches: Vec<usize>,
    pub(super) dominates: &'a dyn Fn(usize, usize) -> bool,
}

/// The copy without checks for a loop that makes no call and no system call, if it has a counting exit and
/// accesses whose checks it can leave out. Only the loop's own blocks, run at most once a turn, are looked at
/// for its changes, its exit and its accesses.
pub(super) fn versioned<G: Guest>(shape: &LoopShape<'_, G>) -> Option<Versioned> {
    let function = shape.function;
    let blocks: Vec<usize> = (0..function.blocks.len())
        .filter(|&b| shape.body[b])
        .collect();
    let instructions = || {
        blocks
            .iter()
            .flat_map(|&b| function.instructions(b).iter().map(move |&o| (b, o)))
    };
    let general = Registers::of_class(G::BITS, Some(wcontainers_guest_cpu::Class::General));
    let mut defs: FxHashMap<u32, Vec<(usize, u64)>> = FxHashMap::default();
    for (b, o) in instructions() {
        for bit in shape.decoded[&o].effects.defs.intersect(general).bits() {
            defs.entry(bit).or_default().push((b, o));
        }
    }
    let invariant = |bit: u32| !defs.contains_key(&bit);
    let induction = |bit: u32| -> Option<Induction> {
        let &[(block, at)] = defs.get(&bit)?.as_slice() else {
            return None;
        };
        if shape.nested[block] {
            return None;
        }
        let i = G::induction(&shape.decoded[&at].instruction)?;
        if G::reg_bit(i.reg) != Some(bit) || !matches!(i.bits, U32_BITS | U64_BITS) {
            return None;
        }
        let step = match i.step {
            Operand::Imm(k) => Value::Imm(k),
            Operand::Reg(r) => {
                let m = G::reg_bit(r)?;
                if !invariant(m) {
                    return None;
                }
                Value::Reg(m)
            }
        };
        Some(Induction {
            at,
            block,
            bits: i.bits,
            step,
            negate: i.negate,
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
        .filter(|&&e| !shape.nested[e] && shape.latches.iter().all(|&l| (shape.dominates)(e, l)))
        .find_map(|&e| counting_exit(shape, e, &invariant, &induction, &steps));

    let mut regions = Vec::new();
    for (b, o) in instructions().filter(|&(b, _)| !shape.nested[b]) {
        let Some(access) =
            only_access(&shape.decoded[&o]).filter(|a| a.plain() && a.segment.is_none())
        else {
            continue;
        };
        let (from, len) = match shape.checks.get(&(o, 0)) {
            Some(CheckPlan::Exact) => (0, access.fixed_bytes().expect("a plain access")),
            Some(&CheckPlan::Group { from, len }) => (from, len),
            _ => continue,
        };
        let Some(Base::Register(base)) = access.base else {
            continue;
        };
        // Added to the base before the access: the displacement, and a write back made first.
        let offset = access.displacement
            + match access.write_back {
                WriteBack::Before(by) => by,
                _ => 0,
            };
        let index = match access.index {
            None => None,
            Some(Index::Register { reg, shift, extend }) => {
                let Some(domain) = Domain::of_extend(extend) else {
                    continue;
                };
                let Some(index) = G::reg_bit(reg) else {
                    continue;
                };
                Some((index, domain, shift))
            }
            Some(Index::BitOffset { .. } | Index::Lanes { .. }) => continue,
        };
        let Some(base) = G::reg_bit(base) else {
            continue;
        };
        let moves = |bit: u32, domain: Domain| -> Option<Option<Moving>> {
            if invariant(bit) {
                return Some(None);
            }
            let i = induction(bit)?;
            // A 32 bit change read as 64 bits is not a fixed step from a value with high bits set.
            if i.bits == U32_BITS && matches!(domain, Domain::U64 | Domain::S64) {
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
        let mut index_max = None;
        let index_moves = match index {
            Some((bit, domain, shift)) => match moves(bit, domain) {
                Some(m) => m,
                None => {
                    let Some(max) = index_bound(shape, b, o, bit, domain) else {
                        continue;
                    };
                    if base_moves.is_some() || max << shift > MAX_STEP {
                        continue;
                    }
                    index_max = Some(max);
                    None
                }
            },
            None => None,
        };
        let moving = match (base_moves, index_moves) {
            (Some(_), Some(_)) => continue,
            (m, None) | (None, m) => m,
        };
        // Moving addresses are bounded by the turns, so they need the counting exit.
        if moving.is_some() && counter.is_none() {
            continue;
        }
        regions.push(Region {
            at: o,
            this: if access.kind == AccessKind::Read {
                PAGE_READ
            } else {
                PAGE_WRITE
            },
            base,
            index,
            offset,
            from,
            len,
            moving,
            index_max,
        });
    }
    if regions.is_empty() {
        return None;
    }
    Some(Versioned {
        counter,
        regions,
        inner: Vec::new(),
    })
}

/// The instructions of block `b` before the one at `before`, oldest first, each with its offset and
/// effects: what `Guest::branch_comparison` looks back over.
fn path<G: Guest>(
    shape: &LoopShape<'_, G>,
    b: usize,
    before: u64,
) -> Vec<(u64, G::Instruction, Effects<G>)> {
    shape
        .function
        .instructions(b)
        .iter()
        .take_while(|&&o| o != before)
        .map(|&o| {
            let d = &shape.decoded[&o];
            (o, d.instruction, d.effects)
        })
        .collect()
}

/// The comparison block `b`'s last instruction makes, as it holds on the edge to `to`, which must be the
/// edge it takes or the one it falls through to and not both.
fn comparison_on_edge<G: Guest>(
    shape: &LoopShape<'_, G>,
    b: usize,
    to: impl Fn(u64) -> bool,
) -> Option<Comparison<G::Reg>> {
    let function = shape.function;
    let last = function.last(b);
    let instruction = shape.decoded[&last].instruction;
    let taken = last.wrapping_add_signed(G::flow(&instruction).target?);
    let along = match (to(taken), to(function.blocks[b].end)) {
        (true, false) => true,
        (false, true) => false,
        _ => return None,
    };
    let c = G::branch_comparison(&path(shape, b, last), (last, &instruction))?;
    Some(Comparison {
        relation: if along {
            c.relation
        } else {
            c.relation.negated()
        },
        ..c
    })
}

/// The counting exit at the end of block `e`, if it has one.
fn counting_exit<G: Guest>(
    shape: &LoopShape<'_, G>,
    e: usize,
    invariant: &dyn Fn(u32) -> bool,
    induction: &dyn Fn(u32) -> Option<Induction>,
    steps: &dyn Fn(&Induction, usize, u64) -> (i64, i64),
) -> Option<Counter> {
    let function = shape.function;
    let inside = |offset: u64| {
        function
            .at
            .get(&offset)
            .is_some_and(|&b| shape.body[b] && function.blocks[b].start == offset)
    };
    // The comparison under which the loop goes on.
    let c = comparison_on_edge(shape, e, inside)?;
    let left = G::reg_bit(c.left)?;
    let right = match c.right {
        Operand::Imm(k) => Value::Imm(k),
        Operand::Reg(r) => Value::Reg(G::reg_bit(r)?),
    };
    // Either the first operand moves and the second does not, or the other way round.
    let (bit, bound, relation) = match right {
        Value::Reg(m) if invariant(left) => (m, Value::Reg(left), c.relation.swapped()),
        Value::Reg(m) if invariant(m) => (left, right, c.relation),
        Value::Imm(_) => (left, right, c.relation),
        _ => return None,
    };
    let relation = match relation {
        Compared::Ne => Relation::Ne,
        Compared::Below { inclusive } => Relation::Below { inclusive },
        Compared::Above { inclusive } => Relation::Above { inclusive },
        Compared::Eq => return None,
    };
    let signed = c.signed;
    let induction = induction(bit)?;
    if induction.bits == U32_BITS && c.bits == U64_BITS {
        return None;
    }
    let ahead = match steps(&induction, e, c.at) {
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
    let domain = match (c.bits, signed) {
        (U32_BITS, false) => Domain::U32,
        (U32_BITS, true) => Domain::S32,
        (U64_BITS, false) => Domain::U64,
        (U64_BITS, true) => Domain::S64,
        _ => return None,
    };
    Some(Counter {
        bit,
        induction,
        domain,
        bits: c.bits,
        ahead,
        bound,
        relation,
    })
}

/// The largest value index register `bit` can hold where it is read as `domain` at `at` in block `b`: set
/// earlier in the block by a load of at most 4 bytes that zero extends, or, when the block does not set it,
/// bounded by an unsigned comparison with a constant on the only edge into the block (a switch's guard).
fn index_bound<G: Guest>(
    shape: &LoopShape<'_, G>,
    b: usize,
    at: u64,
    bit: u32,
    domain: Domain,
) -> Option<i64> {
    let function = shape.function;
    let sets = |o: &u64| shape.decoded[o].effects.defs.has_bit(bit);
    // The largest value a zero extending load leaves in the register it loads (not a base it writes back).
    let loaded = |o: u64| match G::loaded_max(&shape.decoded[&o].instruction) {
        Some((r, max)) if G::reg_bit(r) == Some(bit) => Some(max),
        _ => None,
    };
    let block = &function.blocks[b];
    let before = |b: usize, at: u64| {
        let instructions = function.instructions(b);
        let end = instructions.iter().position(|&o| o == at).unwrap();
        instructions[..end].iter().rev().copied()
    };
    if let Some(o) = before(b, at).find(sets) {
        return loaded(o);
    }
    let preds: Vec<usize> = (0..function.blocks.len())
        .filter(|&p| function.blocks[p].succs.contains(&b))
        .collect();
    let &[p] = preds.as_slice() else {
        return None;
    };
    if p == b || b == 0 {
        return None;
    }
    // The comparison that holds on the edge into the block.
    let c = comparison_on_edge(shape, p, |o| o == block.start)?;
    let max = match (c.relation, c.signed, c.right) {
        (Compared::Below { inclusive: false }, false, Operand::Imm(k)) => k - 1,
        (Compared::Below { inclusive: true }, false, Operand::Imm(k)) => k,
        _ => return None,
    };
    let last = function.last(p);
    let between: Vec<u64> = function
        .instructions(p)
        .iter()
        .copied()
        .skip_while(|&o| o != c.at)
        .take_while(|&o| o != last)
        .collect();
    if G::reg_bit(c.left) != Some(bit) || between.iter().any(sets) {
        return None;
    }
    if !(0..=i64::from(i32::MAX)).contains(&max) {
        return None;
    }
    // A 32 bit comparison bounds all 64 bits only when what set the register cleared the top half.
    if c.bits == U32_BITS && matches!(domain, Domain::U64 | Domain::S64) {
        let def = before(p, c.at).find(sets)?;
        loaded(def)?;
    }
    Some(max)
}

/// The largest step proven loops allow, so that no product here overflows.
const MAX_STEP: i64 = 1 << 24;
/// The most turns a proven loop may run.
const MAX_TURNS: i64 = 1 << u32::BITS;
/// Addresses at the loop's entry must lie within this far of zero.
const MAX_ADDRESS: i64 = 1 << 61;

impl<G: Guest> FullEmitter<'_, G> {
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
        if let Some(c) = v.counter {
            self.turns(c, turns, (step, x, y));
        }
        for region in &v.regions {
            self.region(region, turns, (step, x, y, low, high));
        }
        self.sink().i32_const(1).br(1).end().i32_const(0).end();
        let released: Vec<_> = self.taken.drain(mark..).collect();
        self.free.extend(released);
    }

    /// Set i64 local `turns` to the most turns the loop can run, or branch out (depth 0) when that is not
    /// known to be small.
    fn turns(&mut self, c: Counter, turns: u32, (step, x, y): (u32, u32, u32)) {
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
                if c.bits == U32_BITS {
                    s.i64_const(i64::from(u32::MAX)).i64_and();
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
        if let Some((bit, domain, shift)) = r.index.filter(|_| r.index_max.is_none()) {
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
                // A bounded index covers 0 to its largest value.
                let reach = match (r.index, r.index_max) {
                    (Some((_, _, shift)), Some(max)) => max << shift,
                    _ => 0,
                };
                s.local_get(a0)
                    .local_tee(low)
                    .i64_const(reach)
                    .i64_add()
                    .local_set(high);
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
        s.i64_const(i64::from(wcontainers_abi::PAGE_SHIFT))
            .i64_shl()
            .i64_ge_s()
            .br_if(0);
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
                        s.i64_const(i64::from(u32::MAX)).i64_and();
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
        let narrow = i.bits == U32_BITS || matches!(domain, Domain::U32 | Domain::S32);
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
    let every = i64::from(this) * crate::code::EVERY_BYTE;
    s.local_get(low)
        .i64_const(i64::from(wcontainers_abi::PAGE_SHIFT))
        .i64_shr_u()
        .local_set(page)
        .local_get(high)
        .i64_const(i64::from(wcontainers_abi::PAGE_SHIFT))
        .i64_shr_u()
        .local_set(high)
        .loop_(BlockType::Empty)
        .local_get(high)
        .local_get(page)
        .i64_sub()
        .i64_const(crate::code::PAGES_PER_LOAD - 1)
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
        .i64_const(crate::code::PAGES_PER_LOAD)
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
