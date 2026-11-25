use alloc::vec::Vec;

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::{Memory, apply_all, assignments};
use crate::{MemoryDelta, MemorySink, ProcessorArchitecture};

verus! {

/// Appends each mutation to the vector.
// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'_, Architecture>>
{
    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        apply_all(memory, assignments(self@))
    }

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self.push(mutation.into_owned());
        proof {
            assert(assignments(self@).drop_last() =~= assignments(old(self)@));
        }
    }
}

} // verus!
