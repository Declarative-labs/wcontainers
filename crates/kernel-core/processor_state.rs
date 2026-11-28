use vstd::prelude::*;

verus! {

pub trait TaskProcessorState<Task> {
    /// The registers of the processor as they were saved when a task last stopped running.
    type SavedProcessorState;

    spec fn saved_processor_state_of(task: Task) -> Self::SavedProcessorState;

    fn saved_processor_state(task: &Task) -> (saved_processor_state: &Self::SavedProcessorState)
        ensures
            *saved_processor_state == Self::saved_processor_state_of(*task),
    ;

    fn saved_processor_state_mut(task: &mut Task) -> (saved_processor_state:
        &mut Self::SavedProcessorState)
        ensures
            *saved_processor_state == Self::saved_processor_state_of(*old(task)),
            Self::saved_processor_state_of(*final(task)) == *final(saved_processor_state),
    ;
}

} // verus!
