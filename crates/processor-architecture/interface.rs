#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
#[cfg(verus_keep_ghost)]
use core::convert::Infallible;
use core::ops::Range;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::slice::{cmp_le, sorted_by_le};

/// The proofs about the memory model that this crate's methods use, which are not part of the specification.
#[cfg(verus_keep_ghost)]
pub mod memory_proof;

#[cfg(verus_keep_ghost)]
use memory_proof::*;
mod trusted;
#[cfg(test)]
mod trusted_tests;

use trusted::*;

verus! {

/// One processor architecture, as a program running on it sees it.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub trait ProcessorArchitecture: Sized {
    /// All stateful components of the processor.
    type ProcessorState: Clone + Eq;
    /// What a program may do with a page of memory.
    type PagePermission: Copy + Eq;
    /// An instruction at its most granular division.
    ///
    /// On A64 this is an [instruction variant](https://developer.arm.com/documentation/ddi0487/mc/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-1-About-the-A64-instruction-set#:~:text=each%20syntax%20is%20an%20instruction%20variant), on x86 an [instruction form](https://intelxed.github.io/ref-manual/group__IFORM.html).
    type Instruction: Copy + Eq;
    /// An instruction with a specific pinned behavior that should be expected of it.
    // VERUS: no trait that refers back to itself, so code that simulates states `Simulator` itself.
    type PreciseInstruction: Copy + Eq;
    // VERUS: no `impl Trait` in a trait, so each returned iterator is a named type.
    type Instructions: Iterator<Item = Self::Instruction>;
    type KnownBehaviors: Iterator<Item = Self::PreciseInstruction>;

    /// The addresses a program can use.
    const ADDRESSABLE: Range<u64>;
    const DEFAULT_PERMISSION: Self::PagePermission;

    /// The instructions of the architecture, in the order they are listed.
    spec fn instructions() -> Seq<Self::Instruction>;

    /// The behaviors a chip is known to show for the instruction, in the order they are listed.
    spec fn known_behaviors(instruction: Self::Instruction) -> Seq<Self::PreciseInstruction>;

    /// `==` on processors and on permissions holds exactly for the same value.
    proof fn equality_is_identity()
        ensures
            obeys_concrete_eq::<Self::ProcessorState>(),
            obeys_concrete_eq::<Self::PagePermission>(),
    ;

    /// Returns the set of instructions of the architecture.
    fn list_instructions() -> (instructions: Self::Instructions)
        ensures
            instructions.obeys_prophetic_iter_laws(),
            instructions.remaining() == Self::instructions(),
            instructions.decrease() is Some,
            Self::instructions().no_duplicates(),
    ;

    /// Returns the set of behaviors a chip is known to show for the instruction.
    fn list_known_behaviors(instruction: Self::Instruction) -> (behaviors: Self::KnownBehaviors)
        ensures
            behaviors.obeys_prophetic_iter_laws(),
            behaviors.remaining() == Self::known_behaviors(instruction),
            behaviors.decrease() is Some,
            Self::known_behaviors(instruction).no_duplicates(),
    ;
}

/// One memory change as the specification reads it: a `MemoryDelta` with its bytes as a sequence.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub ghost enum Assignment<Permission> {
    PermissionAssignment { addresses: Range<u64>, permission: Permission },
    ValueAssignment { start: u64, bytes: Seq<u8> },
}

/// Memory as the specification reads it: the byte and the permission at each address.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub type Memory<Permission> = spec_fn(u64) -> (u8, Permission);

/// The memory no change has reached: every byte zero, every address with the default permission.
pub open spec fn zeroed<Architecture: ProcessorArchitecture>() -> Memory<Architecture::PagePermission> {
    |address: u64| (0u8, Architecture::DEFAULT_PERMISSION)
}

/// The memory after one change; a value assignment that runs past the last address goes on from address zero.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub open spec fn apply<Permission>(memory: Memory<Permission>, change: Assignment<Permission>) -> Memory<Permission> {
    match change {
        Assignment::PermissionAssignment { addresses, permission } => |address: u64|
            if addresses.start <= address < addresses.end { (memory(address).0, permission) } else { memory(address) },
        Assignment::ValueAssignment { start, bytes } => |address: u64| {
            let offset = if start <= address { address - start } else { address + 0x1_0000_0000_0000_0000 - start };
            if offset < bytes.len() { (bytes[offset], memory(address).1) } else { memory(address) }
        },
    }
}

/// The memory after the changes, applied in order.
pub open spec fn apply_all<Permission>(memory: Memory<Permission>, changes: Seq<Assignment<Permission>>) -> Memory<Permission>
    decreases changes.len(),
{
    if changes.len() == 0 { memory } else { apply(apply_all(memory, changes.drop_last()), changes.last()) }
}

/// The memory a list of mutations leaves, applied in order to the zeroed memory.
pub open spec fn memory_of<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
) -> Memory<Architecture::PagePermission> {
    apply_all(zeroed::<Architecture>(), assignments(mutations))
}

/// Each mutation as the specification reads it.
pub open spec fn assignments<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
) -> Seq<Assignment<Architecture::PagePermission>> {
    mutations.map_values(|mutation: MemoryDelta<'memory, Architecture>| mutation@)
}

/// The processor and memory a behavior that cannot fail leaves from the state.
pub open spec fn simulated_after<'memory, Architecture, Behavior>(
    behavior: Behavior,
    before: ExecutionState<'memory, Architecture>,
) -> (Architecture::ProcessorState, Memory<Architecture::PagePermission>)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    let memory = memory_of(before.memory_mutations@);
    match behavior.simulation(before.processor, memory) {
        Ok((processor, changes)) => (processor, apply_all(memory, changes)),
        // Never: the error is `Infallible`.
        Err(_) => (before.processor, memory),
    }
}

/// The processor and memory a state leaves once its processor becomes the outcome's and its memory receives the outcome's changes.
pub open spec fn state_after<'memory, Architecture: ProcessorArchitecture>(
    before: ExecutionState<'memory, Architecture>,
    outcome: (Architecture::ProcessorState, Seq<Assignment<Architecture::PagePermission>>),
) -> (Architecture::ProcessorState, Memory<Architecture::PagePermission>) {
    (outcome.0, apply_all(memory_of(before.memory_mutations@), outcome.1))
}

/// The sink went from `before` to `after` by receiving the changes, in order.
pub open spec fn receives<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    before: Sink,
    after: Sink,
    changes: Seq<Assignment<Architecture::PagePermission>>,
) -> bool {
    Sink::obeys_sink_spec() ==> forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), changes)
}

pub trait Simulator<Architecture: ProcessorArchitecture> {
    #[allow(
        reviewable::explicit_shared_error,
        reason = "simulate only runs simulate_in_place and passes its error on unchanged"
    )]
    type Error;

    /// The processor this instruction leaves and the changes it makes to memory, or its error, from the processor and memory before.
    spec fn simulation(
        &self,
        processor: Architecture::ProcessorState,
        memory: Memory<Architecture::PagePermission>,
    ) -> Result<(Architecture::ProcessorState, Seq<Assignment<Architecture::PagePermission>>), Self::Error>;

    /// Simulates applying this instruction to the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    /// On error the processor is left as it was.
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::ProcessorState,
        memory: &[MemoryDelta<'_, Architecture>],
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> (result: Result<(), Self::Error>)
        ensures
            match self.simulation(*old(processor), memory_of(memory@)) {
                Ok((after, changes)) => {
                    &&& result is Ok
                    &&& *final(processor) == after
                    &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
                },
                Err(error) => result == Err::<(), Self::Error>(error) && *final(processor) == *old(processor),
            },
    ;

    /// Simulates applying this instruction to the before state.
    fn simulate<'memory>(
        &self,
        mut before: ExecutionState<'memory, Architecture>,
    ) -> (after: Result<ExecutionState<'memory, Architecture>, Self::Error>)
        ensures
            match self.simulation(before.processor, memory_of(before.memory_mutations@)) {
                Ok((processor, changes)) => after matches Ok(after) && after.processor == processor
                    && memory_of(after.memory_mutations@) == apply_all(memory_of(before.memory_mutations@), changes),
                Err(error) => after == Err::<ExecutionState<'memory, Architecture>, Self::Error>(error),
            },
    {
        let ghost memory = before.memory_mutations@;
        let ghost processor = before.processor;
        let mut mutations: Vec<MemoryDelta<'memory, Architecture>> = Vec::new();
        let ghost empty = mutations;
        let result = self.simulate_in_place(
            &mut before.processor,
            before.memory_mutations.as_slice(),
            &mut mutations,
        );
        // VERUS: no `?` whose error a specification compares, so the error is matched.
        match result {
            Ok(()) => {},
            Err(error) => return Err(error),
        }
        let ghost filled = mutations;
        before.memory_mutations.append(&mut mutations);
        proof {
            if let Ok((_, changes)) = self.simulation(processor, memory_of(memory)) {
                assert(filled.applied_to(memory_of(memory)) == apply_all(empty.applied_to(memory_of(memory)), changes));
                lemma_no_assignments(empty@, memory_of(memory));
                lemma_appended_memory(memory, filled@);
            }
        }
        Ok(before)
    }
}

// VERUS: no contract on what a closure has received, so the memory hook is a trait.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub trait MemorySink<Architecture: ProcessorArchitecture> {
    /// Whether the sink holds what it received where a specification can read it, which a closure does not.
    spec fn obeys_sink_spec() -> bool;

    /// The memory after every change the sink has received, applied in order to the given memory.
    spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission>;

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>)
        ensures
            Self::obeys_sink_spec() ==> forall|memory: Memory<Architecture::PagePermission>|
                #[trigger] final(self).applied_to(memory) == apply(old(self).applied_to(memory), mutation@),
    ;
}

// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<'memory, Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'memory, Architecture>>
{
    open spec fn obeys_sink_spec() -> bool {
        true
    }

    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        apply_all(memory, assignments(self@))
    }

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        let ghost before = self@;
        let ghost change = mutation@;
        self.push(mutation.into_owned());
        proof {
            assert(assignments(self@).drop_last() =~= assignments(before));
            assert(assignments(self@).last() == change);
        }
    }
}

/// All state that some instruction might mutate during execution.
#[derive(Clone)]
pub struct ExecutionState<'memory, Architecture: ProcessorArchitecture> {
    pub processor: Architecture::ProcessorState,
    /// The changes that have been applied to the memory (as if it started from a fully zeroed out state), in chronological order.
    pub memory_mutations: Vec<MemoryDelta<'memory, Architecture>>,
}

#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[derive(Clone, PartialEq, Eq)]
pub enum MemoryDelta<'memory, Architecture: ProcessorArchitecture> {
    PermissionAssignment {
        addresses: Range<u64>,
        permission: Architecture::PagePermission,
    },
    ValueAssignment {
        start: u64,
        bytes: Cow<'memory, [u8]>,
    },
}

#[cfg_attr(verus_keep_ghost, verus::trusted)]
impl<'memory, Architecture: ProcessorArchitecture> View for MemoryDelta<'memory, Architecture> {
    type V = Assignment<Architecture::PagePermission>;

    open spec fn view(&self) -> Assignment<Architecture::PagePermission> {
        match self {
            MemoryDelta::PermissionAssignment { addresses, permission } =>
                Assignment::PermissionAssignment { addresses: *addresses, permission: *permission },
            MemoryDelta::ValueAssignment { start, bytes } => Assignment::ValueAssignment { start: *start, bytes: bytes@ },
        }
    }
}

impl<Architecture: ProcessorArchitecture> MemoryDelta<'_, Architecture> {
    pub fn into_owned(self) -> (owned: MemoryDelta<'static, Architecture>)
        ensures
            owned@ == self@,
    {
        match self {
            Self::PermissionAssignment {
                addresses,
                permission,
            } => MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            },
            Self::ValueAssignment { start, bytes } => MemoryDelta::ValueAssignment {
                start,
                // VERUS: no `Cow::into_owned`, so its two cases are written out.
                bytes: Cow::Owned(match bytes {
                    Cow::Borrowed(borrowed) => bytes_to_vec(borrowed),
                    Cow::Owned(owned) => owned,
                }),
            },
        }
    }
}

#[cfg(verus_keep_ghost)]
impl<Architecture: ProcessorArchitecture> vstd::std_specs::cmp::PartialEqSpecImpl for ExecutionState<'_, Architecture> {
    open spec fn obeys_eq_spec() -> bool {
        true
    }

    /// The same processor, and the same byte and permission at every address.
    open spec fn eq_spec(&self, other: &Self) -> bool {
        &&& self.processor == other.processor
        &&& memory_of(self.memory_mutations@) == memory_of(other.memory_mutations@)
    }
}

/// Two states are equal when they hold the same processor and the same memory, regardless of the order of changes.
impl<Architecture: ProcessorArchitecture> PartialEq for ExecutionState<'_, Architecture> {
    fn eq(&self, other: &Self) -> (equal: bool) {
        proof {
            Architecture::equality_is_identity();
            reveal(obeys_concrete_eq);
        }
        if self.processor != other.processor {
            return false;
        }
        // VERUS: no closure that captures a mutable reference, so the mutations are collected and counted.
        let mut mutations: Vec<MemoryDelta<'_, Architecture>> = Vec::new();
        let ghost empty = mutations;
        diff_memory(
            self.memory_mutations.as_slice(),
            other.memory_mutations.as_slice(),
            &mut mutations,
        );
        proof {
            lemma_diff_into_empty(memory_of(self.memory_mutations@), memory_of(other.memory_mutations@), empty, mutations);
        }
        mutations.is_empty()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}

/// Calls the hook with each mutation that turns the memory of the first list into the memory of the second, bytes before permissions.
pub fn diff_memory<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
)
    ensures
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(memory_of(first@), changes)]
            receives(*old(mutation_hook), *final(mutation_hook), changes)
                && apply_all(memory_of(first@), changes) == memory_of(second@),
        memory_of(first@) == memory_of(second@) ==> *final(mutation_hook) == *old(mutation_hook),
{
    proof {
        Architecture::equality_is_identity();
    }
    let ghost hook_before = *mutation_hook;
    // Mutations both lists make in the same order leave the same memory wherever no other mutation reaches, so only
    // what the unpaired mutations reach can differ.
    // VERUS: a function this long is more than the verifier checks at once, so each step is a function of its own.
    let (mut written, has_unpaired_permission) = record_unpaired_reach(first, second);
    proof {
        broadcast use vstd::laws_cmp::group_laws_cmp;
    }
    let ghost unsorted = written@;
    written.as_mut_slice().sort_unstable();
    proof {
        lemma_sorted_ranges(unsorted, written@);
    }
    // Overlapping and touching ranges become one, so that each address is compared once.
    let ghost sorted = written@;
    merge_ranges(&mut written);
    proof {
        assert forall|address: u64| !in_ranges(written@, address)
            implies #[trigger] memory_of(first@)(address).0 == memory_of(second@)(address).0 by {
            assert(in_ranges(sorted, address) == in_ranges(unsorted, address));
        }
    }
    let ghost hook_between = *mutation_hook;
    // The buffers live until the diff ends, so that the permissions below allocate as they did before the proof.
    let (mut first_bytes, mut second_bytes) = (Vec::new(), Vec::new());
    #[cfg_attr(not(verus_keep_ghost), expect(unused_variables, reason = "only the proof reads the changes sent"))]
    let byte_changes = send_changed_bytes(first, second, written.as_slice(), &mut first_bytes, &mut second_bytes, mutation_hook);
    if !has_unpaired_permission {
        proof {
            assert(apply_all(memory_of(first@), byte_changes@) =~= memory_of(second@)) by {
                assert forall|address: u64| #[trigger] apply_all(memory_of(first@), byte_changes@)(address) == memory_of(second@)(address) by {
                    assert(memory_of(first@)(address).1 == memory_of(second@)(address).1);
                }
            }
        }
        return;
    }
    let ghost hook_bytes = *mutation_hook;
    #[cfg_attr(not(verus_keep_ghost), expect(unused_variables, reason = "only the proof reads the changes sent"))]
    let permission_changes = send_changed_permissions(first, second, mutation_hook);
    proof {
        let changes = byte_changes@ + permission_changes@;
        lemma_receives_append(hook_before, hook_bytes, *mutation_hook, byte_changes@, permission_changes@);
        lemma_apply_all_append(memory_of(first@), byte_changes@, permission_changes@);
        let after_bytes = apply_all(memory_of(first@), byte_changes@);
        assert(after_bytes =~= (|address: u64| (memory_of(second@)(address).0, memory_of(first@)(address).1)));
        assert(apply_all(memory_of(first@), changes) == memory_of(second@));
    }
}

/// Returns ranges that hold every byte an unpaired mutation of either list writes, and whether an unpaired one gives
/// a permission: outside those the two lists leave the same memory.
fn record_unpaired_reach<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
) -> (reach: (Vec<(u128, u128)>, bool))
    ensures
        well_formed(reach.0@),
        same_outside(memory_of(first@), memory_of(second@), reach.0@, reach.1),
{
    proof {
        Architecture::equality_is_identity();
    }
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    // VERUS: no sort by a key a specification can read, so the ranges are (start, end) pairs, which sort by start first.
    let (mut written, mut has_unpaired_permission): (Vec<(u128, u128)>, bool) = (Vec::new(), false);
    // VERUS: no `position` with a closure, so the lists are walked by index.
    let mut rest: usize = 0;
    let mut index: usize = 0;
    proof {
        assert(first@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(second@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(memory_of(first@.take(0)) =~= memory_of(second@.take(0)));
    }
    while index < first.len()
        invariant
            index <= first@.len(),
            rest <= second@.len(),
            obeys_concrete_eq::<Architecture::PagePermission>(),
            well_formed(written@),
            same_outside(memory_of(first@.take(index as int)), memory_of(second@.take(rest as int)), written@, has_unpaired_permission),
        decreases first@.len() - index,
    {
        let mutation = &first[index];
        let mut candidate = rest;
        let mut found = false;
        while !found && candidate < second.len()
            invariant
                index < first@.len(),
                rest <= candidate <= second@.len(),
                *mutation == first@[index as int],
                found ==> candidate < second@.len() && first@[index as int]@ == second@[candidate as int]@,
                obeys_concrete_eq::<Architecture::PagePermission>(),
            decreases second@.len() - candidate + if found { 0int } else { 1int },
        {
            if is_same_mutation(mutation, &second[candidate]) {
                found = true;
            } else {
                candidate += 1;
            }
        }
        if found {
            let ghost paired_written = written@;
            let ghost paired_flag = has_unpaired_permission;
            let mut skipped = rest;
            while skipped < candidate
                invariant
                    rest <= skipped <= candidate < second@.len(),
                    well_formed(written@),
                    paired_written.is_prefix_of(written@),
                    paired_flag ==> has_unpaired_permission,
                    same_outside(memory_of(second@.take(skipped as int)), memory_of(second@.take(rest as int)), written@, has_unpaired_permission),
                decreases candidate - skipped,
            {
                let ghost skipped_written = written@;
                let ghost skipped_flag = has_unpaired_permission;
                add_unpaired(&second[skipped], &mut written, &mut has_unpaired_permission);
                proof {
                    lemma_unpaired_change(second@, skipped as int, memory_of(second@.take(rest as int)), skipped_written,
                        skipped_flag, written@, has_unpaired_permission);
                    assert(paired_written.is_prefix_of(written@)) by {
                        assert(written@.take(skipped_written.len() as int) == skipped_written);
                        assert(skipped_written.take(paired_written.len() as int) == paired_written);
                        assert(written@.take(paired_written.len() as int) =~= paired_written);
                    }
                }
                skipped += 1;
            }
            proof {
                if rest == candidate {
                    assert(same_outside(memory_of(second@.take(skipped as int)), memory_of(second@.take(rest as int)), written@, has_unpaired_permission));
                }
                lemma_paired_change(first@, second@, index as int, rest as int, candidate as int, paired_written, paired_flag,
                    written@, has_unpaired_permission);
            }
            rest = candidate + 1;
        } else {
            let ghost unpaired_written = written@;
            let ghost unpaired_flag = has_unpaired_permission;
            add_unpaired(mutation, &mut written, &mut has_unpaired_permission);
            proof {
                lemma_unpaired_change(first@, index as int, memory_of(second@.take(rest as int)), unpaired_written, unpaired_flag,
                    written@, has_unpaired_permission);
            }
        }
        index += 1;
    }
    proof {
        assert(first@.take(first@.len() as int) =~= first@);
        lemma_same_outside_symmetric(memory_first, memory_of(second@.take(rest as int)), written@, has_unpaired_permission);
    }
    while rest < second.len()
        invariant
            rest <= second@.len(),
            well_formed(written@),
            same_outside(memory_of(second@.take(rest as int)), memory_first, written@, has_unpaired_permission),
            memory_first == memory_of(first@),
        decreases second@.len() - rest,
    {
        let ghost rest_written = written@;
        let ghost rest_flag = has_unpaired_permission;
        add_unpaired(&second[rest], &mut written, &mut has_unpaired_permission);
        proof {
            lemma_unpaired_change(second@, rest as int, memory_first, rest_written, rest_flag, written@, has_unpaired_permission);
        }
        rest += 1;
    }
    proof {
        assert(second@.take(second@.len() as int) =~= second@);
        lemma_same_outside_symmetric(memory_second, memory_first, written@, has_unpaired_permission);
        broadcast use vstd::laws_cmp::group_laws_cmp;
    }
    proof {
        assert(second@.take(second@.len() as int) =~= second@);
        lemma_same_outside_symmetric(memory_second, memory_first, written@, has_unpaired_permission);
    }
    (written, has_unpaired_permission)
}

/// Sends a mutation for each run of bytes the two lists leave different within the ranges, which hold every byte
/// they leave different; returns the changes sent.
fn send_changed_bytes<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    written: &[(u128, u128)],
    first_bytes: &mut Vec<u8>,
    second_bytes: &mut Vec<u8>,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (changes: Ghost<Seq<Assignment<Architecture::PagePermission>>>)
    requires
        well_formed(written@),
        separated(written@),
        forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_of(first@)(address).0 == memory_of(second@)(address).0,
    ensures
        receives(*old(mutation_hook), *final(mutation_hook), changes@),
        forall|address: u64| #[trigger] apply_all(memory_of(first@), changes@)(address)
            == (memory_of(second@)(address).0, memory_of(first@)(address).1),
        memory_of(first@) == memory_of(second@) ==> changes@.len() == 0 && *final(mutation_hook) == *old(mutation_hook),
{
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    let ghost hook_before = *mutation_hook;
    proof {
        assert(values_up_to(memory_first, memory_second, apply_all(memory_first, Seq::empty()), written@,
            if 0 < written@.len() { written@[0].0 as int } else { address_count() })) by {
            if written@.len() > 0 {
                assert forall|address: u64| address < written@[0].0 implies !#[trigger] in_ranges(written@, address) by {
                    lemma_before_ranges(written@, address);
                }
            }
        }
    }
    let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
    let mut range_index: usize = 0;
    while range_index < written.len()
        invariant
            range_index <= written@.len(),
            well_formed(written@),
            separated(written@),
            forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_first(address).0 == memory_second(address).0,
            values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@,
                if range_index < written@.len() { written@[range_index as int].0 as int } else { address_count() }),
            receives(hook_before, *mutation_hook, changes),
            memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
            memory_first == memory_of(first@),
            memory_second == memory_of(second@),
        decreases written@.len() - range_index,
    {
        let (range_start, range_end) = written[range_index];
        // VERUS: a range of every address has no length a `usize` holds, so a range is compared in pieces of at most `usize::MAX` bytes.
        let mut piece_start = range_start;
        while piece_start < range_end
            invariant
                range_index < written@.len(),
                written@[range_index as int] == (range_start, range_end),
                range_start <= piece_start <= range_end <= address_count(),
                well_formed(written@),
                separated(written@),
                forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_first(address).0 == memory_second(address).0,
                values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, piece_start as int),
                receives(hook_before, *mutation_hook, changes),
                memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
                memory_first == memory_of(first@),
                memory_second == memory_of(second@),
            decreases range_end - piece_start,
        {
            let piece_end = if range_end - piece_start > usize::MAX as u128 { piece_start + usize::MAX as u128 } else { range_end };
            let start = piece_start as u64;
            let length = (piece_end - piece_start) as usize;
            resolve_bytes(first, start, length, first_bytes);
            resolve_bytes(second, start, length, second_bytes);
            // Each run of bytes the two lists leave different is one mutation.
            let mut run_end: usize = 0;
            while run_end < length
                invariant
                    run_end <= length,
                    start as int + length as int <= range_end as int,
                    range_end as int <= address_count(),
                    start as int == piece_start as int,
                    range_start <= piece_start,
                    range_index < written@.len(),
                    written@[range_index as int] == (range_start, range_end),
                    first_bytes@.len() == length,
                    second_bytes@.len() == length,
                    bytes_resolved(memory_first, first_bytes@, start),
                    bytes_resolved(memory_second, second_bytes@, start),
                    values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, start + run_end),
                    receives(hook_before, *mutation_hook, changes),
                    memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
                decreases length - run_end,
            {
                let ghost entered = run_end;
                let chunk_end = if length - run_end > COMPARED_AT_ONCE { run_end + COMPARED_AT_ONCE } else { length };
                let first_chunk = vstd::slice::slice_subrange(first_bytes.as_slice(), run_end, chunk_end);
                let second_chunk = vstd::slice::slice_subrange(second_bytes.as_slice(), run_end, chunk_end);
                if *first_chunk == *second_chunk {
                    proof {
                        assert forall|at: int| run_end <= at < chunk_end implies
                            #[trigger] memory_first((start + at) as u64).0 == memory_second((start + at) as u64).0 by {
                            assert(first_chunk@[at - run_end] == second_chunk@[at - run_end]);
                            assert(first_bytes@[at] == memory_first((start + at) as u64).0);
                            assert(second_bytes@[at] == memory_second((start + at) as u64).0);
                        }
                        lemma_same_bytes_passed(memory_first, memory_second, apply_all(memory_first, changes), written@,
                            range_index as int, start, run_end as int, chunk_end as int);
                    }
                    run_end = chunk_end;
                } else {
                    while run_end < chunk_end && first_bytes[run_end] == second_bytes[run_end]
                        invariant
                            entered <= run_end <= chunk_end <= length,
                            entered < chunk_end,
                            run_end <= length,
                            start as int + length as int <= range_end as int,
                            range_end as int <= address_count(),
                            start as int == piece_start as int,
                            range_start <= piece_start,
                            range_index < written@.len(),
                            written@[range_index as int] == (range_start, range_end),
                            first_bytes@.len() == length,
                            second_bytes@.len() == length,
                            bytes_resolved(memory_first, first_bytes@, start),
                            bytes_resolved(memory_second, second_bytes@, start),
                            values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, start + run_end),
                            receives(hook_before, *mutation_hook, changes),
                            memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
                        decreases chunk_end - run_end,
                    {
                        proof {
                            assert(first_bytes@[run_end as int] == memory_first((start + run_end) as u64).0);
                            assert(second_bytes@[run_end as int] == memory_second((start + run_end) as u64).0);
                            assert(range_start <= start + run_end < range_end);
                            lemma_same_byte_passed(memory_first, memory_second, apply_all(memory_first, changes), written@,
                                range_index as int, (start + run_end) as u64);
                        }
                        run_end += 1;
                    }
                    if run_end < chunk_end {
                        let run_start = run_end;
                        // The byte at the start differs, which the loop above found.
                        run_end += 1;
                        while run_end < length && first_bytes[run_end] != second_bytes[run_end]
                            invariant
                                entered <= run_start < run_end <= length,
                                first_bytes@.len() == length,
                                second_bytes@.len() == length,
                                forall|at: int| run_start <= at < run_end ==> first_bytes@[at] != second_bytes@[at],
                            decreases length - run_end,
                        {
                            run_end += 1;
                        }
                        let ghost hook_now = *mutation_hook;
                        let mutation = MemoryDelta::ValueAssignment {
                            start: start + run_start as u64,
                            bytes: Cow::Borrowed(vstd::slice::slice_subrange(second_bytes.as_slice(), run_start, run_end)),
                        };
                        let ghost change = mutation@;
                        mutation_hook.on_memory_mutated(mutation);
                        proof {
                            lemma_run_written(memory_first, memory_second, apply_all(memory_first, changes), written@, range_index as int,
                                start, second_bytes@, run_start as int, run_end as int, change);
                            lemma_receives_one(hook_before, hook_now, *mutation_hook, changes, change);
                            assert(apply_all(memory_first, changes.push(change)) == apply(apply_all(memory_first, changes), change)) by {
                                assert(changes.push(change).drop_last() =~= changes);
                            }
                            if memory_first == memory_second {
                                assert(first_bytes@[run_start as int] == memory_first((start + run_start) as u64).0);
                                assert(second_bytes@[run_start as int] == memory_second((start + run_start) as u64).0);
                            }
                            changes = changes.push(change);
                        }
                    }
                }
            }
            piece_start = piece_end;
        }
        proof {
            if range_index + 1 < written@.len() {
                assert(written@[range_index as int].1 < written@[range_index + 1].0);
                assert forall|address: u64| range_end <= address < written@[range_index + 1].0 implies !#[trigger] in_ranges(written@, address) by {
                    lemma_between_ranges(written@, range_index as int, address);
                }
                lemma_cursor_skip(memory_first, memory_second, apply_all(memory_first, changes), written@, range_end as int,
                    written@[range_index + 1].0 as int);
            } else {
                assert forall|address: u64| range_end <= address < address_count() implies !#[trigger] in_ranges(written@, address) by {
                    lemma_between_ranges(written@, range_index as int, address);
                }
                lemma_cursor_skip(memory_first, memory_second, apply_all(memory_first, changes), written@, range_end as int, address_count());
            }
        }
        range_index += 1;
    }
    let ghost after_values = apply_all(memory_first, changes);
    proof {
        assert forall|address: u64| #[trigger] after_values(address) == (memory_second(address).0, memory_first(address).1) by {
            assert(after_values(address)
                == if in_ranges(written@, address) && address < address_count() { (memory_second(address).0, memory_first(address).1) } else { memory_first(address) });
        }
    }
    Ghost(changes)
}

/// Sends a mutation for each interval between neighboring bounds of the two lists' permissions where they leave
/// different ones; returns the changes sent, which turn the second list's bytes with the first's permissions into the
/// second list's memory.
fn send_changed_permissions<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> (changes: Ghost<Seq<Assignment<Architecture::PagePermission>>>)
    ensures
        receives(*old(mutation_hook), *final(mutation_hook), changes@),
        apply_all(|address: u64| (memory_of(second@)(address).0, memory_of(first@)(address).1), changes@) == memory_of(second@),
        memory_of(first@) == memory_of(second@) ==> changes@.len() == 0 && *final(mutation_hook) == *old(mutation_hook),
{
    proof {
        Architecture::equality_is_identity();
    }
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    let ghost hook_before = *mutation_hook;
    let ghost after_values = |address: u64| (memory_second(address).0, memory_first(address).1);
    let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
    proof {
        assert(apply_all(after_values, changes) == after_values);
    }
    let first_permissions = resolve_permissions(first);
    let second_permissions = resolve_permissions(second);
    // VERUS: no iterator chain over the range maps, so each map's bounds are appended to one list made to hold them.
    let mut bounds: Vec<u64> =
        Vec::with_capacity(range_map_len(&first_permissions).saturating_add(range_map_len(&second_permissions)).saturating_mul(2));
    append_bounds(&first_permissions, &mut bounds);
    let ghost first_bounds = bounds@;
    proof {
        assert(bounds@.skip(0) =~= first_bounds);
    }
    append_bounds(&second_permissions, &mut bounds);
    let ghost second_bounds = bounds@.skip(first_bounds.len() as int);
    let ghost unsorted_bounds = bounds@;
    proof {
        broadcast use vstd::laws_cmp::group_laws_cmp;
    }
    bounds.as_mut_slice().sort_unstable();
    let ghost sorted_bounds = bounds@;
    // VERUS: no `dedup`, so repeated bounds are dropped in place by index.
    dedup_sorted(&mut bounds);
    proof {
        assert forall|at: int| 0 <= at < first_bounds.len() implies #[trigger] first_bounds[at] == unsorted_bounds[0 + at] by {
            assert(first_bounds =~= unsorted_bounds.subrange(0, first_bounds.len() as int));
        }
        lemma_bounds_kept(first_bounds, 0, unsorted_bounds, sorted_bounds, bounds@);
        lemma_bounds_kept(second_bounds, first_bounds.len() as int, unsorted_bounds, sorted_bounds, bounds@);
        lemma_values_between(range_map_values(&first_permissions), first_bounds, bounds@);
        lemma_values_between(range_map_values(&second_permissions), second_bounds, bounds@);
    }
    // Between two neighboring bounds each list leaves one permission.
    // VERUS: no `for` over a consumed iterator whose first item is taken apart, so the bounds are walked by index.
    if bounds.is_empty() {
        proof {
            assert(permissions_up_to(memory_first, memory_second, after_values, 0, 0));
            lemma_permissions_outside::<Architecture>(memory_first, memory_second, after_values, first_permissions, second_permissions,
                bounds@, 0, 0);
            assert(apply_all(after_values, changes) == memory_second);
            assert(after_values =~= (|address: u64| (memory_of(second@)(address).0, memory_of(first@)(address).1)));
        }
        return Ghost(changes);
    }
    let ghost low = bounds@[0] as int;
    let mut bound_index: usize = 1;
    while bound_index < bounds.len()
        invariant
            1 <= bound_index <= bounds@.len(),
            low == bounds@[0] as int,
            strictly_ascending(bounds@),
            permissions_resolved::<Architecture>(memory_first, first_permissions),
            permissions_resolved::<Architecture>(memory_second, second_permissions),
            values_by_bounds(range_map_values(&first_permissions), bounds@),
            values_by_bounds(range_map_values(&second_permissions), bounds@),
            permissions_up_to(memory_first, memory_second, apply_all(after_values, changes), low, bounds@[bound_index - 1] as int),
            receives(hook_before, *mutation_hook, changes),
            memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
            memory_first == memory_of(first@),
            memory_second == memory_of(second@),
            obeys_concrete_eq::<Architecture::PagePermission>(),
        decreases bounds@.len() - bound_index,
    {
        let addresses = bounds[bound_index - 1]..bounds[bound_index];
        let permission = read_permission::<Architecture>(&second_permissions, addresses.start);
        let first_permission = read_permission::<Architecture>(&first_permissions, addresses.start);
        let differ = first_permission != permission;
        proof {
            reveal(obeys_concrete_eq);
            assert(differ <==> first_permission != permission);
            assert(bounds@[bound_index - 1] < bounds@[bound_index as int]);
            assert(memory_first(bounds@[bound_index - 1]).1 == first_permission);
            assert(memory_second(bounds@[bound_index - 1]).1 == permission);
        }
        if differ {
            let ghost hook_now = *mutation_hook;
            let mutation = MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            };
            let ghost change = mutation@;
            mutation_hook.on_memory_mutated(mutation);
            proof {
                lemma_receives_one(hook_before, hook_now, *mutation_hook, changes, change);
                assert(apply_all(after_values, changes.push(change)) == apply(apply_all(after_values, changes), change)) by {
                    assert(changes.push(change).drop_last() =~= changes);
                }
                lemma_permission_interval::<Architecture>(memory_first, memory_second, apply_all(after_values, changes),
                    apply_all(after_values, changes.push(change)), first_permissions, second_permissions, bounds@,
                    bound_index as int, low, true);
                changes = changes.push(change);
            }
        } else {
            proof {
                lemma_permission_interval::<Architecture>(memory_first, memory_second, apply_all(after_values, changes),
                    apply_all(after_values, changes), first_permissions, second_permissions, bounds@, bound_index as int, low, false);
            }
        }
        bound_index += 1;
    }
    proof {
        lemma_permissions_outside::<Architecture>(memory_first, memory_second, apply_all(after_values, changes), first_permissions,
            second_permissions, bounds@, low, bounds@[bounds@.len() - 1] as int);
        assert(apply_all(after_values, changes) == memory_second);
        assert(after_values =~= (|address: u64| (memory_of(second@)(address).0, memory_of(first@)(address).1)));
    }
    Ghost(changes)
}

/// Returns whether the two mutations assign the same thing to the same addresses, the same slice of bytes first.
fn is_same_mutation<Architecture: ProcessorArchitecture>(
    first: &MemoryDelta<'_, Architecture>,
    second: &MemoryDelta<'_, Architecture>,
) -> (same: bool)
    requires
        obeys_concrete_eq::<Architecture::PagePermission>(),
    ensures
        same ==> first@ == second@,
{
    proof {
        reveal(obeys_concrete_eq);
    }
    match (first, second) {
        (
            MemoryDelta::PermissionAssignment {
                addresses: first_addresses,
                permission: first_permission,
            },
            MemoryDelta::PermissionAssignment {
                addresses: second_addresses,
                permission: second_permission,
            },
        ) => {
            // VERUS: no `==` on ranges, so their bounds are compared.
            first_addresses.start == second_addresses.start && first_addresses.end == second_addresses.end
                && first_permission == second_permission
        },
        (
            MemoryDelta::ValueAssignment {
                start: first_start,
                bytes: first_bytes,
            },
            MemoryDelta::ValueAssignment {
                start: second_start,
                bytes: second_bytes,
            },
        ) => {
            let (first_slice, second_slice) = (cow_bytes(first_bytes), cow_bytes(second_bytes));
            let same = *first_start == *second_start
                && (is_same_slice(first_slice, second_slice) || *first_slice == *second_slice);
            proof {
                if same {
                    assert(first_slice@ =~= second_slice@);
                }
            }
            same
        },
        _ => false,
    }
}

// VERUS: no deref of a `Cow`, so its two cases are written out.
#[expect(clippy::ptr_arg, reason = "the verifier reads a `Cow` only by matching its two cases")]
fn cow_bytes<'bytes>(bytes: &'bytes Cow<'_, [u8]>) -> (slice: &'bytes [u8])
    ensures
        slice@ == bytes@,
{
    match bytes {
        Cow::Borrowed(borrowed) => borrowed,
        Cow::Owned(owned) => owned.as_slice(),
    }
}

/// Bytes the memory diff compares at once before looking at single ones.
const COMPARED_AT_ONCE: usize = 64;

/// One past the last address, where a value assignment goes on from zero.
// VERUS: no shift in a constant it evaluates, so the end is the last address plus one.
const ADDRESSES_END: u128 = u64::MAX as u128 + 1;

/// Records the addresses an unpaired mutation reaches: the bytes it writes, or that it assigns a permission.
// VERUS: no closure that captures a mutable reference, so what is recorded is passed in.
fn add_unpaired<Architecture: ProcessorArchitecture>(
    mutation: &MemoryDelta<'_, Architecture>,
    written: &mut Vec<(u128, u128)>,
    has_unpaired_permission: &mut bool,
)
    ensures
        records_unpaired(mutation@, old(written)@, *old(has_unpaired_permission), final(written)@, *final(has_unpaired_permission)),
{
    proof {
        assert(ADDRESSES_END == address_count());
    }
    let ghost before = written@;
    match mutation {
        MemoryDelta::ValueAssignment { start, bytes } => {
            let slice = cow_bytes(bytes);
            let end = *start as u128 + slice.len() as u128;
            // A write that runs past the last address goes on from zero, in a second range.
            // VERUS: no `filter` with a closure, so each of the two ranges is checked.
            let below_end = if end < ADDRESSES_END { end } else { ADDRESSES_END };
            if (*start as u128) < below_end {
                written.push((*start as u128, below_end));
            }
            let ghost first_pushed = written@;
            if end > ADDRESSES_END {
                written.push((0, end - ADDRESSES_END));
            }
            proof {
                assert(before.is_prefix_of(written@)) by {
                    assert(written@.take(before.len() as int) =~= before);
                }
                assert(well_formed(before) ==> well_formed(written@)) by {
                    if well_formed(before) {
                        assert forall|index: int| 0 <= index < written@.len() implies #[trigger] written@[index].0 < written@[index].1 <= address_count() by {
                            if index >= before.len() {
                                assert(end <= ADDRESSES_END + ADDRESSES_END);
                            } else {
                                assert(written@[index] == before[index]);
                            }
                        }
                    }
                }
                assert forall|address: u64| #[trigger] writes_byte(mutation@, address) implies in_ranges(written@, address) by {
                    assert(slice@ == bytes@);
                    if *start <= address {
                        assert(address < end && (address as u128) < below_end);
                        let at = before.len() as int;
                        assert(first_pushed[at] == (*start as u128, below_end));
                        assert(written@[at] == (*start as u128, below_end));
                        assert(written@[at].0 <= address < written@[at].1);
                    } else {
                        assert(end > ADDRESSES_END);
                        let at = first_pushed.len() as int;
                        assert(written@[at] == (0u128, (end - ADDRESSES_END) as u128));
                        assert(written@[at].0 <= address < written@[at].1);
                    }
                }
            }
        },
        MemoryDelta::PermissionAssignment { addresses, .. } => {
            *has_unpaired_permission = *has_unpaired_permission || addresses.start < addresses.end;
            proof {
                assert(before.is_prefix_of(written@)) by {
                    assert(written@.take(before.len() as int) =~= before);
                }
            }
        },
    }
}

/// Joins each range that overlaps or touches the one before into it.
fn merge_ranges(written: &mut Vec<(u128, u128)>)
    requires
        well_formed(old(written)@),
        starts_ascending(old(written)@),
    ensures
        well_formed(final(written)@),
        separated(final(written)@),
        forall|address: u64| #[trigger] in_ranges(final(written)@, address) == in_ranges(old(written)@, address),
{
    let ghost sorted = written@;
    let mut kept: usize = 0;
    let mut next: usize = 0;
    while next < written.len()
        invariant
            kept <= next <= written@.len(),
            written@.len() == sorted.len(),
            well_formed(sorted),
            starts_ascending(sorted),
            forall|at: int| next <= at < sorted.len() ==> #[trigger] written@[at] == sorted[at],
            well_formed(written@.take(kept as int)),
            separated(written@.take(kept as int)),
            kept > 0 ==> forall|at: int| next <= at < sorted.len() ==> written@[kept - 1].0 <= #[trigger] sorted[at].0,
            forall|address: u64| #[trigger] in_ranges(written@.take(kept as int), address) == in_ranges(sorted.take(next as int), address),
        decreases sorted.len() - next,
    {
        let ghost merged_before = written@.take(kept as int);
        let (start, end) = written[next];
        if kept > 0 && start <= written[kept - 1].1 {
            let (merged_start, merged_end) = written[kept - 1];
            written[kept - 1] = (merged_start, if end > merged_end { end } else { merged_end });
            proof {
                assert(written@.take(kept as int) =~= merged_before.update(kept - 1, (merged_start, if end > merged_end { end } else { merged_end })));
                lemma_merge_into_last(merged_before, sorted, next as int, (merged_start, if end > merged_end { end } else { merged_end }));
            }
        } else {
            written[kept] = (start, end);
            proof {
                assert(written@.take(kept + 1) =~= merged_before.push(sorted[next as int]));
                lemma_append_range(merged_before, sorted, next as int);
                assert forall|at: int| next < at < sorted.len() implies written@[kept as int].0 <= #[trigger] sorted[at].0 by {
                    assert(sorted[next as int].0 <= sorted[at].0);
                }
            }
            kept += 1;
        }
        next += 1;
    }
    written.truncate(kept);
    proof {
        assert(sorted.take(sorted.len() as int) =~= sorted);
    }
}

/// Drops each bound equal to the one before.
fn dedup_sorted(bounds: &mut Vec<u64>)
    requires
        sorted_by_le(old(bounds)@, |first: u64, second: u64| cmp_le(first, second)),
    ensures
        strictly_ascending(final(bounds)@),
        forall|bound: u64| #[trigger] final(bounds)@.contains(bound) == old(bounds)@.contains(bound),
{
    let ghost sorted = bounds@;
    let mut kept: usize = 0;
    let mut next: usize = 0;
    while next < bounds.len()
        invariant
            kept <= next <= bounds@.len(),
            bounds@.len() == sorted.len(),
            sorted_by_le(sorted, |first: u64, second: u64| cmp_le(first, second)),
            forall|at: int| next <= at < sorted.len() ==> #[trigger] bounds@[at] == sorted[at],
            strictly_ascending(bounds@.take(kept as int)),
            kept > 0 ==> forall|at: int| next <= at < sorted.len() ==> bounds@[kept - 1] <= #[trigger] sorted[at],
            forall|bound: u64| #[trigger] bounds@.take(kept as int).contains(bound) == sorted.take(next as int).contains(bound),
        decreases sorted.len() - next,
    {
        let ghost kept_before = bounds@.take(kept as int);
        let bound = bounds[next];
        if kept == 0 || bounds[kept - 1] != bound {
            bounds[kept] = bound;
            proof {
                assert(bounds@.take(kept + 1) =~= kept_before.push(sorted[next as int]));
                lemma_append_bound(kept_before, sorted, next as int);
                assert forall|at: int| next < at < sorted.len() implies bounds@[kept as int] <= #[trigger] sorted[at] by {
                    assert(cmp_le(sorted[next as int], sorted[at]));
                }
            }
            kept += 1;
        } else {
            proof {
                lemma_repeat_bound(kept_before, sorted, next as int);
            }
        }
        next += 1;
    }
    bounds.truncate(kept);
    proof {
        assert(sorted.take(sorted.len() as int) =~= sorted);
    }
}

/// Copies `count` bytes from `from` on into the buffer from `at` on.
fn copy_into(buffer: &mut Vec<u8>, at: usize, bytes: &[u8], from: usize, count: usize)
    requires
        at + count <= old(buffer)@.len(),
        from + count <= bytes@.len(),
        at + count <= usize::MAX,
        from + count <= usize::MAX,
    ensures
        final(buffer)@.len() == old(buffer)@.len(),
        forall|index: int| 0 <= index < old(buffer)@.len() ==> #[trigger] final(buffer)@[index]
            == if at <= index < at + count { bytes@[from + index - at] } else { old(buffer)@[index] },
{
    buffer.as_mut_slice()[at..at + count].copy_from_slice(&bytes[from..from + count]);
}

/// Writes into `rendered` what the mutations leave at its addresses from `start` on, which is zero where none wrote.
fn resolve_bytes<Architecture: ProcessorArchitecture>(
    mutations: &[MemoryDelta<'_, Architecture>],
    start: u64,
    length: usize,
    rendered: &mut Vec<u8>,
)
    requires
        start as int + length as int <= address_count(),
    ensures
        final(rendered)@.len() == length,
        bytes_resolved(memory_of(mutations@), final(rendered)@, start),
{
    rendered.clear();
    rendered.resize(length, 0u8);
    proof {
        assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(bytes_resolved(memory_of(mutations@.take(0)), rendered@, start));
    }
    let mut index: usize = 0;
    while index < mutations.len()
        invariant
            index <= mutations@.len(),
            rendered@.len() == length,
            start as int + length as int <= address_count(),
            bytes_resolved(memory_of(mutations@.take(index as int)), rendered@, start),
        decreases mutations@.len() - index,
    {
        let ghost before = rendered@;
        if let MemoryDelta::ValueAssignment {
            start: mutation_start,
            bytes,
        } = &mutations[index]
        {
            let bytes = cow_bytes(bytes);
            // Addresses wrap after the last one, so the offsets are taken modulo 2^64.
            let rendered_start_in_bytes = start.wrapping_sub(*mutation_start);
            let bytes_start_in_rendered = mutation_start.wrapping_sub(start);
            if rendered_start_in_bytes < bytes.len() as u64 {
                let available = bytes.len() - rendered_start_in_bytes as usize;
                let copied = if available < length { available } else { length };
                copy_into(rendered, 0, bytes, rendered_start_in_bytes as usize, copied);
            }
            let ghost after_first = rendered@;
            // VERUS: proven only with this: a write that wraps past the last address can reach the end of the window too.
            if bytes_start_in_rendered != 0 && bytes_start_in_rendered < length as u64 {
                let available = length - bytes_start_in_rendered as usize;
                let copied = if bytes.len() < available { bytes.len() } else { available };
                copy_into(rendered, bytes_start_in_rendered as usize, bytes, 0, copied);
            }
            proof {
                lemma_copy_window(before, after_first, rendered@, start, *mutation_start, bytes@, length as int);
            }
        }
        proof {
            lemma_memory_of_take(mutations@, index as int);
            let memory = memory_of(mutations@.take(index as int));
            let change = mutations@[index as int]@;
            assert forall|at: int| 0 <= at < rendered@.len() implies #[trigger] rendered@[at] == apply(memory, change)((start + at) as u64).0 by {
                assert(before[at] == memory((start + at) as u64).0);
            }
        }
        index += 1;
    }
    proof {
        assert(mutations@.take(mutations@.len() as int) =~= mutations@);
    }
}

/// Returns the permission of every range of addresses once the mutations are applied, keyed by address range.
fn resolve_permissions<Architecture: ProcessorArchitecture>(
    mutations: &[MemoryDelta<'_, Architecture>],
) -> (rendered: RangeMap<u64, Architecture::PagePermission>)
    requires
        obeys_concrete_eq::<Architecture::PagePermission>(),
    ensures
        permissions_resolved::<Architecture>(memory_of(mutations@), rendered),
{
    let mut rendered = new_range_map();
    proof {
        assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(assignments(mutations@.take(0)) =~= Seq::empty());
    }
    let mut index: usize = 0;
    while index < mutations.len()
        invariant
            index <= mutations@.len(),
            obeys_concrete_eq::<Architecture::PagePermission>(),
            permissions_resolved::<Architecture>(memory_of(mutations@.take(index as int)), rendered),
        decreases mutations@.len() - index,
    {
        let ghost before = rendered;
        // VERUS: no `let` in a condition, so the empty check is an `if` of its own.
        #[expect(clippy::collapsible_if, reason = "the verifier reads no `let` chain")]
        if let MemoryDelta::PermissionAssignment {
            addresses,
            permission,
        } = &mutations[index]
        {
            if addresses.start < addresses.end {
                insert_range(&mut rendered, addresses.start..addresses.end, *permission);
            }
        }
        proof {
            lemma_memory_of_take(mutations@, index as int);
            let memory = memory_of(mutations@.take(index as int));
            assert forall|address: u64| #[trigger] apply(memory, mutations@[index as int]@)(address).1
                == permission_or_default::<Architecture>(range_map_values(&rendered)(address)) by {
                assert(memory(address).1 == permission_or_default::<Architecture>(range_map_values(&before)(address)));
            }
        }
        index += 1;
    }
    proof {
        assert(mutations@.take(mutations@.len() as int) =~= mutations@);
    }
    rendered
}

/// The permission the map gives the address, or the default where it gives none.
fn read_permission<Architecture: ProcessorArchitecture>(
    permissions: &RangeMap<u64, Architecture::PagePermission>,
    address: u64,
) -> (permission: Architecture::PagePermission)
    ensures
        permission == permission_or_default::<Architecture>(range_map_values(permissions)(address)),
{
    match range_map_get(permissions, address) {
        Some(permission) => permission,
        None => Architecture::DEFAULT_PERMISSION,
    }
}

// A closure promises nothing to a specification, so calling it is trusted.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
impl<Architecture: ProcessorArchitecture, Hook: FnMut(MemoryDelta<'_, Architecture>)>
    MemorySink<Architecture> for Hook
{
    open spec fn obeys_sink_spec() -> bool {
        false
    }

    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        memory
    }

    #[cfg_attr(verus_keep_ghost, verus::trusted)]
    #[verifier::external_body]
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self(mutation)
    }
}

} // verus!
