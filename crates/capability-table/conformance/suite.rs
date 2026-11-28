//! The tests every implementation of wcontainers-capability-table runs, written against its trait alone.
//!
//! Each takes functions that make an empty table, give the identifier of a number, a config that creates the
//! capability of a number and one whose create fails, and tell a capability's number.

#![no_std]

use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};

// The number of the first identifier and capability a test uses.
const FIRST: u8 = u8::MIN;

// The number of the second identifier and capability a test uses.
const SECOND: u8 = u8::MAX;

// The functions an implementation gives the suite, as the crate's comment says.
type Value<T, Identifier, Capability> = (
    fn() -> T,
    fn(u8) -> Identifier,
    fn(u8) -> <T as CapabilityTable<Identifier, Capability>>::Config,
    fn() -> <T as CapabilityTable<Identifier, Capability>>::Config,
    fn(&Capability) -> u8,
);

// The functions an implementation serving two capability types gives the suite: the table, the identifiers, and the
// config and the number of each capability type.
type TwoTypes<T, Identifier, First, Second> = (
    fn() -> T,
    fn(u8) -> Identifier,
    fn(u8) -> <T as CapabilityTable<Identifier, First>>::Config,
    fn(&First) -> u8,
    fn(u8) -> <T as CapabilityTable<Identifier, Second>>::Config,
    fn(&Second) -> u8,
);

// Returns the number of the capability the table holds under the identifier, if it holds one.
fn lookup<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    table: &T,
    identifier: &Identifier,
    number: fn(&Capability) -> u8,
) -> Option<u8> {
    table.get(identifier).map(number)
}

// Returns a table holding the first capability under the first identifier, made by the value's functions.
fn filled<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: &Value<T, Identifier, Capability>,
) -> T {
    let (make, identifier, config, _, _) = value;
    let mut table = make();
    assert!(table.create(identifier(FIRST), config(FIRST)).is_ok());
    table
}

/// Checks that a create on an identifier holding nothing puts the capability of its config there.
pub fn creates_on_a_free_identifier<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let (make, identifier, config, _, number) = value;
    let mut table = make();
    assert!(matches!(table.create(identifier(FIRST), config(FIRST)), Ok(())));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a create on a taken identifier is refused, whatever its config, and keeps what the identifier holds.
pub fn refuses_to_create_on_a_taken_identifier<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, config, failing, number) = value;
    assert!(matches!(table.create(identifier(FIRST), config(SECOND)), Err(CreateError::IdentifierTaken)));
    assert!(matches!(table.create(identifier(FIRST), failing()), Err(CreateError::IdentifierTaken)));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a create whose config fails says so and leaves every identifier as it was.
pub fn changes_nothing_when_a_create_fails<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, _, failing, number) = value;
    assert!(matches!(table.create(identifier(SECOND), failing()), Err(CreateError::Failed(_))));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a table made anew lends nothing.
pub fn gets_nothing_where_nothing_was_created<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let (make, identifier, _, _, number) = value;
    let table = make();
    assert_eq!(lookup(&table, &identifier(FIRST), number), None);
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
}

/// Checks that a dropped identifier lends nothing and takes a new capability.
pub fn gets_nothing_after_a_drop<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, config, _, number) = value;
    table.drop(&identifier(FIRST));
    assert_eq!(lookup(&table, &identifier(FIRST), number), None);
    assert!(matches!(table.create(identifier(FIRST), config(SECOND)), Ok(())));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(SECOND));
}

/// Checks that two identifiers hold their own capabilities, and that dropping one leaves the other.
pub fn keeps_two_identifiers_apart<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, config, _, number) = value;
    assert!(matches!(table.create(identifier(SECOND), config(SECOND)), Ok(())));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(SECOND));
    table.drop(&identifier(FIRST));
    assert_eq!(lookup(&table, &identifier(FIRST), number), None);
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(SECOND));
}

/// Checks that dropping an identifier that holds nothing leaves every other identifier as it was.
pub fn drops_nothing_where_nothing_is_held<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, _, _, number) = value;
    table.drop(&identifier(SECOND));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a duplicate within the table puts the capability under the new identifier and keeps it under the old.
pub fn duplicates_within_the_table<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, _, _, number) = value;
    assert!(matches!(table.duplicate_within(&identifier(FIRST), identifier(SECOND)), Ok(())));
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(FIRST));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a duplicate within the table of an identifier holding nothing is refused, even onto a taken identifier.
pub fn refuses_to_duplicate_nothing_within_the_table<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, _, _, number) = value;
    assert!(matches!(
        table.duplicate_within(&identifier(SECOND), identifier(FIRST)),
        Err(DuplicateError::NothingHeld)
    ));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
}

/// Checks that a duplicate within the table onto a taken identifier, the held one itself too, is refused and keeps both.
pub fn refuses_to_duplicate_within_onto_a_taken_identifier<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, config, _, number) = value;
    assert!(matches!(table.create(identifier(SECOND), config(SECOND)), Ok(())));
    assert!(matches!(
        table.duplicate_within(&identifier(FIRST), identifier(SECOND)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert!(matches!(
        table.duplicate_within(&identifier(FIRST), identifier(FIRST)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(SECOND));
}

/// Checks that a duplicate within the table of a held capability onto a free identifier succeeds or changes nothing.
///
/// These are the only two outcomes the clauses leave it.
pub fn changes_nothing_when_a_duplicate_within_fails<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (_, identifier, _, _, number) = value;
    let expected = match table.duplicate_within(&identifier(FIRST), identifier(SECOND)) {
        Ok(()) => Some(FIRST),
        Err(DuplicateError::Failed(_)) => None,
        Err(DuplicateError::NothingHeld | DuplicateError::IdentifierTaken) => {
            panic!("a duplicate of a held capability onto a free identifier was refused")
        }
    };
    assert_eq!(lookup(&table, &identifier(SECOND), number), expected);
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a duplicate from another table puts the other's capability under the new identifier, and the other keeps it.
///
/// This table holds a capability of its own under the identifier duplicated, which must not be the one copied.
pub fn duplicates_from_another_table<T: CapabilityTable<Identifier, Capability>, Identifier, Capability>(
    value: Value<T, Identifier, Capability>,
) {
    let other = filled(&value);
    let (make, identifier, config, _, number) = value;
    let mut table = make();
    assert!(matches!(table.create(identifier(FIRST), config(SECOND)), Ok(())));
    assert!(matches!(table.duplicate_from(&other, &identifier(FIRST), identifier(SECOND)), Ok(())));
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(FIRST));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(SECOND));
    assert_eq!(lookup(&other, &identifier(FIRST), number), Some(FIRST));
    assert_eq!(lookup(&other, &identifier(SECOND), number), None);
}

/// Checks that a duplicate from another table holding nothing under the identifier is refused and changes nothing.
///
/// This table holds the identifier itself, which must not count.
pub fn refuses_to_duplicate_nothing_from_another_table<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (make, identifier, _, _, number) = value;
    let other = make();
    assert!(matches!(
        table.duplicate_from(&other, &identifier(FIRST), identifier(SECOND)),
        Err(DuplicateError::NothingHeld)
    ));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that a duplicate from another table onto an identifier this table holds is refused and keeps what it holds.
///
/// Only this table's identifiers count as taken: the other's do not.
pub fn refuses_to_duplicate_from_onto_a_taken_identifier<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut other = filled(&value);
    let (make, identifier, config, _, number) = value;
    assert!(matches!(other.create(identifier(SECOND), config(SECOND)), Ok(())));
    let mut table = make();
    assert!(matches!(table.create(identifier(FIRST), config(SECOND)), Ok(())));
    assert!(matches!(
        table.duplicate_from(&other, &identifier(SECOND), identifier(FIRST)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(SECOND));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
    let mut empty = make();
    assert!(matches!(empty.duplicate_from(&other, &identifier(FIRST), identifier(SECOND)), Ok(())));
    assert_eq!(lookup(&empty, &identifier(SECOND), number), Some(FIRST));
}

/// Checks that a duplicate from another table of a held capability onto a free identifier succeeds or changes nothing.
///
/// These are the only two outcomes the clauses leave it.
pub fn changes_nothing_when_a_duplicate_from_fails<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let other = filled(&value);
    let (make, identifier, config, _, number) = value;
    let mut table = make();
    assert!(matches!(table.create(identifier(SECOND), config(SECOND)), Ok(())));
    let expected = match table.duplicate_from(&other, &identifier(FIRST), identifier(FIRST)) {
        Ok(()) => Some(FIRST),
        Err(DuplicateError::Failed(_)) => None,
        Err(DuplicateError::NothingHeld | DuplicateError::IdentifierTaken) => {
            panic!("a duplicate of a held capability onto a free identifier was refused")
        }
    };
    assert_eq!(lookup(&table, &identifier(FIRST), number), expected);
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(SECOND));
}

/// Checks that a duplicate, within the table or from another, keeps its capability when the original is dropped.
pub fn keeps_a_duplicate_after_the_original_is_dropped<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (make, identifier, _, _, number) = value;
    assert!(matches!(table.duplicate_within(&identifier(FIRST), identifier(SECOND)), Ok(())));
    let mut copy = make();
    assert!(matches!(copy.duplicate_from(&table, &identifier(FIRST), identifier(FIRST)), Ok(())));
    table.drop(&identifier(FIRST));
    assert_eq!(lookup(&table, &identifier(FIRST), number), None);
    assert_eq!(lookup(&table, &identifier(SECOND), number), Some(FIRST));
    assert_eq!(lookup(&copy, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that the original keeps its capability when a duplicate of it, within the table or from another, is dropped.
pub fn keeps_the_original_after_a_duplicate_is_dropped<
    T: CapabilityTable<Identifier, Capability>,
    Identifier,
    Capability,
>(
    value: Value<T, Identifier, Capability>,
) {
    let mut table = filled(&value);
    let (make, identifier, _, _, number) = value;
    assert!(matches!(table.duplicate_within(&identifier(FIRST), identifier(SECOND)), Ok(())));
    let mut copy = make();
    assert!(matches!(copy.duplicate_from(&table, &identifier(FIRST), identifier(FIRST)), Ok(())));
    table.drop(&identifier(SECOND));
    copy.drop(&identifier(FIRST));
    assert_eq!(lookup(&table, &identifier(SECOND), number), None);
    assert_eq!(lookup(&copy, &identifier(FIRST), number), None);
    assert_eq!(lookup(&table, &identifier(FIRST), number), Some(FIRST));
}

/// Checks that one type serving two capability types keeps the clauses of each.
///
/// Each runs on a table made anew; on one table what one capability type does to the other's is unspecified, so there
/// the test takes every outcome the clauses allow.
pub fn serves_two_capability_types<
    T: CapabilityTable<Identifier, First> + CapabilityTable<Identifier, Second>,
    Identifier,
    First,
    Second,
>(
    value: TwoTypes<T, Identifier, First, Second>,
) {
    let (make, identifier, first_config, first_number, second_config, second_number) = value;
    let mut firsts = make();
    assert!(matches!(
        CapabilityTable::<Identifier, First>::create(&mut firsts, identifier(FIRST), first_config(FIRST)),
        Ok(())
    ));
    assert_eq!(lookup::<T, Identifier, First>(&firsts, &identifier(FIRST), first_number), Some(FIRST));
    let mut seconds = make();
    assert!(matches!(
        CapabilityTable::<Identifier, Second>::create(&mut seconds, identifier(FIRST), second_config(SECOND)),
        Ok(())
    ));
    assert_eq!(lookup::<T, Identifier, Second>(&seconds, &identifier(FIRST), second_number), Some(SECOND));
    CapabilityTable::<Identifier, Second>::drop(&mut seconds, &identifier(FIRST));
    assert_eq!(lookup::<T, Identifier, Second>(&seconds, &identifier(FIRST), second_number), None);
    let created = CapabilityTable::<Identifier, Second>::create(&mut firsts, identifier(SECOND), second_config(SECOND));
    let held = lookup::<T, Identifier, Second>(&firsts, &identifier(SECOND), second_number);
    match created {
        Ok(()) => assert_eq!(held, Some(SECOND)),
        Err(CreateError::IdentifierTaken) => assert!(held.is_some()),
        Err(CreateError::Failed(_)) => assert_eq!(held, None),
    }
}

/// Runs each suite function above as its own test on values made anew for every test.
///
/// The first value holds the functions the crate's comment names, the second those of a type serving two.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr, $two:expr) => {
        #[test]
        fn creates_on_a_free_identifier() {
            $crate::creates_on_a_free_identifier($value);
        }
        #[test]
        fn refuses_to_create_on_a_taken_identifier() {
            $crate::refuses_to_create_on_a_taken_identifier($value);
        }
        #[test]
        fn changes_nothing_when_a_create_fails() {
            $crate::changes_nothing_when_a_create_fails($value);
        }
        #[test]
        fn gets_nothing_where_nothing_was_created() {
            $crate::gets_nothing_where_nothing_was_created($value);
        }
        #[test]
        fn gets_nothing_after_a_drop() {
            $crate::gets_nothing_after_a_drop($value);
        }
        #[test]
        fn keeps_two_identifiers_apart() {
            $crate::keeps_two_identifiers_apart($value);
        }
        #[test]
        fn drops_nothing_where_nothing_is_held() {
            $crate::drops_nothing_where_nothing_is_held($value);
        }
        #[test]
        fn duplicates_within_the_table() {
            $crate::duplicates_within_the_table($value);
        }
        #[test]
        fn refuses_to_duplicate_nothing_within_the_table() {
            $crate::refuses_to_duplicate_nothing_within_the_table($value);
        }
        #[test]
        fn refuses_to_duplicate_within_onto_a_taken_identifier() {
            $crate::refuses_to_duplicate_within_onto_a_taken_identifier($value);
        }
        #[test]
        fn changes_nothing_when_a_duplicate_within_fails() {
            $crate::changes_nothing_when_a_duplicate_within_fails($value);
        }
        #[test]
        fn duplicates_from_another_table() {
            $crate::duplicates_from_another_table($value);
        }
        #[test]
        fn refuses_to_duplicate_nothing_from_another_table() {
            $crate::refuses_to_duplicate_nothing_from_another_table($value);
        }
        #[test]
        fn refuses_to_duplicate_from_onto_a_taken_identifier() {
            $crate::refuses_to_duplicate_from_onto_a_taken_identifier($value);
        }
        #[test]
        fn changes_nothing_when_a_duplicate_from_fails() {
            $crate::changes_nothing_when_a_duplicate_from_fails($value);
        }
        #[test]
        fn keeps_a_duplicate_after_the_original_is_dropped() {
            $crate::keeps_a_duplicate_after_the_original_is_dropped($value);
        }
        #[test]
        fn keeps_the_original_after_a_duplicate_is_dropped() {
            $crate::keeps_the_original_after_a_duplicate_is_dropped($value);
        }
        #[test]
        fn serves_two_capability_types() {
            $crate::serves_two_capability_types($two);
        }
    };
}
