//! Full look-ahead: the blob is cut into functions and each becomes one piece. Inside a piece registers are
//! locals, flags are kept as the last comparison, jumps are blocks and loops, and calls to other pieces are
//! WebAssembly calls.

use crate::code::*;
use crate::contract::*;
use crate::thread::memarg;
use petgraph::algo::dominators;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::DfsPostOrder;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_arm64::{
    Access, Address, BranchRegister, Carry, Cond, Effects, Emitter, Flag, Flags, Instruction,
    Operand, Reg, Registers, Target, Trap, Width, condition_direct, condition_from_flags, emit,
    flag_value, habits, untag,
};

/// How far back a jump through a register looks for the switch it may come from.
const SWITCH_WINDOW: u64 = 64;

/// The pieces of a full translation, by entry offset.
pub(crate) fn pieces(
    code: &Code<'_>,
    starts: &[u64],
    omit: Omit,
) -> BTreeMap<u64, wasm_encoder::Function> {
    let mut finder = Finder {
        code,
        decoded: HashMap::new(),
    };
    let functions = finder.functions(starts);
    let index: BTreeMap<u64, u32> = functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.entry, F_FIRST_PIECE + i as u32))
        .collect();
    functions
        .iter()
        .map(|function| {
            let plan = Plan::new(function, &finder.decoded);
            (
                function.entry,
                FullEmitter::piece(function, &plan, &finder.decoded, &index, omit),
            )
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Decoded {
    word: u32,
    instruction: Instruction,
    effects: Effects,
}

/// A run of instructions entered only at its start and left only at its end.
struct Block {
    start: u64,
    /// Just past the last instruction.
    end: u64,
    succs: Vec<usize>,
    /// The targets of the jump table this block ends with, if it ends with one.
    table: Option<Vec<u64>>,
}

/// One function: everything reachable from its entry without passing another entry. Block 0 is the entry.
struct Function {
    entry: u64,
    blocks: Vec<Block>,
    /// Block index by start offset.
    at: HashMap<u64, usize>,
}

struct Finder<'a> {
    code: &'a Code<'a>,
    decoded: HashMap<u64, Decoded>,
}

/// Whether control can leave an instruction other than by going on to the next one or by a call that returns.
fn ends_block(instruction: &Instruction) -> bool {
    let flow = instruction.flow();
    let call = matches!(instruction, Instruction::Branch { link: true, .. });
    !(flow.falls_through && (flow.target.is_none() || call))
}

impl Finder<'_> {
    fn get(&mut self, offset: u64) -> Decoded {
        let code = self.code;
        *self.decoded.entry(offset).or_insert_with(|| {
            let word = code.word(offset);
            let instruction = wcontainers_arm64::decode(word);
            Decoded {
                word,
                instruction,
                effects: instruction.effects(),
            }
        })
    }

    /// Every function: the starts, targets of direct calls, addresses the code builds that point into code,
    /// and the extra entries that keep every function's control flow reducible.
    fn functions(&mut self, starts: &[u64]) -> Vec<Function> {
        let mut entries: BTreeSet<u64> = starts.iter().copied().collect();
        let mut explored = BTreeSet::new();
        loop {
            let mut work: Vec<u64> = entries.difference(&explored).copied().collect();
            while let Some(entry) = work.pop() {
                explored.insert(entry);
                let mut found = BTreeSet::new();
                self.explore(entry, &entries, &mut found);
                for offset in found {
                    if self.code.inside(offset) && entries.insert(offset) {
                        work.push(offset);
                    }
                }
            }
            let functions: Vec<Function> = entries
                .iter()
                .map(|&entry| self.explore(entry, &entries, &mut BTreeSet::new()))
                .collect();
            let mut more = false;
            for function in &functions {
                for offset in irreducible(function) {
                    more |= entries.insert(offset);
                }
            }
            if !more {
                return functions;
            }
        }
    }

    /// The function at an entry, with what it shows of other entries added to `found`.
    fn explore(
        &mut self,
        entry: u64,
        entries: &BTreeSet<u64>,
        found: &mut BTreeSet<u64>,
    ) -> Function {
        let mut body = BTreeSet::new();
        let mut leaders = BTreeSet::from([entry]);
        let mut tables: HashMap<u64, Vec<u64>> = HashMap::new();
        // Addresses the code builds; those a jump table of this function goes to are its own blocks.
        let mut built = Vec::new();
        let mut work = vec![entry];
        while let Some(offset) = work.pop() {
            if !self.code.inside(offset) || (offset != entry && entries.contains(&offset)) {
                continue;
            }
            if !body.insert(offset) {
                continue;
            }
            let instruction = self.get(offset).instruction;
            let flow = instruction.flow();
            let mut jumps = Vec::new();
            match instruction {
                Instruction::Branch {
                    link: true,
                    offset: by,
                } => {
                    found.insert(offset.wrapping_add_signed(by));
                }
                Instruction::PcRelative {
                    page: false,
                    offset: by,
                    ..
                } => {
                    built.push(offset.wrapping_add_signed(by));
                }
                Instruction::AddSub {
                    width: Width::X64,
                    subtract: false,
                    set_flags: false,
                    rn,
                    operand: Operand::Imm(value),
                    ..
                } if offset >= 4 && self.code.inside(offset - 4) => {
                    if let Instruction::PcRelative {
                        rd,
                        page: true,
                        offset: by,
                    } = self.get(offset - 4).instruction
                        && rd == rn
                    {
                        built.push(
                            ((offset - 4) & !0xfff)
                                .wrapping_add_signed(by)
                                .wrapping_add(value),
                        );
                    }
                }
                Instruction::BranchRegister {
                    op: BranchRegister::Br,
                    rn,
                } => {
                    if let Some(targets) = self.jump_table(offset, rn) {
                        jumps.extend(&targets);
                        tables.insert(offset, targets);
                    }
                }
                _ => {}
            }
            let call = matches!(instruction, Instruction::Branch { link: true, .. });
            if let Some(by) = flow.target
                && !call
            {
                jumps.push(offset.wrapping_add_signed(by));
            }
            leaders.extend(&jumps);
            if flow.falls_through {
                if ends_block(&instruction) {
                    leaders.insert(offset + 4);
                }
                work.push(offset + 4);
            }
            work.extend(jumps);
        }

        found.extend(
            built
                .into_iter()
                .filter(|b| !tables.values().any(|t| t.contains(b))),
        );

        // Cut the body into blocks, the entry's first.
        let mut blocks: Vec<Block> = Vec::new();
        let mut previous: Option<u64> = None;
        for &offset in &body {
            let joined = previous.is_some_and(|p| {
                p + 4 == offset
                    && !leaders.contains(&offset)
                    && !ends_block(&self.decoded[&p].instruction)
            });
            if joined {
                blocks.last_mut().unwrap().end = offset + 4;
            } else {
                blocks.push(Block {
                    start: offset,
                    end: offset + 4,
                    succs: Vec::new(),
                    table: None,
                });
            }
            previous = Some(offset);
        }
        let first = blocks.iter().position(|b| b.start == entry).unwrap();
        blocks.swap(0, first);
        let at: HashMap<u64, usize> = blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.start, i))
            .collect();
        for block in &mut blocks {
            let last = block.end - 4;
            let instruction = self.decoded[&last].instruction;
            let flow = instruction.flow();
            let mut targets = Vec::new();
            if let Some(by) = flow.target
                && ends_block(&instruction)
            {
                targets.push(last.wrapping_add_signed(by));
            }
            if flow.falls_through {
                targets.push(last + 4);
            }
            if let Some(table) = tables.get(&last) {
                targets.extend(table);
                block.table = Some(table.clone());
            }
            block.succs = targets
                .into_iter()
                .filter(|t| body.contains(t))
                .map(|t| at[&t])
                .collect();
        }
        Function { entry, blocks, at }
    }

    /// The targets of a switch's jump table ending at the jump through a register at `offset`, if the code
    /// before it has that shape and every target is code.
    fn jump_table(&mut self, offset: u64, rn: Reg) -> Option<Vec<u64>> {
        let first = offset
            .saturating_sub(4 * SWITCH_WINDOW)
            .max(self.code.range.start);
        let before: Vec<(u64, Instruction)> = (first..offset)
            .step_by(4)
            .filter(|&o| self.code.inside(o))
            .map(|o| (o, self.get(o).instruction))
            .collect();
        let targets = habits::jump_table(&before, rn, |at, bytes| self.code.read(at, bytes))?;
        targets
            .iter()
            .all(|&t| self.code.inside(t))
            .then_some(targets)
    }
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
fn tracked() -> Registers {
    Registers::ALL
        .minus(Registers::FLAGS)
        .minus(Registers(((1 << 4) - 1) | (1 << 31)))
}

/// Where the flags are known to be at a point: a set of [`FlagState`] bits, one bit when it is known which.
type FlagStates = u32;
/// The flags are in the piece's flag locals.
const FLAGS_IN_LOCALS: u32 = 0;
/// The flags are in the thread's globals, which are up to date.
const FLAGS_IN_GLOBALS: u32 = 1;

/// How a flag setting that is kept lazily looks, so setters of one shape can share locals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Add {
        width: Width,
        carry: u8,
        subtract: bool,
    },
    Logic(Width),
}

impl Shape {
    fn of(flags: Flags) -> Option<Shape> {
        match flags {
            Flags::Add {
                width,
                carry,
                subtrahend,
                ..
            } => Some(Shape::Add {
                width,
                carry: match carry {
                    Carry::Zero => 0,
                    Carry::One => 1,
                    Carry::Local(_) => 2,
                },
                subtract: subtrahend.is_some(),
            }),
            Flags::Logic { width, .. } => Some(Shape::Logic(width)),
            _ => None,
        }
    }
}

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
struct Plan {
    rank: Vec<usize>,
    children: Vec<Vec<usize>>,
    merge: Vec<bool>,
    loop_header: Vec<bool>,
    /// Registers that must hold their true value at the start of each block; the entry's are loaded.
    live_in: Vec<Registers>,
    /// Registers that must hold their true value after each instruction.
    live_after: HashMap<u64, Registers>,
    dirty_in: Vec<Registers>,
    flags_in: Vec<FlagStates>,
    aligned_in: Vec<bool>,
    /// Shape of every flag setting kept lazily, by its flag state number.
    shapes: Vec<Shape>,
    /// Some point does not know statically where the flags are, so a local says.
    dynamic_flags: bool,
    checks: HashMap<u64, CheckPlan>,
    /// Ranges relative to the stack pointer before the instruction, checked as the stack grows.
    frame_checks: HashMap<u64, (i64, u32)>,
    /// Calls after which a frame is known, so the stack pointer must come back unchanged.
    frame_across: HashSet<u64>,
    /// The piece calls or jumps to other pieces, so its entry needs the stop check.
    calls_out: bool,
}

/// How the stack pointer moves at an instruction: by a known amount (before or after its access), or to
/// something unknown.
enum StackMove {
    None,
    By { delta: i64, before_access: bool },
    Unknown,
}

fn stack_move(d: &Decoded) -> StackMove {
    match d.instruction {
        Instruction::AddSub {
            width: Width::X64,
            subtract,
            set_flags: false,
            rd: Reg::Sp,
            rn: Reg::Sp,
            operand: Operand::Imm(k),
        } if k < 1 << 24 => StackMove::By {
            delta: if subtract { -(k as i64) } else { k as i64 },
            before_access: true,
        },
        _ => match d.instruction.memory_access().map(|a| a.address) {
            Some(Address::PreIndex {
                base: Reg::Sp,
                offset,
            }) => StackMove::By {
                delta: offset,
                before_access: true,
            },
            Some(Address::PostIndex {
                base: Reg::Sp,
                offset,
            }) => StackMove::By {
                delta: offset,
                before_access: false,
            },
            _ if d.effects.defs.has(Reg::Sp) => StackMove::Unknown,
            _ => StackMove::None,
        },
    }
}

/// Whether the stack pointer is known to be a multiple of 16 after an instruction: an access through it checks
/// that, and moving it by a multiple of 16 keeps it.
fn aligned_after(d: &Decoded, aligned: bool) -> bool {
    let checked = d.instruction.memory_access().is_some_and(|a| {
        matches!(
            a.address,
            Address::Offset { base: Reg::Sp, .. }
                | Address::PreIndex { base: Reg::Sp, .. }
                | Address::PostIndex { base: Reg::Sp, .. }
                | Address::Indexed { base: Reg::Sp, .. }
        )
    });
    match stack_move(d) {
        StackMove::None => aligned || checked,
        StackMove::By { delta, .. } => (aligned || checked) && delta % 16 == 0,
        StackMove::Unknown => false,
    }
}

/// Shift a frame to a stack pointer moved by `delta`.
fn shift(frame: Frame, delta: i64) -> Frame {
    frame.map(|(lo, hi)| (lo - delta, hi - delta))
}

fn covers(frame: Frame, lo: i64, hi: i64) -> bool {
    frame.is_some_and(|(a, b)| a <= lo && hi <= b)
}

impl Plan {
    fn new(function: &Function, decoded: &HashMap<u64, Decoded>) -> Plan {
        let blocks = &function.blocks;
        let n = blocks.len();
        let (graph, rank) = ranks(function);
        let doms = dominators::simple_fast(&graph, NodeIndex::new(0));
        let mut children = vec![Vec::new(); n];
        for b in 1..n {
            let idom = doms.immediate_dominator(NodeIndex::new(b)).unwrap().index();
            children[idom].push(b);
        }
        let mut forward_in = vec![0; n];
        let mut loop_header = vec![false; n];
        let mut merge = vec![false; n];
        let mut preds = vec![Vec::new(); n];
        for (u, block) in blocks.iter().enumerate() {
            for &v in &block.succs {
                preds[v].push(u);
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
        let instructions = |b: usize| (blocks[b].start..blocks[b].end).step_by(4);

        // Flag setting shapes, numbered after the two fixed states.
        let mut shapes: Vec<Shape> = Vec::new();
        for d in function_instructions(function).map(|o| &decoded[&o]) {
            if let Some(shape) = d.effects.flags.and_then(Shape::of)
                && !shapes.contains(&shape)
            {
                shapes.push(shape);
            }
        }
        let flag_state = |d: &Decoded, state: FlagStates| -> FlagStates {
            if d.effects.calls {
                1 << FLAGS_IN_GLOBALS
            } else if let Some(shape) = d.effects.flags.and_then(Shape::of) {
                1 << (2 + shapes.iter().position(|s| *s == shape).unwrap())
            } else if d.effects.defs.intersect(Registers::FLAGS) != Registers::NONE {
                1 << FLAGS_IN_LOCALS
            } else {
                state
            }
        };
        let dirty_after = |d: &Decoded, dirty: Registers| -> Registers {
            if d.effects.calls || d.effects.syscall {
                Registers::NONE
            } else {
                dirty.union(d.effects.defs.intersect(tracked()))
            }
        };
        let frame_after = |d: &Decoded, frame: Frame| -> Frame {
            match stack_move(d) {
                StackMove::None => frame,
                StackMove::Unknown => None,
                StackMove::By { delta, .. } => shift(grow(frame, delta), delta),
            }
        };

        // Forward: what may be dirty, where the flags are, and the frame known to be allowed.
        let mut dirty_in = vec![Registers::NONE; n];
        let mut flags_in: Vec<FlagStates> = vec![0; n];
        let mut frame_in: Vec<Option<Frame>> = vec![None; n];
        let mut aligned_in = vec![true; n];
        aligned_in[0] = false;
        flags_in[0] = 1 << FLAGS_IN_GLOBALS;
        frame_in[0] = Some(None);
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
        let mut live_after = HashMap::new();
        let mut changed = true;
        while changed {
            changed = false;
            for &b in order.iter().rev() {
                let mut live = blocks[b]
                    .succs
                    .iter()
                    .fold(Registers::NONE, |l, &s| l.union(live_in_blocks[s]));
                for o in instructions(b).collect::<Vec<_>>().into_iter().rev() {
                    let d = &decoded[&o];
                    live_after.insert(o, live);
                    live = if d.effects.calls {
                        d.effects.uses
                    } else {
                        live.minus(d.effects.defs).union(d.effects.uses)
                    }
                    .intersect(tracked());
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
        } = checks(function, decoded, &frame_in);
        let dynamic_flags = flags_in.iter().any(|f| f.count_ones() > 1);
        let calls_out = function_instructions(function).any(|o| {
            let d = &decoded[&o];
            d.effects.calls || leaves_piece(function, o, &d.instruction)
        });
        Plan {
            rank,
            children,
            merge,
            loop_header,
            live_in: live_in_blocks,
            live_after,
            dirty_in,
            flags_in,
            aligned_in,
            shapes,
            dynamic_flags,
            checks,
            frame_checks,
            frame_across,
            calls_out,
        }
    }
}

fn function_instructions(function: &Function) -> impl Iterator<Item = u64> + '_ {
    function
        .blocks
        .iter()
        .flat_map(|b| (b.start..b.end).step_by(4))
}

/// Whether an instruction can pass control to another piece other than by a call that returns.
fn leaves_piece(function: &Function, offset: u64, instruction: &Instruction) -> bool {
    let flow = instruction.flow();
    let outside = |t: u64| !function.at.contains_key(&t);
    let call = matches!(instruction, Instruction::Branch { link: true, .. });
    match instruction {
        Instruction::BranchRegister {
            op: BranchRegister::Br,
            ..
        } => true,
        _ => {
            (flow
                .target
                .is_some_and(|by| !call && outside(offset.wrapping_add_signed(by))))
                || (flow.falls_through
                    && (offset + 4) == block_end(function, offset)
                    && outside(offset + 4))
        }
    }
}

fn block_end(function: &Function, offset: u64) -> u64 {
    function
        .blocks
        .iter()
        .find(|b| (b.start..b.end).contains(&offset))
        .unwrap()
        .end
}

/// Which accesses need a check, where the stack's frame is checked, and the calls a frame is kept across.
struct Checks {
    plans: HashMap<u64, CheckPlan>,
    frame_checks: HashMap<u64, (i64, u32)>,
    frame_across: HashSet<u64>,
}

fn checks(function: &Function, decoded: &HashMap<u64, Decoded>, frame_in: &[Frame]) -> Checks {
    let mut plans = HashMap::new();
    let mut frame_checks = HashMap::new();
    let mut frame_across = HashSet::new();
    for (b, block) in function.blocks.iter().enumerate() {
        let mut frame = frame_in[b];
        // Per base register: how far its value has moved from where its current groups are measured.
        let mut moved: HashMap<u8, i64> = HashMap::new();
        // Open groups by base register and write: first access, its start, and the bytes covered.
        let mut open: HashMap<(u8, bool), usize> = HashMap::new();
        let mut groups: Vec<(u64, i64, i64, i64, u32)> = Vec::new();
        for o in (block.start..block.end).step_by(4) {
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
                frame_checks.insert(o, (delta, (-delta) as u32));
            }
            if let Some(access) = d.instruction.memory_access() {
                let plan = match access.address {
                    Address::Offset { base: Reg::Sp, .. }
                    | Address::PreIndex { base: Reg::Sp, .. }
                    | Address::PostIndex { base: Reg::Sp, .. } => {
                        let at = match (access.address, &movement) {
                            (Address::Offset { offset, .. }, _) => offset,
                            (Address::PreIndex { .. }, _) => 0,
                            _ => 0,
                        };
                        let known = match movement {
                            StackMove::By {
                                delta,
                                before_access: true,
                            } => shift(grow(frame, delta), delta),
                            _ => frame,
                        };
                        if covers(known, at, at + i64::from(access.bytes)) {
                            CheckPlan::Skip
                        } else {
                            CheckPlan::Exact
                        }
                    }
                    Address::Offset {
                        base: Reg::X(n),
                        offset,
                    }
                    | Address::PreIndex {
                        base: Reg::X(n),
                        offset,
                    }
                    | Address::PostIndex {
                        base: Reg::X(n),
                        offset,
                    } => {
                        let m = *moved.get(&n).unwrap_or(&0);
                        let (start, after) = match access.address {
                            Address::Offset { .. } => (m + offset, m),
                            Address::PreIndex { .. } => (m + offset, m + offset),
                            _ => (m, m + offset),
                        };
                        let end = start + i64::from(access.bytes);
                        moved.insert(n, after);
                        let key = (n, access.write);
                        match open.get(&key).map(|&g| (g, groups[g])) {
                            Some((g, (first, first_start, lo, hi, count)))
                                if hi.max(end) - lo.min(start) <= 4096 =>
                            {
                                groups[g] =
                                    (first, first_start, lo.min(start), hi.max(end), count + 1);
                                CheckPlan::Skip
                            }
                            _ => {
                                open.insert(key, groups.len());
                                groups.push((o, start, start, end, 1));
                                CheckPlan::Exact
                            }
                        }
                    }
                    _ => CheckPlan::Exact,
                };
                plans.insert(o, plan);
            }
            // Any other write of a base starts it afresh.
            let written_back = d.instruction.memory_access().and_then(|a| match a.address {
                Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => Some(base),
                _ => None,
            });
            for bit in d.effects.defs.bits().filter(|&b| b < 31) {
                let r = Reg::X(bit as u8);
                if Some(r) != written_back {
                    moved.remove(&(bit as u8));
                    open.remove(&(bit as u8, false));
                    open.remove(&(bit as u8, true));
                }
            }
            frame = match movement {
                StackMove::None => frame,
                StackMove::Unknown => None,
                StackMove::By { delta, .. } => shift(grow(frame, delta), delta),
            };
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

/// A frame before the stack pointer moves by `delta`, with the part a move down checks added.
fn grow(frame: Frame, delta: i64) -> Frame {
    if delta >= 0 {
        return frame;
    }
    match frame {
        Some((lo, hi)) if lo <= 0 && hi >= 0 => Some((delta.min(lo), hi)),
        _ => Some((delta, 0)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Label {
    Loop(usize),
    Merge(usize),
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
}

/// What a stop check or write back sets `pc` to.
#[derive(Clone, Copy)]
enum Pc {
    Offset(u64),
    /// The guest address in this i64 local.
    Local(u32),
}

struct FullEmitter<'a> {
    function: &'a Function,
    plan: &'a Plan,
    decoded: &'a HashMap<u64, Decoded>,
    index: &'a BTreeMap<u64, u32>,
    omit: Omit,
    body: Vec<u8>,
    locals: Vec<ValType>,
    free: Vec<(ValType, u32)>,
    taken: Vec<(ValType, u32)>,
    registers: HashMap<u32, u32>,
    flag_locals: [u32; 4],
    /// Which flag state holds, when that is only known at run time.
    flag_state: u32,
    /// The locals each lazily kept flag shape shares.
    canonical: Vec<Flags>,
    labels: Vec<Label>,
    at: At,
}

impl<'a> FullEmitter<'a> {
    fn piece(
        function: &'a Function,
        plan: &'a Plan,
        decoded: &'a HashMap<u64, Decoded>,
        index: &'a BTreeMap<u64, u32>,
        omit: Omit,
    ) -> wasm_encoder::Function {
        let mut e = FullEmitter {
            function,
            plan,
            decoded,
            index,
            omit,
            body: Vec::new(),
            locals: Vec::new(),
            free: Vec::new(),
            taken: Vec::new(),
            registers: HashMap::new(),
            flag_locals: [0; 4],
            flag_state: 0,
            canonical: Vec::new(),
            labels: Vec::new(),
            at: At {
                offset: function.entry,
                block: 0,
                dirty: Registers::NONE,
                flags: 1 << FLAGS_IN_GLOBALS,
                aligned: false,
            },
        };
        e.flag_locals = [0; 4].map(|_| e.new_local(ValType::I32));
        e.flag_state = e.new_local(ValType::I32);
        for shape in plan.shapes.clone() {
            let flags = e.canonical_flags(shape);
            e.canonical.push(flags);
        }
        for bit in plan.live_in[0].bits() {
            let local = e.register(bit);
            let global = global_of(bit);
            e.sink().global_get(global).local_set(local);
        }
        if plan.dynamic_flags {
            let state = e.flag_state;
            e.sink().i32_const(FLAGS_IN_GLOBALS as i32).local_set(state);
        }
        if plan.calls_out && !omit.stop {
            e.stop_check(Pc::Offset(function.entry), plan.live_in[0], 0);
        }
        e.tree(0);
        e.sink().unreachable().end();
        let mut f = wasm_encoder::Function::new_with_locals_types(e.locals);
        f.raw(e.body);
        f
    }

    fn new_local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        6 + self.locals.len() as u32 - 1
    }

    /// The locals a shape keeps: the operands only (a and the value subtracted, or a, b and the carry),
    /// since the rest can be worked out from them when needed; `b` and `result` of a kept addition are
    /// not locals.
    fn canonical_flags(&mut self, shape: Shape) -> Flags {
        let ty = |w: Width| match w {
            Width::W32 => ValType::I32,
            Width::X64 => ValType::I64,
        };
        match shape {
            Shape::Add {
                width,
                carry,
                subtract,
            } => Flags::Add {
                width,
                a: self.new_local(ty(width)),
                b: if subtract {
                    u32::MAX
                } else {
                    self.new_local(ty(width))
                },
                carry: match carry {
                    0 => Carry::Zero,
                    1 => Carry::One,
                    _ => Carry::Local(self.new_local(ValType::I32)),
                },
                result: u32::MAX,
                subtrahend: subtract.then(|| self.new_local(ty(width))),
            },
            Shape::Logic(width) => Flags::Logic {
                width,
                result: self.new_local(ty(width)),
            },
        }
    }

    /// The flags of a kept shape with every local filled in: what was not kept is worked out into
    /// temporaries (an engine drops what is not used).
    fn shape_flags(&mut self, id: usize) -> Flags {
        let Flags::Add {
            width,
            a,
            b,
            carry,
            subtrahend,
            ..
        } = self.canonical[id]
        else {
            return self.canonical[id];
        };
        let ty = match width {
            Width::W32 => ValType::I32,
            Width::X64 => ValType::I64,
        };
        let b = match subtrahend {
            Some(m) => {
                let inverted = self.temporary(ty);
                let mut s = self.sink();
                s.local_get(m);
                match width {
                    Width::W32 => s.i32_const(-1).i32_xor(),
                    Width::X64 => s.i64_const(-1).i64_xor(),
                };
                s.local_set(inverted);
                inverted
            }
            None => b,
        };
        let result = self.temporary(ty);
        let mut s = self.sink();
        s.local_get(a).local_get(b);
        let add = |s: &mut InstructionSink<'_>| match width {
            Width::W32 => {
                s.i32_add();
            }
            Width::X64 => {
                s.i64_add();
            }
        };
        add(&mut s);
        match carry {
            Carry::Zero => {}
            Carry::One => {
                match width {
                    Width::W32 => s.i32_const(1),
                    Width::X64 => s.i64_const(1),
                };
                add(&mut s);
            }
            Carry::Local(c) => {
                s.local_get(c);
                if width == Width::X64 {
                    s.i64_extend_i32_u();
                }
                add(&mut s);
            }
        }
        s.local_set(result);
        Flags::Add {
            width,
            a,
            b,
            carry,
            result,
            subtrahend,
        }
    }

    /// The local of a register bit: a parameter for the passed ones.
    fn register(&mut self, bit: u32) -> u32 {
        match bit {
            0..4 => bit,
            31 => L_SP,
            _ => {
                if let Some(&local) = self.registers.get(&bit) {
                    return local;
                }
                let ty = if bit >= 64 {
                    ValType::V128
                } else {
                    ValType::I64
                };
                let local = self.new_local(ty);
                self.registers.insert(bit, local);
                local
            }
        }
    }

    fn decoded(&self) -> Decoded {
        self.decoded[&self.at.offset]
    }

    // Structure, after Ramsey's "Beyond Relooper".

    fn tree(&mut self, b: usize) {
        let merges: Vec<usize> = self.plan.children[b]
            .iter()
            .copied()
            .filter(|&c| self.plan.merge[c])
            .collect();
        if self.plan.loop_header[b] {
            self.sink().loop_(BlockType::Empty);
            self.labels.push(Label::Loop(b));
            self.within(b, &merges);
            self.sink().end();
            self.labels.pop();
        } else {
            self.within(b, &merges);
        }
    }

    fn within(&mut self, b: usize, merges: &[usize]) {
        match merges.split_first() {
            Some((&m, rest)) => {
                self.sink().block(BlockType::Empty);
                self.labels.push(Label::Merge(m));
                self.within(b, rest);
                self.sink().end();
                self.labels.pop();
                self.tree(m);
            }
            None => self.block_code(b),
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
        };
        for offset in (block.start..block.end).step_by(4) {
            self.at.offset = offset;
            let mark = self.taken.len();
            if let Some(&(from, len)) = self.plan.frame_checks.get(&offset)
                && !self.omit.permissions
            {
                self.frame_check(from, len);
            }
            let decoded = self.decoded();
            emit(self, &decoded.instruction);
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
        match self.label_for(to) {
            Some(label) => {
                if let Label::Loop(_) = label
                    && !self.omit.stop
                {
                    let start = self.function.blocks[to].start;
                    let live = self.plan.live_in[to];
                    self.stop_check(Pc::Offset(start), live, 0);
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
        self.write_back(false);
        self.arguments();
        match self.index.get(&target) {
            Some(&f) => {
                self.sink().return_call(f);
            }
            None => {
                address(&mut self.sink(), target);
                self.sink().return_call(F_DISPATCH);
            }
        }
    }

    fn arguments(&mut self) {
        let mut s = self.sink();
        for i in 0..PASSED {
            s.local_get(i);
        }
        s.local_get(L_RETURN);
    }

    fn push_pc(&mut self, pc: Pc) {
        match pc {
            Pc::Offset(offset) => address(&mut self.sink(), offset),
            Pc::Local(local) => {
                self.sink().local_get(local);
            }
        }
    }

    /// Copy every register whose global may be stale into its global, the flags too, and the passed
    /// registers when asked.
    fn write_back(&mut self, passed: bool) {
        for bit in self.at.dirty.bits() {
            let local = self.register(bit);
            self.sink().local_get(local).global_set(global_of(bit));
        }
        for flag in Flag::ALL {
            if self.at.flags == 1 << FLAGS_IN_GLOBALS {
                break;
            }
            self.push_flag(flag);
            self.sink().global_set(g_flag(flag));
        }
        if passed {
            save_passed(&mut self.sink());
        }
    }

    /// Write back everything, with `pc` set.
    fn save(&mut self, pc: Pc) {
        self.write_back(true);
        self.push_pc(pc);
        self.sink().global_set(G_PC);
    }

    /// Call the stop hook if the flag is raised. If the thread carries on, nothing has changed, so registers
    /// are loaded back from the globals just written rather than kept across the call: `live` (the tracked
    /// registers needed after) and the passed ones from `passed_from` up.
    fn stop_check(&mut self, pc: Pc, live: Registers, passed_from: u32) {
        self.sink()
            .global_get(G_THREAD_BLOCK)
            .i32_load(memarg(
                MEMORY_STATE,
                wcontainers_abi::STOP_FLAG_OFFSET as u64,
                2,
            ))
            .if_(BlockType::Empty);
        self.save(pc);
        self.sink()
            .call(F_STOP)
            .if_(BlockType::Empty)
            .i32_const(UNWIND_LEAVE)
            .throw(0)
            .end();
        self.reload(live, passed_from);
        self.sink().end();
    }

    /// Load registers from their globals: the tracked ones in `live`, and the passed ones from `passed_from`.
    fn reload(&mut self, live: Registers, passed_from: u32) {
        for bit in live.intersect(tracked()).bits() {
            let local = self.register(bit);
            self.sink().global_get(global_of(bit)).local_set(local);
        }
        let mut s = self.sink();
        for i in passed_from..4 {
            s.global_get(g_x(i as u8)).local_set(i);
        }
        s.global_get(G_SP).local_set(L_SP);
    }

    /// Carry on at an instruction from code translated from there, which checks every access exactly.
    fn leave_to(&mut self, offset: u64) {
        self.save(Pc::Offset(offset));
        self.sink().i32_const(UNWIND_GO_TO).throw(0);
    }

    /// The pages of `len` bytes from `from` past the stack pointer allow reading and writing, or this
    /// instruction runs from code that checks each access.
    fn frame_check(&mut self, from: i64, len: u32) {
        let at = self.temporary(ValType::I64);
        let locals = (
            self.temporary(ValType::I64),
            self.temporary(ValType::I32),
            self.temporary(ValType::I64),
        );
        for chunk in (0..len).step_by(4096) {
            let piece = (len - chunk).min(4096);
            self.sink()
                .local_get(L_SP)
                .i64_const(from + i64::from(chunk))
                .i64_add()
                .local_set(at);
            let mut s = self.sink();
            permission_fast(&mut s, at, PAGE_READ | PAGE_WRITE, locals.0);
            permission_exact(&mut s, at, piece, PAGE_READ | PAGE_WRITE, locals);
            let offset = self.at.offset;
            self.leave_to(offset);
            self.sink().end().end();
        }
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
    fn push_flag(&mut self, flag: Flag) {
        let states = self.at.flags;
        if states.count_ones() == 1 {
            self.push_flag_from(states.trailing_zeros(), flag);
            return;
        }
        // Known only at run time: choose by the state local.
        let ids: Vec<u32> = (0..32).filter(|i| states >> i & 1 == 1).collect();
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

    fn push_flag_from(&mut self, state: u32, flag: Flag) {
        let i = Flag::ALL.iter().position(|f| *f == flag).unwrap();
        match state {
            FLAGS_IN_LOCALS => {
                let local = self.flag_locals[i];
                self.sink().local_get(local);
            }
            FLAGS_IN_GLOBALS => {
                self.sink().global_get(g_flag(flag));
            }
            shape => {
                let flags = self.shape_flags(shape as usize - 2);
                flag_value(&mut self.sink(), flags, flag);
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
        if present.is_empty() {
            self.unrecognised_jump(target);
            return;
        }
        let lo = present.iter().map(|c| c.0).min().unwrap();
        let hi = present.iter().map(|c| c.0).max().unwrap();
        let slots = (hi - lo) / 4 + 1;
        if !self.omit.stop
            && present
                .iter()
                .any(|&(_, b)| self.plan.rank[b] <= self.plan.rank[self.at.block])
        {
            let live = present
                .iter()
                .fold(Registers::NONE, |l, &(_, b)| l.union(self.plan.live_in[b]));
            self.stop_check(Pc::Local(target), live, 0);
        }
        let index = self.temporary(ValType::I64);
        self.sink().block(BlockType::Empty);
        self.labels.push(Label::Other);
        let mut labels = Vec::new();
        for k in 0..slots {
            let label = match present.iter().find(|c| c.0 == lo + 4 * k) {
                Some(&(_, b)) => self.depth(self.label_for(b).unwrap()),
                None => 0,
            };
            labels.push(label);
        }
        // A target that is not a multiple of four past the lowest case turns into a huge index.
        self.sink()
            .local_get(target)
            .global_get(G_LOAD_ADDRESS)
            .i64_sub()
            .i64_const(lo as i64)
            .i64_sub()
            .i64_const(2)
            .i64_rotr()
            .local_tee(index)
            .i32_wrap_i64()
            .i32_const(slots as i32)
            .local_get(index)
            .i64_const(slots as i64)
            .i64_lt_u()
            .select()
            .br_table(labels, 0)
            .end();
        self.labels.pop();
        self.unrecognised_jump(target);
    }

    /// Leave the piece through the lookup, counted.
    fn unrecognised_jump(&mut self, target: u32) {
        self.write_back(false);
        self.sink()
            .i32_const(Reason::JumpThroughRegisterNotRecognised as i32)
            .local_get(target)
            .call(F_WARN);
        self.arguments();
        self.sink().local_get(target).return_call(F_DISPATCH);
    }
}

/// The global of a tracked register bit.
fn global_of(bit: u32) -> u32 {
    match bit {
        0..31 => g_x(bit as u8),
        31 => G_SP,
        _ => g_v((bit - 64) as u8),
    }
}

impl Emitter for FullEmitter<'_> {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.body)
    }

    fn local(&mut self, ty: ValType) -> u32 {
        self.temporary(ty)
    }

    fn get(&mut self, r: Reg) {
        match Registers::bit(r) {
            Some(bit) => {
                let local = self.register(bit);
                self.sink().local_get(local);
            }
            None => {
                self.sink().i64_const(0);
            }
        }
    }

    fn set(&mut self, r: Reg) {
        match Registers::bit(r) {
            Some(bit) => {
                let local = self.register(bit);
                self.sink().local_set(local);
                self.at.dirty = self.at.dirty.union(Registers::of(r).intersect(tracked()));
            }
            None => {
                self.sink().drop();
            }
        }
    }

    fn this_address(&mut self) {
        let offset = self.at.offset;
        address(&mut self.sink(), offset);
    }

    fn set_flags(&mut self, flags: Flags) {
        let lazy = self.decoded().effects.flags.is_some();
        match Shape::of(flags).filter(|_| lazy) {
            Some(shape) => {
                let id = self.plan.shapes.iter().position(|s| *s == shape).unwrap();
                let canonical = self.canonical[id];
                let copy = |s: &mut InstructionSink<'_>, from: u32, to: u32| {
                    s.local_get(from).local_set(to);
                };
                let mut s = InstructionSink::new(&mut self.body);
                match (flags, canonical) {
                    (
                        Flags::Add {
                            a,
                            b,
                            carry,
                            subtrahend,
                            ..
                        },
                        Flags::Add {
                            a: ca,
                            b: cb,
                            carry: cc,
                            subtrahend: cm,
                            ..
                        },
                    ) => {
                        copy(&mut s, a, ca);
                        match (subtrahend, cm) {
                            (Some(m), Some(to)) => copy(&mut s, m, to),
                            _ => copy(&mut s, b, cb),
                        }
                        if let (Carry::Local(c), Carry::Local(to)) = (carry, cc) {
                            copy(&mut s, c, to);
                        }
                    }
                    (Flags::Logic { result, .. }, Flags::Logic { result: cr, .. }) => {
                        copy(&mut s, result, cr);
                    }
                    _ => unreachable!("a shape's locals match its flags"),
                }
                self.set_flag_state(2 + id as u32);
            }
            None => {
                for (i, flag) in Flag::ALL.into_iter().enumerate() {
                    let local = self.flag_locals[i];
                    let mut s = self.sink();
                    flag_value(&mut s, flags, flag);
                    s.local_set(local);
                }
                self.set_flag_state(FLAGS_IN_LOCALS);
            }
        }
    }

    fn flag(&mut self, flag: Flag) {
        self.push_flag(flag);
    }

    fn condition(&mut self, cond: Cond) {
        let states = self.at.flags;
        if states.count_ones() == 1 && states.trailing_zeros() >= 2 {
            let flags = self.shape_flags(states.trailing_zeros() as usize - 2);
            if condition_direct(&mut self.sink(), flags, cond) {
                return;
            }
        }
        condition_from_flags(self, cond);
    }

    fn get_vector(&mut self, n: u8) {
        let local = self.register(Registers::vector_bit(n));
        self.sink().local_get(local);
    }

    fn set_vector(&mut self, n: u8) {
        let bit = Registers::vector_bit(n);
        let local = self.register(bit);
        self.sink().local_set(local);
        self.at.dirty = self.at.dirty.union(Registers(1 << bit));
    }

    fn get_thread_pointer(&mut self) {
        self.sink().global_get(G_TPIDR);
    }

    fn stack_aligned(&mut self) -> bool {
        self.at.aligned
    }

    fn set_thread_pointer(&mut self) {
        self.sink().global_set(G_TPIDR);
    }

    fn check(&mut self, address: u32, bytes: u32, access: Access) {
        if self.omit.permissions {
            return;
        }
        let this = match access {
            Access::Read => PAGE_READ,
            Access::Write => PAGE_WRITE,
        };
        let plan = self.plan.checks[&self.at.offset];
        if let CheckPlan::Skip = plan {
            return;
        }
        let locals = (
            self.temporary(ValType::I64),
            self.temporary(ValType::I32),
            self.temporary(ValType::I64),
        );
        let offset = self.at.offset;
        match plan {
            CheckPlan::Skip => {}
            CheckPlan::Exact => {
                // An address the table allows has a clear top byte, so only one it does not is untagged and
                // tried again.
                let mut s = self.sink();
                permission_fast(&mut s, address, this, locals.0);
                untag(&mut s, address);
                s.local_set(address);
                permission_exact(&mut s, address, bytes, this, locals);
                self.save(Pc::Offset(offset));
                self.sink()
                    .local_get(locals.2)
                    .i32_const(i32::from(access == Access::Write))
                    .call(F_MEMORY_FAULT)
                    .i32_const(UNWIND_LEAVE)
                    .throw(0)
                    .end()
                    .end();
            }
            CheckPlan::Group { from, len } => {
                let start = self.temporary(ValType::I64);
                self.sink()
                    .local_get(address)
                    .i64_const(from)
                    .i64_add()
                    .local_set(start);
                let mut s = self.sink();
                permission_fast(&mut s, start, this, locals.0);
                permission_exact(&mut s, start, len, this, locals);
                self.leave_to(offset);
                self.sink().end().end();
            }
        }
    }

    fn jump(&mut self, target: Target) {
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
        self.sink().if_(BlockType::Empty);
        self.labels.push(Label::Other);
        let saved = self.at;
        self.jump(target);
        self.at = saved;
        self.sink().end();
        self.labels.pop();
    }

    fn call(&mut self, target: Target) {
        let offset = self.at.offset;
        self.write_back(false);
        let mut s = self.sink();
        for i in 0..PASSED {
            s.local_get(i);
        }
        address(&mut s, offset + 4);
        match target {
            Target::Relative(by) => {
                let to = offset.wrapping_add_signed(by);
                match self.index.get(&to) {
                    Some(&f) => {
                        self.sink().call(f);
                    }
                    None => {
                        address(&mut self.sink(), to);
                        self.sink().call(F_DISPATCH);
                    }
                }
            }
            Target::Address(local) => {
                self.sink().local_get(local).call(F_DISPATCH);
            }
        }
        let returned_sp = self.temporary(ValType::I64);
        let mut s = self.sink();
        s.local_set(returned_sp);
        for i in (0..4).rev() {
            s.local_set(i);
        }
        self.at.dirty = Registers::NONE;
        self.set_flag_state(FLAGS_IN_GLOBALS);
        // What is known about the stack pointer (its frame, its alignment) holds after the call only if the
        // callee gave it back unchanged; otherwise carry on from code that knows nothing.
        let kept =
            self.at.aligned || (self.plan.frame_across.contains(&offset) && !self.omit.permissions);
        if kept {
            self.sink()
                .local_get(returned_sp)
                .local_get(L_SP)
                .i64_ne()
                .local_get(returned_sp)
                .local_set(L_SP)
                .if_(BlockType::Empty);
            self.leave_to(offset + 4);
            self.sink().end();
        } else {
            self.sink().local_get(returned_sp).local_set(L_SP);
        }
        let reload = self.plan.live_after[&offset];
        for bit in reload.bits() {
            let local = self.register(bit);
            self.sink().global_get(global_of(bit)).local_set(local);
        }
    }

    fn ret(&mut self, target: u32) {
        self.write_back(false);
        if self.omit.returns {
            let mut s = self.sink();
            for i in 0..PASSED {
                s.local_get(i);
            }
            s.return_();
            return;
        }
        let mut s = self.sink();
        s.local_get(target)
            .local_get(L_RETURN)
            .i64_eq()
            .if_(BlockType::Empty);
        for i in 0..PASSED {
            s.local_get(i);
        }
        s.return_().end();
        // Not where the caller expects: go to the address from the bottom loop.
        save_passed(&mut s);
        s.local_get(target)
            .global_set(G_PC)
            .i32_const(UNWIND_GO_TO)
            .throw(0);
    }

    fn syscall(&mut self) {
        let next = self.at.offset + 4;
        self.save(Pc::Offset(next));
        self.get(Reg::X(8));
        for i in 0..6 {
            self.get(Reg::X(i));
        }
        self.sink().call(F_SYSCALL).local_set(0);
        self.at.dirty = Registers::NONE;
        // Nothing but x0 has changed, and everything was just written back.
        let live = self.plan.live_after[&self.at.offset];
        self.reload(live, 1);
        // The kernel raises the flag during the call when the thread must not carry on.
        self.stop_check(Pc::Offset(next), live, 0);
    }

    fn trap(&mut self, trap: Trap) {
        let offset = self.at.offset;
        self.save(Pc::Offset(offset));
        let word = i64::from(self.decoded().word);
        let mut s = self.sink();
        match trap {
            Trap::Undefined => s.i32_const(TrapKind::Undefined as i32).i64_const(word),
            Trap::Unsupported => s.i32_const(TrapKind::Unsupported as i32).i64_const(word),
            Trap::Breakpoint(comment) => s
                .i32_const(TrapKind::Breakpoint as i32)
                .i64_const(comment.into()),
            Trap::Alignment(address) => s.i32_const(TrapKind::Alignment as i32).local_get(address),
        };
        s.call(F_TRAP).i32_const(UNWIND_LEAVE).throw(0);
    }

    fn next(&mut self) {
        let next = self.at.offset + 4;
        if next < self.function.blocks[self.at.block].end {
            return;
        }
        self.goto(next);
    }
}
