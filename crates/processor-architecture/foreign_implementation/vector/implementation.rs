use alloc::vec::Vec;

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::memory_proof::lemma_list_sink_push;
#[cfg(verus_keep_ghost)]
use crate::{Memory, apply_all, assignments};
use crate::{MemoryDelta, MemorySink, ProcessorArchitecture};

verus! {

// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'_, Architecture>>
{
    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        apply_all(memory, assignments(self@))
    }

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        // VERUS: no fact about the memory of a list one longer without its proof, so the proof is named.
        proof { lemma_list_sink_push(self@, mutation@); }
        self.push(mutation.into_owned())
    }
}

} // verus!
