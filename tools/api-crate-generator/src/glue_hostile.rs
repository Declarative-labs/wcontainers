//! The hostile implementation set, for tests only: `implementation/hostile`. For every safe trait src/trust.rs lists, an
//! implementation that lies in each way: the API's traits (every method may panic, re-enter the glue or never return at
//! a chosen call), the objects' `Drop` (the same), their `Clone`, `PartialEq` and `Hash` (each panics: the glue never
//! asks them), the bytes Map lends (lengths that change on every call, other bytes on every read, more bytes than the
//! length, and the three ways), and the caller's bytes (`LyingBytes`, the same lies on the calling glue's side). A
//! generated driver calls every method the calling glue carries, of every kind of object it can make, through both glues
//! (`served`): the hostile tests run it under each lie.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use super::{Part, Writer, direct_alias, snake};
use crate::api::Exception;
use crate::emit::{alias, package};

/// The crate alias of an API's hostile implementation.
fn hostile_alias(folder: &str) -> String {
    format!("{}_hostile", folder.replace('-', "_"))
}

/// The lies, their counters and the gate a blocked call waits at, the same in every hostile crate.
pub(super) const HOSTILE_BASE: &str = r#"use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use wcontainers_component_object_model::constants::{E_NOINTERFACE, E_NOTIMPL};

/// Where a lie is told: a method of the API's traits, an object's `Drop`, or the bytes a Map lends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Method,
    Drop,
    Mapped,
}

/// How: a panic, a call back into the glue (the function `on_reentry` set), or a call that never returns until the
/// gate opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Panic,
    Reenter,
    Block,
}

/// When: at the nth call of the site since the plan (from 0), or at every nth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    At(u64),
    Every(u64),
}

/// One lie of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lie {
    pub site: Site,
    pub way: Way,
    pub when: When,
}

static PLAN: Mutex<Option<Lie>> = Mutex::new(None);
static COUNTS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
static TOLD: AtomicU64 = AtomicU64::new(0);
/// Whether a blocking lie blocks; the gate's epoch, which each release moves on.
static BLOCKING: AtomicBool = AtomicBool::new(false);
static EPOCH: Mutex<u64> = Mutex::new(0);
static RELEASED: Condvar = Condvar::new();
static BLOCKED: AtomicU64 = AtomicU64::new(0);
/// Blocks begun since the process started.
static BEGUN: AtomicU64 = AtomicU64::new(0);
/// Calls of a method the glue never asks.
static NEVER: AtomicU64 = AtomicU64::new(0);
type Reentry = Arc<dyn Fn() + Send + Sync>;
static REENTRY: Mutex<Option<Reentry>> = Mutex::new(None);
/// Objects alive now.
static LIVE: AtomicI64 = AtomicI64::new(0);
/// The next object's identity.
static NEXT: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// Whether this thread is inside a re-entry already: a re-entry does not re-enter again.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

fn site_index(site: Site) -> usize {
    match site {
        Site::Method => 0,
        Site::Drop => 1,
        Site::Mapped => 2,
    }
}

/// Sets the plan (none: honest) and counts every site from 0 again; a blocking plan makes blocking lies block.
pub fn plan(lie: Option<Lie>) {
    COUNTS.iter().for_each(|count| count.store(0, Ordering::SeqCst));
    TOLD.store(0, Ordering::SeqCst);
    set_blocking(lie.is_some_and(|lie| lie.way == Way::Block));
    *PLAN.lock().unwrap_or_else(PoisonError::into_inner) = lie;
}

/// Makes blocking lies (of the plan or of `LyingBytes`) block, or stops them, releasing every blocked call.
pub fn set_blocking(blocking: bool) {
    BLOCKING.store(blocking, Ordering::SeqCst);
    if !blocking {
        release();
    }
}

/// Releases every call blocked now; a later blocking lie blocks again.
pub fn release() {
    *EPOCH.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    RELEASED.notify_all();
}

/// Returns how many calls of a method the glue never asks (`Clone`, `PartialEq`, `Hash`) were made.
pub fn never_asked_calls() -> u64 {
    NEVER.load(Ordering::SeqCst)
}

/// Returns how many blocks began since the process started.
pub fn blocks_begun() -> u64 {
    BEGUN.load(Ordering::SeqCst)
}

/// Returns how many calls wait at the gate now.
pub fn blocked() -> u64 {
    BLOCKED.load(Ordering::SeqCst)
}

/// Returns how many lies were told since the plan.
pub fn lies_told() -> u64 {
    TOLD.load(Ordering::SeqCst)
}

/// Returns how many calls a site had since the plan.
pub fn calls_at(site: Site) -> u64 {
    COUNTS[site_index(site)].load(Ordering::SeqCst)
}

/// Sets what a re-entering lie runs (none: nothing).
pub fn on_reentry(reentry: Option<Reentry>) {
    *REENTRY.lock().unwrap_or_else(PoisonError::into_inner) = reentry;
}

/// Returns how many objects of this crate are alive.
pub fn live() -> i64 {
    LIVE.load(Ordering::SeqCst)
}

/// Waits until the next release, when blocking lies block.
fn wait_at_gate() {
    if !BLOCKING.load(Ordering::SeqCst) {
        return;
    }
    let mut epoch = EPOCH.lock().unwrap_or_else(PoisonError::into_inner);
    let started = *epoch;
    BLOCKED.fetch_add(1, Ordering::SeqCst);
    BEGUN.fetch_add(1, Ordering::SeqCst);
    while *epoch == started && BLOCKING.load(Ordering::SeqCst) {
        epoch = RELEASED.wait(epoch).unwrap_or_else(PoisonError::into_inner);
    }
    drop(epoch);
    BLOCKED.fetch_sub(1, Ordering::SeqCst);
}

/// Runs the re-entry once on this thread, never inside another.
fn reenter() {
    if INSIDE.with(Cell::get) {
        return;
    }
    let reentry = REENTRY.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let Some(reentry) = reentry else { return };
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            INSIDE.with(|inside| inside.set(false));
        }
    }
    INSIDE.with(|inside| inside.set(true));
    let _leave = Leave;
    reentry();
}

/// Tells the plan's lie when this call of `site` is the one it names.
fn lie(site: Site, what: &str) {
    let n = COUNTS[site_index(site)].fetch_add(1, Ordering::SeqCst);
    let Some(lie) = *PLAN.lock().unwrap_or_else(PoisonError::into_inner) else { return };
    let hit = lie.site == site
        && match lie.when {
            When::At(at) => n == at,
            When::Every(every) => every > 0 && n.is_multiple_of(every),
        };
    if !hit {
        return;
    }
    TOLD.fetch_add(1, Ordering::SeqCst);
    match lie.way {
        Way::Panic => panic!("the hostile implementation panics in {what} (call {n} of {site:?})"),
        Way::Reenter => reenter(),
        Way::Block => wait_at_gate(),
    }
}

/// The standard traits of an implementation's objects the glue never calls (src/trust.rs), by name.
const CLONE_NAME: &str = "Clone";
const PARTIAL_EQ_NAME: &str = "PartialEq";
const HASH_NAME: &str = "Hash";

/// A method the glue never calls: a call is a defect the tests see.
fn never_asked(what: &str) -> ! {
    NEVER.fetch_add(1, Ordering::SeqCst);
    panic!("the glue asked an implementation's {what}, which it never does (src/trust.rs)")
}

/// How many calls reached the implementation, by place (`Interface::Method`).
static REACHED: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// Returns how many calls of `place` reached the implementation.
pub fn reached(place: &str) -> u64 {
    REACHED.lock().unwrap_or_else(PoisonError::into_inner).get(place).copied().unwrap_or(0)
}

/// Calls that reached the implementation whose arguments were not those a row answers with, by place.
static REACHED_OTHERWISE: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// Returns how many calls of `place` reached the implementation with arguments other than `=value`.
pub fn reached_otherwise(place: &str) -> u64 {
    REACHED_OTHERWISE.lock().unwrap_or_else(PoisonError::into_inner).get(place).copied().unwrap_or(0)
}

/// A call of a method of the API's traits.
fn record(call: String) {
    let place = call.split('(').next().unwrap_or("").to_string();
    if let Some((_, value)) = ANSWERED.iter().find(|(answered, _)| *answered == place)
        && !call.contains(&format!("={value}"))
    {
        *REACHED_OTHERWISE.lock().unwrap_or_else(PoisonError::into_inner).entry(place.clone()).or_default() += 1;
    }
    *REACHED.lock().unwrap_or_else(PoisonError::into_inner).entry(place).or_default() += 1;
    lie(Site::Method, &call);
}

/// Uses every byte lent: writes as many as the length says, reads them back, and both copies through.
fn touch(bytes: &impl wcontainers_volatile_bytes::VolatileBytes) {
    let length = usize::try_from(bytes.len()).unwrap_or(usize::MAX).min(1 << 24);
    let pattern = vec![0xa5_u8; length];
    let _ = bytes.write(&pattern);
    let mut back = vec![u8::MIN; length];
    let _ = bytes.read(&mut back);
    let _ = bytes.read_to(&mut Swallow);
    let _ = bytes.write_from(&mut Pattern(0));
}

/// A sink that takes everything.
struct Swallow;

impl wcontainers_byte_sink::ByteSink for Swallow {
    type Error = core::convert::Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        Ok(bytes.len())
    }
}

/// A source that fills whatever it is given, with a pattern of its count.
struct Pattern(u8);

impl wcontainers_byte_source::ByteSource for Pattern {
    type Error = core::convert::Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Self::Error> {
        self.0 = self.0.wrapping_add(1);
        into.fill(self.0);
        Ok(into.len())
    }
}

/// One object alive, counted until its last clone drops; its drop lies as the plan says.
#[derive(Debug)]
struct Live(u64);

impl Live {
    fn new() -> Arc<Live> {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Arc::new(Live(NEXT.fetch_add(1, Ordering::SeqCst)))
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
        lie(Site::Drop, "Drop");
    }
}

/// The lengths a lying length answers in turn, the first being the honest one.
const LYING_LENGTHS: [u64; 8] = [u64::MAX, 0, 7, 1 << 40, 3, 1 << 20, 16, 4096];

/// Whose turn the next lying length is, over every value of the process.
static TURN: AtomicU64 = AtomicU64::new(0);

/// A length that is honest one time in nine, and else one of `LYING_LENGTHS`.
fn lying_length(honest: u64) -> u64 {
    let turn = TURN.fetch_add(1, Ordering::SeqCst);
    let lies = LYING_LENGTHS.len() as u64 + 1;
    match usize::try_from(turn % lies).ok().and_then(|at| at.checked_sub(1)).and_then(|at| LYING_LENGTHS.get(at)) {
        Some(lie) => *lie,
        None => honest,
    }
}

/// A pitch that is honest one time in four.
fn lying_pitch(honest: u32) -> u32 {
    match TURN.fetch_add(1, Ordering::SeqCst) % 4 {
        0 => honest,
        1 => 0,
        2 => u32::MAX,
        _ => 1,
    }
}

/// Gives a sink more bytes than any length said, in pieces of changing sizes and contents, ignoring what it took.
fn flood_sink<Sink: wcontainers_byte_sink::ByteSink>(sink: &mut Sink, most: usize) -> Result<usize, Sink::Error> {
    let mut given = 0;
    let mut piece = 1_usize;
    while given < most {
        let chunk = vec![u8::try_from(piece % 251).unwrap_or(0); piece.min(most - given)];
        let _ = sink.write(&chunk)?;
        given += chunk.len();
        piece = piece * 3 + 1;
    }
    Ok(given.saturating_mul(2))
}

/// Asks a source for bytes in pieces of changing sizes, more than any length said.
fn drain_source<Source: wcontainers_byte_source::ByteSource>(source: &mut Source, most: usize) -> Result<usize, Source::Error> {
    let mut taken = 0;
    let mut piece = 2_usize;
    while taken < most {
        let mut chunk = vec![u8::MIN; piece.min(most - taken)];
        let read = source.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        taken += chunk.len();
        piece = piece * 5 + 3;
    }
    Ok(taken.saturating_add(1))
}

/// Bytes the caller lends the calling glue, lying in every way a safe implementation can: a length that changes on
/// every call (the `lengths` in turn), more bytes than the length to a sink, other bytes on every read, counts that
/// are not what was copied, and at the nth call of any of its methods, a panic, a call back into the glue or a call
/// that never returns until the gate opens.
pub struct LyingBytes {
    lengths: Vec<u64>,
    way: Option<(Way, u64)>,
    asked: AtomicU64,
}

impl LyingBytes {
    /// Bytes whose length answers `lengths` in turn, honest otherwise.
    pub fn lengths(lengths: &[u64]) -> LyingBytes {
        LyingBytes { lengths: lengths.to_vec(), way: None, asked: AtomicU64::new(0) }
    }

    /// Bytes of a steady length that lie `way` at the call `at` of any of their methods.
    pub fn lying(way: Way, at: u64) -> LyingBytes {
        LyingBytes { lengths: vec![64], way: Some((way, at)), asked: AtomicU64::new(0) }
    }

    /// How many calls of their methods the glue made.
    pub fn asked(&self) -> u64 {
        self.asked.load(Ordering::SeqCst)
    }

    fn call(&self) -> u64 {
        let n = self.asked.fetch_add(1, Ordering::SeqCst);
        if let Some((way, at)) = self.way
            && at == n
        {
            TOLD.fetch_add(1, Ordering::SeqCst);
            match way {
                Way::Panic => panic!("the caller's lying bytes panic (call {n})"),
                Way::Reenter => reenter(),
                Way::Block => wait_at_gate(),
            }
        }
        n
    }

    fn length(&self, n: u64) -> u64 {
        match self.lengths.len() {
            0 => 0,
            count => self.lengths[usize::try_from(n).unwrap_or(0) % count],
        }
    }
}

impl wcontainers_volatile_bytes::VolatileBytes for LyingBytes {
    fn len(&self) -> u64 {
        let n = self.call();
        self.length(n)
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let n = self.call();
        into.fill(u8::try_from(n % 251).unwrap_or(0));
        into.len().saturating_add(5)
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let _ = self.call();
        bytes.len().saturating_mul(3).saturating_add(1)
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let n = self.call();
        let most = usize::try_from(self.length(n)).unwrap_or(usize::MAX).min(1 << 16).saturating_add(4096);
        flood_sink(sink, most)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let n = self.call();
        let most = usize::try_from(self.length(n)).unwrap_or(usize::MAX).min(1 << 16).saturating_add(4096);
        drain_source(source, most)
    }
}
"#;

/// The bytes the hostile implementation lends for a Map: a length that changes on every call, more bytes than it says,
/// other bytes on every read, and the plan's lies at the site `Mapped`.
pub(super) const HOSTILE_BYTES: &str = r#"
/// Bytes the hostile implementation lends for a Map.
#[derive(Debug)]
pub struct RecordedBytes(Arc<Mutex<Vec<u8>>>);

impl RecordedBytes {
    /// New bytes, of the length every Map lends, whose answers lie.
    pub fn new() -> RecordedBytes {
        RecordedBytes(Arc::new(Mutex::new(vec![0; MAPPED_LENGTH])))
    }
}

impl Default for RecordedBytes {
    fn default() -> RecordedBytes {
        RecordedBytes::new()
    }
}

impl wcontainers_volatile_bytes::VolatileBytes for RecordedBytes {
    fn len(&self) -> u64 {
        lie(Site::Mapped, "Mapped::len");
        lying_length(MAPPED_LENGTH as u64)
    }

    fn read(&self, into: &mut [u8]) -> usize {
        lie(Site::Mapped, "Mapped::read");
        into.fill(u8::try_from(TURN.fetch_add(1, Ordering::SeqCst) % 251).unwrap_or(0));
        into.len().saturating_mul(2)
    }

    fn write(&self, bytes: &[u8]) -> usize {
        lie(Site::Mapped, "Mapped::write");
        bytes.len().saturating_add(9)
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        lie(Site::Mapped, "Mapped::read_to");
        flood_sink(sink, MAPPED_LENGTH * 3)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        lie(Site::Mapped, "Mapped::write_from");
        drain_source(source, MAPPED_LENGTH * 3)
    }
}
"#;

/// The driver's bookkeeping, the same in every hostile crate that drives.
const DRIVER_BASE: &str = r#"
/// What one drive did: methods called (a panic counted too), skipped for want of an object, and calls that panicked
/// back into the driver (the caller's own bytes' panics, or a method of the calling glue that is not glued).
#[derive(Clone, Copy, Debug, Default)]
pub struct Driven {
    pub called: usize,
    pub skipped: usize,
    pub panicked: usize,
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

/// Runs a maker, which may panic (a lie): its object, if any.
fn made<T>(make: impl FnOnce() -> Option<T>) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make)).ok().flatten()
}

/// The fuzzer's numbers: splitmix64 of a seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn step(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number below `bound` (0 for 0).
    pub fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.step() % bound).unwrap_or(0)
    }

    pub fn coin(&mut self) -> bool {
        self.step() & 1 == 1
    }

    /// A value of a type, mostly small, sometimes an edge.
    pub fn value<T: FromRng>(&mut self) -> T {
        T::from_rng(self)
    }

    /// `count` random bytes.
    pub fn bytes(&mut self, count: usize) -> Vec<u8> {
        (0..count).map(|_| self.step().to_le_bytes()[0]).collect()
    }

    /// A number with an edge one time in four: 0, the most, or one past a small power of two.
    fn edged(&mut self) -> u64 {
        match self.below(8) {
            0 => 0,
            1 => u64::MAX,
            2 => 1 << self.below(40),
            3 => (1 << self.below(16)) + 1,
            _ => self.step() % 64,
        }
    }
}

/// A type the fuzzer can make a value of.
pub trait FromRng {
    fn from_rng(rng: &mut Rng) -> Self;
}

macro_rules! from_rng {
    ($($type:ty),*) => {$(
        impl FromRng for $type {
            fn from_rng(rng: &mut Rng) -> $type {
                let wide = rng.edged();
                <$type>::try_from(wide).unwrap_or_else(|_| <$type>::try_from(wide % 251).unwrap_or_default())
            }
        }
    )*};
}

from_rng!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl FromRng for bool {
    fn from_rng(rng: &mut Rng) -> bool {
        rng.coin()
    }
}

impl FromRng for f32 {
    fn from_rng(rng: &mut Rng) -> f32 {
        [0.0, 1.0, -1.0, f32::NAN, f32::INFINITY, 1.0e30][rng.below(6)]
    }
}

impl FromRng for f64 {
    fn from_rng(rng: &mut Rng) -> f64 {
        [0.0, 1.0, -1.0, f64::NAN, f64::INFINITY, 1.0e300][rng.below(6)]
    }
}

/// A fuzzed call's slices hold fewer elements than this, all of one call the same number.
const SLICE_LENGTHS: usize = 4;

/// The caller's bytes for a fuzzed call: a length that changes on every call, most often small.
pub fn random_bytes(rng: &mut Rng) -> LyingBytes {
    let lengths: Vec<u64> = (0..1 + rng.below(3)).map(|_| [0, 1, 4, 8, 16, 64, 256, 4096, 1 << 20, u64::MAX][rng.below(10)]).collect();
    LyingBytes::lengths(&lengths)
}

/// What a fuzzing run did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fuzzed {
    pub driven: Driven,
    pub remade: usize,
}
"#;

/// What a type of a trait signature is, for the driver.
enum Shape {
    /// An object of a kind, by its index; `by_ref` when the type is a reference.
    Object { kind: usize, by_ref: bool },
    /// A `<Interface>Ref` enum: its path and the kinds of its variants.
    Ref { path: String, kinds: Vec<usize> },
    /// An `UnknownObject` of an API: its path and its kinds.
    Unknown { path: String, kinds: Vec<usize> },
    /// An associated type of the API value that is no kind (a bound object): the driver makes none.
    Unmakeable,
    Other,
}

impl Writer<'_, '_> {
    /// The kind an `<A as path::Top>::Name` names, among `scope`.
    fn kind_named(&self, top: &str, name: &str, scope: &[usize]) -> Option<usize> {
        scope.iter().copied().find(|&k| {
            let kind = &self.g.kinds[k];
            kind.name == name && self.g.apis[kind.api].top_trait.is_some_and(|t| format!("{}::{t}", alias(self.g.apis[kind.api])) == top)
        })
    }

    /// What a type of a trait signature is.
    fn shape(&self, ty: &str, scope: &[usize]) -> Shape {
        let by_ref = ty.starts_with('&');
        let bare = ty.trim_start_matches('&').trim();
        if let Some(rest) = bare.strip_prefix("<Self::Api as ").or_else(|| bare.strip_prefix("<Self as "))
            && let Some((top, name)) = rest.split_once(">::")
        {
            return self.kind_named(top, name, scope).map_or(Shape::Unmakeable, |kind| Shape::Object { kind, by_ref });
        }
        let Some((path, generics)) = bare.split_once('<') else { return Shape::Other };
        if !(generics.starts_with("'_, Self") || generics.starts_with("Self")) {
            return Shape::Other;
        }
        let api_of = |path: &str| {
            let prefix = path.rsplit_once("::").map_or("", |(prefix, _)| prefix);
            self.g.apis.iter().position(|spec| alias(spec) == prefix)
        };
        if path.ends_with("::UnknownObject") || path.ends_with("::UnknownRef") {
            let Some(api) = api_of(path) else { return Shape::Other };
            let kinds: Vec<usize> = scope.iter().copied().filter(|&k| self.g.kinds[k].api == api).collect();
            return if path.ends_with("::UnknownObject") { Shape::Unknown { path: path.to_string(), kinds } } else { Shape::Ref { path: path.to_string(), kinds } };
        }
        if path.ends_with("Ref") {
            let kinds: Vec<usize> = self.g.kinds_deriving(&self.enum_interface(path)).into_iter().filter(|k| scope.contains(k)).collect();
            return Shape::Ref { path: path.to_string(), kinds };
        }
        Shape::Other
    }

    /// The expression of one argument, with the lines that bind what it needs (each of which leaves the call with
    /// `skip` when an object is missing); none when the driver cannot make it.
    #[allow(clippy::too_many_arguments, reason = "the argument's own parts")]
    fn argument(&self, place: &str, name: &str, ty: &str, scope: &[usize], codecs: &Codecs, prelude: &mut String, counter: &mut usize, skip: &str, riid: Option<&str>) -> Option<String> {
        // A value src/api.rs `SUITE_ARGUMENTS` gives for this argument of this method: valid by the method's documented
        // rules, so that the call reaches the implementation through both glues and is compared.
        if let Some((_, _, value)) = crate::api::SUITE_ARGUMENTS.iter().find(|(at, argument, _)| *at == place && *argument == name) {
            return Some((*value).to_string());
        }
        let t = ty.trim();
        if riid.is_some() && t.ends_with("Guid") {
            return riid.map(|iid| format!("&{iid}"));
        }
        if t.contains("VolatileBytes") {
            if !prelude.contains("let lying = bytes();") {
                prelude.push_str("        let lying = bytes();\n");
            }
            return Some(if t.starts_with("Option<") { "Some(&lying)".to_string() } else { "&lying".to_string() });
        }
        if let Some(inner) = t.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')) {
            // An object the suite holds, or a known value of plain data, so that dropping it on the way is seen.
            let inner = inner.trim();
            if let Shape::Object { kind, by_ref: true } = self.shape(inner, scope) {
                return Some(format!("o.k{kind}.as_ref()"));
            }
            if let Some(data) = inner.strip_prefix('&')
                && let Some(value) = codecs.shapes.known(data, super::record::salt_of(name), true).filter(|value| value != "None")
            {
                return Some(format!("Some(&{value})"));
            }
            // An array given or not: one element, as every array the suite passes.
            if (inner.starts_with("&[") || inner.starts_with("&'_ ["))
                && let Some(value) = self.argument(place, name, inner, scope, codecs, prelude, counter, skip, riid).filter(|value| value != "&[]")
            {
                return Some(format!("Some({value})"));
            }
            return Some("None".to_string());
        }
        // Plain data is a known value (src/glue_hostile.rs `Shapes`): never its type's default where it has another, and
        // of an enumeration a value the glue passes, so that a glue dropping or mixing up an argument is seen.
        let salt = super::record::salt_of(name);
        let bare_type = t.trim_start_matches('&').trim_start_matches("'_ ").trim();
        if (t.starts_with("&[") || t.starts_with('[')) && t.contains(';') {
            let reference = if t.starts_with('&') { "&" } else { "" };
            return Some(codecs.shapes.known(bare_type, salt, true).map_or(format!("{reference}Default::default()"), |value| format!("{reference}{value}")));
        }
        if let Some(element) = t.strip_prefix("&[").or_else(|| t.strip_prefix("&'_ [")).and_then(|rest| rest.strip_suffix(']')) {
            // One element, as every array the suite passes (arrays of one count must have one length): an object the
            // suite holds (or none), or a known value.
            if let Some(object) = element.trim().strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')) {
                if let Shape::Object { kind, by_ref: true } = self.shape(object.trim(), scope) {
                    return Some(format!("&[o.k{kind}.as_ref()]"));
                }
                return Some("&[None]".to_string());
            }
            return Some(codecs.shapes.known(element, salt, true).filter(|value| value != "None").map_or("&[]".to_string(), |value| format!("&[{value}]")));
        }
        if t == "&str" {
            return Some("\"known\"".to_string());
        }
        if t == "&CStr" || t.ends_with("::CStr") && t.starts_with('&') {
            return Some("c\"known\"".to_string());
        }
        match self.shape(t, scope) {
            Shape::Object { kind, by_ref } => {
                *counter += 1;
                let var = format!("v{counter}");
                if skip == "None" {
                    let _ = writeln!(prelude, "        let {var} = o.k{kind}.as_ref()?;");
                } else {
                    let _ = writeln!(prelude, "        let Some({var}) = o.k{kind}.as_ref() else {{ return {skip}; }};");
                }
                return Some(if by_ref { var } else { format!("{var}.clone()") });
            }
            Shape::Ref { path, kinds } => {
                if kinds.is_empty() {
                    return None;
                }
                *counter += 1;
                let var = format!("v{counter}");
                let arms: Vec<String> = kinds.iter().map(|&k| format!("if let Some(x) = o.k{k}.as_ref() {{ Some({path}::{}(x)) }}", self.g.kinds[k].name)).collect();
                if skip == "None" {
                    let _ = writeln!(prelude, "        let {var} = ({} else {{ None }})?;", arms.join(" else "));
                } else {
                    let _ = writeln!(prelude, "        let Some({var}) = ({} else {{ None }}) else {{ return {skip}; }};", arms.join(" else "));
                }
                return Some(var);
            }
            Shape::Unknown { .. } | Shape::Unmakeable => return None,
            Shape::Other => {}
        }
        let bare = t.trim_start_matches('&').trim();
        let reference = if t.starts_with('&') { "&" } else { "" };
        if let Some(value) = codecs.shapes.known(bare, salt, true).filter(|_| !bare.contains("Foreign") && !bare.contains('<')) {
            return Some(format!("{reference}{value}"));
        }
        if bare.contains("::structures::") {
            if bare.ends_with("::Guid") || codecs.defaults.contains(bare) {
                return Some(format!("{reference}Default::default()"));
            }
            let (module, ident, size) = codecs.decoders.get(bare)?;
            return Some(format!("{reference}{module}::decode_{ident}(&[u8::MIN; {size}])"));
        }
        if bare.contains("Foreign") || bare.contains("impl ") || bare.contains("dyn ") {
            return None;
        }
        let _ = name;
        Some(format!("{reference}Default::default()"))
    }

    /// The expression of one random argument, with the lines that bind what it needs (each leaving the call with `false`
    /// when an object is missing); none when the fuzzer cannot make it. Slices of one call share the length `count`.
    fn argument_random(&self, ty: &str, scope: &[usize], codecs: &Codecs, prelude: &mut String, counter: &mut usize) -> Option<String> {
        let t = ty.trim();
        if t.contains("VolatileBytes") {
            *counter += 1;
            let var = format!("b{counter}");
            let _ = writeln!(prelude, "        let {var} = random_bytes(r);");
            return Some(if t.starts_with("Option<") { format!("if r.coin() {{ Some(&{var}) }} else {{ None }}") } else { format!("&{var}") });
        }
        if let Some(inner) = t.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')) {
            let inner = inner.trim();
            if let Shape::Object { kind, by_ref: true } = self.shape(inner, scope) {
                return Some(format!("o.k{kind}.as_ref().filter(|_| r.coin())"));
            }
            let mut inner_prelude = String::new();
            let expr = self.argument_random(inner, scope, codecs, &mut inner_prelude, counter)?;
            if inner_prelude.contains("return false") {
                return Some("None".to_string());
            }
            prelude.push_str(&inner_prelude);
            *counter += 1;
            let var = format!("v{counter}");
            let _ = writeln!(prelude, "        let {var} = if r.coin() {{ Some({expr}) }} else {{ None }};");
            return Some(var);
        }
        if t.contains(';') && (t.starts_with("&[") || t.starts_with('[')) {
            return Some(if t.starts_with('&') { "&Default::default()" } else { "Default::default()" }.to_string());
        }
        if let Some(element) = t.strip_prefix("&[").or_else(|| t.strip_prefix("&'_ [")).and_then(|rest| rest.strip_suffix(']')) {
            let element = element.trim();
            let value = if let Some(object) = element.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')) {
                match self.shape(object.trim(), scope) {
                    Shape::Object { kind, by_ref: true } => format!("o.k{kind}.as_ref().filter(|_| r.coin())"),
                    _ => return Some("&[]".to_string()),
                }
            } else {
                let mut element_prelude = String::new();
                let expr = self.argument_random(element, scope, codecs, &mut element_prelude, counter)?;
                if !element_prelude.is_empty() || expr.starts_with('&') {
                    return Some("&[]".to_string());
                }
                expr
            };
            *counter += 1;
            let var = format!("s{counter}");
            let _ = writeln!(prelude, "        let {var}: Vec<_> = (0..count).map(|_| {value}).collect();");
            return Some(format!("{var}.as_slice()"));
        }
        if t == "&str" {
            return Some("\"\"".to_string());
        }
        if t == "&CStr" || t.ends_with("::CStr") && t.starts_with('&') {
            return Some("c\"\"".to_string());
        }
        match self.shape(t, scope) {
            Shape::Object { kind, by_ref } => {
                *counter += 1;
                let var = format!("v{counter}");
                let _ = writeln!(prelude, "        let Some({var}) = o.k{kind}.as_ref() else {{ return false; }};");
                return Some(if by_ref { var } else { format!("{var}.clone()") });
            }
            Shape::Ref { path, kinds } => {
                if kinds.is_empty() {
                    return None;
                }
                *counter += 1;
                let var = format!("v{counter}");
                let pushes: String = kinds.iter().map(|&k| format!(" if let Some(x) = o.k{k}.as_ref() {{ choices.push({path}::{}(x)); }}", self.g.kinds[k].name)).collect();
                let _ = writeln!(prelude, "        let {var} = {{ let mut choices = Vec::new();{pushes} if choices.is_empty() {{ return false; }} let at = r.below(choices.len()); choices.swap_remove(at) }};");
                return Some(var);
            }
            Shape::Unknown { .. } | Shape::Unmakeable => return None,
            Shape::Other => {}
        }
        let bare = t.trim_start_matches('&').trim();
        let reference = if t.starts_with('&') { "&" } else { "" };
        if bare.contains("::structures::") {
            if bare.ends_with("::Guid") || codecs.defaults.contains(bare) {
                return Some(format!("{reference}Default::default()"));
            }
            let (module, ident, size) = codecs.decoders.get(bare)?;
            return Some(format!("{reference}{module}::decode_{ident}(&r.bytes({size}))"));
        }
        if bare.contains("::enumerations::") {
            return Some(format!("{reference}{bare}(r.value())"));
        }
        if ["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "bool", "f32", "f64"].contains(&bare) {
            return Some(format!("{reference}r.value::<{bare}>()"));
        }
        if bare.contains("Foreign") || bare.contains("impl ") || bare.contains("dyn ") {
            return None;
        }
        Some(format!("{reference}Default::default()"))
    }

    /// The kinds in the driver's reach from API `api`: its own and those of the APIs it extends.
    fn driver_scope(&self, api: usize) -> Vec<usize> {
        let spec = self.g.apis[api];
        let apis: BTreeSet<usize> = std::iter::once(api).chain(spec.supertraits.iter().filter_map(|folder| self.g.apis.iter().position(|a| a.folder == *folder))).collect();
        (0..self.g.kinds.len()).filter(|k| apis.contains(&self.g.kinds[*k].api)).collect()
    }

    /// Every method the calling glue carries on a kind's objects: the interface it is of, its trait path, its slot and
    /// the API its signature is rendered for.
    fn kind_methods(&self, kind: usize) -> Vec<(crate::emit::Key, String, usize, &crate::metadata::Method, Option<super::Glued>)> {
        let g = self.g;
        let k = &g.kinds[kind];
        let spec = g.apis[k.api];
        let mut out = Vec::new();
        let mut traits: Vec<crate::emit::Key> = Vec::new();
        for newest in &k.newest {
            for base in g.bases_of(newest) {
                if g.interface_api.contains_key(&base) && !traits.contains(&base) {
                    traits.push(base);
                }
            }
        }
        let saved = self.current.replace(k.api);
        for interface_key in &traits {
            for slot in self.slots(interface_key).into_iter().filter(|slot| slot.interface == *interface_key && g.read.interfaces[interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                out.push((interface_key.clone(), self.trait_path(interface_key), k.api, slot.method, slot.glued));
            }
        }
        let faces: Vec<crate::emit::Key> = spec
            .faces_of(&k.root.1)
            .into_iter()
            .filter_map(|face| g.interface_api.keys().find(|key| key.1 == *face).cloned())
            .collect();
        for face in &faces {
            let face_api = g.interface_api[face];
            for interface_key in self.chain(face) {
                if traits.contains(&interface_key) {
                    continue;
                }
                traits.push(interface_key.clone());
                self.current.set(face_api);
                for slot in self.slots(face).into_iter().filter(|slot| slot.interface == interface_key && g.read.interfaces[&interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                    out.push((interface_key.clone(), self.trait_path(&interface_key), face_api, slot.method, slot.glued));
                }
            }
        }
        self.current.set(saved);
        out
    }

    /// One call of the driver or a maker: the lines that bind its receiver (`this`, of kind `bind`) and arguments, and
    /// the call; none when an argument cannot be made.
    #[allow(clippy::too_many_arguments, reason = "the call's own parts")]
    fn driven_call(&self, place: &str, bind: Option<usize>, receiver_type: &str, trait_path: &str, rust_name: &str, inputs: &[(String, String)], scope: &[usize], codecs: &Codecs, skip: &str, riid: Option<(&str, &str)>) -> Option<(String, String)> {
        let mut prelude = String::new();
        let receiver = match bind {
            Some(kind) => {
                if skip == "None" {
                    let _ = writeln!(prelude, "        let this = o.k{kind}.as_ref()?;");
                } else {
                    let _ = writeln!(prelude, "        let Some(this) = o.k{kind}.as_ref() else {{ return {skip}; }};");
                }
                "this"
            }
            None => "&o.api",
        };
        let mut counter = 0;
        let mut args = Vec::new();
        for (name, ty) in inputs {
            let iid = riid.filter(|(riid_name, _)| riid_name == name).map(|(_, iid)| iid);
            args.push(self.argument(place, name, ty, scope, codecs, &mut prelude, &mut counter, skip, iid)?);
        }
        // By the trait's path, the receiver's type inferred: the linter's conformance_untested_method reads a call of
        // `Trait::method(value)`, not one through a qualified self type (`<T as Trait>::method`).
        let _ = receiver_type;
        let call = format!("{trait_path}::{rust_name}({receiver}{})", args.iter().map(|a| format!(", {a}")).collect::<String>());
        Some((prelude, call))
    }

    /// One fuzzed call: the receiver bound as `driven_call` binds it, every argument random.
    #[allow(clippy::too_many_arguments, reason = "the call's own parts")]
    fn fuzzed_call(&self, bind: Option<usize>, receiver_type: &str, trait_path: &str, rust_name: &str, inputs: &[(String, String)], scope: &[usize], codecs: &Codecs) -> Option<(String, String)> {
        let mut prelude = String::new();
        let receiver = match bind {
            Some(kind) => {
                let _ = writeln!(prelude, "        let Some(this) = o.k{kind}.as_ref() else {{ return false; }};");
                "this"
            }
            None => "&o.api",
        };
        // Every slice of one call has this length, so that the arrays a count covers agree.
        prelude.push_str("        let count = r.below(SLICE_LENGTHS);\n");
        let mut counter = 0;
        let mut args = Vec::new();
        for (_, ty) in inputs {
            args.push(self.argument_random(ty, scope, codecs, &mut prelude, &mut counter)?);
        }
        let call = format!("<{receiver_type} as {trait_path}>::{rust_name}({receiver}{})", args.iter().map(|a| format!(", {a}")).collect::<String>());
        Some((prelude, call))
    }

    /// The makers of one method: what it hands out (an object, objects of a tuple, or an interface by id, one maker per
    /// kind it can name).
    #[allow(clippy::too_many_arguments, reason = "the method's own parts")]
    fn makers_of(&self, place: &str, bind: Option<usize>, receiver_type: &str, trait_path: &str, rust_name: &str, inputs: &[(String, String)], result: &str, glued: &super::Glued, scope: &[usize], codecs: &Codecs) -> Vec<Maker> {
        let g = self.g;
        let fallible = result.starts_with("Result<");
        let inner = if fallible { result["Result<".len()..].rsplit_once(',').map_or(result, |(value, _)| value).trim() } else { result.trim() };
        let parts: Vec<String> = if inner.starts_with('(') && inner.ends_with(')') { split_top(&inner[1..inner.len() - 1]) } else { vec![inner.to_string()] };
        let mut makers = Vec::new();
        let mut stores: Vec<(usize, String)> = Vec::new();
        let mut unknown: Option<(String, Vec<usize>)> = None;
        for (position, part) in parts.iter().enumerate() {
            match self.shape(part, scope) {
                Shape::Object { kind, .. } => stores.push((kind, format!("p{position}"))),
                Shape::Unknown { path, kinds } if parts.len() == 1 => unknown = Some((path, kinds)),
                _ => {}
            }
        }
        let pattern = if parts.len() > 1 { format!("({})", (0..parts.len()).map(|p| format!("p{p}")).collect::<Vec<_>>().join(", ")) } else { "p0".to_string() };
        if !stores.is_empty()
            && let Some((prelude, call)) = self.driven_call(place, bind, receiver_type, trait_path, rust_name, inputs, scope, codecs, "None", None)
        {
            makers.push(Maker { place: place.to_string(), prelude, call, fallible, pattern, stores });
        }
        let riid = glued.parts.iter().find_map(|(_, part)| match part {
            Part::InterfaceOut { riid } => Some(glued.parts[*riid].0.clone()),
            _ => None,
        });
        if let (Some((path, kinds)), Some(riid)) = (unknown, riid) {
            for kind in kinds {
                let Some(iid) = g.kinds[kind].newest.first().map(|newest| self.iid(newest)) else { continue };
                if let Some((prelude, call)) = self.driven_call(place, bind, receiver_type, trait_path, rust_name, inputs, scope, codecs, "None", Some((&riid, &iid))) {
                    let pattern = format!("{path}::{}(p0)", g.kinds[kind].name);
                    makers.push(Maker { place: format!("{place} for {}", g.kinds[kind].name), prelude, call, fallible, pattern, stores: vec![(kind, "p0".to_string())] });
                }
            }
        }
        makers
    }

    /// The driver of API `api`'s hostile crate: the objects it can make, the makers, and a call of every method.
    fn driver(&self, api: usize, codecs: &Codecs) -> (String, usize, usize) {
        let g = self.g;
        let spec = g.apis[api];
        let Some(top_name) = spec.top_trait else { return (String::new(), 0, 0) };
        // The driver reaches the implementation through `served`, which an API with no exports does not have.
        if self.exported(api).is_empty() {
            return (String::new(), 0, 0);
        }
        let scope = self.driver_scope(api);
        let top_system = format!("{}::System{top_name}", direct_alias(spec.folder));
        let top_trait = format!("{}::{top_name}", alias(spec));
        let system = |k: usize| format!("{}::System{}<Api>", direct_alias(g.apis[g.kinds[k].api].folder), g.kinds[k].name);
        let mut text = String::new();
        text.push_str(DRIVER_BASE);
        let _ = writeln!(text, "\n/// The calling glue's API value over this hostile implementation, served by the serving glue.\npub type Api = {top_system};\n");
        let _ = writeln!(text, "/// One object of each kind the driver made, by kind.\npub struct Objects {{\n    pub api: Api,");
        for &k in &scope {
            let _ = writeln!(text, "    /// `{}`.\n    pub k{k}: Option<{}>,", g.kinds[k].name, system(k));
        }
        let _ = writeln!(text, "}}\n\nimpl Objects {{\n    /// No object yet, and the API served by the serving glue over the hostile implementation.\n    pub fn new() -> Objects {{\n        Objects {{\n            api: {}::served(HostileLibrary),", direct_alias(spec.folder));
        for &k in &scope {
            let _ = writeln!(text, "            k{k}: None,");
        }
        let held: Vec<String> = scope.iter().map(|k| format!("self.k{k}.is_some()")).collect();
        let _ = writeln!(text, "        }}\n    }}\n\n    /// How many kinds have an object.\n    pub fn kinds_held(&self) -> usize {{\n        [{}].iter().filter(|held| **held).count()\n    }}\n}}\n", held.join(", "));
        let _ = writeln!(text, "impl Default for Objects {{\n    fn default() -> Objects {{\n        Objects::new()\n    }}\n}}\n");
        // The API's own kinds by name, so that a test can take one and give one it made with a valid description.
        let _ = writeln!(text, "impl Objects {{");
        for &k in scope.iter().filter(|&&k| g.kinds[k].api == api) {
            let name = snake(&g.kinds[k].name);
            let _ = writeln!(text, "    /// The `{}` held, if any.\n    pub fn {name}(&self) -> Option<&{}> {{\n        self.k{k}.as_ref()\n    }}\n", g.kinds[k].name, system(k));
            let _ = writeln!(text, "    /// Holds a `{}`, in place of any held.\n    pub fn put_{name}(&mut self, value: {}) {{\n        self.k{k} = Some(value);\n    }}\n", g.kinds[k].name, system(k));
        }
        let _ = writeln!(text, "}}\n");
        let mut calls: Vec<(String, String, String)> = Vec::new();
        let mut fuzzed: Vec<(String, String, String)> = Vec::new();
        let mut makers: Vec<Maker> = Vec::new();
        for &k in &scope {
            for (interface_key, trait_path, sig_api, method, glued) in self.kind_methods(k) {
                let place = format!("{}::{}", interface_key.1, method.name);
                let Some(glued) = glued else { continue };
                if matches!(g.treatment(&place), Some(crate::emit::kinds::Treatment::Refused(_))) {
                    continue;
                }
                let saved = self.current.replace(sig_api);
                let (inputs, result) = self.trait_signature(method, sig_api, "Self::Api", &place);
                self.current.set(saved);
                let rust_name = Self::method_name(method);
                if let Some((prelude, call)) = self.driven_call(&place, Some(k), &system(k), &trait_path, &rust_name, &inputs, &scope, codecs, "false", None) {
                    calls.push((format!("{place} on {}", g.kinds[k].name), prelude, call));
                }
                if let Some((prelude, call)) = self.fuzzed_call(Some(k), &system(k), &trait_path, &rust_name, &inputs, &scope, codecs) {
                    fuzzed.push((format!("{place} on {}", g.kinds[k].name), prelude, call));
                }
                makers.extend(self.makers_of(&place, Some(k), &system(k), &trait_path, &rust_name, &inputs, &result, &glued, &scope, codecs));
            }
        }
        for function in self.exported(api) {
            let rust_name = crate::names::escaped(snake(crate::names::strip_first(&function.name, spec.function_prefixes)));
            let mut renamed = function.clone();
            renamed.name = crate::names::strip_first(&function.name, spec.function_prefixes).to_string();
            let Ok(glued) = self.glue_of(function, &function.name) else { continue };
            if matches!(g.treatment(&function.name), Some(crate::emit::kinds::Treatment::Refused(_))) {
                continue;
            }
            let (inputs, result) = self.trait_signature(&renamed, api, "Self", &function.name);
            if let Some((prelude, call)) = self.driven_call(&function.name, None, "Api", &top_trait, &rust_name, &inputs, &scope, codecs, "false", None) {
                calls.push((function.name.clone(), prelude, call));
            }
            if let Some((prelude, call)) = self.fuzzed_call(None, "Api", &top_trait, &rust_name, &inputs, &scope, codecs) {
                fuzzed.push((function.name.clone(), prelude, call));
            }
            makers.extend(self.makers_of(&function.name, None, "Api", &top_trait, &rust_name, &inputs, &result, &glued, &scope, codecs));
        }
        let _ = writeln!(text, "/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new; returns how\n/// many kinds hold one.\npub fn make_all(o: &mut Objects) -> usize {{\n    for _ in 0..6 {{\n        let before = o.kinds_held();");
        for maker in &makers {
            let wanted: Vec<String> = maker.stores.iter().map(|(k, _)| format!("o.k{k}.is_none()")).collect();
            let answer = if maker.fallible { "answer.ok()" } else { "Some(answer)" };
            let _ = writeln!(text, "        // {}\n        if {} {{\n            let answer = {{\n                let o = &*o;\n                made(|| {{\n{}        let answer = {};\n        {answer}\n                }})\n            }};", maker.place, wanted.join(" || "), maker.prelude, maker.call);
            if let [(kind, var)] = maker.stores.as_slice() {
                let _ = writeln!(text, "            if let Some({}) = answer\n                && o.k{kind}.is_none()\n            {{\n                o.k{kind} = Some({var});\n            }}\n        }}", maker.pattern);
            } else {
                let _ = writeln!(text, "            if let Some({}) = answer {{", maker.pattern);
                for (kind, var) in &maker.stores {
                    let _ = writeln!(text, "                if o.k{kind}.is_none() {{\n                    o.k{kind} = Some({var});\n                }}");
                }
                let _ = writeln!(text, "            }}\n        }}");
            }
        }
        let _ = writeln!(text, "        if o.kinds_held() == before {{\n            break;\n        }}\n    }}\n    o.kinds_held()\n}}\n");
        let _ = writeln!(text, "/// Calls every method the calling glue carries on every object held, and the library's exports, each with arguments of\n/// defaults and `bytes()` for every caller's bytes; returns what was called.\npub fn drive(o: &Objects, bytes: &dyn Fn() -> LyingBytes) -> Driven {{\n    drive_where(o, bytes, &|_| true)\n}}\n");
        let _ = writeln!(text, "/// `drive`, of the methods of the interfaces (or the exports) `wanted` names only.\npub fn drive_where(o: &Objects, bytes: &dyn Fn() -> LyingBytes, wanted: &dyn Fn(&str) -> bool) -> Driven {{\n    let mut driven = Driven::default();");
        let mut interface_names: BTreeSet<String> = BTreeSet::new();
        for (place, prelude, call) in &calls {
            let interface = place.split("::").next().unwrap_or(place).split(' ').next().unwrap_or(place);
            interface_names.insert(interface.to_string());
            let _ = writeln!(text, "    // {place}\n    if wanted({}) {{\n        driven.attempt(|| {{\n{prelude}        let _ = {call};\n        true\n        }});\n    }}", crate::names::snake_case(interface).to_ascii_uppercase());
        }
        let _ = writeln!(text, "    driven\n}}\n");
        for interface in &interface_names {
            let _ = writeln!(text, "/// The interface (or export) `{interface}`, by its name in the header.\nconst {}: &str = \"{interface}\";", crate::names::snake_case(interface).to_ascii_uppercase());
        }
        // The effects judgments of the APIs in reach (src/api.rs), and the methods the rows keep from the system.
        let in_reach: BTreeSet<usize> = scope.iter().map(|&k| g.kinds[k].api).chain(std::iter::once(api)).collect();
        let mut groups: Vec<String> = Vec::new();
        for &a in &in_reach {
            for effects in g.apis[a].effects {
                let listed: Vec<String> = effects.interfaces.iter().map(|name| format!("\"{name}\"")).collect();
                groups.push(format!("    (&[{}], \"{}\"),", listed.join(", "), effects.why.replace('"', "'")));
            }
        }
        let _ = writeln!(text, "/// Each effects judgment of the APIs in reach (src/api.rs `effects`): the interfaces (or exports) judged together, and\n/// the judgment.\npub const EFFECTS: &[(&[&str], &str)] = &[\n{}\n];\n", groups.join("\n"));
        let mut never: Vec<String> = Vec::new();
        for &a in &in_reach {
            for pin in g.apis[a].pins {
                if matches!(pin.hold, crate::api::Hold::NotCalled { .. }) && !never.contains(&pin.method.to_string()) {
                    never.push(pin.method.to_string());
                }
            }
        }
        let never_listed: Vec<String> = never.iter().map(|place| format!("    \"{place}\",")).collect();
        let _ = writeln!(text, "/// The methods the rows keep from the system (never called): none reaches an implementation.\npub const NEVER_REACHED: &[&str] = &[\n{}\n];\n", never_listed.join("\n"));
        // The fuzzer: each method a function of random arguments, picked at random; objects dropped and made again.
        for (index, (place, prelude, call)) in fuzzed.iter().enumerate() {
            let _ = writeln!(text, "/// {place}, of random arguments.\nfn fuzz_{index}(o: &Objects, r: &mut Rng) -> bool {{\n{prelude}        let _ = {call};\n        true\n}}\n");
        }
        // Kinds a maker of the library's exports makes are kept: zeros make none of them again.
        let kept: BTreeSet<usize> = makers.iter().filter(|maker| !maker.prelude.contains("let this")).flat_map(|maker| maker.stores.iter().map(|(k, _)| *k)).collect();
        let _ = writeln!(text, "/// Drops each object but those the library's exports made, one time in `one_in`.\nfn drop_some(o: &mut Objects, r: &mut Rng, one_in: usize) {{");
        for &k in scope.iter().filter(|k| !kept.contains(k)) {
            let _ = writeln!(text, "    if r.below(one_in) == 0 {{\n        o.k{k} = None;\n    }}");
        }
        let _ = writeln!(text, "}}\n");
        let listed: Vec<String> = (0..fuzzed.len()).map(|index| format!("fuzz_{index}")).collect();
        let _ = writeln!(text, "/// Every method's fuzzer, picked from at random.\nconst FUZZERS: [fn(&Objects, &mut Rng) -> bool; {}] = [{}];\n", fuzzed.len(), listed.join(", "));
        let _ = writeln!(text, "/// Calls `calls` methods picked at random, each of random arguments; every `REMAKE_EVERY` calls drops some objects and\n/// makes them again (`reseed` gives those of valid descriptions the makers cannot make). A call that panics back (the caller's own lie) is counted.\npub fn fuzz(o: &mut Objects, r: &mut Rng, calls: u64, reseed: &dyn Fn(&mut Objects)) -> Fuzzed {{\n    let mut fuzzed = Fuzzed::default();\n    for n in 1..=calls {{\n        if n % REMAKE_EVERY == 0 {{\n            drop_some(o, r, DROP_ONE_IN);\n            make_all(o);\n            reseed(o);\n            make_all(o);\n            fuzzed.remade += 1;\n        }}\n        let objects = &*o;\n        if let Some(fuzzer) = FUZZERS.get(r.below(FUZZERS.len())) {{\n            fuzzed.driven.attempt(|| fuzzer(objects, r));\n        }}\n    }}\n    fuzzed\n}}");
        let _ = writeln!(text, "\n/// How many fuzzed calls between two drops and remakes of objects.\nconst REMAKE_EVERY: u64 = 4096;\n\n/// One object in this many is dropped at a remake.\nconst DROP_ONE_IN: usize = 4;");
        (text, calls.len(), makers.len())
    }
}

/// One maker: the method, the lines binding its arguments, the call, whether it answers a `Result`, the pattern its
/// answer is matched with, and where each object goes.
struct Maker {
    place: String,
    prelude: String,
    call: String,
    fallible: bool,
    pattern: String,
    stores: Vec<(usize, String)>,
}

/// Splits a tuple's types at their top-level commas.
fn split_top(list: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, ch) in list.char_indices() {
        match ch {
            '(' | '<' | '[' => depth += 1,
            ')' | '>' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(list[start..at].trim().to_string());
                start = at + 1;
            }
            _ => {}
        }
    }
    if !list[start..].trim().is_empty() {
        parts.push(list[start..].trim().to_string());
    }
    parts
}

/// The decoders of the structures each direct crate of the set writes, by the structure's path, and the structures
/// that derive `Default`.
pub(super) struct Codecs {
    decoders: BTreeMap<String, (String, String, u64)>,
    defaults: BTreeSet<String>,
    /// The data modules the hostile crate includes: module name and path from the crate.
    modules: Vec<(String, String)>,
    /// Every structure and enumeration of the set as the interface crates write them, for known values made without
    /// any code of the glue.
    pub(super) shapes: Shapes,
}

/// The structures (their fields, and whether each is a C union) and the enumerations (their values) of the set, read
/// from the interface crates' text: what known values are made of, so that neither the recorder's known answers nor
/// the suite's known arguments share any code with the glue they check.
#[derive(Clone, Debug, Default)]
pub(super) struct Shapes {
    /// Each structure's fields (name, type), by path; and whether it is a C union.
    structs: BTreeMap<String, (Vec<(String, String)>, bool)>,
    /// Each enumeration's values that pass the glue (declared, none a special value src/api.rs refuses or clears),
    /// as (constant path, value), by the enumeration's path.
    values: BTreeMap<String, Vec<(String, i64)>>,
    /// The structures and enumerations of one public integer field.
    newtypes: BTreeSet<String>,
    /// The integer type of each structure or enumeration of one public integer field, by path.
    inner: BTreeMap<String, String>,
    /// The value of each `usize` constant (an array's length), by path.
    lengths: BTreeMap<String, usize>,
}

/// The Rust integer types a known value is written for as a literal.
const KNOWN_INTEGERS: [&str; 10] = ["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize"];
/// A known integer is one of `KNOWN_SPAN` values from 1: never zero, and within every integer type.
const KNOWN_SPAN: usize = 126;
/// What a known value's salt moves by from one field to the next.
const KNOWN_STEP: usize = 7;

impl Shapes {
    /// A Rust expression of a known value of type `ty`, made only of literals and the interface crates' constants: never
    /// the type's default where it has another value (`salt` folded into 1 to 126 for each integer, moving by field), so
    /// that a glue dropping, zeroing or mixing up a value is seen. `valid`: an enumeration takes a value the glue passes
    /// (its first declared nonzero value no special line refuses), as the suite's arguments must; otherwise any. A C
    /// union is all zero bytes (its members read the same bytes, so no other value is the same in every member). None
    /// for a type of no known value (an object, a structure generic over the API).
    pub(super) fn known(&self, ty: &str, salt: usize, valid: bool) -> Option<String> {
        let ty = ty.trim();
        let number = 1 + salt % KNOWN_SPAN;
        if KNOWN_INTEGERS.contains(&ty) {
            return Some(format!("{number}_{ty}"));
        }
        if ["f32", "f64"].contains(&ty) {
            return Some(format!("{number}.5_{ty}"));
        }
        if ty == "bool" {
            return Some("true".to_string());
        }
        if ty.ends_with("CString") {
            return Some("std::ffi::CString::from(c\"known\")".to_string());
        }
        if let Some(element) = ty.strip_prefix("Vec<").and_then(|rest| rest.strip_suffix('>')) {
            return self.known(element, salt, valid).map(|value| format!("vec![{value}]"));
        }
        if ty.starts_with("Option<") {
            return Some("None".to_string());
        }
        if let Some((element, _)) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).and_then(|inner| inner.rsplit_once(';')) {
            return self.known(element, salt, valid).map(|value| format!("core::array::from_fn(|_| {value})"));
        }
        if let Some(choices) = self.values.get(ty) {
            if valid {
                return Some(choices.iter().find(|(_, value)| *value != 0).or(choices.first()).map_or(format!("{ty}(0)"), |(name, _)| name.clone()));
            }
            return Some(format!("{ty}({number})"));
        }
        if self.newtypes.contains(ty) {
            // A module handle names code the system would load: an argument passes none.
            if valid && ty.ends_with("::Hmodule") {
                return Some(format!("{ty}(0)"));
            }
            return Some(format!("{ty}({number})"));
        }
        let (fields, union) = self.structs.get(ty)?;
        if *union {
            return self.union_known(ty, salt).or_else(|| self.zero(ty));
        }
        let parts: Option<Vec<String>> = fields.iter().enumerate().map(|(at, (name, field))| self.known(field, salt + KNOWN_STEP * (at + 1), valid).map(|value| format!("{name}: {value}"))).collect();
        parts.map(|parts| format!("{ty} {{ {} }}", parts.join(", ")))
    }

    /// A known value of the C union `ty` the same in every member: bytes of 1 to 126 (salted; no `f32` or `f64` of them
    /// is a NaN or an infinity), the widest member read from them, the bytes it does not read (its padding) made zero,
    /// and every member read from those bytes as C lays it out. None where a member's layout is not known here (a `bool`,
    /// whose C type the Rust form does not say).
    fn union_known(&self, ty: &str, salt: usize) -> Option<String> {
        let (fields, _) = self.structs.get(ty)?;
        let (size, _) = self.layout(ty)?;
        let mut bytes: Vec<u8> = (0..size).map(|at| u8::try_from(1 + (salt + at) % KNOWN_SPAN).unwrap_or(1)).collect();
        let mut widest: Option<(usize, &str)> = None;
        for (_, field) in fields {
            let (field_size, _) = self.layout(field)?;
            if widest.is_none_or(|(most, _)| field_size > most) {
                widest = Some((field_size, field));
            }
        }
        let (_, widest) = widest?;
        let mut read = vec![false; size];
        self.read_value(widest, &bytes, 0, &mut read)?;
        for (byte, was_read) in bytes.iter_mut().zip(&read) {
            if !was_read {
                *byte = 0;
            }
        }
        let mut ignored = vec![false; size];
        let parts: Option<Vec<String>> = fields.iter().map(|(name, field)| self.read_value(field, &bytes, 0, &mut ignored).map(|value| format!("{name}: {value}"))).collect();
        parts.map(|parts| format!("{ty} {{ {} }}", parts.join(", ")))
    }

    /// The C size and alignment of `ty` as the Rust form names it (x86-64 and ARM64 Windows); none where not known here.
    fn layout(&self, ty: &str) -> Option<(usize, usize)> {
        let ty = ty.trim();
        match ty {
            "u8" | "i8" => return Some((1, 1)),
            "u16" | "i16" => return Some((2, 2)),
            "u32" | "i32" | "f32" => return Some((4, 4)),
            "u64" | "i64" | "f64" | "usize" | "isize" => return Some((8, 8)),
            _ => {}
        }
        if let Some((element, length)) = self.array_of(ty) {
            let (size, align) = self.layout(element)?;
            return Some((size.checked_mul(length)?, align));
        }
        if let Some(inner) = self.inner.get(ty) {
            return self.layout(inner);
        }
        let (fields, union) = self.structs.get(ty)?;
        let (mut size, mut align) = (0_usize, 1_usize);
        for (_, field) in fields {
            let (field_size, field_align) = self.layout(field)?;
            align = align.max(field_align);
            size = if *union { size.max(field_size) } else { size.next_multiple_of(field_align) + field_size };
        }
        Some((size.next_multiple_of(align), align))
    }

    /// The element type and length of an array type `[T; N]` (N a literal or a constant of the set).
    fn array_of<'t>(&self, ty: &'t str) -> Option<(&'t str, usize)> {
        let (element, length) = ty.strip_prefix('[')?.strip_suffix(']')?.rsplit_once(';')?;
        let length = length.trim();
        let length = length.parse().ok().or_else(|| self.lengths.get(length).copied())?;
        Some((element.trim(), length))
    }

    /// A Rust expression of the value of type `ty` that C reads from `bytes` at `at` (little-endian), marking each byte
    /// read in `read`; none where the layout is not known here.
    fn read_value(&self, ty: &str, bytes: &[u8], at: usize, read: &mut [bool]) -> Option<String> {
        let ty = ty.trim();
        let integer = |size: usize, read: &mut [bool]| -> Option<u64> {
            let mut value = 0_u64;
            for index in (0..size).rev() {
                value = (value << 8) | u64::from(*bytes.get(at + index)?);
                *read.get_mut(at + index)? = true;
            }
            Some(value)
        };
        match ty {
            "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
                let (size, _) = self.layout(ty)?;
                return integer(size, read).map(|value| format!("{value:#x}_{ty}"));
            }
            "f32" => return integer(4, read).map(|value| format!("f32::from_bits({value:#x})")),
            "f64" => return integer(8, read).map(|value| format!("f64::from_bits({value:#x})")),
            _ => {}
        }
        if let Some((element, length)) = self.array_of(ty) {
            let (size, _) = self.layout(element)?;
            let items: Option<Vec<String>> = (0..length).map(|index| self.read_value(element, bytes, at + index * size, read)).collect();
            return items.map(|items| format!("[{}]", items.join(", ")));
        }
        if let Some(inner) = self.inner.get(ty) {
            return self.read_value(inner, bytes, at, read).map(|value| format!("{ty}({value})"));
        }
        let (fields, union) = self.structs.get(ty)?;
        let mut offset = 0_usize;
        let mut parts = Vec::new();
        for (name, field) in fields {
            let (field_size, field_align) = self.layout(field)?;
            let field_at = if *union { 0 } else { offset.next_multiple_of(field_align) };
            parts.push(format!("{name}: {}", self.read_value(field, bytes, at + field_at, read)?));
            offset = field_at + field_size;
        }
        Some(format!("{ty} {{ {} }}", parts.join(", ")))
    }

    /// A Rust expression of type `ty` that is all zero bytes, made only of literals.
    fn zero(&self, ty: &str) -> Option<String> {
        let ty = ty.trim();
        if KNOWN_INTEGERS.contains(&ty) {
            return Some(format!("0_{ty}"));
        }
        if ["f32", "f64"].contains(&ty) {
            return Some(format!("0.0_{ty}"));
        }
        if ty == "bool" {
            return Some("false".to_string());
        }
        if let Some((element, _)) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).and_then(|inner| inner.rsplit_once(';')) {
            return self.zero(element).map(|value| format!("core::array::from_fn(|_| {value})"));
        }
        if self.values.contains_key(ty) || self.newtypes.contains(ty) {
            return Some(format!("{ty}(0)"));
        }
        let (fields, _) = self.structs.get(ty)?;
        let parts: Option<Vec<String>> = fields.iter().map(|(name, field)| self.zero(field).map(|value| format!("{name}: {value}"))).collect();
        parts.map(|parts| format!("{ty} {{ {} }}", parts.join(", ")))
    }
}

/// Sets the writer's structure values from the set's crates (a decoder of zero bytes, or `Default`), and returns the
/// data modules they name: module name and path from an implementation's crate.
pub(super) fn struct_values(writer: &Writer<'_, '_>, crates: &Path, api: usize) -> Vec<(String, String)> {
    let codecs = codecs(writer, crates, api);
    let mut values = writer.struct_values.borrow_mut();
    values.clear();
    for path in &codecs.defaults {
        values.insert(path.clone(), "Default::default()".to_string());
    }
    for (path, (module, ident, size)) in &codecs.decoders {
        values.entry(path.clone()).or_insert_with(|| format!("{module}::decode_{ident}(&[u8::MIN; {size}])"));
    }
    *writer.shapes.borrow_mut() = codecs.shapes.clone();
    codecs.modules
}

/// Reads the decoders and `Default` structures the set's crates were written with.
fn codecs(writer: &Writer<'_, '_>, crates: &Path, api: usize) -> Codecs {
    let g = writer.g;
    let mut decoders = BTreeMap::new();
    let mut defaults = BTreeSet::new();
    let mut newtypes = BTreeSet::new();
    let mut modules = Vec::new();
    for spec in g.apis.iter() {
        for (file, module) in [("structures.rs", "structures"), ("enumerations.rs", "enumerations")] {
            let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join(file)) else { continue };
            for rest in text.lines().filter_map(|line| line.strip_prefix("pub struct ")) {
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                let field = rest[name.len()..].trim_start_matches("(pub ").trim_end_matches(");");
                if rest[name.len()..].starts_with("(pub ") && ["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize"].contains(&field) {
                    newtypes.insert(format!("{}::{module}::{name}", alias(spec)));
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join("structures.rs")) {
            let mut derives_default = false;
            for line in text.lines() {
                if line.starts_with("#[derive(") {
                    derives_default = line.contains("Default");
                } else if let Some(rest) = line.strip_prefix("pub struct ") {
                    let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                    if derives_default {
                        defaults.insert(format!("{}::structures::{name}", alias(spec)));
                    }
                    derives_default = false;
                }
            }
        }
        let data = crates.join(spec.folder).join("implementation").join("direct").join("data.rs");
        let Ok(text) = std::fs::read_to_string(&data) else { continue };
        let module = format!("data_{}", spec.folder.replace('-', "_"));
        let from = if spec.folder == g.apis[api].folder { "../direct/data.rs".to_string() } else { format!("../../../{}/implementation/direct/data.rs", spec.folder) };
        let mut size = 0;
        let mut used = false;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("/// Reads `")
                && let Some((_, after)) = rest.split_once("` from the ")
            {
                size = after.split(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("pub fn decode_")
                && let Some((ident, after)) = rest.split_once("(bytes: &[u8]) -> ")
            {
                let path = after.trim_end_matches('{').trim().to_string();
                if size > 0 && !path.contains('<') {
                    decoders.entry(path).or_insert_with(|| (module.clone(), ident.to_string(), size));
                    used = true;
                }
            }
        }
        if used {
            modules.push((module, from));
        }
    }
    let shapes = shapes_of(g, crates, &newtypes);
    Codecs { decoders, defaults, modules, shapes }
}

/// Writes API `api`'s hostile crate under its folder, and returns the report's line.
pub(in crate::emit) fn write_hostile(writer: &Writer<'_, '_>, api: usize, crate_folder: &Path) -> Result<String, String> {
    let g = writer.g;
    writer.current.set(api);
    let spec = g.apis[api];
    let crates = crate_folder.parent().ok_or("no crates folder")?;
    let codecs = codecs(writer, crates, api);
    let _ = struct_values(writer, crates, api);
    writer.hostile.set(true);
    let recorder = writer.recorder(api);
    writer.hostile.set(false);
    writer.struct_values.borrow_mut().clear();
    writer.current.set(api);
    let (driver, calls, makers) = writer.driver(api, &codecs);
    // The rows that answer for the system (src/api.rs): a call reaching the implementation with other arguments is one
    // the glue let through.
    let mut answered: Vec<String> = Vec::new();
    for other in g.apis.iter() {
        for pin in other.pins {
            if let crate::api::Hold::Answers { value, .. } = pin.hold {
                let line = format!("    (\"{}\", \"{value}\"),", pin.method);
                if !answered.contains(&line) {
                    answered.push(line);
                }
            }
        }
    }
    let answered = format!("/// The methods a row answers for the system, and the argument it answers with (src/api.rs).\npub const ANSWERED: &[(&str, &str)] = &[\n{}\n];\n", answered.join("\n"));
    let mut text = format!("{recorder}\n{answered}\n{driver}");
    // The recorder's names become the hostile implementation's.
    text = text.replace("Recorder", "HostileLibrary").replace("Recorded", "Hostile").replace("_recorder", "_hostile").replace("recorder", "hostile implementation");
    text = text.replacen("//! A recording implementation of", "//! The hostile implementation (src/glue_hostile.rs) of", 1);
    let mut modules = String::new();
    for (module, from) in &codecs.modules {
        let _ = writeln!(modules, "#[path = \"{from}\"]\nmod {module};");
    }
    // After the inner attributes: the data modules, and the imports of every crate named.
    let mut imports: Vec<String> = Vec::new();
    for other in g.apis.iter() {
        let named = |needle: &str| text.lines().any(|line| !line.starts_with("use ") && !line.starts_with("//") && line.contains(needle));
        if named(&format!("{}::", alias(other))) {
            imports.push(format!("use {} as {};", package(other).replace('-', "_"), alias(other)));
        }
        if named(&format!("{}::", hostile_alias(other.folder))) {
            imports.push(format!("use {}_hostile as {};", package(other).replace('-', "_"), hostile_alias(other.folder)));
        }
        if named(&format!("{}::", direct_alias(other.folder))) {
            imports.push(format!("use {}_direct as {};", package(other).replace('-', "_"), direct_alias(other.folder)));
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|line| !(line.starts_with("//!") || line.starts_with("#![") || line.is_empty())).unwrap_or(lines.len());
    let mut out = lines[..at].join("\n");
    out = out.replace("#![allow(unused_variables, unused_imports, clippy::let_unit_value, clippy::unit_arg, clippy::new_without_default, reason = \"generated: every method has the same shape\")]", "#![allow(unused_variables, unused_imports, dead_code, unreachable_code, clippy::let_unit_value, clippy::unit_arg, clippy::new_without_default, clippy::redundant_closure_call, clippy::too_many_lines, clippy::unnecessary_lazy_evaluations, clippy::option_map_unit_fn, reason = \"generated: every method and call has the same shape\")]");
    let _ = write!(out, "\n{modules}\n{}\n\n{}\n", imports.join("\n"), lines[at..].join("\n"));
    let folder = crate_folder.join("implementation").join("hostile");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    std::fs::write(folder.join("hostile.rs"), &out).map_err(|error| error.to_string())?;
    let mut manifest = format!(
        "[package]\nname = \"{}-hostile\"\ndescription = \"A hostile implementation of {}'s traits and a driver of every method through both glues, for tests only.\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"hostile.rs\"\n\n[dependencies]\n",
        package(spec),
        spec.folder
    );
    for other in g.apis.iter() {
        let _ = writeln!(manifest, "{} = {{ path = \"../../../{}\" }}", package(other), other.folder);
        let _ = writeln!(manifest, "{}-direct = {{ path = \"../../../{}/implementation/direct\" }}", package(other), other.folder);
        if other.folder != spec.folder && out.contains(&format!("{}::", hostile_alias(other.folder))) {
            let _ = writeln!(manifest, "{}-hostile = {{ path = \"../../../{}/implementation/hostile\" }}", package(other), other.folder);
        }
    }
    let _ = writeln!(manifest, "wcontainers-byte-sink = {{ path = \"../../../byte-sink\" }}\nwcontainers-byte-source = {{ path = \"../../../byte-source\" }}\nwcontainers-volatile-bytes = {{ path = \"../../../volatile-bytes\" }}");
    manifest.push_str("\n[lints]\nworkspace = true\n");
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    if spec.folder == "direct3d-11" {
        let tests = folder.join("tests");
        std::fs::create_dir_all(&tests).map_err(|error| error.to_string())?;
        std::fs::write(tests.join("lies.rs"), include_str!("../harness/hostile/lies.rs")).map_err(|error| error.to_string())?;
        std::fs::write(tests.join("effects.rs"), include_str!("../harness/hostile/effects.rs")).map_err(|error| error.to_string())?;
    }
    Ok(format!("HOSTILE {}: driver calls {calls}, makers {makers}\n", spec.folder))
}

/// The conformance suite's fixed part: what a drive counted, makers that may panic, and honest bytes for the caller's.
const SUITE_BASE: &str = r#"
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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make)).ok().flatten()
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
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![u8::MIN; self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

/// A method's failure is negative, as every HRESULT failure is.
fn failure_is_negative<T>(answer: Result<T, component_object_model::structures::HResult>) -> bool {
    if let Err(failure) = answer {
        assert!(failure.0 < 0, "a failure answered as {:#x}, which reads as success", failure.0);
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
struct Answers(std::sync::Mutex<Vec<String>>, Option<Received>, std::sync::atomic::AtomicUsize);

/// The implementation's own log of the arguments it received: how many so far, and those after the first n.
type Received = (fn() -> usize, fn(usize) -> Vec<String>);

impl Answers {
    fn with_received(received: Received) -> Answers {
        Answers(std::sync::Mutex::default(), Some(received), std::sync::atomic::AtomicUsize::new(0))
    }

    /// Marks where the implementation's log of arguments stands before a compared call.
    fn start(&self) {
        if let Some((logged, _)) = self.1 {
            self.2.store(logged(), std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn push(&self, line: String) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(line);
    }

    /// The caller's bytes after the method.
    fn bytes(&self, place: &str, bytes: &SuiteBytes) {
        let held = bytes.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        self.push(format!("{place} wrote {held:?}"));
    }

    fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()
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
        self.0.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
"#;

impl Writer<'_, '_> {
    /// The conformance suite of API `api`: generic over any implementation of its top trait, it makes an object of
    /// every kind it can and calls every method of every kind and every export on them, checking each answer as the
    /// interface promises (a failure negative, an object equal to its clone).
    fn suite(&self, api: usize, codecs: &Codecs) -> Option<(String, String)> {
        let g = self.g;
        let spec = g.apis[api];
        let top_name = spec.top_trait?;
        let top = format!("{}::{top_name}", alias(spec));
        let scope = self.driver_scope(api);
        let generic = |k: usize| {
            let kind_api = g.apis[g.kinds[k].api];
            format!("<A as {}::{}>::{}", alias(kind_api), kind_api.top_trait.unwrap_or(""), g.kinds[k].name)
        };
        let mut text = String::new();
        let _ = writeln!(text, "//! {}\n//!\n//! The conformance suite of {}, written against the traits alone (src/glue_hostile.rs): an object of every kind\n//! the implementation makes, every method of every kind and every export called on them, each answer checked as the\n//! interface promises.\n", crate::emit::HEADER, spec.folder);
        text.push_str("#![allow(unused_variables, unused_imports, dead_code, unreachable_code, clippy::let_unit_value, clippy::unit_arg, clippy::redundant_closure_call, clippy::too_many_lines, clippy::unnecessary_lazy_evaluations, clippy::option_map_unit_fn, clippy::bool_assert_comparison, clippy::eq_op, clippy::format_in_format_args, clippy::to_string_in_format_args, reason = \"generated: every call has the same shape\")]\n\n");
        text.push_str("@IMPORTS@\n");
        text.push_str(SUITE_BASE);
        let _ = writeln!(text, "\n/// One object of each kind the suite made, by kind.\nstruct Objects<A: {top}> {{\n    api: A,");
        for &k in &scope {
            let _ = writeln!(text, "    k{k}: Option<{}>,", generic(k));
        }
        let _ = writeln!(text, "}}\n\nimpl<A: {top}> Objects<A> {{\n    fn new(api: A) -> Objects<A> {{\n        Objects {{\n            api,");
        for &k in &scope {
            let _ = writeln!(text, "            k{k}: None,");
        }
        let held: Vec<String> = scope.iter().map(|k| format!("self.k{k}.is_some()")).collect();
        let _ = writeln!(text, "        }}\n    }}\n\n    fn kinds_held(&self) -> usize {{\n        [{}].iter().filter(|held| **held).count()\n    }}\n}}\n", if held.is_empty() { "false".to_string() } else { held.join(", ") });
        let mut calls: Vec<(String, String, String)> = Vec::new();
        let mut makers: Vec<Maker> = Vec::new();
        let check = |result: &str, call: &str, scope: &[usize]| -> String {
            if result.starts_with("Result<") {
                format!("failure_is_negative({call})")
            } else if matches!(self.shape(result, scope), Shape::Object { by_ref: false, .. }) {
                format!("equals_its_clone({call})")
            } else {
                format!("answered({call})")
            }
        };
        // Which answers the suite compares (`Answers`): those of a method both glues carry to the implementation, never
        // one a glue answers from its own record; by what each answer holds.
        let tally: RefCell<BTreeMap<&'static str, BTreeSet<String>>> = RefCell::new(BTreeMap::new());
        let count = |what: &'static str, place: &str| {
            tally.borrow_mut().entry(what).or_default().insert(place.to_string());
        };
        let compared = |place: &str, glued: bool, result: &str, prelude: &str, call: &str| -> String {
            let carried = self.outcomes.borrow().get(place).is_some_and(|outcome| outcome.serving.iter().any(|what| what.starts_with("glued")) && outcome.calling.iter().any(|what| what.starts_with("glued")));
            let rowed = self.g.exceptions().any(|exception| matches!(exception, Exception::Recorded { getter, .. } | Exception::Derived { getter, .. } if *getter == place));
            if !glued {
                count("not glued: refused by both glues, nothing asked of the implementation", place);
                return call.to_string();
            }
            if !carried || rowed {
                count("answered by a glue from its own record or row, never asking the implementation", place);
                return call.to_string();
            }
            let bytes = prelude.contains("let lying = bytes();");
            let mut lines = String::new();
            if result != "()" {
                let (summary, what) = answer_summary(result, "&answer", 0);
                count(what, place);
                let _ = write!(lines, "answers.push(format!(\"{place} -> {{}}\", {summary})); ");
            } else if bytes {
                count("returns nothing: compared, the caller's bytes it writes", place);
            } else {
                // Nothing comes back, but what the implementation received is still compared (`Answers`).
                count("returns nothing and writes nothing: compared, what the implementation received", place);
                let _ = write!(lines, "answers.push(\"{place} -> nothing\".to_string()); ");
            }
            if bytes {
                let _ = write!(lines, "answers.bytes(\"{place}\", &lying); ");
                count("of all compared, also the caller's bytes they write", place);
            }
            if lines.is_empty() { call.to_string() } else { format!("{{ let answer = {call}; {lines}answer }}") }
        };
        for &k in &scope {
            for (interface_key, trait_path, sig_api, method, glued) in self.kind_methods(k) {
                let place = format!("{}::{}", interface_key.1, method.name);
                let saved = self.current.replace(sig_api);
                let (inputs, result) = self.trait_signature(method, sig_api, "Self::Api", &place);
                self.current.set(saved);
                let rust_name = Self::method_name(method);
                if let Some((prelude, call)) = self.driven_call(&place, Some(k), &generic(k), &trait_path, &rust_name, &inputs, &scope, codecs, "false", None) {
                    let logged = compared(&place, glued.is_some(), &result, &prelude, &call);
                    calls.push((format!("{place} on {}", g.kinds[k].name), prelude, check(&result, &logged, &scope)));
                }
                if let Some(glued) = glued {
                    makers.extend(self.makers_of(&place, Some(k), &generic(k), &trait_path, &rust_name, &inputs, &result, &glued, &scope, codecs));
                }
            }
        }
        for function in self.exported(api) {
            let rust_name = crate::names::escaped(snake(crate::names::strip_first(&function.name, spec.function_prefixes)));
            let mut renamed = function.clone();
            renamed.name = crate::names::strip_first(&function.name, spec.function_prefixes).to_string();
            let (inputs, result) = self.trait_signature(&renamed, api, "Self", &function.name);
            if let Some((prelude, call)) = self.driven_call(&function.name, None, "A", &top, &rust_name, &inputs, &scope, codecs, "false", None) {
                let logged = compared(&function.name, self.glue_of(function, &function.name).is_ok(), &result, &prelude, &call);
                calls.push((function.name.clone(), prelude, check(&result, &logged, &scope)));
            }
            if let Ok(glued) = self.glue_of(function, &function.name) {
                makers.extend(self.makers_of(&function.name, None, "A", &top, &rust_name, &inputs, &result, &glued, &scope, codecs));
            }
        }
        // The methods that create an object first, then the others that hand out an object of their own kind; only when
        // none of those makes a new one, those that hand out an interface by id: an object a creation made carries the
        // description the serving glue recorded, which the length rules read.
        makers.sort_by_key(|maker| (maker.place.contains(" for "), !maker.place.contains("::Create")));
        let _ = writeln!(text, "/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new: first by\n/// the methods that hand out an object of their own kind, then by those too that hand out an interface by id.\nfn make_all<A: {top}>(o: &mut Objects<A>) -> usize {{\n    for by_id in [false, true] {{\n    for _ in 0..6 {{\n        let before = o.kinds_held();");
        for maker in &makers {
            let mut wanted: Vec<String> = maker.stores.iter().map(|(k, _)| format!("o.k{k}.is_none()")).collect();
            if maker.place.contains(" for ") {
                wanted = vec![format!("by_id && ({})", wanted.join(" || "))];
            }
            let answer = if maker.fallible { "answer.ok()" } else { "Some(answer)" };
            let _ = writeln!(text, "        // {}\n        if {} {{\n            let answer = {{\n                let o = &*o;\n                made(|| {{\n{}        let answer = {};\n        {answer}\n                }})\n            }};", maker.place, wanted.join(" || "), maker.prelude, maker.call);
            if let [(kind, var)] = maker.stores.as_slice() {
                let _ = writeln!(text, "            if let Some({}) = answer\n                && o.k{kind}.is_none()\n            {{\n                o.k{kind} = Some({var});\n            }}\n        }}", maker.pattern);
            } else {
                let _ = writeln!(text, "            if let Some({}) = answer {{", maker.pattern);
                for (kind, var) in &maker.stores {
                    let _ = writeln!(text, "                if o.k{kind}.is_none() {{\n                    o.k{kind} = Some({var});\n                }}");
                }
                let _ = writeln!(text, "            }}\n        }}");
            }
        }
        let _ = writeln!(text, "        if o.kinds_held() == before {{\n            break;\n        }}\n    }}\n    }}\n    o.kinds_held()\n}}\n");
        let _ = writeln!(text, "/// Calls every method on every object held, and the exports, each answer checked, and notes in `answers` what each\n/// method both glues carry to the implementation answered.\nfn drive<A: {top}>(o: &Objects<A>, answers: &Answers) -> Driven {{\n    let mut driven = Driven::default();");
        for (place, prelude, call) in &calls {
            if call.contains("answers.") {
                // A compared call that was skipped or panicked is noted too, so that both drives line up.
                let _ = writeln!(text, "    // {place}\n    let noted = answers.len();\n    answers.start();\n    driven.attempt(|| {{\n{prelude}        {call}\n    }});\n    answers.not_called_since(noted, \"{place}\");");
            } else {
                let _ = writeln!(text, "    // {place}\n    driven.attempt(|| {{\n{prelude}        {call}\n    }});");
            }
        }
        let _ = writeln!(text, "    driven\n}}\n");
        let _ = writeln!(text, "/// The number of methods and exports the suite calls where it holds the objects and can make the arguments.\nconst CALLS: usize = {};\n", calls.len());
        let noted: usize = calls.iter().map(|(_, _, call)| call.matches("answers.").count()).sum();
        let _ = writeln!(text, "/// The number of answers the suite notes and compares: each answer of a method both glues carry to the\n/// implementation, and the caller's bytes after each such method that writes them.\nconst COMPARED: usize = {noted};\n");
        let _ = writeln!(text, "/// Makes an object of every kind the implementation makes and calls every method of every kind and every export on\n/// them: no method panics, a failure is negative, an object handed out equals its clone.\npub fn every_method_answers<A: {top}>(api: A) {{\n    let mut o = Objects::new(api);\n    let kinds = make_all(&mut o);\n    let driven = drive(&o, &Answers::default());\n    println!(\"SUITE {}: kinds made {{kinds}}, calls {{CALLS}}: called {{}}, skipped {{}}, panicked {{}}\", driven.called, driven.skipped, driven.panicked);\n    assert_eq!(driven.panicked, 0, \"a method panicked\");\n    assert_eq!(driven.called + driven.skipped, CALLS, \"a call was neither made nor skipped\");\n}}\n", spec.folder);
        let _ = writeln!(text, "/// Makes an object of every kind and drives every method as `every_method_answers` does, and returns what each\n/// method both glues carry to the implementation answered, in the order called.\npub fn answers_of<A: {top}>(api: A) -> Vec<String> {{\n    let mut o = Objects::new(api);\n    make_all(&mut o);\n    let answers = Answers::default();\n    drive(&o, &answers);\n    answers.lines()\n}}\n");
        let _ = writeln!(text, "/// Drives `api` and `reference` alike and compares every answer noted, field by field, in the order called: `api` is\n/// `reference` served and called through both glues, so each answer must be what the implementation gave.\npub fn every_answer_equals_the_reference<A: {top}, R: {top}>(api: A, reference: R) {{\n    let got = answers_of(api);\n    let wanted = answers_of(reference);\n    println!(\"ANSWERS {}: the reference answered {{}} of {{COMPARED}}, through both glues {{}}\", wanted.len(), got.len());\n    let differing: Vec<String> = got.iter().zip(&wanted).enumerate().filter(|(_, (got, wanted))| got != wanted).map(|(at, (got, wanted))| format!(\"answer {{at}}: through both glues {{got}}, the implementation {{wanted}}\")).collect();\n    for line in &differing {{\n        println!(\"DIFFERS {{line}}\");\n    }}\n    assert_eq!(differing, Vec::<String>::new(), \"answers differ through both glues\");\n    assert_eq!(got.len(), wanted.len(), \"through both glues, a different number of answers\");\n    assert_eq!(wanted.len(), COMPARED, \"the reference answered a different number of answers than the suite notes\");\n}}\n", spec.folder);
        let _ = writeln!(text, "/// As `answers_of`, with what the implementation received during each compared call after its answer (`logged` and\n/// `since`: the implementation's own log of the arguments it received).\npub fn answers_received_of<A: {top}>(api: A, logged: fn() -> usize, since: fn(usize) -> Vec<String>) -> Vec<String> {{\n    let mut o = Objects::new(api);\n    make_all(&mut o);\n    let answers = Answers::with_received((logged, since));\n    drive(&o, &answers);\n    answers.lines()\n}}\n\n/// As `every_answer_equals_the_reference`, and every argument the implementation received through both glues equals\n/// what it received from the same call made directly: nothing the caller passed is dropped, zeroed or mixed up on the\n/// way. A method whose call through both glues differs by design (a check of the glue's on an argument the suite\n/// passes, an answer the glue gives from its own record, a bit a row clears) must be one of `refused` (src/api.rs\n/// `SUITE_DIFFERENT`, each with the reason). A call the calling glue makes of its own for a method (`extra`: the method and\n/// the line, src/api.rs `SUITE_EXTRA_CALLS`) is taken out of what the implementation received before the comparison.\npub fn every_answer_and_argument_equals_the_reference<A: {top}, R: {top}>(api: A, reference: R, logged: fn() -> usize, since: fn(usize) -> Vec<String>, refused: &[&str], extra: &[(&str, &str)]) {{\n    let wanted = answers_received_of(reference, logged, since);\n    let got = answers_received_of(api, logged, since);\n    let mut differing = Vec::new();\n    let mut accepted = Vec::new();\n    for (at, (got_line, wanted_line)) in got.iter().zip(&wanted).enumerate() {{\n        if got_line == wanted_line {{\n            continue;\n        }}\n        // The method's place: the text before \" -> \", \" wrote \" or \" on \" (a line of a call not made, or of what the\n        // implementation received, names the kind too).\n        let place = got_line.split(\" -> \").next().unwrap_or_default().split(\" wrote \").next().unwrap_or_default().split(\" on \").next().unwrap_or_default().split(\": \").next().unwrap_or_default().to_string();\n        let mut without_extra = got_line.clone();\n        for (_, line) in extra.iter().filter(|(at, _)| *at == place) {{\n            for form in [format!(\", \\\"{{line}}\\\"\"), format!(\"\\\"{{line}}\\\", \"), format!(\"\\\"{{line}}\\\"\")] {{\n                without_extra = without_extra.replace(&form, \"\");\n            }}\n        }}\n        if without_extra == *wanted_line {{\n            continue;\n        }}\n        // Only what the implementation received may differ by design: every answer must be the implementation's.\n        let received = got_line.contains(\": the implementation received \");\n        if received && (refused.contains(&place.as_str()) || refused.iter().any(|entry| got_line.starts_with(entry) && got_line[entry.len()..].starts_with([' ', ':']))) {{\n            accepted.push(place);\n        }} else {{\n            differing.push(format!(\"{{at}}: through both glues {{got_line}}, directly {{wanted_line}}\"));\n        }}\n    }}\n    accepted.dedup();\n    println!(\"ANSWERS AND ARGUMENTS {}: {{}} lines directly, {{}} through both glues; {{}} methods differing as src/api.rs SUITE_DIFFERENT states: {{accepted:?}}\", wanted.len(), got.len(), accepted.len());\n    for line in &differing {{\n        println!(\"DIFFERS {{line}}\");\n    }}\n    assert_eq!(differing, Vec::<String>::new(), \"answers or arguments differ through both glues\");\n    assert_eq!(got.len(), wanted.len(), \"through both glues, a different number of answers\");\n}}\n", spec.folder);
        let _ = writeln!(text, "/// Runs each suite function above as its own test on the value an implementation gives; given a reference, also\n/// compares every answer of the value with the reference's.\n#[macro_export]\nmacro_rules! tests {{\n    ($value:expr) => {{\n        #[test]\n        fn every_method_answers() {{\n            $crate::every_method_answers($value);\n        }}\n    }};\n    ($value:expr, $reference:expr) => {{\n        $crate::tests!($value);\n\n        #[test]\n        fn every_answer_equals_the_reference() {{\n            $crate::every_answer_equals_the_reference($value, $reference);\n        }}\n    }};\n    ($value:expr, $reference:expr, $logged:expr, $since:expr, $refused:expr, $extra:expr) => {{\n        // One test, so that no other drive adds to the implementation's log of arguments while it is read.\n        #[test]\n        fn every_method_answers_and_every_answer_and_argument_equals_the_reference() {{\n            $crate::every_method_answers($value);\n            $crate::every_answer_and_argument_equals_the_reference($value, $reference, $logged, $since, $refused, $extra);\n        }}\n    }};\n}}");
        let mut imports: Vec<String> = Vec::new();
        for other in g.apis.iter() {
            let named = text.lines().any(|line| !line.starts_with("//") && line.contains(&format!("{}::", alias(other))));
            if named || other.folder == "component-object-model" {
                imports.push(format!("use {} as {};", package(other).replace('-', "_"), alias(other)));
            }
        }
        let summary: Vec<String> = tally.borrow().iter().map(|(what, places)| format!("{} {what}", places.len())).collect();
        Some((text.replace("@IMPORTS@", &imports.join("\n")), format!("ANSWERS {}: {}\n", spec.folder, summary.join("; "))))
    }
}

/// Writes API `api`'s conformance crate, and has each of its implementations written in this set run it.
pub(in crate::emit) fn write_suite(writer: &Writer<'_, '_>, api: usize, crate_folder: &Path) -> Result<String, String> {
    let g = writer.g;
    writer.current.set(api);
    let spec = g.apis[api];
    let crates = crate_folder.parent().ok_or("no crates folder")?;
    let mut codecs = codecs(writer, crates, api);
    // The suite names no implementation: structures it cannot default are not made, and their calls skipped.
    codecs.decoders.clear();
    codecs.modules.clear();
    let Some((text, answers)) = writer.suite(api, &codecs) else { return Ok(String::new()) };
    writer.current.set(api);
    let folder = crate_folder.join("conformance");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    std::fs::write(folder.join("suite.rs"), &text).map_err(|error| error.to_string())?;
    let mut manifest = format!("[package]\nname = \"{}-conformance\"\ndescription = \"The conformance suite of {}'s traits.\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"suite.rs\"\n\n[dependencies]\n", package(spec), spec.folder);
    for other in g.apis.iter() {
        if text.contains(&format!("use {} as", package(other).replace('-', "_"))) {
            let path = if other.folder == spec.folder { "..".to_string() } else { format!("../../{}", other.folder) };
            let _ = writeln!(manifest, "{} = {{ path = \"{path}\" }}", package(other));
        }
    }
    let _ = writeln!(manifest, "wcontainers-byte-sink = {{ path = \"../../byte-sink\" }}\nwcontainers-byte-source = {{ path = \"../../byte-source\" }}\nwcontainers-volatile-bytes = {{ path = \"../../volatile-bytes\" }}\n\n[lints]\nworkspace = true");
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    // Each implementation of this set runs the suite once.
    let suite_crate = format!("{}_conformance", package(spec).replace('-', "_"));
    let mut runs = Vec::new();
    for (implementation, value) in [("hostile", format!("{}_hostile::HostileLibrary", package(spec).replace('-', "_"))), ("recorder", format!("{}_recorder::Recorder", package(spec).replace('-', "_")))] {
        let place = crate_folder.join("implementation").join(implementation);
        let manifest_path = place.join("Cargo.toml");
        let Ok(mut manifest) = std::fs::read_to_string(&manifest_path) else { continue };
        if !manifest.contains("[lib]") {
            continue;
        }
        std::fs::write(place.join("conformance_run.rs"), format!("//! {}\n//!\n//! The conformance suite of {}, run on this implementation.\n\n{suite_crate}::tests!({value});\n", crate::emit::HEADER, spec.folder)).map_err(|error| error.to_string())?;
        let mut test = "\n[[test]]\nname = \"conformance_run\"\npath = \"conformance_run.rs\"\n".to_string();
        // The same suite through both glues: the calling glue over the serving glue's own exports of this implementation,
        // as if it were the system's library (every method's C form crossed twice).
        let direct_crate = format!("{}_direct", package(spec).replace('-', "_"));
        let serve_text = std::fs::read_to_string(crate_folder.join("implementation").join("direct").join("serve.rs")).unwrap_or_default();
        if serve_text.contains("pub fn served") {
            // The recorder's every answer through both glues is compared with the recorder's own (its answers are known
            // values, never a type's default); the hostile implementation's answers are its lies, so only checked.
            let run = if implementation == "recorder" {
                let recorder_crate = format!("{}_recorder", package(spec).replace('-', "_"));
                format!("//! The conformance suite of {}, run on this implementation through both glues (`served`), every answer and every\n//! argument the implementation received compared with the same call made on the implementation directly.\n\n{suite_crate}::tests!(\n    {direct_crate}::served({value}),\n    {value},\n    {recorder_crate}::arguments_logged,\n    {recorder_crate}::arguments_since,\n    &[{refused}],\n    &[{extra}]\n);\n", spec.folder, refused = crate::api::SUITE_DIFFERENT.iter().map(|(place, _)| format!("\"{place}\"")).collect::<Vec<_>>().join(", "), extra = crate::api::SUITE_EXTRA_CALLS.iter().map(|(place, line, _)| format!("(\"{place}\", \"{line}\")")).collect::<Vec<_>>().join(", "))
            } else {
                format!("//! The conformance suite of {}, run on this implementation through both glues (`served`).\n\n{suite_crate}::tests!({direct_crate}::served({value}));\n", spec.folder)
            };
            std::fs::write(place.join("conformance_through_glues.rs"), format!("//! {}\n//!\n{run}", crate::emit::HEADER)).map_err(|error| error.to_string())?;
            test.push_str("\n[[test]]\nname = \"conformance_through_glues\"\npath = \"conformance_through_glues.rs\"\n");
            if implementation == "recorder" && serve_text.contains("pub fn refusals_reached") {
                // Its own process: the count is the whole program's.
                let refusals = format!("//! {}\n//!\n//! Every method the serving glue of {} refuses by its form is refused by the calling glue first, on its own: the\n//! conformance suite's whole drive through both glues reaches no refusal entry of the serving glue.\n\n#[test]\nfn no_call_reaches_a_refusal_of_the_serving_glue() {{\n    let answers = {suite_crate}::answers_of({direct_crate}::served({value}));\n    assert!(!answers.is_empty());\n    assert_eq!({direct_crate}::refusals_reached(), 0);\n}}\n", crate::emit::HEADER, spec.folder);
                std::fs::write(place.join("refusals_through_glues.rs"), refusals).map_err(|error| error.to_string())?;
                test.push_str("\n[[test]]\nname = \"refusals_through_glues\"\npath = \"refusals_through_glues.rs\"\n");
            }
            if !manifest.contains(&format!("{}-direct = ", package(spec))) {
                runs.push("(no direct dependency)");
            }
            runs.push(match implementation {
                "hostile" => "hostile through both glues",
                _ => "recorder through both glues",
            });
        }
        let test = test.as_str();
        let at = manifest.find("\n[dependencies]").unwrap_or(manifest.len());
        manifest.insert_str(at, test);
        let dev = format!("{}-conformance = {{ path = \"../../conformance\" }}\n", package(spec));
        if let Some(at) = manifest.find("[dev-dependencies]\n") {
            manifest.insert_str(at + "[dev-dependencies]\n".len(), &dev);
        } else {
            let at = manifest.find("\n[lints]").unwrap_or(manifest.len());
            manifest.insert_str(at, &format!("\n[dev-dependencies]\n{dev}"));
        }
        std::fs::write(&manifest_path, manifest).map_err(|error| error.to_string())?;
        runs.push(implementation);
    }
    Ok(format!("SUITE {}: written; run by {}\n{answers}", spec.folder, if runs.is_empty() { "none".to_string() } else { runs.join(", ") }))
}

/// A Rust expression of a `String` that shows every value in an answer of type `ty` held by reference at `expr`: plain
/// data by `Debug`, every field; an object (or a structure holding one) as `an object`, its identity being no value; a
/// result, option, list or tuple by its parts. With what the answer holds, for the report.
fn answer_summary(ty: &str, expr: &str, depth: usize) -> (String, &'static str) {
    let ty = ty.trim();
    if !ty.contains("Self") {
        let what = if ty.starts_with("Result<()") { "a status only: compared" } else { "plain data: compared, every field" };
        return (format!("format!(\"{{:?}}\", {expr})"), what);
    }
    let v = format!("v{depth}");
    let holding = "holds objects: compared, every value but an object's identity";
    if let Some(inner) = ty.strip_prefix("Result<").and_then(|rest| rest.strip_suffix('>')) {
        let parts = super::record::split_types(inner);
        let (ok, what) = answer_summary(parts.first().map_or("", String::as_str), &v, depth + 1);
        let what = if what.starts_with("an object only") { "an object, or a failure: compared, its success and the failure's code" } else { what };
        return (format!("match {expr} {{ Ok({v}) => format!(\"Ok({{}})\", {ok}), Err({v}) => format!(\"Err({{:#x}})\", {v}.0) }}"), what);
    }
    if let Some(inner) = ty.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')) {
        let (some, what) = answer_summary(inner, &v, depth + 1);
        let what = if what.starts_with("an object only") { "an object or none: compared, whether there is one" } else { what };
        return (format!("match {expr} {{ Some({v}) => format!(\"Some({{}})\", {some}), None => \"None\".to_string() }}"), what);
    }
    if let Some(inner) = ty.strip_prefix("Vec<").and_then(|rest| rest.strip_suffix('>')) {
        let (each, what) = answer_summary(inner, &v, depth + 1);
        let what = if what.starts_with("an object") { "a list of objects: compared, its length and which are none" } else { holding };
        return (format!("format!(\"[{{}}]\", {expr}.iter().map(|{v}| {each}).collect::<Vec<String>>().join(\", \"))"), what);
    }
    if let Some(inner) = ty.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        let parts = super::record::split_types(inner);
        let names: Vec<String> = (0..parts.len()).map(|at| format!("t{depth}_{at}")).collect();
        let shown: Vec<String> = parts.iter().zip(&names).map(|(part, name)| answer_summary(part, name, depth + 1).0).collect();
        let holes = vec!["{}"; parts.len()].join(", ");
        return (format!("{{ let ({}) = {expr}; format!(\"({holes})\", {}) }}", names.join(", "), shown.join(", ")), holding);
    }
    ("\"an object\".to_string()".to_string(), "an object only: its identity is no value to compare")
}

/// Reads the set's structures and enumerations from the interface crates' text (src/glue_hostile.rs `Shapes`).
fn shapes_of(g: &crate::emit::Generator<'_>, crates: &Path, newtypes: &BTreeSet<String>) -> Shapes {
    let specials: BTreeSet<&str> = g.apis.iter().flat_map(|api| api.specials.iter()).map(|special| special.value).collect();
    let mut shapes = Shapes { newtypes: newtypes.clone(), ..Shapes::default() };
    for spec in g.apis.iter() {
        let own = alias(spec);
        for (file, module) in [("structures.rs", "structures"), ("enumerations.rs", "enumerations")] {
            let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join(file)) else { continue };
            for rest in text.lines().filter_map(|line| line.strip_prefix("pub struct ")) {
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if let Some(inner) = rest[name.len()..].strip_prefix("(pub ").and_then(|inner| inner.strip_suffix(");")) {
                    shapes.inner.insert(format!("{own}::{module}::{name}"), inner.to_string());
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join("constants.rs")) {
            for rest in text.lines().filter_map(|line| line.strip_prefix("pub const ")) {
                if let Some((name, value)) = rest.split_once(": usize = ")
                    && let Ok(length) = value.trim_end_matches(';').replace('_', "").parse()
                {
                    shapes.lengths.insert(format!("{own}::constants::{name}"), length);
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join("structures.rs")) {
            let mut union = false;
            let mut current: Option<(String, Vec<(String, String)>, bool)> = None;
            for line in text.lines() {
                if line.starts_with("/// A C union of") {
                    union = true;
                } else if let Some(rest) = line.strip_prefix("pub struct ") {
                    let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                    if rest[name.len()..].starts_with(" {") {
                        current = Some((format!("{own}::structures::{name}"), Vec::new(), union));
                    }
                    union = false;
                } else if let Some((_, fields, _)) = current.as_mut()
                    && let Some(rest) = line.strip_prefix("    pub ")
                    && let Some((name, ty)) = rest.split_once(": ")
                {
                    fields.push((name.to_string(), ty.trim_end_matches(',').replace("crate::", &format!("{own}::"))));
                } else if line == "}"
                    && let Some((path, fields, is_union)) = current.take()
                {
                    shapes.structs.insert(path, (fields, is_union));
                } else if line.starts_with("#[") || line.starts_with("///") {
                } else {
                    union = false;
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(crates.join(spec.folder).join("enumerations.rs")) {
            for rest in text.lines().filter_map(|line| line.strip_prefix("pub const ")) {
                let Some((name, rest)) = rest.split_once(": ") else { continue };
                let Some((ty, value)) = rest.split_once(" = ") else { continue };
                let inner = value.trim_end_matches(';').trim_start_matches(ty).trim_start_matches('(').trim_end_matches(')').replace('_', "");
                let parsed = match inner.strip_prefix("0x") {
                    Some(hex) => i64::from_str_radix(hex, 16),
                    None => inner.parse::<i64>(),
                };
                let Ok(number) = parsed else { continue };
                let path = format!("{own}::enumerations::{ty}");
                let entry = shapes.values.entry(path).or_default();
                if !specials.contains(name) {
                    entry.push((format!("{own}::enumerations::{name}"), number));
                }
            }
        }
    }
    shapes
}
