//! The tests every implementation of wcontainers-capability-table runs, written against its trait alone.
//!
//! Each takes functions that make a table, name a number's identifier, give a config that makes its capability and one
//! that fails, tell a capability's number, and write and read a mark that only the same capability shares.

#![no_std]

use core::num::NonZeroU8;

use wcontainers_capability_table::{
    CapabilityConfig, CapabilityTable, CreateError, DuplicateError,
};

// The number of the first identifier and capability a test uses.
const FIRST: u8 = NonZeroU8::MIN.get();

// The number of the second identifier and capability a test uses.
const SECOND: u8 = u8::MAX;

// The number of the identifier and capability that stand by, which no test's operation names.
const THIRD: u8 = i8::MAX.unsigned_abs();

// The functions an implementation gives the suite, as the crate's comment says.
type Value<T, Identifier, Capability, Config> = (
    fn() -> T,
    fn(u8) -> Identifier,
    fn(u8) -> Config,
    fn() -> Config,
    fn(&Capability) -> u8,
    fn(&Capability, u8),
    fn(&Capability) -> u8,
);

// Returns the number and the mark of the capability the table holds under the identifier, if it holds one.
fn lookup<T, Identifier, Capability, Config, Error>(
    table: &T,
    value: &Value<T, Identifier, Capability, Config>,
    identifier: u8,
) -> Option<(u8, u8)>
where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let (_, name, _, _, number, _, read) = value;
    table
        .get(&name(identifier))
        .map(|capability| (number(capability), read(capability)))
}

// Writes the mark through the capability the table holds under the identifier.
fn mark<T, Identifier, Capability, Config, Error>(
    table: &T,
    value: &Value<T, Identifier, Capability, Config>,
    identifier: u8,
    mark: u8,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let (_, name, _, _, _, write, _) = value;
    write(
        table
            .get(&name(identifier))
            .expect("the table holds the identifier the test marks"),
        mark,
    );
}

// Creates the capability of the number under the identifier of the number, and marks it with the number.
fn create<T, Identifier, Capability, Config, Error>(
    table: &mut T,
    value: &Value<T, Identifier, Capability, Config>,
    number: u8,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let (_, name, config, _, _, _, _) = value;
    assert!(matches!(table.create(name(number), config(number)), Ok(())));
    mark(table, value, number, number);
}

// Returns a table holding the first and the third capability, each marked with its number.
fn filled<T, Identifier, Capability, Config, Error>(
    value: &Value<T, Identifier, Capability, Config>,
) -> T
where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = value.0();
    create(&mut table, value, FIRST);
    create(&mut table, value, THIRD);
    table
}

/// Checks that a create on an identifier holding nothing puts the capability of its config there and keeps the rest.
pub fn creates_on_a_free_identifier<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, config, _, _, _, _) = value;
    assert!(matches!(table.create(name(SECOND), config(SECOND)), Ok(())));
    mark(&table, &value, SECOND, SECOND);
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a create on a taken identifier is refused, whatever its config, and keeps every capability.
pub fn refuses_to_create_on_a_taken_identifier<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, config, failing, _, _, _) = value;
    assert!(matches!(
        table.create(name(FIRST), config(SECOND)),
        Err(CreateError::IdentifierTaken)
    ));
    assert!(matches!(
        table.create(name(FIRST), failing()),
        Err(CreateError::IdentifierTaken)
    ));
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a create whose config fails says so and keeps every capability.
pub fn changes_nothing_when_a_create_fails<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, failing, _, _, _) = value;
    assert!(matches!(
        table.create(name(SECOND), failing()),
        Err(CreateError::Failed(_))
    ));
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a table made anew lends nothing.
pub fn gets_nothing_where_nothing_was_created<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let table = value.0();
    assert_eq!(lookup(&table, &value, FIRST), None);
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&table, &value, THIRD), None);
}

/// Checks that a dropped identifier lends nothing, keeps the rest, and takes a new capability.
pub fn gets_nothing_after_a_drop<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, config, _, _, _, _) = value;
    table.drop(&name(FIRST));
    assert_eq!(lookup(&table, &value, FIRST), None);
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
    assert!(matches!(table.create(name(FIRST), config(SECOND)), Ok(())));
    mark(&table, &value, FIRST, SECOND);
    assert_eq!(lookup(&table, &value, FIRST), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a dropped identifier stays empty when another identifier takes a new capability.
pub fn forgets_a_dropped_identifier<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    table.drop(&name(FIRST));
    create(&mut table, &value, SECOND);
    assert_eq!(lookup(&table, &value, FIRST), None);
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that identifiers hold their own capabilities, and that dropping one leaves the others.
pub fn keeps_identifiers_apart<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    create(&mut table, &value, SECOND);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
    table.drop(&name(FIRST));
    assert_eq!(lookup(&table, &value, FIRST), None);
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that dropping an identifier that holds nothing keeps every capability.
pub fn drops_nothing_where_nothing_is_held<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    table.drop(&name(SECOND));
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate within the table puts the same capability under the new identifier and keeps the rest.
pub fn duplicates_within_the_table<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    assert!(matches!(
        table.duplicate_within(&name(FIRST), name(SECOND)),
        Ok(())
    ));
    assert_eq!(lookup(&table, &value, SECOND), Some((FIRST, FIRST)));
    mark(&table, &value, SECOND, SECOND);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate within the table of an identifier holding nothing is refused, even onto a taken identifier.
pub fn refuses_to_duplicate_nothing_within_the_table<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    assert!(matches!(
        table.duplicate_within(&name(SECOND), name(FIRST)),
        Err(DuplicateError::NothingHeld)
    ));
    assert!(matches!(
        table.duplicate_within(&name(SECOND), name(SECOND)),
        Err(DuplicateError::NothingHeld)
    ));
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate within the table onto a taken identifier, the held one itself too, is refused and keeps all.
pub fn refuses_to_duplicate_within_onto_a_taken_identifier<
    T,
    Identifier,
    Capability,
    Config,
    Error,
>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    create(&mut table, &value, SECOND);
    assert!(matches!(
        table.duplicate_within(&name(FIRST), name(SECOND)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert!(matches!(
        table.duplicate_within(&name(FIRST), name(FIRST)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate within the table of a held capability onto a free identifier succeeds or changes nothing.
///
/// These are the only two outcomes the clauses leave it.
pub fn changes_nothing_when_a_duplicate_within_fails<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (_, name, _, _, _, _, _) = value;
    let expected = match table.duplicate_within(&name(FIRST), name(SECOND)) {
        Ok(()) => Some((FIRST, FIRST)),
        Err(DuplicateError::Failed(_)) => None,
        Err(DuplicateError::NothingHeld | DuplicateError::IdentifierTaken) => {
            panic!("a duplicate of a held capability onto a free identifier was refused")
        }
    };
    assert_eq!(lookup(&table, &value, SECOND), expected);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate from another table puts the other's same capability under the new identifier, and keeps all.
///
/// This table holds a capability of its own under the identifier duplicated, which must not be the one copied.
pub fn duplicates_from_another_table<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let other = filled(&value);
    let (make, name, config, _, _, _, _) = value;
    let mut table = make();
    assert!(matches!(table.create(name(FIRST), config(SECOND)), Ok(())));
    mark(&table, &value, FIRST, SECOND);
    create(&mut table, &value, THIRD);
    assert!(matches!(
        table.duplicate_from(&other, &name(FIRST), name(SECOND)),
        Ok(())
    ));
    assert_eq!(lookup(&table, &value, SECOND), Some((FIRST, FIRST)));
    mark(&table, &value, SECOND, THIRD);
    assert_eq!(lookup(&other, &value, FIRST), Some((FIRST, THIRD)));
    assert_eq!(lookup(&table, &value, FIRST), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
    assert_eq!(lookup(&other, &value, SECOND), None);
    assert_eq!(lookup(&other, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate from another table holding nothing under the identifier is refused and changes nothing.
///
/// This table holds the identifier itself, which must not count.
pub fn refuses_to_duplicate_nothing_from_another_table<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let mut other = value.0();
    create(&mut other, &value, SECOND);
    let (_, name, _, _, _, _, _) = value;
    assert!(matches!(
        table.duplicate_from(&other, &name(FIRST), name(SECOND)),
        Err(DuplicateError::NothingHeld)
    ));
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
    assert_eq!(lookup(&other, &value, SECOND), Some((SECOND, SECOND)));
}

/// Checks that a duplicate from another table onto an identifier this table holds is refused and keeps all.
///
/// Only this table's identifiers count as taken: the other's do not.
pub fn refuses_to_duplicate_from_onto_a_taken_identifier<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut other = filled(&value);
    create(&mut other, &value, SECOND);
    let (make, name, config, _, _, _, _) = value;
    let mut table = make();
    assert!(matches!(table.create(name(FIRST), config(SECOND)), Ok(())));
    mark(&table, &value, FIRST, SECOND);
    assert!(matches!(
        table.duplicate_from(&other, &name(SECOND), name(FIRST)),
        Err(DuplicateError::IdentifierTaken)
    ));
    assert_eq!(lookup(&table, &value, FIRST), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, SECOND), None);
    let mut empty = make();
    assert!(matches!(
        empty.duplicate_from(&other, &name(FIRST), name(SECOND)),
        Ok(())
    ));
    assert_eq!(lookup(&empty, &value, SECOND), Some((FIRST, FIRST)));
    assert_eq!(lookup(&other, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate from another table of a held capability onto a free identifier succeeds or changes nothing.
///
/// These are the only two outcomes the clauses leave it.
pub fn changes_nothing_when_a_duplicate_from_fails<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let other = filled(&value);
    let mut table = value.0();
    create(&mut table, &value, SECOND);
    create(&mut table, &value, THIRD);
    let (_, name, _, _, _, _, _) = value;
    let expected = match table.duplicate_from(&other, &name(FIRST), name(FIRST)) {
        Ok(()) => Some((FIRST, FIRST)),
        Err(DuplicateError::Failed(_)) => None,
        Err(DuplicateError::NothingHeld | DuplicateError::IdentifierTaken) => {
            panic!("a duplicate of a held capability onto a free identifier was refused")
        }
    };
    assert_eq!(lookup(&table, &value, FIRST), expected);
    assert_eq!(lookup(&table, &value, SECOND), Some((SECOND, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that a duplicate, within the table or from another, keeps the same capability when the original is dropped.
pub fn keeps_a_duplicate_after_the_original_is_dropped<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (make, name, _, _, _, _, _) = value;
    assert!(matches!(
        table.duplicate_within(&name(FIRST), name(SECOND)),
        Ok(())
    ));
    let mut copy = make();
    assert!(matches!(
        copy.duplicate_from(&table, &name(FIRST), name(FIRST)),
        Ok(())
    ));
    table.drop(&name(FIRST));
    assert_eq!(lookup(&table, &value, FIRST), None);
    mark(&copy, &value, FIRST, SECOND);
    assert_eq!(lookup(&table, &value, SECOND), Some((FIRST, SECOND)));
    assert_eq!(lookup(&copy, &value, FIRST), Some((FIRST, SECOND)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Checks that the original keeps its capability when a duplicate of it, within the table or from another, is dropped.
pub fn keeps_the_original_after_a_duplicate_is_dropped<T, Identifier, Capability, Config, Error>(
    value: Value<T, Identifier, Capability, Config>,
) where
    T: CapabilityTable<Identifier, Capability, Config, Error>,
    Config: CapabilityConfig<Capability>,
{
    let mut table = filled(&value);
    let (make, name, _, _, _, _, _) = value;
    assert!(matches!(
        table.duplicate_within(&name(FIRST), name(SECOND)),
        Ok(())
    ));
    let mut copy = make();
    assert!(matches!(
        copy.duplicate_from(&table, &name(FIRST), name(FIRST)),
        Ok(())
    ));
    table.drop(&name(SECOND));
    copy.drop(&name(FIRST));
    assert_eq!(lookup(&table, &value, SECOND), None);
    assert_eq!(lookup(&copy, &value, FIRST), None);
    assert_eq!(lookup(&table, &value, FIRST), Some((FIRST, FIRST)));
    assert_eq!(lookup(&table, &value, THIRD), Some((THIRD, THIRD)));
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
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
        fn forgets_a_dropped_identifier() {
            $crate::forgets_a_dropped_identifier($value);
        }
        #[test]
        fn keeps_identifiers_apart() {
            $crate::keeps_identifiers_apart($value);
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
    };
}
