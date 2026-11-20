//! The tests every implementation of wcontainers-system runs, written against its traits alone.

/// Runs each suite function above as its own test on the value an implementation gives, made anew for
/// every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {};
}
