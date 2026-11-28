#![no_std]

use thiserror::Error;
use vstd::prelude::*;

verus! {

#[allow(
    reviewable::explicit_shared_error::DuplicateError,
    reason = "duplicating within a table and from another table fail in the same ways"
)]
pub trait CapabilityTable<Identifier, Capability> {
    type Config;

    type Error;

    spec fn state(&self) -> Map<Identifier, Capability>;

    fn create(&mut self, identifier: Identifier, config: Self::Config) -> (created: Result<
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

    fn get(&self, identifier: &Identifier) -> (capability: Option<&Capability>)
        ensures
            match capability {
                Some(capability) => self.state().contains_key(*identifier) && *capability
                    == self.state().index(*identifier),
                None => !self.state().contains_key(*identifier),
            },
    ;

    fn drop(&mut self, identifier: &Identifier)
        ensures
            final(self).state() == old(self).state().remove(*identifier),
    ;

    fn duplicate_within(
        &mut self,
        identifier: &Identifier,
        new_identifier: Identifier,
    ) -> (duplicated: Result<(), DuplicateError<Self::Error>>)
        ensures
            match duplicated {
                Ok(_) => old(self).state().contains_key(*identifier) && !old(
                    self,
                ).state().contains_key(new_identifier) && final(self).state() == old(
                    self,
                ).state().insert(new_identifier, old(self).state().index(*identifier)),
                Err(DuplicateError::NothingHeld) => !old(self).state().contains_key(*identifier)
                    && final(self).state() == old(self).state(),
                Err(DuplicateError::IdentifierTaken) => old(self).state().contains_key(*identifier)
                    && old(self).state().contains_key(new_identifier) && final(self).state() == old(
                    self,
                ).state(),
                Err(DuplicateError::Failed(_)) => old(self).state().contains_key(*identifier)
                    && !old(self).state().contains_key(new_identifier) && final(self).state()
                    == old(self).state(),
            },
    ;

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &Identifier,
        new_identifier: Identifier,
    ) -> (duplicated: Result<(), DuplicateError<Self::Error>>)
        ensures
            match duplicated {
                Ok(_) => other.state().contains_key(*identifier) && !old(self).state().contains_key(
                    new_identifier,
                ) && final(self).state() == old(self).state().insert(
                    new_identifier,
                    other.state().index(*identifier),
                ),
                Err(DuplicateError::NothingHeld) => !other.state().contains_key(*identifier)
                    && final(self).state() == old(self).state(),
                Err(DuplicateError::IdentifierTaken) => other.state().contains_key(*identifier)
                    && old(self).state().contains_key(new_identifier) && final(self).state() == old(
                    self,
                ).state(),
                Err(DuplicateError::Failed(_)) => other.state().contains_key(*identifier) && !old(
                    self,
                ).state().contains_key(new_identifier) && final(self).state() == old(self).state(),
            },
    ;
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum CreateError<Error> {
    #[error("the identifier already holds a capability")]
    IdentifierTaken,
    #[error(transparent)]
    Failed(Error),
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum DuplicateError<Error> {
    #[error("no capability is held under the identifier")]
    NothingHeld,
    #[error("the new identifier already holds a capability")]
    IdentifierTaken,
    #[error(transparent)]
    Failed(Error),
}

} // verus!
