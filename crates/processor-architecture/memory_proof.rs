//! The proofs about the memory model that the methods of this crate use; nothing here is part of the specification.

use vstd::prelude::*;

use crate::*;

verus! {

/// Applying two lists of changes one after the other is applying the joined list.
pub proof fn lemma_apply_all_append<Permission>(memory: Memory<Permission>, first: Seq<Assignment<Permission>>, second: Seq<Assignment<Permission>>)
    ensures
        apply_all(memory, first + second) == apply_all(apply_all(memory, first), second),
    decreases second.len(),
{
    if second.len() == 0 {
        assert(first + second =~= first);
    } else {
        lemma_apply_all_append(memory, first, second.drop_last());
        assert((first + second).drop_last() =~= first + second.drop_last());
    }
}

/// The memory of a list with mutations appended is the memory of the list with their changes applied.
pub proof fn lemma_appended_memory<'memory, Architecture: ProcessorArchitecture>(
    memory: Seq<MemoryDelta<'memory, Architecture>>,
    added: Seq<MemoryDelta<'memory, Architecture>>,
)
    ensures
        memory_of(memory + added) == apply_all(memory_of(memory), assignments(added)),
{
    assert(assignments(memory + added) =~= assignments(memory) + assignments(added));
    lemma_apply_all_append(zeroed::<Architecture>(), assignments(memory), assignments(added));
}

/// No change applied leaves the memory as it was.
pub proof fn lemma_no_assignments<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
    memory: Memory<Architecture::PagePermission>,
)
    requires
        mutations.len() == 0,
    ensures
        apply_all(memory, assignments(mutations)) == memory,
{
    assert(assignments(mutations) =~= Seq::empty());
}

/// A list sink that starts empty and is sent changes that turn one memory into another stays empty exactly when the
/// two memories are the same, given that equal memories send it nothing.
pub proof fn lemma_diff_into_empty<'memory, Architecture: ProcessorArchitecture>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Vec<MemoryDelta<'memory, Architecture>>,
    after: Vec<MemoryDelta<'memory, Architecture>>,
)
    requires
        before@.len() == 0,
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second,
        first == second ==> after == before,
    ensures
        after@.len() == 0 <==> first == second,
{
    if after@.len() == 0 && first != second {
        let changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second;
        assert(after.applied_to(first) == apply_all(before.applied_to(first), changes));
        lemma_no_assignments(after@, first);
        lemma_no_assignments(before@, first);
    }
}

} // verus!
