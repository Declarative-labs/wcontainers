//! The native output (docs/native.md section 3): host code of the guest's own architecture, each guest
//! instruction carried over by its route, with the count's adds and the position map. No WebAssembly is made.

use std::collections::{BTreeMap, BTreeSet};

use wcontainers_guest_cpu::position::DirectExit;
use wcontainers_guest_cpu::{FlowKind, HostCode, Native, Route, count_placement, counted};

use crate::LookAhead;

/// One instruction a piece holds.
struct Held<I> {
    /// Its offset from the piece's start.
    at: usize,
    instruction: I,
    length: usize,
    route: Route,
    /// Whether the code goes on at the next instruction after it.
    goes_on: bool,
    /// The offset of the direct branch target it may go to inside the code, a call's excepted.
    target: Option<usize>,
}

/// The piece at guest `address`, whose bytes start `code`: what the code reaches from there in at most one
/// instruction at look-ahead none, at most `n` at `Bounded(n)`, and up to the end of `code` at full, going on
/// after every instruction that falls through (a call's return comes back through its entry slot) and to every
/// direct branch's target inside `code`. A backward branch's next instruction, where a loop is left, is reached
/// through an exit and starts a piece of its own, unless the code after it comes back into the piece (docs/
/// native.md section 3.2). Instructions are laid out in address order;
/// a branch to an instruction the piece holds goes straight to it, any other leaves through an exit, and a
/// conditional branch's taken side is placed out of line. `None` when no instruction starts there.
///
/// The count's adds are written only when `exact_replay` is set: the process asked for exact replay
/// (`ReplayDeterminism::Exact`, from which the caller derives it and which keys its cache of translations).
/// Each straight-line part, entered only at its start and ended by a branch or where the code goes elsewhere,
/// has its add placed by `count_placement` (section 4), or counted ahead by the parts it is entered from
/// (`counted_ahead`), and every exit leaves the count the guest's, so it is exact wherever the piece is entered
/// or left. The position map keeps the count ahead by the events still to come from the add to each event's
/// commit point.
///
/// A part that loops to its own start begins with the test of what one test can settle for all its turns
/// (`Native::loop_invariant`); where it holds, the code goes to a copy of the part made with that settled, which
/// loops to itself, and where it does not, it goes on into the part as made without it.
pub fn translate_native<G: Native>(
    code: &[u8],
    address: u64,
    look_ahead: LookAhead,
    exact_replay: bool,
) -> Option<HostCode> {
    let limit = match look_ahead {
        LookAhead::None => 1,
        LookAhead::Bounded(n) => usize::try_from(n).unwrap_or(usize::MAX).max(1),
        LookAhead::Full => usize::MAX,
    };
    let held = reach::<G>(code, address, limit);
    if held.is_empty() {
        return None;
    }
    let targets: BTreeSet<usize> = held.iter().filter_map(|h| h.target).collect();
    // Where each straight-line part starts: the first instruction, a branch's target, and wherever the code
    // before does not go straight on to it.
    let part_starts: Vec<bool> = (0..held.len())
        .map(|k| {
            k == 0 || targets.contains(&held[k].at) || {
                let before = &held[k - 1];
                !before.goes_on
                    || before.route == Route::Transfer
                    || before.at + before.length != held[k].at
            }
        })
        .collect();
    let guest = |at: usize| address.wrapping_add(at as u64);
    let events: Vec<bool> = held
        .iter()
        .map(|h| exact_replay && counted(guest(h.at), &G::flow(&h.instruction)))
        .collect();
    let starts: Vec<usize> = (0..held.len()).filter(|&k| part_starts[k]).collect();
    let part_of = |k: usize| starts.partition_point(|&s| s <= k) - 1;
    let index_at: BTreeMap<usize, usize> =
        held.iter().enumerate().map(|(k, h)| (h.at, k)).collect();
    let end_of = |p: usize| starts.get(p + 1).copied().unwrap_or(held.len());
    // The parts each part goes on to inside the piece: the next by falling through, a branch's target.
    let ways: Vec<(Option<usize>, Option<usize>)> = (0..starts.len())
        .map(|p| {
            let last = &held[end_of(p) - 1];
            let through = (last.goes_on
                && held
                    .get(end_of(p))
                    .is_some_and(|n| n.at == last.at + last.length))
            .then_some(p + 1);
            let branch = last
                .target
                .and_then(|t| index_at.get(&t))
                .map(|&k| part_of(k));
            (through, branch)
        })
        .collect();
    let successors: Vec<BTreeSet<usize>> = ways
        .iter()
        .map(|&(through, branch)| through.into_iter().chain(branch).collect())
        .collect();
    // The loops the piece holds, as the offsets from a branch back's target to the branch.
    let loops: Vec<(usize, usize)> = held
        .iter()
        .filter_map(|h| {
            h.target
                .filter(|&t| t <= h.at && index_at.contains_key(&t))
                .map(|t| (t, h.at))
        })
        .collect();
    // The way each part most likely goes on inside the piece: the one that stays in the innermost loop holding
    // the part, else a branch back, else falling through.
    let likely: Vec<Option<usize>> = (0..starts.len())
        .map(|p| match ways[p] {
            (Some(through), Some(branch)) if through != branch => {
                let (first, last) = (held[starts[p]].at, held[end_of(p) - 1].at);
                let innermost = loops
                    .iter()
                    .filter(|&&(head, end)| head <= first && last <= end)
                    .min_by_key(|&&(head, end)| end - head);
                let stays = |q: usize| {
                    innermost.is_some_and(|&(head, end)| (head..=end).contains(&held[starts[q]].at))
                };
                let back = held[starts[branch]].at <= last;
                Some(if stays(branch) == stays(through) {
                    if back { branch } else { through }
                } else if stays(branch) {
                    branch
                } else {
                    through
                })
            }
            (through, branch) => through.or(branch),
        })
        .collect();
    let placed: Vec<Option<(usize, u32)>> = (0..starts.len())
        .map(|p| {
            let end = starts.get(p + 1).copied().unwrap_or(held.len());
            count_placement(&events[starts[p]..end])
        })
        .collect();
    let own: Vec<u32> = placed
        .iter()
        .map(|p| p.map_or(0, |(_, total)| total))
        .collect();
    let throughs: Vec<Option<usize>> = ways.iter().map(|&(through, _)| through).collect();
    let (amounts, leads) = counted_ahead(&successors, &likely, &throughs, &own);
    let mut adds = vec![None; held.len()];
    for (p, &amount) in amounts.iter().enumerate() {
        if amount > 0 {
            adds[starts[p] + placed[p].map_or(0, |(k, _)| k)] = Some(amount);
        }
    }
    // What one test before each part that loops to its own start settles for every turn of it.
    let invariants: Vec<G::LoopInvariant> = (0..starts.len())
        .map(|p| {
            if !successors[p].contains(&p) {
                return G::LoopInvariant::default();
            }
            let end = starts.get(p + 1).copied().unwrap_or(held.len());
            let part: Vec<(u64, G::Instruction)> = held[starts[p]..end]
                .iter()
                .map(|h| (guest(h.at), h.instruction))
                .collect();
            G::loop_invariant(&part)
        })
        .collect();
    let settled_nothing = G::LoopInvariant::default();
    // One instruction's add and code, after the current straight-line part's instructions so far (`part`).
    let carry_one = |out: &mut HostCode,
                     k: usize,
                     part: &mut Vec<(u64, G::Instruction)>,
                     invariant: &G::LoopInvariant| {
        let h = &held[k];
        let here = guest(h.at);
        if let Some(total) = adds[k] {
            G::count(out, total);
        }
        G::carry(
            part,
            &h.instruction,
            &code[h.at..h.at + h.length],
            here,
            invariant,
            out,
        );
        part.push((here, h.instruction));
    };
    let mut out = HostCode::new();
    // The host offset of each part's start and the count's lead there, by guest address.
    let mut entries = BTreeMap::new();
    // The current straight-line part's instructions so far, with their guest addresses.
    let mut part: Vec<(u64, G::Instruction)> = Vec::new();
    // Each looping part whose test was written, with the offset of the test's branch to the part's copy.
    let mut tested = Vec::new();
    for (k, h) in held.iter().enumerate() {
        let here = guest(h.at);
        out.start(here, events[k], G::stopping(&h.instruction, here));
        if part_starts[k] {
            let p = part_of(k);
            entries.insert(here, (out.offset(), leads[p]));
            out.enter_ahead(leads[p]);
            part.clear();
            if let Some(branch) = G::test_loop_invariant(&mut out, &invariants[p]) {
                tested.push((p, branch));
            }
        }
        carry_one(&mut out, k, &mut part, &settled_nothing);
        let next = h.at + h.length;
        if h.goes_on && held.get(k + 1).is_none_or(|n| n.at != next) {
            G::exit(&mut out, guest(next));
        }
    }
    out.finish();
    // Each tested part again, made with what its test settled and looping to itself: the exits it wrote, its
    // start and its guest address.
    let mut copies = Vec::new();
    for (p, branch) in tested {
        let (first, end) = (starts[p], starts.get(p + 1).copied().unwrap_or(held.len()));
        let head = guest(held[first].at);
        let start = out.offset();
        let written = out.exits.len();
        part.clear();
        for k in first..end {
            let here = guest(held[k].at);
            out.start(here, events[k], G::stopping(&held[k].instruction, here));
            if k == first {
                out.enter_ahead(leads[p]);
            }
            carry_one(&mut out, k, &mut part, &invariants[p]);
        }
        let last = &held[end - 1];
        if last.goes_on {
            G::exit(&mut out, guest(last.at + last.length));
        }
        out.finish();
        let test = DirectExit {
            start: u32::try_from(branch).expect("host code fits in u32 offsets"),
            target: head,
            retarget: Some(0),
        };
        assert!(
            G::link(&mut out, &test, start),
            "a piece's own copy is near"
        );
        copies.push((written..out.exits.len(), start, head, leads[p]));
    }
    for (n, exit) in std::mem::take(&mut out.exits).into_iter().enumerate() {
        // A copy's branch back to its start stays in the copy.
        let within = copies
            .iter()
            .find(|(written, _, head, _)| written.contains(&n) && exit.target == *head)
            .map(|&(_, start, _, lead)| (start, lead))
            .or_else(|| entries.get(&exit.target).copied());
        // A part entered with the guest's count is linked to past the exit's settling, as a loader links.
        let linked = within.and_then(|(to, lead)| {
            let from = if lead == 0 {
                out.for_loader(exit)?
            } else {
                exit
            };
            G::link(&mut out, &from, to).then_some(from)
        });
        if let Some(from) = linked {
            // The word linked over no longer builds an address.
            out.places.retain(|place| place.start != from.start);
        } else if let Some(exit) = out.for_loader(exit) {
            out.exits.push(exit);
        }
    }
    Some(out)
}

/// Each part's add and how far the count is ahead where the part is entered, given the parts each goes on to
/// inside the piece (`successors`), the one it most likely goes on to (`likely`), the one it falls through to
/// (`throughs`) and its own events (`own`). A part entered only from parts that most likely go on to it, each
/// going on only to it or having an add of its own to join, has its events counted by their adds, so a loop of
/// such parts adds once a turn; the first part is entered from outside, where the count is the guest's. A part's
/// other way on, and its code leaving the piece, with the count still ahead, goes through code that brings the
/// count back, so no such way is a part's fall-through.
fn counted_ahead(
    successors: &[BTreeSet<usize>],
    likely: &[Option<usize>],
    throughs: &[Option<usize>],
    own: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    let parts = own.len();
    let mut predecessors = vec![BTreeSet::new(); parts];
    for (p, next) in successors.iter().enumerate() {
        for &q in next {
            predecessors[q].insert(p);
        }
    }
    let mut ahead: Vec<bool> = (0..parts)
        .map(|q| {
            q != 0
                && !predecessors[q].is_empty()
                && predecessors[q].iter().all(|&p| {
                    p != q && likely[p] == Some(q) && (successors[p].len() == 1 || own[p] > 0)
                })
        })
        .collect();
    let next_ahead = |ahead: &[bool], p: usize| likely[p].filter(|&s| ahead[s]);
    let leads_of = |ahead: &[bool]| -> Vec<u32> {
        (0..parts)
            .map(|q| {
                let (mut lead, mut at) = (0, Some(q));
                while let Some(x) = at.filter(|&x| ahead[x]) {
                    lead += own[x];
                    at = next_ahead(ahead, x);
                }
                lead
            })
            .collect()
    };
    let leads = loop {
        // A loop of parts each counted ahead would add nothing a turn: its first part keeps its own add.
        for start in 0..parts {
            let mut seen = vec![start];
            while let Some(s) = next_ahead(&ahead, *seen.last().expect("a part")) {
                if let Some(k) = seen.iter().position(|&x| x == s) {
                    let first = *seen[k..].iter().min().expect("a loop has parts");
                    ahead[first] = false;
                    break;
                }
                seen.push(s);
            }
        }
        let leads = leads_of(&ahead);
        let falls_ahead = (0..parts).find_map(|p| {
            let s = likely[p]?;
            (throughs[p].is_some_and(|t| t != s) && leads[s] > 0).then_some(s)
        });
        match falls_ahead {
            Some(s) => ahead[s] = false,
            None => break leads,
        }
    };
    let amounts = (0..parts)
        .map(|q| {
            if ahead[q] {
                0
            } else {
                own[q] + likely[q].map_or(0, |s| leads[s])
            }
        })
        .collect();
    (amounts, leads)
}

/// The instructions of the piece at `address` whose bytes start `code`, in address order: at most `limit`, seen
/// in the order the code runs, its next instruction first.
fn reach<G: Native>(code: &[u8], address: u64, limit: usize) -> Vec<Held<G::Instruction>> {
    let mut held = BTreeMap::new();
    let mut later = Vec::new();
    walk::<G>(
        code,
        address,
        0,
        limit,
        &BTreeMap::new(),
        &mut held,
        &mut later,
    );
    // The code after a branch back is held when it comes back into the piece: the rest of a turn of a loop the
    // piece holds. Code that only leaves (a return after a loop's last turn) starts its own piece.
    while let Some(next) = later.pop() {
        let (mut more, mut further) = (BTreeMap::new(), Vec::new());
        walk::<G>(
            code,
            address,
            next,
            limit - held.len(),
            &held,
            &mut more,
            &mut further,
        );
        let comes_back = more
            .values()
            .any(|h: &Held<G::Instruction>| h.target.is_some_and(|t| held.contains_key(&t)));
        if comes_back {
            held.append(&mut more);
            later.append(&mut further);
        }
    }
    held.into_values().collect()
}

/// Walks from `from` into `found`, at most `limit` instructions, past what `known` holds: fall-through and
/// direct branch targets inside `code`, but not a call's next instruction; the next instruction of a branch back
/// goes to `later`.
fn walk<G: Native>(
    code: &[u8],
    address: u64,
    from: usize,
    limit: usize,
    known: &BTreeMap<usize, Held<G::Instruction>>,
    found: &mut BTreeMap<usize, Held<G::Instruction>>,
    later: &mut Vec<usize>,
) {
    let mut work = vec![from];
    while let Some(at) = work.pop() {
        if found.len() == limit || found.contains_key(&at) || known.contains_key(&at) {
            continue;
        }
        let Some((instruction, length)) = code.get(at..).and_then(G::decode) else {
            continue;
        };
        let length = usize::from(length);
        let route = G::route(&instruction);
        let flow = G::flow(&instruction);
        let goes_on = match route {
            Route::Trap => false,
            Route::Transfer => flow.falls_through && matches!(flow.kind, FlowKind::Plain),
            _ => true,
        };
        let target = flow
            .target
            .filter(|_| route == Route::Transfer && matches!(flow.kind, FlowKind::Plain))
            .and_then(|by| i64::try_from(at).ok()?.checked_add(by))
            .and_then(|t| usize::try_from(t).ok())
            .filter(|&t| t < code.len() && address.wrapping_add(t as u64).is_multiple_of(G::ALIGN));
        work.extend(target);
        let backward = route == Route::Transfer && flow.target.is_some_and(|by| by <= 0);
        if goes_on {
            if backward {
                later.push(at + length);
            } else {
                work.push(at + length);
            }
        }
        found.insert(
            at,
            Held {
                at,
                instruction,
                length,
                route,
                goes_on,
                target,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::counted_ahead;

    /// Each part as the parts it goes on to inside the piece, the one it most likely goes on to and the one it
    /// falls through to.
    type Part<'a> = (&'a [usize], Option<usize>, Option<usize>);

    fn ahead(parts: &[Part], own: &[u32]) -> (Vec<u32>, Vec<u32>) {
        let successors: Vec<_> = parts
            .iter()
            .map(|p| p.0.iter().copied().collect())
            .collect();
        let likely: Vec<_> = parts.iter().map(|p| p.1).collect();
        let throughs: Vec<_> = parts.iter().map(|p| p.2).collect();
        counted_ahead(&successors, &likely, &throughs, own)
    }

    /// A loop of two parts entered at the first adds once a turn, the second part entered with its event ahead.
    #[test]
    fn a_loop_of_two_parts_adds_once() {
        assert_eq!(
            ahead(&[(&[1], Some(1), Some(1)), (&[0], Some(0), None)], &[1, 1]),
            (vec![2, 0], vec![0, 1])
        );
    }

    /// A chain of parts each entered only from the one before is counted by the first.
    #[test]
    fn a_chain_is_counted_by_its_first_part() {
        assert_eq!(
            ahead(
                &[
                    (&[1], Some(1), Some(1)),
                    (&[2], Some(2), Some(2)),
                    (&[], None, None)
                ],
                &[1, 1, 1]
            ),
            (vec![3, 0, 0], vec![0, 2, 1])
        );
    }

    /// A loop entered at its second part, which goes back to its first or on to a third that goes back too, with
    /// a way out of the first to the piece's start: the first part adds for the second, once a turn, and its way
    /// out settles.
    #[test]
    fn an_inner_loop_adds_once_a_turn() {
        assert_eq!(
            ahead(
                &[
                    (&[2], Some(2), None),
                    (&[0, 2], Some(2), Some(2)),
                    (&[1, 3], Some(1), Some(3)),
                    (&[1], Some(1), None),
                ],
                &[0, 1, 1, 1]
            ),
            (vec![1, 2, 0, 1], vec![0, 0, 1, 0])
        );
    }

    /// A loop of parts that would each be counted by the one before keeps the add in its first part.
    #[test]
    fn a_loop_counted_ahead_keeps_one_add() {
        assert_eq!(
            ahead(
                &[
                    (&[1], Some(1), Some(1)),
                    (&[2], Some(2), Some(2)),
                    (&[1], Some(1), None)
                ],
                &[0, 1, 1]
            ),
            (vec![0, 2, 0], vec![0, 0, 1])
        );
    }

    /// A part with an add of its own counts the part it most likely goes on to by falling through; its branch
    /// elsewhere settles.
    #[test]
    fn a_fork_counts_its_likely_fall_through() {
        assert_eq!(
            ahead(
                &[
                    (&[1, 2], Some(1), Some(1)),
                    (&[], None, None),
                    (&[], None, None)
                ],
                &[1, 1, 1]
            ),
            (vec![2, 0, 1], vec![0, 1, 0])
        );
    }

    /// A part whose likely way on is a branch keeps the add of its target there, so its fall-through needs none.
    #[test]
    fn a_fork_falling_through_elsewhere_keeps_the_adds() {
        assert_eq!(
            ahead(
                &[
                    (&[1, 2], Some(2), Some(1)),
                    (&[], None, None),
                    (&[], None, None)
                ],
                &[1, 1, 1]
            ),
            (vec![1, 1, 1], vec![0, 0, 0])
        );
    }

    /// A part entered from two parts, or from a fork with no add of its own, keeps its own add.
    #[test]
    fn a_join_or_a_fork_without_an_add_keeps_its_adds() {
        assert_eq!(
            ahead(
                &[
                    (&[1, 2], Some(1), Some(1)),
                    (&[2], Some(2), Some(2)),
                    (&[], None, None)
                ],
                &[0, 1, 1]
            ),
            (vec![0, 1, 1], vec![0, 0, 0])
        );
    }
}
