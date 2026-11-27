//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The hostile implementation (src/glue_hostile.rs) of direct3d-common's traits, for tests only: every call is logged by its place, with the values
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
#[path = "../../../directx-graphics-infrastructure/implementation/direct/data.rs"]
mod data_directx_graphics_infrastructure;

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;

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
        .entry(place)
        .or_default() += 1;
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

    fn read(&self, into: &mut [u8]) -> usize {
        let n = self.call();
        into.fill(u8::try_from(n % 251).unwrap_or(0));
        into.len().saturating_add(5)
    }

    fn write(&self, bytes: &[u8]) -> usize {
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

/// A recorded object of the kind `Blob`.
pub struct HostileBlob<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileBlob<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileBlob<A> {
        HostileBlob {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileBlob<A> {
    fn clone(&self) -> HostileBlob<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileBlob<A> {
    fn eq(&self, other: &HostileBlob<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileBlob<A> {}

impl<A> Hash for HostileBlob<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileBlob<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileBlob")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DestructionNotifier`.
pub struct HostileDestructionNotifier<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDestructionNotifier<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDestructionNotifier<A> {
        HostileDestructionNotifier {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDestructionNotifier<A> {
    fn clone(&self) -> HostileDestructionNotifier<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDestructionNotifier<A> {
    fn eq(&self, other: &HostileDestructionNotifier<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDestructionNotifier<A> {}

impl<A> Hash for HostileDestructionNotifier<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDestructionNotifier<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDestructionNotifier")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this hostile implementation's.
pub trait Hostile:
    direct3d_common::Direct3DCommon<
        Blob = HostileBlob<Self>,
        DestructionNotifier = HostileDestructionNotifier<Self>,
    >
{
}

impl<A: Hostile> direct3d_common::Blob for HostileBlob<A> {
    fn get_buffer_pointer(&self) -> u8 {
        record("ID3DBlob::GetBufferPointer()".to_string());
        Default::default()
    }
    fn get_buffer_size(&self) -> usize {
        record("ID3DBlob::GetBufferSize()".to_string());
        Default::default()
    }
}

impl<A: Hostile> direct3d_common::DestructionNotifier for HostileDestructionNotifier<A> {
    fn register_destruction_callback(
        &self,
        callback_fn: usize,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = &callback_fn;
        let _ = &data;
        record("ID3DDestructionNotifier::RegisterDestructionCallback()".to_string());
        touch(data);
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(
        &self,
        callback_id: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &callback_id;
        record(format!(
            "ID3DDestructionNotifier::UnregisterDestructionCallback(callback_id={:?})",
            callback_id
        ));
        Ok(())
    }
}

/// The hostile implementation's library: `Direct3DCommon` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostileLibrary;

impl direct3d_common::Direct3DCommon for HostileLibrary {
    type Blob = HostileBlob<Self>;
    type DestructionNotifier = HostileDestructionNotifier<Self>;
}

impl Hostile for HostileLibrary {}

/// The methods a row answers for the system, and the argument it answers with (src/api.rs).
pub const ANSWERED: &[(&str, &str)] = &[("ID3D11Multithread::SetMultithreadProtected", "true")];
