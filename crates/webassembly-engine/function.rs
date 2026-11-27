use thiserror::Error;
use vstd::prelude::*;

use crate::WebAssemblyEngine;

/// The part of an engine's [store](https://webassembly.github.io/spec/core/exec/runtime.html#store) that holds its [function instances](https://webassembly.github.io/spec/core/exec/runtime.html#function-instances).
verus! {

pub trait FunctionStore {
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [function instance](https://webassembly.github.io/spec/core/exec/runtime.html#function-instances).
    type FunctionAddress: Clone;
    /// The engine's description of a [trap](https://webassembly.github.io/spec/core/intro/overview.html#trap).
    type Trap;
}

/// An engine that binds functions of one [function type](https://webassembly.github.io/spec/core/syntax/types.html#:~:text=Function%20types%20classify%20the%20signature%20of%20functions) to a Rust signature.
pub trait FunctionTypeBinder<Parameters, Results>: WebAssemblyEngine {
    type Function: BoundFunction<Self, Parameters, Results>;

    /// Returns the function at the address after checking that it has the function type.
    fn bind_engine_function(
        &mut self,
        address: Self::FunctionAddress,
    ) -> Result<Self::Function, BindFunctionError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns the address of a new [host function](https://webassembly.github.io/spec/core/exec/runtime.html#:~:text=A%20host%20function%20is%20a%20function%20expressed%20outside%20WebAssembly) that runs the given function.
    fn bind_host_function(
        &mut self,
        function: impl Fn(&mut Self::HostContext, Parameters) -> Results + Send + Sync + 'static,
    ) -> Self::FunctionAddress
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

pub trait BoundFunction<Engine: WebAssemblyEngine, Parameters, Results> {
    /// Runs the [invocation](https://webassembly.github.io/spec/core/exec/modules.html#invocation) of the function.
    fn invoke(
        &self,
        engine: &mut Engine,
        parameters: Parameters,
    ) -> Result<Results, InvokeError<Engine::Trap>>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the function does not match the function type it is being bound to")]
pub struct BindFunctionError;

/// A [result](https://webassembly.github.io/spec/core/exec/runtime.html#results) of an invocation that holds no values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum InvokeError<Trap> {
    #[error("the function trapped: {0}")]
    Trap(Trap),
    #[error("an exception left the function uncaught")]
    Exception,
}

} // verus!
