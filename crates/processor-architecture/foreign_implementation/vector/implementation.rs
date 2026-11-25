use alloc::vec::Vec;

use crate::{MemoryDelta, MemorySink, ProcessorArchitecture};

/// Appends each mutation to the vector.
// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'_, Architecture>>
{
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self.push(mutation.into_owned())
    }
}
