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
/// through an exit and starts a piece of its own (docs/native.md section 3.2). Instructions are laid out in address order; a branch to an instruction
/// the piece holds goes straight to it, any other leaves through an exit, and a conditional branch's taken side
/// is placed out of line. `None` when no instruction starts there.
///
/// The count's adds are written only when `exact_replay` is set: the process asked for exact replay
/// (`ReplayDeterminism::Exact`, from which the caller derives it and which keys its cache of translations).
/// Each straight-line part, entered only at its start and ended by a branch or where the code goes elsewhere,
/// has its own add, placed by `count_placement` (section 4), so the count is exact wherever the piece is
/// entered or left. The position map keeps the count ahead by the events still to come from the add to each
/// event's commit point.
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
    let mut adds = vec![None; held.len()];
    let mut part = 0;
    for end in 1..=held.len() {
        if end == held.len() || part_starts[end] {
            if let Some((k, total)) = count_placement(&events[part..end]) {
                adds[part + k] = Some(total);
            }
            part = end;
        }
    }
    let mut out = HostCode::new();
    // The host offset of each part's start, by guest address.
    let mut entries = BTreeMap::new();
    for (k, h) in held.iter().enumerate() {
        let here = guest(h.at);
        if part_starts[k] {
            entries.insert(here, out.offset());
        }
        out.start(here, events[k], G::stopping(&h.instruction, here));
        if let Some(total) = adds[k] {
            G::count(&mut out, total);
        }
        G::carry(&h.instruction, &code[h.at..h.at + h.length], here, &mut out);
        let next = h.at + h.length;
        if h.goes_on && held.get(k + 1).is_none_or(|n| n.at != next) {
            G::exit(&mut out, guest(next));
        }
    }
    out.finish();
    for exit in std::mem::take(&mut out.exits) {
        let within = entries.get(&exit.target).copied();
        if !within.is_some_and(|to| G::link(&mut out, &exit, to)) {
            out.exits.push(exit);
        }
    }
    Some(out)
}

/// The instructions of the piece at `address` whose bytes start `code`, in address order: at most `limit`, seen
/// in the order the code runs, its next instruction first.
fn reach<G: Native>(code: &[u8], address: u64, limit: usize) -> Vec<Held<G::Instruction>> {
    let mut held = BTreeMap::new();
    let mut work = vec![0];
    while let Some(at) = work.pop() {
        if held.len() == limit || held.contains_key(&at) {
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
        if goes_on && !backward {
            work.push(at + length);
        }
        held.insert(
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
    held.into_values().collect()
}
