//! The tests every implementation of wcontainers-test-window runs, written against its traits alone.

use wcontainers_test_window::TestWindow;

/// Checks that the value has a name.
///
/// TODO: replace this skeleton with tests that call every method of every trait. Each suite function
/// takes one value of the implementation, by value, and returns nothing.
pub fn has_a_name<T: TestWindow>(subject: T) {
    assert!(!subject.name().is_empty());
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn has_a_name() {
            $crate::has_a_name($value);
        }
    };
}
