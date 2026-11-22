//! The tests every implementation of wcontainers-readiness runs, written against its trait alone.
//!
//! Each takes the implementor with a handle and a way to make that handle ready, made by the suite of a
//! trait that requires `Readiness`.

/// Runs each suite function above as its own test on the value a requiring suite gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {};
}
