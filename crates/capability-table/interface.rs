#![no_std]

use vstd::prelude::*;

verus! {

pub trait CapabilityTable {
    type Identifier;

    type Config;

    type Capability;

    type Error;

    spec fn state(&self) -> Map<Self::Identifier, Self::Capability>;

    fn create(&mut self, identifier: Self::Identifier, config: Self::Config) -> (created: Result<
        (),
        CreateError<Self::Error>,
    >)
        ensures
            match created {
                Ok(_) => !old(self).state().contains_key(identifier)
                    && final(self).state().contains_key(identifier) && final(self).state().remove(
                    identifier,
                ) == old(self).state(),
                Err(CreateError::IdentifierTaken) => old(self).state().contains_key(identifier)
                    && final(self).state() == old(self).state(),
                Err(CreateError::Failed(_)) => !old(self).state().contains_key(identifier)
                    && final(self).state() == old(self).state(),
            },
    ;

    fn get(&self, identifier: &Self::Identifier) -> (capability: Option<&Self::Capability>)
        ensures
            match capability {
                Some(capability) => self.state().contains_key(*identifier) && *capability
                    == self.state().index(*identifier),
                None => !self.state().contains_key(*identifier),
            },
    ;

    fn drop(&mut self, identifier: &Self::Identifier)
        ensures
            final(self).state() == old(self).state().remove(*identifier),
    ;
}

pub enum CreateError<Error> {
    IdentifierTaken,
    Failed(Error),
}

} // verus!
