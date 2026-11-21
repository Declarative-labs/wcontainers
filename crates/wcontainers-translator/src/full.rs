//! Full look-ahead: the blob is cut into functions and each becomes one piece. Inside a piece registers are
//! locals, flags are kept as the last comparison, jumps are blocks and loops, and calls to other pieces are
//! WebAssembly calls.

use crate::code::*;
use crate::contract::*;
use crate::layout::Layout;
use crate::state::{Slot, load_flag, push_word};
use crate::thread::memarg;
use petgraph::algo::dominators;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::DfsPostOrder;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, BTreeSet};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_guest_cpu::{
    Access, AccessKind, Base, Emitter, FlowKind, Guest, Index, MemoryAccess, Registers,
    ReturnAddress, StackMove, Target, Through, TrapKind, TrapPc, WriteBack,
};

mod proof;

/// The functions of a full or bounded translation, found and planned, ready to be emitted in groups.
///
/// Each function becomes the piece the lookup table holds for its entry, and each of its loops with a loop
/// piece one more function. A loop piece starts at the header of a loop of a function and holds that loop and
/// what follows it up to the header of any other loop, going on there in that loop's piece. Only a stop check
/// that carries on at a jump back enters one, so it may rely on what its function knows at the loop. That way
/// a thread carries on in a new call, which runs whatever code the engine has made for the function by then,
/// and the function itself has no second way into its loops.
pub(crate) struct Planned<'a, G: Guest> {
    finder: Finder<'a, G>,
    functions: Vec<Function>,
    plans: Vec<Plan<G>>,
    /// Each function's loop pieces, with their plans.
    loops: Vec<Vec<(Function, Plan<G>)>>,
    omit: Omit,
    optimise: Optimise,
}

/// Find and plan the functions of a translation from `starts`: the whole blob's (`bound` none) or one stretch
/// of at most `bound` instructions from each start.
pub(crate) fn plan<'a, G: Guest>(
    code: &'a Code<'a, G>,
    starts: &[u64],
    bound: Option<usize>,
    omit: Omit,
    optimise: Optimise,
) -> Planned<'a, G> {
    let mut finder = Finder {
        code,
        decoded: Decodes::default(),
        means: FxHashMap::default(),
        wholes: FxHashMap::default(),
    };
    let functions = match bound {
        None => finder.functions(starts),
        Some(limit) => finder.stretches(starts, limit),
    };
    let entries: BTreeSet<u64> = functions.iter().map(|f| f.entry).collect();
    let plans: Vec<Plan<G>> = functions
        .par_iter()
        .map(|f| Plan::new(f, &finder.decoded, Seed::NOTHING))
        .collect();

    // Loop pieces, by the function they come from; with no stop checks nothing enters them. Translated for
    // size, there are none: a stop at a jump back carries on in place.
    let pieced = !omit.stop && optimise == Optimise::Speed;
    let loops: Vec<Vec<(Function, Plan<G>)>> = functions
        .par_iter()
        .zip(&plans)
        .map(|(function, plan)| {
            if !pieced || plan.outside.is_empty() {
                return Vec::new();
            }
            // A stretch's loop piece stays within the stretch.
            let within: Option<FxHashSet<u64>> =
                bound.map(|_| function_instructions(function).collect());
            let reach = match &within {
                None => Reach::All,
                Some(within) => Reach::Within(within),
            };
            plan.outside
                .iter()
                .filter_map(|(h, others)| {
                    let headers: Vec<u64> =
                        others.iter().map(|&d| function.blocks[d].start).collect();
                    let start = function.blocks[*h].start;
                    let bounds = (&entries, &headers[..]);
                    let piece = finder.explore(start, bounds, &mut Found::new(), reach);
                    // Where a loop piece would need more entries, the stop check carries on in place instead.
                    if !irreducible(&piece).is_empty() {
                        return None;
                    }
                    let seed = Seed {
                        frame: plan.frame_in[*h],
                        aligned: plan.aligned_in[*h],
                    };
                    let plan = Plan::new(&piece, &finder.decoded, seed);
                    Some((piece, plan))
                })
                .collect()
        })
        .collect();
    Planned {
        finder,
        functions,
        plans,
        loops,
        omit,
        optimise,
    }
}

impl<G: Guest> Planned<'_, G> {
    /// The entries of the pieces the lookup table holds, in order.
    pub(crate) fn entries(&self) -> Vec<u64> {
        self.functions.iter().map(|f| f.entry).collect()
    }

    /// What made an entry one: [`Means::Given`] for every start of the plan.
    pub(crate) fn means(&self, entry: u64) -> Means {
        self.finder
            .means
            .get(&entry)
            .copied()
            .unwrap_or(Means::Given)
    }

    /// Offsets that cut the `i`th function in two, as starts.
    pub(crate) fn halves(&self, i: usize) -> Vec<u64> {
        cuts(&self.functions[i], 2)
    }

    /// The pieces `group` of the table's (by their order), each with its loop pieces, emitted for a module that
    /// holds that group alone: the table's pieces take the function indices from [`F_FIRST_PIECE`] in order,
    /// then the loop pieces follow in order. Code outside the group is reached through the lookup.
    pub(crate) fn emit(&self, group: std::ops::Range<usize>) -> Vec<Unit> {
        let code = self.finder.code;
        let index: BTreeMap<u64, u32> = self.functions[group.clone()]
            .iter()
            .enumerate()
            .map(|(i, f)| (f.entry, G::F_FIRST_PIECE + i as u32))
            .collect();
        let mut next = G::F_FIRST_PIECE + group.len() as u32;
        let loop_index: Vec<FxHashMap<u64, u32>> = group
            .clone()
            .map(|i| {
                self.loops[i]
                    .iter()
                    .map(|(piece, _)| {
                        next += 1;
                        (piece.entry, next - 1)
                    })
                    .collect()
            })
            .collect();
        let decoded = &self.finder.decoded;
        group
            .clone()
            .into_par_iter()
            .zip(&loop_index)
            .map(|(i, loops)| {
                let function = &self.functions[i];
                let whole = match self.finder.wholes.get(&function.entry) {
                    Some(whole) => whole.clone(),
                    None => Whole::stretch(function, code),
                };
                let emit = |function, plan, own| {
                    let how = (self.omit, self.optimise, code);
                    let pieces = (&index, loops, own);
                    FullEmitter::piece(function, &whole, plan, decoded, pieces, how)
                };
                let (piece, mut locals, mut sites) =
                    emit(function, &self.plans[i], index[&function.entry]);
                let others = self.loops[i]
                    .iter()
                    .map(|(piece, plan)| {
                        let (f, n, more) = emit(piece, plan, loops[&piece.entry]);
                        locals = locals.max(n);
                        add_sites(&mut sites, &more);
                        f
                    })
                    .collect();
                Unit {
                    piece,
                    others,
                    locals,
                    sites,
                }
            })
            .collect()
    }
}

/// A run of instructions entered only at its start and left only at its end.
struct Block {
    start: u64,
    /// Just past the last instruction.
    end: u64,
    /// Where its instructions' offsets are in its function's `offsets`.
    instructions: std::ops::Range<usize>,
    succs: Vec<usize>,
    /// The targets of the jump table this block ends with, if it ends with one.
    table: Option<Vec<u64>>,
}

/// One function: everything reachable from its entry without passing another entry. Block 0 is the entry.
struct Function {
    entry: u64,
    blocks: Vec<Block>,
    /// Block index by start offset.
    at: FxHashMap<u64, usize>,
    /// The offsets of the blocks' instructions, each block's in order.
    offsets: Vec<u64>,
}

impl Function {
    /// The offsets of block `b`'s instructions, in order.
    fn instructions(&self, b: usize) -> &[u64] {
        &self.offsets[self.blocks[b].instructions.clone()]
    }

    /// The offset of block `b`'s last instruction.
    fn last(&self, b: usize) -> u64 {
        *self
            .instructions(b)
            .last()
            .expect("a block has an instruction")
    }
}

/// The function a piece's code belongs to, as found before it was cut in parts.
#[derive(Clone)]
struct Whole {
    entry: u64,
    /// From its lowest instruction's offset to past its highest's, or to the next function's entry where that
    /// is further: code there that exploring did not reach (cases of a table not followed, labels of a computed
    /// goto) is taken as its own.
    span: std::ops::Range<u64>,
}

impl Whole {
    fn of(function: &Function) -> Whole {
        let offsets = || function.offsets.iter().copied();
        let low = offsets().min().expect("a function has an instruction");
        let high = offsets().max().expect("a function has an instruction");
        Whole {
            entry: function.entry,
            span: low..high + 1,
        }
    }

    /// A stretch knows no function's bounds, so all of its code may be its own.
    fn stretch<G: Guest>(stretch: &Function, code: &Code<'_, G>) -> Whole {
        Whole {
            entry: stretch.entry,
            span: code.range.clone(),
        }
    }

    /// The same, reaching up to `next`, the next function's entry.
    fn up_to(self, next: u64) -> Whole {
        Whole {
            span: self.span.start..self.span.end.max(next),
            ..self
        }
    }
}

struct Finder<'a, G: Guest> {
    code: &'a Code<'a, G>,
    decoded: Decodes<G>,
    /// What made each function's entry one, where it is not a start given.
    means: FxHashMap<u64, Means>,
    /// The function each part of a function cut in parts comes from, by the part's entry.
    wholes: FxHashMap<u64, Whole>,
}

/// Entries a function shows, with what shows each.
type Found = BTreeMap<u64, Means>;

/// Note an entry found, keeping the first listed means where several find it.
fn note(found: &mut Found, offset: u64, means: Means) {
    let kept = found.entry(offset).or_insert(means);
    *kept = (*kept).min(means);
}

/// Decoded instructions by offset.
type Decodes<G> = FxHashMap<u64, Decoded<G>>;

/// How far exploring a function may go.
#[derive(Clone, Copy)]
enum Reach<'a> {
    All,
    /// At most this many instructions.
    Count(usize),
    /// Only these instructions.
    Within(&'a FxHashSet<u64>),
}

/// The function after [`Layout::F_FLAGS_WORD`] that works out the flags word for a shape.
fn flags_word_function<G: Guest>(shape: G::Shape) -> u32 {
    let i = G::SHAPES
        .iter()
        .position(|s| *s == shape)
        .expect("every shape has one");
    G::F_FLAGS_WORD + 1 + i as u32
}

/// The functions that work out the flags word for a write back on the way to the runner, with their
/// parameters, all locals of the piece: first from the tracked flags, then for each of the guest's shapes in
/// order from its kept locals. Out of line, as such a write back is rare and the same everywhere.
pub(crate) fn flags_word_functions<G: Guest>() -> Vec<(Vec<ValType>, wasm_encoder::Function)> {
    let mut functions = Vec::new();
    if G::FLAGS.is_empty() {
        return functions;
    }
    let mut f = wasm_encoder::Function::new([]);
    let mut s = f.instructions();
    push_word::<G>(&mut s, |s, flag| {
        s.local_get(G::flag_index(flag) as u32);
    });
    s.end();
    functions.push((vec![ValType::I32; G::FLAGS.len()], f));
    for &shape in G::SHAPES {
        let params = G::shape_locals(shape).to_vec();
        let n = params.len() as u32;
        let locals: Vec<u32> = (0..n).collect();
        let flags = G::with_locals(shape, &locals);
        let ty = params[0];
        let mut f = wasm_encoder::Function::new([(2, ty)]);
        let mut s = f.instructions();
        let flags = G::complete(&mut s, flags, [n, n + 1]);
        push_word::<G>(&mut s, |s, flag| G::flag_value(s, flags, flag));
        s.end();
        functions.push((params, f));
    }
    assert_eq!(functions.len() as u32, G::FLAGS_WORDS);
    functions
}

/// Whether control can leave an instruction other than by going on to the next one or by a call that returns.
fn ends_block<G: Guest>(instruction: &G::Instruction) -> bool {
    let flow = G::flow(instruction);
    let call = flow.kind == FlowKind::Call;
    !(flow.falls_through && (flow.target.is_none() || call))
}

impl<G: Guest> Finder<'_, G> {
    /// The instruction at an offset inside the code, decoded again unless it is kept; `None` when it would
    /// run past the end of the code.
    fn try_get(&self, offset: u64) -> Option<Decoded<G>> {
        match self.decoded.get(&offset) {
            Some(&d) => Some(d),
            None => self.code.decode(offset),
        }
    }

    /// An instruction of a function found.
    fn get(&self, offset: u64) -> Decoded<G> {
        self.try_get(offset).expect("a whole instruction")
    }

    /// Keep every instruction of these functions decoded, for planning and emitting them.
    fn keep(&mut self, functions: &[Function]) {
        let code = self.code;
        let decoded = &self.decoded;
        let missing: Vec<(u64, Decoded<G>)> = functions
            .par_iter()
            .flat_map_iter(|f| function_instructions(f).filter(|o| !decoded.contains_key(o)))
            .map(|o| (o, code.decode(o).expect("a whole instruction")))
            .collect();
        self.decoded.extend(missing);
    }

    /// Every function: the starts, targets of direct calls, addresses the code builds that point into code,
    /// the extra entries that keep every function's control flow reducible, and those that cut functions too
    /// large in parts.
    fn functions(&mut self, starts: &[u64]) -> Vec<Function> {
        let mut entries: BTreeSet<u64> = starts.iter().copied().collect();
        let mut explored = BTreeSet::new();
        loop {
            // In rounds: every entry not yet explored, all at once, then what they found.
            let mut work: Vec<u64> = entries.difference(&explored).copied().collect();
            while !work.is_empty() {
                explored.extend(&work);
                let found: Vec<Found> = work
                    .par_iter()
                    .map(|&entry| {
                        let mut found = Found::new();
                        self.explore(entry, (&entries, &[]), &mut found, Reach::All);
                        found
                    })
                    .collect();
                work = Vec::new();
                let mut round = Found::new();
                for (offset, means) in found.into_iter().flatten() {
                    if self.code.inside(offset) && !entries.contains(&offset) {
                        note(&mut round, offset, means);
                    }
                }
                for (offset, means) in round {
                    entries.insert(offset);
                    self.means.insert(offset, means);
                    work.push(offset);
                }
            }
            let functions: Vec<Function> = entries
                .par_iter()
                .map(|&entry| self.explore(entry, (&entries, &[]), &mut Found::new(), Reach::All))
                .collect();
            let more_entries: Vec<Vec<(u64, Means)>> = functions
                .par_iter()
                .map(|function| {
                    let parts = function.blocks.len().div_ceil(FUNCTION_BLOCKS);
                    let reducible = irreducible(function).into_iter();
                    let cut = cuts(function, parts).into_iter();
                    reducible
                        .map(|o| (o, Means::Reducible))
                        .chain(cut.map(|o| (o, Means::Cut)))
                        .collect()
                })
                .collect();
            let mut more = false;
            for (function, found) in functions.iter().zip(more_entries) {
                for (offset, means) in found {
                    if entries.insert(offset) {
                        self.means.insert(offset, means);
                        let whole = self
                            .wholes
                            .entry(function.entry)
                            .or_insert_with(|| Whole::of(function))
                            .clone();
                        self.wholes.insert(offset, whole);
                        more = true;
                    }
                }
            }
            if !more {
                self.keep(&functions);
                self.reach_next_functions(&functions, &entries);
                return functions;
            }
        }
    }

    /// Give every function's whole, and each of its parts', the code up to the next start of a function.
    fn reach_next_functions(&mut self, functions: &[Function], entries: &BTreeSet<u64>) {
        let means = &self.means;
        let starts: BTreeSet<u64> = entries
            .iter()
            .copied()
            .filter(|e| means.get(e).is_none_or(|m| m.starts_function()))
            .collect();
        let end = self.code.range.end;
        for function in functions {
            let whole = match self.wholes.get(&function.entry) {
                Some(whole) => whole.clone(),
                None => Whole::of(function),
            };
            let next = starts
                .range(whole.entry + 1..)
                .next()
                .copied()
                .unwrap_or(end);
            self.wholes.insert(function.entry, whole.up_to(next));
        }
    }

    /// One stretch per start: what the code reaches from it in its first `limit` instructions, stopping at the
    /// other starts and at the blocks that would make its control flow irreducible.
    fn stretches(&mut self, starts: &[u64], limit: usize) -> Vec<Function> {
        let starts: BTreeSet<u64> = starts.iter().copied().collect();
        let mut bounds = starts.clone();
        loop {
            let stretches: Vec<Function> = starts
                .par_iter()
                .map(|&s| {
                    let reach = Reach::Count(limit);
                    self.explore(s, (&bounds, &[]), &mut Found::new(), reach)
                })
                .collect();
            let mut more = false;
            for stretch in &stretches {
                let parts = stretch.blocks.len().div_ceil(FUNCTION_BLOCKS);
                for offset in irreducible(stretch).into_iter().chain(cuts(stretch, parts)) {
                    more |= bounds.insert(offset);
                }
            }
            if !more {
                self.keep(&stretches);
                return stretches;
            }
        }
    }

    /// The function at an entry, stopping at the other entries and at the other offsets given with them,
    /// with what it shows of other entries added to `found`.
    fn explore(
        &self,
        entry: u64,
        (entries, others): (&BTreeSet<u64>, &[u64]),
        found: &mut Found,
        reach: Reach<'_>,
    ) -> Function {
        let mut body = BTreeMap::new();
        let mut leaders = BTreeSet::from([entry]);
        let mut tables: FxHashMap<u64, Vec<u64>> = FxHashMap::default();
        // Addresses the code builds; those a jump table of this function goes to are its own blocks.
        let mut built = Vec::new();
        let mut work = vec![entry];
        while let Some(offset) = work.pop() {
            if !self.code.inside(offset)
                || (offset != entry && (entries.contains(&offset) || others.contains(&offset)))
                || body.contains_key(&offset)
            {
                continue;
            }
            match reach {
                Reach::All => {}
                Reach::Count(limit) if body.len() < limit => {}
                Reach::Within(allowed) if allowed.contains(&offset) => {}
                _ => continue,
            }
            let Some(decoded) = self.try_get(offset) else {
                continue;
            };
            let length = u64::from(decoded.length);
            body.insert(offset, length);
            let instruction = decoded.instruction;
            let flow = G::flow(&instruction);
            let call = flow.kind == FlowKind::Call;
            let mut jumps = Vec::new();
            if call && let Some(by) = flow.target {
                note(found, offset.wrapping_add_signed(by), Means::Call);
            }
            let blob = self.code.blob;
            let before = |o: u64| self.before(&body, &leaders, o);
            if let Some(address) =
                G::built_address(offset, &instruction, blob, self.code.range.clone(), before)
            {
                built.push(address);
            }
            if let FlowKind::JumpThrough(Through::Register(r)) = flow.kind
                && let Some(targets) = self.jump_table(offset, r, before)
            {
                jumps.extend(&targets);
                tables.insert(offset, targets);
            }
            if let Some(by) = flow.target
                && !call
            {
                jumps.push(offset.wrapping_add_signed(by));
            }
            leaders.extend(&jumps);
            work.extend(jumps);
            // The next instruction is seen first, so a bounded stretch follows the code in order.
            if flow.falls_through {
                if ends_block::<G>(&instruction) {
                    leaders.insert(offset + length);
                }
                work.push(offset + length);
            }
        }

        let switched: FxHashSet<u64> = tables.values().flatten().copied().collect();
        for b in built.into_iter().filter(|b| !switched.contains(b)) {
            note(found, b, Means::Built);
        }

        // Cut the body into blocks, the entry's first.
        let mut blocks: Vec<Block> = Vec::new();
        let mut offsets = Vec::with_capacity(body.len());
        let mut previous: Option<(u64, u64)> = None;
        // The furthest end of the instructions before `previous`: only past it can another end where it does.
        let mut furthest = 0;
        for (&offset, &length) in &body {
            let joined = previous.is_some_and(|(p, n)| {
                p + n == offset
                    && !leaders.contains(&offset)
                    && !ends_block::<G>(&self.get(p).instruction)
                    // Where two overlapping streams meet again, the offset leads a block of its own.
                    && (furthest < offset || ending_at::<G>(&body, offset).nth(1).is_none())
            });
            offsets.push(offset);
            if joined {
                let block = blocks.last_mut().unwrap();
                block.end = offset + length;
                block.instructions.end += 1;
            } else {
                blocks.push(Block {
                    start: offset,
                    end: offset + length,
                    instructions: offsets.len() - 1..offsets.len(),
                    succs: Vec::new(),
                    table: None,
                });
            }
            if let Some((p, n)) = previous {
                furthest = furthest.max(p + n);
            }
            previous = Some((offset, length));
        }
        let first = blocks.iter().position(|b| b.start == entry).unwrap();
        blocks.swap(0, first);
        let at: FxHashMap<u64, usize> = blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.start, i))
            .collect();
        for block in &mut blocks {
            let last = offsets[block.instructions.end - 1];
            let instruction = self.get(last).instruction;
            let flow = G::flow(&instruction);
            let mut targets = Vec::new();
            if let Some(by) = flow.target
                && ends_block::<G>(&instruction)
            {
                targets.push(last.wrapping_add_signed(by));
            }
            if flow.falls_through {
                targets.push(block.end);
            }
            if let Some(table) = tables.get(&last) {
                targets.extend(table);
                block.table = Some(table.clone());
            }
            block.succs = targets
                .into_iter()
                .filter(|t| body.contains_key(t))
                .map(|t| at[&t])
                .collect();
        }
        Function {
            entry,
            blocks,
            at,
            offsets,
        }
    }

    /// The instruction of the walk so far that falls through into `offset`, with its offset; `None` when
    /// none does, two do or a jump is known to enter there.
    fn before(
        &self,
        body: &BTreeMap<u64, u64>,
        leaders: &BTreeSet<u64>,
        offset: u64,
    ) -> Option<(u64, G::Instruction)> {
        if leaders.contains(&offset) {
            return None;
        }
        let mut ending = ending_at::<G>(body, offset);
        let p = ending.next()?;
        if ending.next().is_some() {
            return None;
        }
        let instruction = self.get(p).instruction;
        (G::flow(&instruction).falls_through && !ends_block::<G>(&instruction))
            .then_some((p, instruction))
    }

    /// The targets of a switch's jump table ending at the jump through a register at `offset`, if the code
    /// before it has that shape and every target is code.
    fn jump_table(
        &self,
        offset: u64,
        r: G::Reg,
        before: impl Fn(u64) -> Option<(u64, G::Instruction)>,
    ) -> Option<Vec<u64>> {
        let targets = G::jump_table(offset, r, self.code.blob, self.code.range.clone(), before)?;
        targets
            .iter()
            .all(|&t| self.code.inside(t))
            .then_some(targets)
    }
}

/// The instructions of `body` that end at `offset`, nearest first. Instructions may overlap, so the one
/// ending there need not be the last to start before it.
fn ending_at<G: Guest>(body: &BTreeMap<u64, u64>, offset: u64) -> impl Iterator<Item = u64> + '_ {
    (offset.saturating_sub(G::MAX_LENGTH)..offset)
        .rev()
        .filter(move |p| body.get(p).is_some_and(|&n| p + n == offset))
}

/// Offsets that cut a function in `parts`: the places control passes from one run of its instructions to
/// another, with blocks in the order a depth first walk first reaches them, so that what follows a block
/// tends to be in its run. Made entries, they leave each run a function of its own.
fn cuts(function: &Function, parts: usize) -> Vec<u64> {
    if parts <= 1 {
        return Vec::new();
    }
    let (_, rank) = ranks(function);
    let mut order: Vec<usize> = (0..function.blocks.len()).collect();
    order.sort_unstable_by_key(|&b| rank[b]);
    // Each instruction's place in that order.
    let mut place: FxHashMap<u64, usize> = FxHashMap::default();
    for &b in &order {
        for &offset in function.instructions(b) {
            let next = place.len();
            place.insert(offset, next);
        }
    }
    let count = place.len();
    let part = |offset: u64| place[&offset] * parts / count;
    let mut found = Vec::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for pair in function.instructions(b).windows(2) {
            if part(pair[1]) != part(pair[0]) {
                found.push(pair[1]);
            }
        }
        let from = part(function.last(b));
        for &s in &block.succs {
            let to = function.blocks[s].start;
            if part(to) != from {
                found.push(to);
            }
        }
    }
    found
}

/// The order blocks are first reached in a depth first walk from the entry, reversed: each block's rank.
fn ranks(function: &Function) -> (DiGraph<(), ()>, Vec<usize>) {
    let mut graph = DiGraph::<(), ()>::new();
    for _ in &function.blocks {
        graph.add_node(());
    }
    for (i, block) in function.blocks.iter().enumerate() {
        for &s in &block.succs {
            graph.add_edge(NodeIndex::new(i), NodeIndex::new(s), ());
        }
    }
    let mut post = Vec::new();
    let mut dfs = DfsPostOrder::new(&graph, NodeIndex::new(0));
    while let Some(node) = dfs.next(&graph) {
        post.push(node.index());
    }
    let mut rank = vec![0; function.blocks.len()];
    for (r, &b) in post.iter().rev().enumerate() {
        rank[b] = r;
    }
    (graph, rank)
}

/// Starts of blocks entered by a jump back that does not come from inside their own loop. Making them
/// entries of their own makes the function's control flow reducible.
fn irreducible(function: &Function) -> Vec<u64> {
    let (graph, rank) = ranks(function);
    let doms = dominators::simple_fast(&graph, NodeIndex::new(0));
    let dominates = |a: usize, b: usize| {
        doms.dominators(NodeIndex::new(b))
            .is_some_and(|mut d| d.any(|n| n.index() == a))
    };
    let mut out = Vec::new();
    for (u, block) in function.blocks.iter().enumerate() {
        for &v in &block.succs {
            if rank[v] <= rank[u] && !dominates(v, u) {
                out.push(function.blocks[v].start);
            }
        }
    }
    out
}

/// Registers that live in a piece's locals and are tracked between its global copies: everything but the
/// registers passed as parameters, and the flags, which have their own state.
fn tracked<G: Guest>() -> Registers {
    G::TRACKED
}

/// Where the flags are known to be at a point: a set of flag state bits, one bit when it is known which:
/// [`FLAGS_IN_LOCALS`], [`FLAGS_IN_BLOCK`], then one for each shape a function uses (at most 126).
type FlagStates = u128;

/// The states in a set of flag states, lowest first.
fn state_ids(mut states: FlagStates) -> Vec<u32> {
    let mut ids = Vec::with_capacity(states.count_ones() as usize);
    while states != 0 {
        ids.push(states.trailing_zeros());
        states &= states - 1;
    }
    ids
}

/// The flags are in the piece's flag locals.
const FLAGS_IN_LOCALS: u32 = 0;
/// The flags are in the thread block, which is up to date.
const FLAGS_IN_BLOCK: u32 = 1;
/// The flags are as the function's first shape keeps them; the other shapes' states follow in order.
const FIRST_SHAPE_STATE: u32 = FLAGS_IN_BLOCK + 1;
/// The most flags a guest may keep in locals of their own.
const MAX_TRACKED_FLAGS: usize = 8;

/// What a check before a memory access does.
#[derive(Clone, Copy, Debug)]
enum CheckPlan {
    /// An earlier check in the piece already covers it.
    Skip,
    /// Check the access itself and fault exactly.
    Exact,
    /// Check, at the first of several accesses through one base, every byte they touch: `len` bytes from
    /// `from` bytes past this access. If any may not be touched, continue at this instruction from code that
    /// checks each access.
    Group { from: i64, len: u32 },
}

/// Bytes relative to the stack pointer whose pages are known to allow reading and writing.
type Frame = Option<(i64, i64)>;

/// What the analysis of one function decides.
struct Plan<G: Guest> {
    rank: Vec<usize>,
    children: Vec<Vec<usize>>,
    merge: Vec<bool>,
    loop_header: Vec<bool>,
    /// Registers that must hold their true value at the start of each block; the entry's are loaded.
    live_in: Vec<Registers>,
    /// Registers that must hold their true value after each instruction.
    live_after: FxHashMap<u64, Registers>,
    dirty_in: Vec<Registers>,
    /// By loop header, the registers written back to the thread block when the loop is entered.
    written_before: Vec<Registers>,
    flags_in: Vec<FlagStates>,
    aligned_in: Vec<bool>,
    /// Shape of every flag setting kept lazily, by its flag state number.
    shapes: Vec<G::Shape>,
    /// Some point does not know statically where the flags are, so a local says.
    dynamic_flags: bool,
    /// By instruction and which of its accesses.
    checks: FxHashMap<(u64, u8), CheckPlan>,
    /// Ranges relative to the stack pointer before the instruction, checked as the stack grows.
    frame_checks: FxHashMap<u64, (i64, u32)>,
    /// Calls after which a frame is known, so the stack pointer must come back unchanged.
    frame_across: FxHashSet<u64>,
    /// The piece calls or jumps to other pieces, so its entry needs the stop check.
    calls_out: bool,
    /// What is known at each block's start about the stack: its frame and whether it is aligned.
    frame_in: Vec<Frame>,
    /// Every loop header but the entry, with the headers of the loops it is not around.
    outside: Vec<(usize, Vec<usize>)>,
    /// Accesses checked exactly inside a loop that makes no call or system call keep the run of pages last
    /// found to allow them and check again only when an access does not lie within that run. Accesses of
    /// one kind (read or write) through one base register in one loop share what they keep. Each such access's number for
    /// what it keeps, by number the longest of those accesses in bytes, and by loop header, the numbers
    /// forgotten when the loop is entered.
    kept: FxHashMap<u64, usize>,
    kept_longest: Vec<u32>,
    kept_by_loop: Vec<Vec<usize>>,
    /// By loop header, the loops that have a copy without the checks their accesses are proven not to need.
    versioned: FxHashMap<usize, proof::Versioned>,
    /// By the header of such a loop, the blocks outside it placed after both copies (and no longer under the
    /// block in the loop that dominates them), so either copy reaches them by a branch.
    after: Vec<Vec<usize>>,
    /// The accesses whose checks those copies leave out, with the header of the loop whose proof covers them.
    proven: FxHashMap<u64, usize>,
    /// By edge leaving loops, registers those loops keep in the thread block: the ones needed after it are
    /// loaded there.
    reload_on_exit: FxHashMap<(usize, usize), Registers>,
}

/// How the stack pointer moves at an instruction: as the guest says, or to something unknown when it is
/// written otherwise.
fn stack_move<G: Guest>(d: &Decoded<G>) -> StackMove {
    match G::stack_move(&d.instruction) {
        StackMove::None if d.effects.defs.has_bit(G::SP_BIT) => StackMove::Unknown,
        movement => movement,
    }
}

/// The register bit of an access's base, when it is a register that has one.
fn base_bit<G: Guest>(access: &MemoryAccess<G::Reg>) -> Option<u32> {
    match access.base? {
        Base::Register(r) => G::reg_bit(r),
        Base::Pc => None,
    }
}

/// The one memory access of an instruction, when it makes exactly one.
fn only_access<G: Guest>(d: &Decoded<G>) -> Option<MemoryAccess<G::Reg>> {
    match G::accesses(&d.instruction) {
        [Some(access), None] => Some(access),
        _ => None,
    }
}

/// Whether the stack pointer is known to be a multiple of 16 after an instruction: an access through it checks
/// that, where the guest checks it, and moving it by a multiple of 16 keeps it.
fn aligned_after<G: Guest>(d: &Decoded<G>, aligned: bool) -> bool {
    let checked = G::SP_ALIGNMENT_CHECKED
        && G::accesses(&d.instruction)
            .iter()
            .flatten()
            .any(|a| base_bit::<G>(a) == Some(G::SP_BIT));
    match stack_move(d) {
        StackMove::None => aligned || checked,
        StackMove::By { delta, .. } => (aligned || checked) && delta % 16 == 0,
        StackMove::Unknown => false,
    }
}

/// Shift a frame to a stack pointer moved by `delta`. What is left below the red zone under the stack
/// pointer is no frame, so it is dropped.
fn shift<G: Guest>(frame: Frame, delta: i64) -> Frame {
    frame.and_then(|(lo, hi)| {
        let (lo, hi) = ((lo - delta).max(-(G::RED_ZONE as i64)), hi - delta);
        (lo < hi).then_some((lo, hi))
    })
}

/// The frame after an instruction: moved as the stack pointer moves, and after a call that leaves its
/// return address on the stack, back up past it, where the callee's return leaves the stack pointer.
fn frame_after<G: Guest>(d: &Decoded<G>, frame: Frame) -> Frame {
    let frame = match stack_move(d) {
        StackMove::None => frame,
        StackMove::Unknown => None,
        StackMove::By { delta, .. } => shift::<G>(grow::<G>(frame, delta), delta),
    };
    match G::RETURN_ADDRESS {
        ReturnAddress::Stack { bytes } if d.effects.calls => shift::<G>(frame, bytes.into()),
        _ => frame,
    }
}

fn covers(frame: Frame, lo: i64, hi: i64) -> bool {
    frame.is_some_and(|(a, b)| a <= lo && hi <= b)
}

/// What is known at a piece's entry about the stack: nothing for a piece any caller may enter; for a loop
/// piece, what its function knows at that loop.
#[derive(Clone, Copy)]
struct Seed {
    frame: Frame,
    aligned: bool,
}

impl Seed {
    const NOTHING: Seed = Seed {
        frame: None,
        aligned: false,
    };
}

impl<G: Guest> Plan<G> {
    fn new(function: &Function, decoded: &Decodes<G>, seed: Seed) -> Plan<G> {
        let blocks = &function.blocks;
        let n = blocks.len();
        let (graph, rank) = ranks(function);

        // Loops, and for each the loops around it.
        let mut preds = vec![Vec::new(); n];
        for (u, block) in blocks.iter().enumerate() {
            for &v in &block.succs {
                preds[v].push(u);
            }
        }
        let headers: Vec<usize> = (0..n)
            .filter(|&h| preds[h].iter().any(|&u| rank[h] <= rank[u]))
            .collect();
        let body = |h: usize| -> Vec<bool> {
            let mut inside = vec![false; n];
            inside[h] = true;
            let mut work: Vec<usize> = preds[h]
                .iter()
                .copied()
                .filter(|&u| rank[h] <= rank[u])
                .collect();
            while let Some(u) = work.pop() {
                if !inside[u] {
                    inside[u] = true;
                    work.extend(preds[u].iter().copied());
                }
            }
            inside
        };
        let bodies: FxHashMap<usize, Vec<bool>> = headers.iter().map(|&h| (h, body(h))).collect();
        let outside = headers
            .iter()
            .copied()
            .filter(|&h| h != 0)
            .map(|h| {
                let others = headers
                    .iter()
                    .copied()
                    .filter(|&d| d != h && !bodies[&h][d])
                    .collect();
                (h, others)
            })
            .collect();
        let doms = dominators::simple_fast(&graph, NodeIndex::new(0));
        let mut children = vec![Vec::new(); n];
        for b in 1..n {
            let idom = doms.immediate_dominator(NodeIndex::new(b)).unwrap().index();
            children[idom].push(b);
        }
        let mut forward_in = vec![0; n];
        let mut loop_header = vec![false; n];
        let mut merge = vec![false; n];
        for (u, block) in blocks.iter().enumerate() {
            for &v in &block.succs {
                if rank[v] <= rank[u] {
                    loop_header[v] = true;
                } else {
                    forward_in[v] += 1;
                }
            }
            if let Some(table) = &block.table {
                for t in table {
                    if let Some(&v) = function.at.get(t) {
                        merge[v] = true;
                    }
                }
            }
        }
        for v in 0..n {
            merge[v] |= forward_in[v] >= 2;
        }
        for c in &mut children {
            c.sort_by_key(|&b| std::cmp::Reverse(rank[b]));
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&b| rank[b]);
        let instructions = |b: usize| function.instructions(b).iter().copied();

        // Flag setting shapes, numbered after the two fixed states.
        const {
            assert!(
                G::SHAPES.len() + FIRST_SHAPE_STATE as usize <= FlagStates::BITS as usize,
                "too many shapes"
            )
        };
        let mut shapes: Vec<G::Shape> = Vec::new();
        for d in function_instructions(function).map(|o| &decoded[&o]) {
            if let Some(shape) = d.effects.flags.and_then(G::shape)
                && !shapes.contains(&shape)
            {
                shapes.push(shape);
            }
        }
        let flag_state = |d: &Decoded<G>, state: FlagStates| -> FlagStates {
            if d.effects.calls {
                1 << FLAGS_IN_BLOCK
            } else if let Some(shape) = d.effects.flags.and_then(G::shape) {
                1 << (FIRST_SHAPE_STATE as usize + shapes.iter().position(|s| *s == shape).unwrap())
            } else if !d.effects.defs.intersect(G::FLAG_BITS).is_empty() {
                1 << FLAGS_IN_LOCALS
            } else {
                state
            }
        };
        let dirty_after = |d: &Decoded<G>, dirty: Registers| -> Registers {
            if d.effects.calls || d.effects.syscall {
                Registers::NONE
            } else {
                dirty.union(d.effects.defs.intersect(tracked::<G>()))
            }
        };

        // Registers a loop's own code neither reads nor writes. Those that may be dirty when the loop is
        // entered are written back before it, so its slow paths do not keep them alive.
        let untouched: Vec<Registers> = (0..n)
            .map(|h| match bodies.get(&h) {
                Some(body) => (0..n).filter(|&b| body[b]).fold(tracked::<G>(), |r, b| {
                    instructions(b).fold(r, |r, o| {
                        let e = &decoded[&o].effects;
                        r.minus(e.defs).minus(e.uses)
                    })
                }),
                None => Registers::NONE,
            })
            .collect();
        let entering = |b: usize, s: usize| bodies.get(&s).is_some_and(|body| !body[b]);
        // Registers kept in the thread block rather than in locals through a loop that neither reads nor
        // writes them, so they take no engine register while it runs: loaded again where it is left. Not
        // done for a loop left through a jump table, whose edges have no place for the loads.
        let through: FxHashMap<usize, Registers> = bodies
            .iter()
            .map(|(&h, body)| {
                let switch = (0..n).any(|b| body[b] && blocks[b].table.is_some());
                (
                    h,
                    if switch {
                        Registers::NONE
                    } else {
                        untouched[h]
                    },
                )
            })
            .collect();
        let leaving = |b: usize, s: usize| {
            bodies
                .iter()
                .filter(|(_, body)| body[b] && !body[s])
                .fold(Registers::NONE, |r, (h, _)| r.union(through[h]))
        };
        let mut reload_on_exit = FxHashMap::default();
        for (b, block) in blocks.iter().enumerate() {
            for &s in &block.succs {
                let r = leaving(b, s);
                if r != Registers::NONE {
                    reload_on_exit.insert((b, s), r);
                }
            }
        }
        let mut written_before = vec![Registers::NONE; n];

        // Forward: what may be dirty, where the flags are, and the frame known to be allowed.
        let mut dirty_in = vec![Registers::NONE; n];
        let mut flags_in: Vec<FlagStates> = vec![0; n];
        let mut frame_in: Vec<Option<Frame>> = vec![None; n];
        let mut aligned_in = vec![true; n];
        aligned_in[0] = seed.aligned;
        flags_in[0] = 1 << FLAGS_IN_BLOCK;
        frame_in[0] = Some(seed.frame);
        let mut changed = true;
        while changed {
            changed = false;
            for &b in &order {
                let Some(mut frame) = frame_in[b] else {
                    continue;
                };
                let mut dirty = dirty_in[b];
                let mut flags = flags_in[b];
                let mut aligned = aligned_in[b];
                for o in instructions(b) {
                    let d = &decoded[&o];
                    aligned = aligned_after(d, aligned);
                    dirty = dirty_after(d, dirty);
                    flags = flag_state(d, flags);
                    frame = frame_after(d, frame);
                }
                for &s in &blocks[b].succs {
                    let mut dirty = dirty;
                    if entering(b, s) {
                        written_before[s] = written_before[s].union(dirty.intersect(untouched[s]));
                        dirty = dirty.minus(untouched[s]);
                    }
                    let joined_dirty = dirty_in[s].union(dirty);
                    let joined_flags = flags_in[s] | flags;
                    let joined_frame = match frame_in[s] {
                        None => frame,
                        Some(old) => match (old, frame) {
                            (Some((a, b)), Some((c, d))) if a.max(c) < b.min(d) => {
                                Some((a.max(c), b.min(d)))
                            }
                            _ => None,
                        },
                    };
                    let joined_aligned = aligned_in[s] && aligned;
                    if joined_dirty != dirty_in[s]
                        || joined_flags != flags_in[s]
                        || frame_in[s] != Some(joined_frame)
                        || joined_aligned != aligned_in[s]
                    {
                        aligned_in[s] = joined_aligned;
                        dirty_in[s] = joined_dirty;
                        flags_in[s] = joined_flags;
                        frame_in[s] = Some(joined_frame);
                        changed = true;
                    }
                }
            }
        }
        let frame_in: Vec<Frame> = frame_in.into_iter().map(Option::flatten).collect();

        // Backward: registers that must hold their true value. Every register that may be dirty at a block's
        // start counts as needed there, so no path reaches a write back with a stale local.
        let mut live_in_blocks = vec![Registers::NONE; n];
        let mut live_after = FxHashMap::default();
        let mut changed = true;
        while changed {
            changed = false;
            for &b in order.iter().rev() {
                let mut live = blocks[b].succs.iter().fold(Registers::NONE, |l, &s| {
                    let l = l.union(
                        live_in_blocks[s]
                            .minus(*reload_on_exit.get(&(b, s)).unwrap_or(&Registers::NONE)),
                    );
                    if entering(b, s) {
                        l.union(written_before[s])
                    } else {
                        l
                    }
                });
                for o in instructions(b).collect::<Vec<_>>().into_iter().rev() {
                    let d = &decoded[&o];
                    live_after.insert(o, live);
                    live = if d.effects.calls {
                        d.effects.uses
                    } else {
                        live.minus(d.effects.defs).union(d.effects.uses)
                    }
                    .intersect(tracked::<G>());
                }
                live = live.union(dirty_in[b]);
                if live != live_in_blocks[b] {
                    live_in_blocks[b] = live;
                    changed = true;
                }
            }
        }

        let Checks {
            plans: checks,
            frame_checks,
            frame_across,
        } = checks::<G>(function, decoded, &frame_in);
        // The innermost loop around each block, and loops in which nothing can change the permission table:
        // no call and no system call. (A stop check that carries on in place forgets every kept run.)
        let innermost = |b: usize| {
            headers
                .iter()
                .copied()
                .filter(|h| bodies[h][b])
                .min_by_key(|h| bodies[h].iter().filter(|&&x| x).count())
        };
        let quiet = |h: usize| {
            (0..n).filter(|&b| bodies[&h][b]).all(|b| {
                instructions(b).all(|o| {
                    let e = &decoded[&o].effects;
                    !e.calls && !e.syscall
                })
            })
        };
        let mut kept = FxHashMap::default();
        let mut kept_longest = Vec::new();
        let mut kept_by_loop = vec![Vec::new(); n];
        let mut keys = Vec::new();
        for b in 0..n {
            if let Some(h) = innermost(b)
                && quiet(h)
            {
                for o in instructions(b) {
                    if let (Some(CheckPlan::Exact), Some(access)) =
                        (checks.get(&(o, 0)), only_access(&decoded[&o]))
                    {
                        let key = (
                            h,
                            access.segment,
                            access.base,
                            access.kind != AccessKind::Read,
                        );
                        let number = match keys.iter().position(|k| *k == key) {
                            Some(number) => number,
                            None => {
                                keys.push(key);
                                kept_longest.push(0);
                                kept_by_loop[h].push(keys.len() - 1);
                                keys.len() - 1
                            }
                        };
                        let bytes = access.fixed_bytes().expect("a planned access");
                        kept_longest[number] = kept_longest[number].max(bytes);
                        kept.insert(o, number);
                    }
                }
            }
        }
        // Loops without calls whose accesses can be proven allowed before they run.
        // Only the loop's own blocks count, not those of loops inside it, which have proofs of their own.
        let dominates = |a: usize, b: usize| {
            doms.dominators(NodeIndex::new(b))
                .is_some_and(|mut d| d.any(|x| x.index() == a))
        };
        let mut versioned = FxHashMap::default();
        let mut after = vec![Vec::new(); n];
        let mut homed = vec![false; n];
        let mut outermost_first = headers.clone();
        outermost_first
            .sort_by_key(|h| std::cmp::Reverse(bodies[h].iter().filter(|&&x| x).count()));
        for &h in &outermost_first {
            let body = &bodies[&h];
            let inner: Vec<usize> = headers
                .iter()
                .copied()
                .filter(|&g| g != h && body[g])
                .collect();
            let nested: Vec<bool> = (0..n).map(|b| inner.iter().any(|g| bodies[g][b])).collect();
            // A stop at a switch's jump back carries on in place, where nothing would be proven again.
            let switch_back = (0..n).any(|b| {
                body[b]
                    && blocks[b]
                        .table
                        .iter()
                        .flatten()
                        .any(|c| function.at.get(c).is_some_and(|&t| rank[t] <= rank[b]))
            });
            if !quiet(h) || switch_back {
                continue;
            }
            let shape = proof::LoopShape {
                function,
                decoded,
                checks: &checks,
                body,
                nested: &nested,
                latches: preds[h]
                    .iter()
                    .copied()
                    .filter(|&u| rank[h] <= rank[u])
                    .collect(),
                dominates: &dominates,
            };
            let Some(mut v) = proof::versioned(&shape) else {
                continue;
            };
            v.inner = inner;
            versioned.insert(h, v);
            // Placed after the outermost such loop they leave.
            for y in 1..n {
                if body[y] || homed[y] {
                    continue;
                }
                let idom = doms.immediate_dominator(NodeIndex::new(y)).unwrap().index();
                if body[idom] {
                    homed[y] = true;
                    children[idom].retain(|&c| c != y);
                    merge[y] = true;
                    after[h].push(y);
                }
            }
            after[h].sort_by_key(|&b| std::cmp::Reverse(rank[b]));
        }
        let proven = versioned
            .iter()
            .flat_map(|(&h, v)| v.regions.iter().map(move |r| (r.at, h)))
            .collect();
        let dynamic_flags = flags_in.iter().any(|f| f.count_ones() > 1);
        let calls_out = (0..n).any(|b| {
            instructions(b).any(|o| {
                let d = &decoded[&o];
                d.effects.calls || leaves_piece(function, b, o, d)
            })
        });
        Plan {
            rank,
            children,
            merge,
            loop_header,
            live_in: live_in_blocks,
            live_after,
            dirty_in,
            written_before,
            flags_in,
            aligned_in,
            shapes,
            dynamic_flags,
            checks,
            frame_checks,
            frame_across,
            calls_out,
            frame_in,
            outside,
            kept,
            kept_longest,
            kept_by_loop,
            versioned,
            after,
            proven,
            reload_on_exit,
        }
    }
}

fn function_instructions(function: &Function) -> impl Iterator<Item = u64> + '_ {
    (0..function.blocks.len()).flat_map(|b| function.instructions(b).iter().copied())
}

/// Whether the instruction at `offset` in block `b` can pass control to another piece other than by a call
/// that returns.
fn leaves_piece<G: Guest>(function: &Function, b: usize, offset: u64, d: &Decoded<G>) -> bool {
    let flow = G::flow(&d.instruction);
    let outside = |t: u64| !function.at.contains_key(&t);
    let call = flow.kind == FlowKind::Call;
    let next = offset + u64::from(d.length);
    match flow.kind {
        FlowKind::JumpThrough(_) => true,
        _ => {
            (flow
                .target
                .is_some_and(|by| !call && outside(offset.wrapping_add_signed(by))))
                || (flow.falls_through && offset == function.last(b) && outside(next))
        }
    }
}

/// One access of one instruction: its offset and which of its accesses.
type AccessAt = (u64, u8);

/// Which accesses need a check, where the stack's frame is checked, and the calls a frame is kept across.
struct Checks {
    plans: FxHashMap<(u64, u8), CheckPlan>,
    frame_checks: FxHashMap<u64, (i64, u32)>,
    frame_across: FxHashSet<u64>,
}

fn checks<G: Guest>(function: &Function, decoded: &Decodes<G>, frame_in: &[Frame]) -> Checks {
    let mut plans = FxHashMap::default();
    let mut frame_checks = FxHashMap::default();
    let mut frame_across = FxHashSet::default();
    for (b, &frame) in frame_in.iter().enumerate() {
        let mut frame = frame;
        // Per base register: how far its value has moved from where its current groups are measured.
        let mut moved: FxHashMap<u32, i64> = FxHashMap::default();
        // Open groups by segment base, base register (none for a segment base alone) and write: first
        // access, its start, and the bytes covered.
        let mut open: FxHashMap<(Option<u32>, Option<u32>, bool), usize> = FxHashMap::default();
        let mut groups: Vec<(AccessAt, i64, i64, i64, u32)> = Vec::new();
        for &o in function.instructions(b) {
            let d = &decoded[&o];
            if d.effects.calls || d.effects.syscall {
                moved.clear();
                open.clear();
            }
            if d.effects.calls && frame.is_some() {
                frame_across.insert(o);
            }
            let movement = stack_move(d);
            if let StackMove::By { delta, .. } = movement
                && delta < 0
            {
                // The red zone below the new stack pointer is part of the frame checked.
                let low = delta - G::RED_ZONE as i64;
                frame_checks.insert(o, (low, (-low) as u32));
            }
            // An access that is not plain has no plan: it is checked by itself each time.
            let accesses = G::accesses(&d.instruction);
            let plain = accesses
                .iter()
                .enumerate()
                .filter_map(|(which, a)| Some((which as u8, a.filter(MemoryAccess::plain)?)));
            for (which, access) in plain {
                let bytes = access.fixed_bytes().expect("a plain access");
                let base = base_bit::<G>(&access).filter(|_| access.index.is_none());
                let segment = access.segment.and_then(G::reg_bit);
                let ahead = match access.write_back {
                    WriteBack::Before(by) => by,
                    _ => 0,
                };
                let behind = match access.write_back {
                    WriteBack::Before(by) | WriteBack::After(by) => by,
                    WriteBack::None => 0,
                };
                // What a group is measured from: a general register other than the stack pointer, or a
                // segment base alone.
                let grouped = match base {
                    Some(n) if G::GENERAL_BUT_SP.has_bit(n) => Some(Some(n)),
                    None if segment.is_some()
                        && access.base.is_none()
                        && access.index.is_none() =>
                    {
                        Some(None)
                    }
                    _ => None,
                };
                let plan = match (base, grouped) {
                    (Some(bit), _) if bit == G::SP_BIT && segment.is_none() => {
                        let at = access.displacement;
                        let known = match movement {
                            StackMove::By {
                                delta,
                                before_access: true,
                            } => shift::<G>(grow::<G>(frame, delta), delta),
                            _ => frame,
                        };
                        if covers(known, at, at + i64::from(bytes)) {
                            CheckPlan::Skip
                        } else {
                            CheckPlan::Exact
                        }
                    }
                    (_, Some(register)) => {
                        let m = register.and_then(|n| moved.get(&n).copied()).unwrap_or(0);
                        let (start, after) = (m + access.displacement + ahead, m + behind);
                        let end = start + i64::from(bytes);
                        if let Some(n) = register {
                            moved.insert(n, after);
                        }
                        let key = (segment, register, access.kind != AccessKind::Read);
                        match open.get(&key).map(|&g| (g, groups[g])) {
                            Some((g, (first, first_start, lo, hi, count)))
                                if hi.max(end) - lo.min(start)
                                    <= wcontainers_abi::PAGE_SIZE as i64 =>
                            {
                                groups[g] =
                                    (first, first_start, lo.min(start), hi.max(end), count + 1);
                                CheckPlan::Skip
                            }
                            _ => {
                                open.insert(key, groups.len());
                                groups.push(((o, which), start, start, end, 1));
                                CheckPlan::Exact
                            }
                        }
                    }
                    _ => CheckPlan::Exact,
                };
                plans.insert((o, which), plan);
            }
            // Any other write of a base, or of a segment base, starts it afresh.
            let written_back: Vec<u32> = accesses
                .iter()
                .flatten()
                .filter(|a| a.write_back != WriteBack::None)
                .filter_map(base_bit::<G>)
                .collect();
            for bit in d.effects.defs.intersect(G::GENERAL_BUT_SP).bits() {
                if !written_back.contains(&bit) {
                    moved.remove(&bit);
                    open.retain(|&(_, base, _), _| base != Some(bit));
                }
            }
            for bit in d.effects.defs.intersect(G::SEGMENT_BITS).bits() {
                open.retain(|&(segment, _, _), _| segment != Some(bit));
            }
            frame = frame_after(d, frame);
        }
        for (first, first_start, lo, hi, count) in groups {
            if count > 1 {
                plans.insert(
                    first,
                    CheckPlan::Group {
                        from: lo - first_start,
                        len: (hi - lo) as u32,
                    },
                );
            }
        }
    }
    Checks {
        plans,
        frame_checks,
        frame_across,
    }
}

/// A frame before the stack pointer moves by `delta`, with the part a move down checks added: down to the
/// red zone below the new stack pointer.
fn grow<G: Guest>(frame: Frame, delta: i64) -> Frame {
    if delta >= 0 {
        return frame;
    }
    let low = delta - G::RED_ZONE as i64;
    match frame {
        Some((lo, hi)) if lo <= 0 && hi >= 0 => Some((low.min(lo), hi)),
        _ => Some((low, 0)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Label {
    Loop(usize),
    Merge(usize),
    /// Where the proof of the loop at a header is made again, before both its copies.
    Reprove(usize),
    Other,
}

/// Where emission stands: the instruction, its block, and what is known there.
#[derive(Clone, Copy)]
struct At {
    offset: u64,
    block: usize,
    dirty: Registers,
    flags: FlagStates,
    /// The stack pointer is known to be a multiple of 16.
    aligned: bool,
    /// The float mode local holds whether floating point takes the slow path.
    float_mode: bool,
}

/// How a piece carries on when the stop hook says so.
#[derive(Clone, Copy)]
enum AfterStop {
    /// Go on in a new call of this piece or of a loop piece (by function index) at the loop jumped back to,
    /// so an engine that has optimised the function since this call began runs the optimised code.
    Enter(u32),
    /// Write back and go on at once in a new call of a piece whose entry check calls the stop hook, so the
    /// loop has no call that returns to it.
    Hand(u32),
    /// Go on with every local as it is: the hook changes nothing when it says carry on. Used at the entry,
    /// before any register but the passed ones is in a local.
    Continue,
    /// Load back the tracked registers in the set, and the passed ones, and go on.
    Reload(Registers),
    /// Load back the tracked registers in the set and make the proof of the loop at this header again: the
    /// stop may have changed what it proved.
    Reprove(Registers, usize),
}

/// What a stop check or write back sets `pc` to.
#[derive(Clone, Copy)]
enum Pc {
    Offset(u64),
    /// The guest address in this i64 local.
    Local(u32),
}

struct FullEmitter<'a, G: Guest> {
    function: &'a Function,
    whole: &'a Whole,
    plan: &'a Plan<G>,
    decoded: &'a Decodes<G>,
    index: &'a BTreeMap<u64, u32>,
    /// The loop pieces of the function this piece comes from, by header offset.
    loops: &'a FxHashMap<u64, u32>,
    omit: Omit,
    optimise: Optimise,
    code: &'a Code<'a, G>,
    /// Whether floating point takes the slow path, loaded once until something may change it.
    float_mode_local: Option<u32>,
    sites: Sites,
    body: Vec<u8>,
    locals: Vec<ValType>,
    free: Vec<(ValType, u32)>,
    taken: Vec<(ValType, u32)>,
    registers: FxHashMap<u32, u32>,
    /// One i32 local for each tracked flag, in `Guest::FLAGS` order.
    flag_locals: [u32; MAX_TRACKED_FLAGS],
    /// Which flag state holds, when that is only known at run time.
    flag_state: u32,
    /// The locals each lazily kept flag shape shares.
    canonical: Vec<G::Flags>,
    labels: Vec<Label>,
    at: At,
    /// Locals the immutable imported globals (by index) are copied into at the entry, once used.
    fixed: [Option<u32>; IMPORTED_VALUES],
    /// The piece's own function index.
    own: u32,
    /// For each number in [`Plan::kept`], the i64 locals holding the run of pages last found allowed: its
    /// first address and how far past it an access may start (0 when nothing is kept).
    kept_pages: Vec<(u32, u32)>,
    /// The loops whose copy without proven checks is being emitted.
    unchecked: Vec<usize>,
    /// The piece checks the stop flag at its entry: when it calls or leaves to other pieces, and always for
    /// a loop piece, which a stop check at a jump back enters.
    entry_check: bool,
    /// The instruction being emitted has written a register, the flags or a control register.
    written: bool,
    /// The instruction being emitted may have raised the stop flag.
    raised_stop: bool,
    /// i64 local: the depth the piece passes to what it calls, its callers' and its own.
    depth: u32,
    /// i64 local: the progress count, when it is kept.
    count: Option<u32>,
    /// The instruction being emitted is an event of the progress count not yet counted.
    event: bool,
}

impl<'a, G: Guest> FullEmitter<'a, G> {
    fn piece(
        function: &'a Function,
        whole: &'a Whole,
        plan: &'a Plan<G>,
        decoded: &'a Decodes<G>,
        (index, loops, own): (&'a BTreeMap<u64, u32>, &'a FxHashMap<u64, u32>, u32),
        (omit, optimise, code): (Omit, Optimise, &'a Code<'a, G>),
    ) -> (wasm_encoder::Function, usize, Sites) {
        let loop_piece = index.get(&function.entry) != Some(&own);
        let mut e = FullEmitter {
            function,
            whole,
            plan,
            decoded,
            index,
            loops,
            omit,
            optimise,
            code,
            float_mode_local: None,
            sites: [0; SITES],
            body: Vec::new(),
            locals: Vec::new(),
            free: Vec::new(),
            taken: Vec::new(),
            registers: FxHashMap::default(),
            flag_locals: [0; MAX_TRACKED_FLAGS],
            flag_state: 0,
            canonical: Vec::new(),
            labels: Vec::new(),
            at: At {
                offset: function.entry,
                block: 0,
                dirty: Registers::NONE,
                flags: 1 << FLAGS_IN_BLOCK,
                aligned: false,
                float_mode: false,
            },
            fixed: [None; IMPORTED_VALUES],
            own,
            kept_pages: Vec::new(),
            unchecked: Vec::new(),
            entry_check: (plan.calls_out || loop_piece) && !omit.stop,
            written: false,
            raised_stop: false,
            depth: 0,
            count: None,
            event: false,
        };
        e.depth = e.new_local(ValType::I64);
        if kept(code.progress) {
            e.count = Some(G::L_COUNT);
        }
        for _ in plan.kept_by_loop.iter().flatten() {
            let low = e.new_local(ValType::I64);
            let room = e.new_local(ValType::I64);
            e.kept_pages.push((low, room));
        }
        const {
            assert!(
                G::FLAGS.len() <= MAX_TRACKED_FLAGS,
                "too many tracked flags"
            )
        };
        for i in 0..G::FLAGS.len() {
            e.flag_locals[i] = e.new_local(ValType::I32);
        }
        if !G::FLAGS.is_empty() {
            e.flag_state = e.new_local(ValType::I32);
        }
        for shape in plan.shapes.clone() {
            let flags = e.canonical_flags(shape);
            e.canonical.push(flags);
        }
        if plan.dynamic_flags {
            let state = e.flag_state;
            e.sink().i32_const(FLAGS_IN_BLOCK as i32).local_set(state);
        }
        if e.entry_check {
            // One branch for both entry checks: the stop flag, and the stack budget.
            let depth = (!omit.depth).then_some(function.entry);
            e.stop_check_or_deep(Pc::Offset(function.entry), AfterStop::Continue, depth);
        }
        for bit in plan.live_in[0].bits() {
            let local = e.register(bit);
            let block = e.fixed(G_THREAD_BLOCK);
            Slot::of_bit::<G>(bit).load_into(&mut e.sink(), block, local);
        }
        e.tree(0);
        e.sink().unreachable().end();
        let mut entry = Vec::new();
        let mut s = InstructionSink::new(&mut entry);
        for (global, local) in e.fixed.iter().enumerate() {
            if let Some(local) = local {
                s.global_get(G_OWN + global as u32).local_set(*local);
            }
        }
        s.local_get(G::L_DEPTH)
            .i64_const(frame_bytes::<G>(&e.locals))
            .i64_add()
            .local_set(e.depth);
        // Only a piece that calls or leaves to others can be part of deep recursion.
        if (plan.calls_out || loop_piece) && !omit.depth && !e.entry_check {
            depth_check::<G>(&mut s, G::L_DEPTH, function.entry, code.progress);
            e.sites[Reason::StackNearlyFull as usize] += 1;
        }
        let locals = e.locals.len();
        let mut f = wasm_encoder::Function::new_with_locals_types(e.locals);
        f.raw(entry);
        f.raw(e.body);
        (f, locals, e.sites)
    }

    fn new_local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        parameters::<G>(self.code.progress) + self.locals.len() as u32 - 1
    }

    /// The locals a shape keeps, fresh ones: what else it needs can be worked out from them.
    fn canonical_flags(&mut self, shape: G::Shape) -> G::Flags {
        let locals: Vec<u32> = G::shape_locals(shape)
            .iter()
            .map(|&ty| self.new_local(ty))
            .collect();
        G::with_locals(shape, &locals)
    }

    /// The flags of a kept shape with every local filled in: what was not kept is worked out into
    /// temporaries (an engine drops what is not used).
    fn shape_flags(&mut self, id: usize) -> G::Flags {
        let flags = self.canonical[id];
        let Some(ty) = G::temporaries(flags) else {
            return flags;
        };
        let temporaries = [self.temporary(ty), self.temporary(ty)];
        G::complete(&mut self.sink(), flags, temporaries)
    }

    /// The local of a register bit: a parameter for the passed ones.
    fn register(&mut self, bit: u32) -> u32 {
        if let Some(local) = G::passed(bit) {
            return local;
        }
        if let Some(&local) = self.registers.get(&bit) {
            return local;
        }
        let local = self.new_local(Slot::of_bit::<G>(bit).ty());
        self.registers.insert(bit, local);
        local
    }

    fn decoded(&self) -> Decoded<G> {
        self.decoded[&self.at.offset]
    }

    // Structure, after Ramsey's "Beyond Relooper".

    /// Emit the subtree of the dominator tree at block `b`. The subtree that follows a node's own code last is
    /// emitted in turn by this loop, not by recursion, so the translator's stack grows only with how deeply
    /// the code nests.
    fn tree(&mut self, b: usize) {
        let mut next = Some(b);
        while let Some(b) = next {
            next = self.node(b);
        }
    }

    /// Emit block `b` and its subtree, except the subtree at the block returned, which follows it last.
    fn node(&mut self, b: usize) -> Option<usize> {
        let merges: Vec<usize> = self.plan.children[b]
            .iter()
            .copied()
            .filter(|&c| self.plan.merge[c])
            .collect();
        if self.plan.loop_header[b] {
            let block = self.fixed(G_THREAD_BLOCK);
            for bit in self.plan.written_before[b].bits() {
                let local = self.register(bit);
                Slot::of_bit::<G>(bit).store_from(&mut self.sink(), block, local);
            }
            // Both locals are set, so nothing kept from before is alive through the loop.
            for number in self.plan.kept_by_loop[b].clone() {
                let (low, room) = self.kept_pages[number];
                self.sink()
                    .i64_const(0)
                    .local_set(low)
                    .i64_const(0)
                    .local_set(room);
            }
            let after = self.plan.after[b].clone();
            for &y in &after {
                self.sink().block(BlockType::Empty);
                self.labels.push(Label::Merge(y));
            }
            match self.plan.versioned.get(&b).filter(|_| self.copies(b)) {
                Some(v) => {
                    // With no loop piece to go on in after a stop, the copy without checks comes back here.
                    let reprove = !self.loops.contains_key(&self.function.blocks[b].start);
                    if reprove {
                        self.sink().loop_(BlockType::Empty);
                        self.labels.push(Label::Reprove(b));
                    }
                    self.proof(v);
                    self.sink().if_(BlockType::Empty);
                    self.labels.push(Label::Other);
                    self.unchecked.push(b);
                    self.loop_of(b, &merges);
                    self.unchecked.pop();
                    self.sink().else_();
                    // The loop with its checks is in its loop piece, so a function's own code holds only
                    // the copy without them.
                    match self.loops.get(&self.function.blocks[b].start) {
                        Some(&f) if b != 0 => {
                            self.at = At {
                                offset: self.function.blocks[b].start,
                                block: b,
                                dirty: self.plan.dirty_in[b],
                                flags: self.plan.flags_in[b],
                                aligned: self.plan.aligned_in[b],
                                float_mode: false,
                            };
                            self.write_back();
                            self.rare_arguments();
                            self.sink().return_call(f);
                        }
                        _ => {
                            // Counted at each entry whose proof fails, warned once for the loop.
                            self.sites[Reason::LoopNotProven as usize] += 1;
                            self.sink().i32_const(Reason::LoopNotProven as i32);
                            self.address(self.function.blocks[b].start);
                            self.sink().call(F_WARN);
                            self.loop_of(b, &merges);
                        }
                    }
                    self.sink().end();
                    self.labels.pop();
                    if reprove {
                        self.sink().end();
                        self.labels.pop();
                    }
                }
                None => self.loop_of(b, &merges),
            }
            let mut last = None;
            for &y in after.iter().rev() {
                if let Some(t) = last.replace(y) {
                    self.tree(t);
                }
                self.sink().end();
                self.labels.pop();
            }
            last
        } else {
            self.within(b, &merges)
        }
    }

    fn loop_of(&mut self, b: usize, merges: &[usize]) {
        self.sink().loop_(BlockType::Empty);
        self.labels.push(Label::Loop(b));
        if let Some(t) = self.within(b, merges) {
            self.tree(t);
        }
        self.sink().end();
        self.labels.pop();
    }

    /// Whether the loop at block `b` gets its copy without proven checks: after a stop at a jump back in it
    /// or in any loop inside it, the thread must carry on where the proof is made again, at a loop's entry.
    /// Translated for speed that is a loop piece; for size, with no loop pieces, only a loop with no loop
    /// inside it has the copy, and a stop at its own jump back makes its proof again.
    fn copies(&self, b: usize) -> bool {
        let pieced = |h: usize| h == 0 || self.loops.contains_key(&self.function.blocks[h].start);
        let inner = &self.plan.versioned[&b].inner;
        let stops_handled = match self.optimise {
            Optimise::Speed => pieced(b) && inner.iter().all(|&g| pieced(g)),
            Optimise::Size => inner.is_empty(),
        };
        !self.omit.permissions && (self.omit.stop || stops_handled)
    }

    /// Emit block `b` inside a block for each merge, each merge's subtree after its block; the subtree that
    /// comes last is returned instead, for the caller to emit.
    fn within(&mut self, b: usize, merges: &[usize]) -> Option<usize> {
        match merges.split_first() {
            Some((&m, rest)) => {
                self.sink().block(BlockType::Empty);
                self.labels.push(Label::Merge(m));
                if let Some(t) = self.within(b, rest) {
                    self.tree(t);
                }
                self.sink().end();
                self.labels.pop();
                Some(m)
            }
            None => {
                self.block_code(b);
                None
            }
        }
    }

    fn block_code(&mut self, b: usize) {
        let block = &self.function.blocks[b];
        self.at = At {
            offset: block.start,
            block: b,
            dirty: self.plan.dirty_in[b],
            flags: self.plan.flags_in[b],
            aligned: self.plan.aligned_in[b],
            float_mode: false,
        };
        for &offset in self.function.instructions(b) {
            self.at.offset = offset;
            self.written = false;
            let mark = self.taken.len();
            if let Some(&(from, len)) = self.plan.frame_checks.get(&offset)
                && !self.omit.permissions
            {
                self.frame_check(from, len);
            }
            let decoded = self.decoded();
            self.event = self.count.is_some() && progress_event::<G>(&decoded.instruction);
            G::emit(self, &decoded.instruction);
            self.at.aligned = aligned_after(&decoded, self.at.aligned);
            let released: Vec<_> = self.taken.drain(mark..).collect();
            self.free.extend(released);
        }
    }

    fn depth(&self, label: Label) -> u32 {
        self.labels.iter().rev().position(|l| *l == label).unwrap() as u32
    }

    /// The label a jump from the current block to block `to` takes, or none if `to` is placed right here.
    fn label_for(&self, to: usize) -> Option<Label> {
        let from = self.at.block;
        if self.plan.rank[to] <= self.plan.rank[from] {
            Some(Label::Loop(to))
        } else if self.plan.merge[to] {
            Some(Label::Merge(to))
        } else {
            None
        }
    }

    fn branch(&mut self, to: usize) {
        if let Some(&kept) = self.plan.reload_on_exit.get(&(self.at.block, to)) {
            let block = self.fixed(G_THREAD_BLOCK);
            for bit in self.plan.live_in[to].intersect(kept).bits() {
                let local = self.register(bit);
                Slot::of_bit::<G>(bit).load_into(&mut self.sink(), block, local);
            }
        }
        match self.label_for(to) {
            Some(label) => {
                if let Label::Loop(_) = label
                    && !self.omit.stop
                {
                    let start = self.function.blocks[to].start;
                    let after = match self.loops.get(&start) {
                        _ if to == 0 && self.entry_check => AfterStop::Hand(self.own),
                        _ if to == 0 => AfterStop::Enter(self.own),
                        Some(&f) => AfterStop::Hand(f),
                        None if self.unchecked.contains(&to) => {
                            AfterStop::Reprove(self.plan.live_in[to], to)
                        }
                        None => AfterStop::Reload(self.plan.live_in[to]),
                    };
                    self.stop_check(Pc::Offset(start), after);
                }
                let depth = self.depth(label);
                self.sink().br(depth);
            }
            None => {
                let saved = self.at;
                self.tree(to);
                self.at = saved;
            }
        }
    }

    /// Go to an offset: inside the piece, or to another piece.
    fn goto(&mut self, target: u64) {
        if let Some(&b) = self.function.at.get(&target) {
            self.branch(b);
            return;
        }
        self.write_back();
        self.arguments();
        match self.index.get(&target).or(self.loops.get(&target)) {
            Some(&f) => {
                self.sink().return_call(f);
            }
            None => {
                self.address(target);
                self.sink().return_call(G::F_DISPATCH);
            }
        }
    }

    /// Push the parameters of a tail call: the passed registers, the expected return address and the depth,
    /// all as this piece was given them, and the progress count.
    fn arguments(&mut self) {
        let count = self.count;
        let mut s = self.sink();
        for i in 0..G::PASSED {
            s.local_get(i);
        }
        s.local_get(G::L_RETURN).local_get(G::L_DEPTH);
        if let Some(count) = count {
            s.local_get(count);
        }
    }

    /// [`Self::arguments`] for a tail call off the hot path (to a stop, to a loop's checked copy, to exact
    /// code): the count goes through the thread block, so its local ends before the call and does not
    /// crowd the registers of the loop it leaves from.
    fn rare_arguments(&mut self) {
        let block = self.fixed(G_THREAD_BLOCK);
        let count = self.count;
        let mut s = self.sink();
        if let Some(count) = count {
            store_count::<G>(&mut s, block, count);
        }
        for i in 0..G::PASSED {
            s.local_get(i);
        }
        s.local_get(G::L_RETURN).local_get(G::L_DEPTH);
        if count.is_some() {
            push_count::<G>(&mut s, block);
        }
    }

    /// Copy every register whose place in the thread block may be stale there, and the flags too.
    fn write_back(&mut self) {
        self.write_back_registers();
        self.write_back_flags(false);
    }

    /// Copy every tracked register that may be stale in the thread block there.
    fn write_back_registers(&mut self) {
        let block = self.fixed(G_THREAD_BLOCK);
        for bit in self.at.dirty.bits() {
            let local = self.register(bit);
            Slot::of_bit::<G>(bit).store_from(&mut self.sink(), block, local);
        }
    }

    /// Count the instruction's event, at its first way out, before anything is passed on.
    fn count_event(&mut self) {
        if std::mem::take(&mut self.event)
            && let Some(count) = self.count
        {
            add_event(&mut self.sink(), count);
        }
    }

    /// Put the flags in the thread block, unless they are there. The word is worked out once for each state
    /// the flags may be in; on the way to the runner (`out_of_line`) by the module's functions for it.
    fn write_back_flags(&mut self, out_of_line: bool) {
        let states = self.at.flags;
        if states == 1 << FLAGS_IN_BLOCK {
            return;
        }
        let block = self.fixed(G_THREAD_BLOCK);
        block.push(&mut self.sink());
        let ids = state_ids(states);
        for (k, &id) in ids.iter().enumerate() {
            if k + 1 < ids.len() {
                let state = self.flag_state;
                self.sink()
                    .local_get(state)
                    .i32_const(id as i32)
                    .i32_eq()
                    .if_(BlockType::Result(ValType::I32));
            }
            self.push_flags_word(id, out_of_line);
            if k + 1 < ids.len() {
                self.sink().else_();
            }
        }
        for _ in 1..ids.len() {
            self.sink().end();
        }
        Slot::flags::<G>().store(&mut self.sink());
    }

    /// Push the flags word of the thread block, with the flags in one state, `out_of_line` by the module's
    /// functions for it.
    fn push_flags_word(&mut self, state: u32, out_of_line: bool) {
        match state {
            FLAGS_IN_BLOCK => {
                let block = self.fixed(G_THREAD_BLOCK);
                let mut s = self.sink();
                block.push(&mut s);
                Slot::flags::<G>().load(&mut s);
            }
            FLAGS_IN_LOCALS if out_of_line => {
                let locals = self.flag_locals;
                let mut s = self.sink();
                for &local in &locals[..G::FLAGS.len()] {
                    s.local_get(local);
                }
                s.call(G::F_FLAGS_WORD);
            }
            FLAGS_IN_LOCALS => {
                let locals = self.flag_locals;
                push_word::<G>(&mut self.sink(), |s, flag| {
                    s.local_get(locals[G::flag_index(flag)]);
                });
            }
            id if out_of_line => {
                let id = (id - FIRST_SHAPE_STATE) as usize;
                let function = flags_word_function::<G>(self.plan.shapes[id]);
                let locals = G::locals_of(self.canonical[id]);
                let mut s = self.sink();
                for &local in locals.as_slice() {
                    s.local_get(local);
                }
                s.call(function);
            }
            id => {
                let flags = self.shape_flags((id - FIRST_SHAPE_STATE) as usize);
                push_word::<G>(&mut self.sink(), |s, flag| G::flag_value(s, flags, flag));
            }
        }
    }

    /// Whether the flags may be read, or written back, after the instruction at `offset` before something
    /// sets them again. Only the rest of its block is looked at, and anything but plain computation that
    /// neither reads a flag still to be set again nor moves the stack pointer counts as reading them.
    fn flags_needed_after(&self, offset: u64) -> bool {
        let instructions = self.function.instructions(self.at.block);
        // A block's instructions are in address order.
        let after = instructions.partition_point(|&o| o <= offset);
        let mut pending = G::FLAG_BITS;
        for o in &instructions[after..] {
            let d = &self.decoded[o];
            if !G::plain(&d.instruction)
                || !d.effects.uses.intersect(pending).is_empty()
                || d.effects.defs.has_bit(G::SP_BIT)
            {
                return true;
            }
            pending = pending.minus(d.effects.defs);
            if pending.is_empty() {
                return false;
            }
        }
        true
    }

    /// Write back everything, with `pc` set, on the way to the runner. Translated for size, on a path the
    /// thread never comes back on (`leaving`), through the module's functions; otherwise in place: calls even
    /// on such a path make the engine's code for a loop around it slower.
    fn save(&mut self, pc: Pc, leaving: bool) {
        let out_of_line = leaving && self.optimise == Optimise::Size;
        self.write_back_registers();
        self.write_back_flags(out_of_line);
        let load = self.fixed(G_LOAD_ADDRESS);
        let progress = self.code.progress;
        if !out_of_line {
            let block = self.fixed(G_THREAD_BLOCK);
            let mut s = self.sink();
            save_passed::<G>(&mut s, block, progress);
            set_pc::<G>(&mut s, block, |s| match pc {
                Pc::Offset(offset) => {
                    load.push(s);
                    s.i64_const(offset as i64).i64_add();
                }
                Pc::Local(local) => {
                    s.local_get(local);
                }
            });
            return;
        }
        save::<G>(&mut self.sink(), progress, |s| match pc {
            Pc::Offset(offset) => {
                s.i64_const(offset as i64);
            }
            Pc::Local(local) => {
                s.local_get(local);
                load.push(s);
                s.i64_sub();
            }
        });
    }

    /// Call the stop hook if the flag is raised, then carry on as `after` says: when the hook says carry on,
    /// nothing has changed.
    fn stop_check(&mut self, pc: Pc, after: AfterStop) {
        self.stop_check_or_deep(pc, after, None);
    }

    /// [`Self::stop_check`], at a piece's entry also leaving to the bottom loop when the depth is past the
    /// budget, with `pc` at the entry given.
    fn stop_check_or_deep(&mut self, pc: Pc, after: AfterStop, deep: Option<u64>) {
        self.sites[Reason::StoppedByFlag as usize] += 1;
        if deep.is_some() {
            self.sites[Reason::StackNearlyFull as usize] += 1;
        }
        self.at.float_mode = false;
        let block = self.fixed(G_THREAD_BLOCK);
        let (count, progress) = (self.count, self.code.progress);
        let mut s = self.sink();
        push_stop_wanted::<G>(&mut s, block, count);
        if let Some(entry) = deep {
            // The callers' depth, not this piece's own frame, so a piece called from the bottom loop always
            // runs, however large its frame.
            s.local_get(G::L_DEPTH)
                .i64_const(STACK_BUDGET)
                .i64_gt_u()
                .i32_or()
                .if_(BlockType::Empty);
            depth_check::<G>(&mut s, G::L_DEPTH, entry, progress);
        } else {
            s.if_(BlockType::Empty);
        }
        if let AfterStop::Hand(f) = after {
            self.write_back();
            self.rare_arguments();
            self.sink().return_call(f).end();
            return;
        }
        self.save(pc, false);
        self.sink()
            .call(F_STOP)
            .if_(BlockType::Empty)
            .i32_const(UNWIND_LEAVE)
            .call(G::F_UNWIND)
            .unreachable()
            .end();
        match after {
            AfterStop::Enter(f) => {
                self.rare_arguments();
                self.sink().return_call(f);
            }
            AfterStop::Reload(live) => {
                self.reload(live, None);
                // The table may have changed during the stop.
                for (_, room) in self.kept_pages.clone() {
                    self.sink().i64_const(0).local_set(room);
                }
            }
            AfterStop::Reprove(live, header) => {
                self.reload(live, None);
                for (_, room) in self.kept_pages.clone() {
                    self.sink().i64_const(0).local_set(room);
                }
                // Out of the stop check's `if` too.
                let depth = self.depth(Label::Reprove(header)) + 1;
                self.sink().br(depth);
            }
            AfterStop::Continue | AfterStop::Hand(_) => {}
        }
        self.sink().end();
    }

    /// Load registers from the thread block: the tracked ones in `live`, the passed ones but the parameter
    /// `skip`, and the progress count, so none of them is live across the call that left.
    fn reload(&mut self, live: Registers, skip: Option<u32>) {
        let block = self.fixed(G_THREAD_BLOCK);
        for bit in live.intersect(tracked::<G>()).bits() {
            let local = self.register(bit);
            Slot::of_bit::<G>(bit).load_into(&mut self.sink(), block, local);
        }
        let count = self.count;
        let mut s = self.sink();
        for (i, slot) in passed_slots::<G>().enumerate() {
            if skip != Some(i as u32) {
                slot.load_into(&mut s, block, i as u32);
            }
        }
        if let Some(count) = count {
            load_count::<G>(&mut s, block, count);
        }
    }

    /// Carry on at an instruction from code translated from there, which checks every access exactly.
    fn leave_to(&mut self, offset: u64) {
        self.sites[Reason::LeftToAddress as usize] += 1;
        self.save(Pc::Offset(offset), true);
        self.sink()
            .i32_const(UNWIND_GO_TO_EXACT)
            .call(G::F_UNWIND)
            .unreachable();
    }

    /// The pages of `len` bytes from `from` past the stack pointer allow reading and writing, or this
    /// instruction runs from code that checks each access.
    fn frame_check(&mut self, from: i64, len: u32) {
        let at = self.temporary(ValType::I64);
        let (page, fault_at) = (self.temporary(ValType::I64), self.temporary(ValType::I64));
        let table = self.table();
        let exact = self.exact(table, page);
        for chunk in (0..len).step_by(wcontainers_abi::PAGE_SIZE as usize) {
            let piece = (len - chunk).min(wcontainers_abi::PAGE_SIZE as _);
            self.sink()
                .local_get(G::L_SP)
                .i64_const(from + i64::from(chunk))
                .i64_add()
                .local_set(at);
            let mut s = self.sink();
            permission_fast(&mut s, table, at, PAGE_READ | PAGE_WRITE, page);
            permission_exact::<G>(&mut s, at, piece, PAGE_READ | PAGE_WRITE, fault_at, exact);
            let offset = self.at.offset;
            self.leave_to(offset);
            self.sink().end().end();
        }
    }

    /// The local an immutable imported global is read from.
    fn fixed(&mut self, global: u32) -> Fixed {
        let local = match self.fixed[global as usize] {
            Some(local) => local,
            None => {
                let local = self.new_local(imported_value_type(global));
                self.fixed[global as usize] = Some(local);
                local
            }
        };
        Fixed::Local(local)
    }

    /// Where an exact permission test goes: in place for speed, in the module's function for size.
    fn exact(&mut self, table: Table, page: u32) -> Exact {
        match self.optimise {
            Optimise::Speed => Exact::InPlace(table, (page, self.temporary(ValType::I32))),
            Optimise::Size => Exact::Called,
        }
    }

    fn table(&mut self) -> Table {
        Table {
            at: self.fixed(G_PERMISSIONS),
            pages: self.fixed(G_PERMISSION_PAGES),
        }
    }

    /// Push the guest address of a blob offset.
    fn address(&mut self, offset: u64) {
        let load = self.fixed(G_LOAD_ADDRESS);
        let mut s = self.sink();
        load.push(&mut s);
        s.i64_const(offset as i64).i64_add();
    }

    fn temporary(&mut self, ty: ValType) -> u32 {
        let local = match self.free.iter().position(|(t, _)| *t == ty) {
            Some(i) => self.free.swap_remove(i).1,
            None => self.new_local(ty),
        };
        self.taken.push((ty, local));
        local
    }

    /// Push one flag from wherever the flags are.
    fn push_flag(&mut self, flag: G::Flag) {
        let states = self.at.flags;
        if states.count_ones() == 1 {
            self.push_flag_from(states.trailing_zeros(), flag);
            return;
        }
        // Known only at run time: choose by the state local.
        let ids = state_ids(states);
        for (k, &id) in ids.iter().enumerate() {
            if k + 1 < ids.len() {
                let state = self.flag_state;
                self.sink()
                    .local_get(state)
                    .i32_const(id as i32)
                    .i32_eq()
                    .if_(BlockType::Result(ValType::I32));
            }
            self.push_flag_from(id, flag);
            if k + 1 < ids.len() {
                self.sink().else_();
            }
        }
        for _ in 1..ids.len() {
            self.sink().end();
        }
    }

    fn push_flag_from(&mut self, state: u32, flag: G::Flag) {
        match state {
            FLAGS_IN_LOCALS => {
                let local = self.flag_locals[G::flag_index(flag)];
                self.sink().local_get(local);
            }
            FLAGS_IN_BLOCK => {
                let block = self.fixed(G_THREAD_BLOCK);
                load_flag::<G>(&mut self.sink(), block, flag);
            }
            shape => {
                let flags = self.shape_flags((shape - FIRST_SHAPE_STATE) as usize);
                G::flag_value(&mut self.sink(), flags, flag);
            }
        }
    }

    fn set_flag_state(&mut self, state: u32) {
        self.at.flags = 1 << state;
        if self.plan.dynamic_flags {
            let local = self.flag_state;
            self.sink().i32_const(state as i32).local_set(local);
        }
    }

    fn table_jump(&mut self, target: u32, cases: &[u64]) {
        let present: Vec<(u64, usize)> = cases
            .iter()
            .filter_map(|c| self.function.at.get(c).map(|&b| (*c, b)))
            .collect();
        // Cases that start pieces of their own (a function cut in parts): a jump to one is a tail call.
        let elsewhere: Vec<u64> = cases
            .iter()
            .copied()
            .filter(|c| !self.function.at.contains_key(c) && self.index.contains_key(c))
            .collect();
        let known = || present.iter().map(|c| c.0).chain(elsewhere.iter().copied());
        let (Some(lo), Some(hi)) = (known().min(), known().max()) else {
            self.unrecognised_jump(target);
            return;
        };
        let slots = (hi - lo) / G::ALIGN + 1;
        // The engines take at most this many targets in one `br_table`.
        if slots > BR_TABLE_TARGETS {
            self.unrecognised_jump(target);
            return;
        }
        if !self.omit.stop
            && present
                .iter()
                .any(|&(_, b)| self.plan.rank[b] <= self.plan.rank[self.at.block])
        {
            let live = present
                .iter()
                .fold(Registers::NONE, |l, &(_, b)| l.union(self.plan.live_in[b]));
            self.stop_check(Pc::Local(target), AfterStop::Reload(live));
        }
        let index = self.temporary(ValType::I64);
        let at = self.at;
        if !elsewhere.is_empty() {
            self.sink().block(BlockType::Empty);
            self.labels.push(Label::Other);
        }
        self.sink().block(BlockType::Empty);
        self.labels.push(Label::Other);
        let mut labels = Vec::new();
        for k in 0..slots {
            let case = lo + G::ALIGN * k;
            let label = match present.iter().find(|c| c.0 == case) {
                Some(&(_, b)) => self.depth(self.label_for(b).unwrap()),
                // The block around this one.
                None if elsewhere.contains(&case) => 1,
                None => 0,
            };
            labels.push(label);
        }
        // A target that is not a multiple of the alignment past the lowest case turns into a huge index. The
        // index is worked out from the register as it is, not as `Guest::branch_target` makes it: such an
        // address is far from every case, so it goes to the lookup, and the engine can see through the
        // address the code built plus an entry.
        let FlowKind::JumpThrough(Through::Register(rn)) =
            G::flow(&self.decoded().instruction).kind
        else {
            unreachable!("a jump through a table is a jump through a register")
        };
        let load = self.fixed(G_LOAD_ADDRESS);
        self.get(rn);
        let mut s = self.sink();
        load.push(&mut s);
        s.i64_sub().i64_const(lo as i64).i64_sub();
        if G::ALIGN > 1 {
            s.i64_const(i64::from(G::ALIGN_SHIFT)).i64_rotr();
        }
        s.local_tee(index)
            .i32_wrap_i64()
            .i32_const(slots as i32)
            .local_get(index)
            .i64_const(slots as i64)
            .i64_lt_u()
            .select()
            .br_table(labels, 0)
            .end();
        self.labels.pop();
        // Made the target again on each way out, so that the engine works it out only when no case is taken.
        let register = self.register(G::reg_bit(rn).expect("a general register"));
        G::branch_target(self, register);
        self.sink().local_set(target);
        self.unrecognised_jump(target);
        if !elsewhere.is_empty() {
            self.sink().end();
            self.labels.pop();
            self.at = at;
            G::branch_target(self, register);
            self.sink().local_set(target);
            self.write_back();
            self.leave_through_lookup(target);
        }
    }

    /// Leave the piece through the lookup. A jump to outside its function, or to its function's entry, is a
    /// tail call, which costs what a call through a register costs; one to inside it (up to the next
    /// function's entry), in this part or in another of a function cut in parts, is a table or computed goto
    /// left unrecognised, the slow path, counted.
    fn unrecognised_jump(&mut self, target: u32) {
        self.sites[Reason::JumpThroughRegisterNotRecognised as usize] += 1;
        self.write_back();
        let (entry, span) = (self.whole.entry, self.whole.span.clone());
        let load = self.fixed(G_LOAD_ADDRESS);
        let mut s = self.sink();
        s.local_get(target);
        load.push(&mut s);
        s.i64_sub()
            .i64_const(span.start as i64)
            .i64_sub()
            .i64_const((span.end - span.start) as i64)
            .i64_lt_u();
        s.local_get(target);
        load.push(&mut s);
        s.i64_sub().i64_const(entry as i64).i64_ne().i32_and();
        s.if_(BlockType::Empty)
            .i32_const(Reason::JumpThroughRegisterNotRecognised as i32)
            .local_get(target)
            .call(F_WARN)
            .end();
        self.leave_through_lookup(target);
    }

    /// Call the piece for the guest address in `target` as a tail call, the registers written back.
    fn leave_through_lookup(&mut self, target: u32) {
        let piece = self.look_up(target);
        self.arguments();
        self.call_looked_up(piece, target, true);
    }

    /// Look up the piece for the guest address in `target`, before the arguments of the call are pushed.
    fn look_up(&mut self, target: u32) -> u32 {
        let offset = self.temporary(ValType::I64);
        let piece = self.temporary(ValType::Ref(crate::thread::piece_ref()));
        let slots = crate::code::slots(
            self.fixed(G_TABLE_BASE),
            self.fixed(G_SLOTS),
            self.fixed(G_SLOTS_COVER),
        );
        crate::thread::lookup::<G>(&mut self.sink(), target, slots, offset, piece);
        piece
    }

    /// Call the piece looked up, or the dispatcher when there is none, with the arguments pushed.
    fn call_looked_up(&mut self, piece: u32, target: u32, tail: bool) {
        let mut s = self.sink();
        s.local_get(piece)
            .ref_is_null()
            .if_(BlockType::FunctionType(0))
            .local_get(target);
        if tail {
            s.return_call(G::F_DISPATCH);
        } else {
            s.call(G::F_DISPATCH);
        }
        s.else_().local_get(piece).ref_as_non_null();
        if tail {
            s.return_call_ref(crate::thread::T_PIECE)
                .end()
                .unreachable();
        } else {
            s.call_ref(crate::thread::T_PIECE).end();
        }
    }
}

impl<G: Guest> Emitter<G> for FullEmitter<'_, G> {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.body)
    }

    fn local(&mut self, ty: ValType) -> u32 {
        self.temporary(ty)
    }

    fn get(&mut self, r: G::Reg) {
        match G::reg_bit(r) {
            Some(bit) => {
                let local = self.register(bit);
                self.sink().local_get(local);
            }
            None => {
                self.sink().i64_const(0);
            }
        }
    }

    fn set(&mut self, r: G::Reg) {
        self.written = true;
        match G::reg_bit(r) {
            Some(bit) => {
                let local = self.register(bit);
                self.sink().local_set(local);
                self.at.dirty = self
                    .at
                    .dirty
                    .union(Registers(1 << bit).intersect(tracked::<G>()));
            }
            None => {
                self.sink().drop();
            }
        }
    }

    fn this_address(&mut self, by: i64) {
        let offset = self.at.offset.wrapping_add_signed(by);
        self.address(offset);
    }

    fn set_flags(&mut self, flags: G::Flags) {
        self.written = true;
        let lazy = self.decoded().effects.flags.is_some();
        match G::shape(flags).filter(|_| lazy) {
            Some(shape) => {
                let id = self.plan.shapes.iter().position(|s| *s == shape).unwrap();
                let from = G::locals_of(flags);
                let to = G::locals_of(self.canonical[id]);
                let mut s = InstructionSink::new(&mut self.body);
                for (&from, &to) in from.as_slice().iter().zip(to.as_slice()) {
                    s.local_get(from).local_set(to);
                }
                self.set_flag_state(FIRST_SHAPE_STATE + id as u32);
            }
            None => {
                for (i, &(flag, _, _)) in G::FLAGS.iter().enumerate() {
                    let local = self.flag_locals[i];
                    let mut s = self.sink();
                    G::flag_value(&mut s, flags, flag);
                    s.local_set(local);
                }
                self.set_flag_state(FLAGS_IN_LOCALS);
            }
        }
    }

    fn flag(&mut self, flag: G::Flag) {
        self.push_flag(flag);
    }

    fn condition(&mut self, cond: G::Cond) {
        let states = self.at.flags;
        if states.count_ones() == 1 && states.trailing_zeros() >= FIRST_SHAPE_STATE {
            let flags = self.shape_flags((states.trailing_zeros() - FIRST_SHAPE_STATE) as usize);
            if G::condition_direct(&mut self.sink(), flags, cond) {
                return;
            }
        }
        G::condition_from_flags(self, cond);
    }

    fn get_vector(&mut self, v: G::Vector) {
        let local = self.register(G::vector_bit(v));
        self.sink().local_get(local);
    }

    fn set_vector(&mut self, v: G::Vector) {
        self.written = true;
        let bit = G::vector_bit(v);
        let local = self.register(bit);
        self.sink().local_set(local);
        self.at.dirty = self.at.dirty.union(Registers(1 << bit));
    }

    fn get_control(&mut self, control: G::Control) {
        let block = self.fixed(G_THREAD_BLOCK);
        let mut s = self.sink();
        block.push(&mut s);
        Slot::control::<G>(control).load(&mut s);
    }

    fn hook(&mut self, hook: G::Hook) {
        // The registers the hook works on go to the thread block before it and come back after: those that
        // may be stale there, and the passed ones, which live only in their parameters.
        let registers = G::HOOKS[G::hook_index(hook)].registers;
        let block = self.fixed(G_THREAD_BLOCK);
        let stale = self.at.dirty.union(Registers::of_bits(G::PASSED_IN));
        for bit in registers.intersect(stale).bits() {
            let local = self.register(bit);
            Slot::of_bit::<G>(bit).store_from(&mut self.sink(), block, local);
        }
        self.at.dirty = self.at.dirty.minus(registers);
        self.sink().call(hook_function::<G>(hook));
        for bit in registers.bits() {
            let local = self.register(bit);
            Slot::of_bit::<G>(bit).load_into(&mut self.sink(), block, local);
        }
    }

    fn host(&self) -> G::Host {
        self.code.host
    }

    fn offer(&self) -> G::Offer {
        self.code.offer
    }

    /// The slow path leaves the piece by a tail call, so no call stays in the function, which would make the
    /// engine keep values out of registers around it.
    fn float_mode(&mut self) -> bool {
        assert!(
            !self.written,
            "floating point's slow path tested after the instruction wrote"
        );
        let local = match self.float_mode_local {
            Some(l) => l,
            None => {
                let l = self.new_local(ValType::I32);
                self.float_mode_local = Some(l);
                l
            }
        };
        if !self.at.float_mode {
            G::float_slow(self);
            self.sink().local_set(local);
            self.at.float_mode = true;
        }
        self.sink().local_get(local).if_(BlockType::Empty);
        self.write_back();
        self.rare_arguments();
        self.this_address(0);
        self.sink().return_call(G::F_EXACT).end();
        false
    }

    fn stack_aligned(&mut self) -> bool {
        self.at.aligned
    }

    fn set_control(&mut self, control: G::Control) {
        self.written = true;
        if G::FLOAT_CONTROLS.contains(&control) {
            self.at.float_mode = false;
        }
        let block = self.fixed(G_THREAD_BLOCK);
        let slot = Slot::control::<G>(control);
        let value = self.temporary(slot.ty());
        let mut s = self.sink();
        s.local_set(value);
        slot.store_from(&mut s, block, value);
    }

    fn check(&mut self, address: u32, bytes: u32, access: Access, which: u8) {
        if self.omit.permissions {
            return;
        }
        let this = match access {
            Access::Read => PAGE_READ,
            Access::Write => PAGE_WRITE,
        };
        // An instruction whose access the plan does not know (cache maintenance) checks exactly.
        let plan = self
            .plan
            .checks
            .get(&(self.at.offset, which))
            .copied()
            .unwrap_or(CheckPlan::Exact);
        if let CheckPlan::Skip = plan {
            return;
        }
        if self
            .plan
            .proven
            .get(&self.at.offset)
            .is_some_and(|h| self.unchecked.contains(h))
        {
            return;
        }
        let (page, fault_at) = (self.temporary(ValType::I64), self.temporary(ValType::I64));
        let offset = self.at.offset;
        // A kept run's check is rare, so it reads the table's place from the module's globals rather than
        // keeping it alive through the loop.
        let table = if self.plan.kept.contains_key(&offset) {
            Table::GLOBALS
        } else {
            self.table()
        };
        let exact = self.exact(table, page);
        match plan {
            CheckPlan::Skip => {}
            CheckPlan::Exact => {
                let kept = self.plan.kept.get(&offset).map(|&n| {
                    let longest = self.plan.kept_longest[n];
                    assert!(
                        bytes <= longest,
                        "an access longer than its kept run allows for"
                    );
                    (self.kept_pages[n], longest)
                });
                let mut s = self.sink();
                if let Some(((low, room), _)) = kept {
                    // Checked again unless the access starts within the kept run of pages.
                    s.local_get(address)
                        .local_get(low)
                        .i64_sub()
                        .local_get(room)
                        .i64_ge_u()
                        .if_(BlockType::Empty);
                }
                // An address the table allows is one data accesses use as it is, so only one it does not is
                // made what they use and tried again.
                permission_fast(&mut s, table, address, this, page);
                G::data_address(&mut s, address);
                s.local_set(address);
                permission_exact::<G>(&mut s, address, bytes, this, fault_at, exact);
                self.sites[Reason::MemoryFault as usize] += 1;
                self.save(Pc::Offset(offset), true);
                let mut s = self.sink();
                s.local_get(fault_at)
                    .i32_const(i32::from(access == Access::Write))
                    .call(F_MEMORY_FAULT)
                    .i32_const(UNWIND_LEAVE)
                    .call(G::F_UNWIND)
                    .unreachable()
                    .end()
                    .end();
                if let Some(((low, room), longest)) = kept {
                    // The access is allowed, so the page it starts on allows this kind.
                    keep_pages(&mut s, table, (address, page), this, longest, (low, room));
                    s.end();
                }
            }
            CheckPlan::Group { from, len } => {
                let start = self.temporary(ValType::I64);
                self.sink()
                    .local_get(address)
                    .i64_const(from)
                    .i64_add()
                    .local_set(start);
                let mut s = self.sink();
                permission_fast(&mut s, table, start, this, page);
                permission_exact::<G>(&mut s, start, len, this, fault_at, exact);
                self.leave_to(offset);
                self.sink().end().end();
            }
        }
    }

    fn check_range(&mut self, address: u32, length: u32, access: Access) -> u32 {
        let (page, last) = (self.temporary(ValType::I64), self.temporary(ValType::I64));
        let allowed = self.temporary(ValType::I32);
        let table = self.table();
        let mut s = self.sink();
        G::data_address(&mut s, address);
        s.local_set(address);
        permission_range(
            &mut s,
            table,
            (address, length),
            page_bit(access),
            (page, last),
            allowed,
        );
        allowed
    }

    fn warn(&mut self, reason: G::Reason) {
        let number = guest_reason::<G>(reason);
        self.sites[number as usize] += 1;
        self.sink().i32_const(number as i32);
        self.this_address(0);
        self.sink().call(F_WARN);
    }

    fn raise_stop(&mut self) {
        self.raised_stop = true;
        let block = self.fixed(G_THREAD_BLOCK);
        let progress = self.code.progress;
        raise_stop::<G>(&mut self.sink(), block, progress);
    }

    fn jump(&mut self, target: Target) {
        self.count_event();
        match target {
            Target::Relative(by) => {
                let to = self.at.offset.wrapping_add_signed(by);
                self.goto(to);
            }
            Target::Address(local) => {
                let table = self.function.blocks[self.at.block].table.clone();
                match table {
                    Some(cases) => self.table_jump(local, &cases),
                    None => self.unrecognised_jump(local),
                }
            }
        }
    }

    fn jump_if(&mut self, target: Target) {
        self.count_event();
        self.sink().if_(BlockType::Empty);
        self.labels.push(Label::Other);
        let saved = self.at;
        self.jump(target);
        self.at = saved;
        self.sink().end();
        self.labels.pop();
    }

    fn call(&mut self, target: Target) {
        self.count_event();
        // The callee may change the floating point controls.
        self.at.float_mode = false;
        let offset = self.at.offset;
        let next = offset + u64::from(self.decoded().length);
        self.write_back();
        let looked_up = match target {
            Target::Address(local) => Some(self.look_up(local)),
            Target::Relative(_) => None,
        };
        let mut s = self.sink();
        for i in 0..G::PASSED {
            s.local_get(i);
        }
        // No return address has the bit, so adding it sets it.
        let wanted = if self.flags_needed_after(offset) {
            G::FLAGS_WANTED as u64
        } else {
            0
        };
        self.address(next + wanted);
        let (depth, count) = (self.depth, self.count);
        self.sink().local_get(depth);
        if let Some(count) = count {
            self.sink().local_get(count);
        }
        match target {
            Target::Relative(by) => {
                let to = offset.wrapping_add_signed(by);
                match self.index.get(&to) {
                    Some(&f) => {
                        self.sink().call(f);
                    }
                    None => {
                        self.address(to);
                        self.sink().call(G::F_DISPATCH);
                    }
                }
            }
            Target::Address(local) => {
                let piece = looked_up.expect("looked up");
                self.call_looked_up(piece, local, false);
            }
        }
        let returned_sp = self.temporary(ValType::I64);
        if let Some(count) = count {
            self.sink().local_set(count);
        }
        let mut reload = self.plan.live_after[&offset];
        for &bit in G::PASSED_OUT.iter().rev() {
            let local = if bit == G::SP_BIT {
                returned_sp
            } else if let Some(local) = G::passed(bit) {
                local
            } else {
                // Passed out only: the thread block holds the same value, so it needs no load.
                reload = reload.minus(Registers(1 << bit));
                self.register(bit)
            };
            self.sink().local_set(local);
        }
        self.at.dirty = Registers::NONE;
        self.set_flag_state(FLAGS_IN_BLOCK);
        // What is known about the stack pointer (its frame, its alignment) holds after the call only if the
        // callee gave it back unchanged; otherwise carry on from code that knows nothing.
        let kept =
            self.at.aligned || (self.plan.frame_across.contains(&offset) && !self.omit.permissions);
        if kept {
            let mut s = self.sink();
            s.local_get(returned_sp).local_get(G::L_SP);
            // The callee's return moved the stack pointer up past the return address.
            if let ReturnAddress::Stack { bytes } = G::RETURN_ADDRESS {
                s.i64_const(bytes.into()).i64_add();
            }
            s.i64_ne()
                .local_get(returned_sp)
                .local_set(G::L_SP)
                .if_(BlockType::Empty);
            self.leave_to(next);
            self.sink().end();
        } else {
            self.sink().local_get(returned_sp).local_set(G::L_SP);
        }
        let block = self.fixed(G_THREAD_BLOCK);
        for bit in reload.bits() {
            let local = self.register(bit);
            Slot::of_bit::<G>(bit).load_into(&mut self.sink(), block, local);
        }
    }

    fn ret(&mut self, target: u32) {
        self.count_event();
        pop_return_address::<G>(&mut self.sink());
        self.write_back_registers();
        // The flags go back only when the caller asks for them.
        let flags = self.at.flags;
        let wanted = |e: &mut Self| {
            if flags != 1 << FLAGS_IN_BLOCK {
                e.sink()
                    .local_get(G::L_RETURN)
                    .i64_const(G::FLAGS_WANTED)
                    .i64_and()
                    .i32_wrap_i64()
                    .if_(BlockType::Empty);
                e.write_back_flags(false);
                e.at.flags = flags;
                e.sink().end();
            }
        };
        let block = self.fixed(G_THREAD_BLOCK);
        let progress = self.code.progress;
        if self.omit.returns {
            wanted(self);
            push_results::<G>(&mut self.sink(), block, progress);
            self.sink().return_();
            return;
        }
        let mut s = self.sink();
        s.local_get(target);
        crate::code::push_expected_return::<G>(&mut s);
        s.i64_eq().if_(BlockType::Empty);
        wanted(self);
        let mut s = self.sink();
        push_results::<G>(&mut s, block, progress);
        s.return_().end();
        // Not where the caller expects: go to the address from the bottom loop.
        self.write_back_flags(false);
        let mut s = self.sink();
        set_pc::<G>(&mut s, block, |s| {
            s.local_get(target);
        });
        bottom_return::<G>(&mut s, block, progress);
        save_passed::<G>(&mut s, block, progress);
        s.i32_const(UNWIND_GO_TO).call(G::F_UNWIND).unreachable();
    }

    fn syscall(&mut self) {
        self.at.float_mode = false;
        let next = self.at.offset + u64::from(self.decoded().length);
        self.save(Pc::Offset(next), false);
        let registers = G::SYSCALL;
        self.get(registers.number);
        for r in registers.args {
            self.get(r);
        }
        self.sink().call(F_SYSCALL);
        // Nothing but the result has changed, and everything was just written back.
        let result = G::reg_bit(registers.result).expect("a result register");
        let local = self.register(result);
        self.sink().local_set(local);
        self.at.dirty = Registers(1 << result).intersect(tracked::<G>());
        let live = self.plan.live_after[&self.at.offset];
        self.reload(live.minus(Registers(1 << result)), G::passed(result));
        // The kernel raises the flag during the call when the thread must not carry on.
        self.stop_check(Pc::Offset(next), AfterStop::Reload(live));
    }

    fn trap(&mut self, trap: G::Trap) {
        let (kind, detail, pc) = G::trap_kind(trap);
        if kind == TrapKind::Unsupported {
            self.sites[Reason::UnsupportedInstructionReached as usize] += 1;
        }
        let offset = self.at.offset;
        let length = self.decoded().length;
        let at = match pc {
            TrapPc::At => offset,
            TrapPc::After => offset + u64::from(length),
        };
        self.save(Pc::Offset(at), true);
        let instruction = G::trap_detail(self.code.bytes(offset));
        let mut s = self.sink();
        s.i32_const(kind as i32);
        trap_detail(&mut s, detail, instruction);
        s.call(F_TRAP)
            .i32_const(UNWIND_LEAVE)
            .call(G::F_UNWIND)
            .unreachable();
    }

    fn next(&mut self) {
        let next = self.at.offset + u64::from(self.decoded().length);
        // The stop hook runs before the next instruction when this one raised the flag.
        if self.raised_stop {
            self.raised_stop = false;
            let live = self.plan.live_after[&self.at.offset];
            self.stop_check(Pc::Offset(next), AfterStop::Reload(live));
        }
        if self.at.offset != self.function.last(self.at.block) {
            return;
        }
        self.goto(next);
    }
}
