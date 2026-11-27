#![no_std]

use vstd::prelude::*;
use wcontainers_kernel::TaskId;

verus! {

pub trait CapabilityTable {
    type Identifier;

    type Config;

    type Capability;

    type Error;

    spec fn state(&self) -> Map<(TaskId, Self::Identifier), Self::Capability>;

    fn create(
        &mut self,
        task: TaskId,
        identifier: Self::Identifier,
        config: Self::Config,
    ) -> (created: Result<(), CreateError<Self::Error>>)
        ensures
            match created {
                Ok(_) => !old(self).state().contains_key((task, identifier))
                    && final(self).state().contains_key((task, identifier))
                    && final(self).state().remove((task, identifier)) == old(self).state(),
                Err(CreateError::IdentifierTaken) => old(self).state().contains_key(
                    (task, identifier),
                ) && final(self).state() == old(self).state(),
                Err(CreateError::Failed(_)) => !old(self).state().contains_key((task, identifier))
                    && final(self).state() == old(self).state(),
            },
    ;

    fn get(&self, task: TaskId, identifier: &Self::Identifier) -> (capability: Option<
        &Self::Capability,
    >)
        ensures
            match capability {
                Some(capability) => self.state().contains_key((task, *identifier)) && *capability
                    == self.state().index((task, *identifier)),
                None => !self.state().contains_key((task, *identifier)),
            },
    ;

    fn drop(&mut self, task: TaskId, identifier: &Self::Identifier)
        ensures
            final(self).state() == old(self).state().remove((task, *identifier)),
    ;
}

pub enum CreateError<Error> {
    IdentifierTaken,
    Failed(Error),
}

} // verus!
