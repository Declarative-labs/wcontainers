#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

use alloc::vec::Vec;
use core::future::poll_fn;
use core::pin::pin;

use futures_core::Stream;

pub trait WebAssemblyEngine {
    /// A compiled [module](https://webassembly.github.io/spec/core/syntax/modules.html#modules).
    type Module: Send + Sync;
    /// A [module instance](https://webassembly.github.io/spec/core/exec/runtime.html#module-instances).
    type Instance;
    /// An [external address](https://webassembly.github.io/spec/core/exec/runtime.html#external-addresses).
    type ExternalAddress: Clone;
    type Error;

    /// Compiles a module from its [binary format](https://webassembly.github.io/spec/core/binary/modules.html#binary-module).
    fn compile(&self, bytes: &[u8]) -> Result<Self::Module, Self::Error>;

    /// Compiles a module [while its bytes arrive](https://webassembly.github.io/spec/web-api/#dom-webassembly-compilestreaming).
    async fn compile_streaming<StreamError>(
        &self,
        chunks: impl Stream<Item = Result<impl AsRef<[u8]>, StreamError>>,
    ) -> Result<Self::Module, CompileStreamingError<StreamError, Self::Error>> {
        let mut chunks = pin!(chunks);
        let mut bytes = Vec::new();
        while let Some(chunk) = poll_fn(|context| chunks.as_mut().poll_next(context)).await {
            bytes.extend_from_slice(chunk.map_err(CompileStreamingError::Stream)?.as_ref());
        }
        self.compile(&bytes).map_err(CompileStreamingError::Compile)
    }

    /// Runs the [instantiation](https://webassembly.github.io/spec/core/exec/modules.html#instantiation) of the module.
    fn instantiate(
        &mut self,
        module: &Self::Module,
        imports: &[Import<'_, Self::ExternalAddress>],
    ) -> Result<Self::Instance, Self::Error>;

    /// Returns the address of the instance's [export](https://webassembly.github.io/spec/core/syntax/modules.html#exports) with the given name.
    fn find_export(
        &mut self,
        instance: &Self::Instance,
        name: &str,
    ) -> Option<Self::ExternalAddress>;
}

/// A WASM [import](https://webassembly.github.io/spec/core/syntax/modules.html#imports).
pub struct Import<'name, ExternalAddress> {
    pub module_name: &'name str,
    pub item_name: &'name str,
    pub address: ExternalAddress,
}

/// An engine that binds functions of one [function type](https://webassembly.github.io/spec/core/syntax/types.html#syntax-functype) to a Rust signature.
pub trait FunctionTypeBinder<Parameters, Results>: WebAssemblyEngine {
    type Function: BoundFunction<Self, Parameters, Results>;

    /// Returns the function at the address after checking that it has the function type.
    fn bind_function(
        &mut self,
        address: Self::ExternalAddress,
    ) -> Result<Self::Function, BindFunctionError>;
}

pub trait BoundFunction<Engine: WebAssemblyEngine + ?Sized, Parameters, Results> {
    /// Runs the [invocation](https://webassembly.github.io/spec/core/exec/modules.html#invocation) of the function.
    fn invoke(&self, engine: &mut Engine, parameters: Parameters) -> Result<Results, Engine::Error>;
}

pub enum BindFunctionError {
    NotFunction,
    OtherFunctionType,
}

pub enum CompileStreamingError<StreamError, CompileError> {
    Stream(StreamError),
    Compile(CompileError),
}
