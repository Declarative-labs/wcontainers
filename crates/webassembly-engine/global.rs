use vstd::prelude::*;

use crate::function::FunctionStore;

verus! {

/// The part of an engine's [store](https://webassembly.github.io/spec/core/exec/runtime.html#store) that holds its [global instances](https://webassembly.github.io/spec/core/exec/runtime.html#global-instances).
pub trait GlobalStore: FunctionStore {
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [global instance](https://webassembly.github.io/spec/core/exec/runtime.html#global-instances).
    type GlobalAddress: Clone;

    type AllocateGlobalError;
    /// Allocates a [global instance](https://webassembly.github.io/spec/core/exec/runtime.html#global-instances) that holds the value, as [`global_alloc`](https://webassembly.github.io/spec/core/appendix/embedding.html#embed-global-alloc) does.
    fn allocate_global(
        &mut self,
        mutability: Mutability,
        value: Value<Self::FunctionAddress>,
    ) -> Result<Self::GlobalAddress, Self::AllocateGlobalError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    fn read_global(
        &self,
        global: &Self::GlobalAddress,
    ) -> Result<Value<Self::FunctionAddress>, ReadGlobalError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    fn write_global(
        &mut self,
        global: &Self::GlobalAddress,
        value: Value<Self::FunctionAddress>,
    ) -> Result<(), WriteGlobalError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// The [mutability](https://webassembly.github.io/spec/core/syntax/types.html#global-types) of a global.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability {
    Constant,
    Variable,
}

/// A [value](https://webassembly.github.io/spec/core/intro/overview.html#value) of a number type, of the vector type, or that refers to a function.
///
/// References to anything but a function are left out: our project has no use for them, and holding one means keeping the engine's garbage collector informed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<FunctionAddress> {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128(u128),
    /// A [reference](https://webassembly.github.io/spec/core/syntax/types.html#:~:text=Reference%20types%20classify%20values%20that%20are%20first%2Dclass%20references) to a function, or null when it holds nothing.
    FunctionReference(Option<FunctionAddress>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadGlobalError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteGlobalError {
    Constant,
    ValueTypeMismatch,
}

} // verus!

impl core::fmt::Display for ReadGlobalError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("the global holds a reference to something other than a function")
    }
}

impl core::error::Error for ReadGlobalError {}

impl core::fmt::Display for WriteGlobalError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Constant => formatter.write_str("the global is constant"),
            Self::ValueTypeMismatch => formatter.write_str("the value is not of the global's value type"),
        }
    }
}

impl core::error::Error for WriteGlobalError {}
