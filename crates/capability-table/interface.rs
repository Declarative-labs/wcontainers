use vstd::prelude::*;
use wcontainers_kernel::TaskId;

verus! {

#[verifier::external_type_specification]
pub struct ExTaskId(pub TaskId);

pub trait CapabilityTable {
    type Identifier;
    type Capability;

    spec fn state(&self) -> Map<(TaskId, Self::Identifier), Self::Capability>;

    fn get(&self, task: TaskId, identifier: &Self::Identifier) -> (capability: Option<&Self::Capability>)
        ensures
            match capability {
                Some(capability) => self.state().contains_key((task, *identifier)) && *capability == self.state().index((task, *identifier)),
                None => !self.state().contains_key((task, *identifier)),
            },
    ;

    fn put(&mut self, task: TaskId, identifier: Self::Identifier, capability: Self::Capability) -> (refused: Option<Self::Capability>)
        ensures
            match refused {
                Some(refused) => old(self).state().contains_key((task, identifier)) && refused == capability && final(self).state() == old(self).state(),
                None => !old(self).state().contains_key((task, identifier)) && final(self).state() == old(self).state().insert((task, identifier), capability),
            },
    ;

    fn drop(&mut self, task: TaskId, identifier: &Self::Identifier)
        ensures
            final(self).state() == old(self).state().remove((task, *identifier)),
    ;
}

}
