//! The tests every implementation of wcontainers-webassembly-engine runs, written against its traits alone.

use core::convert::Infallible;
use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};

use futures_core::Stream;
use wcontainers_webassembly_engine::{Import, WebAssemblyEngine};

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

/// The name the memory is exported and imported under.
const MEMORY_NAME: &str = "memory";

/// The module name the memory is imported under.
const IMPORT_MODULE_NAME: &str = "env";

/// A name nothing is exported under.
const UNEXPORTED_NAME: &str = "other";

/// Text, which lacks the [magic number](https://webassembly.github.io/spec/core/binary/modules.html#binary-magic) every module starts with.
const NOT_A_MODULE: &[u8] = b"not a module";

/// Checks that a module with nothing in it compiles and instantiates.
pub fn instantiates_an_empty_module<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject.compile(EMPTY).ok().expect("the module compiles");
    assert!(subject.instantiate(&module, &[]).is_ok());
}

/// Checks that one compiled module instantiates twice, each instance with its own exports.
pub fn instantiates_one_module_twice<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject.compile(EXPORTS_MEMORY).ok().expect("the module compiles");
    let first = subject.instantiate(&module, &[]).ok().expect("the module instantiates");
    let second = subject.instantiate(&module, &[]).ok().expect("the module instantiates again");
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
    let module = finish(subject.compile_streaming(chunks)).ok().expect("the module compiles");
    let instance = subject.instantiate(&module, &[]).ok().expect("the module instantiates");
    assert!(subject.find_export(&instance, MEMORY_NAME).is_some());
}

/// Checks that `compile` returns an error for bytes that lack the magic number.
pub fn refuses_bytes_that_are_not_a_module<T: WebAssemblyEngine>(subject: T) {
    assert!(subject.compile(NOT_A_MODULE).is_err());
}

/// Checks that an export is found by its name, and that no other name finds one.
pub fn finds_an_export_by_its_name<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject.compile(EXPORTS_MEMORY).ok().expect("the module compiles");
    let instance = subject.instantiate(&module, &[]).ok().expect("the module instantiates");
    assert!(subject.find_export(&instance, MEMORY_NAME).is_some());
    assert!(subject.find_export(&instance, UNEXPORTED_NAME).is_none());
}

/// Checks that one instance's export can be supplied for another module's import.
pub fn supplies_an_export_for_an_import<T: WebAssemblyEngine>(mut subject: T) {
    let exporting = subject.compile(EXPORTS_MEMORY).ok().expect("the module compiles");
    let exporter = subject.instantiate(&exporting, &[]).ok().expect("the module instantiates");
    let address = subject.find_export(&exporter, MEMORY_NAME).expect("the export");
    let importing = subject.compile(IMPORTS_MEMORY).ok().expect("the module compiles");
    let imports = [Import { module_name: IMPORT_MODULE_NAME, item_name: MEMORY_NAME, address }];
    assert!(subject.instantiate(&importing, &imports).is_ok());
}

/// Checks that a module does not instantiate when one of its imports is not supplied.
pub fn refuses_a_missing_import<T: WebAssemblyEngine>(mut subject: T) {
    let module = subject.compile(IMPORTS_MEMORY).ok().expect("the module compiles");
    assert!(subject.instantiate(&module, &[]).is_err());
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
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
    };
}
