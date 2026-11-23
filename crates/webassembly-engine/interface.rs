#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

use alloc::vec::Vec;
use core::future::poll_fn;
use core::pin::pin;

use futures_core::Stream;
use thiserror::Error;

pub trait WebAssemblyEngine: Sized {
    type Config: Clone;
    /// A compiled [module](https://webassembly.github.io/spec/core/syntax/modules.html#modules).
    type Module: Send + Sync;
    /// A [module instance](https://webassembly.github.io/spec/core/exec/runtime.html#module-instances).
    type Instance;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [function](https://webassembly.github.io/spec/core/exec/runtime.html#function-instances).
    type FunctionAddress: Clone;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [table](https://webassembly.github.io/spec/core/exec/runtime.html#table-instances).
    type TableAddress: Clone;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [memory](https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances).
    type MemoryAddress: Clone;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [global](https://webassembly.github.io/spec/core/exec/runtime.html#global-instances).
    type GlobalAddress: Clone;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [tag](https://webassembly.github.io/spec/core/exec/runtime.html#tag-instances).
    type TagAddress: Clone;
    /// A value held by the engine and lent to each [host function](https://webassembly.github.io/spec/core/exec/runtime.html#syntax-hostfunc) when it is invoked.
    type HostContext;
    type InvokeError;

    type NewError;
    fn new(config: Self::Config, host_context: Self::HostContext) -> Result<Self, Self::NewError>;

    type CompileError;
    /// Compiles a module from its [binary format](https://webassembly.github.io/spec/core/binary/modules.html#binary-module).
    fn compile(&self, bytes: &[u8]) -> Result<Self::Module, Self::CompileError>;

    /// Compiles a module [while its bytes arrive](https://webassembly.github.io/spec/web-api/#dom-webassembly-compilestreaming).
    async fn compile_streaming<StreamError>(
        &self,
        chunks: impl Stream<Item = Result<impl AsRef<[u8]>, StreamError>>,
    ) -> Result<Self::Module, CompileStreamingError<StreamError, Self::CompileError>> {
        let mut chunks = pin!(chunks);
        let mut bytes = Vec::new();
        while let Some(chunk) = poll_fn(|context| chunks.as_mut().poll_next(context)).await {
            bytes.extend_from_slice(chunk.map_err(CompileStreamingError::Stream)?.as_ref());
        }
        self.compile(&bytes).map_err(CompileStreamingError::Compile)
    }

    type InstantiateError;
    /// Runs the [instantiation](https://webassembly.github.io/spec/core/exec/modules.html#instantiation) of the module.
    fn instantiate(
        &mut self,
        module: &Self::Module,
        imports: &[Import<'_, Self>],
    ) -> Result<Self::Instance, Self::InstantiateError>;

    /// Returns the address of the instance's [export](https://webassembly.github.io/spec/core/syntax/modules.html#exports) with the given name.
    fn find_export(
        &mut self,
        instance: &Self::Instance,
        name: &str,
    ) -> Option<ExternalAddress<Self>>;

    fn host_context(&mut self) -> &mut Self::HostContext;
}

/// A WASM [import](https://webassembly.github.io/spec/core/syntax/modules.html#imports).
pub struct Import<'name, Engine: WebAssemblyEngine> {
    pub module_name: &'name str,
    pub item_name: &'name str,
    pub address: ExternalAddress<Engine>,
}

/// An [external address](https://webassembly.github.io/spec/core/exec/runtime.html#external-addresses).
pub enum ExternalAddress<Engine: WebAssemblyEngine> {
    Function(Engine::FunctionAddress),
    Table(Engine::TableAddress),
    Memory(Engine::MemoryAddress),
    Global(Engine::GlobalAddress),
    Tag(Engine::TagAddress),
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompileStreamingError<StreamError, CompileError> {
    #[error("the stream of the module's bytes failed: {0}")]
    Stream(StreamError),
    #[error("the module could not compile: {0}")]
    Compile(CompileError),
}
