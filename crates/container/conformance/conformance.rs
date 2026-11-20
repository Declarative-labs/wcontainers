//! The tests every implementation of wcontainers-container runs, written against its traits alone.

use wcontainers_container::Container;

/// TODO: replace this skeleton with tests that call every method of every trait. Each suite function
/// takes one value of the implementation, by value, and returns nothing.
pub fn has_a_name<T: Container>(subject: T) {
    assert!(!subject.name().is_empty());
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for
/// every test. List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn has_a_name() {
            $crate::has_a_name($value);
        }
    };
}
