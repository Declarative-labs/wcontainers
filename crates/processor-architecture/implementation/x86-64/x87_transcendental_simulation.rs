//! The stack of the transcendental forms: the operands read, their stack faults, the results written and pushed or
//! popped, and C1 and C2.

use super::fault;
use crate::X87;
use crate::planted::{Bug, is_active};
use crate::x87::{C1, C2, INDEFINITE};
use crate::x87_transcendental::{Computed, Function, compute};

/// What the behavior does not decide after a transcendental form: the physical registers that hold the model's
/// approximations, and the FSW bits that depend on the chip's own last bits: C1, which tells how they were rounded,
/// and UE or OE where a result lies at the threshold.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Undecided {
    pub registers: u8,
    pub status: u16,
}

/// Applies the form to the x87 state.
pub(super) fn execute_transcendental(x87: &mut X87, function: Function) -> Undecided {
    let mut undecided = Undecided::default();
    let pushes = function.pushes();
    // An empty operand is an underflow; else a full ST(7) is an overflow for a form that pushes.
    let empty = x87.is_empty(0) || function.is_binary() && x87.is_empty(1);
    if empty || pushes && !x87.is_empty(7) {
        if fault(x87, !empty) {
            if function.is_binary() {
                x87.write(1, INDEFINITE);
                x87.pop();
            } else {
                x87.write(0, INDEFINITE);
                if pushes {
                    x87.set_top(x87.top() + 7);
                    x87.write(0, INDEFINITE);
                }
            }
        }
        if function.is_trigonometric() {
            x87.set_code(C2, false);
        }
        return undecided;
    }
    let (top, next) = (x87.read(0), if function.is_binary() { x87.read(1) } else { 0 });
    match compute(function, top, next, x87.control) {
        Computed::Stopped { flags } => x87.raise(flags),
        Computed::OutOfRange => {
            x87.set_code(C2, true);
            x87.set_code(C1, false);
        }
        Computed::Done { flags, first, second, undecided: by_last_bit } => {
            x87.raise(flags);
            let index = usize::from(function.is_binary());
            x87.write(index, first.value);
            if first.approximate {
                undecided.registers |= 1 << x87.to_physical(index);
            }
            x87.set_code(C1, first.up);
            if function.is_binary() && !is_active(Bug::BinaryTranscendentalKeepsStack) {
                x87.pop();
            }
            if is_active(Bug::UndecidedRegisterMisplaced) && function.is_binary() && first.approximate {
                undecided.registers = 1 << x87.to_physical(7);
            }
            if let Some(second) = second {
                x87.set_top(x87.top() + 7);
                x87.write(0, second.value);
                if second.approximate {
                    undecided.registers |= 1 << x87.to_physical(0);
                }
            }
            if function.is_trigonometric() {
                x87.set_code(C2, false);
            }
            if undecided.registers != 0 {
                undecided.status |= C1 | by_last_bit;
            }
        }
    }
    undecided
}
