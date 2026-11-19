//! The native output (docs/native.md section 3): host code of the guest's own architecture, each guest
//! instruction carried over by its route, with the count's adds and the position map. No WebAssembly is made.

use wcontainers_guest_cpu::{HostCode, InsideCall, Native, Route, counted};

use crate::LookAhead;

/// The piece at guest `address`, whose bytes start `code`: the straight-line run from there, at most one
/// instruction at look-ahead none, at most `n` at `Bounded(n)`, and up to the end of `code` at full, ended
/// early by a transfer or a trap (whose routes make their own exits). Each instruction is carried over by its
/// route and the run's end falls through to an exit to the inside. `None` when no instruction starts there.
///
/// Under definition E every counted event is a transfer and so ends its run: a run holds at most one event,
/// and its add is placed at the start of the run, where it was measured cheapest (section 4). The position
/// map keeps the count one ahead from the add to the event's commit point.
pub fn translate_native<G: Native>(
    code: &[u8],
    address: u64,
    look_ahead: LookAhead,
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
        let event = counted(here, &G::flow(&instruction));
        run.push((instruction, at..at + length, here, route, event));
        at += length;
        if matches!(route, Route::Transfer | Route::Trap) {
            break;
        }
    }
    let ends_itself = matches!(run.last()?.3, Route::Transfer | Route::Trap);
    let mut out = HostCode::new();
    for (k, (instruction, bytes, here, _, event)) in run.iter().enumerate() {
        out.start(*here, *event);
        if k == 0 {
            for _ in run.iter().filter(|r| r.4) {
                G::count(&mut out);
            }
        }
        G::carry(instruction, &code[bytes.clone()], *here, &mut out);
    }
    if !ends_itself {
        G::call(&mut out, InsideCall::Exit);
    }
    out.finish();
    Some(out)
}
