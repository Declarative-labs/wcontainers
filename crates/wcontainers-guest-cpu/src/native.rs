//! What the native output needs of a guest that can also be the host: how each decoded instruction is
//! carried over to host code of the same architecture. It never says what an instruction computes; the
//! hardware computes it, and the meanings stay the definition the routes are proven against.

use crate::{Flow, FlowKind, Guest};

/// How one decoded instruction is carried over (docs/native.md section 3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    /// The bytes as they are.
    Copy,
    /// A load or store whose access may cross a page: the page test and probes, then the bytes as they are.
    Split,
    /// A copy whose register fields collide where the architecture leaves the result open (a load or store
    /// writing back to its own transfer register, a load pair into one register): the colliding transfer
    /// register is replaced by the scratch register, which gives the meaning's result.
    Collision,
    /// Nothing: the meaning only goes on to the next instruction.
    Nothing,
    /// A copy with an alignment test in front.
    Aligned,
    /// A pc-relative instruction rebuilt on the guest address.
    Rebase,
    /// A branch, call or return.
    Transfer,
    /// A call into the inside, which answers as the meaning does.
    Kernel,
    /// A register read answered with the presented value.
    Constant,
    /// A read or write of the float status or mode, between two calls into the inside.
    FloatStatus,
    /// A trap with the meaning's kind.
    Trap,
    /// The exclusive monitor kept as the meaning keeps it.
    Exclusive,
}

/// What the native output does with a hook or a control a meaning uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeClass {
    /// The hardware holds it as presented.
    Hardware,
    /// The WebAssembly output's own bookkeeping, with nothing to keep natively.
    Bookkeeping,
    /// Kept by the native output or answered by the inside: a form touching it takes this route.
    Routed(Route),
}

/// A guest whose instructions the native output carries over to the same architecture.
pub trait Native: Guest {
    /// The route of one decoded instruction.
    fn route(i: &Self::Instruction) -> Route;
    /// The class of a hook, given the constant its meaning passes as the first argument where it passes one
    /// (the float hook's operation).
    fn hook_class(hook: Self::Hook, first: Option<i64>) -> NativeClass;
    /// The class of a control, read or written.
    fn control_class(control: Self::Control, write: bool) -> NativeClass;
}

/// Whether an executed instruction at `address` is one counted event of the progress count (definition E):
/// every instruction that can transfer backward, a direct branch whose target is at or below its own
/// address, taken or not, every call, every return, every jump through a register.
pub fn counted<R>(address: u64, flow: &Flow<R>) -> bool {
    match flow.kind {
        FlowKind::Call | FlowKind::CallThrough(_) | FlowKind::Return | FlowKind::JumpThrough(_) => {
            true
        }
        FlowKind::Plain => flow
            .target
            .is_some_and(|offset| address.wrapping_add_signed(offset) <= address),
    }
}

#[cfg(test)]
mod tests {
    use super::counted;
    use crate::{Flow, FlowKind, Through};

    fn direct(target: i64) -> Flow<u8> {
        Flow {
            falls_through: true,
            target: Some(target),
            kind: FlowKind::Plain,
        }
    }

    #[test]
    fn counts_what_can_go_backward() {
        let at = 0x40_1000;
        let step = 4;
        assert!(counted(at, &direct(-step)));
        assert!(counted(at, &direct(0)));
        assert!(!counted(at, &direct(step)));
        let plain = Flow::<u8> {
            falls_through: true,
            target: None,
            kind: FlowKind::Plain,
        };
        assert!(!counted(at, &plain));
        for kind in [
            FlowKind::Call,
            FlowKind::CallThrough(Through::Register(0)),
            FlowKind::Return,
            FlowKind::JumpThrough(Through::Memory),
        ] {
            let flow = Flow {
                falls_through: false,
                target: Some(step),
                kind,
            };
            assert!(counted(at, &flow));
        }
    }
}
