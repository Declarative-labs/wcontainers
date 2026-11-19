//! The native output (docs/native.md section 3): host code of the guest's own architecture, each guest
//! instruction carried over by its route, with the count's adds and the position map. No WebAssembly is made.

use wcontainers_guest_cpu::{HostCode, InsideCall, Native, Route, counted};

/// The piece of one instruction (look-ahead none) at guest `address`, whose bytes start `code`: its count's
/// add, its route's host code, and its exit to the inside; `None` when no instruction starts there.
///
/// Under definition E every counted event is a transfer and ends its straight-line run, so the run holds one
/// event, and its add is placed at the start of the run, where it was measured cheapest (section 4).
pub fn translate_native<G: Native>(code: &[u8], address: u64) -> Option<HostCode> {
    let (instruction, length) = G::decode(code)?;
    let mut out = HostCode::new();
    out.start(address);
    let flow = G::flow(&instruction);
    if counted(address, &flow) {
        G::count(&mut out);
    }
    let route = G::route(&instruction);
    G::carry(&instruction, &code[..length.into()], address, &mut out);
    // A transfer makes its own exits and a trap never comes back.
    if !matches!(route, Route::Transfer | Route::Trap) {
        G::call(&mut out, InsideCall::Exit);
    }
    out.finish();
    Some(out)
}
