//! The tests every implementation of wcontainers-test-window runs, written against its traits alone.
//!
//! Nothing presents into the window here, so each test reads a window no GPU API drew into.

use wcontainers_test_window::{TEXEL_BYTES, TestWindow, WINDOW_SIDE};

/// The alpha of an opaque texel.
const OPAQUE: u8 = u8::MAX;

/// The place of alpha in a texel.
const ALPHA: usize = TEXEL_BYTES - 1;

/// Checks that the window gives a handle of its own and one of its display.
pub fn gives_its_handles<T: TestWindow>(subject: T) {
    assert!(subject.window_handle().is_ok());
    assert!(subject.display_handle().is_ok());
}

/// Checks that the window shows `WINDOW_SIDE` by `WINDOW_SIDE` opaque texels.
pub fn shows_its_side_of_opaque_texels<T: TestWindow>(subject: T) {
    let texels = subject.shown_texels();
    assert_eq!(texels.len(), WINDOW_SIDE * WINDOW_SIDE * TEXEL_BYTES);
    assert!(
        texels
            .as_chunks::<TEXEL_BYTES>()
            .0
            .iter()
            .all(|texel| texel[ALPHA] == OPAQUE)
    );
}

/// Runs each suite function above as its own test on a value made anew for every test, and each group's where the value
/// has it.
#[macro_export]
macro_rules! tests {
    (@StandaloneTestWindow yes ($value:expr)) => {
        $crate::tests!(@StandaloneTestWindow yes ($value) ($value));
    };
    (@StandaloneTestWindow yes ($base:expr) ($value:expr)) => {
        mod standalone_test_window {}
    };
    (@StandaloneTestWindow no ($value:expr)) => {
        mod standalone_test_window {
            #[test]
            fn lacks_standalone_test_window() {
                struct Probe<T>(::core::marker::PhantomData<T>);
                impl<T> ::core::clone::Clone for Probe<T> {
                    fn clone(&self) -> Self {
                        Probe(::core::marker::PhantomData)
                    }
                }
                struct Implemented;
                #[allow(dead_code)]
                impl<T: ::wcontainers_test_window::StandaloneTestWindow> Probe<T> {
                    fn clone(&self) -> Implemented {
                        Implemented
                    }
                }
                fn probe_of<T>(_value: &T) -> Probe<T> {
                    Probe(::core::marker::PhantomData)
                }
                let _: Probe<_> = probe_of(&$value).clone();
            }
        }
    };
    ($value:expr; StandaloneTestWindow: $standalone_test_window:ident $(($standalone_test_window_value:expr))?) => {
        #[test]
        fn gives_its_handles() {
            $crate::gives_its_handles($value);
        }

        #[test]
        fn shows_its_side_of_opaque_texels() {
            $crate::shows_its_side_of_opaque_texels($value);
        }
        $crate::tests!(@StandaloneTestWindow $standalone_test_window ($value) $(($standalone_test_window_value))?);
    };
}
