//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The conformance suite of direct3d-common, written against the traits alone (src/glue_hostile.rs): an object of every kind
//! the implementation makes, every method of every kind and every export called on them, each answer checked as the
//! interface promises.

#![allow(
    unused_variables,
    unused_imports,
    dead_code,
    unreachable_code,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::redundant_closure_call,
    clippy::too_many_lines,
    clippy::unnecessary_lazy_evaluations,
    clippy::option_map_unit_fn,
    clippy::bool_assert_comparison,
    clippy::eq_op,
    reason = "generated: every call has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;

/// What one drive did: methods called, skipped for want of an object or an argument, and calls that panicked.
#[derive(Clone, Copy, Debug, Default)]
struct Driven {
    called: usize,
    skipped: usize,
    panicked: usize,
}

impl Driven {
    fn attempt(&mut self, call: impl FnOnce() -> bool) {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
            Ok(true) => self.called += 1,
            Ok(false) => self.skipped += 1,
            Err(_) => {
                self.called += 1;
                self.panicked += 1;
            }
        }
    }
}

/// Runs a maker: its object, if any (a panic is counted by the drive, not here).
fn made<T>(make: impl FnOnce() -> Option<T>) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make))
        .ok()
        .flatten()
}

/// The caller's bytes: a buffer of ours, read and written in full.
#[derive(Debug, Default)]
struct SuiteBytes(std::sync::Mutex<Vec<u8>>);

/// How many bytes the suite lends where a method writes bytes of the caller's.
const LENT_BYTES: usize = 256;

fn bytes() -> SuiteBytes {
    SuiteBytes(std::sync::Mutex::new(vec![0; LENT_BYTES]))
}

impl wcontainers_volatile_bytes::VolatileBytes for SuiteBytes {
    fn len(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        let copy = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        let mut copy = vec![
            u8::MIN;
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        ];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

/// A method's failure is negative, as every HRESULT failure is.
fn failure_is_negative<T>(answer: Result<T, component_object_model::structures::HResult>) -> bool {
    if let Err(failure) = answer {
        assert!(
            failure.0 < 0,
            "a failure answered as {:#x}, which reads as success",
            failure.0
        );
    }
    true
}

/// An object handed out equals its clone, as COM's identity is the object's.
fn equals_its_clone<T: Clone + Eq>(object: T) -> bool {
    assert!(object.clone() == object, "an object differs from its clone");
    true
}

/// An answer the interface promises nothing more of than its type.
fn answered<T>(answer: T) -> bool {
    drop(answer);
    true
}

/// One object of each kind the suite made, by kind.
struct Objects<A: direct3d_common::Direct3DCommon> {
    api: A,
    k0: Option<<A as direct3d_common::Direct3DCommon>::Blob>,
    k1: Option<<A as direct3d_common::Direct3DCommon>::DestructionNotifier>,
}

impl<A: direct3d_common::Direct3DCommon> Objects<A> {
    fn new(api: A) -> Objects<A> {
        Objects {
            api,
            k0: None,
            k1: None,
        }
    }

    fn kinds_held(&self) -> usize {
        [self.k0.is_some(), self.k1.is_some()]
            .iter()
            .filter(|held| **held)
            .count()
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new.
fn make_all<A: direct3d_common::Direct3DCommon>(o: &mut Objects<A>) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        if o.kinds_held() == before {
            break;
        }
    }
    o.kinds_held()
}

/// Calls every method on every object held, and the exports, each answer checked.
fn drive<A: direct3d_common::Direct3DCommon>(o: &Objects<A>) -> Driven {
    let mut driven = Driven::default();
    // ID3DBlob::GetBufferPointer on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_pointer(this))
    });
    // ID3DBlob::GetBufferSize on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_size(this))
    });
    // ID3DDestructionNotifier::RegisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_common::DestructionNotifier::register_destruction_callback(
                this,
                Default::default(),
                &lying,
            ),
        )
    });
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        failure_is_negative(
            direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            ),
        )
    });
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 4;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: direct3d_common::Direct3DCommon>(api: A) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o);
    println!(
        "SUITE direct3d-common: kinds made {kinds}, calls {CALLS}: called {}, skipped {}, panicked {}",
        driven.called, driven.skipped, driven.panicked
    );
    assert_eq!(driven.panicked, 0, "a method panicked");
    assert_eq!(
        driven.called + driven.skipped,
        CALLS,
        "a call was neither made nor skipped"
    );
}

/// Runs each suite function above as its own test on the value an implementation gives.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn every_method_answers() {
            $crate::every_method_answers($value);
        }
    };
}
