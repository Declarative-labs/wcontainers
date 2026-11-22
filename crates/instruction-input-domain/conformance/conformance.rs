//! The tests every implementation of wcontainers-instruction-input-domain runs, written against its traits alone.

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {};
}
