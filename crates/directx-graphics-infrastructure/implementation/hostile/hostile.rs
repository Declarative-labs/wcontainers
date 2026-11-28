//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The hostile implementation (src/glue_hostile.rs) of directx-graphics-infrastructure's traits, for tests only: every call is logged by its place, with the values
//! passed by value; an object asked for is made; a structure, a bound object or a list asked for is not (E_NOTIMPL).

#![allow(
    unused_variables,
    unused_imports,
    dead_code,
    unreachable_code,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::new_without_default,
    clippy::redundant_closure_call,
    clippy::too_many_lines,
    clippy::unnecessary_lazy_evaluations,
    clippy::option_map_unit_fn,
    reason = "generated: every method and call has the same shape"
)]

#[path = "../../../direct3d-11/implementation/direct/data.rs"]
mod data_direct3d_11;
#[path = "../../../direct3d-12/implementation/direct/data.rs"]
mod data_direct3d_12;
#[path = "../direct/data.rs"]
mod data_directx_graphics_infrastructure;

use wcontainers_component_object_model as component_object_model;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;
use wcontainers_directx_graphics_infrastructure_direct as directx_graphics_infrastructure_direct;

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
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
    COUNTS
        .iter()
        .for_each(|count| count.store(0, Ordering::SeqCst));
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
    let reentry = REENTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
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
    let Some(lie) = *PLAN.lock().unwrap_or_else(PoisonError::into_inner) else {
        return;
    };
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
    REACHED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(place)
        .copied()
        .unwrap_or(0)
}

/// The last call of each place that reached the implementation, with the values passed by value.
static LAST: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Returns the last call of `place` that reached the implementation, with the values it passed by value (empty: none).
pub fn last_call(place: &str) -> String {
    LAST.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(place)
        .cloned()
        .unwrap_or_default()
}

/// Calls that reached the implementation whose arguments were not those a row answers with, by place.
static REACHED_OTHERWISE: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// Returns how many calls of `place` reached the implementation with arguments other than `=value`.
pub fn reached_otherwise(place: &str) -> u64 {
    REACHED_OTHERWISE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(place)
        .copied()
        .unwrap_or(0)
}

/// A call of a method of the API's traits.
fn record(call: String) {
    let place = call.split('(').next().unwrap_or("").to_string();
    if let Some((_, value)) = ANSWERED.iter().find(|(answered, _)| *answered == place)
        && !call.contains(&format!("={value}"))
    {
        *REACHED_OTHERWISE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(place.clone())
            .or_default() += 1;
    }
    *REACHED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(place.clone())
        .or_default() += 1;
    LAST.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(place, call.clone());
    lie(Site::Method, &call);
}

/// Uses every byte lent: writes as many as the length says, reads them back, and both copies through.
fn touch(bytes: &impl wcontainers_volatile_bytes::VolatileBytes) {
    let length = usize::try_from(bytes.len())
        .unwrap_or(usize::MAX)
        .min(1 << 24);
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
    match usize::try_from(turn % lies)
        .ok()
        .and_then(|at| at.checked_sub(1))
        .and_then(|at| LYING_LENGTHS.get(at))
    {
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
fn flood_sink<Sink: wcontainers_byte_sink::ByteSink>(
    sink: &mut Sink,
    most: usize,
) -> Result<usize, Sink::Error> {
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
fn drain_source<Source: wcontainers_byte_source::ByteSource>(
    source: &mut Source,
    most: usize,
) -> Result<usize, Source::Error> {
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
        LyingBytes {
            lengths: lengths.to_vec(),
            way: None,
            asked: AtomicU64::new(0),
        }
    }

    /// Bytes of a steady length that lie `way` at the call `at` of any of their methods.
    pub fn lying(way: Way, at: u64) -> LyingBytes {
        LyingBytes {
            lengths: vec![64],
            way: Some((way, at)),
            asked: AtomicU64::new(0),
        }
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

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let _ = offset;
        let n = self.call();
        into.fill(u8::try_from(n % 251).unwrap_or(0));
        into.len().saturating_add(5)
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let _ = offset;
        let _ = self.call();
        bytes.len().saturating_mul(3).saturating_add(1)
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        let n = self.call();
        let most = usize::try_from(self.length(n))
            .unwrap_or(usize::MAX)
            .min(1 << 16)
            .saturating_add(4096);
        flood_sink(sink, most)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        let n = self.call();
        let most = usize::try_from(self.length(n))
            .unwrap_or(usize::MAX)
            .min(1 << 16)
            .saturating_add(4096);
        drain_source(source, most)
    }
}

/// A recorded object of the kind `Adapter`.
pub struct HostileAdapter<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileAdapter<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileAdapter<A> {
        HostileAdapter {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileAdapter<A> {
    fn clone(&self) -> HostileAdapter<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileAdapter<A> {
    fn eq(&self, other: &HostileAdapter<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileAdapter<A> {}

impl<A> Hash for HostileAdapter<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileAdapter<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileAdapter")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Debug`.
pub struct HostileDebug<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDebug<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDebug<A> {
        HostileDebug {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDebug<A> {
    fn clone(&self) -> HostileDebug<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDebug<A> {
    fn eq(&self, other: &HostileDebug<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDebug<A> {}

impl<A> Hash for HostileDebug<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDebug<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDebug")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DecodeSwapChain`.
pub struct HostileDecodeSwapChain<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDecodeSwapChain<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDecodeSwapChain<A> {
        HostileDecodeSwapChain {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDecodeSwapChain<A> {
    fn clone(&self) -> HostileDecodeSwapChain<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDecodeSwapChain<A> {
    fn eq(&self, other: &HostileDecodeSwapChain<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDecodeSwapChain<A> {}

impl<A> Hash for HostileDecodeSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDecodeSwapChain<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDecodeSwapChain")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Device`.
pub struct HostileDevice<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDevice<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDevice<A> {
        HostileDevice {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDevice<A> {
    fn clone(&self) -> HostileDevice<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDevice<A> {
    fn eq(&self, other: &HostileDevice<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDevice<A> {}

impl<A> Hash for HostileDevice<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDevice<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDevice")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DisplayControl`.
pub struct HostileDisplayControl<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDisplayControl<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDisplayControl<A> {
        HostileDisplayControl {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDisplayControl<A> {
    fn clone(&self) -> HostileDisplayControl<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDisplayControl<A> {
    fn eq(&self, other: &HostileDisplayControl<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDisplayControl<A> {}

impl<A> Hash for HostileDisplayControl<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDisplayControl<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDisplayControl")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Factory`.
pub struct HostileFactory<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileFactory<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileFactory<A> {
        HostileFactory {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileFactory<A> {
    fn clone(&self) -> HostileFactory<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileFactory<A> {
    fn eq(&self, other: &HostileFactory<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileFactory<A> {}

impl<A> Hash for HostileFactory<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileFactory<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileFactory")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `FactoryMedia`.
pub struct HostileFactoryMedia<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileFactoryMedia<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileFactoryMedia<A> {
        HostileFactoryMedia {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileFactoryMedia<A> {
    fn clone(&self) -> HostileFactoryMedia<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileFactoryMedia<A> {
    fn eq(&self, other: &HostileFactoryMedia<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileFactoryMedia<A> {}

impl<A> Hash for HostileFactoryMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileFactoryMedia<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileFactoryMedia")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `InfoQueue`.
pub struct HostileInfoQueue<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileInfoQueue<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileInfoQueue<A> {
        HostileInfoQueue {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileInfoQueue<A> {
    fn clone(&self) -> HostileInfoQueue<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileInfoQueue<A> {
    fn eq(&self, other: &HostileInfoQueue<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileInfoQueue<A> {}

impl<A> Hash for HostileInfoQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileInfoQueue<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileInfoQueue")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `KeyedMutex`.
pub struct HostileKeyedMutex<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileKeyedMutex<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileKeyedMutex<A> {
        HostileKeyedMutex {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileKeyedMutex<A> {
    fn clone(&self) -> HostileKeyedMutex<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileKeyedMutex<A> {
    fn eq(&self, other: &HostileKeyedMutex<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileKeyedMutex<A> {}

impl<A> Hash for HostileKeyedMutex<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileKeyedMutex<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileKeyedMutex")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Output`.
pub struct HostileOutput<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileOutput<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileOutput<A> {
        HostileOutput {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileOutput<A> {
    fn clone(&self) -> HostileOutput<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileOutput<A> {
    fn eq(&self, other: &HostileOutput<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileOutput<A> {}

impl<A> Hash for HostileOutput<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileOutput<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileOutput")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `OutputDuplication`.
pub struct HostileOutputDuplication<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileOutputDuplication<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileOutputDuplication<A> {
        HostileOutputDuplication {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileOutputDuplication<A> {
    fn clone(&self) -> HostileOutputDuplication<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileOutputDuplication<A> {
    fn eq(&self, other: &HostileOutputDuplication<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileOutputDuplication<A> {}

impl<A> Hash for HostileOutputDuplication<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileOutputDuplication<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileOutputDuplication")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Resource`.
pub struct HostileResource<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileResource<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileResource<A> {
        HostileResource {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileResource<A> {
    fn clone(&self) -> HostileResource<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileResource<A> {
    fn eq(&self, other: &HostileResource<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileResource<A> {}

impl<A> Hash for HostileResource<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileResource<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileResource")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `Surface`.
pub struct HostileSurface<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileSurface<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileSurface<A> {
        HostileSurface {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileSurface<A> {
    fn clone(&self) -> HostileSurface<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileSurface<A> {
    fn eq(&self, other: &HostileSurface<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileSurface<A> {}

impl<A> Hash for HostileSurface<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileSurface<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileSurface")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SwapChain`.
pub struct HostileSwapChain<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileSwapChain<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileSwapChain<A> {
        HostileSwapChain {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileSwapChain<A> {
    fn clone(&self) -> HostileSwapChain<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileSwapChain<A> {
    fn eq(&self, other: &HostileSwapChain<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileSwapChain<A> {}

impl<A> Hash for HostileSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileSwapChain<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileSwapChain")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `SwapChainMedia`.
pub struct HostileSwapChainMedia<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileSwapChainMedia<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileSwapChainMedia<A> {
        HostileSwapChainMedia {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileSwapChainMedia<A> {
    fn clone(&self) -> HostileSwapChainMedia<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileSwapChainMedia<A> {
    fn eq(&self, other: &HostileSwapChainMedia<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileSwapChainMedia<A> {}

impl<A> Hash for HostileSwapChainMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileSwapChainMedia<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileSwapChainMedia")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `GraphicsAnalysis`.
pub struct HostileGraphicsAnalysis<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileGraphicsAnalysis<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileGraphicsAnalysis<A> {
        HostileGraphicsAnalysis {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileGraphicsAnalysis<A> {
    fn clone(&self) -> HostileGraphicsAnalysis<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileGraphicsAnalysis<A> {
    fn eq(&self, other: &HostileGraphicsAnalysis<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileGraphicsAnalysis<A> {}

impl<A> Hash for HostileGraphicsAnalysis<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileGraphicsAnalysis<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileGraphicsAnalysis")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this hostile implementation's.
pub trait Hostile:
    directx_graphics_infrastructure::DirectXGraphicsInfrastructure<
        Adapter = HostileAdapter<Self>,
        Debug = HostileDebug<Self>,
        DecodeSwapChain = HostileDecodeSwapChain<Self>,
        Device = HostileDevice<Self>,
        DisplayControl = HostileDisplayControl<Self>,
        Factory = HostileFactory<Self>,
        FactoryMedia = HostileFactoryMedia<Self>,
        InfoQueue = HostileInfoQueue<Self>,
        KeyedMutex = HostileKeyedMutex<Self>,
        Output = HostileOutput<Self>,
        OutputDuplication = HostileOutputDuplication<Self>,
        Resource = HostileResource<Self>,
        Surface = HostileSurface<Self>,
        SwapChain = HostileSwapChain<Self>,
        SwapChainMedia = HostileSwapChainMedia<Self>,
        GraphicsAnalysis = HostileGraphicsAnalysis<Self>,
    >
{
}

impl<A: Hostile> directx_graphics_infrastructure::Adapter4 for HostileAdapter<A> {
    fn get_desc3(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc3,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter4::GetDesc3()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Adapter3 for HostileAdapter<A> {
    fn register_hardware_content_protection_teardown_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_hardware_content_protection_teardown_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn query_video_memory_info(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
    ) -> Result<
        directx_graphics_infrastructure::structures::QueryVideoMemoryInfo,
        component_object_model::structures::HResult,
    > {
        let _ = &node_index;
        let _ = &memory_segment_group;
        record(format!(
            "IDXGIAdapter3::QueryVideoMemoryInfo(node_index={:?}, memory_segment_group={:?})",
            node_index, memory_segment_group
        ));
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_video_memory_reservation(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
        reservation: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &node_index;
        let _ = &memory_segment_group;
        let _ = &reservation;
        record(format!(
            "IDXGIAdapter3::SetVideoMemoryReservation(node_index={:?}, memory_segment_group={:?}, reservation={:?})",
            node_index, memory_segment_group, reservation
        ));
        Ok(())
    }
    fn register_video_memory_budget_change_notification_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_video_memory_budget_change_notification(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification(dw_cookie={:?})",
            dw_cookie
        ));
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Adapter2 for HostileAdapter<A> {
    fn get_desc2(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc2,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter2::GetDesc2()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Adapter1 for HostileAdapter<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter1::GetDesc1()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Adapter for HostileAdapter<A> {
    fn enum_outputs(
        &self,
        output: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        let _ = &output;
        record(format!("IDXGIAdapter::EnumOutputs(output={:?})", output));
        Ok(HostileOutput::new())
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGIAdapter::GetDesc()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn check_interface_support(
        &self,
        interface_name: &component_object_model::structures::Guid,
    ) -> Result<i64, component_object_model::structures::HResult> {
        let _ = &interface_name;
        record("IDXGIAdapter::CheckInterfaceSupport()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileAdapter<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Debug1 for HostileDebug<A> {
    fn enable_leak_tracking_for_thread(&self) {
        record("IDXGIDebug1::EnableLeakTrackingForThread()".to_string());
    }
    fn disable_leak_tracking_for_thread(&self) {
        record("IDXGIDebug1::DisableLeakTrackingForThread()".to_string());
    }
    fn is_leak_tracking_enabled_for_thread(&self) -> bool {
        record("IDXGIDebug1::IsLeakTrackingEnabledForThread()".to_string());
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Debug for HostileDebug<A> {
    fn report_live_objects(
        &self,
        apiid: component_object_model::structures::Guid,
        flags: directx_graphics_infrastructure::enumerations::DebugRloFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &apiid;
        let _ = &flags;
        record(format!(
            "IDXGIDebug::ReportLiveObjects(apiid={:?}, flags={:?})",
            apiid, flags
        ));
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DecodeSwapChain for HostileDecodeSwapChain<A> {
    fn present_buffer(
        &self,
        buffer_to_present: u32,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &buffer_to_present;
        let _ = &sync_interval;
        let _ = &flags;
        record(format!(
            "IDXGIDecodeSwapChain::PresentBuffer(buffer_to_present={:?}, sync_interval={:?}, flags={:?})",
            buffer_to_present, sync_interval, flags
        ));
        Ok(())
    }
    fn set_source_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rect;
        record("IDXGIDecodeSwapChain::SetSourceRect()".to_string());
        Ok(())
    }
    fn set_target_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rect;
        record("IDXGIDecodeSwapChain::SetTargetRect()".to_string());
        Ok(())
    }
    fn set_dest_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        record(format!(
            "IDXGIDecodeSwapChain::SetDestSize(width={:?}, height={:?})",
            width, height
        ));
        Ok(())
    }
    fn get_source_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        record("IDXGIDecodeSwapChain::GetSourceRect()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_target_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        record("IDXGIDecodeSwapChain::GetTargetRect()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_dest_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        record("IDXGIDecodeSwapChain::GetDestSize()".to_string());
        Ok((Default::default(), Default::default()))
    }
    fn set_color_space(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGIDecodeSwapChain::SetColorSpace(color_space={:?})",
            color_space
        ));
        Ok(())
    }
    fn get_color_space(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags {
        record("IDXGIDecodeSwapChain::GetColorSpace()".to_string());
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Device4 for HostileDevice<A> {
    fn offer_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
        flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resources;
        let _ = &priority;
        let _ = &flags;
        record(format!(
            "IDXGIDevice4::OfferResources1(priority={:?}, flags={:?})",
            priority, flags
        ));
        Ok(())
    }
    fn reclaim_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::ReclaimResourceResults>,
        component_object_model::structures::HResult,
    > {
        let _ = &resources;
        record("IDXGIDevice4::ReclaimResources1()".to_string());
        Ok((0..2_u32).map(|_| Default::default()).collect())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Device3 for HostileDevice<A> {
    fn trim(&self) {
        record("IDXGIDevice3::Trim()".to_string());
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Device2 for HostileDevice<A> {
    fn offer_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &resources;
        let _ = &priority;
        record(format!(
            "IDXGIDevice2::OfferResources(priority={:?})",
            priority
        ));
        Ok(())
    }
    fn reclaim_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<bool>, component_object_model::structures::HResult> {
        let _ = &resources;
        record("IDXGIDevice2::ReclaimResources()".to_string());
        Ok((0..2_u32).map(|_| Default::default()).collect())
    }
    fn enqueue_set_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIDevice2::EnqueueSetEvent(h_event={:?})",
            h_event
        ));
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Device1 for HostileDevice<A> {
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &max_latency;
        record(format!(
            "IDXGIDevice1::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGIDevice1::GetMaximumFrameLatency()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Device for HostileDevice<A> {
    fn get_adapter(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        record("IDXGIDevice::GetAdapter()".to_string());
        Ok(HostileAdapter::new())
    }
    fn create_surface(&self, desc: &directx_graphics_infrastructure::structures::SurfaceDesc, num_surfaces: u32, usage: directx_graphics_infrastructure::enumerations::Usage, shared_resource: Option<&directx_graphics_infrastructure::structures::SharedResource>) -> Result<Vec<Option<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>>, component_object_model::structures::HResult>{
        let _ = &desc;
        let _ = &num_surfaces;
        let _ = &usage;
        let _ = &shared_resource;
        record(format!(
            "IDXGIDevice::CreateSurface(num_surfaces={:?}, usage={:?})",
            num_surfaces, usage
        ));
        Ok((0..num_surfaces)
            .map(|_| Some(HostileSurface::new()))
            .collect())
    }
    fn query_resource_residency(
        &self,
        resources: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::Residency>,
        component_object_model::structures::HResult,
    > {
        let _ = &resources;
        record("IDXGIDevice::QueryResourceResidency()".to_string());
        Err(E_NOTIMPL)
    }
    fn set_gpu_thread_priority(
        &self,
        priority: i32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &priority;
        record(format!(
            "IDXGIDevice::SetGPUThreadPriority(priority={:?})",
            priority
        ));
        Ok(())
    }
    fn get_gpu_thread_priority(&self) -> Result<i32, component_object_model::structures::HResult> {
        record("IDXGIDevice::GetGPUThreadPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileDevice<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DisplayControl for HostileDisplayControl<A> {
    fn is_stereo_enabled(&self) -> bool {
        record("IDXGIDisplayControl::IsStereoEnabled()".to_string());
        Default::default()
    }
    fn set_stereo_enabled(&self, enabled: bool) {
        let _ = &enabled;
        record(format!(
            "IDXGIDisplayControl::SetStereoEnabled(enabled={:?})",
            enabled
        ));
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory7 for HostileFactory<A> {
    fn register_adapters_changed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory7::RegisterAdaptersChangedEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_adapters_changed_event(
        &self,
        dw_cookie: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory7::UnregisterAdaptersChangedEvent(dw_cookie={:?})",
            dw_cookie
        ));
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory6 for HostileFactory<A> {
    fn enum_adapter_by_gpu_preference(
        &self,
        adapter: u32,
        gpu_preference: directx_graphics_infrastructure::enumerations::GpuPreference,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        let _ = &gpu_preference;
        let _ = &riid;
        record(format!(
            "IDXGIFactory6::EnumAdapterByGpuPreference(adapter={:?}, gpu_preference={:?})",
            adapter, gpu_preference
        ));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory5 for HostileFactory<A> {
    fn check_feature_support(
        &self,
        feature: directx_graphics_infrastructure::enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &feature;
        let _ = &feature_support_data;
        record(format!(
            "IDXGIFactory5::CheckFeatureSupport(feature={:?})",
            feature
        ));
        touch(feature_support_data);
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory4 for HostileFactory<A> {
    fn enum_adapter_by_luid(
        &self,
        adapter_luid: component_object_model::structures::Luid,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter_luid;
        let _ = &riid;
        record("IDXGIFactory4::EnumAdapterByLuid()".to_string());
        Err(E_NOTIMPL)
    }
    fn enum_warp_adapter(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIFactory4::EnumWarpAdapter()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory3 for HostileFactory<A> {
    fn get_creation_flags(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::CreateFactoryFlags {
        record("IDXGIFactory3::GetCreationFlags()".to_string());
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory2 for HostileFactory<A> {
    fn is_windowed_stereo_enabled(&self) -> bool {
        record("IDXGIFactory2::IsWindowedStereoEnabled()".to_string());
        Default::default()
    }
    fn create_swap_chain_for_hwnd(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_wnd: component_object_model::structures::Hwnd,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        fullscreen_desc: Option<
            &directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        >,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &h_wnd;
        let _ = &desc;
        let _ = &fullscreen_desc;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactory2::CreateSwapChainForHwnd(h_wnd={:?})",
            h_wnd
        ));
        Ok(HostileSwapChain::new())
    }
    fn create_swap_chain_for_core_window(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        window: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &window;
        let _ = &desc;
        let _ = &restrict_to_output;
        record("IDXGIFactory2::CreateSwapChainForCoreWindow()".to_string());
        Ok(HostileSwapChain::new())
    }
    fn get_shared_resource_adapter_luid(
        &self,
        h_resource: component_object_model::structures::Handle,
    ) -> Result<component_object_model::structures::Luid, component_object_model::structures::HResult>
    {
        let _ = &h_resource;
        record(format!(
            "IDXGIFactory2::GetSharedResourceAdapterLuid(h_resource={:?})",
            h_resource
        ));
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn register_stereo_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &w_msg;
        record(format!(
            "IDXGIFactory2::RegisterStereoStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        Ok(Default::default())
    }
    fn register_stereo_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory2::RegisterStereoStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_stereo_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory2::UnregisterStereoStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn register_occlusion_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &w_msg;
        record(format!(
            "IDXGIFactory2::RegisterOcclusionStatusWindow(window_handle={:?}, w_msg={:?})",
            window_handle, w_msg
        ));
        Ok(Default::default())
    }
    fn register_occlusion_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &h_event;
        record(format!(
            "IDXGIFactory2::RegisterOcclusionStatusEvent(h_event={:?})",
            h_event
        ));
        Ok(Default::default())
    }
    fn unregister_occlusion_status(&self, dw_cookie: u32) {
        let _ = &dw_cookie;
        record(format!(
            "IDXGIFactory2::UnregisterOcclusionStatus(dw_cookie={:?})",
            dw_cookie
        ));
    }
    fn create_swap_chain_for_composition(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &desc;
        let _ = &restrict_to_output;
        record("IDXGIFactory2::CreateSwapChainForComposition()".to_string());
        Ok(HostileSwapChain::new())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory1 for HostileFactory<A> {
    fn enum_adapters1(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        record(format!(
            "IDXGIFactory1::EnumAdapters1(adapter={:?})",
            adapter
        ));
        Ok(HostileAdapter::new())
    }
    fn is_current(&self) -> bool {
        record("IDXGIFactory1::IsCurrent()".to_string());
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Factory for HostileFactory<A> {
    fn enum_adapters(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &adapter;
        record(format!("IDXGIFactory::EnumAdapters(adapter={:?})", adapter));
        Ok(HostileAdapter::new())
    }
    fn make_window_association(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        flags: directx_graphics_infrastructure::enumerations::MwaFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &window_handle;
        let _ = &flags;
        record(format!(
            "IDXGIFactory::MakeWindowAssociation(window_handle={:?}, flags={:?})",
            window_handle, flags
        ));
        Ok(())
    }
    fn get_window_association(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        record("IDXGIFactory::GetWindowAssociation()".to_string());
        Ok(Default::default())
    }
    fn create_swap_chain(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &desc;
        record("IDXGIFactory::CreateSwapChain()".to_string());
        Ok(HostileSwapChain::new())
    }
    fn create_software_adapter(
        &self,
        module: component_object_model::structures::Hmodule,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = &module;
        record(format!(
            "IDXGIFactory::CreateSoftwareAdapter(module={:?})",
            module
        ));
        Ok(HostileAdapter::new())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileFactory<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::FactoryMedia for HostileFactoryMedia<A> {
    type Api = A;

    fn create_swap_chain_for_composition_surface_handle(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_surface: component_object_model::structures::Handle,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let _ = &device;
        let _ = &h_surface;
        let _ = &desc;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle(h_surface={:?})",
            h_surface
        ));
        Ok(HostileSwapChain::new())
    }
    fn create_decode_swap_chain_for_composition_surface_handle(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, h_surface: component_object_model::structures::Handle, desc: &directx_graphics_infrastructure::structures::DecodeSwapChainDesc, yuv_decode_buffers: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource, restrict_to_output: Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain, component_object_model::structures::HResult>{
        let _ = &device;
        let _ = &h_surface;
        let _ = &desc;
        let _ = &yuv_decode_buffers;
        let _ = &restrict_to_output;
        record(format!(
            "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle(h_surface={:?})",
            h_surface
        ));
        Ok(HostileDecodeSwapChain::new())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::InfoQueue for HostileInfoQueue<A> {
    fn set_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
        message_count_limit: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &message_count_limit;
        record(format!(
            "IDXGIInfoQueue::SetMessageCountLimit(producer={:?}, message_count_limit={:?})",
            producer, message_count_limit
        ));
        Ok(())
    }
    fn clear_stored_messages(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearStoredMessages(producer={:?})",
            producer
        ));
    }
    fn get_message(
        &self,
        producer: component_object_model::structures::Guid,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &message_index;
        let _ = &message;
        record("IDXGIInfoQueue::GetMessage()".to_string());
        if let Some(bytes) = message {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_num_stored_messages_allowed_by_retrieval_filters(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn get_num_stored_messages(&self, producer: component_object_model::structures::Guid) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumStoredMessages(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn get_num_messages_discarded_by_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn get_message_count_limit(&self, producer: component_object_model::structures::Guid) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetMessageCountLimit(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn get_num_messages_allowed_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn get_num_messages_denied_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn add_storage_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::AddStorageFilterEntries()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::GetStorageFilter()".to_string());
        if let Some(bytes) = filter {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn clear_storage_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearStorageFilter(producer={:?})",
            producer
        ));
    }
    fn push_empty_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushEmptyStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_deny_all_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushDenyAllStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_copy_of_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushCopyOfStorageFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::PushStorageFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn pop_storage_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PopStorageFilter(producer={:?})",
            producer
        ));
    }
    fn get_storage_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetStorageFilterStackSize(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn add_retrieval_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::AddRetrievalFilterEntries()".to_string());
        Err(E_NOTIMPL)
    }
    fn get_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::GetRetrievalFilter()".to_string());
        if let Some(bytes) = filter {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn clear_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::ClearRetrievalFilter(producer={:?})",
            producer
        ));
    }
    fn push_empty_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushEmptyRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_deny_all_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushDenyAllRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_copy_of_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PushCopyOfRetrievalFilter(producer={:?})",
            producer
        ));
        Ok(())
    }
    fn push_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &filter;
        record("IDXGIInfoQueue::PushRetrievalFilter()".to_string());
        Err(E_NOTIMPL)
    }
    fn pop_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::PopRetrievalFilter(producer={:?})",
            producer
        ));
    }
    fn get_retrieval_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetRetrievalFilterStackSize(producer={:?})",
            producer
        ));
        Default::default()
    }
    fn add_message(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        id: i32,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &category;
        let _ = &severity;
        let _ = &id;
        let _ = &description;
        record(format!(
            "IDXGIInfoQueue::AddMessage(producer={:?}, category={:?}, severity={:?}, id={:?})",
            producer, category, severity, id
        ));
        Ok(())
    }
    fn add_application_message(
        &self,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &severity;
        let _ = &description;
        record(format!(
            "IDXGIInfoQueue::AddApplicationMessage(severity={:?})",
            severity
        ));
        Ok(())
    }
    fn set_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &category;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnCategory(producer={:?}, category={:?}, b_enable={:?})",
            producer, category, b_enable
        ));
        Ok(())
    }
    fn set_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &severity;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnSeverity(producer={:?}, severity={:?}, b_enable={:?})",
            producer, severity, b_enable
        ));
        Ok(())
    }
    fn set_break_on_id(
        &self,
        producer: component_object_model::structures::Guid,
        id: i32,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &producer;
        let _ = &id;
        let _ = &b_enable;
        record(format!(
            "IDXGIInfoQueue::SetBreakOnID(producer={:?}, id={:?}, b_enable={:?})",
            producer, id, b_enable
        ));
        Ok(())
    }
    fn get_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
    ) -> bool {
        let _ = &producer;
        let _ = &category;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnCategory(producer={:?}, category={:?})",
            producer, category
        ));
        Default::default()
    }
    fn get_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
    ) -> bool {
        let _ = &producer;
        let _ = &severity;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnSeverity(producer={:?}, severity={:?})",
            producer, severity
        ));
        Default::default()
    }
    fn get_break_on_id(&self, producer: component_object_model::structures::Guid, id: i32) -> bool {
        let _ = &producer;
        let _ = &id;
        record(format!(
            "IDXGIInfoQueue::GetBreakOnID(producer={:?}, id={:?})",
            producer, id
        ));
        Default::default()
    }
    fn set_mute_debug_output(
        &self,
        producer: component_object_model::structures::Guid,
        b_mute: bool,
    ) {
        let _ = &producer;
        let _ = &b_mute;
        record(format!(
            "IDXGIInfoQueue::SetMuteDebugOutput(producer={:?}, b_mute={:?})",
            producer, b_mute
        ));
    }
    fn get_mute_debug_output(&self, producer: component_object_model::structures::Guid) -> bool {
        let _ = &producer;
        record(format!(
            "IDXGIInfoQueue::GetMuteDebugOutput(producer={:?})",
            producer
        ));
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::KeyedMutex for HostileKeyedMutex<A> {
    fn acquire_sync(
        &self,
        key: u64,
        dw_milliseconds: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &key;
        let _ = &dw_milliseconds;
        record(format!(
            "IDXGIKeyedMutex::AcquireSync(key={:?}, dw_milliseconds={:?})",
            key, dw_milliseconds
        ));
        Ok(())
    }
    fn release_sync(&self, key: u64) -> Result<(), component_object_model::structures::HResult> {
        let _ = &key;
        record(format!("IDXGIKeyedMutex::ReleaseSync(key={:?})", key));
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DeviceSubObject for HostileKeyedMutex<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileKeyedMutex<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output6 for HostileOutput<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput6::GetDesc1()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn check_hardware_composition_support(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGIOutput6::CheckHardwareCompositionSupport()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output5 for HostileOutput<A> {
    fn duplicate_output1(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, flags: u32, supported_formats: &[directx_graphics_infrastructure::enumerations::Format]) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let _ = &device;
        let _ = &flags;
        let _ = &supported_formats;
        record(format!("IDXGIOutput5::DuplicateOutput1(flags={:?})", flags));
        Ok(HostileOutputDuplication::new())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output4 for HostileOutput<A> {
    fn check_overlay_color_space_support(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &format;
        let _ = &color_space;
        let _ = &concerned_device;
        record(format!(
            "IDXGIOutput4::CheckOverlayColorSpaceSupport(format={:?}, color_space={:?})",
            format, color_space
        ));
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output3 for HostileOutput<A> {
    fn check_overlay_support(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &enum_format;
        let _ = &concerned_device;
        record(format!(
            "IDXGIOutput3::CheckOverlaySupport(enum_format={:?})",
            enum_format
        ));
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output2 for HostileOutput<A> {
    fn supports_overlays(&self) -> bool {
        record("IDXGIOutput2::SupportsOverlays()".to_string());
        Default::default()
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output1 for HostileOutput<A> {
    fn get_display_mode_list1(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc1>,
        component_object_model::structures::HResult,
    > {
        let _ = &enum_format;
        let _ = &flags;
        record(format!(
            "IDXGIOutput1::GetDisplayModeList1(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        Ok((0..2_u32)
            .map(|_| directx_graphics_infrastructure::structures::ModeDesc1 {
                width: 0_u32,
                height: 0_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 0_u32,
                    denominator: 0_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(0),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    0,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(0),
                stereo: false,
            })
            .collect())
    }
    fn find_closest_matching_mode1(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc1,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc1,
        component_object_model::structures::HResult,
    > {
        let _ = &mode_to_match;
        let _ = &concerned_device;
        record("IDXGIOutput1::FindClosestMatchingMode1()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_display_surface_data1(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &destination;
        record("IDXGIOutput1::GetDisplaySurfaceData1()".to_string());
        Ok(())
    }
    fn duplicate_output(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let _ = &device;
        record("IDXGIOutput1::DuplicateOutput()".to_string());
        Ok(HostileOutputDuplication::new())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Output for HostileOutput<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetDesc()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_display_mode_list(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc>,
        component_object_model::structures::HResult,
    > {
        let _ = &enum_format;
        let _ = &flags;
        record(format!(
            "IDXGIOutput::GetDisplayModeList(enum_format={:?}, flags={:?})",
            enum_format, flags
        ));
        Ok((0..2_u32)
            .map(|_| directx_graphics_infrastructure::structures::ModeDesc {
                width: 0_u32,
                height: 0_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 0_u32,
                    denominator: 0_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(0),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    0,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(0),
            })
            .collect())
    }
    fn find_closest_matching_mode(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc,
        component_object_model::structures::HResult,
    > {
        let _ = &mode_to_match;
        let _ = &concerned_device;
        record("IDXGIOutput::FindClosestMatchingMode()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn wait_for_v_blank(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutput::WaitForVBlank()".to_string());
        Ok(())
    }
    fn take_ownership(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        exclusive: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &device;
        let _ = &exclusive;
        record(format!(
            "IDXGIOutput::TakeOwnership(exclusive={:?})",
            exclusive
        ));
        Ok(())
    }
    fn release_ownership(&self) {
        record("IDXGIOutput::ReleaseOwnership()".to_string());
    }
    fn get_gamma_control_capabilities(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControlCapabilities,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetGammaControlCapabilities()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_gamma_control(
        &self,
        array: &directx_graphics_infrastructure::structures::GammaControl,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &array;
        record("IDXGIOutput::SetGammaControl()".to_string());
        Ok(())
    }
    fn get_gamma_control(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControl,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetGammaControl()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_display_surface(
        &self,
        scanout_surface: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &scanout_surface;
        record("IDXGIOutput::SetDisplaySurface()".to_string());
        Ok(())
    }
    fn get_display_surface_data(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &destination;
        record("IDXGIOutput::GetDisplaySurfaceData()".to_string());
        Ok(())
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutput::GetFrameStatistics()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileOutput<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::OutputDuplication
    for HostileOutputDuplication<A>
{
    fn get_desc(&self) -> directx_graphics_infrastructure::structures::OutduplDesc {
        record("IDXGIOutputDuplication::GetDesc()".to_string());
        directx_graphics_infrastructure::structures::OutduplDesc {
            mode_desc: directx_graphics_infrastructure::structures::ModeDesc {
                width: 0_u32,
                height: 0_u32,
                refresh_rate: directx_graphics_infrastructure::structures::Rational {
                    numerator: 0_u32,
                    denominator: 0_u32,
                },
                format: directx_graphics_infrastructure::enumerations::Format(0),
                scanline_ordering: directx_graphics_infrastructure::enumerations::ModeScanlineOrder(
                    0,
                ),
                scaling: directx_graphics_infrastructure::enumerations::ModeScaling(0),
            },
            rotation: directx_graphics_infrastructure::enumerations::ModeRotation(0),
            desktop_image_in_system_memory: false,
        }
    }
    fn acquire_next_frame(
        &self,
        timeout_in_milliseconds: u32,
    ) -> Result<
        (
            directx_graphics_infrastructure::structures::OutduplFrameInfo,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &timeout_in_milliseconds;
        record(format!(
            "IDXGIOutputDuplication::AcquireNextFrame(timeout_in_milliseconds={:?})",
            timeout_in_milliseconds
        ));
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_frame_dirty_rects(
        &self,
        dirty_rects_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &dirty_rects_buffer;
        record("IDXGIOutputDuplication::GetFrameDirtyRects()".to_string());
        touch(dirty_rects_buffer);
        Ok(Default::default())
    }
    fn get_frame_move_rects(
        &self,
        move_rect_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &move_rect_buffer;
        record("IDXGIOutputDuplication::GetFrameMoveRects()".to_string());
        touch(move_rect_buffer);
        Ok(Default::default())
    }
    fn get_frame_pointer_shape(
        &self,
        pointer_shape_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<
        (
            u32,
            directx_graphics_infrastructure::structures::OutduplPointerShapeInfo,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &pointer_shape_buffer;
        record("IDXGIOutputDuplication::GetFramePointerShape()".to_string());
        touch(pointer_shape_buffer);
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn map_desktop_surface(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        record("IDXGIOutputDuplication::MapDesktopSurface()".to_string());
        Err(E_NOTIMPL)
    }
    fn un_map_desktop_surface(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutputDuplication::UnMapDesktopSurface()".to_string());
        Ok(())
    }
    fn release_frame(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGIOutputDuplication::ReleaseFrame()".to_string());
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileOutputDuplication<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Resource1 for HostileResource<A> {
    fn create_subresource_surface(
        &self,
        index: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
        component_object_model::structures::HResult,
    > {
        let _ = &index;
        record(format!(
            "IDXGIResource1::CreateSubresourceSurface(index={:?})",
            index
        ));
        Ok(HostileSurface::new())
    }
    fn create_shared_handle(
        &self,
        attributes: Option<&component_object_model::structures::SecurityAttributes>,
        dw_access: u32,
        lp_name: Option<&[u16]>,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        let _ = &attributes;
        let _ = &dw_access;
        let _ = &lp_name;
        record("IDXGIResource1::CreateSharedHandle()".to_string());
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Resource for HostileResource<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetSharedHandle()".to_string());
        Ok(Default::default())
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetUsage()".to_string());
        Ok(Default::default())
    }
    fn set_eviction_priority(
        &self,
        eviction_priority: directx_graphics_infrastructure::enumerations::ResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &eviction_priority;
        record(format!(
            "IDXGIResource::SetEvictionPriority(eviction_priority={:?})",
            eviction_priority
        ));
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        record("IDXGIResource::GetEvictionPriority()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DeviceSubObject for HostileResource<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileResource<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Surface2 for HostileSurface<A> {
    fn get_resource(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        (
            directx_graphics_infrastructure::UnknownObject<Self::Api>,
            u32,
        ),
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGISurface2::GetResource()".to_string());
        Ok((
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
            Default::default(),
        ))
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Surface1 for HostileSurface<A> {
    fn get_dc(
        &self,
        discard: bool,
    ) -> Result<component_object_model::structures::Hdc, component_object_model::structures::HResult>
    {
        let _ = &discard;
        record(format!("IDXGISurface1::GetDC(discard={:?})", discard));
        Ok(Default::default())
    }
    fn release_dc(
        &self,
        dirty_rect: Option<&component_object_model::structures::Rect>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &dirty_rect;
        record("IDXGISurface1::ReleaseDC()".to_string());
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Surface for HostileSurface<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SurfaceDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISurface::GetDesc()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn map(
        &self,
        map_flags: directx_graphics_infrastructure::enumerations::MapFlags,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        let _ = &map_flags;
        record("IDXGISurface::Map()".to_string());
        Err(E_NOTIMPL)
    }
    fn unmap(&self) -> Result<(), component_object_model::structures::HResult> {
        record("IDXGISurface::Unmap()".to_string());
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DeviceSubObject for HostileSurface<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileSurface<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChain4 for HostileSwapChain<A> {
    fn set_hdr_meta_data(
        &self,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        meta_data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &type_;
        let _ = &meta_data;
        record(format!(
            "IDXGISwapChain4::SetHDRMetaData(type_={:?})",
            type_
        ));
        Ok(())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChain3 for HostileSwapChain<A> {
    fn get_current_back_buffer_index(&self) -> u32 {
        record("IDXGISwapChain3::GetCurrentBackBufferIndex()".to_string());
        Default::default()
    }
    fn check_color_space_support(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGISwapChain3::CheckColorSpaceSupport(color_space={:?})",
            color_space
        ));
        Ok(Default::default())
    }
    fn set_color_space1(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color_space;
        record(format!(
            "IDXGISwapChain3::SetColorSpace1(color_space={:?})",
            color_space
        ));
        Ok(())
    }
    fn resize_buffers1(
        &self,
        width: u32,
        height: u32,
        format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
        creation_node_mask: &[u32],
        present_queue: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        let _ = &format;
        let _ = &swap_chain_flags;
        let _ = &creation_node_mask;
        let _ = &present_queue;
        record("IDXGISwapChain3::ResizeBuffers1()".to_string());
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChain2 for HostileSwapChain<A> {
    fn set_source_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &width;
        let _ = &height;
        record(format!(
            "IDXGISwapChain2::SetSourceSize(width={:?}, height={:?})",
            width, height
        ));
        Ok(())
    }
    fn get_source_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        record("IDXGISwapChain2::GetSourceSize()".to_string());
        Ok((Default::default(), Default::default()))
    }
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &max_latency;
        record(format!(
            "IDXGISwapChain2::SetMaximumFrameLatency(max_latency={:?})",
            max_latency
        ));
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGISwapChain2::GetMaximumFrameLatency()".to_string());
        Ok(Default::default())
    }
    fn get_frame_latency_waitable_object(&self) -> component_object_model::structures::Handle {
        record("IDXGISwapChain2::GetFrameLatencyWaitableObject()".to_string());
        Default::default()
    }
    fn set_matrix_transform(
        &self,
        matrix: &directx_graphics_infrastructure::structures::Matrix3x2F,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &matrix;
        record("IDXGISwapChain2::SetMatrixTransform()".to_string());
        Ok(())
    }
    fn get_matrix_transform(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Matrix3x2F,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain2::GetMatrixTransform()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChain1 for HostileSwapChain<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc1,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetDesc1()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_fullscreen_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetFullscreenDesc()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_hwnd(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        record("IDXGISwapChain1::GetHwnd()".to_string());
        Ok(Default::default())
    }
    fn get_core_window(
        &self,
        refiid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &refiid;
        record("IDXGISwapChain1::GetCoreWindow()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(refiid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn present1(
        &self,
        sync_interval: u32,
        present_flags: directx_graphics_infrastructure::enumerations::Present,
        present_parameters: &directx_graphics_infrastructure::structures::PresentParameters,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &sync_interval;
        let _ = &present_flags;
        let _ = &present_parameters;
        record("IDXGISwapChain1::Present1()".to_string());
        Err(E_NOTIMPL)
    }
    fn is_temporary_mono_supported(&self) -> bool {
        record("IDXGISwapChain1::IsTemporaryMonoSupported()".to_string());
        Default::default()
    }
    fn get_restrict_to_output(
        &self,
    ) -> Result<
        Option<
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetRestrictToOutput()".to_string());
        Ok(Some(HostileOutput::new()))
    }
    fn set_background_color(
        &self,
        color: &directx_graphics_infrastructure::structures::Rgba,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &color;
        record("IDXGISwapChain1::SetBackgroundColor()".to_string());
        Ok(())
    }
    fn get_background_color(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Rgba,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetBackgroundColor()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_rotation(
        &self,
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &rotation;
        record(format!(
            "IDXGISwapChain1::SetRotation(rotation={:?})",
            rotation
        ));
        Ok(())
    }
    fn get_rotation(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ModeRotation,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain1::GetRotation()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChain for HostileSwapChain<A> {
    fn present(
        &self,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &sync_interval;
        let _ = &flags;
        record(format!(
            "IDXGISwapChain::Present(sync_interval={:?}, flags={:?})",
            sync_interval, flags
        ));
        Ok(())
    }
    fn get_buffer(
        &self,
        buffer: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer,
        component_object_model::structures::HResult,
    > {
        let _ = &buffer;
        record(format!("IDXGISwapChain::GetBuffer(buffer={:?})", buffer));
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_fullscreen_state(
        &self,
        fullscreen: bool,
        target: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fullscreen;
        let _ = &target;
        record(format!(
            "IDXGISwapChain::SetFullscreenState(fullscreen={:?})",
            fullscreen
        ));
        Ok(())
    }
    fn get_fullscreen_state(
        &self,
    ) -> Result<
        (
            bool,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        ),
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetFullscreenState()".to_string());
        Ok((Default::default(), HostileOutput::new()))
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetDesc()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &buffer_count;
        let _ = &width;
        let _ = &height;
        let _ = &new_format;
        let _ = &swap_chain_flags;
        record(format!(
            "IDXGISwapChain::ResizeBuffers(buffer_count={:?}, width={:?}, height={:?}, new_format={:?}, swap_chain_flags={:?})",
            buffer_count, width, height, new_format, swap_chain_flags
        ));
        Ok(())
    }
    fn resize_target(
        &self,
        new_target_parameters: &directx_graphics_infrastructure::structures::ModeDesc,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &new_target_parameters;
        record("IDXGISwapChain::ResizeTarget()".to_string());
        Ok(())
    }
    fn get_containing_output(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetContainingOutput()".to_string());
        Ok(HostileOutput::new())
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChain::GetFrameStatistics()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn get_last_present_count(&self) -> Result<u32, component_object_model::structures::HResult> {
        record("IDXGISwapChain::GetLastPresentCount()".to_string());
        Ok(Default::default())
    }
}

impl<A: Hostile> directx_graphics_infrastructure::DeviceSubObject for HostileSwapChain<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIDeviceSubObject::GetDevice()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::Object for HostileSwapChain<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::SetPrivateData()".to_string());
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &unknown;
        record("IDXGIObject::SetPrivateDataInterface()".to_string());
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &name;
        let _ = &data;
        record("IDXGIObject::GetPrivateData()".to_string());
        if let Some(bytes) = data {
            touch(bytes);
        }
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("IDXGIObject::GetParent()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> directx_graphics_infrastructure::SwapChainMedia for HostileSwapChainMedia<A> {
    fn get_frame_statistics_media(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatisticsMedia,
        component_object_model::structures::HResult,
    > {
        record("IDXGISwapChainMedia::GetFrameStatisticsMedia()".to_string());
        // A structure or a bound object: the hostile implementation makes none.
        Err(E_NOTIMPL)
    }
    fn set_present_duration(
        &self,
        duration: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &duration;
        record(format!(
            "IDXGISwapChainMedia::SetPresentDuration(duration={:?})",
            duration
        ));
        Ok(())
    }
    fn check_present_duration_support(
        &self,
        desired_present_duration: u32,
    ) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let _ = &desired_present_duration;
        record(format!(
            "IDXGISwapChainMedia::CheckPresentDurationSupport(desired_present_duration={:?})",
            desired_present_duration
        ));
        Ok((Default::default(), Default::default()))
    }
}

impl<A: Hostile> directx_graphics_infrastructure::GraphicsAnalysis for HostileGraphicsAnalysis<A> {
    fn begin_capture(&self) {
        record("IDXGraphicsAnalysis::BeginCapture()".to_string());
    }
    fn end_capture(&self) {
        record("IDXGraphicsAnalysis::EndCapture()".to_string());
    }
}

/// The hostile implementation's library: `DirectXGraphicsInfrastructure` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostileLibrary;

impl directx_graphics_infrastructure::DirectXGraphicsInfrastructure for HostileLibrary {
    type Adapter = HostileAdapter<Self>;
    type Debug = HostileDebug<Self>;
    type DecodeSwapChain = HostileDecodeSwapChain<Self>;
    type Device = HostileDevice<Self>;
    type DisplayControl = HostileDisplayControl<Self>;
    type Factory = HostileFactory<Self>;
    type FactoryMedia = HostileFactoryMedia<Self>;
    type InfoQueue = HostileInfoQueue<Self>;
    type KeyedMutex = HostileKeyedMutex<Self>;
    type Output = HostileOutput<Self>;
    type OutputDuplication = HostileOutputDuplication<Self>;
    type Resource = HostileResource<Self>;
    type Surface = HostileSurface<Self>;
    type SwapChain = HostileSwapChain<Self>;
    type SwapChainMedia = HostileSwapChainMedia<Self>;
    type GraphicsAnalysis = HostileGraphicsAnalysis<Self>;
    type Buffer = HostileSurface<Self>;

    fn create_dxgi_factory(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("CreateDXGIFactory()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_dxgi_factory1(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &riid;
        record("CreateDXGIFactory1()".to_string());
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_dxgi_factory2(
        &self,
        flags: directx_graphics_infrastructure::enumerations::CreateFactoryFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &flags;
        let _ = &riid;
        record(format!("CreateDXGIFactory2(flags={:?})", flags));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn dxgi_declare_adapter_removal_support(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("DXGIDeclareAdapterRemovalSupport()".to_string());
        Ok(())
    }
    fn dxgi_disable_v_blank_virtualization(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        record("DXGIDisableVBlankVirtualization()".to_string());
        Ok(())
    }
    fn dxgi_get_debug_interface1(
        &self,
        flags: u32,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let _ = &flags;
        let _ = &riid;
        record(format!("DXGIGetDebugInterface1(flags={:?})", flags));
        Ok(
            if [
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
                directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Adapter(HostileAdapter::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Debug(HostileDebug::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(
                    HostileDecodeSwapChain::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Device(HostileDevice::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::DisplayControl(
                    HostileDisplayControl::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
                directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Factory(HostileFactory::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(
                    HostileFactoryMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::InfoQueue(HostileInfoQueue::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(HostileKeyedMutex::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Output(HostileOutput::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(
                    HostileOutputDuplication::new(),
                )
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
                directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Resource(HostileResource::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::Surface(HostileSurface::new())
            } else if [
                directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
                directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            ]
            .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChain(HostileSwapChain::new())
            } else if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(
                    HostileSwapChainMedia::new(),
                )
            } else if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
                .contains(riid)
            {
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(
                    HostileGraphicsAnalysis::new(),
                )
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl Hostile for HostileLibrary {}

/// The methods a row answers for the system, and the argument it answers with (src/api.rs).
pub const ANSWERED: &[(&str, &str)] = &[("ID3D11Multithread::SetMultithreadProtected", "true")];

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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make))
        .ok()
        .flatten()
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
    let lengths: Vec<u64> = (0..1 + rng.below(3))
        .map(|_| [0, 1, 4, 8, 16, 64, 256, 4096, 1 << 20, u64::MAX][rng.below(10)])
        .collect();
    LyingBytes::lengths(&lengths)
}

/// What a fuzzing run did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fuzzed {
    pub driven: Driven,
    pub remade: usize,
}

/// The calling glue's API value over this hostile implementation, served by the serving glue.
pub type Api = directx_graphics_infrastructure_direct::SystemDirectXGraphicsInfrastructure;

/// One object of each kind the driver made, by kind.
pub struct Objects {
    pub api: Api,
    /// `Adapter`.
    pub k61: Option<directx_graphics_infrastructure_direct::SystemAdapter<Api>>,
    /// `Debug`.
    pub k62: Option<directx_graphics_infrastructure_direct::SystemDebug<Api>>,
    /// `DecodeSwapChain`.
    pub k63: Option<directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api>>,
    /// `Device`.
    pub k64: Option<directx_graphics_infrastructure_direct::SystemDevice<Api>>,
    /// `DisplayControl`.
    pub k65: Option<directx_graphics_infrastructure_direct::SystemDisplayControl<Api>>,
    /// `Factory`.
    pub k66: Option<directx_graphics_infrastructure_direct::SystemFactory<Api>>,
    /// `FactoryMedia`.
    pub k67: Option<directx_graphics_infrastructure_direct::SystemFactoryMedia<Api>>,
    /// `InfoQueue`.
    pub k68: Option<directx_graphics_infrastructure_direct::SystemInfoQueue<Api>>,
    /// `KeyedMutex`.
    pub k69: Option<directx_graphics_infrastructure_direct::SystemKeyedMutex<Api>>,
    /// `Output`.
    pub k70: Option<directx_graphics_infrastructure_direct::SystemOutput<Api>>,
    /// `OutputDuplication`.
    pub k71: Option<directx_graphics_infrastructure_direct::SystemOutputDuplication<Api>>,
    /// `Resource`.
    pub k72: Option<directx_graphics_infrastructure_direct::SystemResource<Api>>,
    /// `Surface`.
    pub k73: Option<directx_graphics_infrastructure_direct::SystemSurface<Api>>,
    /// `SwapChain`.
    pub k74: Option<directx_graphics_infrastructure_direct::SystemSwapChain<Api>>,
    /// `SwapChainMedia`.
    pub k75: Option<directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api>>,
    /// `GraphicsAnalysis`.
    pub k76: Option<directx_graphics_infrastructure_direct::SystemGraphicsAnalysis<Api>>,
}

impl Objects {
    /// No object yet, and the API served by the serving glue over the hostile implementation.
    pub fn new() -> Objects {
        Objects {
            api: directx_graphics_infrastructure_direct::served(HostileLibrary),
            k61: None,
            k62: None,
            k63: None,
            k64: None,
            k65: None,
            k66: None,
            k67: None,
            k68: None,
            k69: None,
            k70: None,
            k71: None,
            k72: None,
            k73: None,
            k74: None,
            k75: None,
            k76: None,
        }
    }

    /// How many kinds have an object.
    pub fn kinds_held(&self) -> usize {
        [
            self.k61.is_some(),
            self.k62.is_some(),
            self.k63.is_some(),
            self.k64.is_some(),
            self.k65.is_some(),
            self.k66.is_some(),
            self.k67.is_some(),
            self.k68.is_some(),
            self.k69.is_some(),
            self.k70.is_some(),
            self.k71.is_some(),
            self.k72.is_some(),
            self.k73.is_some(),
            self.k74.is_some(),
            self.k75.is_some(),
            self.k76.is_some(),
        ]
        .iter()
        .filter(|held| **held)
        .count()
    }
}

impl Default for Objects {
    fn default() -> Objects {
        Objects::new()
    }
}

impl Objects {
    /// The `Adapter` held, if any.
    pub fn adapter(&self) -> Option<&directx_graphics_infrastructure_direct::SystemAdapter<Api>> {
        self.k61.as_ref()
    }

    /// Holds a `Adapter`, in place of any held.
    pub fn put_adapter(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemAdapter<Api>,
    ) {
        self.k61 = Some(value);
    }

    /// The `Debug` held, if any.
    pub fn debug(&self) -> Option<&directx_graphics_infrastructure_direct::SystemDebug<Api>> {
        self.k62.as_ref()
    }

    /// Holds a `Debug`, in place of any held.
    pub fn put_debug(&mut self, value: directx_graphics_infrastructure_direct::SystemDebug<Api>) {
        self.k62 = Some(value);
    }

    /// The `DecodeSwapChain` held, if any.
    pub fn decode_swap_chain(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api>> {
        self.k63.as_ref()
    }

    /// Holds a `DecodeSwapChain`, in place of any held.
    pub fn put_decode_swap_chain(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api>,
    ) {
        self.k63 = Some(value);
    }

    /// The `Device` held, if any.
    pub fn device(&self) -> Option<&directx_graphics_infrastructure_direct::SystemDevice<Api>> {
        self.k64.as_ref()
    }

    /// Holds a `Device`, in place of any held.
    pub fn put_device(&mut self, value: directx_graphics_infrastructure_direct::SystemDevice<Api>) {
        self.k64 = Some(value);
    }

    /// The `DisplayControl` held, if any.
    pub fn display_control(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemDisplayControl<Api>> {
        self.k65.as_ref()
    }

    /// Holds a `DisplayControl`, in place of any held.
    pub fn put_display_control(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemDisplayControl<Api>,
    ) {
        self.k65 = Some(value);
    }

    /// The `Factory` held, if any.
    pub fn factory(&self) -> Option<&directx_graphics_infrastructure_direct::SystemFactory<Api>> {
        self.k66.as_ref()
    }

    /// Holds a `Factory`, in place of any held.
    pub fn put_factory(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemFactory<Api>,
    ) {
        self.k66 = Some(value);
    }

    /// The `FactoryMedia` held, if any.
    pub fn factory_media(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemFactoryMedia<Api>> {
        self.k67.as_ref()
    }

    /// Holds a `FactoryMedia`, in place of any held.
    pub fn put_factory_media(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemFactoryMedia<Api>,
    ) {
        self.k67 = Some(value);
    }

    /// The `InfoQueue` held, if any.
    pub fn info_queue(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemInfoQueue<Api>> {
        self.k68.as_ref()
    }

    /// Holds a `InfoQueue`, in place of any held.
    pub fn put_info_queue(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemInfoQueue<Api>,
    ) {
        self.k68 = Some(value);
    }

    /// The `KeyedMutex` held, if any.
    pub fn keyed_mutex(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemKeyedMutex<Api>> {
        self.k69.as_ref()
    }

    /// Holds a `KeyedMutex`, in place of any held.
    pub fn put_keyed_mutex(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemKeyedMutex<Api>,
    ) {
        self.k69 = Some(value);
    }

    /// The `Output` held, if any.
    pub fn output(&self) -> Option<&directx_graphics_infrastructure_direct::SystemOutput<Api>> {
        self.k70.as_ref()
    }

    /// Holds a `Output`, in place of any held.
    pub fn put_output(&mut self, value: directx_graphics_infrastructure_direct::SystemOutput<Api>) {
        self.k70 = Some(value);
    }

    /// The `OutputDuplication` held, if any.
    pub fn output_duplication(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemOutputDuplication<Api>> {
        self.k71.as_ref()
    }

    /// Holds a `OutputDuplication`, in place of any held.
    pub fn put_output_duplication(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemOutputDuplication<Api>,
    ) {
        self.k71 = Some(value);
    }

    /// The `Resource` held, if any.
    pub fn resource(&self) -> Option<&directx_graphics_infrastructure_direct::SystemResource<Api>> {
        self.k72.as_ref()
    }

    /// Holds a `Resource`, in place of any held.
    pub fn put_resource(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemResource<Api>,
    ) {
        self.k72 = Some(value);
    }

    /// The `Surface` held, if any.
    pub fn surface(&self) -> Option<&directx_graphics_infrastructure_direct::SystemSurface<Api>> {
        self.k73.as_ref()
    }

    /// Holds a `Surface`, in place of any held.
    pub fn put_surface(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemSurface<Api>,
    ) {
        self.k73 = Some(value);
    }

    /// The `SwapChain` held, if any.
    pub fn swap_chain(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemSwapChain<Api>> {
        self.k74.as_ref()
    }

    /// Holds a `SwapChain`, in place of any held.
    pub fn put_swap_chain(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemSwapChain<Api>,
    ) {
        self.k74 = Some(value);
    }

    /// The `SwapChainMedia` held, if any.
    pub fn swap_chain_media(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api>> {
        self.k75.as_ref()
    }

    /// Holds a `SwapChainMedia`, in place of any held.
    pub fn put_swap_chain_media(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api>,
    ) {
        self.k75 = Some(value);
    }

    /// The `GraphicsAnalysis` held, if any.
    pub fn graphics_analysis(
        &self,
    ) -> Option<&directx_graphics_infrastructure_direct::SystemGraphicsAnalysis<Api>> {
        self.k76.as_ref()
    }

    /// Holds a `GraphicsAnalysis`, in place of any held.
    pub fn put_graphics_analysis(
        &mut self,
        value: directx_graphics_infrastructure_direct::SystemGraphicsAnalysis<Api>,
    ) {
        self.k76 = Some(value);
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new; returns how
/// many kinds hold one.
pub fn make_all(o: &mut Objects) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        // IDXGIAdapter::EnumOutputs
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Adapter::enum_outputs(this, 22_u32);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k61.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIDevice::GetAdapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Device::get_adapter(this);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k64.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIFactory6::EnumAdapterByGpuPreference for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(this, 9_u32, directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIFactory4::EnumWarpAdapter for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIFactory2::CreateSwapChainForHwnd
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_hwnd(this, v1, component_object_model::structures::Hwnd(98), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, Some(&directx_graphics_infrastructure::structures::SwapChainFullscreenDesc { refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 87_u32, denominator: 94_u32 }, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, windowed: true }), o.k70.as_ref());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory2::CreateSwapChainForCoreWindow
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let v2 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_core_window(this, v1, v2, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory2::CreateSwapChainForComposition
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Factory2::create_swap_chain_for_composition(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactory1::EnumAdapters1
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory1::enum_adapters1(this, 9_u32);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory::EnumAdapters
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Factory::enum_adapters(this, 9_u32);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIFactory::CreateSwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Factory::create_swap_chain(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc { buffer_desc: directx_graphics_infrastructure::structures::ModeDesc { width: 65_u32, height: 72_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 86_u32, denominator: 93_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 72_u32, quality: 79_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 79_u32, output_window: component_object_model::structures::Hwnd(86), windowed: true, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, flags: 107_u32 });
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k66.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k67.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::FactoryMedia::create_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k67.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let v2 = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::FactoryMedia::create_decode_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::DecodeSwapChainDesc { flags: 58_u32 }, v2, o.k70.as_ref());
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k69.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIOutput5::DuplicateOutput1
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer = directx_graphics_infrastructure::Output5::duplicate_output1(this, v1, 31_u32, &[directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS]);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIOutput1::DuplicateOutput
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let v1 = (if let Some(x) = o.k61.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
                    } else if let Some(x) = o.k62.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
                    } else if let Some(x) = o.k63.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
                    } else if let Some(x) = o.k64.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Device(x))
                    } else if let Some(x) = o.k65.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                            x,
                        ))
                    } else if let Some(x) = o.k66.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
                    } else if let Some(x) = o.k67.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
                    } else if let Some(x) = o.k68.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
                    } else if let Some(x) = o.k69.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
                    } else if let Some(x) = o.k70.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Output(x))
                    } else if let Some(x) = o.k71.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
                    } else if let Some(x) = o.k72.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
                    } else if let Some(x) = o.k73.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
                    } else if let Some(x) = o.k74.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
                    } else if let Some(x) = o.k75.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                            x,
                        ))
                    } else if let Some(x) = o.k76.as_ref() {
                        Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
                    } else {
                        None
                    })?;
                    let answer =
                        directx_graphics_infrastructure::Output1::duplicate_output(this, v1);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k70.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIOutputDuplication::AcquireNextFrame
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                            this, 24_u32,
                        );
                    answer.ok()
                })
            };
            if let Some((p0, p1)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p1);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k71.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIResource1::CreateSubresourceSurface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::Resource1::create_subresource_surface(
                            this, 35_u32,
                        );
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k72.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k73.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGISwapChain1::GetCoreWindow for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::SwapChain1::get_core_window(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGISwapChain::GetFullscreenState
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this);
                    answer.ok()
                })
            };
            if let Some((p0, p1)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p1);
            }
        }
        // IDXGISwapChain::GetContainingOutput
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer =
                        directx_graphics_infrastructure::SwapChain::get_containing_output(this);
                    answer.ok()
                })
            };
            if let Some(p0) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIDeviceSubObject::GetDevice for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::DeviceSubObject::get_device(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // IDXGIObject::GetParent for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k74.as_ref()?;
                    let answer = directx_graphics_infrastructure::Object::get_parent(
                        this,
                        &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                    );
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // CreateDXGIFactory for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // CreateDXGIFactory for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // CreateDXGIFactory for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // CreateDXGIFactory for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // CreateDXGIFactory for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // CreateDXGIFactory for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // CreateDXGIFactory for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // CreateDXGIFactory for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // CreateDXGIFactory for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // CreateDXGIFactory for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // CreateDXGIFactory1 for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // CreateDXGIFactory2 for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Adapter
        if o.k61.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Adapter(p0)) = answer
                && o.k61.is_none()
            {
                o.k61 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Debug
        if o.k62.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Debug(p0)) = answer
                && o.k62.is_none()
            {
                o.k62 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for DecodeSwapChain
        if o.k63.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(p0)) =
                answer
                && o.k63.is_none()
            {
                o.k63 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Device
        if o.k64.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Device(p0)) = answer
                && o.k64.is_none()
            {
                o.k64 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for DisplayControl
        if o.k65.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::DisplayControl(p0)) = answer
                && o.k65.is_none()
            {
                o.k65 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Factory
        if o.k66.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Factory(p0)) = answer
                && o.k66.is_none()
            {
                o.k66 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for FactoryMedia
        if o.k67.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(p0)) = answer
                && o.k67.is_none()
            {
                o.k67 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for InfoQueue
        if o.k68.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(p0)) = answer
                && o.k68.is_none()
            {
                o.k68 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for KeyedMutex
        if o.k69.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(p0)) = answer
                && o.k69.is_none()
            {
                o.k69 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Output
        if o.k70.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Output(p0)) = answer
                && o.k70.is_none()
            {
                o.k70 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for OutputDuplication
        if o.k71.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::OutputDuplication(p0)) =
                answer
                && o.k71.is_none()
            {
                o.k71 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Resource
        if o.k72.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Resource(p0)) = answer
                && o.k72.is_none()
            {
                o.k72 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for Surface
        if o.k73.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::Surface(p0)) = answer
                && o.k73.is_none()
            {
                o.k73 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for SwapChain
        if o.k74.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChain(p0)) = answer
                && o.k74.is_none()
            {
                o.k74 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for SwapChainMedia
        if o.k75.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::SwapChainMedia(p0)) = answer
                && o.k75.is_none()
            {
                o.k75 = Some(p0);
            }
        }
        // DXGIGetDebugInterface1 for GraphicsAnalysis
        if o.k76.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS);
                    answer.ok()
                })
            };
            if let Some(directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(p0)) =
                answer
                && o.k76.is_none()
            {
                o.k76 = Some(p0);
            }
        }
        if o.kinds_held() == before {
            break;
        }
    }
    o.kinds_held()
}

/// Calls every method the calling glue carries on every object held, and the library's exports, each with arguments of
/// defaults and `bytes()` for every caller's bytes; returns what was called.
pub fn drive(o: &Objects, bytes: &dyn Fn() -> LyingBytes) -> Driven {
    drive_where(o, bytes, &|_| true)
}

/// `drive`, of the methods of the interfaces (or the exports) `wanted` names only.
pub fn drive_where(
    o: &Objects,
    bytes: &dyn Fn() -> LyingBytes,
    wanted: &dyn Fn(&str) -> bool,
) -> Driven {
    let mut driven = Driven::default();
    // IDXGIAdapter4::GetDesc3 on Adapter
    if wanted(IDXGI_ADAPTER4) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter4::get_desc3(this);
            true
        });
    }
    // IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::Adapter3::register_hardware_content_protection_teardown_status_event(this, component_object_model::structures::Handle(82));
        true
        });
    }
    // IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::Adapter3::unregister_hardware_content_protection_teardown_status(this, 87_u32);
        true
        });
    }
    // IDXGIAdapter3::QueryVideoMemoryInfo on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter3::query_video_memory_info(
                this,
                72_u32,
                directx_graphics_infrastructure::enumerations::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
            );
            true
        });
    }
    // IDXGIAdapter3::SetVideoMemoryReservation on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter3::set_video_memory_reservation(
                this,
                72_u32,
                directx_graphics_infrastructure::enumerations::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
                73_u64,
            );
            true
        });
    }
    // IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::Adapter3::register_video_memory_budget_change_notification_event(this, component_object_model::structures::Handle(82));
        true
        });
    }
    // IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter
    if wanted(IDXGI_ADAPTER3) {
        driven.attempt(|| {
        let Some(this) = o.k61.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::Adapter3::unregister_video_memory_budget_change_notification(this, 87_u32);
        true
        });
    }
    // IDXGIAdapter2::GetDesc2 on Adapter
    if wanted(IDXGI_ADAPTER2) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter2::get_desc2(this);
            true
        });
    }
    // IDXGIAdapter1::GetDesc1 on Adapter
    if wanted(IDXGI_ADAPTER1) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter1::get_desc1(this);
            true
        });
    }
    // IDXGIAdapter::EnumOutputs on Adapter
    if wanted(IDXGI_ADAPTER) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter::enum_outputs(this, 22_u32);
            true
        });
    }
    // IDXGIAdapter::GetDesc on Adapter
    if wanted(IDXGI_ADAPTER) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter::get_desc(this);
            true
        });
    }
    // IDXGIAdapter::CheckInterfaceSupport on Adapter
    if wanted(IDXGI_ADAPTER) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Adapter::check_interface_support(
                this,
                &component_object_model::structures::Guid {
                    data1: 117_u32,
                    data2: 124_u16,
                    data3: 5_u16,
                    data4: core::array::from_fn(|_| 12_u8),
                },
            );
            true
        });
    }
    // IDXGIObject::SetPrivateData on Adapter
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Adapter
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Adapter
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k61.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIDebug1::EnableLeakTrackingForThread on Debug
    if wanted(IDXGI_DEBUG1) {
        driven.attempt(|| {
            let Some(this) = o.k62.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Debug1::enable_leak_tracking_for_thread(this);
            true
        });
    }
    // IDXGIDebug1::DisableLeakTrackingForThread on Debug
    if wanted(IDXGI_DEBUG1) {
        driven.attempt(|| {
            let Some(this) = o.k62.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Debug1::disable_leak_tracking_for_thread(this);
            true
        });
    }
    // IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug
    if wanted(IDXGI_DEBUG1) {
        driven.attempt(|| {
            let Some(this) = o.k62.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::Debug1::is_leak_tracking_enabled_for_thread(this);
            true
        });
    }
    // IDXGIDebug::ReportLiveObjects on Debug
    if wanted(IDXGI_DEBUG) {
        driven.attempt(|| {
            let Some(this) = o.k62.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Debug::report_live_objects(
                this,
                component_object_model::structures::Guid {
                    data1: 61_u32,
                    data2: 68_u16,
                    data3: 75_u16,
                    data4: core::array::from_fn(|_| 82_u8),
                },
                directx_graphics_infrastructure::enumerations::DXGI_DEBUG_RLO_SUMMARY,
            );
            true
        });
    }
    // IDXGIDecodeSwapChain::PresentBuffer on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::present_buffer(
                this,
                61_u32,
                46_u32,
                directx_graphics_infrastructure::enumerations::DXGI_PRESENT_TEST,
            );
            true
        });
    }
    // IDXGIDecodeSwapChain::SetSourceRect on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::set_source_rect(
                this,
                &component_object_model::structures::Rect {
                    left: 37_i32,
                    top: 44_i32,
                    right: 51_i32,
                    bottom: 58_i32,
                },
            );
            true
        });
    }
    // IDXGIDecodeSwapChain::SetTargetRect on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::set_target_rect(
                this,
                &component_object_model::structures::Rect {
                    left: 37_i32,
                    top: 44_i32,
                    right: 51_i32,
                    bottom: 58_i32,
                },
            );
            true
        });
    }
    // IDXGIDecodeSwapChain::SetDestSize on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::set_dest_size(
                this, 120_u32, 44_u32,
            );
            true
        });
    }
    // IDXGIDecodeSwapChain::GetSourceRect on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::get_source_rect(this);
            true
        });
    }
    // IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::get_target_rect(this);
            true
        });
    }
    // IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::get_dest_size(this);
            true
        });
    }
    // IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
        let Some(this) = o.k63.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::DecodeSwapChain::set_color_space(this, directx_graphics_infrastructure::enumerations::DXGI_MULTIPLANE_OVERLAY_YCBCR_FLAG_NOMINAL_RANGE);
        true
        });
    }
    // IDXGIDecodeSwapChain::GetColorSpace on DecodeSwapChain
    if wanted(IDXGI_DECODE_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k63.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DecodeSwapChain::get_color_space(this);
            true
        });
    }
    // IDXGIDevice4::OfferResources1 on Device
    if wanted(IDXGI_DEVICE4) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device4::offer_resources1(
                this,
                &[o.k72.as_ref()],
                directx_graphics_infrastructure::enumerations::DXGI_OFFER_RESOURCE_PRIORITY_LOW,
                31_u32,
            );
            true
        });
    }
    // IDXGIDevice4::ReclaimResources1 on Device
    if wanted(IDXGI_DEVICE4) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device4::reclaim_resources1(
                this,
                &[o.k72.as_ref()],
            );
            true
        });
    }
    // IDXGIDevice3::Trim on Device
    if wanted(IDXGI_DEVICE3) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device3::trim(this);
            true
        });
    }
    // IDXGIDevice2::OfferResources on Device
    if wanted(IDXGI_DEVICE2) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device2::offer_resources(
                this,
                &[o.k72.as_ref()],
                directx_graphics_infrastructure::enumerations::DXGI_OFFER_RESOURCE_PRIORITY_LOW,
            );
            true
        });
    }
    // IDXGIDevice2::ReclaimResources on Device
    if wanted(IDXGI_DEVICE2) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device2::reclaim_resources(
                this,
                &[o.k72.as_ref()],
            );
            true
        });
    }
    // IDXGIDevice2::EnqueueSetEvent on Device
    if wanted(IDXGI_DEVICE2) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device2::enqueue_set_event(
                this,
                component_object_model::structures::Handle(82),
            );
            true
        });
    }
    // IDXGIDevice1::SetMaximumFrameLatency on Device
    if wanted(IDXGI_DEVICE1) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::Device1::set_maximum_frame_latency(this, 63_u32);
            true
        });
    }
    // IDXGIDevice1::GetMaximumFrameLatency on Device
    if wanted(IDXGI_DEVICE1) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device1::get_maximum_frame_latency(this);
            true
        });
    }
    // IDXGIDevice::GetAdapter on Device
    if wanted(IDXGI_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device::get_adapter(this);
            true
        });
    }
    // IDXGIDevice::CreateSurface on Device
    if wanted(IDXGI_DEVICE) {
        driven.attempt(|| {
        let Some(this) = o.k64.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::Device::create_surface(this, &directx_graphics_infrastructure::structures::SurfaceDesc { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 86_u32, quality: 93_u32 } }, 116_u32, directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, Some(&directx_graphics_infrastructure::structures::SharedResource { handle: component_object_model::structures::Handle(55) }));
        true
        });
    }
    // IDXGIDevice::SetGPUThreadPriority on Device
    if wanted(IDXGI_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device::set_gpu_thread_priority(this, 14_i32);
            true
        });
    }
    // IDXGIDevice::GetGPUThreadPriority on Device
    if wanted(IDXGI_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Device::get_gpu_thread_priority(this);
            true
        });
    }
    // IDXGIObject::SetPrivateData on Device
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Device
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Device
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k64.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIDisplayControl::IsStereoEnabled on DisplayControl
    if wanted(IDXGI_DISPLAY_CONTROL) {
        driven.attempt(|| {
            let Some(this) = o.k65.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DisplayControl::is_stereo_enabled(this);
            true
        });
    }
    // IDXGIDisplayControl::SetStereoEnabled on DisplayControl
    if wanted(IDXGI_DISPLAY_CONTROL) {
        driven.attempt(|| {
            let Some(this) = o.k65.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DisplayControl::set_stereo_enabled(this, true);
            true
        });
    }
    // IDXGIFactory7::RegisterAdaptersChangedEvent on Factory
    if wanted(IDXGI_FACTORY7) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory7::register_adapters_changed_event(
                this,
                component_object_model::structures::Handle(82),
            );
            true
        });
    }
    // IDXGIFactory7::UnregisterAdaptersChangedEvent on Factory
    if wanted(IDXGI_FACTORY7) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory7::unregister_adapters_changed_event(
                this, 87_u32,
            );
            true
        });
    }
    // IDXGIFactory6::EnumAdapterByGpuPreference on Factory
    if wanted(IDXGI_FACTORY6) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory6::enum_adapter_by_gpu_preference(
                this,
                9_u32,
                directx_graphics_infrastructure::enumerations::DXGI_GPU_PREFERENCE_MINIMUM_POWER,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIFactory5::CheckFeatureSupport on Factory
    if wanted(IDXGI_FACTORY5) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let lying = bytes();
            let _ = directx_graphics_infrastructure::Factory5::check_feature_support(
                this,
                directx_graphics_infrastructure::enumerations::Feature(1),
                &lying,
            );
            true
        });
    }
    // IDXGIFactory4::EnumWarpAdapter on Factory
    if wanted(IDXGI_FACTORY4) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory4::enum_warp_adapter(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIFactory3::GetCreationFlags on Factory
    if wanted(IDXGI_FACTORY3) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory3::get_creation_flags(this);
            true
        });
    }
    // IDXGIFactory2::IsWindowedStereoEnabled on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::is_windowed_stereo_enabled(this);
            true
        });
    }
    // IDXGIFactory2::CreateSwapChainForHwnd on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Factory2::create_swap_chain_for_hwnd(this, v1, component_object_model::structures::Hwnd(98), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, Some(&directx_graphics_infrastructure::structures::SwapChainFullscreenDesc { refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 87_u32, denominator: 94_u32 }, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, windowed: true }), o.k70.as_ref());
        true
        });
    }
    // IDXGIFactory2::CreateSwapChainForCoreWindow on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let Some(v2) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Factory2::create_swap_chain_for_core_window(this, v1, v2, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
        true
        });
    }
    // IDXGIFactory2::GetSharedResourceAdapterLuid on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::get_shared_resource_adapter_luid(
                this,
                component_object_model::structures::Handle(68),
            );
            true
        });
    }
    // IDXGIFactory2::RegisterStereoStatusWindow on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::register_stereo_status_window(
                this,
                component_object_model::structures::Hwnd(95),
                29_u32,
            );
            true
        });
    }
    // IDXGIFactory2::RegisterStereoStatusEvent on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::register_stereo_status_event(
                this,
                component_object_model::structures::Handle(82),
            );
            true
        });
    }
    // IDXGIFactory2::UnregisterStereoStatus on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::Factory2::unregister_stereo_status(this, 87_u32);
            true
        });
    }
    // IDXGIFactory2::RegisterOcclusionStatusWindow on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::register_occlusion_status_window(
                this,
                component_object_model::structures::Hwnd(95),
                29_u32,
            );
            true
        });
    }
    // IDXGIFactory2::RegisterOcclusionStatusEvent on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::register_occlusion_status_event(
                this,
                component_object_model::structures::Handle(82),
            );
            true
        });
    }
    // IDXGIFactory2::UnregisterOcclusionStatus on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory2::unregister_occlusion_status(
                this, 87_u32,
            );
            true
        });
    }
    // IDXGIFactory2::CreateSwapChainForComposition on Factory
    if wanted(IDXGI_FACTORY2) {
        driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Factory2::create_swap_chain_for_composition(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
        true
        });
    }
    // IDXGIFactory1::EnumAdapters1 on Factory
    if wanted(IDXGI_FACTORY1) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory1::enum_adapters1(this, 9_u32);
            true
        });
    }
    // IDXGIFactory1::IsCurrent on Factory
    if wanted(IDXGI_FACTORY1) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory1::is_current(this);
            true
        });
    }
    // IDXGIFactory::EnumAdapters on Factory
    if wanted(IDXGI_FACTORY) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory::enum_adapters(this, 9_u32);
            true
        });
    }
    // IDXGIFactory::MakeWindowAssociation on Factory
    if wanted(IDXGI_FACTORY) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory::make_window_association(
                this,
                component_object_model::structures::Hwnd(95),
                directx_graphics_infrastructure::enumerations::DXGI_MWA_NO_WINDOW_CHANGES,
            );
            true
        });
    }
    // IDXGIFactory::GetWindowAssociation on Factory
    if wanted(IDXGI_FACTORY) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Factory::get_window_association(this);
            true
        });
    }
    // IDXGIFactory::CreateSwapChain on Factory
    if wanted(IDXGI_FACTORY) {
        driven.attempt(|| {
        let Some(this) = o.k66.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Factory::create_swap_chain(this, v1, &directx_graphics_infrastructure::structures::SwapChainDesc { buffer_desc: directx_graphics_infrastructure::structures::ModeDesc { width: 65_u32, height: 72_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 86_u32, denominator: 93_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 72_u32, quality: 79_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 79_u32, output_window: component_object_model::structures::Hwnd(86), windowed: true, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, flags: 107_u32 });
        true
        });
    }
    // IDXGIObject::SetPrivateData on Factory
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Factory
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Factory
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k66.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle on FactoryMedia
    if wanted(IDXGI_FACTORY_MEDIA) {
        driven.attempt(|| {
        let Some(this) = o.k67.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::FactoryMedia::create_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::SwapChainDesc1 { width: 58_u32, height: 65_u32, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, stereo: true, sample_desc: directx_graphics_infrastructure::structures::SampleDesc { count: 93_u32, quality: 100_u32 }, buffer_usage: directx_graphics_infrastructure::enumerations::DXGI_USAGE_SHADER_INPUT, buffer_count: 100_u32, scaling: directx_graphics_infrastructure::enumerations::DXGI_SCALING_NONE, swap_effect: directx_graphics_infrastructure::enumerations::DXGI_SWAP_EFFECT_SEQUENTIAL, alpha_mode: directx_graphics_infrastructure::enumerations::DXGI_ALPHA_MODE_PREMULTIPLIED, flags: 2_u32 }, o.k70.as_ref());
        true
        });
    }
    // IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle on FactoryMedia
    if wanted(IDXGI_FACTORY_MEDIA) {
        driven.attempt(|| {
        let Some(this) = o.k67.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let Some(v2) = o.k72.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::FactoryMedia::create_decode_swap_chain_for_composition_surface_handle(this, v1, component_object_model::structures::Handle(67), &directx_graphics_infrastructure::structures::DecodeSwapChainDesc { flags: 58_u32 }, v2, o.k70.as_ref());
        true
        });
    }
    // IDXGIInfoQueue::SetMessageCountLimit on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::set_message_count_limit(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                110_u64,
            );
            true
        });
    }
    // IDXGIInfoQueue::ClearStoredMessages on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::clear_stored_messages(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_num_stored_messages_allowed_by_retrieval_filters(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) });
        true
        });
    }
    // IDXGIInfoQueue::GetNumStoredMessages on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_num_stored_messages(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_num_messages_discarded_by_message_count_limit(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) });
        true
        });
    }
    // IDXGIInfoQueue::GetMessageCountLimit on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_message_count_limit(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_num_messages_allowed_by_storage_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) });
        true
        });
    }
    // IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_num_messages_denied_by_storage_filter(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) });
        true
        });
    }
    // IDXGIInfoQueue::ClearStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::clear_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushEmptyStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_empty_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushDenyAllStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_deny_all_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushCopyOfStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_copy_of_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PopStorageFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::pop_storage_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::GetStorageFilterStackSize on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_storage_filter_stack_size(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::ClearRetrievalFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::clear_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushEmptyRetrievalFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_empty_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushDenyAllRetrievalFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_deny_all_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PushCopyOfRetrievalFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::push_copy_of_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::PopRetrievalFilter on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::pop_retrieval_filter(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::GetRetrievalFilterStackSize on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_retrieval_filter_stack_size(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIInfoQueue::AddMessage on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::add_message(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, 101_i32, c"known");
        true
        });
    }
    // IDXGIInfoQueue::AddApplicationMessage on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::add_application_message(this, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, c"known");
        true
        });
    }
    // IDXGIInfoQueue::SetBreakOnCategory on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::set_break_on_category(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS, false);
        true
        });
    }
    // IDXGIInfoQueue::SetBreakOnSeverity on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::set_break_on_severity(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR, false);
        true
        });
    }
    // IDXGIInfoQueue::SetBreakOnID on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::set_break_on_id(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                101_i32,
                false,
            );
            true
        });
    }
    // IDXGIInfoQueue::GetBreakOnCategory on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_break_on_category(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS);
        true
        });
    }
    // IDXGIInfoQueue::GetBreakOnSeverity on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
        let Some(this) = o.k68.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::InfoQueue::get_break_on_severity(this, component_object_model::structures::Guid { data1: 84_u32, data2: 91_u16, data3: 98_u16, data4: core::array::from_fn(|_| 105_u8) }, directx_graphics_infrastructure::enumerations::DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR);
        true
        });
    }
    // IDXGIInfoQueue::GetBreakOnID on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_break_on_id(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                101_i32,
            );
            true
        });
    }
    // IDXGIInfoQueue::SetMuteDebugOutput on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::set_mute_debug_output(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
                true,
            );
            true
        });
    }
    // IDXGIInfoQueue::GetMuteDebugOutput on InfoQueue
    if wanted(IDXGI_INFO_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k68.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::InfoQueue::get_mute_debug_output(
                this,
                component_object_model::structures::Guid {
                    data1: 84_u32,
                    data2: 91_u16,
                    data3: 98_u16,
                    data4: core::array::from_fn(|_| 105_u8),
                },
            );
            true
        });
    }
    // IDXGIKeyedMutex::AcquireSync on KeyedMutex
    if wanted(IDXGI_KEYED_MUTEX) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::KeyedMutex::acquire_sync(this, 35_u64, 45_u32);
            true
        });
    }
    // IDXGIKeyedMutex::ReleaseSync on KeyedMutex
    if wanted(IDXGI_KEYED_MUTEX) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::KeyedMutex::release_sync(this, 35_u64);
            true
        });
    }
    // IDXGIDeviceSubObject::GetDevice on KeyedMutex
    if wanted(IDXGI_DEVICE_SUB_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIObject::SetPrivateData on KeyedMutex
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on KeyedMutex
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on KeyedMutex
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k69.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIOutput6::GetDesc1 on Output
    if wanted(IDXGI_OUTPUT6) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output6::get_desc1(this);
            true
        });
    }
    // IDXGIOutput6::CheckHardwareCompositionSupport on Output
    if wanted(IDXGI_OUTPUT6) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::Output6::check_hardware_composition_support(this);
            true
        });
    }
    // IDXGIOutput5::DuplicateOutput1 on Output
    if wanted(IDXGI_OUTPUT5) {
        driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Output5::duplicate_output1(this, v1, 31_u32, &[directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS]);
        true
        });
    }
    // IDXGIOutput4::CheckOverlayColorSpaceSupport on Output
    if wanted(IDXGI_OUTPUT4) {
        driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let Some(v1) = (if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None }) else { return false; };
        let _ = directx_graphics_infrastructure::Output4::check_overlay_color_space_support(this, directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709, v1);
        true
        });
    }
    // IDXGIOutput3::CheckOverlaySupport on Output
    if wanted(IDXGI_OUTPUT3) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = (if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            }) else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output3::check_overlay_support(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                v1,
            );
            true
        });
    }
    // IDXGIOutput2::SupportsOverlays on Output
    if wanted(IDXGI_OUTPUT2) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output2::supports_overlays(this);
            true
        });
    }
    // IDXGIOutput1::GetDisplayModeList1 on Output
    if wanted(IDXGI_OUTPUT1) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output1::get_display_mode_list1(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                directx_graphics_infrastructure::enumerations::DXGI_ENUM_MODES_INTERLACED,
            );
            true
        });
    }
    // IDXGIOutput1::FindClosestMatchingMode1 on Output
    if wanted(IDXGI_OUTPUT1) {
        driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let v1 = if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None };
        let _ = directx_graphics_infrastructure::Output1::find_closest_matching_mode1(this, &directx_graphics_infrastructure::structures::ModeDesc1 { width: 4_u32, height: 11_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 25_u32, denominator: 32_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED, stereo: true }, v1);
        true
        });
    }
    // IDXGIOutput1::GetDisplaySurfaceData1 on Output
    if wanted(IDXGI_OUTPUT1) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output1::get_display_surface_data1(this, v1);
            true
        });
    }
    // IDXGIOutput1::DuplicateOutput on Output
    if wanted(IDXGI_OUTPUT1) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = (if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            }) else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output1::duplicate_output(this, v1);
            true
        });
    }
    // IDXGIOutput::GetDesc on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_desc(this);
            true
        });
    }
    // IDXGIOutput::GetDisplayModeList on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_display_mode_list(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                directx_graphics_infrastructure::enumerations::DXGI_ENUM_MODES_INTERLACED,
            );
            true
        });
    }
    // IDXGIOutput::FindClosestMatchingMode on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
        let Some(this) = o.k70.as_ref() else { return false; };
        let v1 = if let Some(x) = o.k61.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Adapter(x)) } else if let Some(x) = o.k62.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Debug(x)) } else if let Some(x) = o.k63.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x)) } else if let Some(x) = o.k64.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Device(x)) } else if let Some(x) = o.k65.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(x)) } else if let Some(x) = o.k66.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Factory(x)) } else if let Some(x) = o.k67.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x)) } else if let Some(x) = o.k68.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x)) } else if let Some(x) = o.k69.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x)) } else if let Some(x) = o.k70.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Output(x)) } else if let Some(x) = o.k71.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x)) } else if let Some(x) = o.k72.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Resource(x)) } else if let Some(x) = o.k73.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::Surface(x)) } else if let Some(x) = o.k74.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x)) } else if let Some(x) = o.k75.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(x)) } else if let Some(x) = o.k76.as_ref() { Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x)) } else { None };
        let _ = directx_graphics_infrastructure::Output::find_closest_matching_mode(this, &directx_graphics_infrastructure::structures::ModeDesc { width: 4_u32, height: 11_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 25_u32, denominator: 32_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED }, v1);
        true
        });
    }
    // IDXGIOutput::WaitForVBlank on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::wait_for_v_blank(this);
            true
        });
    }
    // IDXGIOutput::TakeOwnership on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = (if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            }) else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::take_ownership(this, v1, true);
            true
        });
    }
    // IDXGIOutput::ReleaseOwnership on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::release_ownership(this);
            true
        });
    }
    // IDXGIOutput::GetGammaControlCapabilities on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_gamma_control_capabilities(this);
            true
        });
    }
    // IDXGIOutput::SetGammaControl on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::set_gamma_control(
                this,
                &directx_graphics_infrastructure::structures::GammaControl {
                    scale: directx_graphics_infrastructure::structures::Rgb {
                        red: 126.5_f32,
                        green: 7.5_f32,
                        blue: 14.5_f32,
                    },
                    offset: directx_graphics_infrastructure::structures::Rgb {
                        red: 7.5_f32,
                        green: 14.5_f32,
                        blue: 21.5_f32,
                    },
                    gamma_curve: core::array::from_fn(|_| {
                        directx_graphics_infrastructure::structures::Rgb {
                            red: 14.5_f32,
                            green: 21.5_f32,
                            blue: 28.5_f32,
                        }
                    }),
                },
            );
            true
        });
    }
    // IDXGIOutput::GetGammaControl on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_gamma_control(this);
            true
        });
    }
    // IDXGIOutput::SetDisplaySurface on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::set_display_surface(this, v1);
            true
        });
    }
    // IDXGIOutput::GetDisplaySurfaceData on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let Some(v1) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_display_surface_data(this, v1);
            true
        });
    }
    // IDXGIOutput::GetFrameStatistics on Output
    if wanted(IDXGI_OUTPUT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Output::get_frame_statistics(this);
            true
        });
    }
    // IDXGIObject::SetPrivateData on Output
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Output
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Output
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k70.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIOutputDuplication::GetDesc on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::OutputDuplication::get_desc(this);
            true
        });
    }
    // IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::OutputDuplication::acquire_next_frame(
                this, 24_u32,
            );
            true
        });
    }
    // IDXGIOutputDuplication::GetFrameDirtyRects on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let lying = bytes();
            let _ = directx_graphics_infrastructure::OutputDuplication::get_frame_dirty_rects(
                this, &lying,
            );
            true
        });
    }
    // IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let lying = bytes();
            let _ = directx_graphics_infrastructure::OutputDuplication::get_frame_move_rects(
                this, &lying,
            );
            true
        });
    }
    // IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let lying = bytes();
            let _ = directx_graphics_infrastructure::OutputDuplication::get_frame_pointer_shape(
                this, &lying,
            );
            true
        });
    }
    // IDXGIOutputDuplication::UnMapDesktopSurface on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::OutputDuplication::un_map_desktop_surface(this);
            true
        });
    }
    // IDXGIOutputDuplication::ReleaseFrame on OutputDuplication
    if wanted(IDXGI_OUTPUT_DUPLICATION) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::OutputDuplication::release_frame(this);
            true
        });
    }
    // IDXGIObject::SetPrivateData on OutputDuplication
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on OutputDuplication
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on OutputDuplication
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k71.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIResource1::CreateSubresourceSurface on Resource
    if wanted(IDXGI_RESOURCE1) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Resource1::create_subresource_surface(
                this, 35_u32,
            );
            true
        });
    }
    // IDXGIResource::GetSharedHandle on Resource
    if wanted(IDXGI_RESOURCE) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Resource::get_shared_handle(this);
            true
        });
    }
    // IDXGIResource::GetUsage on Resource
    if wanted(IDXGI_RESOURCE) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Resource::get_usage(this);
            true
        });
    }
    // IDXGIResource::SetEvictionPriority on Resource
    if wanted(IDXGI_RESOURCE) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Resource::set_eviction_priority(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_RESOURCE_PRIORITY_MINIMUM,
            );
            true
        });
    }
    // IDXGIResource::GetEvictionPriority on Resource
    if wanted(IDXGI_RESOURCE) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Resource::get_eviction_priority(this);
            true
        });
    }
    // IDXGIDeviceSubObject::GetDevice on Resource
    if wanted(IDXGI_DEVICE_SUB_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIObject::SetPrivateData on Resource
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Resource
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Resource
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k72.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGISurface2::GetResource on Surface
    if wanted(IDXGI_SURFACE2) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Surface2::get_resource(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGISurface1::GetDC on Surface
    if wanted(IDXGI_SURFACE1) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Surface1::get_dc(this, true);
            true
        });
    }
    // IDXGISurface1::ReleaseDC on Surface
    if wanted(IDXGI_SURFACE1) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Surface1::release_dc(
                this,
                Some(&component_object_model::structures::Rect {
                    left: 1_i32,
                    top: 8_i32,
                    right: 15_i32,
                    bottom: 22_i32,
                }),
            );
            true
        });
    }
    // IDXGISurface::GetDesc on Surface
    if wanted(IDXGI_SURFACE) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Surface::get_desc(this);
            true
        });
    }
    // IDXGISurface::Unmap on Surface
    if wanted(IDXGI_SURFACE) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Surface::unmap(this);
            true
        });
    }
    // IDXGIDeviceSubObject::GetDevice on Surface
    if wanted(IDXGI_DEVICE_SUB_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIObject::SetPrivateData on Surface
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on Surface
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on Surface
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k73.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGISwapChain4::SetHDRMetaData on SwapChain
    if wanted(IDXGI_SWAP_CHAIN4) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain4::set_hdr_meta_data(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_HDR_METADATA_TYPE_HDR10,
                Some(&[45_u8]),
            );
            true
        });
    }
    // IDXGISwapChain3::GetCurrentBackBufferIndex on SwapChain
    if wanted(IDXGI_SWAP_CHAIN3) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::SwapChain3::get_current_back_buffer_index(this);
            true
        });
    }
    // IDXGISwapChain3::CheckColorSpaceSupport on SwapChain
    if wanted(IDXGI_SWAP_CHAIN3) {
        driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::SwapChain3::check_color_space_support(this, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709);
        true
        });
    }
    // IDXGISwapChain3::SetColorSpace1 on SwapChain
    if wanted(IDXGI_SWAP_CHAIN3) {
        driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::SwapChain3::set_color_space1(this, directx_graphics_infrastructure::enumerations::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709);
        true
        });
    }
    // IDXGISwapChain2::SetSourceSize on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::SwapChain2::set_source_size(this, 120_u32, 44_u32);
            true
        });
    }
    // IDXGISwapChain2::GetSourceSize on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::get_source_size(this);
            true
        });
    }
    // IDXGISwapChain2::SetMaximumFrameLatency on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::set_maximum_frame_latency(
                this, 63_u32,
            );
            true
        });
    }
    // IDXGISwapChain2::GetMaximumFrameLatency on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::get_maximum_frame_latency(this);
            true
        });
    }
    // IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::get_frame_latency_waitable_object(
                this,
            );
            true
        });
    }
    // IDXGISwapChain2::SetMatrixTransform on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::set_matrix_transform(
                this,
                &directx_graphics_infrastructure::structures::Matrix3x2F {
                    _11: 3.5_f32,
                    _12: 10.5_f32,
                    _21: 17.5_f32,
                    _22: 24.5_f32,
                    _31: 31.5_f32,
                    _32: 38.5_f32,
                },
            );
            true
        });
    }
    // IDXGISwapChain2::GetMatrixTransform on SwapChain
    if wanted(IDXGI_SWAP_CHAIN2) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain2::get_matrix_transform(this);
            true
        });
    }
    // IDXGISwapChain1::GetDesc1 on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_desc1(this);
            true
        });
    }
    // IDXGISwapChain1::GetFullscreenDesc on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_fullscreen_desc(this);
            true
        });
    }
    // IDXGISwapChain1::GetHwnd on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_hwnd(this);
            true
        });
    }
    // IDXGISwapChain1::GetCoreWindow on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_core_window(
                this,
                &component_object_model::structures::Guid {
                    data1: 11_u32,
                    data2: 18_u16,
                    data3: 25_u16,
                    data4: core::array::from_fn(|_| 32_u8),
                },
            );
            true
        });
    }
    // IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::is_temporary_mono_supported(this);
            true
        });
    }
    // IDXGISwapChain1::GetRestrictToOutput on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_restrict_to_output(this);
            true
        });
    }
    // IDXGISwapChain1::SetBackgroundColor on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::set_background_color(
                this,
                &directx_graphics_infrastructure::structures::Rgba {
                    r: 79.5_f32,
                    g: 86.5_f32,
                    b: 93.5_f32,
                    a: 100.5_f32,
                },
            );
            true
        });
    }
    // IDXGISwapChain1::GetBackgroundColor on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_background_color(this);
            true
        });
    }
    // IDXGISwapChain1::SetRotation on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::set_rotation(
                this,
                directx_graphics_infrastructure::enumerations::DXGI_MODE_ROTATION_IDENTITY,
            );
            true
        });
    }
    // IDXGISwapChain1::GetRotation on SwapChain
    if wanted(IDXGI_SWAP_CHAIN1) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain1::get_rotation(this);
            true
        });
    }
    // IDXGISwapChain::Present on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::present(
                this,
                46_u32,
                directx_graphics_infrastructure::enumerations::DXGI_PRESENT_TEST,
            );
            true
        });
    }
    // IDXGISwapChain::GetBuffer on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_buffer(this, 0_u32);
            true
        });
    }
    // IDXGISwapChain::SetFullscreenState on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::set_fullscreen_state(
                this,
                true,
                o.k70.as_ref(),
            );
            true
        });
    }
    // IDXGISwapChain::GetFullscreenState on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_fullscreen_state(this);
            true
        });
    }
    // IDXGISwapChain::GetDesc on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_desc(this);
            true
        });
    }
    // IDXGISwapChain::ResizeBuffers on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::resize_buffers(
                this,
                107_u32,
                120_u32,
                44_u32,
                directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS,
                5_u32,
            );
            true
        });
    }
    // IDXGISwapChain::ResizeTarget on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
        let Some(this) = o.k74.as_ref() else { return false; };
        let _ = directx_graphics_infrastructure::SwapChain::resize_target(this, &directx_graphics_infrastructure::structures::ModeDesc { width: 118_u32, height: 125_u32, refresh_rate: directx_graphics_infrastructure::structures::Rational { numerator: 13_u32, denominator: 20_u32 }, format: directx_graphics_infrastructure::enumerations::DXGI_FORMAT_R32G32B32A32_TYPELESS, scanline_ordering: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, scaling: directx_graphics_infrastructure::enumerations::DXGI_MODE_SCALING_CENTERED });
        true
        });
    }
    // IDXGISwapChain::GetContainingOutput on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_containing_output(this);
            true
        });
    }
    // IDXGISwapChain::GetFrameStatistics on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_frame_statistics(this);
            true
        });
    }
    // IDXGISwapChain::GetLastPresentCount on SwapChain
    if wanted(IDXGI_SWAP_CHAIN) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChain::get_last_present_count(this);
            true
        });
    }
    // IDXGIDeviceSubObject::GetDevice on SwapChain
    if wanted(IDXGI_DEVICE_SUB_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::DeviceSubObject::get_device(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGIObject::SetPrivateData on SwapChain
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                &[115_u8],
            );
            true
        });
    }
    // IDXGIObject::SetPrivateDataInterface on SwapChain
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let v1 = if let Some(x) = o.k61.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Adapter(x))
            } else if let Some(x) = o.k62.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Debug(x))
            } else if let Some(x) = o.k63.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x))
            } else if let Some(x) = o.k64.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Device(x))
            } else if let Some(x) = o.k65.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                    x,
                ))
            } else if let Some(x) = o.k66.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Factory(x))
            } else if let Some(x) = o.k67.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x))
            } else if let Some(x) = o.k68.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(x))
            } else if let Some(x) = o.k69.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x))
            } else if let Some(x) = o.k70.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Output(x))
            } else if let Some(x) = o.k71.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x))
            } else if let Some(x) = o.k72.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Resource(x))
            } else if let Some(x) = o.k73.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::Surface(x))
            } else if let Some(x) = o.k74.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChain(x))
            } else if let Some(x) = o.k75.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                    x,
                ))
            } else if let Some(x) = o.k76.as_ref() {
                Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x))
            } else {
                None
            };
            let _ = directx_graphics_infrastructure::Object::set_private_data_interface(
                this,
                &component_object_model::structures::Guid {
                    data1: 5_u32,
                    data2: 12_u16,
                    data3: 19_u16,
                    data4: core::array::from_fn(|_| 26_u8),
                },
                v1,
            );
            true
        });
    }
    // IDXGIObject::GetParent on SwapChain
    if wanted(IDXGI_OBJECT) {
        driven.attempt(|| {
            let Some(this) = o.k74.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::Object::get_parent(
                this,
                &component_object_model::structures::Guid {
                    data1: 3_u32,
                    data2: 10_u16,
                    data3: 17_u16,
                    data4: core::array::from_fn(|_| 24_u8),
                },
            );
            true
        });
    }
    // IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia
    if wanted(IDXGI_SWAP_CHAIN_MEDIA) {
        driven.attempt(|| {
            let Some(this) = o.k75.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::SwapChainMedia::get_frame_statistics_media(this);
            true
        });
    }
    // IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia
    if wanted(IDXGI_SWAP_CHAIN_MEDIA) {
        driven.attempt(|| {
            let Some(this) = o.k75.as_ref() else {
                return false;
            };
            let _ =
                directx_graphics_infrastructure::SwapChainMedia::set_present_duration(this, 59_u32);
            true
        });
    }
    // IDXGISwapChainMedia::CheckPresentDurationSupport on SwapChainMedia
    if wanted(IDXGI_SWAP_CHAIN_MEDIA) {
        driven.attempt(|| {
            let Some(this) = o.k75.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::SwapChainMedia::check_present_duration_support(
                this, 101_u32,
            );
            true
        });
    }
    // IDXGraphicsAnalysis::BeginCapture on GraphicsAnalysis
    if wanted(IDX_GRAPHICS_ANALYSIS) {
        driven.attempt(|| {
            let Some(this) = o.k76.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::GraphicsAnalysis::begin_capture(this);
            true
        });
    }
    // IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis
    if wanted(IDX_GRAPHICS_ANALYSIS) {
        driven.attempt(|| {
            let Some(this) = o.k76.as_ref() else {
                return false;
            };
            let _ = directx_graphics_infrastructure::GraphicsAnalysis::end_capture(this);
            true
        });
    }
    // CreateDXGIFactory
    if wanted(CREATE_DXGI_FACTORY) {
        driven.attempt(|| {
            let _ =
                directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory(
                    &o.api,
                    &component_object_model::structures::Guid {
                        data1: 3_u32,
                        data2: 10_u16,
                        data3: 17_u16,
                        data4: core::array::from_fn(|_| 24_u8),
                    },
                );
            true
        });
    }
    // CreateDXGIFactory1
    if wanted(CREATE_DXGI_FACTORY1) {
        driven.attempt(|| {
        let _ = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory1(&o.api, &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) });
        true
        });
    }
    // CreateDXGIFactory2
    if wanted(CREATE_DXGI_FACTORY2) {
        driven.attempt(|| {
        let _ = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(0), &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) });
        true
        });
    }
    // DXGIDeclareAdapterRemovalSupport
    if wanted(DXGI_DECLARE_ADAPTER_REMOVAL_SUPPORT) {
        driven.attempt(|| {
        let _ = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_declare_adapter_removal_support(&o.api);
        true
        });
    }
    // DXGIDisableVBlankVirtualization
    if wanted(DXGI_DISABLE_V_BLANK_VIRTUALIZATION) {
        driven.attempt(|| {
        let _ = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_disable_v_blank_virtualization(&o.api);
        true
        });
    }
    // DXGIGetDebugInterface1
    if wanted(DXGI_GET_DEBUG_INTERFACE1) {
        driven.attempt(|| {
        let _ = directx_graphics_infrastructure::DirectXGraphicsInfrastructure::dxgi_get_debug_interface1(&o.api, 31_u32, &component_object_model::structures::Guid { data1: 3_u32, data2: 10_u16, data3: 17_u16, data4: core::array::from_fn(|_| 24_u8) });
        true
        });
    }
    driven
}

/// The interface (or export) `CreateDXGIFactory`, by its name in the header.
const CREATE_DXGI_FACTORY: &str = "CreateDXGIFactory";
/// The interface (or export) `CreateDXGIFactory1`, by its name in the header.
const CREATE_DXGI_FACTORY1: &str = "CreateDXGIFactory1";
/// The interface (or export) `CreateDXGIFactory2`, by its name in the header.
const CREATE_DXGI_FACTORY2: &str = "CreateDXGIFactory2";
/// The interface (or export) `DXGIDeclareAdapterRemovalSupport`, by its name in the header.
const DXGI_DECLARE_ADAPTER_REMOVAL_SUPPORT: &str = "DXGIDeclareAdapterRemovalSupport";
/// The interface (or export) `DXGIDisableVBlankVirtualization`, by its name in the header.
const DXGI_DISABLE_V_BLANK_VIRTUALIZATION: &str = "DXGIDisableVBlankVirtualization";
/// The interface (or export) `DXGIGetDebugInterface1`, by its name in the header.
const DXGI_GET_DEBUG_INTERFACE1: &str = "DXGIGetDebugInterface1";
/// The interface (or export) `IDXGIAdapter`, by its name in the header.
const IDXGI_ADAPTER: &str = "IDXGIAdapter";
/// The interface (or export) `IDXGIAdapter1`, by its name in the header.
const IDXGI_ADAPTER1: &str = "IDXGIAdapter1";
/// The interface (or export) `IDXGIAdapter2`, by its name in the header.
const IDXGI_ADAPTER2: &str = "IDXGIAdapter2";
/// The interface (or export) `IDXGIAdapter3`, by its name in the header.
const IDXGI_ADAPTER3: &str = "IDXGIAdapter3";
/// The interface (or export) `IDXGIAdapter4`, by its name in the header.
const IDXGI_ADAPTER4: &str = "IDXGIAdapter4";
/// The interface (or export) `IDXGIDebug`, by its name in the header.
const IDXGI_DEBUG: &str = "IDXGIDebug";
/// The interface (or export) `IDXGIDebug1`, by its name in the header.
const IDXGI_DEBUG1: &str = "IDXGIDebug1";
/// The interface (or export) `IDXGIDecodeSwapChain`, by its name in the header.
const IDXGI_DECODE_SWAP_CHAIN: &str = "IDXGIDecodeSwapChain";
/// The interface (or export) `IDXGIDevice`, by its name in the header.
const IDXGI_DEVICE: &str = "IDXGIDevice";
/// The interface (or export) `IDXGIDevice1`, by its name in the header.
const IDXGI_DEVICE1: &str = "IDXGIDevice1";
/// The interface (or export) `IDXGIDevice2`, by its name in the header.
const IDXGI_DEVICE2: &str = "IDXGIDevice2";
/// The interface (or export) `IDXGIDevice3`, by its name in the header.
const IDXGI_DEVICE3: &str = "IDXGIDevice3";
/// The interface (or export) `IDXGIDevice4`, by its name in the header.
const IDXGI_DEVICE4: &str = "IDXGIDevice4";
/// The interface (or export) `IDXGIDeviceSubObject`, by its name in the header.
const IDXGI_DEVICE_SUB_OBJECT: &str = "IDXGIDeviceSubObject";
/// The interface (or export) `IDXGIDisplayControl`, by its name in the header.
const IDXGI_DISPLAY_CONTROL: &str = "IDXGIDisplayControl";
/// The interface (or export) `IDXGIFactory`, by its name in the header.
const IDXGI_FACTORY: &str = "IDXGIFactory";
/// The interface (or export) `IDXGIFactory1`, by its name in the header.
const IDXGI_FACTORY1: &str = "IDXGIFactory1";
/// The interface (or export) `IDXGIFactory2`, by its name in the header.
const IDXGI_FACTORY2: &str = "IDXGIFactory2";
/// The interface (or export) `IDXGIFactory3`, by its name in the header.
const IDXGI_FACTORY3: &str = "IDXGIFactory3";
/// The interface (or export) `IDXGIFactory4`, by its name in the header.
const IDXGI_FACTORY4: &str = "IDXGIFactory4";
/// The interface (or export) `IDXGIFactory5`, by its name in the header.
const IDXGI_FACTORY5: &str = "IDXGIFactory5";
/// The interface (or export) `IDXGIFactory6`, by its name in the header.
const IDXGI_FACTORY6: &str = "IDXGIFactory6";
/// The interface (or export) `IDXGIFactory7`, by its name in the header.
const IDXGI_FACTORY7: &str = "IDXGIFactory7";
/// The interface (or export) `IDXGIFactoryMedia`, by its name in the header.
const IDXGI_FACTORY_MEDIA: &str = "IDXGIFactoryMedia";
/// The interface (or export) `IDXGIInfoQueue`, by its name in the header.
const IDXGI_INFO_QUEUE: &str = "IDXGIInfoQueue";
/// The interface (or export) `IDXGIKeyedMutex`, by its name in the header.
const IDXGI_KEYED_MUTEX: &str = "IDXGIKeyedMutex";
/// The interface (or export) `IDXGIObject`, by its name in the header.
const IDXGI_OBJECT: &str = "IDXGIObject";
/// The interface (or export) `IDXGIOutput`, by its name in the header.
const IDXGI_OUTPUT: &str = "IDXGIOutput";
/// The interface (or export) `IDXGIOutput1`, by its name in the header.
const IDXGI_OUTPUT1: &str = "IDXGIOutput1";
/// The interface (or export) `IDXGIOutput2`, by its name in the header.
const IDXGI_OUTPUT2: &str = "IDXGIOutput2";
/// The interface (or export) `IDXGIOutput3`, by its name in the header.
const IDXGI_OUTPUT3: &str = "IDXGIOutput3";
/// The interface (or export) `IDXGIOutput4`, by its name in the header.
const IDXGI_OUTPUT4: &str = "IDXGIOutput4";
/// The interface (or export) `IDXGIOutput5`, by its name in the header.
const IDXGI_OUTPUT5: &str = "IDXGIOutput5";
/// The interface (or export) `IDXGIOutput6`, by its name in the header.
const IDXGI_OUTPUT6: &str = "IDXGIOutput6";
/// The interface (or export) `IDXGIOutputDuplication`, by its name in the header.
const IDXGI_OUTPUT_DUPLICATION: &str = "IDXGIOutputDuplication";
/// The interface (or export) `IDXGIResource`, by its name in the header.
const IDXGI_RESOURCE: &str = "IDXGIResource";
/// The interface (or export) `IDXGIResource1`, by its name in the header.
const IDXGI_RESOURCE1: &str = "IDXGIResource1";
/// The interface (or export) `IDXGISurface`, by its name in the header.
const IDXGI_SURFACE: &str = "IDXGISurface";
/// The interface (or export) `IDXGISurface1`, by its name in the header.
const IDXGI_SURFACE1: &str = "IDXGISurface1";
/// The interface (or export) `IDXGISurface2`, by its name in the header.
const IDXGI_SURFACE2: &str = "IDXGISurface2";
/// The interface (or export) `IDXGISwapChain`, by its name in the header.
const IDXGI_SWAP_CHAIN: &str = "IDXGISwapChain";
/// The interface (or export) `IDXGISwapChain1`, by its name in the header.
const IDXGI_SWAP_CHAIN1: &str = "IDXGISwapChain1";
/// The interface (or export) `IDXGISwapChain2`, by its name in the header.
const IDXGI_SWAP_CHAIN2: &str = "IDXGISwapChain2";
/// The interface (or export) `IDXGISwapChain3`, by its name in the header.
const IDXGI_SWAP_CHAIN3: &str = "IDXGISwapChain3";
/// The interface (or export) `IDXGISwapChain4`, by its name in the header.
const IDXGI_SWAP_CHAIN4: &str = "IDXGISwapChain4";
/// The interface (or export) `IDXGISwapChainMedia`, by its name in the header.
const IDXGI_SWAP_CHAIN_MEDIA: &str = "IDXGISwapChainMedia";
/// The interface (or export) `IDXGraphicsAnalysis`, by its name in the header.
const IDX_GRAPHICS_ANALYSIS: &str = "IDXGraphicsAnalysis";
/// Each effects judgment of the APIs in reach (src/api.rs `effects`): the interfaces (or exports) judged together, and
/// the judgment.
pub const EFFECTS: &[(&[&str], &str)] = &[
    (
        &[
            "IDXGIObject",
            "IDXGIDeviceSubObject",
            "IDXGIResource",
            "IDXGIResource1",
            "IDXGIKeyedMutex",
        ],
        "each method describes, names, compares or holds its own object, or stores private data the system copies (an interface stored is held by the system's own reference): none ends memory lent, changes how threads may call, raises an exception or runs code of ours",
    ),
    (
        &["IDXGISurface", "IDXGISurface1", "IDXGISurface2"],
        "a texture's surface: its description and its resource; Map, Unmap, GetDC and ReleaseDC are never called (rows)",
    ),
    (
        &[
            "IDXGIAdapter",
            "IDXGIAdapter1",
            "IDXGIAdapter2",
            "IDXGIAdapter3",
            "IDXGIAdapter4",
        ],
        "an adapter's description, outputs, memory budget and events (handles the kernel checks)",
    ),
    (
        &[
            "IDXGIOutput",
            "IDXGIOutput1",
            "IDXGIOutput2",
            "IDXGIOutput3",
            "IDXGIOutput4",
            "IDXGIOutput5",
            "IDXGIOutput6",
        ],
        "a monitor's modes, gamma, ownership for full screen and duplication: state of the display, no memory or code of ours",
    ),
    (
        &["IDXGIOutputDuplication"],
        "the desktop's frames, as objects; mapping the desktop's memory is never called (rows)",
    ),
    (
        &[
            "IDXGIDevice",
            "IDXGIDevice1",
            "IDXGIDevice2",
            "IDXGIDevice3",
            "IDXGIDevice4",
        ],
        "a device's adapter, priorities, frame latency and residency; offering and reclaiming are never called (rows); Trim frees only the driver's own allocations",
    ),
    (
        &[
            "IDXGIFactory",
            "IDXGIFactory1",
            "IDXGIFactory2",
            "IDXGIFactory3",
            "IDXGIFactory4",
            "IDXGIFactory5",
            "IDXGIFactory6",
            "IDXGIFactory7",
            "IDXGIFactoryMedia",
        ],
        "makes adapters and swap chains and watches windows and events (handles the kernel checks); a software adapter's module is a kind the system trusts (refused by default)",
    ),
    (
        &[
            "IDXGISwapChain",
            "IDXGISwapChain1",
            "IDXGISwapChain2",
            "IDXGISwapChain3",
            "IDXGISwapChain4",
            "IDXGISwapChainMedia",
            "IDXGIDecodeSwapChain",
        ],
        "presents and resizes: ResizeBuffers fails while any reference to a buffer is held, and lent memory holds its resource; the waitable object is an event handle the kernel checks",
    ),
    (
        &["IDXGIDisplayControl", "IDXGraphicsAnalysis"],
        "whether stereo is on, and a tool's capture: state outside our memory",
    ),
    (
        &["IDXGIDebug", "IDXGIDebug1"],
        "reports live objects and tracks leaks: it changes no count and runs nothing of ours",
    ),
    (
        &["IDXGIInfoQueue"],
        "stores, filters and reports messages; turning a break on is refused (rows)",
    ),
    (
        &[
            "CreateDXGIFactory",
            "CreateDXGIFactory1",
            "CreateDXGIFactory2",
            "DXGIDeclareAdapterRemovalSupport",
            "DXGIDisableVBlankVirtualization",
            "DXGIGetDebugInterface1",
        ],
        "make a factory or a debug interface, or set a process-wide option of display behavior; the debug layer is refused (a special value)",
    ),
];

/// The methods the rows keep from the system (never called): none reaches an implementation.
pub const NEVER_REACHED: &[&str] = &[
    "IDXGIDevice2::OfferResources",
    "IDXGIDevice2::ReclaimResources",
    "IDXGIDevice4::OfferResources1",
    "IDXGIDevice4::ReclaimResources1",
    "IDXGIOutputDuplication::MapDesktopSurface",
    "IDXGIOutputDuplication::UnMapDesktopSurface",
    "IDXGISurface::Map",
    "IDXGISurface::Unmap",
    "IDXGISurface1::GetDC",
    "IDXGISurface1::ReleaseDC",
];

/// IDXGIAdapter4::GetDesc3 on Adapter, of random arguments.
fn fuzz_0(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter4>::get_desc3(this);
    true
}

/// IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent on Adapter, of random arguments.
fn fuzz_1(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::register_hardware_content_protection_teardown_status_event(this, Default::default());
    true
}

/// IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus on Adapter, of random arguments.
fn fuzz_2(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::unregister_hardware_content_protection_teardown_status(this, r.value::<u32>());
    true
}

/// IDXGIAdapter3::QueryVideoMemoryInfo on Adapter, of random arguments.
fn fuzz_3(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::query_video_memory_info(this, r.value::<u32>(), directx_graphics_infrastructure::enumerations::MemorySegmentGroup(r.value()));
    true
}

/// IDXGIAdapter3::SetVideoMemoryReservation on Adapter, of random arguments.
fn fuzz_4(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::set_video_memory_reservation(this, r.value::<u32>(), directx_graphics_infrastructure::enumerations::MemorySegmentGroup(r.value()), r.value::<u64>());
    true
}

/// IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent on Adapter, of random arguments.
fn fuzz_5(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::register_video_memory_budget_change_notification_event(this, Default::default());
    true
}

/// IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification on Adapter, of random arguments.
fn fuzz_6(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter3>::unregister_video_memory_budget_change_notification(this, r.value::<u32>());
    true
}

/// IDXGIAdapter2::GetDesc2 on Adapter, of random arguments.
fn fuzz_7(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter2>::get_desc2(this);
    true
}

/// IDXGIAdapter1::GetDesc1 on Adapter, of random arguments.
fn fuzz_8(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter1>::get_desc1(this);
    true
}

/// IDXGIAdapter::EnumOutputs on Adapter, of random arguments.
fn fuzz_9(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter>::enum_outputs(this, r.value::<u32>());
    true
}

/// IDXGIAdapter::GetDesc on Adapter, of random arguments.
fn fuzz_10(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter>::get_desc(this);
    true
}

/// IDXGIAdapter::CheckInterfaceSupport on Adapter, of random arguments.
fn fuzz_11(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Adapter>::check_interface_support(this, &Default::default());
    true
}

/// IDXGIObject::SetPrivateData on Adapter, of random arguments.
fn fuzz_12(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Adapter, of random arguments.
fn fuzz_13(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Adapter, of random arguments.
fn fuzz_14(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k61.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemAdapter<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIDebug1::EnableLeakTrackingForThread on Debug, of random arguments.
fn fuzz_15(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k62.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDebug<Api> as directx_graphics_infrastructure::Debug1>::enable_leak_tracking_for_thread(this);
    true
}

/// IDXGIDebug1::DisableLeakTrackingForThread on Debug, of random arguments.
fn fuzz_16(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k62.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDebug<Api> as directx_graphics_infrastructure::Debug1>::disable_leak_tracking_for_thread(this);
    true
}

/// IDXGIDebug1::IsLeakTrackingEnabledForThread on Debug, of random arguments.
fn fuzz_17(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k62.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDebug<Api> as directx_graphics_infrastructure::Debug1>::is_leak_tracking_enabled_for_thread(this);
    true
}

/// IDXGIDebug::ReportLiveObjects on Debug, of random arguments.
fn fuzz_18(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k62.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDebug<Api> as directx_graphics_infrastructure::Debug>::report_live_objects(this, Default::default(), directx_graphics_infrastructure::enumerations::DebugRloFlags(r.value()));
    true
}

/// IDXGIDecodeSwapChain::PresentBuffer on DecodeSwapChain, of random arguments.
fn fuzz_19(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::present_buffer(this, r.value::<u32>(), r.value::<u32>(), directx_graphics_infrastructure::enumerations::Present(r.value()));
    true
}

/// IDXGIDecodeSwapChain::SetSourceRect on DecodeSwapChain, of random arguments.
fn fuzz_20(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::set_source_rect(this, &data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(&r.bytes(16)));
    true
}

/// IDXGIDecodeSwapChain::SetTargetRect on DecodeSwapChain, of random arguments.
fn fuzz_21(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::set_target_rect(this, &data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(&r.bytes(16)));
    true
}

/// IDXGIDecodeSwapChain::SetDestSize on DecodeSwapChain, of random arguments.
fn fuzz_22(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::set_dest_size(this, r.value::<u32>(), r.value::<u32>());
    true
}

/// IDXGIDecodeSwapChain::GetSourceRect on DecodeSwapChain, of random arguments.
fn fuzz_23(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::get_source_rect(this);
    true
}

/// IDXGIDecodeSwapChain::GetTargetRect on DecodeSwapChain, of random arguments.
fn fuzz_24(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::get_target_rect(this);
    true
}

/// IDXGIDecodeSwapChain::GetDestSize on DecodeSwapChain, of random arguments.
fn fuzz_25(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::get_dest_size(this);
    true
}

/// IDXGIDecodeSwapChain::SetColorSpace on DecodeSwapChain, of random arguments.
fn fuzz_26(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::set_color_space(this, directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags(r.value()));
    true
}

/// IDXGIDecodeSwapChain::GetColorSpace on DecodeSwapChain, of random arguments.
fn fuzz_27(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k63.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDecodeSwapChain<Api> as directx_graphics_infrastructure::DecodeSwapChain>::get_color_space(this);
    true
}

/// IDXGIDevice4::OfferResources1 on Device, of random arguments.
fn fuzz_28(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count)
        .map(|_| o.k72.as_ref().filter(|_| r.coin()))
        .collect();
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device4>::offer_resources1(this, s1.as_slice(), directx_graphics_infrastructure::enumerations::OfferResourcePriority(r.value()), r.value::<u32>());
    true
}

/// IDXGIDevice4::ReclaimResources1 on Device, of random arguments.
fn fuzz_29(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count)
        .map(|_| o.k72.as_ref().filter(|_| r.coin()))
        .collect();
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device4>::reclaim_resources1(this, s1.as_slice());
    true
}

/// IDXGIDevice3::Trim on Device, of random arguments.
fn fuzz_30(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device3>::trim(this);
    true
}

/// IDXGIDevice2::OfferResources on Device, of random arguments.
fn fuzz_31(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count)
        .map(|_| o.k72.as_ref().filter(|_| r.coin()))
        .collect();
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device2>::offer_resources(this, s1.as_slice(), directx_graphics_infrastructure::enumerations::OfferResourcePriority(r.value()));
    true
}

/// IDXGIDevice2::ReclaimResources on Device, of random arguments.
fn fuzz_32(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count)
        .map(|_| o.k72.as_ref().filter(|_| r.coin()))
        .collect();
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device2>::reclaim_resources(this, s1.as_slice());
    true
}

/// IDXGIDevice2::EnqueueSetEvent on Device, of random arguments.
fn fuzz_33(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device2>::enqueue_set_event(this, Default::default());
    true
}

/// IDXGIDevice1::SetMaximumFrameLatency on Device, of random arguments.
fn fuzz_34(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device1>::set_maximum_frame_latency(this, r.value::<u32>());
    true
}

/// IDXGIDevice1::GetMaximumFrameLatency on Device, of random arguments.
fn fuzz_35(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device1>::get_maximum_frame_latency(this);
    true
}

/// IDXGIDevice::GetAdapter on Device, of random arguments.
fn fuzz_36(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device>::get_adapter(this);
    true
}

/// IDXGIDevice::CreateSurface on Device, of random arguments.
fn fuzz_37(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = if r.coin() {
        Some(&data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_shared_resource(&r.bytes(8)))
    } else {
        None
    };
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device>::create_surface(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_surface_desc(&r.bytes(20)), r.value::<u32>(), directx_graphics_infrastructure::enumerations::Usage(r.value()), v1);
    true
}

/// IDXGIDevice::SetGPUThreadPriority on Device, of random arguments.
fn fuzz_38(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device>::set_gpu_thread_priority(this, r.value::<i32>());
    true
}

/// IDXGIDevice::GetGPUThreadPriority on Device, of random arguments.
fn fuzz_39(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Device>::get_gpu_thread_priority(this);
    true
}

/// IDXGIObject::SetPrivateData on Device, of random arguments.
fn fuzz_40(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Device, of random arguments.
fn fuzz_41(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Device, of random arguments.
fn fuzz_42(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k64.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDevice<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIDisplayControl::IsStereoEnabled on DisplayControl, of random arguments.
fn fuzz_43(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k65.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDisplayControl<Api> as directx_graphics_infrastructure::DisplayControl>::is_stereo_enabled(this);
    true
}

/// IDXGIDisplayControl::SetStereoEnabled on DisplayControl, of random arguments.
fn fuzz_44(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k65.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemDisplayControl<Api> as directx_graphics_infrastructure::DisplayControl>::set_stereo_enabled(this, r.value::<bool>());
    true
}

/// IDXGIFactory7::RegisterAdaptersChangedEvent on Factory, of random arguments.
fn fuzz_45(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory7>::register_adapters_changed_event(this, Default::default());
    true
}

/// IDXGIFactory7::UnregisterAdaptersChangedEvent on Factory, of random arguments.
fn fuzz_46(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory7>::unregister_adapters_changed_event(this, r.value::<u32>());
    true
}

/// IDXGIFactory6::EnumAdapterByGpuPreference on Factory, of random arguments.
fn fuzz_47(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory6>::enum_adapter_by_gpu_preference(this, r.value::<u32>(), directx_graphics_infrastructure::enumerations::GpuPreference(r.value()), &Default::default());
    true
}

/// IDXGIFactory5::CheckFeatureSupport on Factory, of random arguments.
fn fuzz_48(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let b1 = random_bytes(r);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory5>::check_feature_support(this, directx_graphics_infrastructure::enumerations::Feature(r.value()), &b1);
    true
}

/// IDXGIFactory4::EnumWarpAdapter on Factory, of random arguments.
fn fuzz_49(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory4>::enum_warp_adapter(this, &Default::default());
    true
}

/// IDXGIFactory3::GetCreationFlags on Factory, of random arguments.
fn fuzz_50(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory3>::get_creation_flags(this);
    true
}

/// IDXGIFactory2::IsWindowedStereoEnabled on Factory, of random arguments.
fn fuzz_51(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::is_windowed_stereo_enabled(this);
    true
}

/// IDXGIFactory2::CreateSwapChainForHwnd on Factory, of random arguments.
fn fuzz_52(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let v2 = if r.coin() {
        Some(&data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(&r.bytes(20)))
    } else {
        None
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::create_swap_chain_for_hwnd(this, v1, Default::default(), &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&r.bytes(48)), v2, o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGIFactory2::CreateSwapChainForCoreWindow on Factory, of random arguments.
fn fuzz_53(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let v2 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::create_swap_chain_for_core_window(this, v1, v2, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&r.bytes(48)), o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGIFactory2::GetSharedResourceAdapterLuid on Factory, of random arguments.
fn fuzz_54(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::get_shared_resource_adapter_luid(this, Default::default());
    true
}

/// IDXGIFactory2::RegisterStereoStatusWindow on Factory, of random arguments.
fn fuzz_55(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::register_stereo_status_window(this, Default::default(), r.value::<u32>());
    true
}

/// IDXGIFactory2::RegisterStereoStatusEvent on Factory, of random arguments.
fn fuzz_56(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::register_stereo_status_event(this, Default::default());
    true
}

/// IDXGIFactory2::UnregisterStereoStatus on Factory, of random arguments.
fn fuzz_57(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::unregister_stereo_status(this, r.value::<u32>());
    true
}

/// IDXGIFactory2::RegisterOcclusionStatusWindow on Factory, of random arguments.
fn fuzz_58(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::register_occlusion_status_window(this, Default::default(), r.value::<u32>());
    true
}

/// IDXGIFactory2::RegisterOcclusionStatusEvent on Factory, of random arguments.
fn fuzz_59(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::register_occlusion_status_event(this, Default::default());
    true
}

/// IDXGIFactory2::UnregisterOcclusionStatus on Factory, of random arguments.
fn fuzz_60(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::unregister_occlusion_status(this, r.value::<u32>());
    true
}

/// IDXGIFactory2::CreateSwapChainForComposition on Factory, of random arguments.
fn fuzz_61(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory2>::create_swap_chain_for_composition(this, v1, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&r.bytes(48)), o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGIFactory1::EnumAdapters1 on Factory, of random arguments.
fn fuzz_62(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory1>::enum_adapters1(this, r.value::<u32>());
    true
}

/// IDXGIFactory1::IsCurrent on Factory, of random arguments.
fn fuzz_63(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory1>::is_current(this);
    true
}

/// IDXGIFactory::EnumAdapters on Factory, of random arguments.
fn fuzz_64(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory>::enum_adapters(this, r.value::<u32>());
    true
}

/// IDXGIFactory::MakeWindowAssociation on Factory, of random arguments.
fn fuzz_65(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory>::make_window_association(this, Default::default(), directx_graphics_infrastructure::enumerations::MwaFlags(r.value()));
    true
}

/// IDXGIFactory::GetWindowAssociation on Factory, of random arguments.
fn fuzz_66(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory>::get_window_association(this);
    true
}

/// IDXGIFactory::CreateSwapChain on Factory, of random arguments.
fn fuzz_67(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Factory>::create_swap_chain(this, v1, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(&r.bytes(72)));
    true
}

/// IDXGIObject::SetPrivateData on Factory, of random arguments.
fn fuzz_68(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Factory, of random arguments.
fn fuzz_69(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Factory, of random arguments.
fn fuzz_70(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k66.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemFactory<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle on FactoryMedia, of random arguments.
fn fuzz_71(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k67.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactoryMedia<Api> as directx_graphics_infrastructure::FactoryMedia>::create_swap_chain_for_composition_surface_handle(this, v1, Default::default(), &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&r.bytes(48)), o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle on FactoryMedia, of random arguments.
fn fuzz_72(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k67.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let Some(v2) = o.k72.as_ref() else {
        return false;
    };
    let _ = <directx_graphics_infrastructure_direct::SystemFactoryMedia<Api> as directx_graphics_infrastructure::FactoryMedia>::create_decode_swap_chain_for_composition_surface_handle(this, v1, Default::default(), &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_decode_swap_chain_desc(&r.bytes(4)), v2, o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGIInfoQueue::SetMessageCountLimit on InfoQueue, of random arguments.
fn fuzz_73(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::set_message_count_limit(this, Default::default(), r.value::<u64>());
    true
}

/// IDXGIInfoQueue::ClearStoredMessages on InfoQueue, of random arguments.
fn fuzz_74(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::clear_stored_messages(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters on InfoQueue, of random arguments.
fn fuzz_75(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_num_stored_messages_allowed_by_retrieval_filters(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetNumStoredMessages on InfoQueue, of random arguments.
fn fuzz_76(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_num_stored_messages(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit on InfoQueue, of random arguments.
fn fuzz_77(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_num_messages_discarded_by_message_count_limit(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetMessageCountLimit on InfoQueue, of random arguments.
fn fuzz_78(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_message_count_limit(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter on InfoQueue, of random arguments.
fn fuzz_79(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_num_messages_allowed_by_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter on InfoQueue, of random arguments.
fn fuzz_80(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_num_messages_denied_by_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::ClearStorageFilter on InfoQueue, of random arguments.
fn fuzz_81(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::clear_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushEmptyStorageFilter on InfoQueue, of random arguments.
fn fuzz_82(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_empty_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushDenyAllStorageFilter on InfoQueue, of random arguments.
fn fuzz_83(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_deny_all_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushCopyOfStorageFilter on InfoQueue, of random arguments.
fn fuzz_84(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_copy_of_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PopStorageFilter on InfoQueue, of random arguments.
fn fuzz_85(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::pop_storage_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetStorageFilterStackSize on InfoQueue, of random arguments.
fn fuzz_86(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_storage_filter_stack_size(this, Default::default());
    true
}

/// IDXGIInfoQueue::ClearRetrievalFilter on InfoQueue, of random arguments.
fn fuzz_87(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::clear_retrieval_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushEmptyRetrievalFilter on InfoQueue, of random arguments.
fn fuzz_88(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_empty_retrieval_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushDenyAllRetrievalFilter on InfoQueue, of random arguments.
fn fuzz_89(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_deny_all_retrieval_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PushCopyOfRetrievalFilter on InfoQueue, of random arguments.
fn fuzz_90(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::push_copy_of_retrieval_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::PopRetrievalFilter on InfoQueue, of random arguments.
fn fuzz_91(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::pop_retrieval_filter(this, Default::default());
    true
}

/// IDXGIInfoQueue::GetRetrievalFilterStackSize on InfoQueue, of random arguments.
fn fuzz_92(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_retrieval_filter_stack_size(this, Default::default());
    true
}

/// IDXGIInfoQueue::AddMessage on InfoQueue, of random arguments.
fn fuzz_93(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::add_message(this, Default::default(), directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(r.value()), directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(r.value()), r.value::<i32>(), c"");
    true
}

/// IDXGIInfoQueue::AddApplicationMessage on InfoQueue, of random arguments.
fn fuzz_94(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::add_application_message(this, directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(r.value()), c"");
    true
}

/// IDXGIInfoQueue::SetBreakOnCategory on InfoQueue, of random arguments.
fn fuzz_95(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::set_break_on_category(this, Default::default(), directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(r.value()), r.value::<bool>());
    true
}

/// IDXGIInfoQueue::SetBreakOnSeverity on InfoQueue, of random arguments.
fn fuzz_96(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::set_break_on_severity(this, Default::default(), directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(r.value()), r.value::<bool>());
    true
}

/// IDXGIInfoQueue::SetBreakOnID on InfoQueue, of random arguments.
fn fuzz_97(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::set_break_on_id(this, Default::default(), r.value::<i32>(), r.value::<bool>());
    true
}

/// IDXGIInfoQueue::GetBreakOnCategory on InfoQueue, of random arguments.
fn fuzz_98(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_break_on_category(this, Default::default(), directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(r.value()));
    true
}

/// IDXGIInfoQueue::GetBreakOnSeverity on InfoQueue, of random arguments.
fn fuzz_99(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_break_on_severity(this, Default::default(), directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(r.value()));
    true
}

/// IDXGIInfoQueue::GetBreakOnID on InfoQueue, of random arguments.
fn fuzz_100(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_break_on_id(this, Default::default(), r.value::<i32>());
    true
}

/// IDXGIInfoQueue::SetMuteDebugOutput on InfoQueue, of random arguments.
fn fuzz_101(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::set_mute_debug_output(this, Default::default(), r.value::<bool>());
    true
}

/// IDXGIInfoQueue::GetMuteDebugOutput on InfoQueue, of random arguments.
fn fuzz_102(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k68.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemInfoQueue<Api> as directx_graphics_infrastructure::InfoQueue>::get_mute_debug_output(this, Default::default());
    true
}

/// IDXGIKeyedMutex::AcquireSync on KeyedMutex, of random arguments.
fn fuzz_103(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::KeyedMutex>::acquire_sync(this, r.value::<u64>(), r.value::<u32>());
    true
}

/// IDXGIKeyedMutex::ReleaseSync on KeyedMutex, of random arguments.
fn fuzz_104(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::KeyedMutex>::release_sync(this, r.value::<u64>());
    true
}

/// IDXGIDeviceSubObject::GetDevice on KeyedMutex, of random arguments.
fn fuzz_105(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::DeviceSubObject>::get_device(this, &Default::default());
    true
}

/// IDXGIObject::SetPrivateData on KeyedMutex, of random arguments.
fn fuzz_106(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on KeyedMutex, of random arguments.
fn fuzz_107(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on KeyedMutex, of random arguments.
fn fuzz_108(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k69.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemKeyedMutex<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIOutput6::GetDesc1 on Output, of random arguments.
fn fuzz_109(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output6>::get_desc1(this);
    true
}

/// IDXGIOutput6::CheckHardwareCompositionSupport on Output, of random arguments.
fn fuzz_110(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output6>::check_hardware_composition_support(this);
    true
}

/// IDXGIOutput5::DuplicateOutput1 on Output, of random arguments.
fn fuzz_111(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let s2: Vec<_> = (0..count)
        .map(|_| directx_graphics_infrastructure::enumerations::Format(r.value()))
        .collect();
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output5>::duplicate_output1(this, v1, r.value::<u32>(), s2.as_slice());
    true
}

/// IDXGIOutput4::CheckOverlayColorSpaceSupport on Output, of random arguments.
fn fuzz_112(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output4>::check_overlay_color_space_support(this, directx_graphics_infrastructure::enumerations::Format(r.value()), directx_graphics_infrastructure::enumerations::ColorSpaceType(r.value()), v1);
    true
}

/// IDXGIOutput3::CheckOverlaySupport on Output, of random arguments.
fn fuzz_113(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output3>::check_overlay_support(this, directx_graphics_infrastructure::enumerations::Format(r.value()), v1);
    true
}

/// IDXGIOutput2::SupportsOverlays on Output, of random arguments.
fn fuzz_114(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output2>::supports_overlays(this);
    true
}

/// IDXGIOutput1::GetDisplayModeList1 on Output, of random arguments.
fn fuzz_115(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output1>::get_display_mode_list1(this, directx_graphics_infrastructure::enumerations::Format(r.value()), directx_graphics_infrastructure::enumerations::EnumModes(r.value()));
    true
}

/// IDXGIOutput1::FindClosestMatchingMode1 on Output, of random arguments.
fn fuzz_116(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output1>::find_closest_matching_mode1(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_mode_desc1(&r.bytes(32)), None);
    true
}

/// IDXGIOutput1::GetDisplaySurfaceData1 on Output, of random arguments.
fn fuzz_117(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k72.as_ref() else {
        return false;
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output1>::get_display_surface_data1(this, v1);
    true
}

/// IDXGIOutput1::DuplicateOutput on Output, of random arguments.
fn fuzz_118(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output1>::duplicate_output(this, v1);
    true
}

/// IDXGIOutput::GetDesc on Output, of random arguments.
fn fuzz_119(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_desc(this);
    true
}

/// IDXGIOutput::GetDisplayModeList on Output, of random arguments.
fn fuzz_120(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_display_mode_list(this, directx_graphics_infrastructure::enumerations::Format(r.value()), directx_graphics_infrastructure::enumerations::EnumModes(r.value()));
    true
}

/// IDXGIOutput::FindClosestMatchingMode on Output, of random arguments.
fn fuzz_121(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::find_closest_matching_mode(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(&r.bytes(28)), None);
    true
}

/// IDXGIOutput::WaitForVBlank on Output, of random arguments.
fn fuzz_122(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::wait_for_v_blank(this);
    true
}

/// IDXGIOutput::TakeOwnership on Output, of random arguments.
fn fuzz_123(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = {
        let mut choices = Vec::new();
        if let Some(x) = o.k61.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Adapter(x));
        }
        if let Some(x) = o.k62.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Debug(x));
        }
        if let Some(x) = o.k63.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(x));
        }
        if let Some(x) = o.k64.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Device(x));
        }
        if let Some(x) = o.k65.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::DisplayControl(
                x,
            ));
        }
        if let Some(x) = o.k66.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Factory(x));
        }
        if let Some(x) = o.k67.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::FactoryMedia(x));
        }
        if let Some(x) = o.k68.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::InfoQueue(x));
        }
        if let Some(x) = o.k69.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::KeyedMutex(x));
        }
        if let Some(x) = o.k70.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Output(x));
        }
        if let Some(x) = o.k71.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::OutputDuplication(x));
        }
        if let Some(x) = o.k72.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Resource(x));
        }
        if let Some(x) = o.k73.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::Surface(x));
        }
        if let Some(x) = o.k74.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChain(x));
        }
        if let Some(x) = o.k75.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(
                x,
            ));
        }
        if let Some(x) = o.k76.as_ref() {
            choices.push(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(x));
        }
        if choices.is_empty() {
            return false;
        }
        let at = r.below(choices.len());
        choices.swap_remove(at)
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::take_ownership(this, v1, r.value::<bool>());
    true
}

/// IDXGIOutput::ReleaseOwnership on Output, of random arguments.
fn fuzz_124(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::release_ownership(this);
    true
}

/// IDXGIOutput::GetGammaControlCapabilities on Output, of random arguments.
fn fuzz_125(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_gamma_control_capabilities(this);
    true
}

/// IDXGIOutput::SetGammaControl on Output, of random arguments.
fn fuzz_126(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::set_gamma_control(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(&r.bytes(12324)));
    true
}

/// IDXGIOutput::GetGammaControl on Output, of random arguments.
fn fuzz_127(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_gamma_control(this);
    true
}

/// IDXGIOutput::SetDisplaySurface on Output, of random arguments.
fn fuzz_128(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k73.as_ref() else {
        return false;
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::set_display_surface(this, v1);
    true
}

/// IDXGIOutput::GetDisplaySurfaceData on Output, of random arguments.
fn fuzz_129(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k73.as_ref() else {
        return false;
    };
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_display_surface_data(this, v1);
    true
}

/// IDXGIOutput::GetFrameStatistics on Output, of random arguments.
fn fuzz_130(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Output>::get_frame_statistics(this);
    true
}

/// IDXGIObject::SetPrivateData on Output, of random arguments.
fn fuzz_131(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Output, of random arguments.
fn fuzz_132(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Output, of random arguments.
fn fuzz_133(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k70.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutput<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIOutputDuplication::GetDesc on OutputDuplication, of random arguments.
fn fuzz_134(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::get_desc(this);
    true
}

/// IDXGIOutputDuplication::AcquireNextFrame on OutputDuplication, of random arguments.
fn fuzz_135(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::acquire_next_frame(this, r.value::<u32>());
    true
}

/// IDXGIOutputDuplication::GetFrameDirtyRects on OutputDuplication, of random arguments.
fn fuzz_136(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let b1 = random_bytes(r);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::get_frame_dirty_rects(this, &b1);
    true
}

/// IDXGIOutputDuplication::GetFrameMoveRects on OutputDuplication, of random arguments.
fn fuzz_137(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let b1 = random_bytes(r);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::get_frame_move_rects(this, &b1);
    true
}

/// IDXGIOutputDuplication::GetFramePointerShape on OutputDuplication, of random arguments.
fn fuzz_138(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let b1 = random_bytes(r);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::get_frame_pointer_shape(this, &b1);
    true
}

/// IDXGIOutputDuplication::UnMapDesktopSurface on OutputDuplication, of random arguments.
fn fuzz_139(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::un_map_desktop_surface(this);
    true
}

/// IDXGIOutputDuplication::ReleaseFrame on OutputDuplication, of random arguments.
fn fuzz_140(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::OutputDuplication>::release_frame(this);
    true
}

/// IDXGIObject::SetPrivateData on OutputDuplication, of random arguments.
fn fuzz_141(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on OutputDuplication, of random arguments.
fn fuzz_142(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on OutputDuplication, of random arguments.
fn fuzz_143(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k71.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemOutputDuplication<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGIResource1::CreateSubresourceSurface on Resource, of random arguments.
fn fuzz_144(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Resource1>::create_subresource_surface(this, r.value::<u32>());
    true
}

/// IDXGIResource::GetSharedHandle on Resource, of random arguments.
fn fuzz_145(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Resource>::get_shared_handle(this);
    true
}

/// IDXGIResource::GetUsage on Resource, of random arguments.
fn fuzz_146(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Resource>::get_usage(this);
    true
}

/// IDXGIResource::SetEvictionPriority on Resource, of random arguments.
fn fuzz_147(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Resource>::set_eviction_priority(this, directx_graphics_infrastructure::enumerations::ResourcePriority(r.value()));
    true
}

/// IDXGIResource::GetEvictionPriority on Resource, of random arguments.
fn fuzz_148(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Resource>::get_eviction_priority(this);
    true
}

/// IDXGIDeviceSubObject::GetDevice on Resource, of random arguments.
fn fuzz_149(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::DeviceSubObject>::get_device(this, &Default::default());
    true
}

/// IDXGIObject::SetPrivateData on Resource, of random arguments.
fn fuzz_150(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Resource, of random arguments.
fn fuzz_151(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Resource, of random arguments.
fn fuzz_152(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k72.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemResource<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGISurface2::GetResource on Surface, of random arguments.
fn fuzz_153(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Surface2>::get_resource(this, &Default::default());
    true
}

/// IDXGISurface1::GetDC on Surface, of random arguments.
fn fuzz_154(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Surface1>::get_dc(this, r.value::<bool>());
    true
}

/// IDXGISurface1::ReleaseDC on Surface, of random arguments.
fn fuzz_155(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = if r.coin() {
        Some(
            &data_directx_graphics_infrastructure::decode_windows_win32_foundation_rect(
                &r.bytes(16),
            ),
        )
    } else {
        None
    };
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Surface1>::release_dc(this, v1);
    true
}

/// IDXGISurface::GetDesc on Surface, of random arguments.
fn fuzz_156(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Surface>::get_desc(this);
    true
}

/// IDXGISurface::Unmap on Surface, of random arguments.
fn fuzz_157(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Surface>::unmap(this);
    true
}

/// IDXGIDeviceSubObject::GetDevice on Surface, of random arguments.
fn fuzz_158(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::DeviceSubObject>::get_device(this, &Default::default());
    true
}

/// IDXGIObject::SetPrivateData on Surface, of random arguments.
fn fuzz_159(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on Surface, of random arguments.
fn fuzz_160(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on Surface, of random arguments.
fn fuzz_161(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k73.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSurface<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGISwapChain4::SetHDRMetaData on SwapChain, of random arguments.
fn fuzz_162(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let v2 = if r.coin() { Some(s1.as_slice()) } else { None };
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain4>::set_hdr_meta_data(this, directx_graphics_infrastructure::enumerations::HdrMetadataType(r.value()), v2);
    true
}

/// IDXGISwapChain3::GetCurrentBackBufferIndex on SwapChain, of random arguments.
fn fuzz_163(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain3>::get_current_back_buffer_index(this);
    true
}

/// IDXGISwapChain3::CheckColorSpaceSupport on SwapChain, of random arguments.
fn fuzz_164(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain3>::check_color_space_support(this, directx_graphics_infrastructure::enumerations::ColorSpaceType(r.value()));
    true
}

/// IDXGISwapChain3::SetColorSpace1 on SwapChain, of random arguments.
fn fuzz_165(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain3>::set_color_space1(this, directx_graphics_infrastructure::enumerations::ColorSpaceType(r.value()));
    true
}

/// IDXGISwapChain2::SetSourceSize on SwapChain, of random arguments.
fn fuzz_166(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::set_source_size(this, r.value::<u32>(), r.value::<u32>());
    true
}

/// IDXGISwapChain2::GetSourceSize on SwapChain, of random arguments.
fn fuzz_167(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::get_source_size(this);
    true
}

/// IDXGISwapChain2::SetMaximumFrameLatency on SwapChain, of random arguments.
fn fuzz_168(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::set_maximum_frame_latency(this, r.value::<u32>());
    true
}

/// IDXGISwapChain2::GetMaximumFrameLatency on SwapChain, of random arguments.
fn fuzz_169(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::get_maximum_frame_latency(this);
    true
}

/// IDXGISwapChain2::GetFrameLatencyWaitableObject on SwapChain, of random arguments.
fn fuzz_170(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::get_frame_latency_waitable_object(this);
    true
}

/// IDXGISwapChain2::SetMatrixTransform on SwapChain, of random arguments.
fn fuzz_171(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::set_matrix_transform(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(&r.bytes(24)));
    true
}

/// IDXGISwapChain2::GetMatrixTransform on SwapChain, of random arguments.
fn fuzz_172(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain2>::get_matrix_transform(this);
    true
}

/// IDXGISwapChain1::GetDesc1 on SwapChain, of random arguments.
fn fuzz_173(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_desc1(this);
    true
}

/// IDXGISwapChain1::GetFullscreenDesc on SwapChain, of random arguments.
fn fuzz_174(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_fullscreen_desc(this);
    true
}

/// IDXGISwapChain1::GetHwnd on SwapChain, of random arguments.
fn fuzz_175(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_hwnd(this);
    true
}

/// IDXGISwapChain1::GetCoreWindow on SwapChain, of random arguments.
fn fuzz_176(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_core_window(this, &Default::default());
    true
}

/// IDXGISwapChain1::IsTemporaryMonoSupported on SwapChain, of random arguments.
fn fuzz_177(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::is_temporary_mono_supported(this);
    true
}

/// IDXGISwapChain1::GetRestrictToOutput on SwapChain, of random arguments.
fn fuzz_178(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_restrict_to_output(this);
    true
}

/// IDXGISwapChain1::SetBackgroundColor on SwapChain, of random arguments.
fn fuzz_179(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::set_background_color(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_dxgi_rgba(&r.bytes(16)));
    true
}

/// IDXGISwapChain1::GetBackgroundColor on SwapChain, of random arguments.
fn fuzz_180(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_background_color(this);
    true
}

/// IDXGISwapChain1::SetRotation on SwapChain, of random arguments.
fn fuzz_181(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::set_rotation(this, directx_graphics_infrastructure::enumerations::ModeRotation(r.value()));
    true
}

/// IDXGISwapChain1::GetRotation on SwapChain, of random arguments.
fn fuzz_182(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain1>::get_rotation(this);
    true
}

/// IDXGISwapChain::Present on SwapChain, of random arguments.
fn fuzz_183(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::present(this, r.value::<u32>(), directx_graphics_infrastructure::enumerations::Present(r.value()));
    true
}

/// IDXGISwapChain::GetBuffer on SwapChain, of random arguments.
fn fuzz_184(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_buffer(this, r.value::<u32>());
    true
}

/// IDXGISwapChain::SetFullscreenState on SwapChain, of random arguments.
fn fuzz_185(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::set_fullscreen_state(this, r.value::<bool>(), o.k70.as_ref().filter(|_| r.coin()));
    true
}

/// IDXGISwapChain::GetFullscreenState on SwapChain, of random arguments.
fn fuzz_186(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_fullscreen_state(this);
    true
}

/// IDXGISwapChain::GetDesc on SwapChain, of random arguments.
fn fuzz_187(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_desc(this);
    true
}

/// IDXGISwapChain::ResizeBuffers on SwapChain, of random arguments.
fn fuzz_188(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::resize_buffers(this, r.value::<u32>(), r.value::<u32>(), r.value::<u32>(), directx_graphics_infrastructure::enumerations::Format(r.value()), r.value::<u32>());
    true
}

/// IDXGISwapChain::ResizeTarget on SwapChain, of random arguments.
fn fuzz_189(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::resize_target(this, &data_directx_graphics_infrastructure::decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(&r.bytes(28)));
    true
}

/// IDXGISwapChain::GetContainingOutput on SwapChain, of random arguments.
fn fuzz_190(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_containing_output(this);
    true
}

/// IDXGISwapChain::GetFrameStatistics on SwapChain, of random arguments.
fn fuzz_191(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_frame_statistics(this);
    true
}

/// IDXGISwapChain::GetLastPresentCount on SwapChain, of random arguments.
fn fuzz_192(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::SwapChain>::get_last_present_count(this);
    true
}

/// IDXGIDeviceSubObject::GetDevice on SwapChain, of random arguments.
fn fuzz_193(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::DeviceSubObject>::get_device(this, &Default::default());
    true
}

/// IDXGIObject::SetPrivateData on SwapChain, of random arguments.
fn fuzz_194(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::Object>::set_private_data(this, &Default::default(), s1.as_slice());
    true
}

/// IDXGIObject::SetPrivateDataInterface on SwapChain, of random arguments.
fn fuzz_195(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::Object>::set_private_data_interface(this, &Default::default(), None);
    true
}

/// IDXGIObject::GetParent on SwapChain, of random arguments.
fn fuzz_196(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k74.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChain<Api> as directx_graphics_infrastructure::Object>::get_parent(this, &Default::default());
    true
}

/// IDXGISwapChainMedia::GetFrameStatisticsMedia on SwapChainMedia, of random arguments.
fn fuzz_197(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k75.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api> as directx_graphics_infrastructure::SwapChainMedia>::get_frame_statistics_media(this);
    true
}

/// IDXGISwapChainMedia::SetPresentDuration on SwapChainMedia, of random arguments.
fn fuzz_198(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k75.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api> as directx_graphics_infrastructure::SwapChainMedia>::set_present_duration(this, r.value::<u32>());
    true
}

/// IDXGISwapChainMedia::CheckPresentDurationSupport on SwapChainMedia, of random arguments.
fn fuzz_199(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k75.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemSwapChainMedia<Api> as directx_graphics_infrastructure::SwapChainMedia>::check_present_duration_support(this, r.value::<u32>());
    true
}

/// IDXGraphicsAnalysis::BeginCapture on GraphicsAnalysis, of random arguments.
fn fuzz_200(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k76.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemGraphicsAnalysis<Api> as directx_graphics_infrastructure::GraphicsAnalysis>::begin_capture(this);
    true
}

/// IDXGraphicsAnalysis::EndCapture on GraphicsAnalysis, of random arguments.
fn fuzz_201(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k76.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <directx_graphics_infrastructure_direct::SystemGraphicsAnalysis<Api> as directx_graphics_infrastructure::GraphicsAnalysis>::end_capture(this);
    true
}

/// CreateDXGIFactory, of random arguments.
fn fuzz_202(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::create_dxgi_factory(&o.api, &Default::default());
    true
}

/// CreateDXGIFactory1, of random arguments.
fn fuzz_203(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::create_dxgi_factory1(&o.api, &Default::default());
    true
}

/// CreateDXGIFactory2, of random arguments.
fn fuzz_204(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::create_dxgi_factory2(&o.api, directx_graphics_infrastructure::enumerations::CreateFactoryFlags(r.value()), &Default::default());
    true
}

/// DXGIDeclareAdapterRemovalSupport, of random arguments.
fn fuzz_205(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::dxgi_declare_adapter_removal_support(&o.api);
    true
}

/// DXGIDisableVBlankVirtualization, of random arguments.
fn fuzz_206(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::dxgi_disable_v_blank_virtualization(&o.api);
    true
}

/// DXGIGetDebugInterface1, of random arguments.
fn fuzz_207(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::dxgi_get_debug_interface1(&o.api, r.value::<u32>(), &Default::default());
    true
}

/// Drops each object but those the library's exports made, one time in `one_in`.
fn drop_some(o: &mut Objects, r: &mut Rng, one_in: usize) {}

/// Every method's fuzzer, picked from at random.
const FUZZERS: [fn(&Objects, &mut Rng) -> bool; 208] = [
    fuzz_0, fuzz_1, fuzz_2, fuzz_3, fuzz_4, fuzz_5, fuzz_6, fuzz_7, fuzz_8, fuzz_9, fuzz_10,
    fuzz_11, fuzz_12, fuzz_13, fuzz_14, fuzz_15, fuzz_16, fuzz_17, fuzz_18, fuzz_19, fuzz_20,
    fuzz_21, fuzz_22, fuzz_23, fuzz_24, fuzz_25, fuzz_26, fuzz_27, fuzz_28, fuzz_29, fuzz_30,
    fuzz_31, fuzz_32, fuzz_33, fuzz_34, fuzz_35, fuzz_36, fuzz_37, fuzz_38, fuzz_39, fuzz_40,
    fuzz_41, fuzz_42, fuzz_43, fuzz_44, fuzz_45, fuzz_46, fuzz_47, fuzz_48, fuzz_49, fuzz_50,
    fuzz_51, fuzz_52, fuzz_53, fuzz_54, fuzz_55, fuzz_56, fuzz_57, fuzz_58, fuzz_59, fuzz_60,
    fuzz_61, fuzz_62, fuzz_63, fuzz_64, fuzz_65, fuzz_66, fuzz_67, fuzz_68, fuzz_69, fuzz_70,
    fuzz_71, fuzz_72, fuzz_73, fuzz_74, fuzz_75, fuzz_76, fuzz_77, fuzz_78, fuzz_79, fuzz_80,
    fuzz_81, fuzz_82, fuzz_83, fuzz_84, fuzz_85, fuzz_86, fuzz_87, fuzz_88, fuzz_89, fuzz_90,
    fuzz_91, fuzz_92, fuzz_93, fuzz_94, fuzz_95, fuzz_96, fuzz_97, fuzz_98, fuzz_99, fuzz_100,
    fuzz_101, fuzz_102, fuzz_103, fuzz_104, fuzz_105, fuzz_106, fuzz_107, fuzz_108, fuzz_109,
    fuzz_110, fuzz_111, fuzz_112, fuzz_113, fuzz_114, fuzz_115, fuzz_116, fuzz_117, fuzz_118,
    fuzz_119, fuzz_120, fuzz_121, fuzz_122, fuzz_123, fuzz_124, fuzz_125, fuzz_126, fuzz_127,
    fuzz_128, fuzz_129, fuzz_130, fuzz_131, fuzz_132, fuzz_133, fuzz_134, fuzz_135, fuzz_136,
    fuzz_137, fuzz_138, fuzz_139, fuzz_140, fuzz_141, fuzz_142, fuzz_143, fuzz_144, fuzz_145,
    fuzz_146, fuzz_147, fuzz_148, fuzz_149, fuzz_150, fuzz_151, fuzz_152, fuzz_153, fuzz_154,
    fuzz_155, fuzz_156, fuzz_157, fuzz_158, fuzz_159, fuzz_160, fuzz_161, fuzz_162, fuzz_163,
    fuzz_164, fuzz_165, fuzz_166, fuzz_167, fuzz_168, fuzz_169, fuzz_170, fuzz_171, fuzz_172,
    fuzz_173, fuzz_174, fuzz_175, fuzz_176, fuzz_177, fuzz_178, fuzz_179, fuzz_180, fuzz_181,
    fuzz_182, fuzz_183, fuzz_184, fuzz_185, fuzz_186, fuzz_187, fuzz_188, fuzz_189, fuzz_190,
    fuzz_191, fuzz_192, fuzz_193, fuzz_194, fuzz_195, fuzz_196, fuzz_197, fuzz_198, fuzz_199,
    fuzz_200, fuzz_201, fuzz_202, fuzz_203, fuzz_204, fuzz_205, fuzz_206, fuzz_207,
];

/// Calls `calls` methods picked at random, each of random arguments; every `REMAKE_EVERY` calls drops some objects and
/// makes them again (`reseed` gives those of valid descriptions the makers cannot make). A call that panics back (the caller's own lie) is counted.
pub fn fuzz(o: &mut Objects, r: &mut Rng, calls: u64, reseed: &dyn Fn(&mut Objects)) -> Fuzzed {
    let mut fuzzed = Fuzzed::default();
    for n in 1..=calls {
        if n % REMAKE_EVERY == 0 {
            drop_some(o, r, DROP_ONE_IN);
            make_all(o);
            reseed(o);
            make_all(o);
            fuzzed.remade += 1;
        }
        let objects = &*o;
        if let Some(fuzzer) = FUZZERS.get(r.below(FUZZERS.len())) {
            fuzzed.driven.attempt(|| fuzzer(objects, r));
        }
    }
    fuzzed
}

/// How many fuzzed calls between two drops and remakes of objects.
const REMAKE_EVERY: u64 = 4096;

/// One object in this many is dropped at a remake.
const DROP_ONE_IN: usize = 4;
