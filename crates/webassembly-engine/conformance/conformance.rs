//! The tests every implementation of wcontainers-webassembly-engine runs, written against its traits alone.

use core::convert::Infallible;
use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};

use futures_core::Stream;
use wcontainers_webassembly_engine::function::{
    BindFunctionError, BoundFunction, FunctionTypeBinder,
};
use wcontainers_webassembly_engine::memory::{
    AddressType, GrowMemoryError, Limits, MemoryType, PAGE_SIZE, ReadMemoryError, Sharing,
    WriteMemoryError,
};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};

/// A [module](https://webassembly.github.io/spec/core/binary/modules.html#binary-module) with nothing in it.
const EMPTY: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

/// A module whose [memory section](https://webassembly.github.io/spec/core/binary/modules.html#memory-section) defines one memory of one page and whose [export section](https://webassembly.github.io/spec/core/binary/modules.html#export-section) exports it.
const EXPORTS_MEMORY: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x05, 0x03, 0x01, 0x00, 0x01, // memory section: one memory, at least one page
    0x07, 0x0a, 0x01, 0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00, // export section
];

/// A module whose [import section](https://webassembly.github.io/spec/core/binary/modules.html#import-section) imports a memory of at least one page.
const IMPORTS_MEMORY: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x02, 0x0f, 0x01, 0x03, b'e', b'n', b'v', 0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00,
    0x01, // import section
];

/// A module whose [code section](https://webassembly.github.io/spec/core/binary/modules.html#code-section) holds one function that takes nothing and returns `i32::MAX`, exported.
const EXPORTS_FUNCTION: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // type section: no parameters, one i32 result
    0x03, 0x02, 0x01, 0x00, // function section: one function of that type
    0x07, 0x0a, 0x01, 0x06, b'a', b'n', b's', b'w', b'e', b'r', 0x00, 0x00, // export section
    0x0a, 0x0a, 0x01, 0x08, 0x00, 0x41, 0xff, 0xff, 0xff, 0xff, 0x07, 0x0b, // code section
];

/// A module whose [import section](https://webassembly.github.io/spec/core/binary/modules.html#import-section) imports a function, and that exports a function returning what the imported one returns.
const CALLS_IMPORTED_FUNCTION: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // type section: no parameters, one i32 result
    0x02, 0x0c, 0x01, 0x03, b'e', b'n', b'v', 0x04, b'h', b'o', b'o', b'k', 0x00,
    0x00, // import section
    0x03, 0x02, 0x01, 0x00, // function section: one function of that type
    0x07, 0x08, 0x01, 0x04, b'c', b'a', b'l', b'l', 0x00, 0x01, // export section
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x10, 0x00, 0x0b, // code section: call the import
];

/// The name the host function is imported under.
const HOST_FUNCTION_NAME: &str = "hook";

/// The name the function that calls the import is exported under.
const CALLER_NAME: &str = "call";

/// The name the function is exported under.
const FUNCTION_NAME: &str = "answer";

/// The pages that the [limits](https://webassembly.github.io/spec/core/binary/types.html#limits) of the memory `IMPORTS_MEMORY` imports ask for at least.
const IMPORTED_PAGES: u64 = 1;

/// The name the memory is exported and imported under.
const MEMORY_NAME: &str = "memory";

/// The module name the memory is imported under.
const IMPORT_MODULE_NAME: &str = "env";

/// A name nothing is exported under.
const UNEXPORTED_NAME: &str = "other";

/// Text, which lacks the [magic number](https://webassembly.github.io/spec/core/binary/modules.html#binary-magic) every module starts with.
const NOT_A_MODULE: &[u8] = b"not a module";

/// Checks that `host_context` returns what `new` was given.
pub fn holds_the_host_context_it_was_given<T: WebAssemblyEngine<HostContext = i32>>(subject: T)
where
    T::Config: Default,
{
    drop(subject);
    let mut made = T::new(T::Config::default(), i32::MAX)
        .ok()
        .expect("the engine is made");
    assert_eq!(*made.host_context(), i32::MAX);
}

/// Checks that a module compiled by one engine instantiates in another made from the same config.
pub fn instantiates_a_module_another_engine_compiled<T: WebAssemblyEngine<HostContext = i32>>(
    subject: T,
) where
    T::Config: Default,
{
    drop(subject);
    let config = T::Config::default();
    let compiling = T::new(config.clone(), i32::default())
        .ok()
        .expect("the engine is made");
    let mut instantiating = T::new(config, i32::default())
        .ok()
        .expect("the engine is made");
    let module = compiling
        .compile(EXPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    assert!(instantiating.instantiate(&module, &[]).is_ok());
}

/// Checks that a module with nothing in it compiles and instantiates.
pub fn instantiates_an_empty_module<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject.compile(EMPTY).ok().expect("the module compiles");
    assert!(subject.instantiate(&module, &[]).is_ok());
}

/// Checks that one compiled module instantiates twice, each instance with its own exports.
pub fn instantiates_one_module_twice<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject
        .compile(EXPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    let first = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates");
    let second = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates again");
    assert!(subject.find_export(&first, MEMORY_NAME).is_some());
    assert!(subject.find_export(&second, MEMORY_NAME).is_some());
}

/// A stream that yields each of its chunks at once.
struct Chunks<'bytes>(core::slice::Chunks<'bytes, u8>);

impl<'bytes> Stream for Chunks<'bytes> {
    type Item = Result<&'bytes [u8], Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.0.next().map(Ok))
    }
}

/// Polls the future until it finishes.
fn finish<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

/// Checks that a module arriving in several chunks compiles to one that instantiates with its export.
pub fn compiles_a_module_arriving_in_chunks<T: WebAssemblyEngine>(mut subject: T) {
    let chunks = Chunks(EXPORTS_MEMORY.chunks(EMPTY.len()));
    let module = finish(subject.compile_streaming(chunks))
        .ok()
        .expect("the module compiles");
    let instance = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates");
    assert!(matches!(
        subject.find_export(&instance, MEMORY_NAME),
        Some(ExternalAddress::Memory(_))
    ));
}

/// Checks that `compile` returns an error for bytes that lack the magic number.
pub fn refuses_bytes_that_are_not_a_module<T: WebAssemblyEngine>(subject: T) {
    assert!(subject.compile(NOT_A_MODULE).is_err());
}

/// Checks that an export is found by its name, and that no other name finds one.
pub fn finds_an_export_by_its_name<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject
        .compile(EXPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    let instance = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates");
    assert!(subject.find_export(&instance, MEMORY_NAME).is_some());
    assert!(subject.find_export(&instance, UNEXPORTED_NAME).is_none());
}

/// Checks that one instance's export can be supplied for another module's import.
pub fn supplies_an_export_for_an_import<T: WebAssemblyEngine>(mut subject: T) {
    let exporting = subject
        .compile(EXPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    let exporter = subject
        .instantiate(&exporting, &[])
        .ok()
        .expect("the module instantiates");
    let address = subject
        .find_export(&exporter, MEMORY_NAME)
        .expect("the export");
    let importing = subject
        .compile(IMPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    let imports = [Import {
        module_name: IMPORT_MODULE_NAME,
        item_name: MEMORY_NAME,
        address,
    }];
    assert!(subject.instantiate(&importing, &imports).is_ok());
}

/// Checks that a module does not instantiate when one of its imports is not supplied.
pub fn refuses_a_missing_import<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject
        .compile(IMPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    assert!(subject.instantiate(&module, &[]).is_err());
}

/// Checks that an exported function, bound to its function type, returns its result when invoked.
pub fn invokes_a_bound_function<T: FunctionTypeBinder<(), i32>>(mut subject: T) {
    let module = subject
        .compile(EXPORTS_FUNCTION)
        .ok()
        .expect("the module compiles");
    let instance = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(address)) = subject.find_export(&instance, FUNCTION_NAME)
    else {
        panic!("the export is a function");
    };
    let function = subject
        .bind_engine_function(address)
        .ok()
        .expect("the function has the function type");
    assert_eq!(function.invoke(&mut subject, ()).ok(), Some(i32::MAX));
}

/// Checks that a function is not bound to a function type with other results.
pub fn refuses_to_bind_another_function_type<T: FunctionTypeBinder<(), i64>>(mut subject: T) {
    let module = subject
        .compile(EXPORTS_FUNCTION)
        .ok()
        .expect("the module compiles");
    let instance = subject
        .instantiate(&module, &[])
        .ok()
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(address)) = subject.find_export(&instance, FUNCTION_NAME)
    else {
        panic!("the export is a function");
    };
    assert!(matches!(
        subject.bind_engine_function(address),
        Err(BindFunctionError)
    ));
}

/// Checks that a host function is invoked with the host context and that its result reaches its caller.
pub fn invokes_a_host_function<T: FunctionTypeBinder<(), i32, HostContext = i32>>(mut subject: T) {
    let module = subject
        .compile(CALLS_IMPORTED_FUNCTION)
        .ok()
        .expect("the module compiles");
    let address = subject.bind_host_function(|context: &mut i32, ()| {
        *context = i32::MAX;
        i32::MIN
    });
    let imports = [Import {
        module_name: IMPORT_MODULE_NAME,
        item_name: HOST_FUNCTION_NAME,
        address: ExternalAddress::Function(address),
    }];
    let instance = subject
        .instantiate(&module, &imports)
        .ok()
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(address)) = subject.find_export(&instance, CALLER_NAME)
    else {
        panic!("the export is a function");
    };
    let function = subject
        .bind_engine_function(address)
        .ok()
        .expect("the function has the function type");
    assert_eq!(function.invoke(&mut subject, ()).ok(), Some(i32::MIN));
    assert_eq!(*subject.host_context(), i32::MAX);
}

/// Returns a memory type of exactly as many pages as `IMPORTS_MEMORY` asks for at least.
fn one_page(sharing: Sharing) -> MemoryType {
    MemoryType {
        address_type: AddressType::I32,
        limits: Limits {
            minimum: IMPORTED_PAGES,
            maximum: Some(IMPORTED_PAGES),
        },
        sharing,
    }
}

/// Checks that an allocated memory has the size of its type and that every byte of it is zero.
pub fn allocates_a_zeroed_memory<T: WebAssemblyEngine>(mut subject: T) {
    let memory = subject
        .allocate_memory(one_page(Sharing::Unshared))
        .ok()
        .expect("the memory");
    assert_eq!(subject.count_memory_pages(&memory), IMPORTED_PAGES);
    let mut read = [u8::MAX; NOT_A_MODULE.len()];
    assert_eq!(subject.read_memory(&memory, u64::MIN, &mut read), Ok(()));
    assert_eq!(read, [u8::MIN; NOT_A_MODULE.len()]);
}

/// Checks that bytes written into a memory are the bytes read from it.
pub fn reads_the_bytes_it_wrote<T: WebAssemblyEngine>(mut subject: T) {
    let memory = subject
        .allocate_memory(one_page(Sharing::Unshared))
        .ok()
        .expect("the memory");
    assert_eq!(
        subject.write_memory(&memory, u64::MIN, NOT_A_MODULE),
        Ok(())
    );
    let mut read = [u8::MIN; NOT_A_MODULE.len()];
    assert_eq!(subject.read_memory(&memory, u64::MIN, &mut read), Ok(()));
    assert_eq!(read, NOT_A_MODULE);
}

/// Checks that bytes that start at the end of a memory are neither read nor written.
pub fn refuses_bytes_outside_a_memory<T: WebAssemblyEngine>(mut subject: T) {
    let memory = subject
        .allocate_memory(one_page(Sharing::Unshared))
        .ok()
        .expect("the memory");
    let end = IMPORTED_PAGES * PAGE_SIZE;
    let mut read = [u8::MIN; NOT_A_MODULE.len()];
    assert_eq!(
        subject.read_memory(&memory, end, &mut read),
        Err(ReadMemoryError)
    );
    assert_eq!(
        subject.write_memory(&memory, end, NOT_A_MODULE),
        Err(WriteMemoryError)
    );
}

/// Checks that a memory grows up to its maximum and no further.
pub fn grows_a_memory_to_its_maximum<T: WebAssemblyEngine>(mut subject: T) {
    let limits = Limits {
        minimum: u64::MIN,
        maximum: Some(IMPORTED_PAGES),
    };
    let memory_type = MemoryType {
        limits,
        ..one_page(Sharing::Unshared)
    };
    let memory = subject
        .allocate_memory(memory_type)
        .ok()
        .expect("the memory");
    assert_eq!(subject.grow_memory(&memory, IMPORTED_PAGES), Ok(()));
    assert_eq!(subject.count_memory_pages(&memory), IMPORTED_PAGES);
    assert_eq!(
        subject.grow_memory(&memory, IMPORTED_PAGES),
        Err(GrowMemoryError)
    );
}

/// Checks that an allocated memory can be supplied for a module's import.
pub fn supplies_an_allocated_memory_for_an_import<T: WebAssemblyEngine>(mut subject: T) {
    let memory = subject
        .allocate_memory(one_page(Sharing::Unshared))
        .ok()
        .expect("the memory");
    let module = subject
        .compile(IMPORTS_MEMORY)
        .ok()
        .expect("the module compiles");
    let imports = [Import {
        module_name: IMPORT_MODULE_NAME,
        item_name: MEMORY_NAME,
        address: ExternalAddress::Memory(memory),
    }];
    assert!(subject.instantiate(&module, &imports).is_ok());
}

/// Checks that an unshared memory has no form another engine can add.
pub fn keeps_an_unshared_memory_to_its_engine<T: WebAssemblyEngine>(mut subject: T) {
    let memory = subject
        .allocate_memory(one_page(Sharing::Unshared))
        .ok()
        .expect("the memory");
    assert!(subject.to_shared_memory(&memory).is_none());
}

/// Checks that bytes one engine writes into a shared memory are read by another engine that added it.
pub fn shares_a_memory_with_another_engine<T: WebAssemblyEngine<HostContext = i32>>(mut subject: T)
where
    T::Config: Default,
{
    let memory = subject
        .allocate_memory(one_page(Sharing::Shared))
        .ok()
        .expect("the memory");
    let shared = subject
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let mut other: T = T::new(T::Config::default(), i32::default())
        .ok()
        .expect("the engine is made");
    let added = other.add_shared_memory(&shared);
    assert_eq!(
        subject.write_memory(&memory, u64::MIN, NOT_A_MODULE),
        Ok(())
    );
    let mut read = [u8::MIN; NOT_A_MODULE.len()];
    assert_eq!(other.read_memory(&added, u64::MIN, &mut read), Ok(()));
    assert_eq!(read, NOT_A_MODULE);
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn holds_the_host_context_it_was_given() {
            $crate::holds_the_host_context_it_was_given($value);
        }

        #[test]
        fn instantiates_a_module_another_engine_compiled() {
            $crate::instantiates_a_module_another_engine_compiled($value);
        }

        #[test]
        fn instantiates_an_empty_module() {
            $crate::instantiates_an_empty_module($value);
        }

        #[test]
        fn instantiates_one_module_twice() {
            $crate::instantiates_one_module_twice($value);
        }

        #[test]
        fn compiles_a_module_arriving_in_chunks() {
            $crate::compiles_a_module_arriving_in_chunks($value);
        }

        #[test]
        fn refuses_bytes_that_are_not_a_module() {
            $crate::refuses_bytes_that_are_not_a_module($value);
        }

        #[test]
        fn finds_an_export_by_its_name() {
            $crate::finds_an_export_by_its_name($value);
        }

        #[test]
        fn supplies_an_export_for_an_import() {
            $crate::supplies_an_export_for_an_import($value);
        }

        #[test]
        fn refuses_a_missing_import() {
            $crate::refuses_a_missing_import($value);
        }

        #[test]
        fn invokes_a_bound_function() {
            $crate::invokes_a_bound_function($value);
        }

        #[test]
        fn refuses_to_bind_another_function_type() {
            $crate::refuses_to_bind_another_function_type($value);
        }

        #[test]
        fn invokes_a_host_function() {
            $crate::invokes_a_host_function($value);
        }

        #[test]
        fn allocates_a_zeroed_memory() {
            $crate::allocates_a_zeroed_memory($value);
        }

        #[test]
        fn reads_the_bytes_it_wrote() {
            $crate::reads_the_bytes_it_wrote($value);
        }

        #[test]
        fn refuses_bytes_outside_a_memory() {
            $crate::refuses_bytes_outside_a_memory($value);
        }

        #[test]
        fn grows_a_memory_to_its_maximum() {
            $crate::grows_a_memory_to_its_maximum($value);
        }

        #[test]
        fn supplies_an_allocated_memory_for_an_import() {
            $crate::supplies_an_allocated_memory_for_an_import($value);
        }

        #[test]
        fn keeps_an_unshared_memory_to_its_engine() {
            $crate::keeps_an_unshared_memory_to_its_engine($value);
        }

        #[test]
        fn shares_a_memory_with_another_engine() {
            $crate::shares_a_memory_with_another_engine($value);
        }
    };
}
