//! The native output (docs/native.md section 3): host code of the guest's own architecture, each guest
//! instruction carried over by its route, with the count's adds and the position map. No WebAssembly is made.

use wcontainers_guest_cpu::{FlowKind, HostCode, Native, Route, count_placement, counted};

use crate::LookAhead;

/// The piece at guest `address`, whose bytes start `code`: the run from there, at most one instruction at
/// look-ahead none, at most `n` at `Bounded(n)`, and up to the end of `code` at full, ended early by a transfer
/// that does not fall through or a trap (whose routes make their own exits). A conditional branch leaves the run
/// for its target through an exit placed out of line, and the run goes on at the next instruction. Each
/// instruction is carried over by its route and the run's end falls through to the exit to the next
/// instruction. `None` when no instruction starts there.
///
/// The count's adds are written only when `exact_replay` is set: the process asked for exact replay
/// (`ReplayDeterminism::Exact`, from which the caller derives it and which keys its cache of translations).
/// Each straight-line part of the run, ended by a conditional branch or by the run's end, has its own add,
/// placed by `count_placement` (section 4), so the count is exact wherever the run is left. The position map
/// keeps the count ahead by the events still to come from the add to each event's commit point.
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
    let mut run = Vec::new();
    let mut at = 0;
    while run.len() < limit {
        let Some((instruction, length)) = code.get(at..).and_then(G::decode) else {
            break;
        };
        let length = usize::from(length);
        let here = address.wrapping_add(at as u64);
        let route = G::route(&instruction);
        let flow = G::flow(&instruction);
        let event = exact_replay && counted(here, &flow);
        let goes_on = match route {
            Route::Trap => false,
            Route::Transfer => flow.falls_through && matches!(flow.kind, FlowKind::Plain),
            _ => true,
        };
        // A conditional branch forward: its fall-through is the likely way on. A backward one is a loop's back
        // edge, whose fall-through leaves the loop.
        let forward = flow.target.is_some_and(|offset| offset > 0);
        run.push((instruction, at..at + length, here, event, route, goes_on));
        at += length;
        if !goes_on || (route == Route::Transfer && !forward) {
            break;
        }
    }
    let ends_itself = !run.last()?.5;
    let after = address.wrapping_add(at as u64);
    // The add before each instruction, by straight-line part: a conditional branch ends one.
    let mut adds = vec![None; run.len()];
    let mut part = 0;
    for end in 1..=run.len() {
        if end == run.len() || run[end - 1].4 == Route::Transfer {
            let events: Vec<bool> = run[part..end].iter().map(|r| r.3).collect();
            if let Some((k, total)) = count_placement(&events) {
                adds[part + k] = Some(total);
            }
            part = end;
        }
    }
    let mut out = HostCode::new();
    for ((instruction, bytes, here, event, ..), add) in run.iter().zip(adds) {
        out.start(*here, *event, G::stopping(instruction, *here));
        if let Some(total) = add {
            G::count(&mut out, total);
        }
        G::carry(instruction, &code[bytes.clone()], *here, &mut out);
    }
    if !ends_itself {
        G::exit(&mut out, after);
    }
    out.finish();
    Some(out)
}
