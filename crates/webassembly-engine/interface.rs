#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod function;
pub mod global;
pub mod memory;
pub mod table;

use alloc::vec::Vec;
use core::future::poll_fn;
use core::pin::pin;

use futures_core::Stream;
use thiserror::Error;

use crate::function::FunctionStore;
use crate::global::GlobalStore;
use crate::memory::MemoryStore;
use crate::table::TableStore;

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait WebAssemblyEngine:
    Sized + FunctionStore + MemoryStore + TableStore + GlobalStore
{
    type Config: Clone;
    /// A compiled [module](https://webassembly.github.io/spec/core/syntax/modules.html#modules).
    type Module: Send + Sync;
    /// A [module instance](https://webassembly.github.io/spec/core/exec/runtime.html#module-instances).
    type Instance;
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [tag instance](https://webassembly.github.io/spec/core/exec/runtime.html#tag-instances).
    type TagAddress: Clone;
    /// A value held by the engine and lent to each [host function](https://webassembly.github.io/spec/core/exec/runtime.html#:~:text=A%20host%20function%20is%20a%20function%20expressed%20outside%20WebAssembly) when it is invoked.
    type HostContext;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompileStreamingError<StreamError, CompileError> {
    #[error("the stream of the module's bytes failed: {0}")]
    Stream(StreamError),
    #[error("the module could not compile: {0}")]
    Compile(CompileError),
}

/// An [address type](https://webassembly.github.io/spec/core/syntax/types.html#address-types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressType {
    I32,
    I64,
}

/// The [limits](https://webassembly.github.io/spec/core/syntax/types.html#limits) of the size of a memory, in pages, or of a table, in elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub minimum: u64,
    pub maximum: Option<u64>,
}
