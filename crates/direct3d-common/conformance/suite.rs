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
    clippy::format_in_format_args,
    clippy::to_string_in_format_args,
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

/// What each method both glues carry to the implementation answered, in the order called: the place and the answer's
/// every field (`Debug`), an object in it named only as one (its identity is no value); and the caller's bytes after a
/// method that writes them. The same drive made on an implementation directly and through both glues must give the same.
#[derive(Debug, Default)]
struct Answers(
    std::sync::Mutex<Vec<String>>,
    Option<Received>,
    std::sync::atomic::AtomicUsize,
);

/// The implementation's own log of the arguments it received: how many so far, and those after the first n.
type Received = (fn() -> usize, fn(usize) -> Vec<String>);

impl Answers {
    fn with_received(received: Received) -> Answers {
        Answers(
            std::sync::Mutex::default(),
            Some(received),
            std::sync::atomic::AtomicUsize::new(0),
        )
    }

    /// Marks where the implementation's log of arguments stands before a compared call.
    fn start(&self) {
        if let Some((logged, _)) = self.1 {
            self.2.store(logged(), std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn push(&self, line: String) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(line);
    }

    /// The caller's bytes after the method.
    fn bytes(&self, place: &str, bytes: &SuiteBytes) {
        let held = bytes
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        self.push(format!("{place} wrote {held:?}"));
    }

    fn len(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Notes a call that noted nothing since `noted` answers (skipped for want of an object, or panicked).
    fn not_called_since(&self, noted: usize, place: &str) {
        if self.len() == noted {
            self.push(format!("{place}: not called, or panicked"));
        }
        // What the implementation received during the call, every argument.
        if let Some((_, since)) = self.1 {
            // The same call asked again (a list's length, then the list) is one call received.
            let mut received = since(self.2.load(std::sync::atomic::Ordering::SeqCst));
            received.dedup();
            self.push(format!("{place}: the implementation received {received:?}"));
        }
    }

    fn lines(self) -> Vec<String> {
        self.0
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
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

/// Calls every method on every object held, and the exports, each answer checked, and notes in `answers` what each
/// method both glues carry to the implementation answered.
fn drive<A: direct3d_common::Direct3DCommon>(o: &Objects<A>, answers: &Answers) -> Driven {
    let mut driven = Driven::default();
    // ID3DBlob::GetBufferPointer on Blob
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered(direct3d_common::Blob::get_buffer_pointer(this))
    });
    // ID3DBlob::GetBufferSize on Blob
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k0.as_ref() else {
            return false;
        };
        answered({
            let answer = direct3d_common::Blob::get_buffer_size(this);
            answers.push(format!(
                "ID3DBlob::GetBufferSize -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(noted, "ID3DBlob::GetBufferSize on Blob");
    // ID3DDestructionNotifier::RegisterDestructionCallback on DestructionNotifier
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        let lying = bytes();
        failure_is_negative(
            direct3d_common::DestructionNotifier::register_destruction_callback(
                this, 97_usize, &lying,
            ),
        )
    });
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier
    let noted = answers.len();
    answers.start();
    driven.attempt(|| {
        let Some(this) = o.k1.as_ref() else {
            return false;
        };
        failure_is_negative({
            let answer =
                direct3d_common::DestructionNotifier::unregister_destruction_callback(this, 35_u32);
            answers.push(format!(
                "ID3DDestructionNotifier::UnregisterDestructionCallback -> {}",
                format!("{:?}", &answer)
            ));
            answer
        })
    });
    answers.not_called_since(
        noted,
        "ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier",
    );
    driven
}

/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.
const CALLS: usize = 4;

/// The number of answers the suite notes and compares: each answer of a method both glues carry to the
/// implementation, and the caller's bytes after each such method that writes them.
const COMPARED: usize = 2;

/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on
/// them: no method panics, a failure is negative, an object handed out equals its clone.
pub fn every_method_answers<A: direct3d_common::Direct3DCommon>(api: A) {
    let mut o = Objects::new(api);
    let kinds = make_all(&mut o);
    let driven = drive(&o, &Answers::default());
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

/// Makes an object of every kind and drives every method as `every_method_answers` does, and returns what each
/// method both glues carry to the implementation answered, in the order called.
pub fn answers_of<A: direct3d_common::Direct3DCommon>(api: A) -> Vec<String> {
    let mut o = Objects::new(api);
    make_all(&mut o);
    let answers = Answers::default();
    drive(&o, &answers);
    answers.lines()
}

/// Drives `api` and `reference` alike and compares every answer noted, field by field, in the order called: `api` is
/// `reference` served and called through both glues, so each answer must be what the implementation gave.
pub fn every_answer_equals_the_reference<
    A: direct3d_common::Direct3DCommon,
    R: direct3d_common::Direct3DCommon,
>(
    api: A,
    reference: R,
) {
    let got = answers_of(api);
    let wanted = answers_of(reference);
    println!(
        "ANSWERS direct3d-common: the reference answered {} of {COMPARED}, through both glues {}",
        wanted.len(),
        got.len()
    );
    let differing: Vec<String> = got
        .iter()
        .zip(&wanted)
        .enumerate()
        .filter(|(_, (got, wanted))| got != wanted)
        .map(|(at, (got, wanted))| {
            format!("answer {at}: through both glues {got}, the implementation {wanted}")
        })
        .collect();
    for line in &differing {
        println!("DIFFERS {line}");
    }
    assert_eq!(
        differing,
        Vec::<String>::new(),
        "answers differ through both glues"
    );
    assert_eq!(
        got.len(),
        wanted.len(),
        "through both glues, a different number of answers"
    );
    assert_eq!(
        wanted.len(),
        COMPARED,
        "the reference answered a different number of answers than the suite notes"
    );
}

/// As `answers_of`, with what the implementation received during each compared call after its answer (`logged` and
/// `since`: the implementation's own log of the arguments it received).
pub fn answers_received_of<A: direct3d_common::Direct3DCommon>(
    api: A,
    logged: fn() -> usize,
    since: fn(usize) -> Vec<String>,
) -> Vec<String> {
    let mut o = Objects::new(api);
    make_all(&mut o);
    let answers = Answers::with_received((logged, since));
    drive(&o, &answers);
    answers.lines()
}

/// As `every_answer_equals_the_reference`, and every argument the implementation received through both glues equals
/// what it received from the same call made directly: nothing the caller passed is dropped, zeroed or mixed up on the
/// way. A method whose call through both glues differs by design (a check of the glue's on an argument the suite
/// passes, an answer the glue gives from its own record, a bit a row clears) must be one of `refused` (src/api.rs
/// `SUITE_DIFFERENT`, each with the reason).
pub fn every_answer_and_argument_equals_the_reference<
    A: direct3d_common::Direct3DCommon,
    R: direct3d_common::Direct3DCommon,
>(
    api: A,
    reference: R,
    logged: fn() -> usize,
    since: fn(usize) -> Vec<String>,
    refused: &[&str],
) {
    let wanted = answers_received_of(reference, logged, since);
    let got = answers_received_of(api, logged, since);
    let mut differing = Vec::new();
    let mut accepted = Vec::new();
    for (at, (got_line, wanted_line)) in got.iter().zip(&wanted).enumerate() {
        if got_line == wanted_line {
            continue;
        }
        // The method's place: the text before " -> ", " wrote " or " on " (a line of a call not made, or of what the
        // implementation received, names the kind too).
        let place = got_line
            .split(" -> ")
            .next()
            .unwrap_or_default()
            .split(" wrote ")
            .next()
            .unwrap_or_default()
            .split(" on ")
            .next()
            .unwrap_or_default()
            .split(": ")
            .next()
            .unwrap_or_default()
            .to_string();
        if refused.contains(&place.as_str())
            || refused.iter().any(|entry| {
                got_line.starts_with(entry) && got_line[entry.len()..].starts_with([' ', ':'])
            })
        {
            accepted.push(place);
        } else {
            differing.push(format!(
                "{at}: through both glues {got_line}, directly {wanted_line}"
            ));
        }
    }
    accepted.dedup();
    println!(
        "ANSWERS AND ARGUMENTS direct3d-common: {} lines directly, {} through both glues; {} methods differing as src/api.rs SUITE_DIFFERENT states: {accepted:?}",
        wanted.len(),
        got.len(),
        accepted.len()
    );
    for line in &differing {
        println!("DIFFERS {line}");
    }
    assert_eq!(
        differing,
        Vec::<String>::new(),
        "answers or arguments differ through both glues"
    );
    assert_eq!(
        got.len(),
        wanted.len(),
        "through both glues, a different number of answers"
    );
}

/// Runs each suite function above as its own test on the value an implementation gives; given a reference, also
/// compares every answer of the value with the reference's.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn every_method_answers() {
            $crate::every_method_answers($value);
        }
    };
    ($value:expr, $reference:expr) => {
        $crate::tests!($value);

        #[test]
        fn every_answer_equals_the_reference() {
            $crate::every_answer_equals_the_reference($value, $reference);
        }
    };
    ($value:expr, $reference:expr, $logged:expr, $since:expr, $refused:expr) => {
        // One test, so that no other drive adds to the implementation's log of arguments while it is read.
        #[test]
        fn every_method_answers_and_every_answer_and_argument_equals_the_reference() {
            $crate::every_method_answers($value);
            $crate::every_answer_and_argument_equals_the_reference(
                $value, $reference, $logged, $since, $refused,
            );
        }
    };
}
