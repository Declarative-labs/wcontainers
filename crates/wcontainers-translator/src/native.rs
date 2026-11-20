//! The native output (docs/native.md section 3): host code of the guest's own architecture, each guest
//! instruction carried over by its route, with the count's adds and the position map. No WebAssembly is made.

use std::collections::{BTreeMap, BTreeSet};

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
/// through an exit and starts a piece of its own, unless the branch goes back before the start and the code
/// after it comes back into the piece (docs/native.md section 3.2). Instructions are laid out in address order;
/// a branch to an instruction the piece holds goes straight to it, any other leaves through an exit, and a
/// conditional branch's taken side is placed out of line. `None` when no instruction starts there.
///
/// The count's adds are written only when `exact_replay` is set: the process asked for exact replay
/// (`ReplayDeterminism::Exact`, from which the caller derives it and which keys its cache of translations).
/// Each straight-line part, entered only at its start and ended by a branch or where the code goes elsewhere,
/// has its add placed by `count_placement` (section 4), or counted ahead by the one part it is entered from
/// (`counted_ahead`), and every exit leaves the count the guest's, so it is exact wherever the piece is entered
/// or left. The position map keeps the count ahead by the events still to come from the add to each event's
/// commit point.
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
    // The parts each part goes on to inside the piece: the next by falling through, a branch's target.
    let successors: Vec<BTreeSet<usize>> = (0..starts.len())
        .map(|p| {
            let end = starts.get(p + 1).copied().unwrap_or(held.len());
            let last = &held[end - 1];
            let through = (last.goes_on
                && held.get(end).is_some_and(|n| n.at == last.at + last.length))
            .then_some(p + 1);
            let branch = last
                .target
                .and_then(|t| index_at.get(&t))
                .map(|&k| part_of(k));
            through.into_iter().chain(branch).collect()
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
    let (amounts, leads) = counted_ahead(&successors, &own);
    let mut adds = vec![None; held.len()];
    for (p, &amount) in amounts.iter().enumerate() {
        if amount > 0 {
            adds[starts[p] + placed[p].map_or(0, |(k, _)| k)] = Some(amount);
        }
    }
    let mut out = HostCode::new();
    // The host offset of each part's start, by guest address.
    let mut entries = BTreeMap::new();
    // The current straight-line part's instructions so far, with their guest addresses.
    let mut part: Vec<(u64, G::Instruction)> = Vec::new();
    for (k, h) in held.iter().enumerate() {
        let here = guest(h.at);
        out.start(here, events[k], G::stopping(&h.instruction, here));
        if part_starts[k] {
            entries.insert(here, out.offset());
            out.enter_ahead(leads[part_of(k)]);
            part.clear();
        }
        if let Some(total) = adds[k] {
            G::count(&mut out, total);
        }
        let known = matches!(G::flow(&h.instruction).kind, FlowKind::JumpThrough(_))
            && G::carry_known_jump(&part, &h.instruction, here, &mut out);
        if !known {
            G::carry(&h.instruction, &code[h.at..h.at + h.length], here, &mut out);
        }
        part.push((here, h.instruction));
        let next = h.at + h.length;
        if h.goes_on && held.get(k + 1).is_none_or(|n| n.at != next) {
            G::exit(&mut out, guest(next));
        }
    }
    out.finish();
    for exit in std::mem::take(&mut out.exits) {
        let within = entries.get(&exit.target).copied();
        if within.is_some_and(|to| G::link(&mut out, &exit, to)) {
            // The word linked over no longer builds an address.
            out.places.retain(|place| place.start != exit.start);
        } else if let Some(exit) = out.for_loader(exit) {
            out.exits.push(exit);
        }
    }
    Some(out)
}

/// Each part's add and how far the count is ahead where the part is entered, given the parts each goes on to
/// inside the piece (`successors`) and its own events (`own`). A part entered only from one part that goes on
/// only to it has its events counted by that part's add, so a loop of such parts adds once a turn; the first
/// part is entered from outside, where the count is the guest's. Where a part's code leaves the piece with the
/// count still ahead, its exit brings it back.
fn counted_ahead(successors: &[BTreeSet<usize>], own: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let parts = own.len();
    let mut predecessors = vec![BTreeSet::new(); parts];
    for (p, next) in successors.iter().enumerate() {
        for &q in next {
            predecessors[q].insert(p);
        }
    }
    // The part whose add counts each part's events in its place.
    let counted_by = |q: usize| -> Option<usize> {
        let &p = predecessors[q].first()?;
        (q != 0 && p != q && predecessors[q].len() == 1 && successors[p].len() == 1).then_some(p)
    };
    let mut amounts = vec![0; parts];
    for (q, &events) in own.iter().enumerate() {
        let mut home = q;
        for _ in 0..parts {
            match counted_by(home) {
                Some(p) => home = p,
                None => break,
            }
        }
        amounts[home] += events;
    }
    let mut leads: Vec<Option<u32>> = vec![None; parts];
    for q in 0..parts {
        // The chain of parts counted by the one before, from q back to the first that is not.
        let mut chain = vec![q];
        while let Some(p) = counted_by(*chain.last().expect("a part")) {
            if leads[p].is_some() || chain.len() > parts {
                break;
            }
            chain.push(p);
        }
        for &part in chain.iter().rev() {
            let lead = match counted_by(part) {
                None => 0,
                Some(p) => {
                    let before = leads[p].expect("the part before has its lead");
                    before + amounts[p] - own[p]
                }
            };
            leads[part] = Some(lead);
        }
    }
    (
        amounts,
        leads
            .into_iter()
            .map(|l| l.expect("every part has its lead"))
            .collect(),
    )
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
    // The code after a branch back before the start is held when it comes back into the piece: the rest of a
    // turn of the piece's loop. Code that only leaves (a return after a loop's last turn) starts its own piece.
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
/// direct branch targets inside `code`, but not a call's next instruction, nor the next instruction of a branch
/// back into the walk's code (where a loop is left); the next instruction of a branch back before the start
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
            match (backward, target) {
                (false, _) => work.push(at + length),
                (true, None) => later.push(at + length),
                (true, Some(_)) => {}
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
    use std::collections::BTreeSet;

    use super::counted_ahead;

    fn successors(parts: &[&[usize]]) -> Vec<BTreeSet<usize>> {
        parts
            .iter()
            .map(|next| next.iter().copied().collect())
            .collect()
    }

    /// A loop of two parts entered at the first adds once a turn, the second part entered with its event ahead.
    #[test]
    fn a_loop_of_two_parts_adds_once() {
        assert_eq!(
            counted_ahead(&successors(&[&[1], &[0]]), &[1, 1]),
            (vec![2, 0], vec![0, 1])
        );
    }

    /// A chain of parts each entered only from the one before is counted by the first.
    #[test]
    fn a_chain_is_counted_by_its_first_part() {
        assert_eq!(
            counted_ahead(&successors(&[&[1], &[2], &[]]), &[1, 1, 1]),
            (vec![3, 0, 0], vec![0, 2, 1])
        );
    }

    /// A part entered from two parts, or from a part that goes on to two, keeps its own add.
    #[test]
    fn a_join_or_a_fork_keeps_its_adds() {
        assert_eq!(
            counted_ahead(&successors(&[&[1, 2], &[2], &[]]), &[0, 1, 1]),
            (vec![0, 1, 1], vec![0, 0, 0])
        );
    }
}
