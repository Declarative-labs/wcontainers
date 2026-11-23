//! The passes in which the meaning runs a repeated string instruction (meaning.rs `repeated_string`), in plain
//! form, for code that counts them without emitting the meaning: the native output and the inside.

use crate::{DF, TF};

/// The bytes one bulk pass of `rep movs` or `rep stos` moves at most, so a stop waits for no more than this.
pub const BULK_STRING_BYTES: u64 = 1 << 20;

/// What each element of a string instruction does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringKind {
    Move,
    Store,
    Load,
    Compare,
    Scan,
}

impl StringKind {
    /// It reads at rsi.
    pub fn source(self) -> bool {
        matches!(
            self,
            StringKind::Move | StringKind::Load | StringKind::Compare
        )
    }

    /// It reads or writes at rdi.
    pub fn destination(self) -> bool {
        self != StringKind::Load
    }
}

/// A repeated string instruction as its passes depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repeated {
    pub kind: StringKind,
    /// The bytes of one element.
    pub element: u8,
    /// The guest's own address-size prefix, under which every pass is one element.
    pub address32: bool,
    pub rflags: u64,
    /// The count register when the instruction started, and when it ended or was stopped.
    pub count_before: u64,
    pub count_after: u64,
    /// rdi less rsi, the same at every pass since both advance together.
    pub distance: u64,
}

/// How many times the meaning has run the instruction once the count went from `count_before` to `count_after`:
/// its passes completed by then, and one that does nothing for a count of zero.
pub fn string_passes(r: &Repeated) -> u64 {
    if r.count_before == 0 {
        return 1;
    }
    let elements = r.count_before - r.count_after;
    let bulk = matches!(r.kind, StringKind::Move | StringKind::Store)
        && !r.address32
        && r.rflags & (DF | TF) == 0;
    if !bulk {
        return elements;
    }
    let bytes = u64::from(r.element);
    let most = BULK_STRING_BYTES / bytes;
    // A forward copy onto a destination 1 to n * bytes - 1 bytes above its source, n the pass's elements, goes
    // one element a pass: while more than distance / bytes elements are left, when that is below the cap.
    let below = r.distance / bytes;
    let one_by_one = if r.kind == StringKind::Move && r.distance != 0 && below < most {
        r.count_before.saturating_sub(below)
    } else {
        0
    };
    if elements <= one_by_one {
        return elements;
    }
    let (left, done) = (r.count_before - one_by_one, elements - one_by_one);
    let bulk_passes = if done == left {
        left.div_ceil(most)
    } else {
        done / most
    };
    one_by_one + bulk_passes
}

#[cfg(test)]
mod tests {
    use super::{BULK_STRING_BYTES, Repeated, StringKind, string_passes};
    use crate::{DF, TF};

    fn ran(kind: StringKind, element: u8, count: u64, distance: u64) -> Repeated {
        Repeated {
            kind,
            element,
            address32: false,
            rflags: 0,
            count_before: count,
            count_after: 0,
            distance,
        }
    }

    /// The case the plan's check gave: 90 one-element passes, then one bulk pass of 10.
    #[test]
    fn a_copy_ten_bytes_above_its_source() {
        assert_eq!(string_passes(&ran(StringKind::Move, 1, 100, 10)), 91);
    }

    #[test]
    fn bulk_passes_are_capped() {
        let most = BULK_STRING_BYTES / 8;
        let store = ran(StringKind::Store, 8, 3 * most + 1, 0);
        assert_eq!(string_passes(&store), 4);
        let apart = ran(StringKind::Move, 8, 2 * most, BULK_STRING_BYTES);
        assert_eq!(string_passes(&apart), 2);
        // Below its source, or onto it, a copy never overlaps forward.
        assert_eq!(string_passes(&ran(StringKind::Move, 1, 100, 0)), 1);
        assert_eq!(
            string_passes(&ran(StringKind::Move, 1, 100, 10u64.wrapping_neg())),
            1
        );
    }

    #[test]
    fn one_element_a_pass() {
        for kind in [StringKind::Load, StringKind::Compare, StringKind::Scan] {
            assert_eq!(string_passes(&ran(kind, 2, 7, 0)), 7);
        }
        for rflags in [DF, TF] {
            let down = Repeated {
                rflags,
                ..ran(StringKind::Store, 1, 7, 0)
            };
            assert_eq!(string_passes(&down), 7);
        }
        let narrow = Repeated {
            address32: true,
            ..ran(StringKind::Move, 1, 7, 1 << 30)
        };
        assert_eq!(string_passes(&narrow), 7);
    }

    #[test]
    fn a_count_of_zero_runs_once() {
        assert_eq!(string_passes(&ran(StringKind::Move, 1, 0, 10)), 1);
    }

    /// Stopped part way, only the passes completed count: none of a bulk pass is a pass's end.
    #[test]
    fn stopped_part_way() {
        let stopped = |after| Repeated {
            count_after: after,
            ..ran(StringKind::Move, 1, 100, 10)
        };
        assert_eq!(string_passes(&stopped(95)), 5);
        assert_eq!(string_passes(&stopped(10)), 90);
        assert_eq!(string_passes(&stopped(3)), 90);
        let most = BULK_STRING_BYTES;
        let store = Repeated {
            count_after: most + 1,
            ..ran(StringKind::Store, 1, 3 * most, 0)
        };
        assert_eq!(string_passes(&store), 1);
    }
}
