//! Checks of translated modules: their validity, and the kind of every guest access an ordered form makes.

use wasmparser::{MemArg, MemArgTarget, Operator, Parser, Payload, Validator, WasmFeatures};
use wcontainers_guest_cpu::AccessOrdering;
use wcontainers_x86::GUEST_MEMORY;

/// What one module's scan found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// How many functions were read.
    pub functions: usize,
    /// How many atomic loads, stores and read-modify-writes of the guest's memory there are.
    pub atomic: usize,
    /// Plain stores of the guest's memory where the form keeps stores plain.
    pub plain_stores_kept: usize,
    /// How many bulk operations on the guest's memory (`memory.copy`, `memory.fill`) there are.
    pub bulk: usize,
    /// Each access the form does not allow, and each function with a bulk operation not between two fences.
    pub wrong: Vec<String>,
}

impl Scan {
    /// Adds another module's scan to this one.
    pub fn add(&mut self, other: Scan) {
        self.functions += other.functions;
        self.atomic += other.atomic;
        self.plain_stores_kept += other.plain_stores_kept;
        self.bulk += other.bulk;
        self.wrong.extend(other.wrong);
    }
}

/// How an operator touches memory without being atomic: a plain load, a plain store, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plain {
    Load,
    Store,
}

/// Finds the plain (not atomic) access an operator makes, with its memory argument.
fn plain_access(operator: &Operator<'_>) -> Option<(Plain, MemArg)> {
    use Operator::*;
    Some(match *operator {
        I32Load { memarg }
        | I64Load { memarg }
        | F32Load { memarg }
        | F64Load { memarg }
        | I32Load8S { memarg }
        | I32Load8U { memarg }
        | I32Load16S { memarg }
        | I32Load16U { memarg }
        | I64Load8S { memarg }
        | I64Load8U { memarg }
        | I64Load16S { memarg }
        | I64Load16U { memarg }
        | I64Load32S { memarg }
        | I64Load32U { memarg }
        | V128Load { memarg }
        | V128Load8x8S { memarg }
        | V128Load8x8U { memarg }
        | V128Load16x4S { memarg }
        | V128Load16x4U { memarg }
        | V128Load32x2S { memarg }
        | V128Load32x2U { memarg }
        | V128Load8Splat { memarg }
        | V128Load16Splat { memarg }
        | V128Load32Splat { memarg }
        | V128Load64Splat { memarg }
        | V128Load32Zero { memarg }
        | V128Load64Zero { memarg }
        | V128Load8Lane { memarg, .. }
        | V128Load16Lane { memarg, .. }
        | V128Load32Lane { memarg, .. }
        | V128Load64Lane { memarg, .. } => (Plain::Load, memarg),
        I32Store { memarg }
        | I64Store { memarg }
        | F32Store { memarg }
        | F64Store { memarg }
        | I32Store8 { memarg }
        | I32Store16 { memarg }
        | I64Store8 { memarg }
        | I64Store16 { memarg }
        | I64Store32 { memarg }
        | V128Store { memarg }
        | V128Store8Lane { memarg, .. }
        | V128Store16Lane { memarg, .. }
        | V128Store32Lane { memarg, .. }
        | V128Store64Lane { memarg, .. } => (Plain::Store, memarg),
        _ => return None,
    })
}

/// Finds the atomic access an operator makes, with its memory argument.
fn atomic_access(operator: &Operator<'_>) -> Option<MemArg> {
    use Operator::*;
    Some(match *operator {
        I64AtomicLoad { memarg }
        | I64AtomicLoad8U { memarg }
        | I64AtomicLoad16U { memarg }
        | I64AtomicLoad32U { memarg }
        | I32AtomicLoad { memarg }
        | I32AtomicLoad8U { memarg }
        | I32AtomicLoad16U { memarg }
        | I64AtomicStore { memarg }
        | I64AtomicStore8 { memarg }
        | I64AtomicStore16 { memarg }
        | I64AtomicStore32 { memarg }
        | I32AtomicStore { memarg }
        | I32AtomicStore8 { memarg }
        | I32AtomicStore16 { memarg }
        | I64AtomicRmwCmpxchg { memarg }
        | I64AtomicRmw32CmpxchgU { memarg }
        | I64AtomicRmw16CmpxchgU { memarg }
        | I64AtomicRmw8CmpxchgU { memarg } => memarg,
        _ => return None,
    })
}

/// Reads every function of a module made in `ordering` and finds each access its form does not allow.
///
/// Not allowed: a plain load of the guest's memory, a plain store of it unless the form keeps stores plain, and a
/// bulk operation on it with fewer than two fences in its function. `ordering` is not `Unordered`.
pub fn scan(wasm: &[u8], ordering: AccessOrdering) -> Scan {
    assert_ne!(
        ordering,
        AccessOrdering::Unordered,
        "only an ordered form is scanned"
    );
    let stores_kept = ordering.stores().is_none();
    let mut found = Scan::default();
    for payload in Parser::new(0).parse_all(wasm) {
        let Payload::CodeSectionEntry(body) = payload.expect("a module") else {
            continue;
        };
        let function = found.functions;
        found.functions += 1;
        let (mut bulk, mut fences) = (0, 0);
        for operator in body.get_operators_reader().expect("a body") {
            let operator = operator.expect("an operator");
            let guest = |memarg: MemArg| memarg.target == MemArgTarget::Memory(GUEST_MEMORY);
            match operator {
                Operator::MemoryCopy { dst_mem, .. } if dst_mem == GUEST_MEMORY => bulk += 1,
                Operator::MemoryFill { mem } if mem == GUEST_MEMORY => bulk += 1,
                Operator::AtomicFence { .. } => fences += 1,
                ref operator => {
                    if atomic_access(operator).is_some_and(guest) {
                        found.atomic += 1;
                    }
                    match plain_access(operator) {
                        Some((Plain::Store, memarg)) if stores_kept && guest(memarg) => {
                            found.plain_stores_kept += 1;
                        }
                        Some((_, memarg)) if guest(memarg) => {
                            found
                                .wrong
                                .push(format!("function {function}: {operator:?}"));
                        }
                        _ => {}
                    }
                }
            }
        }
        found.bulk += bulk;
        if fences < 2 * bulk {
            found.wrong.push(format!(
                "function {function}: {bulk} bulk operations and {fences} fences"
            ));
        }
    }
    found
}

/// Validates a translated module with the features the translator's output may use.
pub fn validity(wasm: &[u8]) -> Result<(), String> {
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::RELAXED_SIMD;
    Validator::new_with_features(features)
        .validate_all(wasm)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
