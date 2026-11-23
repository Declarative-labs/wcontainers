//! The tests every implementation of wcontainers-webassembly-engine runs, written against its traits alone.

use core::convert::Infallible;
use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};

use futures_core::Stream;
use wcontainers_webassembly_engine::{
    BindFunctionError, BoundFunction, ExternalAddress, FunctionTypeBinder, Import,
    WebAssemblyEngine,
};

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
    };
}
