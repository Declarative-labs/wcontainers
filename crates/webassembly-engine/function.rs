use thiserror::Error;

use crate::WebAssemblyEngine;

/// An engine that binds functions of one [function type](https://webassembly.github.io/spec/core/syntax/types.html#syntax-functype) to a Rust signature.
pub trait FunctionTypeBinder<Parameters, Results>: WebAssemblyEngine {
    type Function: BoundFunction<Self, Parameters, Results>;

    /// Returns the function at the address after checking that it has the function type.
    fn bind_engine_function(
        &mut self,
        address: Self::FunctionAddress,
    ) -> Result<Self::Function, BindFunctionError>;

    /// Returns the address of a new [host function](https://webassembly.github.io/spec/core/exec/runtime.html#syntax-hostfunc) that runs the given function.
    fn bind_host_function(
        &mut self,
        function: impl Fn(&mut Self::HostContext, Parameters) -> Results + Send + Sync + 'static,
    ) -> Self::FunctionAddress;
}

pub trait BoundFunction<Engine: WebAssemblyEngine, Parameters, Results> {
    /// Runs the [invocation](https://webassembly.github.io/spec/core/exec/modules.html#invocation) of the function.
    fn invoke(
        &self,
        engine: &mut Engine,
        parameters: Parameters,
    ) -> Result<Results, Engine::InvokeError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the function does not match the function type it is being bound to")]
pub struct BindFunctionError;
