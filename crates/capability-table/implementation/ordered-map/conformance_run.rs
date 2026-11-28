//! The conformance suite run on tables of pages, and on a set holding pages and slices of bytes in one type.

use core::cell::RefCell;

use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_capability_table_ordered_map::{OrderedMapCapabilityTable, Page, PageError, Shared};
use wcontainers_volatile_bytes::VolatileBytes;

// A table of pages, each made from its length.
type Pages = OrderedMapCapabilityTable<u64, u64, Page>;

// A slice of bytes the test lends a table for good, the volatile bytes of the interface crate's own implementation.
type Slice = RefCell<&'static mut [u8]>;

// A table of slices of bytes, each made from the slice.
type Slices = OrderedMapCapabilityTable<u64, &'static mut [u8], Slice>;

// A set of a task holding pages and slices, one table of the implementation for each.
#[derive(Default)]
struct PagesAndSlices {
    pages: Pages,
    slices: Slices,
}

impl CapabilityTable<u64, Shared<Page>> for PagesAndSlices {
    type Config = u64;
    type Error = PageError;

    fn create(&mut self, identifier: u64, config: u64) -> Result<(), CreateError<PageError>> {
        self.pages.create(identifier, config)
    }

    fn get(&self, identifier: &u64) -> Option<&Shared<Page>> {
        self.pages.get(identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        CapabilityTable::<u64, Shared<Page>>::drop(&mut self.pages, identifier);
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> Result<(), DuplicateError<PageError>> {
        self.pages.duplicate_within(identifier, new_identifier)
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<PageError>> {
        self.pages.duplicate_from(&other.pages, identifier, new_identifier)
    }
}

impl CapabilityTable<u64, Shared<Slice>> for PagesAndSlices {
    type Config = &'static mut [u8];
    type Error = core::convert::Infallible;

    fn create(&mut self, identifier: u64, config: &'static mut [u8]) -> Result<(), CreateError<Self::Error>> {
        self.slices.create(identifier, config)
    }

    fn get(&self, identifier: &u64) -> Option<&Shared<Slice>> {
        self.slices.get(identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        CapabilityTable::<u64, Shared<Slice>>::drop(&mut self.slices, identifier);
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> Result<(), DuplicateError<Self::Error>> {
        self.slices.duplicate_within(identifier, new_identifier)
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<Self::Error>> {
        self.slices.duplicate_from(&other.slices, identifier, new_identifier)
    }
}

// Returns a slice of as many bytes as the number, which the test never takes back.
fn slice(number: u8) -> &'static mut [u8] {
    vec![u8::MIN; usize::from(number)].leak()
}

// Returns the number a page or slice was made for, its length.
fn number<Bytes: VolatileBytes>(bytes: &Bytes) -> u8 {
    u8::try_from(bytes.len()).expect("every page and slice of the tests is as long as its number")
}

wcontainers_capability_table_conformance::tests!(
    (
        Pages::default as fn() -> Pages,
        u64::from as fn(u8) -> u64,
        u64::from as fn(u8) -> u64,
        (|| u64::MAX) as fn() -> u64,
        number::<Shared<Page>> as fn(&Shared<Page>) -> u8,
    ),
    (
        PagesAndSlices::default as fn() -> PagesAndSlices,
        u64::from as fn(u8) -> u64,
        u64::from as fn(u8) -> u64,
        number::<Shared<Page>> as fn(&Shared<Page>) -> u8,
        slice as fn(u8) -> &'static mut [u8],
        number::<Shared<Slice>> as fn(&Shared<Slice>) -> u8,
    )
);
