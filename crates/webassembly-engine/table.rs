use thiserror::Error;

use crate::function::FunctionStore;
use crate::{AddressType, Limits};

/// The part of an engine's [store](https://webassembly.github.io/spec/core/exec/runtime.html#store) that holds its [table instances](https://webassembly.github.io/spec/core/exec/runtime.html#table-instances).
pub trait TableStore: FunctionStore {
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [table instance](https://webassembly.github.io/spec/core/exec/runtime.html#table-instances).
    type TableAddress: Clone;

    type AllocateTableError;
    /// Allocates a [table instance](https://webassembly.github.io/spec/core/exec/runtime.html#table-instances) of the given type with every element null, as [`table_alloc`](https://webassembly.github.io/spec/core/appendix/embedding.html#embed-table-alloc) does.
    fn allocate_table(
        &mut self,
        table_type: TableType,
    ) -> Result<Self::TableAddress, Self::AllocateTableError>;

    fn count_table_elements(&self, table: &Self::TableAddress) -> u64;

    /// Grows the table by the given number of null elements, as [`table_grow`](https://webassembly.github.io/spec/core/appendix/embedding.html#embed-table-grow) does.
    fn grow_table(
        &mut self,
        table: &Self::TableAddress,
        elements: u64,
    ) -> Result<(), GrowTableError>;

    /// Returns the function the element at the index refers to, or nothing when the element is null.
    fn read_table(
        &self,
        table: &Self::TableAddress,
        index: u64,
    ) -> Result<Option<Self::FunctionAddress>, ReadTableError>;

    /// Sets the element at the index to a reference to the function, or to null when given nothing.
    fn write_table(
        &mut self,
        table: &Self::TableAddress,
        index: u64,
        function: Option<&Self::FunctionAddress>,
    ) -> Result<(), WriteTableError>;
}

/// A [table type](https://webassembly.github.io/spec/core/syntax/types.html#table-types) whose [reference type](https://webassembly.github.io/spec/core/syntax/types.html#reference-types) is `funcref`.
///
/// Other reference types are left out, as they are from `Value`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableType {
    pub address_type: AddressType,
    pub limits: Limits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the table could not be grown by the given number of elements")]
pub struct GrowTableError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ReadTableError {
    #[error("the index lies outside the table")]
    IndexOutsideTable,
    #[error("the element does not refer to a function")]
    ElementNotFunction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum WriteTableError {
    #[error("the index lies outside the table")]
    IndexOutsideTable,
    #[error("the table's reference type does not take the function")]
    ReferenceTypeMismatch,
}
